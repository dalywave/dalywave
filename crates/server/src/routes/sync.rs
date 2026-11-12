//! Синхронизация локальной базы приложений: `GET /api/sync?since=<cursor>`.
//!
//! Изменения отдаются по возрастанию `rev`: строки медиатеки человека, каталожные строки, на которые
//! медиатека ссылается, и tombstones. `rev` выдаётся при записи, а видна строка становится при коммите,
//! поэтому полноту обеспечивает id транзакции: курсор помнит xmin снимка (все транзакции ниже него уже
//! закончились), и следующий проход берёт всё с `txid >= xmin`. Строка может прийти повторно, но не
//! потеряется. Связанные строки каталога (трек понравился, а сам трек старый) приходят вместе со страницей.

use std::collections::HashSet;

use axum::{Router, extract::State, routing::get};
use chrono::{DateTime, NaiveDate, Utc};
use dalywave_protocol::{
    ErrorBody,
    catalog::{ArtistRole, AudioStatus, RecordType},
    library::{BlockedKind, PlaylistKind},
    sync::{
        SyncAlbum, SyncArtist, SyncBlockedItem, SyncDeviceDownload, SyncDeviceSetItem, SyncEntity,
        SyncFollowedArtist, SyncLikedTrack, SyncPage, SyncPlaylist, SyncPlaylistItem, SyncQuery,
        SyncRecording, SyncSavedAlbum, SyncTombstone, SyncTrack, SyncTrackArtist,
        SyncTrackFeatures, SyncWavePreset,
    },
};
use serde_json::{Map, Value};
use sqlx::PgConnection;
use uuid::Uuid;

use crate::{
    auth::Principal,
    catalog::views::{ImageKind, image_set, palette},
    cursor,
    error::{AppResult, bad},
    extract::{Json, Query},
    state::AppState,
};

pub fn routes() -> Router<AppState> {
    Router::new().route("/sync", get(sync))
}

/// Курсор синхронизации. Снаружи — непрозрачная строка: `"<w>"` или посреди прохода `"<w>.<after>.<next>"`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SyncCursor {
    /// Нижняя граница транзакций этого прохода.
    pub watermark: u64,
    /// Последний отданный `rev`; 0 — проход только начинается.
    pub after: i64,
    /// Граница для следующего прохода: xmin снимка первой страницы.
    pub next: u64,
}

impl SyncCursor {
    pub fn parse(s: &str) -> Option<Self> {
        let parts: Vec<&str> = s.split('.').collect();
        match parts.as_slice() {
            [w] => Some(Self {
                watermark: w.parse().ok()?,
                ..Default::default()
            }),
            [w, after, next] => Some(Self {
                watermark: w.parse().ok()?,
                after: after.parse().ok().filter(|a: &i64| *a > 0)?,
                next: next.parse().ok()?,
            }),
            _ => None,
        }
    }

    pub fn encode(self) -> String {
        if self.after == 0 {
            self.watermark.to_string()
        } else {
            format!("{}.{}.{}", self.watermark, self.after, self.next)
        }
    }
}

enum Change {
    Artist(SyncArtist),
    Album(SyncAlbum),
    Track(SyncTrack),
    Recording(SyncRecording),
    Liked(SyncLikedTrack),
    Saved(SyncSavedAlbum),
    Followed(SyncFollowedArtist),
    Blocked(SyncBlockedItem),
    Playlist(SyncPlaylist),
    Item(SyncPlaylistItem),
    Download(SyncDeviceDownload),
    Set(SyncDeviceSetItem),
    Features(SyncTrackFeatures),
    Preset(SyncWavePreset),
    Tombstone(SyncTombstone),
}

impl Change {
    fn rev(&self) -> i64 {
        match self {
            Self::Artist(r) => r.rev,
            Self::Album(r) => r.rev,
            Self::Track(r) => r.rev,
            Self::Recording(r) => r.rev,
            Self::Liked(r) => r.rev,
            Self::Saved(r) => r.rev,
            Self::Followed(r) => r.rev,
            Self::Blocked(r) => r.rev,
            Self::Playlist(r) => r.rev,
            Self::Item(r) => r.rev,
            Self::Download(r) => r.rev,
            Self::Set(r) => r.rev,
            Self::Features(r) => r.rev,
            Self::Preset(r) => r.rev,
            Self::Tombstone(r) => r.rev,
        }
    }
}

