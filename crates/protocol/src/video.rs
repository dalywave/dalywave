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

use crate::{catalog::Track, fetcher::UploadTarget};

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
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
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
    /// Клип целиком в такт с треком: плеер ведёт видео по позиции трека, а не по кругу.
    #[serde(default)]
    #[schema(required = true)]
    pub sync: Option<ClipSync>,
}

/// Подготовить видео ленты: клипы и живые обложки этих треков ставятся в очередь, готовые —
/// со ссылками.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClipsRequest {
    /// До 20 треков.
    pub track_ids: Vec<Uuid>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
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
    /// Лейбл альбома (Deezer): его канал тоже официальный.
    #[schema(required = true)]
    pub label: Option<String>,
    /// Что сделать: найти и нарезать, нарезать выбранный, только кандидаты, перепроверить.
    pub mode: ClipMode,
    /// `pinned`: ролик, который выбрал администратор.
    #[schema(required = true)]
    pub video_id: Option<String>,
    /// `review`: ролик, из которого нарезан нынешний клип.
    #[schema(required = true)]
    pub current_video_id: Option<String>,
    pub upload: UploadTarget,
    /// Не пришёл отчёт к этому сроку — задание вернётся в очередь.
    pub lease_expires_at: DateTime<Utc>,
    /// Высота кадра (360–1080). Старый загрузчик её не знает и режет 480p.
    #[serde(default = "default_clip_height")]
    pub height: i32,
    /// Клип целиком, а не кусок `length_ms` с припевом. Только загрузчику с `clip-whole`.
    #[serde(default)]
    pub whole: bool,
    /// Целиком и в такт: подписанная ссылка на звук трека, с которым загрузчик сопоставит звук
    /// ролика. Не сошлось — режет кусок с припевом.
    #[serde(default)]
    pub sync_audio_url: Option<String>,
}

fn default_clip_height() -> i32 {
    480
}

/// Клип целиком в такт с треком: позиция трека = `offset_ms` + позиция видео × `rate`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ClipSync {
    pub offset_ms: i32,
    pub rate: f64,
}

/// Что сделать с клипом.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ClipMode {
    /// Найти лучший официальный клип и нарезать.
    #[default]
    Auto,
    /// Нарезать ролик, который выбрал администратор, без отбора.
    Pinned,
    /// Только найти кандидатов (для «Клип» у администратора), без нарезки.
    Search,
    /// Перепроверить нарезанный старым отбором клип: тот же лучший — `kept`, есть с канала выше
    /// рангом — нарезать его, текущий не годится и замены нет — `none`.
    Review,
}

names!(ClipMode { Auto => "auto", Pinned => "pinned", Search => "search", Review => "review" });

/// Запрос задания клипа: ответ сразу — задание или 204.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ClipClaimRequest {
    pub worker_id: String,
    /// Что умеет загрузчик сверх «найти и нарезать»: `clip-modes` — режимы `pinned`, `search`,
    /// `review`. Без него задания с ними не выдаются.
    #[serde(default)]
    pub features: Vec<String>,
}

/// Загрузчик умеет режимы клипа сверх `auto`.
pub const CLIP_MODES_FEATURE: &str = "clip-modes";
/// Загрузчик режет клип целиком и сопоставляет его звук с треком.
pub const CLIP_WHOLE_FEATURE: &str = "clip-whole";

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
    /// `review`: нынешний клип и есть лучший — оставить как есть.
    Kept,
    /// Сбой (сеть, YouTube): задание повторится позже.
    Error,
}

names!(ClipOutcome { Ready => "ready", None => "none", Kept => "kept", Error => "error" });

/// Кандидат в клипы из выдачи YouTube с оценкой загрузчика.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ClipCandidate {
    pub video_id: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub channel: String,
    #[serde(default)]
    pub duration_s: f64,
    #[serde(default)]
    pub views: i64,
    /// Чей канал: `artist` (исполнителя), `official` (проверенный, с пометкой клипа), `vevo`,
    /// `label` (лейбла альбома); нет — чужой.
    #[serde(default)]
    #[schema(required = true)]
    pub source: Option<String>,
    #[serde(default)]
    pub score: f64,
    /// Почему не годится; нет — годится.
    #[serde(default)]
    #[schema(required = true)]
    pub rejected: Option<Vec<String>>,
}

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
    /// Название ролика.
    #[serde(default)]
    pub video_title: Option<String>,
    /// Канал ролика.
    #[serde(default)]
    pub channel: Option<String>,
    /// Чей канал (см. [`ClipCandidate::source`]).
    #[serde(default)]
    pub source: Option<String>,
    /// Оценка отбора.
    #[serde(default)]
    pub score: Option<f64>,
    /// До 8 кандидатов поиска, годные первыми.
    #[serde(default)]
    pub candidates: Vec<ClipCandidate>,
    /// Код и текст сбоя для журнала задачи.
    #[serde(default)]
    pub error: Option<String>,
    /// Что ещё загрузчик хочет сохранить в журнале (кандидаты, тайминги).
    #[serde(default)]
    #[schema(value_type = Object)]
    pub details: Value,
    /// Нарезан клип целиком.
    #[serde(default)]
    pub whole: bool,
    /// Целиком и сопоставлен со звуком трека.
    #[serde(default)]
    pub sync: Option<ClipSync>,
}

// ───────────── «Клип» у трека: выбор администратора, «Скрыть клип» ─────────────

/// Клип трека для листа «Клип».
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ClipView {
    pub track_id: Uuid,
    /// `ready` — есть (ссылка в `url`), `pending` — ищется или режется, `none` — нет.
    pub state: VideoState,
    #[schema(required = true)]
    pub url: Option<String>,
    #[schema(required = true)]
    pub video_id: Option<String>,
    #[schema(required = true)]
    pub video_title: Option<String>,
    #[schema(required = true)]
    pub channel: Option<String>,
    /// Чей канал (см. [`ClipCandidate::source`]); `pinned` — выбрал администратор.
    #[schema(required = true)]
    pub source: Option<String>,
    /// Выбор администратора: закреплён ролик или «без клипа». Автопоиск его не меняет.
    pub pinned: bool,
    /// Этот человек скрыл нынешний клип у себя.
    pub hidden: bool,
    /// Может выбирать клип (администратор). Остальные — только «Скрыть клип».
    pub can_choose: bool,
    /// Администратору: ищутся кандидаты.
    pub searching: bool,
    /// Администратору: кандидаты последнего поиска (до 8), годные первыми.
    pub candidates: Vec<ClipCandidate>,
    /// Администратору: сколько человек скрыли нынешний клип.
    pub reports: i64,
}

/// Выбор администратора.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
pub enum ClipChoice {
    /// Закрепить ролик из кандидатов.
    #[serde(rename_all = "camelCase")]
    Video { video_id: String },
    /// Закрепить ролик по ссылке YouTube.
    Url { url: String },
    /// Без клипа: показывать обложку и больше не искать.
    None,
    /// Снять выбор и найти лучший заново.
    Auto,
    /// Оставить нынешний клип (жалобы разобраны).
    Keep,
    /// Обновить кандидатов.
    Search,
}

/// Трек, у которого скрыли клип: для «Спорных».
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ClipReportItem {
    pub track: Track,
    pub video_id: String,
    #[schema(required = true)]
    pub video_title: Option<String>,
    #[schema(required = true)]
    pub channel: Option<String>,
    /// Сколько человек скрыли.
    pub reports: i64,
    pub last_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ClipReportList {
    pub items: Vec<ClipReportItem>,
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
