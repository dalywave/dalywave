//! Медиатека: лайки, сохранённые альбомы, подписки на артистов, «не рекомендовать». Каждое изменение
//! рассылает `library.changed` — другие устройства запрашивают `/api/sync`. Медиатека живёт на сервере:
//! лайк и сохранённый альбом ставят треки на скачивание (полоса «Медиатека»), подписка — дискографию.

use std::collections::HashMap;

use axum::{
    Router,
    extract::State,
    http::StatusCode,
    routing::{get, put},
};
use chrono::{DateTime, Utc};
use dalywave_protocol::{
    ErrorBody,
    jobs::{AcquireReason, SyncDiscographyPayload, kind},
    library::{
        BlockedItem, BlockedKind, BlockedList, DownloadPolicy, FollowRequest, FollowedArtist,
        FollowedArtistPage, LibraryCounts, LibraryOverview, LikedTrack, LikedTrackPage, PageQuery,
        SavedAlbum, SavedAlbumPage,
    },
    realtime::{LibraryChange, ServerEvent},
    sync::SyncEntity,
};
use serde_json::Value;
use sqlx::{PgConnection, PgPool};
use uuid::Uuid;

use crate::{
    acquire,
    auth::Principal,
    catalog::{self, views},
    cursor,
    error::{AppError, AppResult},
    extract::{Json, JsonOrDefault, Path, Query},
    jobs::{self, NewJob, lanes},
    realtime::{self, Audience},
    reco,
    routes::playlists,
    state::AppState,
};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/library", get(overview))
        .route("/library/tracks", get(liked_tracks))
        .route("/library/tracks/{id}", put(like).delete(unlike))
        .route("/library/albums", get(saved_albums))
        .route("/library/albums/{id}", put(save_album).delete(unsave_album))
        .route("/library/artists", get(followed_artists))
        .route("/library/artists/{id}", put(follow).delete(unfollow))
        .route("/library/blocked", get(blocked))
        .route("/library/blocked/{kind}/{id}", put(block).delete(unblock))
}

/// Ставит треки медиатеки на скачивание. Сбой постановки не отменяет лайк: он пишется в лог,
/// а трек скачается при первом запуске.
async fn acquire_tracks(st: &AppState, ids: &[Uuid], reason: AcquireReason, user: Uuid) {
    for id in ids {
        let req = acquire::Request::new(*id, reason, lanes::LIBRARY).by(user);
        if let Err(e) = acquire::request(&st.db, &st.deezer, req).await {
            tracing::warn!(track = %id, error = %e, "could not queue a library track for download");
        }
    }
}

async fn changed(conn: &mut PgConnection, user_id: Uuid, entities: Vec<SyncEntity>) {
    realtime::publish(
        conn,
        Audience::User(user_id),
        ServerEvent::LibraryChanged(LibraryChange { entities }),
    )
    .await;
}

async fn exists(db: &PgPool, table: &str, id: Uuid) -> Result<bool, sqlx::Error> {
    let found = match table {
        "tracks" => {
            sqlx::query_scalar!(
                r#"SELECT EXISTS (SELECT 1 FROM tracks WHERE id = $1) AS "e!""#,
                id
            )
            .fetch_one(db)
            .await?
        }
        "albums" => {
            sqlx::query_scalar!(
                r#"SELECT EXISTS (SELECT 1 FROM albums WHERE id = $1) AS "e!""#,
                id
            )
            .fetch_one(db)
            .await?
        }
        _ => {
            sqlx::query_scalar!(
                r#"SELECT EXISTS (SELECT 1 FROM artists WHERE id = $1) AS "e!""#,
                id
            )
            .fetch_one(db)
            .await?
        }
    };
    Ok(found)
}

struct FollowRow {
    artist_id: Uuid,
    download_policy: Value,
    notify: bool,
    created_at: DateTime<Utc>,
}