struct ArtistRow {
    id: Uuid,
    rev: i64,
    deezer_id: Option<i64>,
    name: String,
    picture_key: Option<String>,
    picture_source: Option<String>,
}

impl From<ArtistRow> for SyncArtist {
    fn from(r: ArtistRow) -> Self {
        Self {
            image: image_set(
                ImageKind::Artists,
                r.id,
                r.picture_key.as_deref(),
                r.picture_source.as_deref(),
            ),
            id: r.id,
            rev: r.rev,
            deezer_id: r.deezer_id,
            name: r.name,
        }
    }
}

struct AlbumRow {
    id: Uuid,
    rev: i64,
    deezer_id: Option<i64>,
    title: String,
    artist_id: Uuid,
    record_type: String,
    release_date: Option<NaiveDate>,
    label: Option<String>,
    genres: Vec<String>,
    explicit: bool,
    tracks_count: Option<i32>,
    duration_ms: Option<i32>,
    cover_key: Option<String>,
    cover_source: Option<String>,
    cover_palette: Option<Value>,
}

impl From<AlbumRow> for SyncAlbum {
    fn from(r: AlbumRow) -> Self {
        Self {
            image: image_set(
                ImageKind::Albums,
                r.id,
                r.cover_key.as_deref(),
                r.cover_source.as_deref(),
            ),
            palette: palette(r.cover_palette),
            id: r.id,
            rev: r.rev,
            deezer_id: r.deezer_id,
            title: r.title,
            artist_id: r.artist_id,
            record_type: r.record_type.parse().unwrap_or(RecordType::Album),
            release_date: r.release_date,
            label: r.label,
            genres: r.genres,
            explicit: r.explicit,
            tracks_count: r.tracks_count,
            duration_ms: r.duration_ms,
        }
    }
}

struct TrackRow {
    id: Uuid,
    rev: i64,
    deezer_id: Option<i64>,
    title: String,
    title_version: String,
    album_id: Option<Uuid>,
    disc_number: i16,
    track_position: Option<i16>,
    duration_ms: i32,
    explicit: bool,
    isrc: Option<String>,
    recording_id: Option<Uuid>,
    artist_ids: Vec<Uuid>,
    artist_roles: Vec<String>,
}

impl From<TrackRow> for SyncTrack {
    fn from(r: TrackRow) -> Self {
        Self {
            artists: r
                .artist_ids
                .iter()
                .zip(&r.artist_roles)
                .map(|(id, role)| SyncTrackArtist {
                    artist_id: *id,
                    role: role.parse().unwrap_or(ArtistRole::Main),
                })
                .collect(),
            id: r.id,
            rev: r.rev,
            deezer_id: r.deezer_id,
            title: r.title,
            version: r.title_version,
            album_id: r.album_id,
            disc_number: r.disc_number,
            track_position: r.track_position,
            duration_ms: r.duration_ms,
            explicit: r.explicit,
            isrc: r.isrc,
            recording_id: r.recording_id,
        }
    }
}

struct RecordingRow {
    id: Uuid,
    rev: i64,
    isrc: Option<String>,
    duration_ms: i32,
    audio_status: String,
    primary_file_id: Option<Uuid>,
}

impl From<RecordingRow> for SyncRecording {
    fn from(r: RecordingRow) -> Self {
        Self {
            id: r.id,
            rev: r.rev,
            isrc: r.isrc,
            duration_ms: r.duration_ms,
            audio_status: r.audio_status.parse().unwrap_or(AudioStatus::None),
            primary_file_id: r.primary_file_id,
        }
    }
}

