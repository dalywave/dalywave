//! Раздел «Люди» админки: список пользователей, «Сейчас» (кто в сети и что слушает) и страница человека.

use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::{
    admin::{AdminUser, InviteStatus},
    auth::{Platform, Role},
    catalog::{ImageSet, Track},
    connect::{ConnectKind, PlayContext},
    events::ContextType,
};

/// Человек коротко: для «Сейчас», приглашений и истории.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PersonBrief {
    pub id: Uuid,
    pub name: String,
    #[schema(required = true)]
    pub avatar_url: Option<String>,
}

/// Строка списка пользователей.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UserSummary {
    pub user: AdminUser,
    /// Хотя бы одно устройство на связи прямо сейчас.
    pub online: bool,
    /// Последний признак жизни: устройство, сессия или вход.
    #[schema(required = true)]
    pub last_seen_at: Option<DateTime<Utc>>,
    /// Сколько слушал за 30 дней.
    pub listened_ms_30d: i64,
    pub plays_30d: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UserDirectory {
    pub items: Vec<UserSummary>,
}

/// Устройство человека на связи.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct LiveDevice {
    pub id: String,
    pub name: String,
    pub kind: ConnectKind,
    pub platform: Platform,
    #[schema(required = true)]
    pub app_version: Option<String>,
    /// Звук играет здесь.
    pub active: bool,
}

/// Что играет у человека.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct LiveNow {
    /// Идёт приватная сессия: трек и источник не показываются.
    pub private: bool,
    #[schema(required = true)]
    pub track: Option<Track>,
    #[schema(required = true)]
    pub context: Option<PlayContext>,
    pub playing: bool,
    /// Позиция на момент `at`; пока играет, время идёт.
    pub position_ms: u32,
    pub duration_ms: u32,
    pub at: DateTime<Utc>,
    /// Устройство, на котором играет.
    #[schema(required = true)]
    pub device_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct LivePerson {
    pub user: PersonBrief,
    pub devices: Vec<LiveDevice>,
    #[schema(required = true)]
    pub now: Option<LiveNow>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct LiveSummary {
    /// Людей в сети.
    pub online: i64,
    /// Из них слушают (играет, не на паузе).
    pub listening: i64,
    /// Прослушиваний за сутки.
    pub plays_24h: i64,
    /// Новых людей за неделю.
    pub new_users_7d: i64,
}

/// «Сейчас»: кто в сети и что слушает.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct LiveOverview {
    pub summary: LiveSummary,
    /// Слушающие — первыми.
    pub people: Vec<LivePerson>,
}

/// Живое событие админке: у человека сменилось, что играет, или устройства.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AdminPresence {
    pub user_id: Uuid,
}

/// Вход через провайдера.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UserIdentityInfo {
    pub id: Uuid,
    /// Название провайдера из настроек (или адрес отключённого).
    pub provider: String,
    pub issuer: String,
    #[schema(required = true)]
    pub email: Option<String>,
    pub created_at: DateTime<Utc>,
    #[schema(required = true)]
    pub last_login_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct StatArtist {
    pub id: Uuid,
    pub name: String,
    #[schema(required = true)]
    pub image: Option<ImageSet>,
    pub plays: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct StatTrack {
    pub track: Track,
    pub plays: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct StatGenre {
    pub name: String,
    pub plays: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct StatDay {
    pub day: NaiveDate,
    pub listened_ms: i64,
    pub plays: i64,
}

/// Статистика человека. Топы — без приватных сессий.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UserStats {
    pub listened_ms_7d: i64,
    pub listened_ms_30d: i64,
    pub plays_7d: i64,
    pub plays_30d: i64,
    pub plays_total: i64,
    /// За 30 дней.
    pub top_artists: Vec<StatArtist>,
    pub top_tracks: Vec<StatTrack>,
    pub genres: Vec<StatGenre>,
    /// 30 дней по порядку, пустые дни — нулями.
    pub days: Vec<StatDay>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UserCounts {
    pub playlists: i64,
    pub liked_tracks: i64,
    pub saved_albums: i64,
    pub followed_artists: i64,
    pub friends: i64,
    pub imports: i64,
}

/// Приглашение, связанное с человеком.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UserInvite {
    pub id: Uuid,
    pub roles: Vec<Role>,
    #[schema(required = true)]
    pub note: Option<String>,
    pub status: InviteStatus,
    pub created_at: DateTime<Utc>,
    /// Отправил (у «пришёл по приглашению») или принял (у «отправил»).
    #[schema(required = true)]
    pub person: Option<PersonBrief>,
    #[schema(required = true)]
    pub used_at: Option<DateTime<Utc>>,
}

/// Скачивание, которое вызвал человек (лайк, плейлист, воспроизведение…).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UserAcquisition {
    pub id: Uuid,
    pub track_id: Uuid,
    pub title: String,
    pub artist: String,
    pub status: String,
    pub reason: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UserAcquisitions {
    pub total: i64,
    pub ready: i64,
    pub failed: i64,
    pub active: i64,
    pub recent: Vec<UserAcquisition>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UserFollowedArtist {
    pub id: Uuid,
    pub name: String,
    #[schema(required = true)]
    pub image: Option<ImageSet>,
}

/// Страница человека в админке.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UserDetail {
    pub summary: UserSummary,
    pub identities: Vec<UserIdentityInfo>,
    /// Когда заблокирован.
    #[schema(required = true)]
    pub disabled_at: Option<DateTime<Utc>>,
    /// Имя задано вручную: провайдер при входе его не меняет.
    pub name_locked: bool,
    /// Единственный незаблокированный администратор: заблокировать, удалить и снять admin нельзя.
    pub last_admin: bool,
    #[schema(required = true)]
    pub live: Option<LivePerson>,
    pub stats: UserStats,
    pub counts: UserCounts,
    /// По какому приглашению пришёл.
    #[schema(required = true)]
    pub invited_by: Option<UserInvite>,
    pub invites_sent: Vec<UserInvite>,
    /// Первые подписки на артистов (всего — в `counts`).
    pub followed: Vec<UserFollowedArtist>,
    pub acquisitions: UserAcquisitions,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, IntoParams)]
#[serde(rename_all = "camelCase")]
#[into_params(parameter_in = Query)]
pub struct UserHistoryQuery {
    pub cursor: Option<String>,
    /// 1…200, по умолчанию 50.
    pub limit: Option<u32>,
}

/// Прослушивание в истории. В приватной сессии трек не показывается.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UserHistoryItem {
    pub id: Uuid,
    pub private: bool,
    #[schema(required = true)]
    pub track: Option<Track>,
    pub started_at: DateTime<Utc>,
    pub played_ms: i32,
    pub duration_ms: i32,
    pub end_reason: String,
    #[schema(required = true)]
    pub context_type: Option<ContextType>,
    #[schema(required = true)]
    pub device_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UserHistoryPage {
    pub items: Vec<UserHistoryItem>,
    #[schema(required = true)]
    pub next_cursor: Option<String>,
}

/// Удаление аккаунта: имя человека для подтверждения.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UserDeletion {
    pub confirm_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct RevokedSessions {
    pub revoked: u64,
}
