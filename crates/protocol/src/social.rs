//! Друзья и совместные плейлисты: запросы в друзья, ссылка-приглашение, что слушают друзья, страница
//! друга с совпадением вкусов, приватность, участники совместного плейлиста.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::{
    catalog::{ArtistSummary, Track},
    events::ContextType,
    library::PlaylistSummary,
};

/// Человек на сервере: имя и аватар из daly.rest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PersonRef {
    pub id: Uuid,
    pub name: String,
    #[serde(default)]
    #[schema(required = true)]
    pub avatar_url: Option<String>,
}

/// Кем человек приходится тому, кто спрашивает.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum Relation {
    /// Это я сам.
    Me,
    None,
    Friend,
    /// Я отправил запрос, ответа ещё нет.
    Outgoing,
    /// Он отправил запрос мне.
    Incoming,
}

names!(Relation {
    Me => "me",
    None => "none",
    Friend => "friend",
    Outgoing => "outgoing",
    Incoming => "incoming",
});

/// Человек в поиске и в приглашении: кем приходится.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PersonCard {
    pub person: PersonRef,
    pub relation: Relation,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PeopleList {
    pub items: Vec<PersonCard>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, IntoParams)]
#[serde(rename_all = "camelCase")]
#[into_params(parameter_in = Query)]
pub struct PeopleQuery {
    /// Часть имени; пусто — все, у кого есть доступ к сервису.
    pub q: Option<String>,
}

/// Откуда играет: альбом, плейлист, артист — нажатие открывает его.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ActivityContext {
    #[serde(rename = "type")]
    pub kind: ContextType,
    pub id: String,
    pub title: String,
}

/// Что слушает (или слушал последним) друг.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct FriendActivity {
    pub track: Track,
    /// Играет прямо сейчас; иначе — последнее, что слушал, в `at`.
    pub now: bool,
    /// Сейчас — когда начался трек; последнее — когда слушал.
    pub at: DateTime<Utc>,
    #[serde(default)]
    #[schema(required = true)]
    pub context: Option<ActivityContext>,
    /// У друга открыто «Слушать вместе»: код для `/jam/{code}`.
    #[serde(default)]
    #[schema(required = true)]
    pub jam_code: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Friend {
    pub person: PersonRef,
    pub since: DateTime<Utc>,
    /// `null` — ничего не слушал или скрывает, что слушает.
    #[serde(default)]
    #[schema(required = true)]
    pub activity: Option<FriendActivity>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct FriendRequest {
    pub person: PersonRef,
    pub created_at: DateTime<Utc>,
}

/// Друзья (слушающие сейчас — первыми) и запросы.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct FriendList {
    pub friends: Vec<Friend>,
    /// Ждут моего ответа.
    pub incoming: Vec<FriendRequest>,
    /// Я отправил, ответа нет.
    pub outgoing: Vec<FriendRequest>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NewFriendRequest {
    pub user_id: Uuid,
}

/// Чем закончился запрос: отправлен или (встречный уже был) — сразу друзья.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct FriendRequestResult {
    pub relation: Relation,
}

/// Ссылка «Добавить в друзья»: кто откроет её, станет другом сразу.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct InviteLink {
    pub code: String,
    /// Полный адрес для QR-кода и «Поделиться».
    pub url: String,
}

/// Артист в топе друга за месяц.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct FriendTopArtist {
    pub artist: ArtistSummary,
    pub plays: i64,
}

/// Совпадение вкусов. Честная метрика из двух частей (каждая 0…1):
/// - `artists` — пересечение долей прослушиваний по артистам за полгода: сумма по общим артистам
///   меньшей из двух долей (1 — слушаете одних и тех же в тех же пропорциях);
/// - `sound` — насколько близки грани вкуса по звучанию (взвешенная лучшая пара граней, косинус
///   приведён к шкале, где «случайные треки» — около нуля).
///
/// `percent` — среднее двух частей (или одна, если второй нет).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TasteMatch {
    pub percent: i32,
    #[serde(default)]
    #[schema(required = true)]
    pub artists: Option<f32>,
    #[serde(default)]
    #[schema(required = true)]
    pub sound: Option<f32>,
}

/// Страница друга.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct FriendProfile {
    pub person: PersonRef,
    pub relation: Relation,
    /// Друзья с этого момента (только у друзей).
    #[serde(default)]
    #[schema(required = true)]
    pub since: Option<DateTime<Utc>>,
    #[serde(default)]
    #[schema(required = true)]
    pub activity: Option<FriendActivity>,
    /// Его плейлисты, открытые друзьям.
    pub playlists: Vec<PlaylistSummary>,
    /// Топ артистов за 30 дней.
    pub top_artists: Vec<FriendTopArtist>,
    /// `null` — мало прослушиваний у кого-то из двоих.
    #[serde(default)]
    #[schema(required = true)]
    pub taste_match: Option<TasteMatch>,
    /// Ваши общие артисты: кого слушаете оба, самые общие первыми.
    pub common_artists: Vec<ArtistSummary>,
}

/// Приватная сессия: на 6 часов или до выключения.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum PrivateSessionMode {
    Off,
    /// На 6 часов.
    Hours6,
    /// До выключения.
    UntilOff,
}

/// Приватность прослушиваний.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Privacy {
    /// «Показывать друзьям, что я слушаю».
    pub share_listening: bool,
    /// Приватная сессия идёт: друзья не видят, что играет, а сыгранное в ней не попадает в
    /// «последнее» и в топ на странице друга.
    pub private_session: bool,
    /// Когда сессия закончится сама; `null` — до выключения (или сессии нет).
    #[serde(default)]
    #[schema(required = true)]
    pub private_until: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PrivacyPatch {
    #[serde(default)]
    pub share_listening: Option<bool>,
    #[serde(default)]
    pub private_session: Option<PrivateSessionMode>,
}

// ---------- Совместные плейлисты ----------

/// Кем человек приходится плейлисту.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum PlaylistAccess {
    /// Свой: можно всё.
    #[default]
    Owner,
    /// Участник совместного: добавляет, убирает и переставляет треки, может выйти.
    Member,
    /// Плейлист друга, открытый друзьям: только слушать и сохранить копию.
    Viewer,
}

names!(PlaylistAccess { Owner => "owner", Member => "member", Viewer => "viewer" });

/// Пригласить друзей в совместный плейлист.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AddPlaylistMembers {
    pub user_ids: Vec<Uuid>,
}

/// Открыть или скрыть плейлист от друзей.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PlaylistVisibility {
    pub friends_can_see: bool,
}

/// Приглашение в совместный плейлист по ссылке: что за плейлист, чей и вошёл ли уже.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistInvite {
    pub playlist: PlaylistSummary,
    pub owner: PersonRef,
    /// Уже участник (или владелец): ссылка просто открывает плейлист.
    pub joined: bool,
}

// ---------- Живые события ----------

/// Друзья или запросы изменились: перечитать список.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct FriendsChange {
    /// Что поменялось: `requests`, `friends`, `privacy`.
    pub what: String,
}

/// Друг начал слушать другое (или перестал).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct FriendActivityEvent {
    pub user_id: Uuid,
    #[serde(default)]
    #[schema(required = true)]
    pub activity: Option<FriendActivity>,
}