struct FeaturesRow {
    recording_id: Uuid,
    rev: i64,
    energy: Option<f32>,
    valence: Option<f32>,
    danceability: Option<f32>,
    bpm: Option<f32>,
    key_camelot: Option<String>,
    voice: Option<f32>,
    language: Option<String>,
    approachability: Option<f32>,
    relaxed: Option<f32>,
    party: Option<f32>,
}

impl From<FeaturesRow> for SyncTrackFeatures {
    fn from(r: FeaturesRow) -> Self {
        Self {
            recording_id: r.recording_id,
            rev: r.rev,
            energy: r.energy,
            valence: r.valence,
            danceability: r.danceability,
            bpm: r.bpm,
            key_camelot: r.key_camelot,
            voice: r.voice,
            language: r.language,
            approachability: r.approachability,
            relaxed: r.relaxed,
            party: r.party,
        }
    }
}

fn tombstone(
    rev: i64,
    table: &str,
    key: Value,
    deleted_at: DateTime<Utc>,
) -> Option<SyncTombstone> {
    let entity = match table {
        "liked_tracks" => SyncEntity::LikedTracks,
        "saved_albums" => SyncEntity::SavedAlbums,
        "followed_artists" => SyncEntity::FollowedArtists,
        "blocked_items" => SyncEntity::BlockedItems,
        "playlists" => SyncEntity::Playlists,
        "playlist_items" => SyncEntity::PlaylistItems,
        "device_downloads" => SyncEntity::DeviceDownloads,
        "device_sets" => SyncEntity::DeviceSets,
        "wave_presets" => SyncEntity::WavePresets,
        "artists" => SyncEntity::Artists,
        "albums" => SyncEntity::Albums,
        "tracks" => SyncEntity::Tracks,
        "recordings" => SyncEntity::Recordings,
        _ => return None,
    };
    Some(SyncTombstone {
        rev,
        entity,
        key: camel_key(key),
        deleted_at,
    })
}

/// Ключ строки в camelCase и без `user_id`: приложение и так знает, чья это медиатека.
fn camel_key(key: Value) -> Value {
    let Value::Object(map) = key else {
        return key;
    };
    let mut out = Map::new();
    for (k, v) in map {
        if k == "user_id" {
            continue;
        }
        let mut camel = String::with_capacity(k.len());
        let mut upper = false;
        for c in k.chars() {
            if c == '_' {
                upper = true;
            } else if upper {
                camel.extend(c.to_uppercase());
                upper = false;
            } else {
                camel.push(c);
            }
        }
        out.insert(camel, v);
    }
    Value::Object(out)
}

#[utoipa::path(
    get,
    path = "/api/sync",
    tag = "library",
    security(("session" = []), ("device" = [])),
    params(SyncQuery),
    responses(
        (status = 200, description = "Изменения после курсора и следующий курсор", body = SyncPage),
        (status = 400, description = "Неверный since или limit", body = ErrorBody),
    ),
)]
pub async fn sync(
    State(st): State<AppState>,
    principal: Principal,
    Query(q): Query<SyncQuery>,
) -> AppResult<Json<SyncPage>> {
    let user = principal.user.id;
    let limit = cursor::limit(q.limit, 500, 2000)?;
    let cur = match q
        .since
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty() && *s != "0")
    {
        None => SyncCursor::default(),
        Some(s) => SyncCursor::parse(s).ok_or_else(|| bad("неверный since"))?,
    };
    let mut tx = st.db.begin().await?;
    // Один снимок на все запросы страницы: xmin и данные должны быть из одного момента
    sqlx::query!("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ, READ ONLY")
        .execute(&mut *tx)
        .await?;
    let xmin =
        sqlx::query_scalar!(r#"SELECT pg_snapshot_xmin(pg_current_snapshot())::text AS "xmin!""#)
            .fetch_one(&mut *tx)
            .await?;
    let next = if cur.after == 0 {
        xmin.parse::<u64>().map_err(anyhow::Error::from)?
    } else {
        cur.next
    };
    let w = cur.watermark.to_string();
    let changes = collect(&mut tx, user, principal.device_id, &w, cur.after, limit + 1).await?;
    let mut changes = changes;
    changes.sort_by_key(Change::rev);
    let has_more = changes.len() > limit as usize;
    changes.truncate(limit as usize);
    let cursor = match (has_more, changes.last()) {
        (true, Some(last)) => SyncCursor {
            watermark: cur.watermark,
            after: last.rev(),
            next,
        },
        _ => SyncCursor {
            watermark: next,
            after: 0,
            next: 0,
        },
    };
    let mut page = SyncPage {
        cursor: cursor.encode(),
        has_more,
        ..Default::default()
    };
    for change in changes {
        match change {
            Change::Artist(r) => page.artists.push(r),
            Change::Album(r) => page.albums.push(r),
            Change::Track(r) => page.tracks.push(r),
            Change::Recording(r) => page.recordings.push(r),
            Change::Liked(r) => page.liked_tracks.push(r),
            Change::Saved(r) => page.saved_albums.push(r),
            Change::Followed(r) => page.followed_artists.push(r),
            Change::Blocked(r) => page.blocked_items.push(r),
            Change::Playlist(r) => page.playlists.push(r),
            Change::Item(r) => page.playlist_items.push(r),
            Change::Download(r) => page.device_downloads.push(r),
            Change::Set(r) => page.device_sets.push(r),
            Change::Features(r) => page.track_features.push(r),
            Change::Preset(r) => page.wave_presets.push(r),
            Change::Tombstone(r) => page.tombstones.push(r),
        }
    }
    add_dependencies(&mut tx, &mut page).await?;
    tx.commit().await?;
    Ok(Json(page))
}

