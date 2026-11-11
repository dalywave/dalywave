//! Очередь задач: задачи, полосы, обзор админки и данные задач (payload) по видам.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::catalog::{AudioStatus, Track};

/// Виды задач. Строки хранятся в `jobs.kind`.
pub mod kind {
    /// Найти и скачать аудио записи (этап 1b).
    pub const ACQUIRE: &str = "acquire";
    /// Скопировать обложку или фото артиста в хранилище.
    pub const MIRROR_IMAGE: &str = "mirror_image";
    /// Обновить карточку, популярное, похожих и список релизов артиста.
    pub const REFRESH_ARTIST: &str = "refresh_artist";
    /// Сверить релизы артиста из подписок и поставить новые треки на скачивание.
    pub const SYNC_DISCOGRAPHY: &str = "sync_discography";
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum JobStatus {
    Queued,
    Running,
    Done,
    Failed,
    Cancelled,
}

names!(JobStatus {
    Queued => "queued",
    Running => "running",
    Done => "done",
    Failed => "failed",
    Cancelled => "cancelled",
});

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Job {
    pub id: Uuid,
    pub kind: String,
    pub lane: String,
    /// Больше — раньше.
    pub priority: i16,
    pub status: JobStatus,
    pub attempts: i16,
    pub max_attempts: i16,
    /// Не раньше этого времени (повтор с задержкой).
    pub run_after: DateTime<Utc>,
    /// Исполнитель: `<host>:<pid>` воркера или `device:<id>` домашнего загрузчика.
    #[schema(required = true)]
    pub locked_by: Option<String>,
    #[schema(required = true)]
    pub heartbeat_at: Option<DateTime<Utc>>,
    /// Аренда у внешнего исполнителя (загрузчика): не отчитался к сроку — задача снова в очереди.
    #[schema(required = true)]
    pub lease_expires_at: Option<DateTime<Utc>>,
    #[schema(required = true)]
    pub last_error: Option<String>,
    #[schema(value_type = Object)]
    pub payload: Value,
    #[schema(value_type = Option<Object>, required = true)]
    pub result: Option<Value>,
    #[schema(required = true)]
    pub dedupe_key: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    #[schema(required = true)]
    pub finished_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct JobPage {
    pub items: Vec<Job>,
    #[schema(required = true)]
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, IntoParams)]
#[serde(rename_all = "camelCase")]
#[into_params(parameter_in = Query)]
pub struct JobsQuery {
    pub lane: Option<String>,
    pub status: Option<JobStatus>,
    pub kind: Option<String>,
    /// `nextCursor` из предыдущей страницы.
    pub cursor: Option<String>,
    /// 1–200, по умолчанию 50.
    pub limit: Option<u32>,
}

/// Полоса очереди со счётчиками.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Lane {
    pub lane: String,
    pub title: String,
    pub paused: bool,
    pub concurrency: i16,
    /// Пауза между стартами задач полосы, мс.
    pub min_interval_ms: i32,
    pub queued: i64,
    pub running: i64,
    pub failed: i64,
    /// Выполнено за сутки.
    pub done_24h: i64,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct LaneList {
    pub items: Vec<Lane>,
}

/// Изменение полосы: только переданные поля.
#[derive(Debug, Clone, Default, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LanePatch {
    #[serde(default)]
    pub paused: Option<bool>,
    /// 1–16.
    #[serde(default)]
    pub concurrency: Option<i16>,
    /// 0–3 600 000.
    #[serde(default)]
    pub min_interval_ms: Option<i32>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct LibraryStats {
    pub artists: i64,
    pub albums: i64,
    pub tracks: i64,
    pub recordings: i64,
    pub liked_tracks: i64,
    pub saved_albums: i64,
    pub followed_artists: i64,
    pub playlists: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct StatusCount {
    pub status: AudioStatus,
    pub count: i64,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct StorageStats {
    pub audio_files: i64,
    pub audio_bytes: i64,
    /// Скопированные обложки и фото (каждая — три размера).
    pub images: i64,
    pub image_bytes: i64,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DayStats {
    pub jobs_done: i64,
    pub jobs_failed: i64,
    pub acquisitions_ready: i64,
    pub acquisitions_failed: i64,
    pub plays: i64,
    pub likes: i64,
    pub new_tracks: i64,
}

/// Раздел «Обзор» админки.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AdminOverview {
    pub library: LibraryStats,
    /// Записи по состоянию аудио; `none` тоже здесь.
    pub recordings_by_status: Vec<StatusCount>,
    pub storage: StorageStats,
    pub lanes: Vec<Lane>,
    pub last_24h: DayStats,
}

// ───────────── Данные задач ─────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ImageOwner {
    Album,
    Artist,
}

names!(ImageOwner { Album => "album", Artist => "artist" });

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct MirrorImagePayload {
    pub owner: ImageOwner,
    pub id: Uuid,
    /// Скопировать заново, даже если копия есть.
    #[serde(default)]
    pub force: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct RefreshArtistPayload {
    pub artist_id: Uuid,
    /// Обновить всё, даже свежее.
    #[serde(default)]
    pub force: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SyncDiscographyPayload {
    pub artist_id: Uuid,
}

/// Зачем скачивается запись. Хранится в `acquisitions.reason`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum AcquireReason {
    Play,
    Wave,
    Like,
    Playlist,
    Album,
    Subscription,
    Discovery,
    Import,
    Manual,
    Upgrade,
}

names!(AcquireReason {
    Play => "play",
    Wave => "wave",
    Like => "like",
    Playlist => "playlist",
    Album => "album",
    Subscription => "subscription",
    Discovery => "discovery",
    Import => "import",
    Manual => "manual",
    Upgrade => "upgrade",
});

/// Шаг поиска и скачивания одной записи. Хранится в `acquisitions.status`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum AcquisitionStatus {
    Queued,
    Matching,
    Downloading,
    Verifying,
    Processing,
    Ready,
    Disputed,
    Unavailable,
    Failed,
    Cancelled,
}

names!(AcquisitionStatus {
    Queued => "queued",
    Matching => "matching",
    Downloading => "downloading",
    Verifying => "verifying",
    Processing => "processing",
    Ready => "ready",
    Disputed => "disputed",
    Unavailable => "unavailable",
    Failed => "failed",
    Cancelled => "cancelled",
});

impl AcquisitionStatus {
    /// Скачивание ещё идёт: у записи может быть только одно такое.
    pub fn is_active(self) -> bool {
        matches!(
            self,
            Self::Queued | Self::Matching | Self::Downloading | Self::Verifying | Self::Processing
        )
    }
}

/// Данные задачи `acquire`: что скачать. Тот же набор полей сервер отдаёт домашнему загрузчику
/// (`POST /api/fetcher/jobs/claim`) вместе с подписанной ссылкой для файла.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AcquirePayload {
    pub acquisition_id: Uuid,
    pub recording_id: Uuid,
    pub track_id: Uuid,
    /// fetcher ищет запись по треку Deezer.
    pub deezer_track_id: i64,
    /// Видео, выбранные вручную в «Спорных»: пробуются первыми.
    #[serde(default)]
    pub prefer_video_ids: Vec<String>,
    /// Отклонённые видео.
    #[serde(default)]
    pub exclude_video_ids: Vec<String>,
    /// Ссылка, вставленная в «Спорных» (SoundCloud, Bandcamp, …): качается она, без поиска.
    #[serde(default)]
    pub direct_url: Option<String>,
}

impl AcquirePayload {
    /// Запись выбрана человеком: файл принимается и без совпадения отпечатка.
    pub fn is_manual(&self) -> bool {
        !self.prefer_video_ids.is_empty() || self.direct_url.is_some()
    }
}

/// Скачивание записи: шаг, место в очереди, есть ли кому качать.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Acquisition {
    pub id: Uuid,
    pub recording_id: Uuid,
    pub track_id: Uuid,
    pub status: AcquisitionStatus,
    pub audio_status: AudioStatus,
    pub reason: AcquireReason,
    pub lane: String,
    #[schema(required = true)]
    pub error: Option<String>,
    /// Сколько раз задачу брали в работу.
    pub attempts: i16,
    /// Сколько скачиваний впереди; 0 — следующее или уже идёт; `null`, если скачивание закончилось.
    #[schema(required = true)]
    pub queue_position: Option<i64>,
    /// Загрузчики на связи (обращались в последние две минуты).
    pub downloaders_online: i64,
    /// В очереди, но качать некому: загрузчиков на связи нет, серверный fetcher выключен.
    pub waiting_for_downloader: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    #[schema(required = true)]
    pub finished_at: Option<DateTime<Utc>>,
}

