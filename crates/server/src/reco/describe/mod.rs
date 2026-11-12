//! «Плейлист по описанию» — только своими средствами: разбор запроса правилами ([`parse`]), название
//! шаблонами ([`title`]), подбор — тем же, чем волна: поиск по звучанию словами (CLAP-вектор текста от
//! сервиса анализа), соседи артистов-ориентиров и их похожие у Deezer, стили Discogs из анализа
//! ([`genres`]) по всему индексу каталога, грани вкуса; отсев и оценка — правила волны ([`super::rank`])
//! с границами занятия, настроения и языка. Порядок — плавный, как «Собрать как сет»
//! ([`dalywave_transitions::set`]). Чего нет на сервере, ставится на скачивание, как у волны.
//!
//! Сервис анализа не ответил — подбор идёт без поиска по звучанию: по жанрам, ориентирам и вкусу.
//! Сохранённый плейлист — обычный плейлист человека с `rules.generator = "describe"` и понятым запросом:
//! из него — «Ещё в том же духе» и обновление раз в неделю ([`refresh_weekly`]).

pub mod genres;
pub mod parse;
#[cfg(test)]
mod parse_tests;
pub mod title;

use std::collections::{HashMap, HashSet};

use chrono::{DateTime, Utc};
use dalywave_protocol::{
    describe::{BriefArtist, DescribeBrief, DescribePreview, DescribedPlaylist},
    jobs::AcquireReason,
    library::PlaylistSummary,
    realtime::{LibraryChange, ServerEvent},
    sync::SyncEntity,
    wave::{Reason, SeedKind, WaveActivity, WaveCharacter, WaveItem, WaveSeed, WaveSettings},
};
use dalywave_transitions::set::{self, Curve, SetTrack};
use serde_json::{Value, json};
use sqlx::{PgConnection, PgPool};
use uuid::Uuid;

use super::{
    Cand, DayPart, Neighbour, Settings, candidates, nearest_fitting, nearest_semantic,
    nearest_sound,
    rank::{
        self, Constraints, Entry, Fit, LastTrack, Origin, RankContext, Scored, SessionState,
        WIDEST, admissible, constraints, fit, merge, novelty_floor, score, sequence,
        similar_threshold, violation, within_bounds,
    },
    taste,
    vectors::{Rng, weighted_mean},
    wave::{self, Env, Pick},
    widen_search,
};
use crate::{
    acquire::{self, Request},
    catalog,
    deezer::Deezer,
    error::{AppError, AppResult, bad},
    jobs::lanes,
    ml::Ml,
    realtime::{self, Audience},
};

/// `rules.generator` сохранённого плейлиста.
pub const GENERATOR: &str = "describe";
/// Описание — до стольких символов.
pub const TEXT_MAX: usize = 300;
/// Треков, если ни длительности, ни числа не назвали.
pub const DEFAULT_COUNT: usize = 25;
/// «Ещё в том же духе» — по умолчанию столько.
pub const MORE_COUNT: usize = 10;
pub const MORE_MAX: usize = 30;
/// Средняя длина трека: сколько подбирать под минуты (с запасом — лишнее срежется).
const AVG_TRACK_MS: i64 = 200_000;
/// От одного артиста — не больше (в длинных плейлистах — на один больше).
const PER_ARTIST: usize = 2;
/// Сколько первых треков качать вне очереди: «Слушать» начинается сразу.
const URGENT: usize = 3;
/// Трек «этого жанра»: сумма вероятностей подходящих стилей не меньше.
const GENRE_FIT: f32 = 0.1;
/// Трек «того жанра, которого не надо».
const GENRE_AVOID: f32 = 0.3;

/// Сколько треков подбирать под понятое: под минуты — с запасом, иначе — названное число.
pub fn target(brief: &DescribeBrief) -> usize {
    match (brief.minutes, brief.count) {
        (Some(m), _) => ((i64::from(m) * 60_000) / AVG_TRACK_MS + 4) as usize,
        (None, Some(n)) => n as usize,
        (None, None) => DEFAULT_COUNT,
    }
    .clamp(parse::COUNT.0 as usize, parse::COUNT.1 as usize + 10)
}

/// Проверка присланного обратно понятого: текст, длительность и число в границах.
pub fn check(brief: &DescribeBrief) -> AppResult<()> {
    if brief.text.chars().count() > TEXT_MAX
        || brief
            .minutes
            .is_some_and(|m| !(parse::MINUTES.0..=parse::MINUTES.1).contains(&m))
        || brief
            .count
            .is_some_and(|n| !(parse::COUNT.0..=parse::COUNT.1).contains(&n))
        || brief.genres.len() > 8
        || brief.like.len() > 6
        || brief.without.len() > 10
    {
        return Err(bad("описание не разобрать"));
    }
    Ok(())
}

// ───────────────────────────── артисты ─────────────────────────────

/// Формы имени для поиска: как написано и в именительном падеже по окончанию — «Скриптонита» →
/// «скриптонит», «Земфиры» → «земфира», «Егора Крида» → «егор крид». Лишние формы безвредны:
/// ищется точное совпадение имени.
pub fn name_forms(said: &str) -> Vec<String> {
    let base = said.to_lowercase().replace('ё', "е");
    // окончание косвенного падежа → окончания именительного
    const ENDINGS: &[(&str, &[&str])] = &[
        ("ого", &["ой", "ый"]),
        ("ому", &["ой", "ый"]),
        ("ой", &["а", "ой"]),
        ("ом", &[""]),
        ("ым", &[""]),
        ("ы", &["а"]),
        ("и", &["а", "я", "ь"]),
        ("а", &["", "ь"]),
        ("я", &["ь", "й"]),
        ("у", &["", "а"]),
        ("е", &["", "а"]),
    ];
    let cyrillic = |w: &str| w.chars().any(|c| matches!(c, 'а'..='я'));
    let alternatives = |w: &str| -> Vec<String> {
        let mut out = vec![w.to_string()];
        if !cyrillic(w) {
            return out;
        }
        if let Some((end, to)) = ENDINGS
            .iter()
            .find(|(e, _)| w.ends_with(e) && w.chars().count() >= e.chars().count() + 3)
        {
            let stem = &w[..w.len() - end.len()];
            for t in *to {
                let f = format!("{stem}{t}");
                if !out.contains(&f) {
                    out.push(f);
                }
            }
        }
        out
    };
    let mut forms: Vec<String> = vec![String::new()];
    for w in base.split_whitespace() {
        let alts = alternatives(w);
        forms = forms
            .iter()
            .flat_map(|f| {
                alts.iter().map(move |a| {
                    if f.is_empty() {
                        a.clone()
                    } else {
                        format!("{f} {a}")
                    }
                })
            })
            .take(12)
            .collect();
    }
    if !forms.contains(&base) {
        forms.insert(0, base);
    }
    forms
}

