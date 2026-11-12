//! Перенос из YouTube Music: плейлист по ссылке и архив Google Takeout. Аккаунты и cookies Google не
//! используются: треки публичного плейлиста и имена видео по id отдаёт ytmusicapi в сервисе fetcher
//! (`/v1/ytmusic/playlist`, `/v1/ytmusic/songs`) без входа.
//!
//! Google Takeout «YouTube и YouTube Music» — ZIP с CSV; названия папок и файлов зависят от языка
//! аккаунта («YouTube and YouTube Music/playlists/…», «YouTube и YouTube Music/плейлисты/…»), поэтому
//! файлы узнаются по колонкам, а не по путям:
//!
//! - библиотека YouTube Music («music library songs.csv»): Video ID, Song Title, Album Title,
//!   Artist Name 1… — уже с именами, кладётся в «Мне нравится»;
//! - плейлист («<название>-videos.csv»: Video ID, Playlist Video Creation Timestamp; старый формат —
//!   блок описания плейлиста с Title, пустая строка, затем Video ID, Time Added) — только id; имена
//!   узнаются в задаче сопоставления. «Понравившиеся» — в «Мне нравится», «Смотреть позже» пропускается;
//! - остальное (подписки, история, метаданные плейлистов, загрузки) не читается.
//!
//! Видео без музыкальной метки YouTube Music (влоги, лекции из «Понравившихся») пропускаются при
//! сопоставлении. Защита от архивов-бомб — те же пределы, что у архива Spotify.

use std::{
    io::{Read, Seek},
    time::Duration,
};

use serde::Deserialize;

use super::{
    collection::{self, Collection},
    spotify::{ArchiveError, Limits},
};

/// Плейлист из сотен треков ytmusicapi листает десятки секунд.
const PLAYLIST_TIMEOUT: Duration = Duration::from_secs(180);
/// Имена пачки видео: по запросу на видео.
const SONGS_TIMEOUT: Duration = Duration::from_secs(120);

/// Id плейлиста из ссылки (`youtube.com/playlist?list=…`, `music.youtube.com/watch?v=…&list=…`,
/// `youtu.be/…?list=…`) или сам id. Миксы «для вас» (`RD…`) и «Понравившиеся» (`LL`, `LM`) чужому
/// серверу недоступны.
pub fn playlist_id(raw: &str) -> Option<String> {
    let raw = raw.trim();
    if valid_playlist_id(raw) {
        return Some(raw.to_string());
    }
    let with_scheme = if raw.contains("://") {
        raw.to_string()
    } else {
        format!("https://{raw}")
    };
    let url = url::Url::parse(&with_scheme).ok()?;
    let host = url.host_str()?.to_ascii_lowercase();
    let host = host.strip_prefix("www.").unwrap_or(&host);
    let host = host.strip_prefix("m.").unwrap_or(host);
    if !matches!(host, "youtube.com" | "music.youtube.com" | "youtu.be") {
        return None;
    }
    // music.youtube.com/browse/VLPL… — страница плейлиста
    if let Some(id) = url
        .path_segments()
        .and_then(|mut s| match (s.next(), s.next()) {
            (Some("browse"), Some(id)) => id.strip_prefix("VL").map(String::from),
            _ => None,
        })
        && valid_playlist_id(&id)
    {
        return Some(id);
    }
    url.query_pairs()
        .find(|(k, _)| k == "list")
        .map(|(_, v)| v.into_owned())
        .filter(|id| valid_playlist_id(id))
}

fn valid_playlist_id(s: &str) -> bool {
    s.len() >= 12
        && s.len() <= 80
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'))
        && ["PL", "OLAK5uy_", "VL", "FL", "UU", "RDCLAK"]
            .iter()
            .any(|p| s.starts_with(p))
}

/// Id видео YouTube: 11 символов.
pub fn valid_video_id(s: &str) -> bool {
    s.len() == 11
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'))
}

