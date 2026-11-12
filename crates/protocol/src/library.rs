//! Медиатека: лайки, сохранённые альбомы, подписки на артистов, «не рекомендовать» и плейлисты.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::catalog::{AlbumSummary, ArtistSummary, ImageSet, Track};

/// Что докачивать на сервер по подписке на артиста.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DownloadPolicy {
    pub albums: bool,
    pub eps: bool,
    pub singles: bool,
    pub compilations: bool,
    /// Концертные записи (по названию релиза).
    pub live: bool,
    /// Релизы, где главный артист альбома — другой (совместные, «участие»).
    pub appears_on: bool,
}

impl Default for DownloadPolicy {
    fn default() -> Self {
        Self {
            albums: true,
            eps: true,
            singles: true,
            compilations: false,
            live: false,
            appears_on: false,
        }
    }
}

/// Подписаться или изменить подписку: только переданные поля.
#[derive(Debug, Clone, Default, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FollowRequest {
    #[serde(default)]
    pub download_policy: Option<DownloadPolicy>,
    /// Сообщать о новых релизах.
    #[serde(default)]
    pub notify: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct FollowedArtist {
    pub artist: ArtistSummary,
    pub policy: DownloadPolicy,
    pub notify: bool,
    pub followed_at: DateTime<Utc>,
    /// Сколько треков дискографии на сервере и сколько всего.
    pub tracks_on_server: i64,
    pub tracks_total: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct FollowedArtistPage {
    pub items: Vec<FollowedArtist>,
    #[schema(required = true)]
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct LikedTrack {
    pub track: Track,
    pub liked_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct LikedTrackPage {
    pub items: Vec<LikedTrack>,
    #[schema(required = true)]
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SavedAlbum {
    pub album: AlbumSummary,
    pub saved_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SavedAlbumPage {
    pub items: Vec<SavedAlbum>,
    #[schema(required = true)]
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, IntoParams)]
#[serde(rename_all = "camelCase")]
#[into_params(parameter_in = Query)]
pub struct PageQuery {
    /// `nextCursor` из предыдущей страницы.
    pub cursor: Option<String>,
    /// Сколько вернуть: 1–200, по умолчанию 50.
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum BlockedKind {
    Track,
    Artist,
}

names!(BlockedKind { Track => "track", Artist => "artist" });

/// «Не рекомендовать»: трек или артист.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct BlockedItem {
    pub kind: BlockedKind,
    pub id: Uuid,
    pub created_at: DateTime<Utc>,
    /// Есть у `kind = track`, если трек ещё в каталоге.
    #[schema(required = true)]
    pub track: Option<Track>,
    /// Есть у `kind = artist`.
    #[schema(required = true)]
    pub artist: Option<ArtistSummary>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct BlockedList {
    pub items: Vec<BlockedItem>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum PlaylistKind {
    User,
    Smart,
    Mix,
    System,
}

names!(PlaylistKind { User => "user", Smart => "smart", Mix => "mix", System => "system" });

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistSummary {
    pub id: Uuid,
    pub title: String,
    pub description: String,
    pub kind: PlaylistKind,
    pub pinned: bool,
    pub tracks_count: i64,
    pub duration_ms: i64,
    /// Обложки первых альбомов плейлиста (до четырёх) — для мозаики.
    pub covers: Vec<ImageSet>,
    /// Своя обложка, загруженная человеком; без неё — мозаика из `covers`.
    #[serde(default)]
    pub image: Option<ImageSet>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    /// Папка, в которой лежит плейлист; `null` — вне папок.
    #[serde(default)]
    pub folder_id: Option<Uuid>,
    /// Место в «Своём порядке» (дробное, по возрастанию): вне папок — общая шкала с папками.
    #[serde(default)]
    pub position: f64,
    /// Когда плейлист в последний раз играл — для порядка «Недавние».
    #[serde(default)]
    pub played_at: Option<DateTime<Utc>>,
    /// Кто собрал плейлист вида `mix`: `daily_mix`, `discover_weekly`, `on_repeat`, `repeat_rewind`, `rewind`…
    #[serde(default)]
    pub generator: Option<String>,
}

/// Папка плейлистов в медиатеке и боковой панели. Вложенных папок нет.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistFolder {
    pub id: Uuid,
    pub title: String,
    /// Место в «Своём порядке» среди папок и плейлистов вне папок.
    pub position: f64,
    /// Свёрнута ли — общее для всех устройств.
    pub collapsed: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistList {
    pub items: Vec<PlaylistSummary>,
    /// Папки по «Своему порядку»; плейлисты ссылаются на них `folderId`.
    #[serde(default)]
    pub folders: Vec<PlaylistFolder>,
}

/// Новая папка; названные плейлисты сразу переезжают в неё.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NewPlaylistFolder {
    pub title: String,
    #[serde(default)]
    pub playlist_ids: Vec<Uuid>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PlaylistFolderPatch {
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub collapsed: Option<bool>,
}

/// Переложить плейлист: в папку (`folderId`) или из неё (`null`) и на место среди соседей. Сосед
/// `afterId` — плейлист или (вне папок) папка; без него — в конец, при `atStart` — в начало.
#[derive(Debug, Clone, Default, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MovePlaylist {
    #[serde(default)]
    pub folder_id: Option<Uuid>,
    #[serde(default)]
    pub after_id: Option<Uuid>,
    #[serde(default)]
    pub at_start: bool,
}

/// Переставить папку среди папок и плейлистов вне папок.
#[derive(Debug, Clone, Default, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MovePlaylistFolder {
    #[serde(default)]
    pub after_id: Option<Uuid>,
    #[serde(default)]
    pub at_start: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistItem {
    pub id: Uuid,
    /// Дробная позиция: порядок — по возрастанию.
    pub position: f64,
    pub added_at: DateTime<Utc>,
    pub track: Track,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistDetail {
    pub playlist: PlaylistSummary,
    pub items: Vec<PlaylistItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NewPlaylist {
    pub title: String,
    #[serde(default)]
    pub description: Option<String>,
    /// Треки в этом порядке.
    #[serde(default)]
    pub track_ids: Vec<Uuid>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PlaylistPatch {
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
}

/// Добавить треки: в конец, в начало или после позиции.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AddPlaylistItems {
    pub track_ids: Vec<Uuid>,
    /// После этой позиции; без неё — в конец (или в начало при `atStart`).
    #[serde(default)]
    pub after_item_id: Option<Uuid>,
    #[serde(default)]
    pub at_start: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AddedPlaylistItems {
    pub items: Vec<PlaylistItem>,
    /// Позиции остальных треков пересчитаны: интерфейсу стоит перечитать плейлист.
    pub rebalanced: bool,
}

/// Убрать несколько позиций разом (выбранные в списке).
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RemovePlaylistItems {
    pub item_ids: Vec<Uuid>,
}

/// Копия плейлиста или микса — обычный плейлист человека.
#[derive(Debug, Clone, Default, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DuplicatePlaylist {
    /// Название копии; без него — как у исходного.
    #[serde(default)]
    pub title: Option<String>,
}

/// Переставить позицию: после другой или в начало (`afterItemId: null`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MovePlaylistItem {
    #[serde(default)]
    pub after_item_id: Option<Uuid>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct MovedPlaylistItem {
    pub id: Uuid,
    pub position: f64,
    /// Позиции остальных треков пересчитаны: интерфейсу стоит перечитать плейлист.
    pub rebalanced: bool,
}

/// Порядок плейлиста целиком: названные позиции — в этом порядке, остальные (добавленные тем
/// временем) — за ними в прежнем порядке. Так «Отменить» возвращает порядок до сборки сета.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PlaylistOrder {
    pub item_ids: Vec<Uuid>,
}

/// «Собрать как сет»: как идёт энергия.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum SetCurve {
    /// Нарастание: от спокойного к пику в конце.
    #[default]
    Rise,
    /// Волна: подъём, пик около двух третей, спад.
    Wave,
    /// Ровно: как можно меньше скачков.
    Flat,
}

names!(SetCurve { Rise => "rise", Wave => "wave", Flat => "flat" });

/// Настройки сборки сета.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SetOptions {
    #[serde(default)]
    pub curve: SetCurve,
    /// Позиция, с которой начать сет; без неё первый выбирается сам.
    #[serde(default)]
    pub first_item_id: Option<Uuid>,
    /// Не ставить подряд треки одного исполнителя.
    #[serde(default = "yes")]
    pub spread_artists: bool,
}

fn yes() -> bool {
    true
}

impl Default for SetOptions {
    fn default() -> Self {
        Self {
            curve: SetCurve::Rise,
            first_item_id: None,
            spread_artists: true,
        }
    }
}

/// Как трек переходит из предыдущего — мягкая подсказка в списке.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum SetFit {
    /// Темп близко, тональности совместимы: «в тон».
    InKey,
    /// Темп близко, тональности не спорят: «плавно».
    Smooth,
    /// Энергия заметно выше.
    Lift,
    /// Энергия заметно ниже.
    Calm,
    /// Смена темпа или тональности.
    Switch,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SetEntry {
    pub item_id: Uuid,
    pub track_id: Uuid,
    /// Энергия трека 0…1 — для линии энергии сета.
    pub energy: Option<f32>,
    /// Переход из предыдущего; у первого и у треков без анализа — нет.
    pub fit: Option<SetFit>,
    /// Поставлен по анализу; `false` — анализа нет, трек в конце в прежнем порядке.
    pub placed: bool,
}

/// Порядок сета: новый порядок позиций и что изменится.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SetPreview {
    pub curve: SetCurve,
    /// Все позиции плейлиста в новом порядке.
    pub entries: Vec<SetEntry>,
    /// Треков без анализа (они в конце).
    pub unplaced: i64,
    /// Переходов, которые сводятся плавно: сейчас и в новом порядке.
    pub smooth_before: i64,
    pub smooth_after: i64,
    /// Новый порядок отличается от нынешнего.
    pub changed: bool,
    /// Плейлист собирается сам (микс): применить можно к своей копии.
    pub read_only: bool,
}

/// Сет применён: новый порядок и прежний — для «Отменить».
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SetApplied {
    pub set: SetPreview,
    /// Порядок позиций до сборки: вернуть — `PUT /api/playlists/{id}/order`.
    pub previous_item_ids: Vec<Uuid>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct LibraryCounts {
    pub liked_tracks: i64,
    pub saved_albums: i64,
    pub followed_artists: i64,
    pub playlists: i64,
    /// Треки медиатеки, которые уже на сервере.
    pub ready_tracks: i64,
}

/// Начальный экран медиатеки.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct LibraryOverview {
    pub counts: LibraryCounts,
    pub pinned_playlists: Vec<PlaylistSummary>,
    /// Подписки, недавние первыми (до 50).
    pub followed_artists: Vec<FollowedArtist>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_policy_matches_database() {
        // Значение по умолчанию в followed_artists.download_policy
        let db: DownloadPolicy = serde_json::from_str(
            r#"{"albums": true, "eps": true, "singles": true, "compilations": false, "live": false, "appearsOn": false}"#,
        )
        .unwrap();
        assert_eq!(db, DownloadPolicy::default());
    }
}