async fn followed_views(
    db: &PgPool,
    user_id: Uuid,
    rows: Vec<FollowRow>,
) -> AppResult<Vec<FollowedArtist>> {
    let ids: Vec<Uuid> = rows.iter().map(|r| r.artist_id).collect();
    let artists: HashMap<Uuid, _> = views::artists(db, &ids, user_id)
        .await?
        .into_iter()
        .map(|a| (a.id, a))
        .collect();
    let progress = views::discography_progress(db, &ids).await?;
    Ok(rows
        .into_iter()
        .filter_map(|r| {
            let (tracks_on_server, tracks_total) =
                progress.get(&r.artist_id).copied().unwrap_or_default();
            Some(FollowedArtist {
                artist: artists.get(&r.artist_id)?.clone(),
                policy: serde_json::from_value(r.download_policy).unwrap_or_default(),
                notify: r.notify,
                followed_at: r.created_at,
                tracks_on_server,
                tracks_total,
            })
        })
        .collect())
}

#[utoipa::path(
    get,
    path = "/api/library",
    tag = "library",
    security(("session" = []), ("device" = [])),
    responses((status = 200, description = "Счётчики, закреплённые плейлисты, подписки", body = LibraryOverview)),
)]
pub async fn overview(
    State(st): State<AppState>,
    principal: Principal,
) -> AppResult<Json<LibraryOverview>> {
    let user = principal.user.id;
    let counts = sqlx::query!(
        r#"SELECT
              (SELECT count(*) FROM liked_tracks WHERE user_id = $1) AS "liked!",
              (SELECT count(*) FROM saved_albums WHERE user_id = $1) AS "saved!",
              (SELECT count(*) FROM followed_artists WHERE user_id = $1) AS "followed!",
              (SELECT count(*) FROM playlists WHERE user_id = $1) AS "playlists!",
              (SELECT count(*) FROM tracks t JOIN recordings r ON r.id = t.recording_id
               WHERE r.audio_status = 'ready' AND t.id IN (SELECT user_track_ids($1))) AS "ready!""#,
        user,
    )
    .fetch_one(&st.db)
    .await?;
    let follows = sqlx::query_as!(
        FollowRow,
        "SELECT artist_id, download_policy, notify, created_at FROM followed_artists
         WHERE user_id = $1 ORDER BY created_at DESC, artist_id DESC LIMIT 50",
        user,
    )
    .fetch_all(&st.db)
    .await?;
    Ok(Json(LibraryOverview {
        counts: LibraryCounts {
            liked_tracks: counts.liked,
            saved_albums: counts.saved,
            followed_artists: counts.followed,
            playlists: counts.playlists,
            ready_tracks: counts.ready,
        },
        pinned_playlists: playlists::summaries(&st.db, user, None, true).await?,
        followed_artists: followed_views(&st.db, user, follows).await?,
    }))
}

#[utoipa::path(
    get,
    path = "/api/library/tracks",
    tag = "library",
    security(("session" = []), ("device" = [])),
    params(PageQuery),
    responses(
        (status = 200, description = "Понравившиеся треки, новые первыми", body = LikedTrackPage),
        (status = 400, description = "Неверный cursor или limit", body = ErrorBody),
    ),
)]
pub async fn liked_tracks(
    State(st): State<AppState>,
    principal: Principal,
    Query(q): Query<PageQuery>,
) -> AppResult<Json<LikedTrackPage>> {
    let user = principal.user.id;
    let limit = cursor::limit(q.limit, 50, 200)?;
    let after = cursor::decode_opt(q.cursor.as_deref())?;
    let mut rows = sqlx::query!(
        "SELECT track_id, created_at FROM liked_tracks
         WHERE user_id = $1 AND ($2::timestamptz IS NULL OR (created_at, track_id) < ($2, $3))
         ORDER BY created_at DESC, track_id DESC
         LIMIT $4",
        user,
        after.map(|(at, _)| at),
        after.map(|(_, id)| id).unwrap_or_default(),
        limit + 1,
    )
    .fetch_all(&st.db)
    .await?;
    let next_cursor = cursor::page(&mut rows, limit, |r| (r.created_at, r.track_id));
    let ids: Vec<Uuid> = rows.iter().map(|r| r.track_id).collect();
    let tracks: HashMap<Uuid, _> = views::tracks(&st.db, &ids, user)
        .await?
        .into_iter()
        .map(|t| (t.id, t))
        .collect();
    let items = rows
        .into_iter()
        .filter_map(|r| {
            Some(LikedTrack {
                track: tracks.get(&r.track_id)?.clone(),
                liked_at: r.created_at,
            })
        })
        .collect();
    Ok(Json(LikedTrackPage { items, next_cursor }))
}