/// Все изменения после курсора, не больше `fetch` из каждой таблицы.
async fn collect(
    conn: &mut PgConnection,
    user: Uuid,
    device: Option<Uuid>,
    w: &str,
    after: i64,
    fetch: i64,
) -> Result<Vec<Change>, sqlx::Error> {
    let mut out = Vec::new();
    out.extend(
        sqlx::query!(
            "SELECT track_id, created_at, rev FROM liked_tracks
             WHERE user_id = $1 AND txid >= $2::text::xid8 AND rev > $3 ORDER BY rev LIMIT $4",
            user,
            w,
            after,
            fetch,
        )
        .fetch_all(&mut *conn)
        .await?
        .into_iter()
        .map(|r| {
            Change::Liked(SyncLikedTrack {
                track_id: r.track_id,
                rev: r.rev,
                liked_at: r.created_at,
            })
        }),
    );
    out.extend(
        sqlx::query!(
            "SELECT album_id, created_at, rev FROM saved_albums
             WHERE user_id = $1 AND txid >= $2::text::xid8 AND rev > $3 ORDER BY rev LIMIT $4",
            user,
            w,
            after,
            fetch,
        )
        .fetch_all(&mut *conn)
        .await?
        .into_iter()
        .map(|r| {
            Change::Saved(SyncSavedAlbum {
                album_id: r.album_id,
                rev: r.rev,
                saved_at: r.created_at,
            })
        }),
    );
    out.extend(
        sqlx::query!(
            "SELECT artist_id, download_policy, notify, created_at, rev FROM followed_artists
             WHERE user_id = $1 AND txid >= $2::text::xid8 AND rev > $3 ORDER BY rev LIMIT $4",
            user,
            w,
            after,
            fetch,
        )
        .fetch_all(&mut *conn)
        .await?
        .into_iter()
        .map(|r| {
            Change::Followed(SyncFollowedArtist {
                artist_id: r.artist_id,
                rev: r.rev,
                policy: serde_json::from_value(r.download_policy).unwrap_or_default(),
                notify: r.notify,
                followed_at: r.created_at,
            })
        }),
    );
    out.extend(
        sqlx::query!(
            "SELECT kind, ref_id, created_at, rev FROM blocked_items
             WHERE user_id = $1 AND txid >= $2::text::xid8 AND rev > $3 ORDER BY rev LIMIT $4",
            user,
            w,
            after,
            fetch,
        )
        .fetch_all(&mut *conn)
        .await?
        .into_iter()
        .map(|r| {
            Change::Blocked(SyncBlockedItem {
                kind: r.kind.parse().unwrap_or(BlockedKind::Track),
                ref_id: r.ref_id,
                rev: r.rev,
                created_at: r.created_at,
            })
        }),
    );
    out.extend(
        sqlx::query!(
            "SELECT id, title, description, kind, pinned, created_at, updated_at, rev FROM playlists
             WHERE user_id = $1 AND txid >= $2::text::xid8 AND rev > $3 ORDER BY rev LIMIT $4",
            user, w, after, fetch,
        )
        .fetch_all(&mut *conn)
        .await?
        .into_iter()
        .map(|r| {
            Change::Playlist(SyncPlaylist {
                id: r.id,
                rev: r.rev,
                title: r.title,
                description: r.description,
                kind: r.kind.parse().unwrap_or(PlaylistKind::User),
                pinned: r.pinned,
                created_at: r.created_at,
                updated_at: r.updated_at,
            })
        }),
    );
    out.extend(
        sqlx::query!(
            "SELECT i.id, i.playlist_id, i.track_id, i.position, i.added_at, i.rev
             FROM playlist_items i JOIN playlists p ON p.id = i.playlist_id
             WHERE p.user_id = $1 AND i.txid >= $2::text::xid8 AND i.rev > $3 ORDER BY i.rev LIMIT $4",
            user, w, after, fetch,
        )
        .fetch_all(&mut *conn)
        .await?
        .into_iter()
        .map(|r| {
            Change::Item(SyncPlaylistItem {
                id: r.id,
                rev: r.rev,
                playlist_id: r.playlist_id,
                track_id: r.track_id,
                position: r.position,
                added_at: r.added_at,
            })
        }),
    );
    out.extend(
        sqlx::query!(
            "SELECT dd.id, dd.device_id, dd.kind, dd.ref_id, dd.rev
             FROM device_downloads dd JOIN devices d ON d.id = dd.device_id
             WHERE d.user_id = $1 AND dd.txid >= $2::text::xid8 AND dd.rev > $3 ORDER BY dd.rev LIMIT $4",
            user, w, after, fetch,
        )
        .fetch_all(&mut *conn)
        .await?
        .into_iter()
        .map(|r| {
            Change::Download(SyncDeviceDownload {
                id: r.id,
                rev: r.rev,
                device_id: r.device_id,
                kind: r.kind,
                ref_id: r.ref_id,
            })
        }),
    );
    // Набор волны — только этого устройства: у сайта его нет
    if let Some(device) = device {
        out.extend(
            sqlx::query!(
                "SELECT track_id, device_id, kind, score, rev FROM device_sets
                 WHERE device_id = $1 AND txid >= $2::text::xid8 AND rev > $3 ORDER BY rev LIMIT $4",
                device,
                w,
                after,
                fetch,
            )
            .fetch_all(&mut *conn)
            .await?
            .into_iter()
            .map(|r| {
                Change::Set(SyncDeviceSetItem {
                    track_id: r.track_id,
                    rev: r.rev,
                    device_id: r.device_id,
                    kind: r.kind,
                    score: r.score,
                })
            }),
        );
    }
    out.extend(
        sqlx::query_as!(
            FeaturesRow,
            r#"SELECT recording_id, rev, energy, valence, danceability, bpm, key_camelot, voice, language,
                      approachability, (moods ->> 'relaxed')::real AS relaxed, (moods ->> 'party')::real AS party
               FROM track_analysis
               WHERE recording_id IN (SELECT t.recording_id FROM tracks t WHERE t.id IN (SELECT user_track_ids($1)))
                 AND txid >= $2::text::xid8 AND rev > $3
               ORDER BY rev LIMIT $4"#,
            user,
            w,
            after,
            fetch,
        )
        .fetch_all(&mut *conn)
        .await?
        .into_iter()
        .map(|r| Change::Features(r.into())),
    );
    out.extend(
        sqlx::query!(
            "SELECT id, name, icon, settings, position, created_at, rev FROM wave_presets
             WHERE user_id = $1 AND txid >= $2::text::xid8 AND rev > $3 ORDER BY rev LIMIT $4",
            user,
            w,
            after,
            fetch,
        )
        .fetch_all(&mut *conn)
        .await?
        .into_iter()
        .map(|r| {
            Change::Preset(SyncWavePreset {
                id: r.id,
                rev: r.rev,
                name: r.name,
                icon: r.icon,
                settings: serde_json::from_value(r.settings).unwrap_or_default(),
                position: r.position,
                created_at: r.created_at,
            })
        }),
    );
    out.extend(
        sqlx::query!(
            "SELECT rev, table_name, row_key, deleted_at FROM tombstones
             WHERE user_id = $1 AND txid >= $2::text::xid8 AND rev > $3 ORDER BY rev LIMIT $4",
            user,
            w,
            after,
            fetch,
        )
        .fetch_all(&mut *conn)
        .await?
        .into_iter()
        .filter_map(|r| {
            tombstone(r.rev, &r.table_name, r.row_key, r.deleted_at).map(Change::Tombstone)
        }),
    );
    // Каталог: только строки, на которые ссылается медиатека человека
    out.extend(
        sqlx::query_as!(
            ArtistRow,
            "SELECT id, rev, deezer_id, name, picture_key, picture_source FROM artists
             WHERE id IN (SELECT user_artist_ids($1)) AND txid >= $2::text::xid8 AND rev > $3
             ORDER BY rev LIMIT $4",
            user,
            w,
            after,
            fetch,
        )
        .fetch_all(&mut *conn)
        .await?
        .into_iter()
        .map(|r| Change::Artist(r.into())),
    );
    out.extend(
        sqlx::query_as!(
            AlbumRow,
            "SELECT id, rev, deezer_id, title, artist_id, record_type, release_date, label, genres, explicit,
                    tracks_count, duration_ms, cover_key, cover_source, cover_palette
             FROM albums
             WHERE id IN (SELECT user_album_ids($1)) AND txid >= $2::text::xid8 AND rev > $3
             ORDER BY rev LIMIT $4",
            user, w, after, fetch,
        )
        .fetch_all(&mut *conn)
        .await?
        .into_iter()
        .map(|r| Change::Album(r.into())),
    );
    out.extend(
        sqlx::query_as!(
            TrackRow,
            r#"SELECT t.id, t.rev, t.deezer_id, t.title, t.title_version, t.album_id, t.disc_number,
                      t.track_position, t.duration_ms, t.explicit, t.isrc, t.recording_id,
                      ARRAY(SELECT ta.artist_id FROM track_artists ta WHERE ta.track_id = t.id
                            ORDER BY ta.position, ta.artist_id) AS "artist_ids!",
                      ARRAY(SELECT ta.role FROM track_artists ta WHERE ta.track_id = t.id
                            ORDER BY ta.position, ta.artist_id) AS "artist_roles!"
               FROM tracks t
               WHERE t.id IN (SELECT user_track_ids($1)) AND t.txid >= $2::text::xid8 AND t.rev > $3
               ORDER BY t.rev LIMIT $4"#,
            user,
            w,
            after,
            fetch,
        )
        .fetch_all(&mut *conn)
        .await?
        .into_iter()
        .map(|r| Change::Track(r.into())),
    );
    out.extend(
        sqlx::query_as!(
            RecordingRow,
            "SELECT id, rev, isrc, duration_ms, audio_status, primary_file_id FROM recordings
             WHERE id IN (SELECT t.recording_id FROM tracks t WHERE t.id IN (SELECT user_track_ids($1)))
               AND txid >= $2::text::xid8 AND rev > $3
             ORDER BY rev LIMIT $4",
            user, w, after, fetch,
        )
        .fetch_all(&mut *conn)
        .await?
        .into_iter()
        .map(|r| Change::Recording(r.into())),
    );
    Ok(out)
}