/// Артист каталога по имени: сначала среди своих (в базе), потом поиском Deezer. Похожие и популярное
/// найденного подгружаются (без ошибки, если Deezer молчит).
pub async fn find_artist(
    db: &PgPool,
    deezer: &Deezer,
    said: &str,
) -> AppResult<Option<(Uuid, String)>> {
    let forms = name_forms(said);
    let found = sqlx::query!(
        r#"SELECT id, name FROM artists
           WHERE replace(search_norm(name), 'ё', 'е') = ANY($1)
           ORDER BY (deezer_id IS NOT NULL) DESC, fans DESC NULLS LAST, id
           LIMIT 1"#,
        &forms,
    )
    .fetch_optional(db)
    .await?;
    let found = match found {
        Some(r) => Some((r.id, r.name)),
        None => {
            let hits = match catalog::search(
                db,
                deezer,
                said,
                catalog::Types {
                    track: false,
                    album: false,
                    artist: true,
                },
                5,
            )
            .await
            {
                Ok(h) => h.artists,
                Err(e) => {
                    tracing::warn!(error = %e, "artist search for a description failed");
                    Vec::new()
                }
            };
            let named = sqlx::query!("SELECT id, name FROM artists WHERE id = ANY($1)", &hits)
                .fetch_all(db)
                .await?;
            // Первый из найденных, чьё имя похоже на написанное: начало совпадает
            let head: String = forms
                .iter()
                .min_by_key(|f| f.chars().count())
                .map(|f| f.chars().take(4).collect())
                .unwrap_or_default();
            hits.iter().find_map(|id| {
                let r = named.iter().find(|r| r.id == *id)?;
                let n = r.name.to_lowercase().replace('ё', "е");
                (forms.contains(&n) || n.starts_with(&head)).then(|| (r.id, r.name.clone()))
            })
        }
    };
    if let Some((id, _)) = &found
        && let Err(e) = catalog::ensure_artist(db, deezer, *id).await
    {
        tracing::warn!(artist = %id, error = %e, "could not load the reference artist");
    }
    Ok(found)
}

/// Находит артистов-ориентиров и исключённых. «Король и Шут» разбор делит на двух артистов —
/// сначала пробуется имя целиком.
pub async fn resolve(db: &PgPool, deezer: &Deezer, brief: &mut DescribeBrief) -> AppResult<()> {
    for list in [&mut brief.like, &mut brief.without] {
        let mut out: Vec<BriefArtist> = Vec::new();
        let mut i = 0;
        while i < list.len() {
            if list[i].id.is_some() {
                out.push(list[i].clone());
                i += 1;
                continue;
            }
            if i + 1 < list.len() {
                let joined = format!("{} и {}", list[i].said, list[i + 1].said);
                if let Some((id, name)) = find_artist(db, deezer, &joined).await? {
                    out.push(BriefArtist {
                        said: joined,
                        id: Some(id),
                        name,
                    });
                    i += 2;
                    continue;
                }
            }
            let mut a = list[i].clone();
            if let Some((id, name)) = find_artist(db, deezer, &a.said).await? {
                a.id = Some(id);
                a.name = name;
            }
            out.push(a);
            i += 1;
        }
        *list = out;
    }
    Ok(())
}

// ───────────────────────────── подбор ─────────────────────────────

/// Откуда кандидат, сходство.
type Raw = (Uuid, Origin, f32);

/// Сходство по месту в списке: у поиска словами и стилей своя шкала, она приводится к шкале соседей.
fn ranked(near: &[Uuid], top: f32, span: f32, origin: &Origin) -> Vec<Raw> {
    let n = near.len().max(1) as f32;
    near.iter()
        .enumerate()
        .map(|(i, id)| (*id, origin.clone(), top - span * i as f32 / n))
        .collect()
}

fn push(out: &mut Vec<Raw>, near: Vec<Neighbour>, origin: &Origin) {
    out.extend(
        near.into_iter()
            .map(|n| (n.track_id, origin.clone(), n.similarity)),
    );
}

/// Ориентиры: средний звук их треков, подпись и id.
struct Like {
    sound: Option<Vec<f32>>,
    label: String,
    artists: Vec<Uuid>,
}

async fn like_of(db: &PgPool, brief: &DescribeBrief) -> Result<Like, sqlx::Error> {
    let artists: Vec<Uuid> = brief.like.iter().filter_map(|a| a.id).collect();
    let label = brief
        .like
        .iter()
        .map(|a| a.name.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    if artists.is_empty() {
        return Ok(Like {
            sound: None,
            label,
            artists,
        });
    }
    let vectors = sqlx::query_scalar!(
        r#"SELECT DISTINCT ON (a.recording_id) a.sound::real[] AS "sound!"
           FROM track_artists ta JOIN tracks t ON t.id = ta.track_id
           JOIN track_analysis a ON a.recording_id = t.recording_id
           WHERE ta.artist_id = ANY($1) AND ta.role = 'main' AND a.sound IS NOT NULL
           LIMIT 80"#,
        &artists,
    )
    .fetch_all(db)
    .await?;
    Ok(Like {
        sound: weighted_mean(vectors.iter().map(|v| (v.as_slice(), 1.0))),
        label,
        artists,
    })
}

/// Шаблоны стилей и жанров Deezer для жанров.
fn patterns(keys: &[String]) -> (Vec<String>, Vec<String>) {
    let gs: Vec<&genres::Genre> = keys.iter().filter_map(|k| genres::by_key(k)).collect();
    let styles = gs
        .iter()
        .flat_map(|g| g.styles.iter().map(|s| (*s).to_string()))
        .collect();
    let deezer = gs
        .iter()
        .flat_map(|g| g.deezer.iter().map(|d| format!("%{d}%")))
        .collect();
    (styles, deezer)
}

/// Треки жанра по всему индексу: стили Discogs анализа или жанр альбома у Deezer. С текстом — ближние
/// к нему по смыслу первыми, без текста — самые уверенные.
async fn by_genre(
    conn: &mut PgConnection,
    keys: &[String],
    text: Option<&[f32]>,
    limit: i64,
) -> Result<Vec<Uuid>, sqlx::Error> {
    let (styles, deezer) = patterns(keys);
    if styles.is_empty() && deezer.is_empty() {
        return Ok(Vec::new());
    }
    sqlx::query_scalar!(
        r#"WITH g AS (
               SELECT a.recording_id, a.semantic,
                      COALESCE((SELECT sum((s ->> 'p')::real) FROM jsonb_array_elements(
                                    CASE WHEN jsonb_typeof(a.styles) = 'array' THEN a.styles ELSE '[]' END) s
                                WHERE s ->> 'name' LIKE ANY($1)), 0) AS p
               FROM track_analysis a
           ),
           hits AS (
               SELECT DISTINCT ON (g.recording_id) t.id, g.p,
                      CASE WHEN $3::real[] IS NULL OR g.semantic IS NULL THEN NULL
                           ELSE g.semantic <=> $3::real[]::halfvec(512) END AS dist
               FROM g JOIN tracks t ON t.recording_id = g.recording_id
               LEFT JOIN albums al ON al.id = t.album_id
               WHERE g.p >= 0.15
                  OR EXISTS (SELECT 1 FROM unnest(COALESCE(al.genres, '{}')) x WHERE x LIKE ANY($2))
               ORDER BY g.recording_id, (t.album_id IS NULL), t.rank DESC NULLS LAST, t.id
           )
           SELECT id AS "id!" FROM hits ORDER BY dist NULLS LAST, p DESC, id LIMIT $4"#,
        &styles,
        &deezer,
        text,
        limit,
    )
    .fetch_all(conn)
    .await
}

