//! Разбор архива данных Spotify (`my_spotify_data.zip`), который человек скачал сам. API Spotify не
//! используется. Внутри архива читаются только нужные JSON:
//!
//! - «Данные аккаунта»: `YourLibrary.json` (лайки, альбомы, подписки), `Playlist*.json`,
//!   `StreamingHistory_music_*.json` (история за год: конец прослушивания с точностью до минуты);
//! - «Расширенная история»: `Streaming_History_Audio_*.json` (или старое `endsong_*.json`) — вся
//!   история с секундами, `spotify_track_uri`, причиной конца, пропуском, перемешиванием, платформой.
//!
//! Защита от архивов-бомб: число записей, объявленный и фактически прочитанный размер каждой записи,
//! сумма размеров и степень сжатия ограничены; всё, кроме нужных JSON, пропускается не читая.

use std::{
    collections::HashMap,
    io::{Read, Seek},
};

use chrono::{DateTime, NaiveDateTime, TimeZone, Utc};
use serde::Deserialize;
use serde_json::Value;

use super::matching::normalize;

/// Пределы разбора архива.
#[derive(Debug, Clone, Copy)]
pub struct Limits {
    pub max_entries: usize,
    /// Один JSON после распаковки.
    pub max_entry_bytes: u64,
    /// Все прочитанные JSON вместе.
    pub max_total_bytes: u64,
    /// Во сколько раз запись может быть больше сжатой: JSON сжимается в 10–20 раз, бомбы — в тысячи.
    pub max_ratio: u64,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_entries: 5000,
            max_entry_bytes: 256 * 1024 * 1024,
            max_total_bytes: 2 * 1024 * 1024 * 1024,
            max_ratio: 200,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ArchiveError {
    #[error("это не ZIP-архив")]
    NotZip,
    #[error("архив повреждён: {0}")]
    Broken(String),
    #[error("в архиве слишком много файлов")]
    TooManyEntries,
    #[error("файл {0} в архиве слишком большой")]
    EntryTooLarge(String),
    #[error("в архиве подозрительно сильно сжатый файл {0}")]
    Bomb(String),
    #[error("архив слишком большой после распаковки")]
    TooLarge,
    #[error("в архиве нет данных Spotify: ни медиатеки, ни плейлистов, ни истории прослушиваний")]
    Empty,
}

/// Причина конца прослушивания до сопоставления: `auto` — решится по длительности трека из каталога.
pub const AUTO_REASON: &str = "auto";

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Track {
    pub artist: String,
    pub title: String,
    pub album: Option<String>,
    /// `spotify:track:…`
    pub uri: Option<String>,
    pub liked: bool,
    pub plays: u32,
    /// Сумма прослушанного — чтобы сначала сопоставлять то, что слушали больше.
    pub played_ms: u64,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Album {
    pub artist: String,
    pub title: String,
    pub uri: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Artist {
    pub name: String,
    pub uri: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Playlist {
    pub name: String,
    pub description: String,
    /// Индексы в [`Archive::tracks`] в порядке плейлиста.
    pub tracks: Vec<usize>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Play {
    pub track: usize,
    pub ended_at: DateTime<Utc>,
    pub played_ms: u32,
    /// `completed`, `skipped`, `previous`, `stopped`, `error`, `interrupted` или [`AUTO_REASON`].
    pub end_reason: &'static str,
    pub shuffle: Option<bool>,
    pub platform: Option<&'static str>,
}

#[derive(Debug, Clone, Default)]
pub struct Archive {
    pub tracks: Vec<Track>,
    pub albums: Vec<Album>,
    pub artists: Vec<Artist>,
    pub playlists: Vec<Playlist>,
    pub plays: Vec<Play>,
    /// Какие файлы прочитаны (для журнала).
    pub files: Vec<String>,
    /// Была расширенная история (короткая годовая тогда не читается: она в ней уже есть).
    pub extended: bool,
    track_by_uri: HashMap<String, usize>,
    track_by_name: HashMap<String, usize>,
}

impl Archive {
    pub fn is_empty(&self) -> bool {
        self.tracks.is_empty() && self.albums.is_empty() && self.artists.is_empty()
    }

    /// Трек по ссылке Spotify или по имени: один и тот же трек из медиатеки, плейлиста и истории — одна позиция.
    fn track(
        &mut self,
        artist: &str,
        title: &str,
        album: Option<&str>,
        uri: Option<&str>,
    ) -> usize {
        let uri = uri
            .map(str::trim)
            .filter(|u| u.starts_with("spotify:track:"));
        let name_key = format!("{}|{}", normalize(artist), normalize(title));
        let found = uri
            .and_then(|u| self.track_by_uri.get(u).copied())
            .or_else(|| self.track_by_name.get(&name_key).copied());
        let idx = match found {
            Some(i) => i,
            None => {
                self.tracks.push(Track {
                    artist: artist.trim().to_string(),
                    title: title.trim().to_string(),
                    album: album
                        .map(str::trim)
                        .filter(|a| !a.is_empty())
                        .map(String::from),
                    uri: uri.map(String::from),
                    ..Track::default()
                });
                self.tracks.len() - 1
            }
        };
        let t = &mut self.tracks[idx];
        if t.uri.is_none() {
            t.uri = uri.map(String::from);
        }
        if t.album.is_none() {
            t.album = album
                .map(str::trim)
                .filter(|a| !a.is_empty())
                .map(String::from);
        }
        if let Some(u) = uri {
            self.track_by_uri.insert(u.to_string(), idx);
        }
        self.track_by_name.entry(name_key).or_insert(idx);
        idx
    }
}

/// Что за файл внутри архива.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Entry {
    Library,
    Playlists,
    History,
    Extended,
}

fn classify(path: &str) -> Option<Entry> {
    let name = path.rsplit(['/', '\\']).next().unwrap_or(path);
    let lower = name.to_ascii_lowercase();
    if !lower.ends_with(".json") || name.starts_with("._") {
        return None;
    }
    let stem = &lower[..lower.len() - 5];
    let digits = |s: &str| !s.is_empty() && s.chars().all(|c| c.is_ascii_digit());
    if stem == "yourlibrary" {
        Some(Entry::Library)
    } else if stem
        .strip_prefix("playlist")
        .is_some_and(|s| s.is_empty() || digits(s))
    {
        Some(Entry::Playlists)
    } else if stem.starts_with("streaming_history_audio_")
        || stem.strip_prefix("endsong_").is_some_and(digits)
    {
        Some(Entry::Extended)
    } else if stem
        .strip_prefix("streaminghistory_music_")
        .is_some_and(digits)
        || stem.strip_prefix("streaminghistory").is_some_and(digits)
    {
        Some(Entry::History)
    } else {
        None
    }
}

/// Разбирает архив. Синхронный: вызывать из `spawn_blocking`.
pub fn parse_zip<R: Read + Seek>(reader: R, limits: Limits) -> Result<Archive, ArchiveError> {
    let mut zip = zip::ZipArchive::new(reader).map_err(|e| match e {
        zip::result::ZipError::InvalidArchive(_) => ArchiveError::NotZip,
        other => ArchiveError::Broken(other.to_string()),
    })?;
    if zip.len() > limits.max_entries {
        return Err(ArchiveError::TooManyEntries);
    }
    // Сначала — что где лежит, чтобы медиатека разобралась раньше плейлистов и истории
    let mut wanted: Vec<(usize, Entry, String)> = Vec::new();
    for i in 0..zip.len() {
        let file = zip
            .by_index_raw(i)
            .map_err(|e| ArchiveError::Broken(e.to_string()))?;
        if file.is_dir() {
            continue;
        }
        let Ok(name) = file.name().map(|n| n.to_string()) else {
            continue;
        };
        let Some(kind) = classify(&name) else {
            continue;
        };
        if file.size() > limits.max_entry_bytes {
            return Err(ArchiveError::EntryTooLarge(name));
        }
        if file.size() > 1024 * 1024
            && file.size() / file.compressed_size().max(1) > limits.max_ratio
        {
            return Err(ArchiveError::Bomb(name));
        }
        wanted.push((i, kind, name));
    }
    let extended = wanted.iter().any(|(_, k, _)| *k == Entry::Extended);
    let order = |k: Entry| match k {
        Entry::Library => 0,
        Entry::Playlists => 1,
        Entry::Extended | Entry::History => 2,
    };
    wanted.sort_by(|a, b| order(a.1).cmp(&order(b.1)).then_with(|| a.2.cmp(&b.2)));

    let mut out = Archive {
        extended,
        ..Archive::default()
    };
    let mut total = 0u64;
    for (i, kind, name) in wanted {
        if kind == Entry::History && extended {
            continue;
        }
        let file = zip
            .by_index(i)
            .map_err(|e| ArchiveError::Broken(e.to_string()))?;
        // Объявленному размеру не верим: читаем не больше предела и проверяем сами
        let mut bytes = Vec::with_capacity(file.size().min(limits.max_entry_bytes) as usize);
        file.take(limits.max_entry_bytes + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| ArchiveError::Broken(format!("{name}: {e}")))?;
        if bytes.len() as u64 > limits.max_entry_bytes {
            return Err(ArchiveError::EntryTooLarge(name));
        }
        total += bytes.len() as u64;
        if total > limits.max_total_bytes {
            return Err(ArchiveError::TooLarge);
        }
        // Битый или чужой JSON с подходящим именем не валит весь импорт
        if let Err(e) = parse_entry(&mut out, kind, &bytes) {
            tracing::warn!(file = %name, error = %e, "skipped an unreadable file in the Spotify archive");
            continue;
        }
        out.files.push(name);
    }
    if out.is_empty() {
        return Err(ArchiveError::Empty);
    }
    Ok(out)
}

/// Отдельный JSON из архива (если человек загрузил его, а не весь ZIP).
pub fn parse_json(name: &str, bytes: &[u8]) -> Result<Archive, ArchiveError> {
    let kind = classify(name).ok_or(ArchiveError::Empty)?;
    let mut out = Archive {
        extended: kind == Entry::Extended,
        ..Archive::default()
    };
    parse_entry(&mut out, kind, bytes).map_err(|e| ArchiveError::Broken(e.to_string()))?;
    out.files.push(name.to_string());
    if out.is_empty() {
        return Err(ArchiveError::Empty);
    }
    Ok(out)
}

fn parse_entry(out: &mut Archive, kind: Entry, bytes: &[u8]) -> Result<(), serde_json::Error> {
    match kind {
        Entry::Library => library(out, serde_json::from_slice(bytes)?),
        Entry::Playlists => playlists(out, serde_json::from_slice(bytes)?),
        Entry::History => {
            for p in serde_json::from_slice::<Vec<HistoryRow>>(bytes)? {
                history(out, p);
            }
        }
        Entry::Extended => {
            for p in serde_json::from_slice::<Vec<ExtendedRow>>(bytes)? {
                extended_row(out, p);
            }
        }
    }
    Ok(())
}

// ───────────── YourLibrary.json ─────────────

#[derive(Deserialize, Default)]
#[serde(default)]
struct Library {
    tracks: Vec<LibTrack>,
    albums: Vec<LibAlbum>,
    artists: Vec<LibArtist>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct LibTrack {
    artist: String,
    album: Option<String>,
    track: String,
    uri: Option<String>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct LibAlbum {
    artist: String,
    album: String,
    uri: Option<String>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct LibArtist {
    name: String,
    uri: Option<String>,
}

fn library(out: &mut Archive, lib: Library) {
    for t in lib.tracks {
        if t.track.trim().is_empty() {
            continue;
        }
        let i = out.track(&t.artist, &t.track, t.album.as_deref(), t.uri.as_deref());
        out.tracks[i].liked = true;
    }
    for a in lib.albums {
        if a.album.trim().is_empty() {
            continue;
        }
        let key = (normalize(&a.artist), normalize(&a.album));
        if out
            .albums
            .iter()
            .any(|x| (normalize(&x.artist), normalize(&x.title)) == key)
        {
            continue;
        }
        out.albums.push(Album {
            artist: a.artist.trim().to_string(),
            title: a.album.trim().to_string(),
            uri: a.uri,
        });
    }
    for a in lib.artists {
        if a.name.trim().is_empty() {
            continue;
        }
        let key = normalize(&a.name);
        if out.artists.iter().any(|x| normalize(&x.name) == key) {
            continue;
        }
        out.artists.push(Artist {
            name: a.name.trim().to_string(),
            uri: a.uri,
        });
    }
}

// ───────────── Playlist*.json ─────────────

#[derive(Deserialize, Default)]
#[serde(default)]
struct Playlists {
    playlists: Vec<PlaylistRow>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct PlaylistRow {
    name: String,
    description: Option<String>,
    items: Vec<PlaylistEntry>,
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct PlaylistEntry {
    track: Option<PlaylistTrack>,
    local_track: Option<LocalTrack>,
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct PlaylistTrack {
    track_name: String,
    artist_name: String,
    album_name: Option<String>,
    track_uri: Option<String>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct LocalTrack {
    uri: String,
}

/// `spotify:local:Artist:Album:Title:Seconds` — локальный файл в плейлисте; части в URL-кодировке.
fn local_track(uri: &str) -> Option<(String, String, Option<String>)> {
    let rest = uri.strip_prefix("spotify:local:")?;
    let parts: Vec<String> = rest
        .split(':')
        .map(|p| {
            url::form_urlencoded::parse(format!("x={p}").as_bytes())
                .next()
                .map(|(_, v)| v.into_owned())
                .unwrap_or_default()
        })
        .collect();
    let artist = parts.first()?.trim().to_string();
    let album = parts
        .get(1)
        .map(|a| a.trim().to_string())
        .filter(|a| !a.is_empty());
    let title = parts.get(2)?.trim().to_string();
    (!title.is_empty()).then_some((artist, title, album))
}

fn playlists(out: &mut Archive, data: Playlists) {
    for p in data.playlists {
        let mut tracks = Vec::with_capacity(p.items.len());
        for item in p.items {
            if let Some(t) = item.track {
                if t.track_name.trim().is_empty() {
                    continue;
                }
                tracks.push(out.track(
                    &t.artist_name,
                    &t.track_name,
                    t.album_name.as_deref(),
                    t.track_uri.as_deref(),
                ));
            } else if let Some((artist, title, album)) =
                item.local_track.as_ref().and_then(|l| local_track(&l.uri))
            {
                tracks.push(out.track(&artist, &title, album.as_deref(), None));
            }
        }
        let name = p.name.trim();
        out.playlists.push(Playlist {
            name: if name.is_empty() {
                "Плейлист".into()
            } else {
                name.to_string()
            },
            description: p.description.unwrap_or_default().trim().to_string(),
            tracks,
        });
    }
}

// ───────────── История ─────────────

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct HistoryRow {
    end_time: String,
    artist_name: String,
    track_name: String,
    ms_played: u64,
}

/// Короткие касания (меньше полсекунды) — не прослушивания: Spotify пишет их при пролистывании.
const MIN_PLAY_MS: u64 = 500;

fn history(out: &mut Archive, row: HistoryRow) {
    if row.ms_played < MIN_PLAY_MS || row.track_name.trim().is_empty() {
        return;
    }
    // «2024-03-01 18:42» — UTC, с точностью до минуты
    let Ok(naive) = NaiveDateTime::parse_from_str(row.end_time.trim(), "%Y-%m-%d %H:%M") else {
        return;
    };
    let track = out.track(&row.artist_name, &row.track_name, None, None);
    push_play(
        out,
        Play {
            track,
            ended_at: Utc.from_utc_datetime(&naive),
            played_ms: row.ms_played.min(u64::from(u32::MAX)) as u32,
            end_reason: AUTO_REASON,
            shuffle: None,
            platform: None,
        },
    );
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct ExtendedRow {
    ts: String,
    platform: Option<String>,
    ms_played: u64,
    master_metadata_track_name: Option<String>,
    master_metadata_album_artist_name: Option<String>,
    master_metadata_album_album_name: Option<String>,
    spotify_track_uri: Option<String>,
    reason_end: Option<String>,
    shuffle: Option<Value>,
    skipped: Option<Value>,
    incognito_mode: Option<Value>,
}

fn flag(v: &Option<Value>) -> Option<bool> {
    match v.as_ref()? {
        Value::Bool(b) => Some(*b),
        Value::Number(n) => Some(n.as_i64() == Some(1)),
        Value::String(s) => match s.as_str() {
            "true" | "1" => Some(true),
            "false" | "0" => Some(false),
            _ => None,
        },
        _ => None,
    }
}

fn extended_row(out: &mut Archive, row: ExtendedRow) {
    // Подкасты, аудиокниги и прослушивания в приватном режиме не переносим
    let (Some(title), Some(uri)) = (&row.master_metadata_track_name, &row.spotify_track_uri) else {
        return;
    };
    if row.ms_played < MIN_PLAY_MS
        || title.trim().is_empty()
        || flag(&row.incognito_mode) == Some(true)
    {
        return;
    }
    let Ok(ended_at) = DateTime::parse_from_rfc3339(row.ts.trim()) else {
        return;
    };
    let artist = row
        .master_metadata_album_artist_name
        .clone()
        .unwrap_or_default();
    let track = out.track(
        &artist,
        title,
        row.master_metadata_album_album_name.as_deref(),
        Some(uri),
    );
    push_play(
        out,
        Play {
            track,
            ended_at: ended_at.with_timezone(&Utc),
            played_ms: row.ms_played.min(u64::from(u32::MAX)) as u32,
            end_reason: end_reason(row.reason_end.as_deref(), flag(&row.skipped)),
            shuffle: flag(&row.shuffle),
            platform: row.platform.as_deref().and_then(platform),
        },
    );
}

fn push_play(out: &mut Archive, play: Play) {
    let t = &mut out.tracks[play.track];
    t.plays += 1;
    t.played_ms += u64::from(play.played_ms);
    out.plays.push(play);
}

/// Причина конца из расширенной истории в наши `end_reason`. Неизвестное решится по длительности трека.
pub fn end_reason(reason_end: Option<&str>, skipped: Option<bool>) -> &'static str {
    let reason = reason_end.unwrap_or("").trim();
    if reason == "trackdone" {
        return "completed";
    }
    if skipped == Some(true) {
        return "skipped";
    }
    match reason {
        "fwdbtn" | "clickrow" => "skipped",
        "backbtn" => "previous",
        "endplay" | "logout" | "remote" => "stopped",
        "trackerror" => "error",
        r if r.starts_with("unexpected-exit") => "interrupted",
        _ => AUTO_REASON,
    }
}

/// Платформа Spotify («iOS 17.1 (iPhone15,2)», «Android OS 13», «OS X 14.0», «windows», «web_player …»)
/// в наши платформы устройств. Колонки, приставки и прочее — без платформы.
pub fn platform(raw: &str) -> Option<&'static str> {
    let p = raw.to_ascii_lowercase();
    if p.contains("ipad") {
        Some("ipados")
    } else if p.starts_with("ios") || p.contains("iphone") {
        Some("ios")
    } else if p.contains("android") {
        Some("android")
    } else if p.contains("os x") || p.contains("osx") || p.contains("macos") {
        Some("macos")
    } else if p.contains("windows") {
        Some("windows")
    } else if p.contains("linux") {
        Some("linux")
    } else if p.contains("web_player") || p.starts_with("web") || p.contains("browser") {
        Some("web")
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use std::io::{Cursor, Write};

    use zip::write::SimpleFileOptions;

    use super::*;

    pub(crate) fn zip_of(files: &[(&str, &str)]) -> Vec<u8> {
        let mut buf = Cursor::new(Vec::new());
        {
            let mut w = zip::ZipWriter::new(&mut buf);
            let opts =
                SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
            for (name, body) in files {
                w.start_file(*name, opts).unwrap();
                w.write_all(body.as_bytes()).unwrap();
            }
            w.finish().unwrap();
        }
        buf.into_inner()
    }

    const LIBRARY: &str = r#"{
      "tracks": [
        {"artist": "Lowborn", "album": "Devil", "track": "Devil", "uri": "spotify:track:1"},
        {"artist": "Mzlff", "album": "фан", "track": "Ёлка", "uri": "spotify:track:2"}
      ],
      "albums": [{"artist": "Lowborn", "album": "Modern Sadness", "uri": "spotify:album:9"}],
      "shows": [],
      "artists": [{"name": "5opka", "uri": "spotify:artist:5"}, {"name": "5OPKA"}]
    }"#;

    const PLAYLISTS: &str = r#"{"playlists": [{
      "name": "Ночь", "lastModifiedDate": "2024-01-01", "description": null,
      "items": [
        {"track": {"trackName": "Devil", "artistName": "Lowborn", "albumName": "Devil", "trackUri": "spotify:track:1"}, "episode": null, "localTrack": null, "addedDate": "2023-01-01"},
        {"track": null, "episode": {"episodeName": "x"}, "localTrack": null},
        {"track": null, "episode": null, "localTrack": {"uri": "spotify:local:Rushex:%D0%A2%D1%8C%D0%BC%D0%B0:Night+Drive:180"}},
        {"track": {"trackName": "Light Me Up", "artistName": "Lowborn", "albumName": "Light Me Up", "trackUri": "spotify:track:3"}}
      ]}]}"#;

    const HISTORY: &str = r#"[
      {"endTime": "2024-03-01 18:42", "artistName": "Lowborn", "trackName": "Devil", "msPlayed": 161000},
      {"endTime": "2024-03-01 18:45", "artistName": "Lowborn", "trackName": "Crazy", "msPlayed": 9000},
      {"endTime": "2024-03-01 18:46", "artistName": "Lowborn", "trackName": "Crazy", "msPlayed": 0}
    ]"#;

    const EXTENDED: &str = r#"[
      {"ts": "2021-05-01T10:00:05Z", "platform": "iOS 14.4 (iPhone12,1)", "ms_played": 162000,
       "master_metadata_track_name": "Devil", "master_metadata_album_artist_name": "Lowborn",
       "master_metadata_album_album_name": "Devil", "spotify_track_uri": "spotify:track:1",
       "reason_start": "trackdone", "reason_end": "trackdone", "shuffle": false, "skipped": null,
       "offline": false, "incognito_mode": false},
      {"ts": "2021-05-01T10:01:00Z", "platform": "OS X 11.2.3 [x86 8]", "ms_played": 12000,
       "master_metadata_track_name": "Hurt", "master_metadata_album_artist_name": "Lowborn",
       "master_metadata_album_album_name": "Hurt", "spotify_track_uri": "spotify:track:4",
       "reason_end": "fwdbtn", "shuffle": true, "skipped": true, "incognito_mode": false},
      {"ts": "2021-05-01T10:02:00Z", "platform": "Android OS 11", "ms_played": 50000,
       "master_metadata_track_name": null, "spotify_track_uri": null, "episode_name": "Подкаст"},
      {"ts": "2021-05-01T10:03:00Z", "platform": "web_player", "ms_played": 90000,
       "master_metadata_track_name": "Secret", "master_metadata_album_artist_name": "X",
       "spotify_track_uri": "spotify:track:7", "reason_end": "endplay", "incognito_mode": true}
    ]"#;

    #[test]
    fn account_archive() {
        let zip = zip_of(&[
            ("Spotify Account Data/YourLibrary.json", LIBRARY),
            ("Spotify Account Data/Playlist1.json", PLAYLISTS),
            (
                "Spotify Account Data/StreamingHistory_music_0.json",
                HISTORY,
            ),
            ("Spotify Account Data/StreamingHistory_podcast_0.json", "[]"),
            ("Spotify Account Data/Read Me First.pdf", "%PDF"),
        ]);
        let a = parse_zip(Cursor::new(zip), Limits::default()).unwrap();
        assert!(!a.extended);
        assert_eq!(a.files.len(), 3);
        // Devil — один трек из медиатеки, плейлиста и истории
        let titles: Vec<&str> = a.tracks.iter().map(|t| t.title.as_str()).collect();
        assert_eq!(
            titles,
            ["Devil", "Ёлка", "Night Drive", "Light Me Up", "Crazy"]
        );
        assert!(a.tracks[0].liked && a.tracks[1].liked && !a.tracks[3].liked);
        assert_eq!(a.tracks[0].plays, 1);
        assert_eq!(a.tracks[2].artist, "Rushex");
        assert_eq!(a.tracks[2].album.as_deref(), Some("Тьма"));
        assert_eq!(a.albums.len(), 1);
        // «5opka» и «5OPKA» — один артист
        assert_eq!(a.artists.len(), 1);
        assert_eq!(a.playlists.len(), 1);
        assert_eq!(a.playlists[0].name, "Ночь");
        assert_eq!(a.playlists[0].tracks, vec![0, 2, 3]);
        // Нулевое касание не считается
        assert_eq!(a.plays.len(), 2);
        assert_eq!(
            a.plays[0].ended_at.to_rfc3339(),
            "2024-03-01T18:42:00+00:00"
        );
        assert_eq!(a.plays[1].end_reason, AUTO_REASON);
    }

    #[test]
    fn extended_history_replaces_the_short_one() {
        let zip = zip_of(&[
            (
                "Spotify Extended Streaming History/Streaming_History_Audio_2019-2021_0.json",
                EXTENDED,
            ),
            (
                "Spotify Account Data/StreamingHistory_music_0.json",
                HISTORY,
            ),
        ]);
        let a = parse_zip(Cursor::new(zip), Limits::default()).unwrap();
        assert!(a.extended);
        assert_eq!(a.files.len(), 1);
        // Подкаст и приватный режим пропущены
        assert_eq!(a.plays.len(), 2);
        assert_eq!(a.tracks.len(), 2);
        let done = &a.plays[0];
        assert_eq!(done.end_reason, "completed");
        assert_eq!(done.platform, Some("ios"));
        assert_eq!(done.played_ms, 162000);
        let skip = &a.plays[1];
        assert_eq!(skip.end_reason, "skipped");
        assert_eq!(skip.shuffle, Some(true));
        assert_eq!(skip.platform, Some("macos"));
        assert_eq!(a.tracks[1].uri.as_deref(), Some("spotify:track:4"));
    }

    #[test]
    fn reasons_and_platforms() {
        assert_eq!(end_reason(Some("trackdone"), Some(true)), "completed");
        assert_eq!(end_reason(Some("endplay"), Some(true)), "skipped");
        assert_eq!(end_reason(Some("backbtn"), None), "previous");
        assert_eq!(
            end_reason(Some("unexpected-exit-while-paused"), None),
            "interrupted"
        );
        assert_eq!(end_reason(Some("unknown"), None), AUTO_REASON);
        assert_eq!(platform("Windows 10 (10.0.19041; x64)"), Some("windows"));
        assert_eq!(platform("iOS 15.1 (iPad8,1)"), Some("ipados"));
        assert_eq!(platform("Partner sonos_one"), None);
    }

    #[test]
    fn rejects_junk_and_bombs() {
        assert!(matches!(
            parse_zip(Cursor::new(b"not a zip".to_vec()), Limits::default()),
            Err(ArchiveError::NotZip)
        ));
        let empty = zip_of(&[("readme.txt", "hi"), ("Playlist1.json", "not json")]);
        assert!(matches!(
            parse_zip(Cursor::new(empty), Limits::default()),
            Err(ArchiveError::Empty)
        ));
        // 8 МБ пробелов сжимаются в тысячи раз
        let spaces = format!("[{}]", " ".repeat(8 * 1024 * 1024));
        let bomb = zip_of(&[("StreamingHistory_music_0.json", &spaces)]);
        assert!(matches!(
            parse_zip(Cursor::new(bomb.clone()), Limits::default()),
            Err(ArchiveError::Bomb(_))
        ));
        let tight = Limits {
            max_entry_bytes: 300,
            ..Limits::default()
        };
        let big = zip_of(&[("YourLibrary.json", LIBRARY), ("Playlist1.json", PLAYLISTS)]);
        assert!(matches!(
            parse_zip(Cursor::new(big), tight),
            Err(ArchiveError::EntryTooLarge(_))
        ));
    }

    #[test]
    fn single_json_file() {
        let a = parse_json("YourLibrary.json", LIBRARY.as_bytes()).unwrap();
        assert_eq!(a.tracks.len(), 2);
        assert!(parse_json("notes.json", b"{}").is_err());
    }
}
