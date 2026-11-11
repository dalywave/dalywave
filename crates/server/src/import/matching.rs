//! Сопоставление позиций импорта с каталогом Deezer. ISRC в архиве Spotify нет, поэтому трек ищется
//! по артисту, названию и альбому: сначала в своём каталоге, затем поиском Deezer (точный запрос
//! `artist:"…" track:"…"`, потом свободный). Кандидаты оцениваются одной функцией [`score_track`]:
//! название без версии и «feat.», артисты (с транслитерацией: «Скриптонит» = «Skriptonit»), альбом,
//! длительность; несовпавшая версия (live, remix, slowed, акустика…) — штраф.

use std::collections::HashSet;

use dalywave_protocol::imports::ImportCandidate;
use unicode_normalization::{UnicodeNormalization, char::is_combining_mark};

use crate::deezer::{Deezer, DeezerError, types as dz};

/// С этой оценки трек считается найденным без проверки человеком.
pub const ACCEPT: f32 = 0.78;
/// Ниже — даже в подсказки не годится.
pub const SUGGEST: f32 = 0.45;
/// Сколько кандидатов хранить для проверки.
pub const KEEP_CANDIDATES: usize = 5;

// ───────────── Нормализация ─────────────

/// Строка для сравнения: нижний регистр, без диакритики, «ё» = «е», «&» = «and», без знаков.
pub fn normalize(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut space = true;
    for c in s.nfkd().filter(|c| !is_combining_mark(*c)) {
        let c = match c {
            'ё' | 'Ё' => 'е',
            '&' | '+' => {
                if !space {
                    out.push(' ');
                }
                out.push_str("and");
                space = false;
                continue;
            }
            c => c,
        };
        if c.is_alphanumeric() {
            out.extend(c.to_lowercase());
            space = false;
        } else if !space {
            out.push(' ');
            space = true;
        }
    }
    // «й» после NFKD теряет кратку — это не страшно: так же теряется с обеих сторон
    if out.ends_with(' ') {
        out.pop();
    }
    out
}

/// Кириллица латиницей — чтобы «Скриптонит» и «Skriptonit» сошлись.
pub fn translit(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        let t = match c {
            'а' => "a",
            'б' => "b",
            'в' => "v",
            'г' => "g",
            'д' => "d",
            'е' | 'э' => "e",
            'ж' => "zh",
            'з' => "z",
            'и' | 'й' | 'ы' => "i",
            'к' => "k",
            'л' => "l",
            'м' => "m",
            'н' => "n",
            'о' => "o",
            'п' => "p",
            'р' => "r",
            'с' => "s",
            'т' => "t",
            'у' => "u",
            'ф' => "f",
            'х' => "h",
            'ц' => "ts",
            'ч' => "ch",
            'ш' => "sh",
            'щ' => "sch",
            'ъ' | 'ь' => "",
            'ю' => "yu",
            'я' => "ya",
            _ => {
                out.push(c);
                continue;
            }
        };
        out.push_str(t);
    }
    // Латиница пишется по-разному: «y» и «i», «kh» и «h» — сводим к одному
    out.replace('y', "i").replace("kh", "h").replace("ks", "x")
}

/// Метки версий, которые делают запись другой.
const VERSION_TAGS: [(&str, &str); 22] = [
    ("live", "live"),
    ("концерт", "live"),
    ("unplugged", "live"),
    ("remix", "remix"),
    ("rmx", "remix"),
    ("ремикс", "remix"),
    ("rework", "remix"),
    ("bootleg", "remix"),
    ("vip", "remix"),
    ("slowed", "slowed"),
    ("замедл", "slowed"),
    ("sped up", "sped"),
    ("speed up", "sped"),
    ("nightcore", "sped"),
    ("acoustic", "acoustic"),
    ("акустик", "acoustic"),
    ("instrumental", "instrumental"),
    ("инструментал", "instrumental"),
    ("karaoke", "karaoke"),
    ("караоке", "karaoke"),
    ("cover", "cover"),
    ("demo", "demo"),
];