/// Кандидаты: соседи текста (CLAP), ориентиров (звук и популярное похожих артистов), треки жанров
/// по всему индексу, соседи граней вкуса (с границами — ещё и подходящее под них), любимое и забытое.
#[allow(clippy::too_many_arguments)]
async fn pool(
    db: &PgPool,
    brief: &DescribeBrief,
    user: Uuid,
    text: Option<&[f32]>,
    like: &Like,
    facets: &[taste::Facet],
    part: DayPart,
    bounds: &Constraints,
) -> Result<Vec<Raw>, sqlx::Error> {
    let fits: Vec<Fit> = if bounds.any() {
        vec![fit(bounds, 0.0), fit(bounds, WIDEST)]
    } else {
        Vec::new()
    };
    let focused = brief.focused;
    let query = Origin::Query {
        text: brief.text.clone(),
    };
    let mut raw: Vec<Raw> = Vec::new();
    let mut tx = db.begin().await?;
    widen_search(&mut tx, 400).await?;
    if let Some(v) = text {
        let near: Vec<Uuid> = nearest_semantic(&mut tx, v, 400, user)
            .await?
            .into_iter()
            .map(|n| n.track_id)
            .collect();
        // Без конкретики соседи текста идут ниже: слова описывают скорее настроение, чем звук
        let top = if focused { 0.95 } else { 0.8 };
        raw.extend(ranked(&near, top, 0.35, &query));
    }
    if let Some(v) = &like.sound {
        let origin = Origin::Seed {
            label: like.label.clone(),
            artist: None,
        };
        push(
            &mut raw,
            nearest_sound(&mut tx, v, 250, user).await?,
            &origin,
        );
        for f in &fits {
            push(
                &mut raw,
                nearest_fitting(&mut tx, v, 150, user, f).await?,
                &origin,
            );
        }
    }
    if !brief.genres.is_empty() {
        let ids = by_genre(&mut tx, &brief.genres, text, 300).await?;
        raw.extend(ranked(&ids, 0.9, 0.3, &query));
    }
    // Вкус: без конкретики — главный источник, с ней — только подмешивается
    let budget = if focused { 60.0 } else { 400.0 };
    let total: f32 = facets.iter().map(|f| f.weight * f.lift(part, None)).sum();
    for f in facets {
        let share = f.weight * f.lift(part, None) / total.max(1e-6);
        let limit = (budget * share).round().clamp(20.0, 220.0) as i64;
        let origin = Origin::Facet {
            artists: f.artists.clone(),
        };
        push(
            &mut raw,
            nearest_sound(&mut tx, &f.centroid, limit, user).await?,
            &origin,
        );
        for k in &fits {
            let limit = (budget * 0.75 * share).round().clamp(30.0, 200.0) as i64;
            push(
                &mut raw,
                nearest_fitting(&mut tx, &f.centroid, limit, user, k).await?,
                &origin,
            );
        }
    }
    tx.commit().await?;

    if !like.artists.is_empty() {
        let origin = Origin::Seed {
            label: like.label.clone(),
            artist: None,
        };
        let related = sqlx::query_scalar!(
            r#"SELECT tt.track_id AS "id!" FROM artist_related r
               JOIN artist_top_tracks tt ON tt.artist_id = r.related_id
               WHERE r.artist_id = ANY($1) AND NOT (r.related_id = ANY($1))
               ORDER BY tt.position, r.position, tt.track_id
               LIMIT 150"#,
            &like.artists,
        )
        .fetch_all(db)
        .await?;
        raw.extend(related.into_iter().map(|id| (id, origin.clone(), 0.8)));
    }
    if !focused || brief.character == WaveCharacter::Fav {
        let favorites = sqlx::query_scalar!(
            "SELECT track_id FROM liked_tracks WHERE user_id = $1 ORDER BY md5(track_id::text || $2) LIMIT 80",
            user,
            brief.text,
        )
        .fetch_all(db)
        .await?;
        raw.extend(favorites.into_iter().map(|id| (id, Origin::Favorite, 0.55)));
        raw.extend(
            wave::forgotten(db, user, 30)
                .await?
                .into_iter()
                .map(|id| (id, Origin::Forgotten, 0.6)),
        );
    }
    if raw.is_empty() {
        // Ни анализа, ни лайков: популярное артистов из подписок
        let top = sqlx::query_scalar!(
            "SELECT tt.track_id FROM followed_artists f JOIN artist_top_tracks tt ON tt.artist_id = f.artist_id
             WHERE f.user_id = $1 ORDER BY tt.position, tt.track_id LIMIT 60",
            user,
        )
        .fetch_all(db)
        .await?;
        raw.extend(top.into_iter().map(|id| (id, Origin::Favorite, 0.5)));
    }
    Ok(raw)
}

/// Стили Discogs и жанры альбома кандидатов — для близости к жанрам.
async fn styles_of(
    db: &PgPool,
    recordings: &[Uuid],
) -> Result<HashMap<Uuid, Vec<(String, f32)>>, sqlx::Error> {
    let rows = sqlx::query!(
        r#"SELECT recording_id, styles AS "styles?" FROM track_analysis WHERE recording_id = ANY($1)"#,
        recordings,
    )
    .fetch_all(db)
    .await?;
    Ok(rows
        .into_iter()
        .map(|r| {
            let styles = r
                .styles
                .as_ref()
                .and_then(Value::as_array)
                .map(|list| {
                    list.iter()
                        .filter_map(|s| {
                            Some((
                                s.get("name")?.as_str()?.to_string(),
                                s.get("p")?.as_f64()? as f32,
                            ))
                        })
                        .collect()
                })
                .unwrap_or_default();
            (r.recording_id, styles)
        })
        .collect())
}

