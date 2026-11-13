//! Версии одной песни: оригинал, slowed, super slowed, sped up. Сервис звук не обрабатывает — он
//! находит релизы, которые выпустили сами исполнители, в своём каталоге и у Deezer.
//!
//! Что версия, а что нет, решает название: «(Slowed + Reverb)», «- Sped Up», «(Nightcore)» —
//! версии; «Slow Down», «Slowly», ремиксы, 8D — нет. Версия должна быть той же песни: то же название
//! без пометки, основной исполнитель одного трека среди исполнителей другого (или название явно
//! «Исполнитель - Песня (slowed)») и длительность, которая сходится с видом: slowed длиннее
//! оригинала, sped up короче. Найденная семья версий хранится в `track_versions` и освежается раз
//! в две недели при открытии.

use std::time::Duration;

use chrono::Utc;
use dalywave_protocol::versions::{TrackVersion, TrackVersions, VersionKind};
use sqlx::PgPool;
use unicode_normalization::UnicodeNormalization;
use uuid::Uuid;

use crate::{catalog, catalog::views, deezer::Deezer, error::AppError};

/// Через сколько версии трека ищутся у Deezer заново.
pub const CHECK_TTL: Duration = Duration::from_secs(14 * 86400);
/// Сколько треков берётся из поиска Deezer.
const SEARCH_LIMIT: u32 = 50;
/// Поиск забит другими треками (популярная песня, каверы): версии ищутся ещё и с пометкой в запросе.
const CROWDED: usize = 25;

// ───────────── Название ─────────────

/// Название трека, разобранное на песню и пометку версии.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Parsed {
    pub kind: VersionKind,
    /// Название без пометки версии и без «feat.» — для поиска.
    pub base: String,
    /// Сравнимый ключ песни: без регистра, диакритики, знаков и «feat.».
    pub key: String,
}

/// Слова, которые могут стоять в пометке версии, но вида не задают.
const FILLER: &[&str] = &[
    "and",
    "n",
    "with",
    "to",
    "perfection",
    "reverb",
    "reverbed",
    "version",
    "ver",
    "edit",
    "down",
    "a",
    "bit",
    "little",
    "tiktok",
    "the",
    "up",
];
/// Слова, усиливающие замедление: «Super», «Ultra», «Hyper», «Extremely»…
const INTENSE: &[&str] = &[
    "super",
    "ultra",
    "hyper",
    "extremely",
    "extreme",
    "extra",
    "very",
    "over",
    "mega",
    "max",
    "insanely",
    "really",
    "more",
];
/// С этих слов может начинаться пометка без скобок в конце названия: «act brazil sped up».
const HEADS: &[&str] = &[
    "slowed",
    "sped",
    "speed",
    "speeded",
    "nightcore",
    "superslowed",
    "ultraslowed",
    "spedup",
    "speedup",
    "hyperslowed",
];

fn words(s: &str) -> Vec<String> {
    fold(s)
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(str::to_string)
        .collect()
}

/// Без регистра и диакритики.
fn fold(s: &str) -> String {
    s.nfkd()
        .filter(|c| !unicode_normalization::char::is_combining_mark(*c))
        .collect::<String>()
        .to_lowercase()
}

/// Ключ для сравнения: слова через пробел.
pub fn norm(s: &str) -> String {
    words(s).join(" ")
}

/// Что за группа в конце названия.
#[derive(Debug, PartialEq, Eq)]
enum Group {
    /// Только пометка версии.
    Marker(VersionKind),
    /// Пометка и что-то ещё: «Japanese Version, Slowed» — «Japanese Version» остаётся частью песни.
    Mixed(VersionKind, String),
    /// «feat. …» — к песне не относится.
    Feat,
    /// Часть названия: «(Remix)», «(8D Audio)», «(Live)»…
    Other,
}