/// Каталожные строки, без которых строки медиатеки этой страницы не показать: понравился старый
/// трек — сам трек, его альбом, артисты и запись приходят вместе с лайком.
async fn add_dependencies(conn: &mut PgConnection, page: &mut SyncPage) -> Result<(), sqlx::Error> {
    let mut track_ids: Vec<Uuid> = page
        .liked_tracks
        .iter()
        .map(|r| r.track_id)
        .chain(page.playlist_items.iter().map(|r| r.track_id))
        .chain(
            page.blocked_items
                .iter()
                .filter(|b| b.kind == BlockedKind::Track)
                .map(|b| b.ref_id),
        )
        .chain(
            page.device_downloads
                .iter()
                .filter(|d| d.kind == "track")
                .filter_map(|d| d.ref_id),
        )
        .chain(page.device_sets.iter().map(|s| s.track_id))
        .collect();
    let mut album_ids: Vec<Uuid> = page
        .saved_albums
        .iter()
        .map(|r| r.album_id)
        .chain(
            page.device_downloads
                .iter()
                .filter(|d| d.kind == "album")
                .filter_map(|d| d.ref_id),
        )
        .collect();
    let mut artist_ids: Vec<Uuid> = page
        .followed_artists
        .iter()
        .map(|r| r.artist_id)
        .chain(
            page.blocked_items
                .iter()
                .filter(|b| b.kind == BlockedKind::Artist)
                .map(|b| b.ref_id),
        )
        .chain(
            page.device_downloads
                .iter()
                .filter(|d| d.kind == "artist")
                .filter_map(|d| d.ref_id),
        )
        .collect();
    if track_ids.is_empty()
        && album_ids.is_empty()
        && artist_ids.is_empty()
        && page.tracks.is_empty()
    {
        return add_features(conn, page).await;
    }
    // Сохранённый альбом приходит со всеми своими треками
    if !album_ids.is_empty() {
        track_ids.extend(
            sqlx::query_scalar!("SELECT id FROM tracks WHERE album_id = ANY($1)", &album_ids)
                .fetch_all(&mut *conn)
                .await?,
        );
    }
    let have: HashSet<Uuid> = page.tracks.iter().map(|t| t.id).collect();
    track_ids.retain(|id| !have.contains(id));
    track_ids.sort();
    track_ids.dedup();
    let tracks = sqlx::query_as!(
        TrackRow,
        r#"SELECT t.id, t.rev, t.deezer_id, t.title, t.title_version, t.album_id, t.disc_number,
                  t.track_position, t.duration_ms, t.explicit, t.isrc, t.recording_id,
                  ARRAY(SELECT ta.artist_id FROM track_artists ta WHERE ta.track_id = t.id
                        ORDER BY ta.position, ta.artist_id) AS "artist_ids!",
                  ARRAY(SELECT ta.role FROM track_artists ta WHERE ta.track_id = t.id
                        ORDER BY ta.position, ta.artist_id) AS "artist_roles!"
           FROM tracks t WHERE t.id = ANY($1)"#,
        &track_ids,
    )
    .fetch_all(&mut *conn)
    .await?;
    let mut recording_ids = Vec::new();
    for t in &tracks {
        album_ids.extend(t.album_id);
        artist_ids.extend(t.artist_ids.iter().copied());
        recording_ids.extend(t.recording_id);
    }
    // Записи, альбомы и артисты уже отданных треков тоже нужны, если сами треки пришли без них
    // (трек обновился в той же транзакции, что и лайк, — и пришёл изменением, а не зависимостью)
    for t in &page.tracks {
        recording_ids.extend(t.recording_id);
        album_ids.extend(t.album_id);
        artist_ids.extend(t.artists.iter().map(|a| a.artist_id));
    }
    page.tracks.extend(tracks.into_iter().map(SyncTrack::from));

    let have: HashSet<Uuid> = page.albums.iter().map(|a| a.id).collect();
    album_ids.retain(|id| !have.contains(id));
    album_ids.sort();
    album_ids.dedup();
    let albums = sqlx::query_as!(
        AlbumRow,
        "SELECT id, rev, deezer_id, title, artist_id, record_type, release_date, label, genres, explicit,
                tracks_count, duration_ms, cover_key, cover_source, cover_palette
         FROM albums WHERE id = ANY($1)",
        &album_ids,
    )
    .fetch_all(&mut *conn)
    .await?;
    artist_ids.extend(albums.iter().map(|a| a.artist_id));
    page.albums.extend(albums.into_iter().map(SyncAlbum::from));

    let have: HashSet<Uuid> = page.artists.iter().map(|a| a.id).collect();
    artist_ids.retain(|id| !have.contains(id));
    artist_ids.sort();
    artist_ids.dedup();
    let artists = sqlx::query_as!(
        ArtistRow,
        "SELECT id, rev, deezer_id, name, picture_key, picture_source FROM artists WHERE id = ANY($1)",
        &artist_ids,
    )
    .fetch_all(&mut *conn)
    .await?;
    page.artists
        .extend(artists.into_iter().map(SyncArtist::from));

    let have: HashSet<Uuid> = page.recordings.iter().map(|r| r.id).collect();
    recording_ids.retain(|id| !have.contains(id));
    recording_ids.sort();
    recording_ids.dedup();
    let recordings = sqlx::query_as!(
        RecordingRow,
        "SELECT id, rev, isrc, duration_ms, audio_status, primary_file_id FROM recordings WHERE id = ANY($1)",
        &recording_ids,
    )
    .fetch_all(&mut *conn)
    .await?;
    page.recordings
        .extend(recordings.into_iter().map(SyncRecording::from));
    add_features(conn, page).await
}