/// «Без мата»: чистая версия вместо explicit, если есть; explicit без неё — в отсев.
async fn explicit_swaps(
    db: &PgPool,
    ids: &[Uuid],
) -> Result<(HashMap<Uuid, Uuid>, HashSet<Uuid>), sqlx::Error> {
    let rows = sqlx::query!(
        r#"SELECT id, clean_version(id) AS clean FROM tracks WHERE id = ANY($1) AND explicit"#,
        ids
    )
    .fetch_all(db)
    .await?;
    let mut swaps = HashMap::new();
    let mut drop = HashSet::new();
    for r in rows {
        match r.clean {
            Some(c) => {
                swaps.insert(r.id, c);
            }
            None => {
                drop.insert(r.id);
            }
        }
    }
    Ok((swaps, drop))
}

/// Как подбирать.
#[derive(Debug, Default, Clone)]
pub struct Ask {
    /// Сколько треков.
    pub want: usize,
    /// Уже показанное или уже в плейлисте: не повторять.
    pub exclude: Vec<Uuid>,
    /// После какого трека — к нему подстраивается начало.
    pub after: Option<Uuid>,
    /// Только то, чего человек ещё не слышал (обновление раз в неделю).
    pub unheard: bool,
    /// Разные сборки одного описания различаются.
    pub salt: u64,
}

/// Настройки волны из понятого.
pub fn settings(brief: &DescribeBrief) -> WaveSettings {
    WaveSettings {
        activity: brief.activity,
        character: brief.character,
        mood: brief.mood,
        language: brief.language,
        seed: brief.focused.then(|| WaveSeed {
            kind: SeedKind::Text,
            id: None,
            text: Some(brief.text.clone()),
        }),
        tempo: None,
    }
}

/// Границы: занятие, настроение, язык и «без медляков» — энергия не ниже 0,45.
pub fn bounds(brief: &DescribeBrief) -> Constraints {
    let mut b = constraints(&settings(brief));
    if brief.no_slow {
        let (lo, hi) = b.energy.unwrap_or((0.0, 1.0));
        b.energy = Some((lo.max(0.45), hi.max(0.55)));
    }
    b
}

/// Вектор текста от сервиса анализа; не ответил — без него.
pub async fn text_vector(ml: &Ml, brief: &DescribeBrief) -> Option<Vec<f32>> {
    let sound = brief.sound.as_deref()?;
    match ml.embed_text(sound).await.and_then(|tv| tv.vector()) {
        Ok(v) => Some(v),
        Err(e) => {
            tracing::warn!(error = %e, "describe: sound search is unavailable, going without it");
            None
        }
    }
}

/// Подбор под понятое: кандидаты, отсев, оценка правилами волны и границами, расстановка, жанры.
/// Отдаёт треки с запасом в порядке оценки (без докачки).
pub async fn pick(
    db: &PgPool,
    brief: &DescribeBrief,
    user: Uuid,
    text: Option<&[f32]>,
    ask: &Ask,
) -> AppResult<Vec<Pick>> {
    let reco = Settings::load(db).await?;
    let now = Utc::now();
    let part = DayPart::of_hour(reco.local_hour(now));
    let facets = taste::facets(db, user).await?;
    let like = like_of(db, brief).await?;
    let bounds = bounds(brief);
    let settings = settings(brief);
    let raw = pool(db, brief, user, text, &like, &facets, part, &bounds).await?;

    let mut ids: Vec<Uuid> = Vec::with_capacity(raw.len());
    let mut seen = HashSet::new();
    for (t, _, _) in &raw {
        if seen.insert(*t) {
            ids.push(*t);
        }
    }
    let mut swaps = super::clean_swaps(db, user, &ids).await?;
    let mut dropped = HashSet::new();
    if brief.no_explicit {
        let (more, gone) = explicit_swaps(db, &ids).await?;
        swaps.extend(more);
        dropped = gone;
    }
    let ids = super::apply_swaps(&ids, &swaps);
    let cands: HashMap<Uuid, Cand> = candidates(db, &ids, user, None, None)
        .await?
        .into_iter()
        .map(|c| (c.track_id, c))
        .collect();
    let entries = merge(
        raw.into_iter()
            .filter(|(t, _, _)| !dropped.contains(t))
            .filter_map(|(t, origin, sim)| {
                Some(Entry {
                    cand: cands.get(swaps.get(&t).unwrap_or(&t))?.clone(),
                    origin,
                    sim,
                })
            })
            .collect(),
    );

    // Уже показанное — по треку и по записи; предыдущий трек — для стыка
    let excluded = sqlx::query!(
        "SELECT id, recording_id FROM tracks WHERE id = ANY($1)",
        &ask.exclude
    )
    .fetch_all(db)
    .await?;
    let mut state = SessionState::default();
    for r in &excluded {
        state.served.push(r.id);
        if let Some(rec) = r.recording_id {
            state.served.push(rec);
        }
    }
    if let Some(after) = ask.after
        && let Some(c) = candidates(db, &[after], user, None, None)
            .await?
            .into_iter()
            .next()
    {
        state.last = Some(LastTrack {
            track_id: c.track_id,
            artist_ids: c.artist_ids.clone(),
            bpm: c.bpm.filter(|_| c.bpm_confidence.unwrap_or(0.0) >= 0.3),
            key_camelot: c
                .key_camelot
                .clone()
                .filter(|_| c.key_confidence.unwrap_or(0.0) >= 0.25),
            energy: c.energy,
        });
    }
    // «Как Скриптонит» — похожее на него, а не он сам: его самого включают с его страницы
    let without: HashSet<Uuid> = brief
        .without
        .iter()
        .chain(&brief.like)
        .filter_map(|a| a.id)
        .collect();
    let without_names: Vec<String> = brief
        .without
        .iter()
        .filter(|a| a.id.is_none())
        .map(|a| a.said.to_lowercase())
        .collect();
    let ctx = RankContext {
        settings: &settings,
        state: &state,
        now,
        similar_threshold: similar_threshold(&entries),
        soundtrack_ok: facets.iter().any(taste::Facet::likes_soundtracks)
            || brief.genres.iter().any(|g| g == "soundtrack"),
    };
    let exclude = state.served_set();
    let admitted: Vec<Entry> = entries
        .into_iter()
        .filter(|e| admissible(e, &ctx, &exclude))
        .filter(|e| !ask.unheard || !e.cand.heard())
        .filter(|e| {
            !e.cand.artist_ids.iter().any(|a| without.contains(a))
                && !e.cand.artist_names.iter().any(|n| {
                    let n = n.to_lowercase();
                    without_names.iter().any(|w| n == *w || w.starts_with(&n))
                })
        })
        .collect();
    let admitted = if brief.focused {
        admitted
    } else {
        novelty_floor(admitted, ask.want * 2)
    };

    // Жанры: близость по стилям и жанру альбома
    let recordings: Vec<Uuid> = admitted
        .iter()
        .filter_map(|e| e.cand.recording_id)
        .collect();
    let styles = if brief.genres.is_empty() && brief.avoid_genres.is_empty() {
        HashMap::new()
    } else {
        styles_of(db, &recordings).await?
    };
    let none: Vec<(String, f32)> = Vec::new();
    let aff = |keys: &[String], c: &Cand| -> f32 {
        let st = c.recording_id.and_then(|r| styles.get(&r)).unwrap_or(&none);
        keys.iter()
            .filter_map(|k| genres::by_key(k))
            .map(|g| genres::affinity(g, st, &c.album_genres))
            .fold(0.0, f32::max)
    };
    let mut admitted: Vec<(Entry, f32)> = admitted
        .into_iter()
        .filter(|e| aff(&brief.avoid_genres, &e.cand) < GENRE_AVOID)
        .map(|e| {
            let a = aff(&brief.genres, &e.cand);
            (e, a)
        })
        .collect();
    if !brief.genres.is_empty() {
        // Жанр назван — держимся его, если подходящего по стилям хватает; у жанров без стилей Discogs
        // («гиперпоп») подходящего нет — тогда решают поиск по звучанию и оценка
        let fits = |(_, a): &(Entry, f32)| *a >= GENRE_FIT;
        if admitted.iter().filter(|x| fits(x)).count() >= ask.want {
            admitted.retain(fits);
        }
    }

    let mut rng = Rng::from_uuid(Uuid::now_v7(), ask.salt);
    let scored: Vec<Scored> = admitted
        .into_iter()
        .map(|(e, a)| {
            let v = violation(&e.cand, &bounds);
            let mut s = score(&e, &ctx, &mut rng) - 1.5 * v.min(1.0);
            if !brief.genres.is_empty() {
                s += 0.5 * a.min(1.0);
            }
            Scored {
                entry: e,
                score: s,
                violation: v,
            }
        })
        .collect();
    let picked = sequence(
        within_bounds(scored, ask.want, &bounds),
        ask.want * 2 + 5,
        &ctx,
    );
    // Не больше двух треков артиста (в длинных — трёх)
    let cap = if ask.want > 40 {
        PER_ARTIST + 1
    } else {
        PER_ARTIST
    };
    let mut per_artist: HashMap<Uuid, usize> = HashMap::new();
    Ok(picked
        .into_iter()
        .filter(|s| {
            let a = s
                .entry
                .cand
                .artist_ids
                .first()
                .copied()
                .unwrap_or(s.entry.cand.track_id);
            let n = per_artist.entry(a).or_default();
            *n += 1;
            *n <= cap
        })
        .map(|s| {
            let reason = rank::reason(&s.entry, &ctx);
            (s, reason)
        })
        .collect())
}

