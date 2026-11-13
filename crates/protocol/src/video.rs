//! Видео без звука: живые обложки альбомов (Apple Music) и клипы ленты открытий.
//!
//! Живая обложка — зацикленное видео вместо обложки в полном плеере: квадратное (`square`) и
//! вертикальное (`tall`, для телефона). Ищет её воркер в каталоге Apple Music по ISRC трека,
//! перекладывает в короткий MP4 и кладёт в хранилище; нет обложки — запоминается и перепроверяется
//! раз в месяц.
//!
//! Клип — 10–15 секунд официального видео вокруг припева, 480p без звука. Ищет и нарезает его
//! домашний загрузчик (`/api/fetcher/clips/*`), хранится неделю (дольше — если трек в любимых),
//! общий объём ограничен: старое вытесняется.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::fetcher::UploadTarget;

/// Есть ли видео.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum VideoState {
    /// Готово: ссылка в ответе.
    Ready,
    /// Ищется или готовится: спросить снова позже.
    Pending,
    /// Нет и не будет (до перепроверки).
    None,
}

names!(VideoState { Ready => "ready", Pending => "pending", None => "none" });

/// Живая обложка альбома.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AlbumMotion {
    pub album_id: Uuid,
    pub state: VideoState,
    /// Квадратное видео (MP4, H.264, без звука), подписанная ссылка на несколько часов.
    #[schema(required = true)]
    pub square: Option<String>,
    /// Вертикальное видео для телефона, если у обложки оно есть.
    #[schema(required = true)]
    pub tall: Option<String>,
}

/// Клип трека для ленты открытий.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TrackClip {
    pub track_id: Uuid,
    pub state: VideoState,
    /// MP4 (H.264, 480p, без звука), подписанная ссылка на несколько часов.
    #[schema(required = true)]
    pub url: Option<String>,
    /// Длина отрывка, мс.
    #[schema(required = true)]
    pub duration_ms: Option<i32>,
    /// Живая обложка альбома трека — запасное видео, когда клипа нет.
    #[schema(required = true)]
    pub motion: Option<AlbumMotion>,
}

/// Подготовить видео ленты: клипы и живые обложки этих треков ставятся в очередь, готовые —
/// со ссылками.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClipsRequest {
    /// До 20 треков.
    pub track_ids: Vec<Uuid>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ClipList {
    /// В порядке запроса; неизвестные треки пропущены.
    pub items: Vec<TrackClip>,
}

// ───────────── Загрузчик: клипы ─────────────

/// Задание загрузчику: найти официальный клип трека, вырезать отрывок вокруг припева и положить
/// MP4 по подписанной ссылке.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ClipJob {
    /// id задачи очереди: с ним отчёт.
    pub id: Uuid,
    pub track_id: Uuid,
    pub title: String,
    /// Исполнители по порядку.
    pub artists: Vec<String>,
    /// Длина трека, мс: клип сверяется с ней.
    pub duration_ms: i32,
    /// Где в треке припев, мс.
    pub hook_start_ms: i32,
    /// Сколько вырезать, мс (10–15 секунд).
    pub length_ms: i32,
    #[schema(required = true)]
    pub isrc: Option<String>,
    /// Видео, которые уже пробовали и отвергли: не предлагать снова.
    pub exclude_video_ids: Vec<String>,
    pub upload: UploadTarget,
    /// Не пришёл отчёт к этому сроку — задание вернётся в очередь.
    pub lease_expires_at: DateTime<Utc>,
}

/// Запрос задания клипа: ответ сразу — задание или 204.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ClipClaimRequest {
    pub worker_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ClipClaimResponse {
    pub job: ClipJob,
}

/// Чем кончилось задание клипа.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ClipOutcome {
    /// Файл загружен.
    Ready,
    /// Официального клипа нет: помнить и не искать до перепроверки.
    None,
    /// Сбой (сеть, YouTube): задание повторится позже.
    Error,
}

names!(ClipOutcome { Ready => "ready", None => "none", Error => "error" });

/// Отчёт загрузчика о клипе.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ClipResult {
    pub status: ClipOutcome,
    /// id ролика YouTube.
    #[serde(default)]
    pub video_id: Option<String>,
    /// Длина ролика целиком, мс.
    #[serde(default)]
    pub video_duration_ms: Option<i32>,
    /// С какого места ролика вырезано, мс.
    #[serde(default)]
    pub clip_start_ms: Option<i32>,
    /// Длина отрывка, мс.
    #[serde(default)]
    pub duration_ms: Option<i32>,
    #[serde(default)]
    pub size_bytes: Option<i64>,
    #[serde(default)]
    pub width: Option<i32>,
    #[serde(default)]
    pub height: Option<i32>,
    /// Код и текст сбоя для журнала задачи.
    #[serde(default)]
    pub error: Option<String>,
    /// Что ещё загрузчик хочет сохранить в журнале (кандидаты, тайминги).
    #[serde(default)]
    #[schema(value_type = Object)]
    pub details: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ClipReport {
    pub worker_id: String,
    pub result: ClipResult,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clip_result_is_lenient() {
        let r: ClipResult = serde_json::from_str(r#"{"status": "none"}"#).unwrap();
        assert_eq!(r.status, ClipOutcome::None);
        assert!(r.video_id.is_none());
        let r: ClipResult = serde_json::from_str(
            r#"{"status": "ready", "videoId": "abc", "clipStartMs": 71000, "durationMs": 12000,
                "sizeBytes": 900000, "width": 854, "height": 480, "details": {"x": 1}}"#,
        )
        .unwrap();
        assert_eq!(r.clip_start_ms, Some(71_000));
        assert_eq!(VideoState::Pending.as_str(), "pending");
    }
}
