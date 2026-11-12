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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
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
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistList {
    pub items: Vec<PlaylistSummary>,
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
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