#[utoipa::path(
    put,
    path = "/api/library/tracks/{id}",
    tag = "library",
    security(("session" = []), ("device" = [])),
    params(("id" = Uuid, Path, description = "id трека")),
    responses(
        (status = 204, description = "Трек в понравившихся (повтор безопасен) и поставлен на скачивание на сервер"),
        (status = 404, description = "Нет такого трека", body = ErrorBody),
    ),
)]
pub async fn like(
    State(st): State<AppState>,
    principal: Principal,
    Path(id): Path<Uuid>,
) -> AppResult<StatusCode> {
    let user = principal.user.id;
    if !exists(&st.db, "tracks", id).await? {
        return Err(AppError::NotFound);
    }
    let mut tx = st.db.begin().await?;
    let inserted = sqlx::query!(
        "INSERT INTO liked_tracks (user_id, track_id) VALUES ($1, $2) ON CONFLICT DO NOTHING",
        user,
        id
    )
    .execute(&mut *tx)
    .await?;
    if inserted.rows_affected() > 0 {
        changed(&mut tx, user, vec![SyncEntity::LikedTracks]).await;
    }
    tx.commit().await?;
    acquire_tracks(&st, &[id], AcquireReason::Like, user).await;
    // Лайк сдвигает ближайшую грань вкуса сразу, не дожидаясь ночного пересчёта
    if inserted.rows_affected() > 0
        && let Err(e) = reco::taste::on_like(&st.db, user, id).await
    {
        tracing::warn!(track = %id, error = %e, "could not move the taste facet");
    }
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    delete,
    path = "/api/library/tracks/{id}",
    tag = "library",
    security(("session" = []), ("device" = [])),
    params(("id" = Uuid, Path, description = "id трека")),
    responses((status = 204, description = "Трек убран из понравившихся (если был)")),
)]
pub async fn unlike(
    State(st): State<AppState>,
    principal: Principal,
    Path(id): Path<Uuid>,
) -> AppResult<StatusCode> {
    let user = principal.user.id;
    let mut tx = st.db.begin().await?;
    let deleted = sqlx::query!(
        "DELETE FROM liked_tracks WHERE user_id = $1 AND track_id = $2",
        user,
        id
    )
    .execute(&mut *tx)
    .await?;
    if deleted.rows_affected() > 0 {
        changed(&mut tx, user, vec![SyncEntity::LikedTracks]).await;
    }
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    get,
    path = "/api/library/albums",
    tag = "library",
    security(("session" = []), ("device" = [])),
    params(PageQuery),
    responses(
        (status = 200, description = "Сохранённые альбомы, новые первыми", body = SavedAlbumPage),
        (status = 400, description = "Неверный cursor или limit", body = ErrorBody),
    ),
)]
pub async fn saved_albums(
    State(st): State<AppState>,
    principal: Principal,
    Query(q): Query<PageQuery>,
) -> AppResult<Json<SavedAlbumPage>> {
    let user = principal.user.id;
    let limit = cursor::limit(q.limit, 50, 200)?;
    let after = cursor::decode_opt(q.cursor.as_deref())?;
    let mut rows = sqlx::query!(
        "SELECT album_id, created_at FROM saved_albums
         WHERE user_id = $1 AND ($2::timestamptz IS NULL OR (created_at, album_id) < ($2, $3))
         ORDER BY created_at DESC, album_id DESC
         LIMIT $4",
        user,
        after.map(|(at, _)| at),
        after.map(|(_, id)| id).unwrap_or_default(),
        limit + 1,
    )
    .fetch_all(&st.db)
    .await?;
    let next_cursor = cursor::page(&mut rows, limit, |r| (r.created_at, r.album_id));
    let ids: Vec<Uuid> = rows.iter().map(|r| r.album_id).collect();
    let albums: HashMap<Uuid, _> = views::albums(&st.db, &ids, user)
        .await?
        .into_iter()
        .map(|a| (a.id, a))
        .collect();
    let items = rows
        .into_iter()
        .filter_map(|r| {
            Some(SavedAlbum {
                album: albums.get(&r.album_id)?.clone(),
                saved_at: r.created_at,
            })
        })
        .collect();
    Ok(Json(SavedAlbumPage { items, next_cursor }))
}