fn classify(text: &str) -> Group {
    let w = words(text);
    let Some(first) = w.first() else {
        return Group::Other;
    };
    if matches!(first.as_str(), "feat" | "ft" | "featuring" | "prod") {
        return Group::Feat;
    }
    // Ремикс, 8D, кавер — другая запись, а не версия скорости
    if w.iter().any(|x| {
        matches!(
            x.as_str(),
            "remix" | "rmx" | "8d" | "cover" | "live" | "acoustic" | "karaoke" | "mashup"
        )
    }) {
        return Group::Other;
    }
    let has = |x: &str| w.iter().any(|y| y == x);
    let marker_word =
        |x: &str| FILLER.contains(&x) || INTENSE.contains(&x) || HEADS.contains(&x) || x == "slow";
    // «(Slow)», «(Super Slow)» — тоже пометка, но только сама по себе: «(Slow Motion)» — название
    let slow = has("slow") && w.iter().all(|x| marker_word(x));
    let sped = has("nightcore")
        || has("spedup")
        || has("speedup")
        || ((has("sped") || has("speed") || has("speeded")) && has("up"));
    let kind = if sped {
        Some(VersionKind::SpedUp)
    } else if has("superslowed") || has("ultraslowed") || has("hyperslowed") {
        Some(VersionKind::SuperSlowed)
    } else if has("slowed") || slow {
        if w.iter().any(|x| INTENSE.contains(&x.as_str())) {
            Some(VersionKind::SuperSlowed)
        } else {
            Some(VersionKind::Slowed)
        }
    } else {
        None
    };
    let Some(kind) = kind else {
        return Group::Other;
    };
    // Остальное: части через запятую, в которых нет пометки, остаются названием.
    // Пометка вперемешку со словами в одной части («Instrumental Ultra Slowed»): слова — названием
    let kept: Vec<String> = text
        .split([',', '/', ';'])
        .map(|part| {
            let w = words(part);
            if w.iter().any(|x| HEADS.contains(&x.as_str()) || x == "slow") {
                w.into_iter()
                    .filter(|x| !marker_word(x))
                    .collect::<Vec<_>>()
                    .join(" ")
            } else {
                w.join(" ")
            }
        })
        .filter(|p| !p.is_empty())
        .collect();
    if kept.is_empty() {
        Group::Marker(kind)
    } else {
        Group::Mixed(kind, kept.join(" "))
    }
}

/// Скобочная группа в конце: (начало, внутренность).
fn trailing_group(s: &str) -> Option<(usize, &str)> {
    let close = s.chars().last()?;
    let open = match close {
        ')' => '(',
        ']' => '[',
        '}' => '{',
        _ => return None,
    };
    let mut depth = 0;
    for (i, c) in s.char_indices().rev() {
        if c == close {
            depth += 1;
        } else if c == open {
            depth -= 1;
            if depth == 0 {
                return Some((i, &s[i + open.len_utf8()..s.len() - close.len_utf8()]));
            }
        }
    }
    None
}

/// Хвост после последнего тире с пробелами: «ZAKHELO - Slowed».
fn trailing_dash(s: &str) -> Option<(usize, &str)> {
    let mut best = None;
    for sep in [" - ", " – ", " — "] {
        if let Some(i) = s.rfind(sep)
            && best.is_none_or(|(b, _)| i > b)
        {
            best = Some((i, &s[i + sep.len()..]));
        }
    }
    best.filter(|(i, _)| *i > 0)
}

/// Пометка без скобок в конце: «act brazil sped up», «Song slowed + reverb».
fn trailing_bare(s: &str) -> Option<(usize, VersionKind)> {
    let tokens: Vec<(usize, &str)> = s
        .split_whitespace()
        .map(|t| (t.as_ptr() as usize - s.as_ptr() as usize, t))
        .collect();
    for n in (1..tokens.len().min(5)).rev() {
        let start = tokens.len() - n;
        let head = norm(tokens[start].1);
        if !HEADS.contains(&head.as_str()) && !INTENSE.contains(&head.as_str()) {
            continue;
        }
        let at = tokens[start].0;
        // Без скобок — только явные «slowed», «sped up», «nightcore»: «Love Me Very Slow» — название
        if !words(&s[at..]).iter().any(|x| HEADS.contains(&x.as_str())) {
            continue;
        }
        if let Group::Marker(kind) = classify(&s[at..]) {
            return Some((at, kind));
        }
    }
    None
}