/// Чем кончилась попытка скачать кандидата.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CandidateAttempt {
    pub downloaded: bool,
    /// Совпал ли отпечаток с превью Deezer; `null` — проверить было нечем.
    #[schema(required = true)]
    pub verified: Option<bool>,
    /// 0..1, порог — 0,8.
    #[schema(required = true)]
    pub similarity: Option<f64>,
    /// Где в файле начинается превью Deezer.
    #[schema(required = true)]
    pub offset_ms: Option<i64>,
    #[schema(required = true)]
    pub error_code: Option<String>,
    #[schema(required = true)]
    pub error: Option<String>,
}

/// Кандидат из поиска fetcher: оценка с разбором и что вышло при попытке скачать.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AcquisitionCandidate {
    /// id видео YouTube или ссылка, вставленная вручную.
    pub video_id: String,
    #[schema(required = true)]
    pub url: Option<String>,
    pub title: String,
    pub artists: Vec<String>,
    #[schema(required = true)]
    pub album: Option<String>,
    #[schema(required = true)]
    pub duration_ms: Option<i64>,
    /// `atv` — официальное аудио, `omv` — клип артиста, `ugc` и `other` — прочее.
    #[schema(required = true)]
    pub kind: Option<String>,
    /// Как найден: `album`, `isrc`, `song`, `video`, `manual`.
    #[schema(required = true)]
    pub source: Option<String>,
    /// 0..1.
    #[schema(required = true)]
    pub score: Option<f64>,
    /// Почему отброшен (длительность, название).
    #[schema(required = true)]
    pub rejected: Option<String>,
    /// Разбор оценки по составляющим: длительность, название, артисты, альбом, источник, множители.
    #[schema(value_type = Object)]
    pub breakdown: Value,
    /// Есть, если кандидата пробовали скачать.
    #[schema(required = true)]
    pub attempt: Option<CandidateAttempt>,
}