#[utoipa::path(
    put,
    path = "/api/library/albums/{id}",
    tag = "library",
    security(("session" = []), ("device" = [])),
    params(("id" = Uuid, Path, description = "id альбома")),
    responses(
        (status = 204, description = "Альбом в медиатеке (повтор безопасен), его треки поставлены на скачивание на сервер"),
        (status = 404, description = "Нет такого альбома", body = ErrorBody),
    ),
)]
pub async fn save_album(
    State(st): State<AppState>,
    principal: Principal,
    Path(id): Path<Uuid>,
) -> AppResult<StatusCode> {
    let user = principal.user.id;
    if !exists(&st.db, "albums", id).await? {
        return Err(AppError::NotFound);
    }
    let mut tx = st.db.begin().await?;
    let inserted = sqlx::query!(
        "INSERT INTO saved_albums (user_id, album_id) VALUES ($1, $2) ON CONFLICT DO NOTHING",
        user,
        id
    )
    .execute(&mut *tx)
    .await?;
    if inserted.rows_affected() > 0 {
        changed(&mut tx, user, vec![SyncEntity::SavedAlbums]).await;
    }
    tx.commit().await?;
    // Треки альбома качаются на сервер; их список загружается из Deezer, если его ещё нет
    if let Err(e) = catalog::ensure_album(&st.db, &st.deezer, id).await {
        tracing::warn!(album = %id, error = %e, "could not load the album tracks");
    }
    let tracks = sqlx::query_scalar!(
        "SELECT id FROM tracks WHERE album_id = $1 ORDER BY disc_number, track_position NULLS LAST, id",
        id
    )
    .fetch_all(&st.db)
    .await?;
    acquire_tracks(&st, &tracks, AcquireReason::Album, user).await;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    delete,
    path = "/api/library/albums/{id}",
    tag = "library",
    security(("session" = []), ("device" = [])),
    params(("id" = Uuid, Path, description = "id альбома")),
    responses((status = 204, description = "Альбом убран из медиатеки (если был)")),
)]
pub async fn unsave_album(
    State(st): State<AppState>,
    principal: Principal,
    Path(id): Path<Uuid>,
) -> AppResult<StatusCode> {
    let user = principal.user.id;
    let mut tx = st.db.begin().await?;
    let deleted = sqlx::query!(
        "DELETE FROM saved_albums WHERE user_id = $1 AND album_id = $2",
        user,
        id
    )
    .execute(&mut *tx)
    .await?;
    if deleted.rows_affected() > 0 {
        changed(&mut tx, user, vec![SyncEntity::SavedAlbums]).await;
    }
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    get,
    path = "/api/library/artists",
    tag = "library",
    security(("session" = []), ("device" = [])),
    params(PageQuery),
    responses(
        (status = 200, description = "Подписки с политикой докачки и прогрессом, новые первыми", body = FollowedArtistPage),
        (status = 400, description = "Неверный cursor или limit", body = ErrorBody),
    ),
)]
pub async fn followed_artists(
    State(st): State<AppState>,
    principal: Principal,
    Query(q): Query<PageQuery>,
) -> AppResult<Json<FollowedArtistPage>> {
    let user = principal.user.id;
    let limit = cursor::limit(q.limit, 50, 200)?;
    let after = cursor::decode_opt(q.cursor.as_deref())?;
    let mut rows = sqlx::query_as!(
        FollowRow,
        "SELECT artist_id, download_policy, notify, created_at FROM followed_artists
         WHERE user_id = $1 AND ($2::timestamptz IS NULL OR (created_at, artist_id) < ($2, $3))
         ORDER BY created_at DESC, artist_id DESC
         LIMIT $4",
        user,
        after.map(|(at, _)| at),
        after.map(|(_, id)| id).unwrap_or_default(),
        limit + 1,
    )
    .fetch_all(&st.db)
    .await?;
    let next_cursor = cursor::page(&mut rows, limit, |r| (r.created_at, r.artist_id));
    Ok(Json(FollowedArtistPage {
        items: followed_views(&st.db, user, rows).await?,
        next_cursor,
    }))
}