/// Длительности треков.
async fn durations(db: &PgPool, ids: &[Uuid]) -> Result<HashMap<Uuid, i64>, sqlx::Error> {
    Ok(
        sqlx::query!("SELECT id, duration_ms FROM tracks WHERE id = ANY($1)", ids)
            .fetch_all(db)
            .await?
            .into_iter()
            .map(|r| (r.id, i64::from(r.duration_ms)))
            .collect(),
    )
}

/// Сколько из подобранного оставить: под минуты — пока не наберётся длительность, иначе — число.
fn trim(
    picks: Vec<Pick>,
    brief: &DescribeBrief,
    want: usize,
    dur: &HashMap<Uuid, i64>,
) -> Vec<Pick> {
    match brief.minutes {
        Some(m) => {
            let goal = i64::from(m) * 60_000;
            let mut total = 0i64;
            let mut out = Vec::new();
            for p in picks {
                // Ближе к цели — с треком или без него
                let d = dur
                    .get(&p.0.entry.cand.track_id)
                    .copied()
                    .unwrap_or(AVG_TRACK_MS);
                if total >= goal || (total + d - goal) > (goal - total) {
                    break;
                }
                total += d;
                out.push(p);
            }
            out
        }
        None => picks.into_iter().take(want).collect(),
    }
}

fn curve(activity: WaveActivity) -> Curve {
    match activity {
        WaveActivity::Wake => Curve::Rise,
        WaveActivity::Party | WaveActivity::Road | WaveActivity::Any => Curve::Wave,
        WaveActivity::Sport | WaveActivity::Work | WaveActivity::Rest | WaveActivity::Sleep => {
            Curve::Flat
        }
    }
}

/// Плавный порядок, как «Собрать как сет»: темп, тональность, энергия. `first` — трек, после которого
/// идут новые: он остаётся первым и в ответ не попадает.
async fn smooth(
    db: &PgPool,
    picks: Vec<Pick>,
    first: Option<Uuid>,
    activity: WaveActivity,
) -> Result<Vec<Pick>, sqlx::Error> {
    if picks.len() < 3 {
        return Ok(picks);
    }
    let mut ids: Vec<Uuid> = first.into_iter().collect();
    ids.extend(picks.iter().map(|p| p.0.entry.cand.track_id));
    let rows = sqlx::query!(
        r#"SELECT t.id, a.source AS "source?", a.bpm AS "bpm?", a.key_camelot AS "key?",
                  a.key_confidence AS "key_confidence?", a.energy AS "energy?",
                  a.energy_curve AS "energy_curve?",
                  ARRAY(SELECT ta.artist_id FROM track_artists ta
                        WHERE ta.track_id = t.id AND ta.role = 'main' ORDER BY ta.artist_id) AS "artists!"
           FROM unnest($1::uuid[]) WITH ORDINALITY AS x(id, n)
           JOIN tracks t ON t.id = x.id
           LEFT JOIN track_analysis a ON a.recording_id = t.recording_id
           ORDER BY x.n"#,
        &ids,
    )
    .fetch_all(db)
    .await?;
    if rows.len() != ids.len() {
        return Ok(picks);
    }
    let mut artist_no: HashMap<Uuid, u32> = HashMap::new();
    let tracks: Vec<SetTrack> = rows
        .into_iter()
        .map(|r| {
            let artists = r
                .artists
                .iter()
                .map(|a| {
                    let n = artist_no.len() as u32;
                    *artist_no.entry(*a).or_insert(n)
                })
                .collect();
            let t = SetTrack {
                bpm: r.bpm.map(f64::from),
                key: r.key,
                key_confidence: r.key_confidence.map(f64::from),
                energy: r.energy.map(f64::from),
                head: None,
                tail: None,
                artists,
            };
            match (r.source.as_deref(), r.energy_curve) {
                (Some("full"), Some(c)) => {
                    let c: Vec<f64> = c.into_iter().map(f64::from).collect();
                    t.with_curve(&c)
                }
                _ => t,
            }
        })
        .collect();
    let options = set::Options {
        curve: curve(activity),
        first: first.map(|_| 0),
        spread_artists: true,
    };
    let order = tokio::task::spawn_blocking(move || set::arrange(&tracks, &options).order)
        .await
        .unwrap_or_default();
    let offset = usize::from(first.is_some());
    if order.len() != ids.len() {
        return Ok(picks);
    }
    let mut slots: Vec<Option<Pick>> = picks.into_iter().map(Some).collect();
    Ok(order
        .into_iter()
        .filter(|&i| i >= offset)
        .filter_map(|i| slots[i - offset].take())
        .collect())
}