/// Решение сервера по одной попытке.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum AttemptOutcome {
    /// Файл принят.
    Ready,
    /// Кандидаты не прошли проверку — «Спорные».
    Disputed,
    /// Записи нет в источниках.
    Unavailable,
    /// Сбой, задача повторится позже.
    Retry,
    /// Сбой, попытки кончились.
    Failed,
    /// Сбой исполнителя, а не записи (YouTube заподозрил бота): задача отложена, попытка не засчитана.
    Postponed,
}

names!(AttemptOutcome {
    Ready => "ready",
    Disputed => "disputed",
    Unavailable => "unavailable",
    Retry => "retry",
    Failed => "failed",
    Postponed => "postponed",
});

/// Попытка скачать запись: кто качал, что ответил fetcher и что решил сервер.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AcquisitionAttempt {
    pub id: Uuid,
    /// `server` или `device:<id>` (домашний загрузчик).
    pub executor: String,
    /// Статус ответа fetcher: `verified`, `unverified`, `disputed`, `not_found`, `error`.
    pub status: String,
    pub outcome: AttemptOutcome,
    #[schema(required = true)]
    pub error_code: Option<String>,
    #[schema(required = true)]
    pub error: Option<String>,
    #[schema(required = true)]
    pub video_id: Option<String>,
    #[schema(required = true)]
    pub yt_dlp_version: Option<String>,
    #[schema(required = true)]
    pub total_ms: Option<i32>,
    pub created_at: DateTime<Utc>,
}