#[utoipa::path(
    put,
    path = "/api/library/artists/{id}",
    tag = "library",
    security(("session" = []), ("device" = [])),
    params(("id" = Uuid, Path, description = "id артиста")),
    request_body(content = FollowRequest, description = "Можно не передавать: политика по умолчанию"),
    responses(
        (status = 200, description = "Подписка; новая или с другой политикой — ставит сверку дискографии", body = FollowedArtist),
        (status = 404, description = "Нет такого артиста", body = ErrorBody),
    ),
)]
pub async fn follow(
    State(st): State<AppState>,
    principal: Principal,
    Path(id): Path<Uuid>,
    JsonOrDefault(body): JsonOrDefault<FollowRequest>,
) -> AppResult<Json<FollowedArtist>> {
    let user = principal.user.id;
    if !exists(&st.db, "artists", id).await? {
        return Err(AppError::NotFound);
    }
    let mut tx = st.db.begin().await?;
    let before = sqlx::query_scalar!(
        "SELECT download_policy FROM followed_artists WHERE user_id = $1 AND artist_id = $2 FOR UPDATE",
        user,
        id
    )
    .fetch_optional(&mut *tx)
    .await?;
    let old_policy: Option<DownloadPolicy> =
        before.map(|p| serde_json::from_value(p).unwrap_or_default());
    let policy = body.download_policy.or(old_policy).unwrap_or_default();
    let policy_json = serde_json::to_value(policy).map_err(anyhow::Error::from)?;
    sqlx::query!(
        "INSERT INTO followed_artists (user_id, artist_id, download_policy, notify)
         VALUES ($1, $2, $3, COALESCE($4, true))
         ON CONFLICT (user_id, artist_id) DO UPDATE SET
            download_policy = EXCLUDED.download_policy,
            notify = COALESCE($4, followed_artists.notify),
            updated_at = now()",
        user,
        id,
        policy_json,
        body.notify,
    )
    .execute(&mut *tx)
    .await?;
    // Новая подписка или новая политика — сверяем дискографию сразу, а не через сутки
    if old_policy != Some(policy) {
        jobs::enqueue(
            &mut tx,
            NewJob::new(
                kind::SYNC_DISCOGRAPHY,
                lanes::MAINTENANCE,
                SyncDiscographyPayload { artist_id: id },
            )
            .dedupe(format!("sync_discography:{id}"))
            .priority(5),
        )
        .await?;
    }
    changed(&mut tx, user, vec![SyncEntity::FollowedArtists]).await;
    tx.commit().await?;
    let row = sqlx::query_as!(
        FollowRow,
        "SELECT artist_id, download_policy, notify, created_at FROM followed_artists
         WHERE user_id = $1 AND artist_id = $2",
        user,
        id,
    )
    .fetch_one(&st.db)
    .await?;
    followed_views(&st.db, user, vec![row])
        .await?
        .into_iter()
        .next()
        .map(Json)
        .ok_or(AppError::NotFound)
}