/// Собрать: подбор, длительность, плавный порядок, докачка того, чего нет на сервере.
pub async fn build(
    env: &Env<'_>,
    brief: &DescribeBrief,
    user: Uuid,
    text: Option<&[f32]>,
    ask: &Ask,
    urgent: usize,
) -> AppResult<Vec<Pick>> {
    let picks = pick(env.db, brief, user, text, ask).await?;
    let ids: Vec<Uuid> = picks.iter().map(|p| p.0.entry.cand.track_id).collect();
    let dur = durations(env.db, &ids).await?;
    // Качать некому — сначала то, что уже на сервере
    let picks = if env.can_acquire {
        picks
    } else {
        let (mut ready, rest): (Vec<Pick>, Vec<Pick>) =
            picks.into_iter().partition(|p| p.0.entry.cand.ready());
        ready.extend(rest);
        ready
    };
    let kept = trim(picks, brief, ask.want, &dur);
    let ordered = smooth(env.db, kept, ask.after, brief.activity).await?;
    let n = ordered.len();
    Ok(wave::take(env, user, ordered, n, urgent).await)
}

/// Треки в виде API.
pub async fn items(db: &PgPool, user: Uuid, picks: Vec<Pick>) -> AppResult<Vec<WaveItem>> {
    wave::items(
        db,
        user,
        picks
            .into_iter()
            .map(|(s, r)| (s.entry.cand, r))
            .collect::<Vec<(Cand, Reason)>>(),
    )
    .await
}

/// Предпросмотр по описанию: разбор, ориентиры, подбор, название.
pub async fn preview(
    env: &Env<'_>,
    ml: &Ml,
    user: Uuid,
    text: &str,
    exclude: &[Uuid],
) -> AppResult<DescribePreview> {
    let text = text.trim();
    let len = text.chars().count();
    if len < 2 {
        return Err(bad("опишите плейлист словами"));
    }
    if len > TEXT_MAX {
        return Err(bad(format!("описание — не длиннее {TEXT_MAX} символов")));
    }
    let parsed = parse::parse(text);
    let mut brief = parsed.brief;
    resolve(env.db, env.deezer, &mut brief).await?;
    let (title, description) = title::compose(&brief, &parsed.phrase);
    let vector = text_vector(ml, &brief).await;
    let want = target(&brief);
    let mut ask = Ask {
        want,
        exclude: exclude.to_vec(),
        salt: exclude.len() as u64,
        ..Default::default()
    };
    let mut picks = build(env, &brief, user, vector.as_deref(), &ask, URGENT).await?;
    // «Пересобрать»: без прошлых треков подходящего мало — с ними
    if !exclude.is_empty() && picks.len() < want / 2 {
        ask.exclude.clear();
        picks = build(env, &brief, user, vector.as_deref(), &ask, URGENT).await?;
    }
    let ids: Vec<Uuid> = picks.iter().map(|p| p.0.entry.cand.track_id).collect();
    let dur = durations(env.db, &ids).await?;
    let duration_ms = ids.iter().filter_map(|id| dur.get(id)).sum();
    Ok(DescribePreview {
        items: items(env.db, user, picks).await?,
        brief,
        title,
        description,
        duration_ms,
    })
}

/// «Ещё в том же духе» по понятому.
pub async fn more(
    env: &Env<'_>,
    ml: &Ml,
    user: Uuid,
    brief: &DescribeBrief,
    exclude: &[Uuid],
    after: Option<Uuid>,
    count: usize,
) -> AppResult<Vec<Pick>> {
    let vector = text_vector(ml, brief).await;
    let brief = DescribeBrief {
        minutes: None,
        count: None,
        ..brief.clone()
    };
    let ask = Ask {
        want: count,
        exclude: exclude.to_vec(),
        after,
        salt: exclude.len() as u64,
        ..Default::default()
    };
    build(env, &brief, user, vector.as_deref(), &ask, 0).await
}

// ───────────────────────────── сохранённые ─────────────────────────────

/// Правила сохранённого плейлиста по описанию.
#[derive(Debug, Clone)]
pub struct Rules {
    pub brief: DescribeBrief,
    pub weekly: bool,
    pub refreshed_at: Option<DateTime<Utc>>,
}

pub fn rules_of(v: &Value) -> Option<Rules> {
    if v.get("generator")?.as_str()? != GENERATOR {
        return None;
    }
    Some(Rules {
        brief: serde_json::from_value(v.get("brief")?.clone()).ok()?,
        weekly: v.get("weekly").and_then(Value::as_bool).unwrap_or(false),
        refreshed_at: v
            .get("refreshedAt")
            .and_then(|x| serde_json::from_value(x.clone()).ok()),
    })
}

fn rules_json(r: &Rules) -> Value {
    json!({
        "generator": GENERATOR,
        "brief": r.brief,
        "weekly": r.weekly,
        "refreshedAt": r.refreshed_at,
    })
}

async fn changed(conn: &mut PgConnection, user: Uuid) {
    realtime::publish(
        conn,
        Audience::User(user),
        ServerEvent::LibraryChanged(LibraryChange {
            entities: vec![SyncEntity::Playlists, SyncEntity::PlaylistItems],
        }),
    )
    .await;
}

/// Чего нет на сервере — на скачивание в полосу медиатеки, в фоне.
fn acquire_later(db: PgPool, deezer: std::sync::Arc<Deezer>, user: Uuid, ids: Vec<Uuid>) {
    tokio::spawn(async move {
        let ready: HashSet<Uuid> = match sqlx::query_scalar!(
            r#"SELECT t.id FROM tracks t JOIN recordings r ON r.id = t.recording_id
               WHERE t.id = ANY($1) AND r.audio_status = 'ready'"#,
            &ids
        )
        .fetch_all(&db)
        .await
        {
            Ok(v) => v.into_iter().collect(),
            Err(e) => {
                tracing::warn!(error = %e, "describe: could not check what is on the server");
                return;
            }
        };
        for id in ids.into_iter().filter(|id| !ready.contains(id)) {
            let req = Request::new(id, AcquireReason::Playlist, lanes::LIBRARY).by(user);
            if let Err(e) = acquire::request(&db, &deezer, req).await {
                tracing::warn!(track = %id, error = %e, "describe: could not queue a track");
            }
        }
    });
}

