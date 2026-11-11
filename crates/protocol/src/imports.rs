//! Импорт музыки: архив данных Spotify (лайки, плейлисты, альбомы, подписки, история прослушиваний),
//! списки («Артист — Трек», CSV, M3U) и свои аудиофайлы. Позиции сопоставляются с каталогом Deezer
//! в фоне; несопоставленные человек проверяет сам.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::catalog::Track;

/// Что импортируется. Хранится в `imports.kind`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ImportKind {
    /// Строки «Артист — Трек».
    Text,
    /// Таблица: artist, title[, album].
    Csv,
    /// Плейлист M3U/M3U8.
    M3u,
    /// Свои аудиофайлы.
    Files,
    /// Архив данных Spotify (`my_spotify_data.zip`).
    SpotifyArchive,
}

names!(ImportKind {
    Text => "text",
    Csv => "csv",
    M3u => "m3u",
    Files => "files",
    SpotifyArchive => "spotify_archive",
});

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ImportStatus {
    Running,
    Done,
    Failed,
    Cancelled,
}

names!(ImportStatus {
    Running => "running",
    Done => "done",
    Failed => "failed",
    Cancelled => "cancelled",
});

/// Шаг импорта: файлы ещё грузятся → сопоставление с каталогом → раскладка в медиатеку → готово.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ImportPhase {
    Uploading,
    Matching,
    Applying,
    Done,
}

names!(ImportPhase {
    Uploading => "uploading",
    Matching => "matching",
    Applying => "applying",
    Done => "done",
});

/// Куда кладётся найденное из списка.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ImportTarget {
    /// В «Мне нравится».
    Likes,
    /// В новый плейлист.
    Playlist,
    /// Как в архиве: лайки — в лайки, плейлисты — в плейлисты (архив Spotify, файлы).
    Library,
}

names!(ImportTarget {
    Likes => "likes",
    Playlist => "playlist",
    Library => "library",
});

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ImportItemKind {
    Track,
    Album,
    Artist,
    File,
}

names!(ImportItemKind {
    Track => "track",
    Album => "album",
    Artist => "artist",
    File => "file",
});

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ImportItemStatus {
    /// Ещё не сопоставлена.
    Pending,
    Matched,
    /// Не нашлась или нашлась неуверенно — ждёт проверки.
    Unmatched,
    /// Человек пропустил.
    Skipped,
}

names!(ImportItemStatus {
    Pending => "pending",
    Matched => "matched",
    Unmatched => "unmatched",
    Skipped => "skipped",
});

