//! Выгрузка медиатеки в файл: CSV для сервисов переноса (TuneMyMusic, Soundiiz) и Excel, M3U8 для
//! плееров, JSON со всем (лайки, плейлисты с порядком и папками, альбомы, подписки, история) и ZIP —
//! вся медиатека отдельными файлами.
//!
//! Файл собирается потоком: строки из базы идут курсором, готовые куски по 64 КБ уходят в тело
//! ответа, поэтому большая медиатека не копится в памяти. Клиент оборвал загрузку — сборка
//! останавливается на следующем куске.

use std::{
    collections::HashSet,
    io::{self, Write},
    sync::{Arc, Mutex},
};

use axum::{
    body::Body,
    http::{HeaderValue, header},
    response::{IntoResponse, Response},
};
use bytes::Bytes;
use chrono::{DateTime, Datelike, SecondsFormat, Timelike, Utc};
use dalywave_protocol::export::ExportFormat;
use futures_util::TryStreamExt;
use serde::Serialize;
use sqlx::PgPool;
use tokio::sync::{OwnedSemaphorePermit, Semaphore, mpsc};
use uuid::Uuid;
use zip::{CompressionMethod, ZipWriter, write::SimpleFileOptions, write::StreamWriter};

/// Кусок тела ответа.
const CHUNK: usize = 64 * 1024;
/// Сколько выгрузок собирается одновременно на весь сервер.
pub const PARALLEL: usize = 4;
/// UTF-8 BOM: без него Excel открывает CSV в кодировке Windows и кириллица ломается.
pub const BOM: &[u8] = b"\xEF\xBB\xBF";
/// Заголовок таблицы треков — в духе Exportify: эти названия колонок узнают сервисы переноса.
pub const TRACKS_HEADER: [&str; 6] = [
    "Artist Name(s)",
    "Track Name",
    "Album Name",
    "ISRC",
    "Duration (ms)",
    "Added At",
];
const ALBUMS_HEADER: [&str; 5] = [
    "Artist Name(s)",
    "Album Name",
    "UPC",
    "Release Date",
    "Added At",
];
const ARTISTS_HEADER: [&str; 2] = ["Artist Name", "Added At"];
const HISTORY_HEADER: [&str; 7] = [
    "Played At",
    "Artist Name(s)",
    "Track Name",
    "Album Name",
    "ISRC",
    "Played (ms)",
    "End Reason",
];

/// Очередь выгрузок: больше `PARALLEL` одновременно сервер не собирает.
pub fn limiter() -> Arc<Semaphore> {
    static LIMIT: std::sync::OnceLock<Arc<Semaphore>> = std::sync::OnceLock::new();
    LIMIT
        .get_or_init(|| Arc::new(Semaphore::new(PARALLEL)))
        .clone()
}

// ---------- форматы ----------

/// Ячейка CSV по RFC 4180: в кавычках, если внутри запятая, кавычка, перевод строки или пробел по краю.
pub fn csv_cell(value: &str) -> String {
    let quote = value.contains([',', '"', '\n', '\r'])
        || value.starts_with(char::is_whitespace)
        || value.ends_with(char::is_whitespace);
    if quote {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_string()
    }
}

/// Строка CSV с переводом строки CRLF (как ждёт Excel).
pub fn csv_line<S: AsRef<str>>(cells: &[S]) -> String {
    let mut out = cells
        .iter()
        .map(|c| csv_cell(c.as_ref()))
        .collect::<Vec<_>>()
        .join(",");
    out.push_str("\r\n");
    out
}

/// Строка для M3U8: переводы строк внутри названия сломали бы список.
fn m3u_text(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect::<String>()
        .trim()
        .to_string()
}

pub fn time(at: DateTime<Utc>) -> String {
    at.to_rfc3339_opts(SecondsFormat::Secs, true)
}