/// Сохранить предпросмотр обычным плейлистом с понятым запросом.
#[allow(clippy::too_many_arguments)]
pub async fn save(
    db: &PgPool,
    deezer: std::sync::Arc<Deezer>,
    user: Uuid,
    brief: DescribeBrief,
    title: &str,
    description: &str,
    track_ids: &[Uuid],
    weekly: bool,
) -> AppResult<PlaylistSummary> {
    check(&brief)?;
    let title = title.trim();
    if title.is_empty() || title.chars().count() > 200 {
        return Err(bad("название плейлиста — от 1 до 200 символов"));
    }
    let description = description.trim();
    if description.chars().count() > 2000 {
        return Err(bad("описание — не длиннее 2000 символов"));
    }
    if track_ids.is_empty() || track_ids.len() > 500 {
        return Err(bad("в плейлисте — от 1 до 500 треков"));
    }
    let mut seen = HashSet::new();
    let unique: Vec<Uuid> = track_ids
        .iter()
        .copied()
        .filter(|t| seen.insert(*t))
        .collect();
    let known: HashSet<Uuid> =
        sqlx::query_scalar!("SELECT id FROM tracks WHERE id = ANY($1)", &unique)
            .fetch_all(db)
            .await?
            .into_iter()
            .collect();
    let ids: Vec<Uuid> = unique.into_iter().filter(|t| known.contains(t)).collect();
    if ids.is_empty() {
        return Err(bad("в списке нет известных треков"));
    }
    let rules = rules_json(&Rules {
        brief,
        weekly,
        refreshed_at: None,
    });
    let positions: Vec<f64> = (1..=ids.len()).map(|k| k as f64 * 1024.0).collect();
    let mut tx = db.begin().await?;
    let id = sqlx::query_scalar!(
        "INSERT INTO playlists (user_id, title, description, rules) VALUES ($1, $2, $3, $4) RETURNING id",
        user,
        title,
        description,
        rules,
    )
    .fetch_one(&mut *tx)
    .await?;
    sqlx::query!(
        "INSERT INTO playlist_items (playlist_id, track_id, position)
         SELECT $1, t, p FROM unnest($2::uuid[], $3::float8[]) AS x(t, p)",
        id,
        &ids,
        &positions,
    )
    .execute(&mut *tx)
    .await?;
    changed(&mut tx, user).await;
    tx.commit().await?;
    acquire_later(db.clone(), deezer, user, ids);
    crate::routes::playlists::summaries(db, user, Some(&[id]), false)
        .await?
        .into_iter()
        .next()
        .ok_or(AppError::NotFound)
}

/// Правила своего плейлиста по описанию; чужой, обычный или микс — «нет такого».
pub async fn rules(db: &PgPool, user: Uuid, id: Uuid) -> AppResult<Rules> {
    let row = sqlx::query!(
        r#"SELECT rules AS "rules?" FROM playlists WHERE id = $1 AND user_id = $2 AND kind = 'user'"#,
        id,
        user
    )
    .fetch_optional(db)
    .await?
    .ok_or(AppError::NotFound)?;
    row.rules
        .as_ref()
        .and_then(rules_of)
        .ok_or(AppError::Missing {
            code: "not_described",
            message: "этот плейлист собран не по описанию".into(),
        })
}

pub fn view(id: Uuid, r: Rules) -> DescribedPlaylist {
    DescribedPlaylist {
        playlist_id: id,
        brief: r.brief,
        weekly: r.weekly,
        refreshed_at: r.refreshed_at,
    }
}

/// Включить или выключить обновление раз в неделю.
pub async fn set_weekly(db: &PgPool, user: Uuid, id: Uuid, weekly: bool) -> AppResult<Rules> {
    let mut r = rules(db, user, id).await?;
    r.weekly = weekly;
    let mut tx = db.begin().await?;
    sqlx::query!(
        "UPDATE playlists SET rules = $2, updated_at = now() WHERE id = $1",
        id,
        rules_json(&r)
    )
    .execute(&mut *tx)
    .await?;
    changed(&mut tx, user).await;
    tx.commit().await?;
    Ok(r)
}

/// Треки плейлиста по порядку: (позиция, трек, когда добавлен).
async fn playlist_tracks(
    db: &PgPool,
    id: Uuid,
) -> Result<Vec<(Uuid, Uuid, f64, DateTime<Utc>)>, sqlx::Error> {
    Ok(sqlx::query!(
        "SELECT id, track_id, position, added_at FROM playlist_items WHERE playlist_id = $1 ORDER BY position, id",
        id
    )
    .fetch_all(db)
    .await?
    .into_iter()
    .map(|r| (r.id, r.track_id, r.position, r.added_at))
    .collect())
}

/// «Ещё в том же духе» в конец сохранённого плейлиста.
pub async fn extend(
    env: &Env<'_>,
    ml: &Ml,
    user: Uuid,
    id: Uuid,
    count: usize,
) -> AppResult<Vec<Uuid>> {
    let r = rules(env.db, user, id).await?;
    let current = playlist_tracks(env.db, id).await?;
    let exclude: Vec<Uuid> = current.iter().map(|x| x.1).collect();
    let after = current.last().map(|x| x.1);
    let picks = more(env, ml, user, &r.brief, &exclude, after, count).await?;
    let ids: Vec<Uuid> = picks.iter().map(|p| p.0.entry.cand.track_id).collect();
    if ids.is_empty() {
        return Ok(ids);
    }
    let start = current.last().map_or(0.0, |x| x.2);
    let positions: Vec<f64> = (1..=ids.len()).map(|k| start + k as f64 * 1024.0).collect();
    let mut tx = env.db.begin().await?;
    sqlx::query_scalar!(
        "SELECT id FROM playlists WHERE id = $1 AND user_id = $2 FOR UPDATE",
        id,
        user
    )
    .fetch_one(&mut *tx)
    .await?;
    sqlx::query!(
        "INSERT INTO playlist_items (playlist_id, track_id, position)
         SELECT $1, t, p FROM unnest($2::uuid[], $3::float8[]) AS x(t, p)",
        id,
        &ids,
        &positions,
    )
    .execute(&mut *tx)
    .await?;
    sqlx::query!("UPDATE playlists SET updated_at = now() WHERE id = $1", id)
        .execute(&mut *tx)
        .await?;
    changed(&mut tx, user).await;
    tx.commit().await?;
    Ok(ids)
}