/// Что добавилось в медиатеку.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", default)]
pub struct ImportStats {
    pub likes: i64,
    pub playlists: i64,
    pub playlist_tracks: i64,
    pub albums: i64,
    pub artists: i64,
    /// Прослушиваний из истории.
    pub plays: i64,
    /// Принятых файлов.
    pub files: i64,
    /// Файлов, заменивших копию хуже качеством.
    pub upgraded: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ImportSummary {
    pub id: Uuid,
    pub kind: ImportKind,
    pub title: String,
    pub status: ImportStatus,
    pub phase: ImportPhase,
    /// Позиций для сопоставления.
    pub total: i32,
    /// Разобрано (найдено, не найдено или пропущено).
    pub processed: i32,
    pub matched: i32,
    /// Ждут проверки.
    pub unmatched: i64,
    pub stats: ImportStats,
    #[schema(required = true)]
    pub error: Option<String>,
    /// Плейлист, куда легли треки списка.
    #[schema(required = true)]
    pub playlist_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    #[schema(required = true)]
    pub finished_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ImportList {
    pub items: Vec<ImportSummary>,
}

/// Кандидат Deezer для позиции, которая нашлась неуверенно.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ImportCandidate {
    pub deezer_id: i64,
    pub title: String,
    pub artist: String,
    #[schema(required = true)]
    pub album: Option<String>,
    #[schema(required = true)]
    pub duration_ms: Option<i32>,
    /// Оценка совпадения 0..1.
    pub score: f32,
}

/// Загруженный файл: формат и что с ним стало.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", default)]
pub struct ImportFileInfo {
    pub name: String,
    pub size_bytes: i64,
    #[schema(required = true)]
    pub codec: Option<String>,
    pub lossless: bool,
    #[schema(required = true)]
    pub bitrate_kbps: Option<i32>,
    #[schema(required = true)]
    pub sample_rate: Option<i32>,
    #[schema(required = true)]
    pub bit_depth: Option<i32>,
    /// Стал основным файлом записи (лучше прежнего).
    pub upgraded: bool,
    /// Отпечаток совпал с превью Deezer; `null` — сравнить было не с чем.
    #[schema(required = true)]
    pub verified: Option<bool>,
    /// Файл не прочитался: почему.
    #[schema(required = true)]
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ImportItem {
    pub id: Uuid,
    pub kind: ImportItemKind,
    pub position: i32,
    /// Как позиция называлась в источнике.
    pub artist: String,
    pub title: String,
    #[schema(required = true)]
    pub album: Option<String>,
    pub status: ImportItemStatus,
    #[schema(required = true)]
    pub confidence: Option<f32>,
    /// Найденный трек (у треков и файлов).
    #[schema(required = true)]
    pub track: Option<Track>,
    /// Лучшие кандидаты, если найти уверенно не вышло.
    pub candidates: Vec<ImportCandidate>,
    /// Сколько раз позиция встречается в истории прослушиваний.
    pub plays: i32,
    pub liked: bool,
    #[schema(required = true)]
    pub file: Option<ImportFileInfo>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ImportItemPage {
    pub items: Vec<ImportItem>,
    #[schema(required = true)]
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize, IntoParams)]
#[serde(rename_all = "camelCase")]
#[into_params(parameter_in = Query)]
pub struct ImportItemsQuery {
    /// Только позиции в этом состоянии; без него — все.
    pub status: Option<ImportItemStatus>,
    pub cursor: Option<String>,
    /// 1–200, по умолчанию 50.
    pub limit: Option<u32>,
}

/// Исправить позицию: выбрать трек (наш id из поиска или кандидат Deezer) или пропустить.
#[derive(Debug, Clone, Default, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ImportItemFix {
    #[serde(default)]
    pub track_id: Option<Uuid>,
    #[serde(default)]
    pub deezer_id: Option<i64>,
    #[serde(default)]
    pub album_id: Option<Uuid>,
    #[serde(default)]
    pub artist_id: Option<Uuid>,
    /// Не искать больше: позиция пропускается.
    #[serde(default)]
    pub skip: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ListFormat {
    Text,
    Csv,
    M3u,
}

/// Импорт списка. Формат без указания определяется по тексту.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ListImportRequest {
    pub text: String,
    #[serde(default)]
    pub format: Option<ListFormat>,
    /// `likes` или `playlist` (по умолчанию — плейлист).
    #[serde(default)]
    pub target: Option<ImportTarget>,
    /// Название нового плейлиста; по умолчанию — имя файла или «Импорт от …».
    #[serde(default)]
    pub playlist_title: Option<String>,
}

/// Новый импорт файлов: дальше файлы грузятся по одному, затем `finish`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FilesImportRequest {
    #[serde(default)]
    pub title: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize, IntoParams)]
#[serde(rename_all = "camelCase")]
#[into_params(parameter_in = Query)]
pub struct UploadQuery {
    /// Имя файла, как у человека на диске.
    pub name: Option<String>,
}

/// Задачи импорта (полоса «Импорт»).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ImportJobPayload {
    pub import_id: Uuid,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ImportFilePayload {
    pub item_id: Uuid,
    /// Трек выбрал человек: файл кладётся к нему без поиска.
    #[serde(default)]
    pub track_id: Option<Uuid>,
}