/// Имя файла из названия: без символов, запрещённых в Windows и macOS, без управляющих, не длиннее
/// 80 символов. Пустое — `fallback`.
pub fn file_name(title: &str, fallback: &str) -> String {
    let cleaned: String = title
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            c if c.is_control() => ' ',
            c => c,
        })
        .collect();
    let cleaned = cleaned.split_whitespace().collect::<Vec<_>>().join(" ");
    // Точка в начале прячет файл, в конце — Windows её отрезает
    let cleaned = cleaned.trim_matches(|c: char| c == '.' || c.is_whitespace());
    let short: String = cleaned.chars().take(80).collect();
    let short = short.trim_end().to_string();
    if short.is_empty() {
        fallback.to_string()
    } else {
        short
    }
}

/// Имена внутри архива не повторяются: второй «Ночь» становится «Ночь (2)».
#[derive(Default)]
pub struct Names(HashSet<String>);

impl Names {
    pub fn unique(&mut self, dir: &str, base: &str) -> String {
        let mut n = 1;
        loop {
            let name = if n == 1 {
                base.to_string()
            } else {
                format!("{base} ({n})")
            };
            let key = format!("{dir}/{name}").to_lowercase();
            if self.0.insert(key) {
                return name;
            }
            n += 1;
        }
    }
}

/// `Content-Disposition` с именем файла: `filename*` в UTF-8 для кириллицы, `filename` — запасное
/// ASCII-имя для старых клиентов.
pub fn disposition(name: &str, ascii_fallback: &str) -> HeaderValue {
    let ascii = if name
        .chars()
        .all(|c| c.is_ascii_graphic() && c != '"' && c != '\\' || c == ' ')
    {
        name.to_string()
    } else {
        ascii_fallback.to_string()
    };
    let mut encoded = String::new();
    for b in name.bytes() {
        // attr-char из RFC 5987: буквы, цифры и !#$&+-.^_`|~
        if b.is_ascii_alphanumeric() || b"!#$&+-.^_`|~".contains(&b) {
            encoded.push(b as char);
        } else {
            encoded.push_str(&format!("%{b:02X}"));
        }
    }
    HeaderValue::from_str(&format!(
        "attachment; filename=\"{ascii}\"; filename*=UTF-8''{encoded}"
    ))
    .unwrap_or_else(|_| HeaderValue::from_static("attachment"))
}

// ---------- строки ----------

/// Трек в выгрузке.
#[derive(Debug, Clone)]
pub struct TrackRow {
    pub id: Uuid,
    pub title: String,
    pub version: String,
    pub artists: Vec<String>,
    pub album: Option<String>,
    pub isrc: Option<String>,
    pub duration_ms: i32,
    pub deezer_id: Option<i64>,
    pub added_at: Option<DateTime<Utc>>,
}

impl TrackRow {
    /// Название с версией: «Dachi (Slowed)».
    pub fn full_title(&self) -> String {
        if self.version.trim().is_empty() {
            self.title.clone()
        } else {
            format!("{} {}", self.title, self.version.trim())
        }
    }

    pub fn csv(&self) -> String {
        csv_line(&[
            self.artists.join(", "),
            self.full_title(),
            self.album.clone().unwrap_or_default(),
            self.isrc.clone().unwrap_or_default(),
            self.duration_ms.to_string(),
            self.added_at.map(time).unwrap_or_default(),
        ])
    }

    pub fn m3u(&self, public_url: &str) -> String {
        format!(
            "#EXTINF:{},{} - {}\n{public_url}/track/{}\n",
            (self.duration_ms.max(0) + 500) / 1000,
            m3u_text(&self.artists.join(", ")),
            m3u_text(&self.full_title()),
            self.id
        )
    }