/// Разбор названия: вид версии и сама песня. `version` — версия отдельно, как её отдаёт Deezer.
pub fn parse(title: &str, version: &str) -> Parsed {
    let full = match version.trim() {
        "" => title.trim().to_string(),
        v => format!("{} {v}", title.trim()),
    };
    let mut s = full.as_str();
    let mut kind = None;
    let mut kept: Vec<String> = Vec::new();
    for _ in 0..8 {
        let t = s.trim_end();
        if let Some((at, inner)) = trailing_group(t).filter(|(at, _)| *at > 0) {
            match classify(inner) {
                Group::Marker(k) => kind = kind.or(Some(k)),
                Group::Mixed(k, rest) => {
                    kind = kind.or(Some(k));
                    kept.insert(0, rest);
                }
                Group::Feat => {}
                Group::Other => break,
            }
            s = t[..at].trim_end();
            continue;
        }
        if let Some((at, tail)) = trailing_dash(t) {
            let inner = trailing_group(tail.trim())
                .filter(|(i, _)| *i == 0)
                .map(|(_, inner)| inner)
                .unwrap_or(tail);
            match classify(inner) {
                Group::Marker(k) => kind = kind.or(Some(k)),
                Group::Mixed(k, rest) => {
                    kind = kind.or(Some(k));
                    kept.insert(0, rest);
                }
                Group::Feat | Group::Other => break,
            }
            s = t[..at].trim_end();
            continue;
        }
        if kind.is_none()
            && let Some((at, k)) = trailing_bare(t)
        {
            kind = Some(k);
            s = t[..at].trim_end();
            continue;
        }
        break;
    }
    // «Song feat. X» без скобок
    let lower = s.to_lowercase();
    let cut = [" feat. ", " feat ", " ft. ", " featuring "]
        .iter()
        .filter_map(|m| lower.find(m))
        .min();
    if let Some(i) = cut
        && s.is_char_boundary(i)
    {
        s = &s[..i];
    }
    let mut base = s.trim().to_string();
    for k in &kept {
        base.push(' ');
        base.push_str(k);
    }
    let key = norm(&base);
    if key.is_empty() {
        // Всё название — пометка («Slowed»): это просто песня с таким названием
        let key = norm(&full);
        return Parsed {
            kind: VersionKind::Original,
            base: full.trim().to_string(),
            key,
        };
    }
    Parsed {
        kind: kind.unwrap_or(VersionKind::Original),
        base,
        key,
    }
}

// ───────────── Длительность ─────────────

/// Длительность версии относительно оригинала: какой она бывает у этого вида.
fn speed(kind: VersionKind) -> (f64, f64) {
    match kind {
        VersionKind::Original => (0.97, 1.03),
        VersionKind::Slowed => (1.02, 1.6),
        VersionKind::SuperSlowed => (1.05, 2.1),
        VersionKind::SpedUp => (0.55, 0.98),
    }
}

/// Сходится ли длительность `b` с `a` для их видов: slowed длиннее оригинала, sped up короче.
pub fn plausible(a: VersionKind, a_ms: i32, b: VersionKind, b_ms: i32) -> bool {
    if a_ms <= 0 || b_ms <= 0 {
        return false;
    }
    let (a_lo, a_hi) = speed(a);
    let (b_lo, b_hi) = speed(b);
    let ratio = f64::from(b_ms) / f64::from(a_ms);
    ratio >= b_lo / a_hi && ratio <= b_hi / a_lo
}

// ───────────── Семья версий ─────────────

/// Трек каталога для сравнения.
#[derive(Debug, Clone)]
pub struct Candidate {
    pub id: Uuid,
    pub title: String,
    pub version: String,
    pub duration_ms: i32,
    pub rank: Option<i32>,
    pub ready: bool,
    /// Основные первыми.
    pub artists: Vec<Uuid>,
    pub main_name: Option<String>,
}

