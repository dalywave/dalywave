//! «Слушать вместе», как Jam у Spotify. Хозяин открывает сессию и показывает QR-код со ссылкой
//! `/jam/{code}`; гости заходят без аккаунта — только имя — и получают токен `dwj_…`, который
//! действует лишь в этой сессии и лишь на `/api/jam/guest/*`. Гости видят, что играет, и общую
//! очередь, ищут по каталогу и добавляют треки (сразу или после одобрения хозяина), ставят ❤ и
//! голосуют за пропуск. Звук играет на активном устройстве хозяина; гость может включить «Слушать
//! у себя» — тот же трек с той же позиции на своём телефоне.
//!
//! Живое — по WebSocket: хозяин получает `jam.*` в обычном `/api/ws`, гость — в
//! `/api/jam/guest/ws?token=dwj_…` (там же `jam.now` — что играет, без остального состояния хозяина).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::catalog::ImageSet;

/// Самое длинное имя гостя.
pub const MAX_GUEST_NAME: usize = 32;
/// Сессия без дел столько часов — закрыта.
pub const IDLE_HOURS: i64 = 6;

/// Трек для гостя: только то, что видно на экране (без медиатеки хозяина).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct JamTrack {
    pub id: Uuid,
    pub title: String,
    /// «Slowed», «Remastered 2011»; пустая строка, если версии нет.
    pub version: String,
    pub artists: Vec<String>,
    #[schema(required = true)]
    pub album: Option<String>,
    #[schema(required = true)]
    pub image: Option<ImageSet>,
    pub duration_ms: i32,
    pub explicit: bool,
}

/// Участник сессии.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct JamGuest {
    pub id: Uuid,
    pub name: String,
    /// Вошёл со своим аккаунтом DalyWave.
    pub member: bool,
    /// Страница гостя открыта (был на связи последние 2 минуты).
    pub online: bool,
    pub joined_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum JamRequestStatus {
    /// Ждёт одобрения хозяина.
    Pending,
    /// В очереди хозяина.
    Added,
    Declined,
}

names!(JamRequestStatus {
    Pending => "pending",
    Added => "added",
    Declined => "declined",
});

/// Трек от гостя.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct JamRequest {
    pub id: Uuid,
    pub track: JamTrack,
    pub guest_id: Uuid,
    pub guest_name: String,
    pub status: JamRequestStatus,
    pub created_at: DateTime<Utc>,
}

/// Настройки сессии.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct JamSettings {
    /// «Гости могут добавлять».
    pub guests_can_add: bool,
    /// Добавленное гостями ждёт одобрения хозяина.
    pub approval: bool,
    /// Гости могут слушать у себя.
    pub guests_can_listen: bool,
}

/// Сессия глазами хозяина.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Jam {
    pub id: Uuid,
    pub code: String,
    /// Ссылка для гостей (и QR-кода).
    pub url: String,
    pub settings: JamSettings,
    pub guests: Vec<JamGuest>,
    /// Ждущие одобрения и добавленные (для подписи «добавил Миша» в очереди), новые первыми.
    pub requests: Vec<JamRequest>,
    pub created_at: DateTime<Utc>,
    /// Закроется сама, если до этого времени ничего не произойдёт.
    pub expires_at: DateTime<Utc>,
}

/// `GET /api/jam`: открытая сессия хозяина или `null`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct JamOverview {
    #[schema(required = true)]
    pub jam: Option<Jam>,
}

/// `PATCH /api/jam`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct JamPatch {
    #[serde(default)]
    pub guests_can_add: Option<bool>,
    #[serde(default)]
    pub approval: Option<bool>,
    #[serde(default)]
    pub guests_can_listen: Option<bool>,
}

/// `GET /api/jam/join/{code}`: что показать на странице входа гостя.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct JamInvite {
    pub host_name: String,
    #[schema(required = true)]
    pub host_avatar: Option<String>,
    pub guests: u32,
    /// Что играет сейчас.
    #[schema(required = true)]
    pub current: Option<JamTrack>,
    /// Ссылку открыл человек со своим аккаунтом: имя возьмётся оттуда.
    #[schema(required = true)]
    pub member_name: Option<String>,
}