/// Название без версии и «feat.» и множество меток версии.
pub fn split_title(title: &str) -> (String, HashSet<&'static str>) {
    let lower = title.to_lowercase();
    let mut tags = HashSet::new();
    for (marker, tag) in VERSION_TAGS {
        if contains_word(&lower, marker) {
            tags.insert(tag);
        }
    }
    // «… (Original Mix)» — не ремикс
    if lower.contains("original mix") || lower.contains("extended mix") {
        tags.remove("remix");
    }
    // Всё в скобках и после « - » — версия или участники; основа — то, что до
    let base = normalize(&title[..base_end(title)]);
    (
        if base.is_empty() {
            normalize(title)
        } else {
            base
        },
        tags,
    )
}

/// Где кончается основа названия. Поиск — по ASCII-нижнему регистру: он не меняет длину строки,
/// поэтому индекс годится и для исходной.
fn base_end(title: &str) -> usize {
    let lower = title.to_ascii_lowercase();
    [
        " - ",
        " – ",
        " — ",
        "(",
        "[",
        " feat. ",
        " feat ",
        " ft. ",
        " featuring ",
    ]
    .iter()
    .filter_map(|sep| lower.find(sep))
    .filter(|i| *i > 0)
    .min()
    .unwrap_or(title.len())
}

fn contains_word(hay: &str, needle: &str) -> bool {
    hay.match_indices(needle).any(|(i, _)| {
        let before = hay[..i].chars().next_back();
        let after = hay[i + needle.len()..].chars().next();
        // У русских меток важна только граница слева: «замедленная», «акустика»
        let cyr = needle.chars().any(|c| ('а'..='я').contains(&c));
        before.is_none_or(|c| !c.is_alphanumeric())
            && (cyr || after.is_none_or(|c| !c.is_alphanumeric()))
    })
}

/// Имена артистов из строки: «A, B», «A & B», «A feat. B», «A x B».
pub fn split_artists(s: &str) -> Vec<String> {
    let lower = s.to_lowercase();
    let mut parts = vec![lower.clone()];
    for sep in [
        ",",
        " & ",
        " feat. ",
        " feat ",
        " ft. ",
        " featuring ",
        " x ",
        " и ",
        ";",
        " / ",
    ] {
        parts = parts
            .iter()
            .flat_map(|p| p.split(sep).map(str::to_string).collect::<Vec<_>>())
            .collect();
    }
    let mut out: Vec<String> = parts
        .iter()
        .map(|p| normalize(p))
        .filter(|p| !p.is_empty())
        .collect();
    let whole = normalize(s);
    if !whole.is_empty() && !out.contains(&whole) {
        out.push(whole);
    }
    out
}