/// Музыка ли это по метке YouTube Music: песня (ATV), клип (OMV), загрузка с музыкой (UGC).
pub fn is_music(video_type: Option<&str>) -> bool {
    matches!(
        video_type,
        Some(
            "MUSIC_VIDEO_TYPE_ATV"
                | "MUSIC_VIDEO_TYPE_OMV"
                | "MUSIC_VIDEO_TYPE_UGC"
                | "MUSIC_VIDEO_TYPE_OFFICIAL_SOURCE_MUSIC"
        )
    )
}

/// Артист и название видео. У песен (ATV) всё верно как есть; у клипов и загрузок канал часто не
/// артист («ZharaMusic», «LowbornVEVO»), а название — «Артист - Песня | Премьера 2019».
pub fn song(title: &str, artists: &[String], video_type: Option<&str>) -> (String, String) {
    let title = title.trim();
    let channel = artists
        .iter()
        .map(|a| clean_channel(a))
        .filter(|a| !a.is_empty())
        .collect::<Vec<_>>()
        .join(", ");
    // Хвост после « | » — подписи канала, не название
    let title = title.split(" | ").next().unwrap_or(title).trim();
    if video_type != Some("MUSIC_VIDEO_TYPE_ATV") {
        for sep in [" - ", " – ", " — "] {
            if let Some((a, t)) = title.split_once(sep)
                && !a.trim().is_empty()
                && !t.trim().is_empty()
            {
                return (a.trim().to_string(), t.trim().to_string());
            }
        }
    }
    (channel, title.to_string())
}

fn clean_channel(name: &str) -> String {
    let n = name.trim();
    let n = n.strip_suffix(" - Topic").unwrap_or(n);
    let n = n.strip_suffix(" - Тема").unwrap_or(n);
    let n = n.strip_suffix("VEVO").unwrap_or(n);
    n.trim().to_string()
}

// ───────────── Сервис fetcher ─────────────