    fn json(&self, public_url: &str) -> TrackJson<'_> {
        TrackJson {
            id: self.id,
            title: &self.title,
            version: &self.version,
            artists: &self.artists,
            album: self.album.as_deref(),
            isrc: self.isrc.as_deref(),
            duration_ms: self.duration_ms,
            deezer_id: self.deezer_id,
            url: format!("{public_url}/track/{}", self.id),
            added_at: self.added_at.map(time),
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TrackJson<'a> {
    id: Uuid,
    title: &'a str,
    version: &'a str,
    artists: &'a [String],
    album: Option<&'a str>,
    isrc: Option<&'a str>,
    duration_ms: i32,
    deezer_id: Option<i64>,
    url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    added_at: Option<String>,
}

/// Список треков: любимые или плейлист.
#[derive(Debug, Clone, Copy)]
pub enum List {
    Liked(Uuid),
    Playlist(Uuid),
}

/// Строки списка курсором по порядку, в котором их видит человек.
fn list_rows(
    db: &PgPool,
    list: List,
) -> futures_util::stream::BoxStream<'_, Result<TrackRow, sqlx::Error>> {
    match list {
        List::Liked(user) => sqlx::query_as!(
            TrackRow,
            r#"SELECT t.id, t.title, t.title_version AS version, al.title AS "album?", t.isrc,
                      t.duration_ms, t.deezer_id, l.created_at AS "added_at?",
                      ARRAY(SELECT a.name FROM track_artists ta JOIN artists a ON a.id = ta.artist_id
                            WHERE ta.track_id = t.id ORDER BY ta.position, ta.artist_id) AS "artists!"
               FROM liked_tracks l JOIN tracks t ON t.id = l.track_id
               LEFT JOIN albums al ON al.id = t.album_id
               WHERE l.user_id = $1
               ORDER BY l.created_at DESC, t.id"#,
            user
        )
        .fetch(db),
        List::Playlist(id) => sqlx::query_as!(
            TrackRow,
            r#"SELECT t.id, t.title, t.title_version AS version, al.title AS "album?", t.isrc,
                      t.duration_ms, t.deezer_id, i.added_at AS "added_at?",
                      ARRAY(SELECT a.name FROM track_artists ta JOIN artists a ON a.id = ta.artist_id
                            WHERE ta.track_id = t.id ORDER BY ta.position, ta.artist_id) AS "artists!"
               FROM playlist_items i JOIN tracks t ON t.id = i.track_id
               LEFT JOIN albums al ON al.id = t.album_id
               WHERE i.playlist_id = $1
               ORDER BY i.position, i.id"#,
            id
        )
        .fetch(db),
    }
}

struct AlbumRow {
    id: Uuid,
    title: String,
    artist: String,
    upc: Option<String>,
    release_date: Option<chrono::NaiveDate>,
    added_at: DateTime<Utc>,
}

struct ArtistRow {
    id: Uuid,
    name: String,
    added_at: DateTime<Utc>,
}

struct HistoryRow {
    played_at: DateTime<Utc>,
    played_ms: i32,
    end_reason: String,
    track: TrackRow,
}

/// Плейлист медиатеки: свой или совместный, где человек участник.
#[derive(Debug, Clone)]
pub struct PlaylistRow {
    pub id: Uuid,
    pub title: String,
    pub description: String,
    pub kind: String,
    pub pinned: bool,
    pub collaborative: bool,
    pub own: bool,
    pub folder_id: Option<Uuid>,
    pub folder: Option<String>,
    pub position: f64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

struct FolderRow {
    id: Uuid,
    title: String,
    position: f64,
}

/// Плейлисты в «Своём порядке»: папки и плейлисты вне папок — по общей шкале, внутри папки — по её.
async fn playlists(db: &PgPool, user: Uuid) -> Result<Vec<PlaylistRow>, sqlx::Error> {
    sqlx::query_as!(
        PlaylistRow,
        r#"SELECT p.id, p.title, p.description, p.kind, p.collaborative,
                  p.user_id = $1 AS "own!",
                  COALESCE(m.pinned, p.pinned) AS "pinned!",
                  CASE WHEN p.user_id = $1 THEN p.folder_id END AS folder_id,
                  f.title AS "folder?",
                  COALESCE(m.position, p.position) AS "position!",
                  p.created_at, p.updated_at
           FROM playlists p
           LEFT JOIN playlist_members m ON m.playlist_id = p.id AND m.user_id = $1 AND p.user_id <> $1
           LEFT JOIN playlist_folders f ON f.id = p.folder_id AND p.user_id = $1
           WHERE p.user_id = $1 OR (m.user_id IS NOT NULL AND p.collaborative)
           ORDER BY COALESCE(f.position, m.position, p.position), (f.id IS NOT NULL),
                    COALESCE(m.position, p.position), p.id"#,
        user
    )
    .fetch_all(db)
    .await
}

// ---------- поток ----------

/// Клиент закрыл соединение: собирать дальше незачем.
#[derive(Debug)]
pub struct Gone;

#[derive(Debug, thiserror::Error)]
pub enum BuildError {
    #[error("client is gone")]
    Gone,
    #[error(transparent)]
    Db(#[from] sqlx::Error),
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error(transparent)]
    Zip(#[from] zip::result::ZipError),
}

impl From<Gone> for BuildError {
    fn from(_: Gone) -> Self {
        BuildError::Gone
    }
}

/// Буфер, общий для архиватора и отправителя кусков.
#[derive(Clone, Default)]
struct Shared(Arc<Mutex<Vec<u8>>>);

impl Write for Shared {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl Shared {
    fn len(&self) -> usize {
        self.0.lock().unwrap_or_else(|e| e.into_inner()).len()
    }