/// Признаки звучания записей этой страницы: волна без сети подбирает по ним настроение и темп.
async fn add_features(conn: &mut PgConnection, page: &mut SyncPage) -> Result<(), sqlx::Error> {
    let have: HashSet<Uuid> = page.track_features.iter().map(|f| f.recording_id).collect();
    let mut ids: Vec<Uuid> = page
        .recordings
        .iter()
        .map(|r| r.id)
        .chain(page.tracks.iter().filter_map(|t| t.recording_id))
        .filter(|id| !have.contains(id))
        .collect();
    if ids.is_empty() {
        return Ok(());
    }
    ids.sort();
    ids.dedup();
    let rows = sqlx::query_as!(
        FeaturesRow,
        r#"SELECT recording_id, rev, energy, valence, danceability, bpm, key_camelot, voice, language,
                  approachability, (moods ->> 'relaxed')::real AS relaxed, (moods ->> 'party')::real AS party
           FROM track_analysis WHERE recording_id = ANY($1)"#,
        &ids,
    )
    .fetch_all(&mut *conn)
    .await?;
    page.track_features
        .extend(rows.into_iter().map(SyncTrackFeatures::from));
    Ok(())
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn cursor_roundtrip() {
        assert_eq!(SyncCursor::parse("0"), Some(SyncCursor::default()));
        let mid = SyncCursor {
            watermark: 12,
            after: 345,
            next: 678,
        };
        assert_eq!(mid.encode(), "12.345.678");
        assert_eq!(SyncCursor::parse("12.345.678"), Some(mid));
        let done = SyncCursor {
            watermark: 678,
            ..Default::default()
        };
        assert_eq!(done.encode(), "678");
        assert_eq!(SyncCursor::parse("678"), Some(done));
        assert_eq!(SyncCursor::parse("x"), None);
        assert_eq!(SyncCursor::parse("1.0.2"), None);
        assert_eq!(SyncCursor::parse("1.2"), None);
    }

    #[test]
    fn tombstone_keys_are_camel_case() {
        let t = tombstone(
            5,
            "liked_tracks",
            json!({ "user_id": "u", "track_id": "t" }),
            Utc::now(),
        )
        .unwrap();
        assert_eq!(t.entity, SyncEntity::LikedTracks);
        assert_eq!(t.key, json!({ "trackId": "t" }));
        let t = tombstone(
            6,
            "blocked_items",
            json!({ "kind": "track", "ref_id": "x" }),
            Utc::now(),
        )
        .unwrap();
        assert_eq!(t.key, json!({ "kind": "track", "refId": "x" }));
        assert!(tombstone(7, "users", json!({}), Utc::now()).is_none());
    }
}