#[derive(Debug, thiserror::Error)]
pub enum YoutubeError {
    #[error("плейлист не нашёлся — он закрыт или удалён")]
    NotFound,
    #[error("YouTube Music сейчас недоступен — попробуйте позже")]
    Unavailable(String),
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct YtTrack {
    pub video_id: Option<String>,
    pub title: String,
    pub artists: Vec<String>,
    pub album: Option<String>,
    pub duration_ms: Option<i32>,
    pub video_type: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct YtPlaylist {
    pub title: String,
    pub description: String,
    pub tracks: Vec<YtTrack>,
}

/// Имя видео по id; `found = false` — видео удалено или скрыто.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct YtSong {
    pub video_id: String,
    pub found: bool,
    pub title: String,
    pub artists: Vec<String>,
    pub duration_ms: Option<i32>,
    pub video_type: Option<String>,
}

#[derive(Deserialize)]
struct SongsResponse {
    songs: Vec<YtSong>,
}

async fn post<T: serde::de::DeserializeOwned>(
    http: &reqwest::Client,
    url: String,
    body: serde_json::Value,
    timeout: Duration,
) -> Result<T, YoutubeError> {
    let res = http
        .post(url)
        .timeout(timeout)
        .json(&body)
        .send()
        .await
        .map_err(|e| YoutubeError::Unavailable(e.without_url().to_string()))?;
    match res.status() {
        s if s.is_success() => res
            .json()
            .await
            .map_err(|e| YoutubeError::Unavailable(e.without_url().to_string())),
        reqwest::StatusCode::NOT_FOUND => Err(YoutubeError::NotFound),
        s => Err(YoutubeError::Unavailable(format!(
            "fetcher: HTTP {}",
            s.as_u16()
        ))),
    }
}

/// Треки публичного плейлиста.
pub async fn fetch_playlist(
    http: &reqwest::Client,
    fetcher: &str,
    id: &str,
) -> Result<YtPlaylist, YoutubeError> {
    post(
        http,
        format!("{fetcher}/v1/ytmusic/playlist"),
        serde_json::json!({ "playlistId": id }),
        PLAYLIST_TIMEOUT,
    )
    .await
}

/// Имена видео по id (до 50 за раз).
pub async fn fetch_songs(
    http: &reqwest::Client,
    fetcher: &str,
    ids: &[String],
) -> Result<Vec<YtSong>, YoutubeError> {
    let res: SongsResponse = post(
        http,
        format!("{fetcher}/v1/ytmusic/songs"),
        serde_json::json!({ "videoIds": ids }),
        SONGS_TIMEOUT,
    )
    .await?;
    Ok(res.songs)
}

/// Плейлист → позиции: все треки в один плейлист с тем же названием.
pub fn playlist_collection(p: &YtPlaylist) -> Collection {
    let mut c = Collection::default();
    let positions: Vec<usize> = p
        .tracks
        .iter()
        .filter(|t| !t.title.trim().is_empty())
        .filter_map(|t| {
            let (artist, title) = song(&t.title, &t.artists, t.video_type.as_deref());
            c.add(collection::Track {
                artist,
                title,
                album: t.album.clone(),
                duration_ms: t.duration_ms,
                source_ref: t
                    .video_id
                    .as_deref()
                    .filter(|v| valid_video_id(v))
                    .map(|v| format!("yt:{v}")),
                liked: false,
            })
        })
        .collect();
    if !positions.is_empty() {
        c.playlist(&p.title, &p.description, positions);
    }
    c
}

// ───────────── Google Takeout ─────────────

/// Что за CSV по колонкам.
#[derive(Debug, Clone, PartialEq)]
enum Sheet {
    /// Песни с именами (библиотека YouTube Music).
    Songs,
    /// Только id видео — плейлист; название — из блока описания или имени файла.
    Playlist { title: String },
}

/// Разбирает архив Takeout. Синхронный: вызывать из `spawn_blocking`.
pub fn parse_takeout<R: Read + Seek>(
    reader: R,
    limits: Limits,
) -> Result<Collection, ArchiveError> {
    let mut zip = zip::ZipArchive::new(reader).map_err(|e| match e {
        zip::result::ZipError::InvalidArchive(_) => ArchiveError::NotZip,
        other => ArchiveError::Broken(other.to_string()),
    })?;
    if zip.len() > limits.max_entries * 4 {
        return Err(ArchiveError::TooManyEntries);
    }
    let mut wanted: Vec<(usize, String)> = Vec::new();
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
        let base = file_name(&name);
        if !base.to_lowercase().ends_with(".csv") || base.starts_with("._") {
            continue;
        }
        if file.size() > limits.max_entry_bytes {
            return Err(ArchiveError::EntryTooLarge(name));
        }
        if file.size() > 1024 * 1024
            && file.size() / file.compressed_size().max(1) > limits.max_ratio
        {
            return Err(ArchiveError::Bomb(name));
        }
        wanted.push((i, name));
    }
    // Сначала песни с именами, затем плейлисты — по имени файла, как в архиве
    let mut sheets = Vec::new();
    let mut total = 0u64;
    for (i, name) in wanted {
        let file = zip
            .by_index(i)
            .map_err(|e| ArchiveError::Broken(e.to_string()))?;
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
        let text = String::from_utf8_lossy(&bytes).into_owned();
        sheets.push((name, text));
    }
    sheets.sort_by(|a, b| a.0.cmp(&b.0));
    let mut c = Collection::default();
    for pass in [true, false] {
        for (name, text) in &sheets {
            let Some((sheet, rows)) = read_sheet(name, text) else {
                continue;
            };
            let songs = sheet == Sheet::Songs;
            if songs != pass {
                continue;
            }
            match sheet {
                Sheet::Songs => {
                    for r in rows {
                        c.add(collection::Track { liked: true, ..r });
                    }
                }
                Sheet::Playlist { title } => match playlist_role(&title) {
                    Role::Skip => {}
                    Role::Likes => {
                        for r in rows {
                            c.add(collection::Track { liked: true, ..r });
                        }
                    }
                    Role::Playlist => {
                        let positions: Vec<usize> =
                            rows.into_iter().filter_map(|r| c.add(r)).collect();
                        if !positions.is_empty() {
                            c.playlist(&title, "", positions);
                        }
                    }
                },
            }
        }
    }
    if c.is_empty() {
        return Err(ArchiveError::Empty);
    }
    Ok(c)
}