/// Раз в неделю: в плейлистах с обновлением прослушанное после добавления меняется на новое в том же духе,
/// чего человек ещё не слышал, — на те же места. Не слушал ничего — плейлист остаётся как есть.
pub async fn refresh_weekly(env: &Env<'_>, ml: &Ml) -> AppResult<Value> {
    let due = sqlx::query!(
        r#"SELECT id, user_id, rules AS "rules!" FROM playlists
           WHERE kind = 'user' AND rules ->> 'generator' = $1 AND (rules ->> 'weekly')::boolean
             AND COALESCE((rules ->> 'refreshedAt')::timestamptz, created_at) < now() - interval '7 days'
           ORDER BY id LIMIT 50"#,
        GENERATOR,
    )
    .fetch_all(env.db)
    .await?;
    let (mut playlists, mut replaced) = (0usize, 0usize);
    for row in due {
        let Some(mut r) = rules_of(&row.rules) else {
            continue;
        };
        match refresh_one(env, ml, row.user_id, row.id, &r.brief).await {
            Ok(n) => {
                replaced += n;
                playlists += 1;
            }
            Err(e) => {
                tracing::warn!(playlist = %row.id, error = %e, "describe: weekly refresh failed");
                continue;
            }
        }
        r.refreshed_at = Some(Utc::now());
        sqlx::query!(
            "UPDATE playlists SET rules = $2 WHERE id = $1",
            row.id,
            rules_json(&r)
        )
        .execute(env.db)
        .await?;
    }
    Ok(json!({ "playlists": playlists, "replaced": replaced }))
}

/// Задача `describe_refresh` (раз в час из обслуживания воркера; срок каждого плейлиста — неделя).
pub async fn refresh_run(
    ctx: std::sync::Arc<crate::jobs::worker::Context>,
    _job: crate::jobs::JobRow,
) -> crate::jobs::worker::JobResult {
    let env = Env::new(&ctx.db, &ctx.deezer, ctx.config.fetcher.server)
        .await
        .map_err(crate::jobs::worker::JobError::retry)?;
    let report = refresh_weekly(&env, &ctx.ml)
        .await
        .map_err(crate::jobs::worker::JobError::retry)?;
    Ok(report.into())
}

async fn refresh_one(
    env: &Env<'_>,
    ml: &Ml,
    user: Uuid,
    id: Uuid,
    brief: &DescribeBrief,
) -> AppResult<usize> {
    let current = playlist_tracks(env.db, id).await?;
    if current.is_empty() {
        return Ok(0);
    }
    let track_ids: Vec<Uuid> = current.iter().map(|x| x.1).collect();
    let heard: HashSet<Uuid> = sqlx::query_scalar!(
        r#"SELECT DISTINCT i.id FROM playlist_items i
           JOIN play_events p ON p.track_id = i.track_id AND p.user_id = $2 AND p.started_at >= i.added_at
           WHERE i.playlist_id = $1"#,
        id,
        user,
    )
    .fetch_all(env.db)
    .await?
    .into_iter()
    .collect();
    // Меняется прослушанное, но не больше двух третей: плейлист остаётся узнаваемым
    let limit = (current.len() * 2).div_ceil(3);
    let old: Vec<&(Uuid, Uuid, f64, DateTime<Utc>)> = current
        .iter()
        .filter(|x| heard.contains(&x.0))
        .take(limit)
        .collect();
    if old.is_empty() {
        return Ok(0);
    }
    let vector = text_vector(ml, brief).await;
    let brief = DescribeBrief {
        minutes: None,
        count: None,
        ..brief.clone()
    };
    let ask = Ask {
        want: old.len(),
        exclude: track_ids,
        unheard: true,
        salt: Utc::now().timestamp() as u64,
        ..Default::default()
    };
    let picks = build(env, &brief, user, vector.as_deref(), &ask, 0).await?;
    let fresh: Vec<Uuid> = picks.iter().map(|p| p.0.entry.cand.track_id).collect();
    let n = fresh.len().min(old.len());
    if n == 0 {
        return Ok(0);
    }
    let remove: Vec<Uuid> = old.iter().take(n).map(|x| x.0).collect();
    let positions: Vec<f64> = old.iter().take(n).map(|x| x.2).collect();
    let mut tx = env.db.begin().await?;
    sqlx::query!(
        "DELETE FROM playlist_items WHERE playlist_id = $1 AND id = ANY($2)",
        id,
        &remove
    )
    .execute(&mut *tx)
    .await?;
    sqlx::query!(
        "INSERT INTO playlist_items (playlist_id, track_id, position)
         SELECT $1, t, p FROM unnest($2::uuid[], $3::float8[]) AS x(t, p)",
        id,
        &fresh[..n],
        &positions,
    )
    .execute(&mut *tx)
    .await?;
    sqlx::query!("UPDATE playlists SET updated_at = now() WHERE id = $1", id)
        .execute(&mut *tx)
        .await?;
    changed(&mut tx, user).await;
    tx.commit().await?;
    Ok(n)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn name_forms_drop_case_endings() {
        let has = |said: &str, form: &str| name_forms(said).iter().any(|f| f == form);
        assert!(has("Скриптонита", "скриптонит"));
        assert!(has("Мияги", "мияги"));
        assert!(has("Егора Крида", "егор крид"));
        assert!(has("Земфиры", "земфира"));
        assert!(has("Кишлака", "кишлак"));
        assert!(has("Моргенштерна", "моргенштерн"));
        assert_eq!(name_forms("Rushex"), vec!["rushex"]);
        assert_eq!(name_forms("Kai Angel"), vec!["kai angel"]);
    }

    #[test]
    fn target_follows_minutes_and_count() {
        let b = |minutes, count| DescribeBrief {
            minutes,
            count,
            ..Default::default()
        };
        assert_eq!(target(&b(None, None)), DEFAULT_COUNT);
        assert_eq!(target(&b(None, Some(15))), 15);
        assert_eq!(target(&b(Some(40), None)), 16);
        assert!(target(&b(Some(240), None)) <= 70);
    }

    #[test]
    fn no_slow_raises_the_energy_floor() {
        let mut brief = DescribeBrief::default();
        assert_eq!(bounds(&brief).energy, None);
        brief.no_slow = true;
        assert_eq!(bounds(&brief).energy, Some((0.45, 1.0)));
        brief.activity = WaveActivity::Sport;
        assert_eq!(bounds(&brief).energy, Some((0.6, 1.0)));
    }

    #[test]
    fn rules_round_trip() {
        let r = Rules {
            brief: DescribeBrief {
                text: "фонк".into(),
                genres: vec!["phonk".into()],
                ..Default::default()
            },
            weekly: true,
            refreshed_at: None,
        };
        let back = rules_of(&rules_json(&r)).unwrap();
        assert_eq!(back.brief, r.brief);
        assert!(back.weekly);
        assert!(rules_of(&json!({ "generator": "daily_mix" })).is_none());
    }
}