#[utoipa::path(
    delete,
    path = "/api/library/artists/{id}",
    tag = "library",
    security(("session" = []), ("device" = [])),
    params(("id" = Uuid, Path, description = "id артиста")),
    responses((status = 204, description = "Подписки больше нет; скачанное остаётся на сервере")),
)]
pub async fn unfollow(
    State(st): State<AppState>,
    principal: Principal,
    Path(id): Path<Uuid>,
) -> AppResult<StatusCode> {
    let user = principal.user.id;
    let mut tx = st.db.begin().await?;
    let deleted = sqlx::query!(
        "DELETE FROM followed_artists WHERE user_id = $1 AND artist_id = $2",
        user,
        id
    )
    .execute(&mut *tx)
    .await?;
    if deleted.rows_affected() > 0 {
        changed(&mut tx, user, vec![SyncEntity::FollowedArtists]).await;
    }
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    get,
    path = "/api/library/blocked",
    tag = "library",
    security(("session" = []), ("device" = [])),
    responses((status = 200, description = "«Не рекомендовать»: треки и артисты, новые первыми", body = BlockedList)),
)]
pub async fn blocked(
    State(st): State<AppState>,
    principal: Principal,
) -> AppResult<Json<BlockedList>> {
    let user = principal.user.id;
    let rows = sqlx::query!(
        "SELECT kind, ref_id, created_at FROM blocked_items WHERE user_id = $1 ORDER BY created_at DESC, ref_id",
        user
    )
    .fetch_all(&st.db)
    .await?;
    let track_ids: Vec<Uuid> = rows
        .iter()
        .filter(|r| r.kind == "track")
        .map(|r| r.ref_id)
        .collect();
    let artist_ids: Vec<Uuid> = rows
        .iter()
        .filter(|r| r.kind == "artist")
        .map(|r| r.ref_id)
        .collect();
    let tracks: HashMap<Uuid, _> = views::tracks(&st.db, &track_ids, user)
        .await?
        .into_iter()
        .map(|t| (t.id, t))
        .collect();
    let artists: HashMap<Uuid, _> = views::artists(&st.db, &artist_ids, user)
        .await?
        .into_iter()
        .map(|a| (a.id, a))
        .collect();
    let items = rows
        .into_iter()
        .map(|r| {
            let kind = r.kind.parse().unwrap_or(BlockedKind::Track);
            BlockedItem {
                track: (kind == BlockedKind::Track)
                    .then(|| tracks.get(&r.ref_id).cloned())
                    .flatten(),
                artist: (kind == BlockedKind::Artist)
                    .then(|| artists.get(&r.ref_id).cloned())
                    .flatten(),
                kind,
                id: r.ref_id,
                created_at: r.created_at,
            }
        })
        .collect();
    Ok(Json(BlockedList { items }))
}

#[utoipa::path(
    put,
    path = "/api/library/blocked/{kind}/{id}",
    tag = "library",
    security(("session" = []), ("device" = [])),
    params(
        ("kind" = BlockedKind, Path, description = "track или artist"),
        ("id" = Uuid, Path, description = "id трека или артиста"),
    ),
    responses(
        (status = 204, description = "Больше не рекомендуется (повтор безопасен)"),
        (status = 404, description = "Нет такого трека или артиста", body = ErrorBody),
    ),
)]
pub async fn block(
    State(st): State<AppState>,
    principal: Principal,
    Path((kind, id)): Path<(BlockedKind, Uuid)>,
) -> AppResult<StatusCode> {
    let user = principal.user.id;
    let table = match kind {
        BlockedKind::Track => "tracks",
        BlockedKind::Artist => "artists",
    };
    if !exists(&st.db, table, id).await? {
        return Err(AppError::NotFound);
    }
    let mut tx = st.db.begin().await?;
    let inserted = sqlx::query!(
        "INSERT INTO blocked_items (user_id, kind, ref_id) VALUES ($1, $2, $3) ON CONFLICT DO NOTHING",
        user,
        kind.as_str(),
        id
    )
    .execute(&mut *tx)
    .await?;
    if inserted.rows_affected() > 0 {
        changed(&mut tx, user, vec![SyncEntity::BlockedItems]).await;
    }
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    delete,
    path = "/api/library/blocked/{kind}/{id}",
    tag = "library",
    security(("session" = []), ("device" = [])),
    params(
        ("kind" = BlockedKind, Path, description = "track или artist"),
        ("id" = Uuid, Path, description = "id трека или артиста"),
    ),
    responses((status = 204, description = "Снова может рекомендоваться")),
)]
pub async fn unblock(
    State(st): State<AppState>,
    principal: Principal,
    Path((kind, id)): Path<(BlockedKind, Uuid)>,
) -> AppResult<StatusCode> {
    let user = principal.user.id;
    let mut tx = st.db.begin().await?;
    let deleted = sqlx::query!(
        "DELETE FROM blocked_items WHERE user_id = $1 AND kind = $2 AND ref_id = $3",
        user,
        kind.as_str(),
        id
    )
    .execute(&mut *tx)
    .await?;
    if deleted.rows_affected() > 0 {
        changed(&mut tx, user, vec![SyncEntity::BlockedItems]).await;
    }
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}