    fn take(&self) -> Vec<u8> {
        std::mem::take(&mut *self.0.lock().unwrap_or_else(|e| e.into_inner()))
    }
}

/// Куда пишется выгрузка: один файл как есть или файлы внутри ZIP.
pub struct Out {
    tx: mpsc::Sender<io::Result<Bytes>>,
    buf: Shared,
    zip: Option<ZipWriter<StreamWriter<Shared>>>,
    stamp: zip::DateTime,
}

impl Out {
    fn new(tx: mpsc::Sender<io::Result<Bytes>>, zip: bool, now: DateTime<Utc>) -> Self {
        let buf = Shared::default();
        let stamp = zip::DateTime::from_date_and_time(
            now.year().clamp(1980, 2107) as u16,
            now.month() as u8,
            now.day() as u8,
            now.hour() as u8,
            now.minute() as u8,
            now.second() as u8,
        )
        .unwrap_or_default();
        Self {
            tx,
            zip: zip.then(|| ZipWriter::new_stream(buf.clone())),
            buf,
            stamp,
        }
    }

    /// Новый файл внутри архива (без архива — ничего).
    fn file(&mut self, name: &str) -> Result<(), BuildError> {
        if let Some(zip) = self.zip.as_mut() {
            let options = SimpleFileOptions::default()
                .compression_method(CompressionMethod::Deflated)
                .last_modified_time(self.stamp);
            zip.start_file(name, options)?;
        }
        Ok(())
    }

    fn put(&mut self, data: &[u8]) -> Result<(), BuildError> {
        match self.zip.as_mut() {
            Some(zip) => zip.write_all(data)?,
            None => self.buf.write_all(data)?,
        }
        Ok(())
    }

    /// Отдать накопленное, если набрался кусок.
    async fn pump(&mut self) -> Result<(), Gone> {
        if self.buf.len() >= CHUNK {
            self.send().await?;
        }
        Ok(())
    }

    async fn send(&mut self) -> Result<(), Gone> {
        let chunk = self.buf.take();
        if chunk.is_empty() {
            return Ok(());
        }
        self.tx.send(Ok(Bytes::from(chunk))).await.map_err(|_| Gone)
    }

