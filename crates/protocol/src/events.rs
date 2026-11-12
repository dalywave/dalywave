//! События прослушивания и реакции. Приложения копят их без сети и отправляют пачками;
//! `clientEventId` делает повторную отправку безопасной.

use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::catalog::Track;

/// Почему прослушивание закончилось.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum EndReason {
    Completed,
    Skipped,
    /// Вернулись к предыдущему треку.
    Previous,
    Stopped,
    Error,
    /// Прервано системой: звонок, другое приложение.
    Interrupted,
    /// Воспроизведение передано на другое устройство.
    Transferred,
}

names!(EndReason {
    Completed => "completed",
    Skipped => "skipped",
    Previous => "previous",
    Stopped => "stopped",
    Error => "error",
    Interrupted => "interrupted",
    Transferred => "transferred",
});

/// Откуда запущено.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ContextType {
    Album,
    Playlist,
    Artist,
    Wave,
    Radio,
    Mix,
    Search,
    Library,
    Queue,
    Remote,
}

names!(ContextType {
    Album => "album",
    Playlist => "playlist",
    Artist => "artist",
    Wave => "wave",
    Radio => "radio",
    Mix => "mix",
    Search => "search",
    Library => "library",
    Queue => "queue",
    Remote => "remote",
});

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum PlaySource {
    /// Аудио с сервера.
    #[default]
    Stream,
    /// Скачанное на устройство.
    Offline,
}

names!(PlaySource { Stream => "stream", Offline => "offline" });

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum FeedbackKind {
    Like,
    Unlike,
    Dislike,
    Undislike,
    /// «Не сейчас» — пропуск без штрафа треку.
    NotNow,
    MoreLikeThis,
    PlaylistAdd,
    QueueAdd,
    Replay,
}

names!(FeedbackKind {
    Like => "like",
    Unlike => "unlike",
    Dislike => "dislike",
    Undislike => "undislike",
    NotNow => "not_now",
    MoreLikeThis => "more_like_this",
    PlaylistAdd => "playlist_add",
    QueueAdd => "queue_add",
    Replay => "replay",
});

/// Одно прослушивание.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PlayEvent {
    pub client_event_id: Uuid,
    pub track_id: Uuid,
    pub started_at: DateTime<Utc>,
    /// Сколько реально сыграно, без перемоток.
    pub played_ms: i32,
    /// С какой позиции началось.
    #[serde(default)]
    pub position_start_ms: i32,
    /// Длительность трека у плеера.
    pub duration_ms: i32,
    pub end_reason: EndReason,
    #[serde(default)]
    pub context_type: Option<ContextType>,
    #[serde(default)]
    pub context_id: Option<String>,
    #[serde(default)]
    pub wave_session_id: Option<Uuid>,
    #[serde(default)]
    pub source: PlaySource,
    /// Режим перехода к следующему треку: `gapless`, `crossfade`, `smart`, `dj`.
    #[serde(default)]
    pub transition: Option<String>,
    /// Выход звука: `headphones`, `speaker`, `bluetooth`, `airplay`.
    #[serde(default)]
    pub output: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FeedbackEvent {
    pub client_event_id: Uuid,
    pub kind: FeedbackKind,
    #[serde(default)]
    pub track_id: Option<Uuid>,
    #[serde(default)]
    pub artist_id: Option<Uuid>,
    #[serde(default)]
    pub context_type: Option<ContextType>,
    #[serde(default)]
    pub context_id: Option<String>,
    #[serde(default)]
    pub wave_session_id: Option<Uuid>,
    pub at: DateTime<Utc>,
}

/// Пачка событий из приложения: до 500 каждого вида.
#[derive(Debug, Clone, Default, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EventBatch {
    #[serde(default)]
    pub plays: Vec<PlayEvent>,
    #[serde(default)]
    pub feedback: Vec<FeedbackEvent>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct RejectedEvent {
    pub client_event_id: Uuid,
    /// Машинная причина: `unknown_track`, `unknown_artist`, `invalid`.
    pub reason: String,
}