enum Role {
    Likes,
    Playlist,
    Skip,
}

fn playlist_role(title: &str) -> Role {
    let t = title.trim().to_lowercase();
    match t.as_str() {
        "liked videos"
        | "liked music"
        | "понравившиеся"
        | "понравившиеся видео"
        | "понравившаяся музыка"
        | "ll"
        | "lm" => Role::Likes,
        "watch later" | "смотреть позже" | "wl" => Role::Skip,
        _ => Role::Playlist,
    }
}

fn file_name(path: &str) -> &str {
    path.rsplit(['/', '\\']).next().unwrap_or(path)
}

/// Название плейлиста из имени файла: «Ночь-videos.csv» → «Ночь».
fn title_from_file(path: &str) -> String {
    let base = file_name(path);
    let stem = base.rsplit_once('.').map(|(s, _)| s).unwrap_or(base).trim();
    for suffix in ["-videos", " - videos", "-видео", " - видео", "-відео"] {
        if let Some(s) = stem.strip_suffix(suffix) {
            return s.trim().to_string();
        }
    }
    stem.to_string()
}

#[derive(Debug, Default)]
struct Cols {
    video: Option<usize>,
    title: Option<usize>,
    album: Option<usize>,
    artists: Vec<usize>,
}

fn columns(cells: &[String]) -> Cols {
    let mut c = Cols::default();
    for (i, cell) in cells.iter().enumerate() {
        let h = cell
            .trim()
            .trim_start_matches('\u{feff}')
            .to_lowercase()
            .replace(['_', '-'], " ");
        let h = h.trim();
        if c.video.is_none()
            && (matches!(
                h,
                "video id" | "videoid" | "id видео" | "идентификатор видео" | "ид видео"
            ) || (h.contains("video") && h.ends_with(" id"))
                || (h.contains("видео") && (h.starts_with("id") || h.starts_with("идентиф"))))
        {
            c.video = Some(i);
        } else if c.title.is_none()
            && (matches!(
                h,
                "song title"
                    | "title"
                    | "song"
                    | "track"
                    | "название"
                    | "название песни"
                    | "название трека"
            ) || h.starts_with("song title")
                || h.starts_with("название песни"))
        {
            c.title = Some(i);
        } else if c.album.is_none()
            && (h.starts_with("album")
                || h.starts_with("альбом")
                || h.starts_with("название альбома"))
        {
            c.album = Some(i);
        } else if h.starts_with("artist") || h.starts_with("исполнител") || h.starts_with("артист")
        {
            c.artists.push(i);
        }
    }
    c
}

/// Строки CSV с переносами внутри кавычек.
fn csv_rows(text: &str) -> Vec<Vec<String>> {
    let text = text.trim_start_matches('\u{feff}');
    let mut rows = Vec::new();
    let mut row = Vec::new();
    let mut cur = String::new();
    let mut quoted = false;
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '"' if quoted && chars.peek() == Some(&'"') => {
                cur.push('"');
                chars.next();
            }
            '"' => quoted = !quoted,
            ',' if !quoted => row.push(std::mem::take(&mut cur)),
            '\r' if !quoted => {}
            '\n' if !quoted => {
                row.push(std::mem::take(&mut cur));
                rows.push(std::mem::take(&mut row));
            }
            c => cur.push(c),
        }
    }
    if !cur.is_empty() || !row.is_empty() {
        row.push(cur);
        rows.push(row);
    }
    rows.into_iter()
        .map(|r| r.into_iter().map(|c| c.trim().to_string()).collect())
        .collect()
}