/// Та же ли это песня: ключ, исполнитель, длительность. Вид у версии другой.
fn same_song(cur: &Candidate, cp: &Parsed, other: &Candidate, op: &Parsed) -> bool {
    if cur.id == other.id || cp.kind == op.kind {
        return false;
    }
    let shared = cur
        .artists
        .first()
        .is_some_and(|m| other.artists.contains(m))
        || other
            .artists
            .first()
            .is_some_and(|m| cur.artists.contains(m));
    let same_key = cp.key == op.key;
    // «Исполнитель - Песня (slowed)» у загрузившего её канала
    let named = |a: &Candidate, ap: &Parsed, b: &Parsed| {
        a.main_name
            .as_deref()
            .is_some_and(|n| b.key == format!("{} {}", norm(n), ap.key))
    };
    let matches = (same_key && shared) || named(cur, cp, op) || named(other, op, cp);
    matches && plausible(cp.kind, cur.duration_ms, op.kind, other.duration_ms)
}

/// Лучшая версия каждого вида для трека: на сервере — первой, затем того же основного
/// исполнителя, затем популярная. Свой вид занимает сам трек.
pub fn pick(cur: &Candidate, others: &[Candidate]) -> Vec<(VersionKind, Uuid)> {
    let cp = parse(&cur.title, &cur.version);
    let mut best: Vec<(VersionKind, &Candidate)> = Vec::new();
    for o in others {
        let op = parse(&o.title, &o.version);
        if !same_song(cur, &cp, o, &op) {
            continue;
        }
        let score = |c: &Candidate| {
            (
                c.ready,
                c.artists.first() == cur.artists.first(),
                c.rank.unwrap_or(0),
                std::cmp::Reverse(c.id),
            )
        };
        match best.iter_mut().find(|(k, _)| *k == op.kind) {
            Some(slot) if score(o) > score(slot.1) => slot.1 = o,
            Some(_) => {}
            None => best.push((op.kind, o)),
        }
    }
    let mut out: Vec<(VersionKind, Uuid)> = best.into_iter().map(|(k, c)| (k, c.id)).collect();
    out.sort();
    out
}

// ───────────── База и Deezer ─────────────

async fn candidate(db: &PgPool, id: Uuid) -> Result<Option<Candidate>, sqlx::Error> {
    let rows = candidates(db, &[id], "", &[]).await?;
    Ok(rows.into_iter().next())
}

/// Треки по id и треки исполнителей, чьё название начинается с `prefix`.
async fn candidates(
    db: &PgPool,
    ids: &[Uuid],
    prefix: &str,
    artists: &[Uuid],
) -> Result<Vec<Candidate>, sqlx::Error> {
    let like = format!(
        "{}%",
        prefix
            .replace('\\', "\\\\")
            .replace('%', "\\%")
            .replace('_', "\\_")
    );
    let rows = sqlx::query!(
        r#"SELECT t.id, t.title, t.title_version, t.duration_ms, t.rank,
                  COALESCE(r.audio_status = 'ready', false) AS "ready!",
                  ARRAY(SELECT ta.artist_id FROM track_artists ta WHERE ta.track_id = t.id
                        ORDER BY ta.position) AS "artists!",
                  (SELECT a.name FROM track_artists ta JOIN artists a ON a.id = ta.artist_id
                   WHERE ta.track_id = t.id ORDER BY ta.position LIMIT 1) AS main_name
           FROM tracks t
           LEFT JOIN recordings r ON r.id = t.recording_id
           WHERE t.id = ANY($1)
              OR ($2 <> '%' AND search_norm(t.title) LIKE search_norm($2)
                  AND EXISTS (SELECT 1 FROM track_artists ta
                              WHERE ta.track_id = t.id AND ta.artist_id = ANY($3)))
           LIMIT 300"#,
        ids,
        like,
        artists,
    )
    .fetch_all(db)
    .await?;
    Ok(rows
        .into_iter()
        .map(|r| Candidate {
            id: r.id,
            title: r.title,
            version: r.title_version,
            duration_ms: r.duration_ms,
            rank: r.rank,
            ready: r.ready,
            artists: r.artists,
            main_name: r.main_name,
        })
        .collect())
}