/// `POST /api/jam/join/{code}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct JamJoin {
    /// Имя гостя, 1–32 символа. Со своим аккаунтом можно не указывать.
    #[serde(default)]
    pub name: Option<String>,
}

/// Гость вошёл. Токен показывается один раз: на сервере только его хеш.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct JamJoined {
    /// `dwj_…` для `Authorization: Bearer` на `/api/jam/guest/*`.
    pub token: String,
    pub guest: JamGuest,
}

/// Элемент очереди для гостя.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct JamQueueItem {
    pub track: JamTrack,
    /// Имя гостя, который добавил трек.
    #[schema(required = true)]
    pub added_by: Option<String>,
}

/// Что играет у хозяина: трек, позиция и ближайшая очередь — без остального его состояния.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct JamNow {
    #[schema(required = true)]
    pub current: Option<JamQueueItem>,
    /// Ближайшие треки: добавленные вручную, затем остальное (до 30).
    pub queue: Vec<JamQueueItem>,
    pub playing: bool,
    /// Позиция на момент `at`.
    pub position_ms: u32,
    pub duration_ms: u32,
    /// Время сервера, к которому относится позиция.
    pub at: DateTime<Utc>,
    /// Активное устройство хозяина на связи.
    pub live: bool,
}

/// `GET /api/jam/guest`: всё для страницы гостя.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct JamGuestView {
    pub jam_id: Uuid,
    pub host_name: String,
    pub settings: JamSettings,
    pub me: JamGuest,
    pub guests: Vec<JamGuest>,
    #[schema(required = true)]
    pub now: Option<JamNow>,
    /// Свои треки (ждут одобрения, добавлены, отклонены), новые первыми.
    pub mine: Vec<JamRequest>,
    /// Сколько голосов «пропустить» уже за текущий трек и сколько нужно.
    pub skip: JamSkip,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct JamSkip {
    pub votes: u32,
    pub needed: u32,
    /// Этот гость уже голосовал за текущий трек.
    pub voted: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct JamSearchQuery {
    /// Запрос, 1–100 символов.
    pub q: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct JamSearchResults {
    pub items: Vec<JamTrack>,
}

/// `POST /api/jam/guest/queue`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct JamAdd {
    pub track_id: Uuid,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum JamReactionKind {
    Heart,
}

/// `POST /api/jam/guest/react`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct JamReact {
    pub kind: JamReactionKind,
}

/// Ссылка на аудио для «Слушать у себя»: только текущий и следующий трек хозяина.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct JamStream {
    pub track_id: Uuid,
    pub url: String,
    pub expires_at: DateTime<Utc>,
    pub mime_type: String,
    pub duration_ms: i32,
    #[schema(required = true)]
    pub loudness_lufs: Option<f32>,
    #[schema(required = true)]
    pub true_peak_db: Option<f32>,
}

/// `jam.changed`: участники, настройки или треки гостей изменились — перечитать. `ended` — сессия
/// закрыта; `removed` — этого гостя убрал хозяин.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct JamChanged {
    pub jam_id: Uuid,
    #[serde(default)]
    pub ended: bool,
    #[serde(default)]
    #[schema(required = true)]
    pub removed: Option<Uuid>,
}

/// `jam.now` — гостям: что играет у хозяина (при каждом снимке его активного устройства).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct JamNowEvent {
    pub jam_id: Uuid,
    pub now: JamNow,
}

/// `jam.reaction` — всем в сессии: кто-то поставил ❤ текущему треку.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct JamReaction {
    pub jam_id: Uuid,
    pub guest_name: String,
    pub kind: JamReactionKind,
    #[schema(required = true)]
    pub track_id: Option<Uuid>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn statuses_round_trip() {
        for s in JamRequestStatus::ALL {
            assert_eq!(s.as_str().parse::<JamRequestStatus>().unwrap(), *s);
        }
        assert_eq!(
            serde_json::to_value(JamRequestStatus::Pending).unwrap(),
            "pending"
        );
    }
}