/// Итог приёма. Принятые и повторы приложение может удалить у себя; отклонённые повторять бессмысленно.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct EventResult {
    pub accepted: u32,
    /// Уже были приняты раньше.
    pub duplicates: u32,
    pub rejected: Vec<RejectedEvent>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct HistoryItem {
    pub id: Uuid,
    pub started_at: DateTime<Utc>,
    pub played_ms: i32,
    pub end_reason: EndReason,
    #[schema(required = true)]
    pub context_type: Option<ContextType>,
    #[schema(required = true)]
    pub context_id: Option<String>,
    /// Название источника: альбом, плейлист, артист; у волны, радио и поиска — нет.
    #[serde(default)]
    #[schema(required = true)]
    pub context_title: Option<String>,
    /// Из истории Spotify (импорт архива).
    #[serde(default)]
    pub imported: bool,
    pub track: Track,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct HistoryPage {
    pub items: Vec<HistoryItem>,
    #[schema(required = true)]
    pub next_cursor: Option<String>,
}

/// История прослушиваний: страница, поиск и переход к дате.
#[derive(Debug, Clone, Default, Serialize, Deserialize, IntoParams)]
#[serde(rename_all = "camelCase")]
#[into_params(parameter_in = Query)]
pub struct HistoryQuery {
    /// `nextCursor` из предыдущей страницы.
    pub cursor: Option<String>,
    /// Сколько вернуть: 1–200, по умолчанию 50.
    pub limit: Option<u32>,
    /// Поиск по названию трека, артисту и альбому (без регистра и диакритики).
    pub q: Option<String>,
    /// Переход к дате: прослушивания раньше этого момента. С `cursor` не сочетается — курсор главнее.
    pub before: Option<DateTime<Utc>>,
}

/// Дни с прослушиваниями — для календаря.
#[derive(Debug, Clone, Default, Serialize, Deserialize, IntoParams)]
#[serde(rename_all = "camelCase")]
#[into_params(parameter_in = Query)]
pub struct HistoryDaysQuery {
    /// Часовой пояс слушателя: минуты к востоку от UTC (Москва — 180), от −840 до 840.
    pub offset_min: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct HistoryDay {
    pub day: NaiveDate,
    pub plays: i64,
}

/// Дни с прослушиваниями, новые первыми.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct HistoryDays {
    pub days: Vec<HistoryDay>,
}

/// «Неверная запись»: что не так с тем, что играет.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum WrongRecordingReason {
    /// Играет другая песня.
    WrongSong,
    /// Другая версия: лайв, ремикс, кавер.
    WrongVersion,
    /// Плохое качество звука.
    BadQuality,
    /// Запись обрезана.
    Cut,
}

names!(WrongRecordingReason {
    WrongSong => "wrong_song",
    WrongVersion => "wrong_version",
    BadQuality => "bad_quality",
    Cut => "cut",
});

impl WrongRecordingReason {
    /// Для админки («Спорные»).
    pub fn title_ru(self) -> &'static str {
        match self {
            Self::WrongSong => "другая песня",
            Self::WrongVersion => "другая версия или лайв",
            Self::BadQuality => "плохое качество",
            Self::Cut => "обрезано",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct WrongRecordingReport {
    pub reason: WrongRecordingReason,
    /// Необязательный комментарий, до 500 символов.
    #[serde(default)]
    pub comment: Option<String>,
}

/// Жалоба принята: спорная запись в админке и новое скачивание без этой записи.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct WrongRecordingResult {
    /// Скачивание со статусом `disputed` — в разделе «Спорные».
    pub dispute_id: Uuid,
    /// Новое скачивание; нет — у записи уже идёт скачивание или её не скачать заново.
    #[schema(required = true)]
    pub acquisition_id: Option<Uuid>,
}