fn levenshtein(a: &[char], b: &[char]) -> usize {
    if a.is_empty() {
        return b.len();
    }
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut cur = vec![0; b.len() + 1];
    for (i, ca) in a.iter().enumerate() {
        cur[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let cost = usize::from(ca != cb);
            cur[j + 1] = (prev[j] + cost).min(prev[j + 1] + 1).min(cur[j] + 1);
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    prev[b.len()]
}

fn similar_once(a: &str, b: &str) -> f32 {
    if a == b {
        return 1.0;
    }
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }
    let ac: Vec<char> = a.chars().collect();
    let bc: Vec<char> = b.chars().collect();
    let longest = ac.len().max(bc.len());
    let lev = 1.0 - levenshtein(&ac, &bc) as f32 / longest as f32;
    let ta: HashSet<&str> = a.split(' ').collect();
    let tb: HashSet<&str> = b.split(' ').collect();
    let jaccard = ta.intersection(&tb).count() as f32 / ta.union(&tb).count().max(1) as f32;
    // Одно целиком внутри другого: «Devil» и «Devil Inside» похожи, но не равны
    let shorter = ac.len().min(bc.len());
    let contained = if shorter >= 4 && (a.contains(b) || b.contains(a)) {
        0.6 + 0.3 * shorter as f32 / longest as f32
    } else {
        0.0
    };
    lev.max(jaccard).max(contained)
}

/// Сходство двух уже нормализованных строк 0..1, с транслитерацией.
pub fn similar(a: &str, b: &str) -> f32 {
    let direct = similar_once(a, b);
    if direct >= 1.0 {
        return direct;
    }
    direct.max(similar_once(&translit(a), &translit(b)))
}

// ───────────── Оценка ─────────────

/// Что ищем.
#[derive(Debug, Clone, Default)]
pub struct Wanted {
    pub artist: String,
    pub title: String,
    pub album: Option<String>,
    pub duration_ms: Option<i32>,
    pub isrc: Option<String>,
}

/// Кандидат: трек каталога или ответа Deezer.
#[derive(Debug, Clone, Default)]
pub struct Cand {
    pub deezer_id: i64,
    pub title: String,
    pub version: String,
    pub artists: Vec<String>,
    pub album: Option<String>,
    pub duration_ms: Option<i32>,
    pub isrc: Option<String>,
    pub rank: i32,
}

impl Cand {
    pub fn from_deezer(t: &dz::Track) -> Self {
        let mut artists: Vec<String> = t.artist.iter().map(|a| a.name.clone()).collect();
        artists.extend(t.contributors.iter().map(|a| a.name.clone()));
        artists.dedup();
        Self {
            deezer_id: t.id,
            title: t.title.clone(),
            version: t.title_version.clone().unwrap_or_default(),
            artists,
            album: t.album.as_ref().map(|a| a.title.clone()),
            duration_ms: Some(t.duration.saturating_mul(1000)),
            isrc: t.isrc.clone(),
            rank: t.rank.unwrap_or(0),
        }
    }

    pub fn view(&self, score: f32) -> ImportCandidate {
        let title = if self.version.is_empty() || self.title.contains(&self.version) {
            self.title.clone()
        } else {
            format!("{} {}", self.title, self.version)
        };
        ImportCandidate {
            deezer_id: self.deezer_id,
            title,
            artist: self.artists.first().cloned().unwrap_or_default(),
            album: self.album.clone(),
            duration_ms: self.duration_ms,
            score: (score * 1000.0).round() / 1000.0,
        }
    }
}

fn artist_score(wanted: &str, artists: &[String]) -> f32 {
    let want = split_artists(wanted);
    if want.is_empty() {
        // Список без артиста: не за что ни хвалить, ни ругать
        return 0.5;
    }
    let have: Vec<String> = artists.iter().flat_map(|a| split_artists(a)).collect();
    want.iter()
        .flat_map(|w| have.iter().map(move |h| similar(w, h)))
        .fold(0.0, f32::max)
}

fn duration_score(a: Option<i32>, b: Option<i32>) -> Option<f32> {
    let diff = (a? - b?).unsigned_abs();
    Some(match diff {
        0..=3000 => 1.0,
        3001..=8000 => 0.6,
        8001..=20000 => 0.2,
        _ => 0.0,
    })
}

/// Оценка трека-кандидата 0..1.
pub fn score_track(w: &Wanted, c: &Cand) -> f32 {
    if let (Some(a), Some(b)) = (&w.isrc, &c.isrc)
        && a.eq_ignore_ascii_case(b)
    {
        return 1.0;
    }
    let (want_title, want_tags) = split_title(&w.title);
    let full = format!("{} {}", c.title, c.version);
    let (cand_title, cand_tags) = split_title(&full);
    let title = similar(&want_title, &cand_title);
    let artist = artist_score(&w.artist, &c.artists);
    let mut score = match (&w.album, &c.album) {
        (Some(a), Some(b)) => {
            let album = similar(&split_title(a).0, &split_title(b).0);
            0.55 * title + 0.35 * artist + 0.10 * album.max(0.3)
        }
        _ => 0.6 * title + 0.4 * artist,
    };
    if let Some(d) = duration_score(w.duration_ms, c.duration_ms) {
        score = 0.85 * score + 0.15 * d;
        if d == 0.0 {
            score -= 0.1;
        }
    }
    // Другая версия той же песни — не она: концерт, ремикс, замедленная, акустика
    let differ = want_tags.symmetric_difference(&cand_tags).count();
    score -= 0.3 * differ.min(2) as f32;
    score.clamp(0.0, 1.0)
}

/// Оценка альбома-кандидата: «Альбом (Deluxe)» и «Альбом» — одно и то же.
pub fn score_album(artist: &str, title: &str, cand_title: &str, cand_artist: Option<&str>) -> f32 {
    let t = similar(&split_title(title).0, &split_title(cand_title).0);
    let names: Vec<String> = cand_artist.map(String::from).into_iter().collect();
    let ar = artist_score(artist, &names);
    (0.6 * t + 0.4 * ar).clamp(0.0, 1.0)
}

/// Оценка артиста по имени.
pub fn score_artist(name: &str, cand: &str) -> f32 {
    similar(&normalize(name), &normalize(cand))
}

/// Название без версии и «feat.» в исходном написании: для запросов и поиска в своём каталоге.
pub fn base_title(title: &str) -> &str {
    let b = title[..base_end(title)].trim();
    if b.is_empty() { title.trim() } else { b }
}

// ───────────── Поиск в Deezer ─────────────

/// Итог поиска трека.
#[derive(Debug, Clone, Default)]
pub struct Found {
    /// Лучший кандидат и его оценка.
    pub best: Option<(dz::Track, f32)>,
    /// Кандидаты по убыванию оценки (для проверки человеком).
    pub candidates: Vec<(Cand, f32)>,
    /// Сколько запросов к Deezer ушло.
    pub requests: u32,
}

impl Found {
    pub fn accepted(&self) -> bool {
        self.best.as_ref().is_some_and(|(_, s)| *s >= ACCEPT)
    }

    pub fn views(&self) -> Vec<ImportCandidate> {
        self.candidates
            .iter()
            .filter(|(_, s)| *s >= SUGGEST)
            .take(KEEP_CANDIDATES)
            .map(|(c, s)| c.view(*s))
            .collect()
    }
}

/// Кавычки внутри точного запроса Deezer ломают его — убираем.
fn quoted(s: &str) -> String {
    s.replace('"', " ").trim().to_string()
}

fn absorb(found: &mut Found, w: &Wanted, tracks: Vec<dz::Track>) {
    for t in tracks {
        if found.candidates.iter().any(|(c, _)| c.deezer_id == t.id) {
            continue;
        }
        let cand = Cand::from_deezer(&t);
        let mut score = score_track(w, &cand);
        // Из равных — не концертная и более популярная
        if crate::catalog::is_live(&t.title) && !crate::catalog::is_live(&w.title) {
            score -= 0.05;
        }
        let better = found
            .best
            .as_ref()
            .is_none_or(|(b, s)| score > *s || (score == *s && t.rank > b.rank));
        if better {
            found.best = Some((t, score));
        }
        found.candidates.push((cand, score));
    }
    found
        .candidates
        .sort_by(|a, b| b.1.total_cmp(&a.1).then(b.0.rank.cmp(&a.0.rank)));
}

async fn search(deezer: &Deezer, q: &str, limit: u32) -> Result<Vec<dz::Track>, DeezerError> {
    match deezer.search_tracks(q, limit).await {
        Ok(t) => Ok(t),
        Err(DeezerError::NotFound) => Ok(Vec::new()),
        Err(e) => Err(e),
    }
}

/// Ищет трек в Deezer: ISRC, точный запрос, свободный запрос — пока не найдётся уверенно.
pub async fn find_track(deezer: &Deezer, w: &Wanted) -> Result<Found, DeezerError> {
    let mut found = Found::default();
    if let Some(isrc) = &w.isrc {
        found.requests += 1;
        match deezer.track_by_isrc(isrc).await {
            Ok(t) => absorb(&mut found, w, vec![t]),
            Err(DeezerError::NotFound) => {}
            Err(e) => return Err(e),
        }
        if found.accepted() {
            return Ok(found);
        }
    }
    let (base, _) = split_title(&w.title);
    let title = if base.is_empty() {
        w.title.clone()
    } else {
        title_for_query(&w.title)
    };
    if !w.artist.trim().is_empty() {
        let artist = split_artists(&w.artist)
            .first()
            .cloned()
            .unwrap_or_else(|| w.artist.clone());
        let q = format!(r#"artist:"{}" track:"{}""#, quoted(&artist), quoted(&title));
        found.requests += 1;
        absorb(&mut found, w, search(deezer, &q, 10).await?);
        if found.accepted() {
            return Ok(found);
        }
    }
    let q = format!("{} {}", w.artist.trim(), title).trim().to_string();
    found.requests += 1;
    absorb(&mut found, w, search(deezer, &q, 15).await?);
    Ok(found)
}

/// Название для запроса: без «feat.» и версии в скобках, но с меткой версии («Live», «Remix»),
/// если она есть — иначе поиск вернёт студийную запись.
fn title_for_query(title: &str) -> String {
    let base = title[..base_end(title)].trim();
    let (_, tags) = split_title(title);
    let mut q = base.to_string();
    for tag in [
        "live",
        "remix",
        "slowed",
        "sped",
        "acoustic",
        "instrumental",
    ] {
        if tags.contains(tag) {
            q.push(' ');
            q.push_str(if tag == "sped" { "sped up" } else { tag });
        }
    }
    q
}

/// Альбом: точный запрос, затем свободный. Возвращает лучший альбом, его оценку и запросы.
pub async fn find_album(
    deezer: &Deezer,
    artist: &str,
    title: &str,
) -> Result<(Option<(dz::Album, f32)>, u32), DeezerError> {
    let mut best: Option<(dz::Album, f32)> = None;
    let mut requests = 0;
    let base = title_for_query(title);
    for q in [
        format!(r#"artist:"{}" album:"{}""#, quoted(artist), quoted(&base)),
        format!("{} {}", artist.trim(), base),
    ] {
        requests += 1;
        let albums = match deezer.search_albums(&q, 10).await {
            Ok(a) => a,
            Err(DeezerError::NotFound) => Vec::new(),
            Err(e) => return Err(e),
        };
        for a in albums {
            let name = a.artist.as_ref().map(|x| x.name.clone());
            let s = score_album(artist, title, &a.title, name.as_deref());
            if best.as_ref().is_none_or(|(_, b)| s > *b) {
                best = Some((a, s));
            }
        }
        if best.as_ref().is_some_and(|(_, s)| *s >= ACCEPT) {
            break;
        }
    }
    Ok((best, requests))
}

/// Артист по имени: из одинаково названных — у кого больше слушателей.
pub async fn find_artist(
    deezer: &Deezer,
    name: &str,
) -> Result<(Option<(dz::Artist, f32)>, u32), DeezerError> {
    let artists = match deezer.search_artists(name, 10).await {
        Ok(a) => a,
        Err(DeezerError::NotFound) => Vec::new(),
        Err(e) => return Err(e),
    };
    let mut best: Option<(dz::Artist, f32)> = None;
    for a in artists {
        let s = score_artist(name, &a.name);
        let better = best.as_ref().is_none_or(|(b, bs)| {
            s > *bs + 0.01 || ((s - *bs).abs() <= 0.01 && a.nb_fan > b.nb_fan)
        });
        if better {
            best = Some((a, s));
        }
    }
    Ok((best, 1))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cand(title: &str, version: &str, artists: &[&str], album: &str, secs: i32) -> Cand {
        Cand {
            deezer_id: 1,
            title: title.into(),
            version: version.into(),
            artists: artists.iter().map(|a| a.to_string()).collect(),
            album: Some(album.into()),
            duration_ms: Some(secs * 1000),
            isrc: None,
            rank: 0,
        }
    }

    fn want(artist: &str, title: &str, album: Option<&str>) -> Wanted {
        Wanted {
            artist: artist.into(),
            title: title.into(),
            album: album.map(String::from),
            ..Wanted::default()
        }
    }

    #[test]
    fn normalization() {
        assert_eq!(normalize("  Beyoncé — Halo!! "), "beyonce halo");
        assert_eq!(normalize("Ёлка"), "елка");
        assert_eq!(normalize("Simon & Garfunkel"), "simon and garfunkel");
        assert_eq!(translit("скриптонит"), "skriptonit");
        assert!(similar(&normalize("Скриптонит"), &normalize("Skryptonite")) > 0.75);
        let (base, tags) = split_title("Devil - Remastered 2011");
        assert_eq!(base, "devil");
        assert!(tags.is_empty());
        let (base, tags) = split_title("Devil (feat. Mzlff) [Slowed + Reverb]");
        assert_eq!(base, "devil");
        assert!(tags.contains("slowed"));
        assert!(split_title("Song (Original Mix)").1.is_empty());
        assert_eq!(split_title("Light as a Feather").0, "light as a feather");
        let w = want("Oasis", "Live Forever", None);
        assert!(
            score_track(
                &w,
                &cand("Live Forever", "", &["Oasis"], "Definitely Maybe", 276)
            ) > 0.95
        );
        assert_eq!(
            split_artists("Lowborn, vista"),
            vec!["lowborn", "vista", "lowborn vista"]
        );
    }

    #[test]
    fn exact_match_scores_high() {
        let w = want("Lowborn", "Devil", Some("Devil"));
        let s = score_track(&w, &cand("Devil", "", &["Lowborn"], "Devil", 162));
        assert!(s > 0.95, "{s}");
        // Сингл против альбомной версии — тот же трек
        let s = score_track(&w, &cand("Devil", "", &["Lowborn"], "Modern Sadness", 162));
        assert!(s >= ACCEPT, "{s}");
        // Spotify: «- Remastered», Deezer: «(Remastered)»
        let w = want("Queen", "Bohemian Rhapsody - Remastered 2011", None);
        let s = score_track(
            &w,
            &cand(
                "Bohemian Rhapsody",
                "(Remastered 2011)",
                &["Queen"],
                "A Night",
                354,
            ),
        );
        assert!(s > 0.95, "{s}");
    }

    #[test]
    fn transliterated_artist() {
        let w = want("Скриптонит", "Положение", None);
        let s = score_track(
            &w,
            &cand("Положение", "", &["Skryptonite"], "Uroboros", 200),
        );
        assert!(s >= ACCEPT, "{s}");
    }

    #[test]
    fn other_versions_and_artists_are_rejected() {
        let w = want("Lowborn", "Devil", None);
        // Кавер другого артиста
        let s = score_track(&w, &cand("Devil", "", &["Someone Else"], "Covers", 162));
        assert!(s < ACCEPT, "{s}");
        // Замедленная версия
        let s = score_track(&w, &cand("Devil", "(Slowed)", &["Lowborn"], "Devil", 200));
        assert!(s < ACCEPT, "{s}");
        // Концерт
        let s = score_track(
            &w,
            &cand("Devil", "(Live at Moscow)", &["Lowborn"], "Live", 170),
        );
        assert!(s < ACCEPT, "{s}");
        // Другая песня того же артиста
        let s = score_track(&w, &cand("Crazy", "", &["Lowborn"], "Crazy", 196));
        assert!(s < SUGGEST + 0.1, "{s}");
        // А если просили ремикс — ремикс и найдётся
        let w = want("Lowborn", "Devil (Remix)", None);
        let s = score_track(&w, &cand("Devil", "(Remix)", &["Lowborn"], "Devil", 180));
        assert!(s >= ACCEPT, "{s}");
    }

    #[test]
    fn duration_and_isrc() {
        let mut w = want("Lowborn", "Devil", None);
        w.duration_ms = Some(162_000);
        let close = score_track(&w, &cand("Devil", "", &["Lowborn"], "Devil", 163));
        let far = score_track(&w, &cand("Devil", "", &["Lowborn"], "Devil", 260));
        assert!(close > far && far < ACCEPT, "{close} {far}");
        w.isrc = Some("QZ5FN2194847".into());
        let mut c = cand("Совсем другое", "", &["X"], "Y", 10);
        c.isrc = Some("qz5fn2194847".into());
        assert_eq!(score_track(&w, &c), 1.0);
    }

    #[test]
    fn title_only_lists() {
        let w = want("", "Doctor Doctor", None);
        let s = score_track(
            &w,
            &cand("Doctor Doctor", "", &["Lowborn"], "Doctor Doctor", 165),
        );
        assert!(s >= 0.75, "{s}");
        assert_eq!(title_for_query("Devil (feat. X) - Live"), "Devil live");
    }
}