fn read_sheet(path: &str, text: &str) -> Option<(Sheet, Vec<collection::Track>)> {
    let rows = csv_rows(text);
    let lower = file_name(path).to_lowercase();
    // Загрузки своих файлов и служебные таблицы
    if lower.contains("upload") || lower.contains("загруз") || lower == "playlists.csv" {
        return None;
    }
    // Старый формат: блок описания плейлиста (Playlist Id, …, Title, …), пустая строка, затем видео
    let mut title = None;
    let mut start = 0;
    if let Some(head) = rows.first() {
        let h: Vec<String> = head.iter().map(|c| c.to_lowercase()).collect();
        if h.iter().any(|c| c == "playlist id" || c == "id плейлиста")
            && !h.iter().any(|c| c.contains("video") || c.contains("видео"))
        {
            let ti = h
                .iter()
                .position(|c| c == "title" || c == "название")
                .unwrap_or(4);
            title = rows.get(1).and_then(|r| r.get(ti)).cloned();
            start = rows
                .iter()
                .skip(2)
                .position(|r| r.iter().all(String::is_empty))
                .map(|p| p + 3)
                .unwrap_or(rows.len());
        }
    }
    let header = rows.get(start)?;
    let cols = columns(header);
    let data: Vec<&Vec<String>> = rows
        .iter()
        .skip(start + 1)
        .filter(|r| !r.iter().all(String::is_empty))
        .collect();
    // Без заголовка Video ID — колонка, где все значения похожи на id видео
    let video = cols.video.or_else(|| {
        (0..header.len()).find(|i| {
            !data.is_empty()
                && data
                    .iter()
                    .all(|r| r.get(*i).is_some_and(|v| valid_video_id(v)))
        })
    })?;
    let cell = |r: &Vec<String>, i: Option<usize>| -> String {
        i.and_then(|i| r.get(i)).cloned().unwrap_or_default()
    };
    let named = cols.title.is_some();
    let tracks: Vec<collection::Track> = data
        .into_iter()
        .filter_map(|r| {
            let id = r.get(video)?.trim();
            if !valid_video_id(id) {
                return None;
            }
            let artists: Vec<String> = cols
                .artists
                .iter()
                .filter_map(|i| r.get(*i))
                .map(|a| clean_channel(a))
                .filter(|a| !a.is_empty())
                .collect();
            let album = cell(r, cols.album);
            Some(collection::Track {
                artist: artists.join(", "),
                title: cell(r, cols.title),
                album: (!album.is_empty()).then_some(album),
                duration_ms: None,
                source_ref: Some(format!("yt:{id}")),
                liked: false,
            })
        })
        .collect();
    if named && !cols.artists.is_empty() {
        Some((Sheet::Songs, tracks))
    } else {
        Some((
            Sheet::Playlist {
                title: title
                    .filter(|t| !t.trim().is_empty())
                    .unwrap_or_else(|| title_from_file(path)),
            },
            tracks,
        ))
    }
}

#[cfg(test)]
mod tests {
    use std::io::{Cursor, Write};

    use zip::write::SimpleFileOptions;

    use super::*;