/// Скачивание в разделах «Загрузки» и «Спорные».
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AdminAcquisition {
    pub acquisition: Acquisition,
    /// Ожидаемый трек; `null`, если его уже нет в каталоге.
    #[schema(required = true)]
    pub track: Option<Track>,
    #[schema(required = true)]
    pub requested_by: Option<Uuid>,
    /// Исполнитель последней попытки: `server` или `device:<id>`.
    #[schema(required = true)]
    pub worker: Option<String>,
    #[schema(required = true)]
    pub job_id: Option<Uuid>,
    #[schema(required = true)]
    pub job_status: Option<JobStatus>,
    /// Видео, выбранные вручную, и отклонённые — из задачи.
    pub prefer_video_ids: Vec<String>,
    #[schema(required = true)]
    pub direct_url: Option<String>,
    /// Кандидаты по убыванию оценки, с разбором и попытками.
    pub candidates: Vec<AcquisitionCandidate>,
    /// Выбранный кандидат, скачивание и проверка — как прислал fetcher.
    #[schema(value_type = Option<Object>, required = true)]
    pub chosen: Option<Value>,
    /// Попытки, новые первыми.
    pub attempts_log: Vec<AcquisitionAttempt>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AdminAcquisitionPage {
    pub items: Vec<AdminAcquisition>,
    #[schema(required = true)]
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, IntoParams)]
#[serde(rename_all = "camelCase")]
#[into_params(parameter_in = Query)]
pub struct AcquisitionsQuery {
    /// Например `disputed` — раздел «Спорные».
    pub status: Option<AcquisitionStatus>,
    /// `nextCursor` из предыдущей страницы.
    pub cursor: Option<String>,
    /// 1–200, по умолчанию 50.
    pub limit: Option<u32>,
}

/// Что сделать со спорной (или неудачной) записью.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ResolveAction {
    /// Скачать выбранное видео (`videoId`) и принять его.
    Choose,
    /// Скачать по ссылке (`url`): YouTube, YouTube Music, SoundCloud, Bandcamp…
    Url,
    /// Записи нет в источниках.
    Unavailable,
    /// Поискать и скачать заново.
    Retry,
}

names!(ResolveAction {
    Choose => "choose",
    Url => "url",
    Unavailable => "unavailable",
    Retry => "retry",
});

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResolveRequest {
    pub action: ResolveAction,
    /// Для `choose`: id видео YouTube (11 символов).
    #[serde(default)]
    pub video_id: Option<String>,
    /// Для `url`.
    #[serde(default)]
    pub url: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn payloads_are_camel_case() {
        let p = AcquirePayload {
            acquisition_id: Uuid::nil(),
            recording_id: Uuid::nil(),
            track_id: Uuid::nil(),
            deezer_track_id: 3040222091,
            prefer_video_ids: vec![],
            exclude_video_ids: vec![],
            direct_url: None,
        };
        let json = serde_json::to_value(&p).unwrap();
        assert_eq!(json["deezerTrackId"], 3040222091i64);
        assert!(json["preferVideoIds"].is_array());
        assert!(!p.is_manual());
        // Задачи, поставленные до этапа 1b, — без новых полей
        let old: AcquirePayload = serde_json::from_value(serde_json::json!({
            "acquisitionId": Uuid::nil(), "recordingId": Uuid::nil(), "trackId": Uuid::nil(),
            "deezerTrackId": 1
        }))
        .unwrap();
        assert!(old.direct_url.is_none() && old.prefer_video_ids.is_empty());
        for o in AttemptOutcome::ALL {
            assert_eq!(serde_json::to_value(o).unwrap(), o.as_str());
        }
        let r: ResolveRequest =
            serde_json::from_value(serde_json::json!({"action": "choose", "videoId": "abc"}))
                .unwrap();
        assert_eq!(r.action, ResolveAction::Choose);
        let parsed: MirrorImagePayload =
            serde_json::from_value(serde_json::json!({"owner": "album", "id": Uuid::nil()}))
                .unwrap();
        assert!(!parsed.force);
        for s in AcquisitionStatus::ALL {
            assert_eq!(serde_json::to_value(s).unwrap(), s.as_str());
        }
        assert!(AcquisitionStatus::Matching.is_active() && !AcquisitionStatus::Ready.is_active());
    }
}