/// Поиск у Deezer: найденное сохраняется в каталог, возвращаются наши id. `None` — Deezer не ответил.
async fn search(db: &PgPool, deezer: &Deezer, artist: &str, base: &str) -> Option<Vec<Uuid>> {
    let q = format!("{artist} {base}");
    let mut found = match deezer.search_tracks(&q, SEARCH_LIMIT).await {
        Ok(found) => found,
        Err(e) => {
            tracing::warn!(error = %e, "versions: Deezer search failed");
            return None;
        }
    };
    if found.len() >= CROWDED {
        for extra in ["slowed", "sped up"] {
            match deezer
                .search_tracks(&format!("{q} {extra}"), SEARCH_LIMIT)
                .await
            {
                Ok(more) => found.extend(more),
                Err(e) => tracing::warn!(error = %e, "versions: Deezer search failed"),
            }
        }
    }
    // Сохраняется только похожее по названию: чужие песни из выдачи каталогу не нужны
    let key = norm(base);
    let first = key.split(' ').next().unwrap_or_default().to_string();
    found.retain(|t| {
        let (title, version) = t.split_title();
        let p = parse(&title, &version);
        p.key.starts_with(&first) || p.key.contains(&key)
    });
    match catalog::save_listed(db, &found).await {
        Ok(ids) => Some(ids),
        Err(e) => {
            tracing::warn!(error = %e, "versions: could not save Deezer results");
            None
        }
    }
}

/// Пересчитать семью версий трека: каталог и, если `deezer` есть, поиск у Deezer.
pub async fn refresh(
    db: &PgPool,
    deezer: Option<&Deezer>,
    cur: &Candidate,
) -> Result<Vec<(VersionKind, Uuid)>, AppError> {
    let parsed = parse(&cur.title, &cur.version);
    let mut ids = Vec::new();
    let mut searched = false;
    if let (Some(deezer), Some(artist)) = (deezer, cur.main_name.as_deref())
        && let Some(found) = search(db, deezer, artist, &parsed.base).await
    {
        ids = found;
        searched = true;
    }
    let first = norm(&parsed.base)
        .split(' ')
        .next()
        .unwrap_or_default()
        .to_string();
    let others = candidates(db, &ids, &first, &cur.artists).await?;
    let picked = pick(cur, &others);
    let own = parsed.kind;
    let mut family: Vec<(VersionKind, Uuid)> = picked.clone();
    family.push((own, cur.id));

    let mut tx = db.begin().await?;
    for (_, member) in &family {
        sqlx::query!("DELETE FROM track_versions WHERE track_id = $1", member)
            .execute(&mut *tx)
            .await?;
        let (kinds, others): (Vec<String>, Vec<Uuid>) = family
            .iter()
            .filter(|(_, id)| id != member)
            .map(|(k, id)| (k.as_str().to_string(), *id))
            .unzip();
        sqlx::query!(
            "INSERT INTO track_versions (track_id, version_id, kind)
             SELECT $1, v, k FROM unnest($2::uuid[], $3::text[]) AS x(v, k)
             ON CONFLICT DO NOTHING",
            member,
            &others,
            &kinds,
        )
        .execute(&mut *tx)
        .await?;
        if searched {
            sqlx::query!(
                "INSERT INTO track_version_checks (track_id) VALUES ($1)
                 ON CONFLICT (track_id) DO UPDATE SET checked_at = now()",
                member,
            )
            .execute(&mut *tx)
            .await?;
        }
    }
    tx.commit().await?;
    Ok(picked)
}