    fn zip_of(files: &[(&str, &str)]) -> Vec<u8> {
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

    #[test]
    fn playlist_links() {
        let id = "PLntVvZRJe0d-6aEQPLPW4C3VjzjP2XbG8";
        for raw in [
            format!("https://www.youtube.com/playlist?list={id}"),
            format!("https://music.youtube.com/playlist?list={id}&si=abc"),
            format!("https://music.youtube.com/watch?v=QoKpz_95OIM&list={id}"),
            format!("youtu.be/QoKpz_95OIM?list={id}"),
            format!("https://music.youtube.com/browse/VL{id}"),
            format!("https://m.youtube.com/playlist?list={id}"),
            id.to_string(),
        ] {
            assert_eq!(playlist_id(&raw).as_deref(), Some(id), "{raw}");
        }
        assert_eq!(
            playlist_id("https://music.youtube.com/playlist?list=OLAK5uy_kxyz0123456789")
                .as_deref(),
            Some("OLAK5uy_kxyz0123456789")
        );
        // Свои «Понравившиеся» и миксы — не публичные плейлисты
        assert_eq!(
            playlist_id("https://music.youtube.com/playlist?list=LM"),
            None
        );
        assert_eq!(
            playlist_id("https://www.youtube.com/watch?v=QoKpz_95OIM"),
            None
        );
        assert_eq!(
            playlist_id("https://example.com/playlist?list=PLxxxxxxxxxxxx"),
            None
        );
    }

    #[test]
    fn video_titles() {
        let ch = |s: &str| vec![s.to_string()];
        assert_eq!(
            song(
                "Devil",
                &ch("Lowborn - Topic"),
                Some("MUSIC_VIDEO_TYPE_ATV")
            ),
            ("Lowborn".into(), "Devil".into())
        );
        // Песня с дефисом в названии остаётся как есть
        assert_eq!(
            song("Hey - Ho", &ch("Band"), Some("MUSIC_VIDEO_TYPE_ATV")),
            ("Band".into(), "Hey - Ho".into())
        );
        assert_eq!(
            song(
                "Slavik Pogosov - Выпуская дым | 2019 Премьера",
                &ch("Zhara Music"),
                Some("MUSIC_VIDEO_TYPE_UGC")
            ),
            ("Slavik Pogosov".into(), "Выпуская дым".into())
        );
        assert_eq!(
            song(
                "Never Gonna Give You Up",
                &ch("RickAstleyVEVO"),
                Some("MUSIC_VIDEO_TYPE_OMV")
            ),
            ("RickAstley".into(), "Never Gonna Give You Up".into())
        );
        assert!(is_music(Some("MUSIC_VIDEO_TYPE_OMV")));
        assert!(!is_music(None));
    }

    const LIBRARY_SONGS: &str = "\u{feff}Video ID,Song Title,Album Title,Artist Name 1,Artist Name 2\r\n\
        QoKpz_95OIM,Devil,Devil,Lowborn,\r\n\
        dQw4w9WgXcQ,\"Never Gonna Give You Up\",Whenever You Need Somebody,Rick Astley,\r\n\
        aaaaaaaaaaa,\"Light Me Up, Again\",Light,Lowborn,vista\r\n";

    const NEW_PLAYLIST: &str = "Video ID,Playlist Video Creation Timestamp\n\
        jawJo8m02nA,2024-03-01T18:42:00+00:00\n\
        QoKpz_95OIM,2024-03-01T18:43:00+00:00\n";

    const OLD_PLAYLIST: &str = "Playlist Id,Channel Id,Time Created,Time Updated,Title,Description,Visibility\n\
        PLabc,UCxyz,2020-01-01 00:00:00 UTC,2021-01-01 00:00:00 UTC,Дорога,,Private\n\
        \n\
        Video Id,Time Added\n\
        bbbbbbbbbbb,2020-01-02 00:00:00 UTC\n";

    #[test]
    fn takeout_in_english() {
        let zip = zip_of(&[
            (
                "Takeout/YouTube and YouTube Music/music (library and uploads)/music library songs.csv",
                LIBRARY_SONGS,
            ),
            (
                "Takeout/YouTube and YouTube Music/music (library and uploads)/music uploads metadata.csv",
                "Video ID,Song Title,Artist Name 1\nccccccccccc,Мой файл,Я\n",
            ),
            (
                "Takeout/YouTube and YouTube Music/playlists/Ночь-videos.csv",
                NEW_PLAYLIST,
            ),
            (
                "Takeout/YouTube and YouTube Music/playlists/Liked videos-videos.csv",
                "Video ID,Playlist Video Creation Timestamp\nddddddddddd,2024-01-01T00:00:00+00:00\n",
            ),
            (
                "Takeout/YouTube and YouTube Music/playlists/Watch later-videos.csv",
                "Video ID,Playlist Video Creation Timestamp\neeeeeeeeeee,2024-01-01T00:00:00+00:00\n",
            ),
            (
                "Takeout/YouTube and YouTube Music/playlists/playlists.csv",
                "Playlist ID,Add new videos to top,Playlist Title (Original),Playlist Visibility\nPL1,False,Ночь,Private\n",
            ),
            (
                "Takeout/YouTube and YouTube Music/subscriptions/subscriptions.csv",
                "Channel Id,Channel Url,Channel Title\nUC1,http://x,Y\n",
            ),
            ("Takeout/archive_browser.html", "<html></html>"),
        ]);
        let c = parse_takeout(Cursor::new(zip), Limits::default()).unwrap();
        let names: Vec<(&str, &str, bool)> = c
            .tracks
            .iter()
            .map(|t| (t.artist.as_str(), t.title.as_str(), t.liked))
            .collect();
        assert_eq!(
            names,
            [
                ("Lowborn", "Devil", true),
                ("Rick Astley", "Never Gonna Give You Up", true),
                ("Lowborn, vista", "Light Me Up, Again", true),
                ("", "", true),
                ("", "", false),
            ]
        );
        assert_eq!(c.tracks[3].source_ref.as_deref(), Some("yt:ddddddddddd"));
        assert_eq!(c.tracks[0].album.as_deref(), Some("Devil"));
        assert_eq!(c.playlists.len(), 1);
        assert_eq!(c.playlists[0].name, "Ночь");
        // Видео из библиотеки в плейлисте — та же позиция
        assert_eq!(c.playlists[0].tracks, vec![4, 0]);
    }

    #[test]
    fn takeout_in_russian_and_old_format() {
        let zip = zip_of(&[
            (
                "Takeout/YouTube и YouTube Music/плейлисты/Понравившиеся-видео.csv",
                "ID видео,Время создания видео в плейлисте\nfffffffffff,2024-01-01T00:00:00+00:00\n",
            ),
            (
                "Takeout/YouTube и YouTube Music/плейлисты/Дорога.csv",
                OLD_PLAYLIST,
            ),
        ]);
        let c = parse_takeout(Cursor::new(zip), Limits::default()).unwrap();
        assert_eq!(c.tracks.len(), 2);
        assert!(
            c.tracks
                .iter()
                .any(|t| t.liked && t.source_ref.as_deref() == Some("yt:fffffffffff"))
        );
        assert_eq!(c.playlists.len(), 1);
        assert_eq!(c.playlists[0].name, "Дорога");
    }

    #[test]
    fn takeout_without_music() {
        let zip = zip_of(&[(
            "Takeout/YouTube and YouTube Music/history/watch-history.html",
            "<html/>",
        )]);
        assert!(matches!(
            parse_takeout(Cursor::new(zip), Limits::default()),
            Err(ArchiveError::Empty)
        ));
        assert!(matches!(
            parse_takeout(Cursor::new(b"nope".to_vec()), Limits::default()),
            Err(ArchiveError::NotZip)
        ));
    }

    #[test]
    fn playlist_from_fetcher() {
        let p = YtPlaylist {
            title: "Russian rap".into(),
            description: String::new(),
            tracks: vec![
                YtTrack {
                    video_id: Some("jawJo8m02nA".into()),
                    title: "Slavik Pogosov - Выпуская дым | 2019 Премьера".into(),
                    artists: vec!["Zhara Music".into()],
                    duration_ms: Some(222_000),
                    video_type: Some("MUSIC_VIDEO_TYPE_UGC".into()),
                    ..YtTrack::default()
                },
                YtTrack {
                    title: " ".into(),
                    ..YtTrack::default()
                },
            ],
        };
        let c = playlist_collection(&p);
        assert_eq!(c.tracks.len(), 1);
        assert_eq!(c.tracks[0].artist, "Slavik Pogosov");
        assert_eq!(c.tracks[0].source_ref.as_deref(), Some("yt:jawJo8m02nA"));
        assert_eq!(c.playlists[0].name, "Russian rap");
    }
}
