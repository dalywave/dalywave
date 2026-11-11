//! Синхронизация локальной базы приложений: `GET /api/sync?since=<cursor>`.
//!
//! Приложение хранит `cursor` из последнего ответа и передаёт его в `since` (первый раз — без него).
//! Пока `hasMore`, сразу просит следующую страницу. Строки — полное текущее состояние; применяются
//! upsert'ом, если `rev` строки не меньше локального. Tombstone удаляет строку, если её локальный `rev`
//! меньше его `rev`. Одна и та же строка может прийти повторно — это нормально.

use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::{
    catalog::{ArtistRole, AudioStatus, ImageSet, RecordType},
    library::{BlockedKind, DownloadPolicy, PlaylistKind},
};

#[derive(Debug, Clone, Default, Serialize, Deserialize, IntoParams)]
#[serde(rename_all = "camelCase")]
#[into_params(parameter_in = Query)]
pub struct SyncQuery {
    /// `cursor` из предыдущего ответа; без него — полная синхронизация.
    pub since: Option<String>,
    /// Сколько изменений в странице: 1–2000, по умолчанию 500. Связанные строки каталога — сверх этого.
    pub limit: Option<u32>,
}

/// Таблица локальной базы.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub enum SyncEntity {
    Artists,
    Albums,
    Tracks,
    Recordings,
    LikedTracks,
    SavedAlbums,
    FollowedArtists,
    BlockedItems,
    Playlists,
    PlaylistItems,
    DeviceDownloads,
}

names!(SyncEntity {
    Artists => "artists",
    Albums => "albums",
    Tracks => "tracks",
    Recordings => "recordings",
    LikedTracks => "likedTracks",
    SavedAlbums => "savedAlbums",
    FollowedArtists => "followedArtists",
    BlockedItems => "blockedItems",
    Playlists => "playlists",
    PlaylistItems => "playlistItems",
    DeviceDownloads => "deviceDownloads",
});

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SyncArtist {
    pub id: Uuid,
    pub rev: i64,
    #[schema(required = true)]
    pub deezer_id: Option<i64>,
    pub name: String,
    #[schema(required = true)]
    pub image: Option<ImageSet>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SyncAlbum {
    pub id: Uuid,
    pub rev: i64,
    #[schema(required = true)]
    pub deezer_id: Option<i64>,
    pub title: String,
    pub artist_id: Uuid,
    pub record_type: RecordType,
    #[schema(required = true)]
    pub release_date: Option<NaiveDate>,
    #[schema(required = true)]
    pub label: Option<String>,
    pub genres: Vec<String>,
    pub explicit: bool,
    #[schema(required = true)]
    pub tracks_count: Option<i32>,
    #[schema(required = true)]
    pub duration_ms: Option<i32>,
    #[schema(required = true)]
    pub image: Option<ImageSet>,
    #[schema(required = true)]
    pub palette: Option<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SyncTrackArtist {
    pub artist_id: Uuid,
    pub role: ArtistRole,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SyncTrack {
    pub id: Uuid,
    pub rev: i64,
    #[schema(required = true)]
    pub deezer_id: Option<i64>,
    pub title: String,
    pub version: String,
    #[schema(required = true)]
    pub album_id: Option<Uuid>,
    pub disc_number: i16,
    #[schema(required = true)]
    pub track_position: Option<i16>,
    pub duration_ms: i32,
    pub explicit: bool,
    #[schema(required = true)]
    pub isrc: Option<String>,
    #[schema(required = true)]
    pub recording_id: Option<Uuid>,
    /// По порядку: основные, затем приглашённые.
    pub artists: Vec<SyncTrackArtist>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SyncRecording {
    pub id: Uuid,
    pub rev: i64,
    #[schema(required = true)]
    pub isrc: Option<String>,
    pub duration_ms: i32,
    pub audio_status: AudioStatus,
    #[schema(required = true)]
    pub primary_file_id: Option<Uuid>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SyncLikedTrack {
    pub track_id: Uuid,
    pub rev: i64,
    pub liked_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SyncSavedAlbum {
    pub album_id: Uuid,
    pub rev: i64,
    pub saved_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SyncFollowedArtist {
    pub artist_id: Uuid,
    pub rev: i64,
    pub policy: DownloadPolicy,
    pub notify: bool,
    pub followed_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SyncBlockedItem {
    pub kind: BlockedKind,
    pub ref_id: Uuid,
    pub rev: i64,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SyncPlaylist {
    pub id: Uuid,
    pub rev: i64,
    pub title: String,
    pub description: String,
    pub kind: PlaylistKind,
    pub pinned: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SyncPlaylistItem {
    pub id: Uuid,
    pub rev: i64,
    pub playlist_id: Uuid,
    pub track_id: Uuid,
    pub position: f64,
    pub added_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SyncDeviceDownload {
    pub id: Uuid,
    pub rev: i64,
    pub device_id: Uuid,
    /// `track`, `album`, `playlist`, `artist` или `liked`.
    pub kind: String,
    #[schema(required = true)]
    pub ref_id: Option<Uuid>,
}

/// Удаление строки. `key` — ключ строки в camelCase, например `{ "trackId": … }` у `likedTracks`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SyncTombstone {
    pub rev: i64,
    pub entity: SyncEntity,
    #[schema(value_type = Object)]
    pub key: Value,
    pub deleted_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SyncPage {
    /// Передать в `since` в следующий раз.
    pub cursor: String,
    /// Есть ещё изменения: запросить следующую страницу сразу.
    pub has_more: bool,
    pub artists: Vec<SyncArtist>,
    pub albums: Vec<SyncAlbum>,
    pub tracks: Vec<SyncTrack>,
    pub recordings: Vec<SyncRecording>,
    pub liked_tracks: Vec<SyncLikedTrack>,
    pub saved_albums: Vec<SyncSavedAlbum>,
    pub followed_artists: Vec<SyncFollowedArtist>,
    pub blocked_items: Vec<SyncBlockedItem>,
    pub playlists: Vec<SyncPlaylist>,
    pub playlist_items: Vec<SyncPlaylistItem>,
    pub device_downloads: Vec<SyncDeviceDownload>,
    pub tombstones: Vec<SyncTombstone>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entities_are_camel_case() {
        for e in SyncEntity::ALL {
            assert_eq!(serde_json::to_value(e).unwrap(), e.as_str());
        }
    }
}