/// Версии песни трека для слушателя. Свежие связи — из базы, старые — пересчитываются.
pub async fn versions(
    db: &PgPool,
    deezer: &Deezer,
    track_id: Uuid,
    user_id: Uuid,
) -> Result<TrackVersions, AppError> {
    let cur = candidate(db, track_id).await?.ok_or(AppError::NotFound)?;
    let own = parse(&cur.title, &cur.version).kind;
    let checked = sqlx::query_scalar!(
        "SELECT checked_at FROM track_version_checks WHERE track_id = $1",
        track_id
    )
    .fetch_optional(db)
    .await?;
    let fresh = checked.is_some_and(|at| {
        Utc::now()
            .signed_duration_since(at)
            .to_std()
            .unwrap_or_default()
            < CHECK_TTL
    });
    let links: Vec<(VersionKind, Uuid)> = if fresh {
        sqlx::query!(
            "SELECT version_id, kind FROM track_versions WHERE track_id = $1",
            track_id
        )
        .fetch_all(db)
        .await?
        .into_iter()
        .filter_map(|r| Some((r.kind.parse().ok()?, r.version_id)))
        .collect()
    } else {
        refresh(db, Some(deezer), &cur).await?
    };

    let mut family = links;
    family.retain(|(k, _)| *k != own);
    family.push((own, track_id));
    family.sort();
    let ids: Vec<Uuid> = family.iter().map(|(_, id)| *id).collect();
    let tracks = views::tracks(db, &ids, user_id).await?;
    let items = family
        .iter()
        .filter_map(|(kind, id)| {
            let track = tracks.iter().find(|t| t.id == *id)?.clone();
            Some(TrackVersion { kind: *kind, track })
        })
        .collect();
    let preferred = match cur.artists.first() {
        Some(artist) => sqlx::query_scalar!(
            "SELECT kind FROM version_choices WHERE user_id = $1 AND artist_id = $2",
            user_id,
            artist,
        )
        .fetch_optional(db)
        .await?
        .and_then(|k| k.parse().ok()),
        None => None,
    };
    Ok(TrackVersions {
        track_id,
        kind: own,
        items,
        preferred,
    })
}