    async fn finish(mut self) -> Result<(), BuildError> {
        if let Some(zip) = self.zip.take() {
            zip.finish()?;
        }
        self.send().await?;
        Ok(())
    }
}

/// Ответ-файл, который собирает `build`. Сборка идёт в своей задаче; разрешение из очереди
/// выгрузок держится до её конца.
pub fn respond<F, Fut>(
    format: ExportFormat,
    name: &str,
    fallback: &str,
    permit: OwnedSemaphorePermit,
    build: F,
) -> Response
where
    F: FnOnce(Out) -> Fut + Send + 'static,
    Fut: std::future::Future<Output = Result<(), BuildError>> + Send + 'static,
{
    let (tx, rx) = mpsc::channel::<io::Result<Bytes>>(4);
    let out = Out::new(tx.clone(), format == ExportFormat::Zip, Utc::now());
    tokio::spawn(async move {
        let _permit = permit;
        match build(out).await {
            Ok(()) | Err(BuildError::Gone) => {}
            Err(e) => {
                tracing::warn!(error = %e, "export failed");
                // Оборванное тело: браузер покажет ошибку загрузки, а не битый файл как готовый
                let _ = tx.send(Err(io::Error::other("export failed"))).await;
            }
        }
    });
    let stream = futures_util::stream::unfold(rx, |mut rx| async move {
        rx.recv().await.map(|item| (item, rx))
    });
    let mut res = Body::from_stream(stream).into_response();
    let headers = res.headers_mut();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static(format.mime()),
    );
    headers.insert(header::CONTENT_DISPOSITION, disposition(name, fallback));
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    res
}

// ---------- сборка ----------

/// Список треков одним файлом: CSV, M3U8 или JSON.
pub async fn list(
    out: &mut Out,
    db: &PgPool,
    public_url: &str,
    list: List,
    title: &str,
    format: ExportFormat,
) -> Result<(), BuildError> {
    let mut rows = list_rows(db, list);
    match format {
        ExportFormat::Csv => {
            out.put(BOM)?;
            out.put(csv_line(&TRACKS_HEADER).as_bytes())?;
            while let Some(row) = rows.try_next().await? {
                out.put(row.csv().as_bytes())?;
                out.pump().await?;
            }
        }
        ExportFormat::M3u8 => {
            out.put(format!("#EXTM3U\n#PLAYLIST:{}\n", m3u_text(title)).as_bytes())?;
            while let Some(row) = rows.try_next().await? {
                out.put(row.m3u(public_url).as_bytes())?;
                out.pump().await?;
            }
        }
        ExportFormat::Json | ExportFormat::Zip => {
            out.put(b"{\"format\":\"dalywave\",\"version\":1,\"exportedAt\":")?;
            out.put(&serde_json::to_vec(&time(Utc::now())).unwrap_or_default())?;
            out.put(b",\"title\":")?;
            out.put(&serde_json::to_vec(title).unwrap_or_default())?;
            out.put(b",\"tracks\":[")?;
            let mut first = true;
            while let Some(row) = rows.try_next().await? {
                if !first {
                    out.put(b",")?;
                }
                first = false;
                out.put(&serde_json::to_vec(&row.json(public_url)).unwrap_or_default())?;
                out.pump().await?;
            }
            out.put(b"]}\n")?;
        }
    }
    Ok(())
}

/// Вся медиатека одним JSON. `history` — с историей прослушиваний.
async fn library_json(
    out: &mut Out,
    db: &PgPool,
    public_url: &str,
    user: Uuid,
    history: bool,
) -> Result<(), BuildError> {
    let name = sqlx::query_scalar!("SELECT name FROM users WHERE id = $1", user)
        .fetch_optional(db)
        .await?
        .unwrap_or_default();
    out.put(b"{\"format\":\"dalywave-library\",\"version\":1,\"exportedAt\":")?;
    out.put(&serde_json::to_vec(&time(Utc::now())).unwrap_or_default())?;
    out.put(b",\"user\":")?;
    out.put(&serde_json::to_vec(&serde_json::json!({ "name": name })).unwrap_or_default())?;

    out.put(b",\"likedTracks\":[")?;
    let mut rows = list_rows(db, List::Liked(user));
    let mut first = true;
    while let Some(row) = rows.try_next().await? {
        if !std::mem::take(&mut first) {
            out.put(b",")?;
        }
        out.put(&serde_json::to_vec(&row.json(public_url)).unwrap_or_default())?;
        out.pump().await?;
    }
    drop(rows);

    out.put(b"],\"savedAlbums\":[")?;
    let albums = saved_albums(db, user).await?;
    for (i, a) in albums.iter().enumerate() {
        if i > 0 {
            out.put(b",")?;
        }
        let v = serde_json::json!({
            "id": a.id,
            "title": a.title,
            "artist": a.artist,
            "upc": a.upc,
            "releaseDate": a.release_date.map(|d| d.to_string()),
            "url": format!("{public_url}/album/{}", a.id),
            "addedAt": time(a.added_at),
        });
        out.put(&serde_json::to_vec(&v).unwrap_or_default())?;
    }
    out.pump().await?;

    out.put(b"],\"followedArtists\":[")?;
    let artists = followed_artists(db, user).await?;
    for (i, a) in artists.iter().enumerate() {
        if i > 0 {
            out.put(b",")?;
        }
        let v = serde_json::json!({
            "id": a.id,
            "name": a.name,
            "url": format!("{public_url}/artist/{}", a.id),
            "addedAt": time(a.added_at),
        });
        out.put(&serde_json::to_vec(&v).unwrap_or_default())?;
    }
    out.pump().await?;

    out.put(b"],\"folders\":[")?;
    let folders = sqlx::query_as!(
        FolderRow,
        "SELECT id, title, position FROM playlist_folders WHERE user_id = $1 ORDER BY position, id",
        user
    )
    .fetch_all(db)
    .await?;
    for (i, f) in folders.iter().enumerate() {
        if i > 0 {
            out.put(b",")?;
        }
        let v = serde_json::json!({ "id": f.id, "title": f.title, "position": f.position });
        out.put(&serde_json::to_vec(&v).unwrap_or_default())?;
    }

    out.put(b"],\"playlists\":[")?;
    for (i, p) in playlists(db, user).await?.iter().enumerate() {
        if i > 0 {
            out.put(b",")?;
        }
        let head = serde_json::json!({
            "id": p.id,
            "title": p.title,
            "description": p.description,
            "kind": p.kind,
            "own": p.own,
            "collaborative": p.collaborative,
            "pinned": p.pinned,
            "folderId": p.folder_id,
            "folder": p.folder,
            "position": p.position,
            "url": format!("{public_url}/playlist/{}", p.id),
            "createdAt": time(p.created_at),
            "updatedAt": time(p.updated_at),
        });
        // Объект без закрывающей скобки — дальше дописываются треки
        let mut head = serde_json::to_vec(&head).unwrap_or_default();
        head.pop();
        out.put(&head)?;
        out.put(b",\"tracks\":[")?;
        let mut rows = list_rows(db, List::Playlist(p.id));
        let mut first = true;
        while let Some(row) = rows.try_next().await? {
            if !std::mem::take(&mut first) {
                out.put(b",")?;
            }
            out.put(&serde_json::to_vec(&row.json(public_url)).unwrap_or_default())?;
            out.pump().await?;
        }
        out.put(b"]}")?;
    }
    out.put(b"]")?;

    if history {
        out.put(b",\"history\":[")?;
        let mut rows = history_rows(db, user);
        let mut first = true;
        while let Some(h) = rows.try_next().await? {
            if !std::mem::take(&mut first) {
                out.put(b",")?;
            }
            let v = serde_json::json!({
                "playedAt": time(h.played_at),
                "playedMs": h.played_ms,
                "endReason": h.end_reason,
                "track": h.track.json(public_url),
            });
            out.put(&serde_json::to_vec(&v).unwrap_or_default())?;
            out.pump().await?;
        }
        out.put(b"]")?;
    }
    out.put(b"}\n")?;
    Ok(())
}

async fn saved_albums(db: &PgPool, user: Uuid) -> Result<Vec<AlbumRow>, sqlx::Error> {
    sqlx::query_as!(
        AlbumRow,
        r#"SELECT al.id, al.title, a.name AS artist, al.upc, al.release_date, s.created_at AS added_at
           FROM saved_albums s JOIN albums al ON al.id = s.album_id JOIN artists a ON a.id = al.artist_id
           WHERE s.user_id = $1 ORDER BY s.created_at DESC, al.id"#,
        user
    )
    .fetch_all(db)
    .await
}

async fn followed_artists(db: &PgPool, user: Uuid) -> Result<Vec<ArtistRow>, sqlx::Error> {
    sqlx::query_as!(
        ArtistRow,
        r#"SELECT a.id, a.name, f.created_at AS added_at
           FROM followed_artists f JOIN artists a ON a.id = f.artist_id
           WHERE f.user_id = $1 ORDER BY f.created_at DESC, a.id"#,
        user
    )
    .fetch_all(db)
    .await
}

/// История прослушиваний, новые первыми; скрытое человеком из истории не выгружается.
fn history_rows(
    db: &PgPool,
    user: Uuid,
) -> futures_util::stream::BoxStream<'_, Result<HistoryRow, sqlx::Error>> {
    Box::pin(
        sqlx::query!(
            r#"SELECT e.started_at, e.played_ms, e.end_reason,
                      t.id, t.title, t.title_version, al.title AS "album?", t.isrc, t.duration_ms, t.deezer_id,
                      ARRAY(SELECT a.name FROM track_artists ta JOIN artists a ON a.id = ta.artist_id
                            WHERE ta.track_id = t.id ORDER BY ta.position, ta.artist_id) AS "artists!"
               FROM play_events e JOIN tracks t ON t.id = e.track_id
               LEFT JOIN albums al ON al.id = t.album_id
               WHERE e.user_id = $1 AND e.hidden_at IS NULL
               ORDER BY e.started_at DESC, e.id"#,
            user
        )
        .fetch(db)
        .map_ok(|r| HistoryRow {
            played_at: r.started_at,
            played_ms: r.played_ms,
            end_reason: r.end_reason,
            track: TrackRow {
                id: r.id,
                title: r.title,
                version: r.title_version,
                artists: r.artists,
                album: r.album,
                isrc: r.isrc,
                duration_ms: r.duration_ms,
                deezer_id: r.deezer_id,
                added_at: None,
            },
        }),
    )
}

/// Вся медиатека: ZIP (CSV и M3U8 каждого списка, таблицы альбомов и артистов, полный JSON) или
/// один JSON.
pub async fn library(
    mut out: Out,
    db: PgPool,
    public_url: String,
    user: Uuid,
    format: ExportFormat,
    history: bool,
) -> Result<(), BuildError> {
    if format != ExportFormat::Zip {
        library_json(&mut out, &db, &public_url, user, history).await?;
        return out.finish().await;
    }
    let mut names = Names::default();
    let liked = "Любимые треки";
    out.file(&format!("{liked}.csv"))?;
    list(
        &mut out,
        &db,
        &public_url,
        List::Liked(user),
        liked,
        ExportFormat::Csv,
    )
    .await?;
    out.file(&format!("{liked}.m3u8"))?;
    list(
        &mut out,
        &db,
        &public_url,
        List::Liked(user),
        liked,
        ExportFormat::M3u8,
    )
    .await?;

    out.file("Альбомы.csv")?;
    out.put(BOM)?;
    out.put(csv_line(&ALBUMS_HEADER).as_bytes())?;
    for a in saved_albums(&db, user).await? {
        out.put(
            csv_line(&[
                a.artist,
                a.title,
                a.upc.unwrap_or_default(),
                a.release_date.map(|d| d.to_string()).unwrap_or_default(),
                time(a.added_at),
            ])
            .as_bytes(),
        )?;
    }
    out.pump().await?;

    out.file("Артисты.csv")?;
    out.put(BOM)?;
    out.put(csv_line(&ARTISTS_HEADER).as_bytes())?;
    for a in followed_artists(&db, user).await? {
        out.put(csv_line(&[a.name, time(a.added_at)]).as_bytes())?;
    }
    out.pump().await?;

    for p in playlists(&db, user).await? {
        let dir = match &p.folder {
            Some(folder) => format!("Плейлисты/{}", file_name(folder, "Папка")),
            None => "Плейлисты".to_string(),
        };
        let base = names.unique(&dir, &file_name(&p.title, "Плейлист"));
        for format in [ExportFormat::Csv, ExportFormat::M3u8] {
            out.file(&format!("{dir}/{base}.{}", format.extension()))?;
            list(
                &mut out,
                &db,
                &public_url,
                List::Playlist(p.id),
                &p.title,
                format,
            )
            .await?;
        }
    }

    if history {
        out.file("История.csv")?;
        out.put(BOM)?;
        out.put(csv_line(&HISTORY_HEADER).as_bytes())?;
        let mut rows = history_rows(&db, user);
        while let Some(h) = rows.try_next().await? {
            out.put(
                csv_line(&[
                    time(h.played_at),
                    h.track.artists.join(", "),
                    h.track.full_title(),
                    h.track.album.clone().unwrap_or_default(),
                    h.track.isrc.clone().unwrap_or_default(),
                    h.played_ms.to_string(),
                    h.end_reason,
                ])
                .as_bytes(),
            )?;
            out.pump().await?;
        }
    }

    out.file("Медиатека.json")?;
    library_json(&mut out, &db, &public_url, user, history).await?;
    out.finish().await
}

/// Один список целиком — для `respond`.
pub async fn single(
    mut out: Out,
    db: PgPool,
    public_url: String,
    which: List,
    title: String,
    format: ExportFormat,
) -> Result<(), BuildError> {
    list(&mut out, &db, &public_url, which, &title, format).await?;
    out.finish().await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn csv_cells_are_escaped() {
        assert_eq!(csv_cell("Lowborn"), "Lowborn");
        assert_eq!(csv_cell("Кино"), "Кино");
        assert_eq!(csv_cell("A, B"), "\"A, B\"");
        assert_eq!(csv_cell("Say \"hi\""), "\"Say \"\"hi\"\"\"");
        assert_eq!(csv_cell("two\nlines"), "\"two\nlines\"");
        assert_eq!(csv_cell(" pad"), "\" pad\"");
        assert_eq!(csv_cell(""), "");
        assert_eq!(csv_line(&["a", "b,c", "д"]), "a,\"b,c\",д\r\n".to_string());
    }

    #[test]
    fn file_names_are_safe() {
        assert_eq!(file_name("Ночь / день: 2026?", "x"), "Ночь _ день_ 2026_");
        assert_eq!(file_name("  ..hidden.  ", "x"), "hidden");
        assert_eq!(file_name("a\nb", "x"), "a b");
        assert_eq!(file_name("", "Плейлист"), "Плейлист");
        assert_eq!(file_name(&"я".repeat(200), "x").chars().count(), 80);
        let mut names = Names::default();
        assert_eq!(names.unique("d", "Ночь"), "Ночь");
        assert_eq!(names.unique("d", "ночь"), "ночь (2)");
        assert_eq!(names.unique("e", "Ночь"), "Ночь");
    }

    #[test]
    fn disposition_carries_utf8_name() {
        let v = disposition("Любимые треки.csv", "liked.csv");
        assert_eq!(
            v.to_str().unwrap(),
            "attachment; filename=\"liked.csv\"; filename*=UTF-8''%D0%9B%D1%8E%D0%B1%D0%B8%D0%BC%D1%8B%D0%B5%20%D1%82%D1%80%D0%B5%D0%BA%D0%B8.csv"
        );
        let v = disposition("Night Drive.m3u8", "playlist.m3u8");
        assert!(
            v.to_str()
                .unwrap()
                .contains("filename=\"Night Drive.m3u8\"")
        );
    }

    #[test]
    fn track_lines() {
        let row = TrackRow {
            id: Uuid::nil(),
            title: "Dachi".into(),
            version: "(Slowed)".into(),
            artists: vec!["Ann".into(), "Bob, Jr.".into()],
            album: Some("Рай".into()),
            isrc: Some("RUA1D2300001".into()),
            duration_ms: 185_400,
            deezer_id: Some(1),
            added_at: Some("2026-11-13T10:00:00Z".parse().unwrap()),
        };
        assert_eq!(
            row.csv(),
            "\"Ann, Bob, Jr.\",Dachi (Slowed),Рай,RUA1D2300001,185400,2026-11-13T10:00:00Z\r\n"
        );
        assert_eq!(
            row.m3u("https://wave.daly.rest"),
            "#EXTINF:185,Ann, Bob, Jr. - Dachi (Slowed)\nhttps://wave.daly.rest/track/00000000-0000-0000-0000-000000000000\n"
        );
    }
}