/// Запомнить выбор: у основного исполнителя трека играть этот вид. Оригинал — забыть выбор.
pub async fn choose(
    db: &PgPool,
    user_id: Uuid,
    track_id: Uuid,
    kind: VersionKind,
) -> Result<(), AppError> {
    let artist = sqlx::query_scalar!(
        "SELECT artist_id FROM track_artists WHERE track_id = $1 ORDER BY position LIMIT 1",
        track_id
    )
    .fetch_optional(db)
    .await?
    .ok_or(AppError::NotFound)?;
    if kind == VersionKind::Original {
        sqlx::query!(
            "DELETE FROM version_choices WHERE user_id = $1 AND artist_id = $2",
            user_id,
            artist
        )
        .execute(db)
        .await?;
    } else {
        sqlx::query!(
            "INSERT INTO version_choices (user_id, artist_id, kind) VALUES ($1, $2, $3)
             ON CONFLICT (user_id, artist_id) DO UPDATE SET kind = EXCLUDED.kind, chosen_at = now()",
            user_id,
            artist,
            kind.as_str(),
        )
        .execute(db)
        .await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use VersionKind::*;

    fn kind(title: &str, version: &str) -> VersionKind {
        parse(title, version).kind
    }

    #[test]
    fn versions_by_title() {
        let cases: &[(&str, &str, VersionKind, &str)] = &[
            ("MONTAGEM RANHURA", "(Slowed)", Slowed, "montagem ranhura"),
            ("MONTAGEM RANHURA (Slowed)", "", Slowed, "montagem ranhura"),
            ("ZAKHELO - Slowed", "", Slowed, "zakhelo"),
            ("ZAKHELO", "(Ultra Slowed)", SuperSlowed, "zakhelo"),
            ("ZAKHELO", "(Hyper Slowed)", SuperSlowed, "zakhelo"),
            ("ZAKHELO", "(Sped Up)", SpedUp, "zakhelo"),
            ("171 - Super Slowed", "", SuperSlowed, "171"),
            ("4MIN TEKK - (Slowed)", "", Slowed, "4min tekk"),
            ("2 PHUT FUNK", "(Slowed Reverb)", Slowed, "2 phut funk"),
            (
                "4444live",
                "(Super Slowed + Reverb)",
                SuperSlowed,
                "4444live",
            ),
            ("Aria Math", "(slowed + reverb)", Slowed, "aria math"),
            ("€UROPHONK (SPED UP)", "", SpedUp, "urophonk"),
            ("act brazil (sped up)", "", SpedUp, "act brazil"),
            ("act brazil sped up", "", SpedUp, "act brazil"),
            (
                "67 Analog Horror Phonk",
                "(Speed Up)",
                SpedUp,
                "67 analog horror phonk",
            ),
            ("ASTRAL FUNK (SuperSlowed)", "", SuperSlowed, "astral funk"),
            (
                "ALMA MORTA",
                "(Extremely Slowed)",
                SuperSlowed,
                "alma morta",
            ),
            ("ATENTO! 2", "(Over Slowed)", SuperSlowed, "atento 2"),
            (
                "Amor Na Praia",
                "(Slowed to Perfection)",
                Slowed,
                "amor na praia",
            ),
            ("ACELERO", "(Slowed Version)", Slowed, "acelero"),
            ("3days (feat. Mitchel Dae) (Nightcore)", "", SpedUp, "3days"),
            (
                "AS NOVINHAS DO JACARE X BEAT PORRADEIRO - SLOWED",
                "",
                Slowed,
                "as novinhas do jacare x beat porradeiro",
            ),
            (
                "Assombração Eterna 2.0 - Slowed",
                "",
                Slowed,
                "assombracao eterna 2 0",
            ),
            ("DANÇA NO BAILÃO", "(Slowed)", Slowed, "danca no bailao"),
            (
                "Aria Math (Phonk Remix) (Slowed)",
                "",
                Slowed,
                "aria math phonk remix",
            ),
            (
                "ACIDO",
                "(Japanese Version, Slowed)",
                Slowed,
                "acido japanese version",
            ),
            (
                "Aria Math (Funk Remix) (Instrumental Ultra Slowed)",
                "",
                SuperSlowed,
                "aria math funk remix instrumental",
            ),
            ("Song (Slowed) [feat. Someone]", "", Slowed, "song"),
            ("Song feat. Someone - Sped Up", "", SpedUp, "song"),
            ("Bota", "(Slow)", Slowed, "bota"),
            ("Bota (Super Slow)", "", SuperSlowed, "bota"),
            ("CAOS INFERNAL", "(Slowed++)", Slowed, "caos infernal"),
            (
                "Sex, Drugs, Etc.",
                "(Sped Up Version)",
                SpedUp,
                "sex drugs etc",
            ),
            (
                "Headlights (feat. KIDDO)",
                "(Slowed Version)",
                Slowed,
                "headlights",
            ),
            ("i love you", "(nightcore)", SpedUp, "i love you"),
            ("Heavenly", "(Over Slowed)", SuperSlowed, "heavenly"),
            (
                "Funk of Galáctico (VØJ Remix)",
                "(Super Slowed)",
                SuperSlowed,
                "funk of galactico vøj remix",
            ),
        ];
        for (title, version, want, key) in cases {
            let p = parse(title, version);
            assert_eq!(p.kind, *want, "{title} {version}");
            assert_eq!(p.key, *key, "{title} {version}");
        }
    }

    #[test]
    fn not_versions() {
        for (title, version) in [
            ("Slow Down", ""),
            ("Slowly", ""),
            ("Slow Motion", ""),
            ("Speed of Light", ""),
            ("7VNSPEED", ""),
            ("Ai Ai Voce Me da Muito Calor Speed", ""),
            ("ASTRO", "(Slowed Opium Remix)"),
            ("Song", "(Remix)"),
            ("Song", "(8D Audio)"),
            ("Song (Slowed + 8D)", ""),
            ("Song", "(Live)"),
            ("Song", "(Reverb)"),
            ("Slowed", ""),
            ("Love Me Down", ""),
            ("Sped Up", ""),
            ("Take It Slow", ""),
            ("Love Me Very Slow", ""),
            ("Rise Slow", ""),
            ("Song", "(Slow Motion)"),
        ] {
            assert_eq!(kind(title, version), Original, "{title} {version}");
        }
        // Ремикс остаётся частью песни: у «Song (Remix)» своя семья
        assert_eq!(parse("Song", "(Remix)").key, "song remix");
        assert_eq!(
            parse("ASTRO", "(Slowed Opium Remix)").key,
            "astro slowed opium remix"
        );
        assert_eq!(parse("Slowed", "").key, "slowed");
        // «Slowed Loop» — замедленная, но другая нарезка: с оригиналом не сойдётся
        assert_eq!(
            parse("Golden Brown", "(Slowed Loop)").key,
            "golden brown loop"
        );
    }

    #[test]
    fn durations_fit_the_kind() {
        // ALMA MORTA: оригинал 108 с, slowed 119, super 130, extremely 154, sped up 100
        assert!(plausible(Original, 108_000, Slowed, 119_000));
        assert!(plausible(Original, 108_000, SuperSlowed, 154_000));
        assert!(plausible(Original, 108_000, SpedUp, 100_000));
        assert!(plausible(Slowed, 119_000, Original, 108_000));
        assert!(plausible(Slowed, 119_000, SpedUp, 100_000));
        // KARVENORA: 143 → slowed 152
        assert!(plausible(Original, 143_000, Slowed, 152_000));
        // Slowed короче оригинала — другая песня или обрезка
        assert!(!plausible(Original, 200_000, Slowed, 150_000));
        assert!(!plausible(Original, 100_000, Slowed, 200_000));
        // Sped up длиннее оригинала — не он
        assert!(!plausible(Original, 100_000, SpedUp, 110_000));
        assert!(!plausible(Original, 200_000, SpedUp, 90_000));
        assert!(!plausible(SpedUp, 100_000, Slowed, 95_000));
        assert!(!plausible(Original, 0, Slowed, 95_000));
    }

    fn c(id: u128, title: &str, version: &str, secs: i32, artists: &[u128]) -> Candidate {
        Candidate {
            id: Uuid::from_u128(id),
            title: title.into(),
            version: version.into(),
            duration_ms: secs * 1000,
            rank: None,
            ready: false,
            artists: artists.iter().map(|a| Uuid::from_u128(*a)).collect(),
            main_name: Some(format!("artist{}", artists[0])),
        }
    }

    #[test]
    fn family_is_picked() {
        let cur = c(1, "ZAKHELO", "", 117, &[10]);
        let others = vec![
            c(2, "ZAKHELO", "(Slowed)", 129, &[10]),
            c(3, "ZAKHELO", "(Ultra Slowed)", 141, &[10]),
            c(4, "ZAKHELO", "(Hyper Slowed)", 156, &[10]),
            c(5, "ZAKHELO", "(Sped Up)", 109, &[10]),
            // Чужой «ускоритель» без исполнителя оригинала — мимо
            c(6, "ZAKHELO", "(Slowed + Reverb)", 135, &[99]),
            // Ремикс и та же запись в другом альбоме — не версии
            c(7, "ZAKHELO", "(Remix)", 120, &[10]),
            c(8, "ZAKHELO", "", 117, &[10]),
            // Slowed, а короче оригинала — не то
            c(9, "ZAKHELO - Slowed", "", 90, &[10]),
        ];
        let mut ready = others.clone();
        ready[2].ready = true; // Hyper на сервере — им и закрывается super slowed
        let picked = pick(&cur, &ready);
        assert_eq!(
            picked,
            vec![
                (Slowed, Uuid::from_u128(2)),
                (SuperSlowed, Uuid::from_u128(4)),
                (SpedUp, Uuid::from_u128(5)),
            ]
        );
        // Обратно: от slowed — оригинал и sped up
        let from_slowed = pick(&others[0], &[cur.clone(), others[3].clone()]);
        assert_eq!(
            from_slowed,
            vec![(Original, Uuid::from_u128(1)), (SpedUp, Uuid::from_u128(5))]
        );
        // Совместный трек: основной исполнитель оригинала среди исполнителей версии
        let feat = c(20, "TOMANADA", "(Sped Up)", 101, &[30, 10, 31]);
        let orig = c(21, "TOMANADA", "", 107, &[10]);
        assert_eq!(pick(&orig, &[feat]), vec![(SpedUp, Uuid::from_u128(20))]);
        // Канал-«ускоритель» с явным «Исполнитель - Песня (slowed)»
        let mut upload = c(40, "artist10 - ZAKHELO", "(slowed)", 130, &[77]);
        upload.main_name = Some("Slowed Channel".into());
        assert_eq!(pick(&cur, &[upload]), vec![(Slowed, Uuid::from_u128(40))]);
    }
}
