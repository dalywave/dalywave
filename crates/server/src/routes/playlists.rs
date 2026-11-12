//! Плейлисты: создание, правка, позиции треков. Позиция дробная: вставка и перестановка меняют одну
//! строку (середина между соседями). Когда соседи сходятся ближе [`MIN_GAP`], позиции перенумеровываются.

use std::collections::HashMap;

use axum::{
    Router,
    body::Bytes,
    extract::{DefaultBodyLimit, State},
    http::StatusCode,
    routing::{get, post, put},
};
use dalywave_protocol::{
    ErrorBody,
    library::{
        AddPlaylistItems, AddedPlaylistItems, DuplicatePlaylist, MovePlaylistItem,
        MovedPlaylistItem, NewPlaylist, PlaylistDetail, PlaylistItem, PlaylistKind, PlaylistList,
        PlaylistPatch, PlaylistSummary, RemovePlaylistItems,
    },
    realtime::{LibraryChange, ServerEvent},
    sync::SyncEntity,
};
use sqlx::{PgConnection, PgPool};
use uuid::Uuid;

use crate::{
    auth::Principal,
    catalog::views::{self, ImageKind, image_set},
    error::{AppError, AppResult, bad},
    extract::{Json, JsonOrDefault, Path},
    images,
    realtime::{self, Audience},
    state::AppState,
    storage::{IMAGE_SIZES, IMMUTABLE, keys},
};

/// Шаг между позициями при добавлении в конец и перенумерации.
pub const STEP: f64 = 1024.0;
/// Ближе — места для вставки нет, позиции плейлиста перенумеровываются.
pub const MIN_GAP: f64 = 1e-6;
pub const MAX_ADD: usize = 500;
/// Своя обложка: фото с телефона бывает крупным, но не больше этого.
pub const MAX_COVER_BYTES: usize = 15 * 1024 * 1024;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/playlists", get(list).post(create))
        .route("/playlists/{id}", get(detail).patch(update).delete(remove))
        .route("/playlists/{id}/pin", put(pin).delete(unpin))
        .route(
            "/playlists/{id}/cover",
            put(set_cover)
                .delete(remove_cover)
                .layer(DefaultBodyLimit::max(MAX_COVER_BYTES)),
        )
        .route("/playlists/{id}/duplicate", post(duplicate))
        .route("/playlists/{id}/items", post(add_items))
        .route("/playlists/{id}/items/remove", post(remove_items))
        .route(
            "/playlists/{id}/items/{itemId}",
            axum::routing::delete(remove_item),
        )
        .route("/playlists/{id}/items/{itemId}/move", post(move_item))
}

/// Позиции для `n` новых элементов между соседями. `None` — места нет, нужна перенумерация.
pub fn positions_between(before: Option<f64>, after: Option<f64>, n: usize) -> Option<Vec<f64>> {
    match (before, after) {
        (None, None) => Some((1..=n).map(|k| k as f64 * STEP).collect()),
        (Some(a), None) => Some((1..=n).map(|k| a + k as f64 * STEP).collect()),
        (None, Some(b)) => Some((0..n).map(|k| b - (n - k) as f64 * STEP).collect()),
        (Some(a), Some(b)) => {
            let gap = (b - a) / (n as f64 + 1.0);
            if gap.is_nan() || gap < MIN_GAP {
                return None;
            }
            Some((1..=n).map(|k| a + k as f64 * gap).collect())
        }
    }
}

/// Перенумеровывает позиции плейлиста: 1024, 2048, … в текущем порядке.
async fn rebalance(conn: &mut PgConnection, playlist_id: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query!(
        "UPDATE playlist_items i SET position = x.rn * 1024.0
         FROM (SELECT id, row_number() OVER (ORDER BY position, id) AS rn
               FROM playlist_items WHERE playlist_id = $1) x
         WHERE i.id = x.id",
        playlist_id,
    )
    .execute(conn)
    .await?;
    Ok(())
}

/// Плейлист человека под замком: правки одного плейлиста идут по очереди.
async fn lock_owned(conn: &mut PgConnection, user_id: Uuid, id: Uuid) -> AppResult<()> {
    sqlx::query_scalar!(
        "SELECT id FROM playlists WHERE id = $1 AND user_id = $2 FOR UPDATE",
        id,
        user_id
    )
    .fetch_optional(&mut *conn)
    .await?
    .ok_or(AppError::NotFound)?;
    Ok(())
}

/// Как [`lock_owned`], но только для плейлистов, которые человек ведёт сам: миксы собираются
/// рекомендациями и при следующей сборке затёрли бы правку — их сначала сохраняют копией.
async fn lock_editable(conn: &mut PgConnection, user_id: Uuid, id: Uuid) -> AppResult<()> {
    let kind = sqlx::query_scalar!(
        "SELECT kind FROM playlists WHERE id = $1 AND user_id = $2 FOR UPDATE",
        id,
        user_id
    )
    .fetch_optional(&mut *conn)
    .await?
    .ok_or(AppError::NotFound)?;
    if kind != PlaylistKind::User.as_str() {
        return Err(AppError::Conflict {
            code: "read_only",
            message: "Этот плейлист собирается сам — сохраните его как свой, чтобы менять".into(),
        });
    }
    Ok(())
}

async fn touch(conn: &mut PgConnection, id: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query!("UPDATE playlists SET updated_at = now() WHERE id = $1", id)
        .execute(conn)
        .await?;
    Ok(())
}

async fn changed(conn: &mut PgConnection, user_id: Uuid, entities: Vec<SyncEntity>) {
    realtime::publish(
        conn,
        Audience::User(user_id),
        ServerEvent::LibraryChanged(LibraryChange { entities }),
    )
    .await;
}

/// Карточки плейлистов человека: закреплённые первыми, затем недавно изменённые.
pub async fn summaries(
    db: &PgPool,
    user_id: Uuid,
    only: Option<&[Uuid]>,
    pinned_only: bool,
) -> Result<Vec<PlaylistSummary>, sqlx::Error> {
    let rows = sqlx::query!(
        r#"SELECT p.id, p.title, p.description, p.kind, p.pinned, p.cover_key, p.created_at, p.updated_at,
                  p.folder_id, p.position, p.rules ->> 'generator' AS generator,
                  (SELECT max(e.started_at) FROM play_events e
                   WHERE e.user_id = p.user_id AND e.context_type IN ('playlist', 'mix')
                     AND e.context_id = p.id::text) AS played_at,
                  (SELECT count(*) FROM playlist_items i WHERE i.playlist_id = p.id) AS "tracks_count!",
                  (SELECT COALESCE(sum(t.duration_ms), 0) FROM playlist_items i JOIN tracks t ON t.id = i.track_id
                   WHERE i.playlist_id = p.id)::bigint AS "duration_ms!",
                  ARRAY(SELECT c.album_id FROM (
                      SELECT t.album_id, min(i.position) AS first
                      FROM playlist_items i JOIN tracks t ON t.id = i.track_id
                      WHERE i.playlist_id = p.id AND t.album_id IS NOT NULL
                      GROUP BY t.album_id ORDER BY first LIMIT 4) c
                    ORDER BY c.first) AS "cover_albums!"
           FROM playlists p
           WHERE p.user_id = $1 AND ($2::uuid[] IS NULL OR p.id = ANY($2)) AND (NOT $3 OR p.pinned)
           ORDER BY p.pinned DESC, p.updated_at DESC, p.id DESC"#,
        user_id,
        only as Option<&[Uuid]>,
        pinned_only,
    )
    .fetch_all(db)
    .await?;
    let album_ids: Vec<Uuid> = rows.iter().flat_map(|r| r.cover_albums.clone()).collect();
    let covers: HashMap<Uuid, (Option<String>, Option<String>)> = sqlx::query!(
        "SELECT id, cover_key, cover_source FROM albums WHERE id = ANY($1)",
        &album_ids
    )
    .fetch_all(db)
    .await?
    .into_iter()
    .map(|r| (r.id, (r.cover_key, r.cover_source)))
    .collect();
    Ok(rows
        .into_iter()
        .map(|r| PlaylistSummary {
            covers: r
                .cover_albums
                .iter()
                .filter_map(|id| {
                    let (key, source) = covers.get(id)?;
                    image_set(ImageKind::Albums, *id, key.as_deref(), source.as_deref())
                })
                .collect(),
            image: image_set(ImageKind::Playlists, r.id, r.cover_key.as_deref(), None),
            id: r.id,
            title: r.title,
            description: r.description,
            kind: r.kind.parse().unwrap_or(PlaylistKind::User),
            pinned: r.pinned,
            tracks_count: r.tracks_count,
            duration_ms: r.duration_ms,
            created_at: r.created_at,
            updated_at: r.updated_at,
            folder_id: r.folder_id,
            position: r.position,
            played_at: r.played_at,
            generator: r.generator,
        })
        .collect())
}

async fn summary(db: &PgPool, user_id: Uuid, id: Uuid) -> AppResult<PlaylistSummary> {
    summaries(db, user_id, Some(&[id]), false)
        .await?
        .into_iter()
        .next()
        .ok_or(AppError::NotFound)
}

async fn items(db: &PgPool, user_id: Uuid, ids: &[Uuid]) -> AppResult<Vec<PlaylistItem>> {
    let rows = sqlx::query!(
        "SELECT id, track_id, position, added_at FROM playlist_items WHERE id = ANY($1) ORDER BY position, id",
        ids
    )
    .fetch_all(db)
    .await?;
    let track_ids: Vec<Uuid> = rows.iter().map(|r| r.track_id).collect();
    let tracks: HashMap<Uuid, _> = views::tracks(db, &track_ids, user_id)
        .await?
        .into_iter()
        .map(|t| (t.id, t))
        .collect();
    Ok(rows
        .into_iter()
        .filter_map(|r| {
            Some(PlaylistItem {
                id: r.id,
                position: r.position,
                added_at: r.added_at,
                track: tracks.get(&r.track_id)?.clone(),
            })
        })
        .collect())
}

async fn detail_of(db: &PgPool, user_id: Uuid, id: Uuid) -> AppResult<PlaylistDetail> {
    let playlist = summary(db, user_id, id).await?;
    let ids = sqlx::query_scalar!(
        "SELECT id FROM playlist_items WHERE playlist_id = $1 ORDER BY position, id",
        id
    )
    .fetch_all(db)
    .await?;
    Ok(PlaylistDetail {
        playlist,
        items: items(db, user_id, &ids).await?,
    })
}

fn clean_title(title: &str) -> AppResult<String> {
    let title = title.trim();
    if title.is_empty() || title.chars().count() > 200 {
        return Err(bad("название плейлиста — от 1 до 200 символов"));
    }
    Ok(title.to_string())
}

fn clean_description(description: &str) -> AppResult<String> {
    let description = description.trim();
    if description.chars().count() > 2000 {
        return Err(bad("описание — не длиннее 2000 символов"));
    }
    Ok(description.to_string())
}

async fn check_tracks(conn: &mut PgConnection, ids: &[Uuid]) -> AppResult<()> {
    if ids.len() > MAX_ADD {
        return Err(bad(format!("не больше {MAX_ADD} треков за раз")));
    }
    let known = sqlx::query_scalar!(
        r#"SELECT count(DISTINCT id) AS "n!" FROM tracks WHERE id = ANY($1)"#,
        ids
    )
    .fetch_one(&mut *conn)
    .await?;
    let mut unique = ids.to_vec();
    unique.sort();
    unique.dedup();
    if known != unique.len() as i64 {
        return Err(bad("в списке есть неизвестные треки"));
    }
    Ok(())
}

async fn insert_items(
    conn: &mut PgConnection,
    playlist_id: Uuid,
    track_ids: &[Uuid],
    positions: &[f64],
) -> Result<Vec<Uuid>, sqlx::Error> {
    sqlx::query_scalar!(
        "INSERT INTO playlist_items (playlist_id, track_id, position)
         SELECT $1, t, p FROM unnest($2::uuid[], $3::float8[]) AS x(t, p)
         RETURNING id",
        playlist_id,
        track_ids,
        positions,
    )
    .fetch_all(conn)
    .await
}

#[utoipa::path(
    get,
    path = "/api/playlists",
    tag = "library",
    security(("session" = []), ("device" = [])),
    responses((status = 200, description = "Плейлисты (закреплённые, затем недавно изменённые) и папки", body = PlaylistList)),
)]
pub async fn list(
    State(st): State<AppState>,
    principal: Principal,
) -> AppResult<Json<PlaylistList>> {
    Ok(Json(PlaylistList {
        items: summaries(&st.db, principal.user.id, None, false).await?,
        folders: super::playlist_folders::folders(&st.db, principal.user.id).await?,
    }))
}

#[utoipa::path(
    post,
    path = "/api/playlists",
    tag = "library",
    security(("session" = []), ("device" = [])),
    request_body = NewPlaylist,
    responses(
        (status = 201, description = "Плейлист создан", body = PlaylistDetail),
        (status = 400, description = "Пустое название или неизвестные треки", body = ErrorBody),
    ),
)]
pub async fn create(
    State(st): State<AppState>,
    principal: Principal,
    Json(body): Json<NewPlaylist>,
) -> AppResult<(StatusCode, Json<PlaylistDetail>)> {
    let user = principal.user.id;
    let title = clean_title(&body.title)?;
    let description = clean_description(body.description.as_deref().unwrap_or(""))?;
    let mut tx = st.db.begin().await?;
    check_tracks(&mut tx, &body.track_ids).await?;
    let id = sqlx::query_scalar!(
        "INSERT INTO playlists (user_id, title, description) VALUES ($1, $2, $3) RETURNING id",
        user,
        title,
        description,
    )
    .fetch_one(&mut *tx)
    .await?;
    if let Some(positions) = positions_between(None, None, body.track_ids.len()) {
        insert_items(&mut tx, id, &body.track_ids, &positions).await?;
    }
    changed(
        &mut tx,
        user,
        vec![SyncEntity::Playlists, SyncEntity::PlaylistItems],
    )
    .await;
    tx.commit().await?;
    Ok((
        StatusCode::CREATED,
        Json(detail_of(&st.db, user, id).await?),
    ))
}

#[utoipa::path(
    get,
    path = "/api/playlists/{id}",
    tag = "library",
    security(("session" = []), ("device" = [])),
    params(("id" = Uuid, Path, description = "id плейлиста")),
    responses(
        (status = 200, description = "Плейлист и треки по порядку", body = PlaylistDetail),
        (status = 404, description = "Нет такого плейлиста", body = ErrorBody),
    ),
)]
pub async fn detail(
    State(st): State<AppState>,
    principal: Principal,
    Path(id): Path<Uuid>,
) -> AppResult<Json<PlaylistDetail>> {
    Ok(Json(detail_of(&st.db, principal.user.id, id).await?))
}

#[utoipa::path(
    patch,
    path = "/api/playlists/{id}",
    tag = "library",
    security(("session" = []), ("device" = [])),
    params(("id" = Uuid, Path, description = "id плейлиста")),
    request_body = PlaylistPatch,
    responses(
        (status = 200, description = "Плейлист после изменения", body = PlaylistSummary),
        (status = 404, description = "Нет такого плейлиста", body = ErrorBody),
    ),
)]
pub async fn update(
    State(st): State<AppState>,
    principal: Principal,
    Path(id): Path<Uuid>,
    Json(patch): Json<PlaylistPatch>,
) -> AppResult<Json<PlaylistSummary>> {
    let user = principal.user.id;
    let title = patch.title.as_deref().map(clean_title).transpose()?;
    let description = patch
        .description
        .as_deref()
        .map(clean_description)
        .transpose()?;
    let mut tx = st.db.begin().await?;
    lock_editable(&mut tx, user, id).await?;
    sqlx::query!(
        "UPDATE playlists SET title = COALESCE($2, title), description = COALESCE($3, description),
                updated_at = now()
         WHERE id = $1",
        id,
        title,
        description,
    )
    .execute(&mut *tx)
    .await?;
    changed(&mut tx, user, vec![SyncEntity::Playlists]).await;
    tx.commit().await?;
    Ok(Json(summary(&st.db, user, id).await?))
}

#[utoipa::path(
    delete,
    path = "/api/playlists/{id}",
    tag = "library",
    security(("session" = []), ("device" = [])),
    params(("id" = Uuid, Path, description = "id плейлиста")),
    responses(
        (status = 204, description = "Плейлист удалён вместе с позициями"),
        (status = 404, description = "Нет такого плейлиста", body = ErrorBody),
    ),
)]
pub async fn remove(
    State(st): State<AppState>,
    principal: Principal,
    Path(id): Path<Uuid>,
) -> AppResult<StatusCode> {
    let user = principal.user.id;
    let mut tx = st.db.begin().await?;
    let deleted = sqlx::query!(
        "DELETE FROM playlists WHERE id = $1 AND user_id = $2",
        id,
        user
    )
    .execute(&mut *tx)
    .await?;
    if deleted.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }
    changed(
        &mut tx,
        user,
        vec![SyncEntity::Playlists, SyncEntity::PlaylistItems],
    )
    .await;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    put,
    path = "/api/playlists/{id}/cover",
    tag = "library",
    security(("session" = []), ("device" = [])),
    params(("id" = Uuid, Path, description = "id плейлиста")),
    request_body(content = Vec<u8>, content_type = "image/jpeg", description = "Картинка: JPEG, PNG или WebP, до 15 МБ. Обрезается по центру до квадрата"),
    responses(
        (status = 200, description = "Плейлист со своей обложкой", body = PlaylistSummary),
        (status = 400, description = "Это не картинка", body = ErrorBody),
        (status = 404, description = "Нет такого плейлиста", body = ErrorBody),
        (status = 409, description = "Плейлист собирается сам (микс)", body = ErrorBody),
        (status = 413, description = "Файл больше 15 МБ", body = ErrorBody),
    ),
)]
pub async fn set_cover(
    State(st): State<AppState>,
    principal: Principal,
    Path(id): Path<Uuid>,
    body: Bytes,
) -> AppResult<Json<PlaylistSummary>> {
    let user = principal.user.id;
    // Сначала — есть ли что менять, чтобы не обрабатывать картинку зря
    {
        let mut tx = st.db.begin().await?;
        lock_editable(&mut tx, user, id).await?;
    }
    if body.is_empty() {
        return Err(bad("файл пустой"));
    }
    let processed = tokio::task::spawn_blocking(move || images::process(&body, false))
        .await
        .map_err(|e| AppError::Internal(e.into()))?
        .map_err(|_| bad("это не картинка: подойдут JPEG, PNG или WebP"))?;
    let prefix = keys::playlist_cover(id, Uuid::now_v7());
    for (size, jpeg) in processed.variants {
        st.storage
            .put(
                &keys::image(&prefix, size),
                jpeg,
                "image/jpeg",
                Some(IMMUTABLE),
            )
            .await
            .map_err(|e| AppError::Unavailable(format!("хранилище не приняло картинку: {e}")))?;
    }
    let mut tx = st.db.begin().await?;
    lock_editable(&mut tx, user, id).await?;
    let old = sqlx::query_scalar!(
        "UPDATE playlists p SET cover_key = $2, updated_at = now()
         FROM (SELECT cover_key FROM playlists WHERE id = $1) old
         WHERE p.id = $1 RETURNING old.cover_key",
        id,
        prefix,
    )
    .fetch_one(&mut *tx)
    .await?;
    changed(&mut tx, user, vec![SyncEntity::Playlists]).await;
    tx.commit().await?;
    if let Some(old) = old {
        drop_cover(&st, &old).await;
    }
    Ok(Json(summary(&st.db, user, id).await?))
}

#[utoipa::path(
    delete,
    path = "/api/playlists/{id}/cover",
    tag = "library",
    security(("session" = []), ("device" = [])),
    params(("id" = Uuid, Path, description = "id плейлиста")),
    responses(
        (status = 200, description = "Своей обложки больше нет — снова мозаика", body = PlaylistSummary),
        (status = 404, description = "Нет такого плейлиста", body = ErrorBody),
        (status = 409, description = "Плейлист собирается сам (микс)", body = ErrorBody),
    ),
)]
pub async fn remove_cover(
    State(st): State<AppState>,
    principal: Principal,
    Path(id): Path<Uuid>,
) -> AppResult<Json<PlaylistSummary>> {
    let user = principal.user.id;
    let mut tx = st.db.begin().await?;
    lock_editable(&mut tx, user, id).await?;
    let old = sqlx::query_scalar!(
        "UPDATE playlists p SET cover_key = NULL, updated_at = now()
         FROM (SELECT cover_key FROM playlists WHERE id = $1) old
         WHERE p.id = $1 RETURNING old.cover_key",
        id,
    )
    .fetch_one(&mut *tx)
    .await?;
    changed(&mut tx, user, vec![SyncEntity::Playlists]).await;
    tx.commit().await?;
    if let Some(old) = old {
        drop_cover(&st, &old).await;
    }
    Ok(Json(summary(&st.db, user, id).await?))
}

/// Копии старой обложки больше не нужны; не удалились — останутся мусором, ссылок на них нет.
async fn drop_cover(st: &AppState, prefix: &str) {
    for size in IMAGE_SIZES {
        if let Err(e) = st.storage.delete(&keys::image(prefix, size)).await {
            tracing::warn!(prefix, error = %e, "could not delete an old playlist cover");
        }
    }
}

/// Копирует файлы своей обложки в папку новой загрузки: у копии плейлиста они свои.
async fn copy_cover(st: &AppState, from: &str, to: &str) -> AppResult<()> {
    for size in IMAGE_SIZES {
        let bytes = st
            .storage
            .get_bytes(&keys::image(from, size))
            .await
            .map_err(|e| AppError::Unavailable(format!("хранилище не отдало обложку: {e}")))?;
        st.storage
            .put(&keys::image(to, size), bytes, "image/jpeg", Some(IMMUTABLE))
            .await
            .map_err(|e| AppError::Unavailable(format!("хранилище не приняло обложку: {e}")))?;
    }
    Ok(())
}

#[utoipa::path(
    post,
    path = "/api/playlists/{id}/duplicate",
    tag = "library",
    security(("session" = []), ("device" = [])),
    params(("id" = Uuid, Path, description = "id плейлиста или микса")),
    request_body = DuplicatePlaylist,
    responses(
        (status = 201, description = "Копия — обычный плейлист с теми же треками, описанием и обложкой", body = PlaylistSummary),
        (status = 404, description = "Нет такого плейлиста", body = ErrorBody),
    ),
)]
pub async fn duplicate(
    State(st): State<AppState>,
    principal: Principal,
    Path(id): Path<Uuid>,
    JsonOrDefault(body): JsonOrDefault<DuplicatePlaylist>,
) -> AppResult<(StatusCode, Json<PlaylistSummary>)> {
    let user = principal.user.id;
    let source = sqlx::query!(
        "SELECT title, description, cover_key FROM playlists WHERE id = $1 AND user_id = $2",
        id,
        user
    )
    .fetch_optional(&st.db)
    .await?
    .ok_or(AppError::NotFound)?;
    let title = clean_title(body.title.as_deref().unwrap_or(&source.title))?;
    let new_id = Uuid::now_v7();
    let cover = match source.cover_key {
        Some(from) => {
            let to = keys::playlist_cover(new_id, Uuid::now_v7());
            copy_cover(&st, &from, &to).await?;
            Some(to)
        }
        None => None,
    };
    let mut tx = st.db.begin().await?;
    sqlx::query!(
        "INSERT INTO playlists (id, user_id, title, description, cover_key) VALUES ($1, $2, $3, $4, $5)",
        new_id,
        user,
        title,
        source.description,
        cover,
    )
    .execute(&mut *tx)
    .await?;
    sqlx::query!(
        "INSERT INTO playlist_items (playlist_id, track_id, position)
         SELECT $1, track_id, row_number() OVER (ORDER BY position, id) * 1024.0
         FROM playlist_items WHERE playlist_id = $2",
        new_id,
        id,
    )
    .execute(&mut *tx)
    .await?;
    changed(
        &mut tx,
        user,
        vec![SyncEntity::Playlists, SyncEntity::PlaylistItems],
    )
    .await;
    tx.commit().await?;
    Ok((
        StatusCode::CREATED,
        Json(summary(&st.db, user, new_id).await?),
    ))
}

async fn set_pinned(st: &AppState, user: Uuid, id: Uuid, pinned: bool) -> AppResult<StatusCode> {
    let mut tx = st.db.begin().await?;
    lock_owned(&mut tx, user, id).await?;
    sqlx::query!("UPDATE playlists SET pinned = $2 WHERE id = $1", id, pinned)
        .execute(&mut *tx)
        .await?;
    changed(&mut tx, user, vec![SyncEntity::Playlists]).await;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    put,
    path = "/api/playlists/{id}/pin",
    tag = "library",
    security(("session" = []), ("device" = [])),
    params(("id" = Uuid, Path, description = "id плейлиста")),
    responses(
        (status = 204, description = "Закреплён в медиатеке"),
        (status = 404, description = "Нет такого плейлиста", body = ErrorBody),
    ),
)]
pub async fn pin(
    State(st): State<AppState>,
    principal: Principal,
    Path(id): Path<Uuid>,
) -> AppResult<StatusCode> {
    set_pinned(&st, principal.user.id, id, true).await
}

#[utoipa::path(
    delete,
    path = "/api/playlists/{id}/pin",
    tag = "library",
    security(("session" = []), ("device" = [])),
    params(("id" = Uuid, Path, description = "id плейлиста")),
    responses(
        (status = 204, description = "Больше не закреплён"),
        (status = 404, description = "Нет такого плейлиста", body = ErrorBody),
    ),
)]
pub async fn unpin(
    State(st): State<AppState>,
    principal: Principal,
    Path(id): Path<Uuid>,
) -> AppResult<StatusCode> {
    set_pinned(&st, principal.user.id, id, false).await
}

/// Соседи для вставки: позиция `after` (или `None` — в начало) и следующая за ней, без строки `skip`.
async fn neighbours(
    conn: &mut PgConnection,
    playlist_id: Uuid,
    after: Option<Uuid>,
    skip: Option<Uuid>,
) -> AppResult<(Option<f64>, Option<f64>)> {
    let before = match after {
        Some(item) => Some(
            sqlx::query_scalar!(
                "SELECT position FROM playlist_items WHERE id = $1 AND playlist_id = $2",
                item,
                playlist_id
            )
            .fetch_optional(&mut *conn)
            .await?
            .ok_or_else(|| bad("нет такой позиции в плейлисте"))?,
        ),
        None => None,
    };
    let next = sqlx::query_scalar!(
        r#"SELECT min(position) AS "position" FROM playlist_items
           WHERE playlist_id = $1 AND ($2::float8 IS NULL OR position > $2)
             AND ($3::uuid IS NULL OR id <> $3)"#,
        playlist_id,
        before,
        skip,
    )
    .fetch_one(&mut *conn)
    .await?;
    Ok((before, next))
}

#[utoipa::path(
    post,
    path = "/api/playlists/{id}/items",
    tag = "library",
    security(("session" = []), ("device" = [])),
    params(("id" = Uuid, Path, description = "id плейлиста")),
    request_body = AddPlaylistItems,
    responses(
        (status = 200, description = "Добавленные позиции", body = AddedPlaylistItems),
        (status = 400, description = "Неизвестные треки или позиция", body = ErrorBody),
        (status = 404, description = "Нет такого плейлиста", body = ErrorBody),
    ),
)]
pub async fn add_items(
    State(st): State<AppState>,
    principal: Principal,
    Path(id): Path<Uuid>,
    Json(body): Json<AddPlaylistItems>,
) -> AppResult<Json<AddedPlaylistItems>> {
    let user = principal.user.id;
    if body.track_ids.is_empty() {
        return Err(bad("нет треков для добавления"));
    }
    let mut tx = st.db.begin().await?;
    lock_editable(&mut tx, user, id).await?;
    check_tracks(&mut tx, &body.track_ids).await?;
    let n = body.track_ids.len();
    let find = async |tx: &mut PgConnection| -> AppResult<(Option<f64>, Option<f64>)> {
        if let Some(after) = body.after_item_id {
            neighbours(tx, id, Some(after), None).await
        } else if body.at_start {
            neighbours(tx, id, None, None)
                .await
                .map(|(_, next)| (None, next))
        } else {
            let last = sqlx::query_scalar!(
                r#"SELECT max(position) AS "position" FROM playlist_items WHERE playlist_id = $1"#,
                id
            )
            .fetch_one(&mut *tx)
            .await?;
            Ok((last, None))
        }
    };
    let (before, after) = find(&mut tx).await?;
    let mut rebalanced = false;
    let positions = match positions_between(before, after, n) {
        Some(p) => p,
        None => {
            rebalance(&mut tx, id).await?;
            rebalanced = true;
            let (before, after) = find(&mut tx).await?;
            positions_between(before, after, n)
                .ok_or_else(|| AppError::Internal(anyhow::anyhow!("no room after rebalance")))?
        }
    };
    let ids = insert_items(&mut tx, id, &body.track_ids, &positions).await?;
    touch(&mut tx, id).await?;
    changed(
        &mut tx,
        user,
        vec![SyncEntity::Playlists, SyncEntity::PlaylistItems],
    )
    .await;
    tx.commit().await?;
    Ok(Json(AddedPlaylistItems {
        items: items(&st.db, user, &ids).await?,
        rebalanced,
    }))
}

#[utoipa::path(
    delete,
    path = "/api/playlists/{id}/items/{itemId}",
    tag = "library",
    security(("session" = []), ("device" = [])),
    params(
        ("id" = Uuid, Path, description = "id плейлиста"),
        ("itemId" = Uuid, Path, description = "id позиции"),
    ),
    responses(
        (status = 204, description = "Позиция удалена"),
        (status = 404, description = "Нет такого плейлиста или позиции", body = ErrorBody),
    ),
)]
pub async fn remove_item(
    State(st): State<AppState>,
    principal: Principal,
    Path((id, item_id)): Path<(Uuid, Uuid)>,
) -> AppResult<StatusCode> {
    let user = principal.user.id;
    let mut tx = st.db.begin().await?;
    lock_editable(&mut tx, user, id).await?;
    let deleted = sqlx::query!(
        "DELETE FROM playlist_items WHERE id = $1 AND playlist_id = $2",
        item_id,
        id
    )
    .execute(&mut *tx)
    .await?;
    if deleted.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }
    touch(&mut tx, id).await?;
    changed(
        &mut tx,
        user,
        vec![SyncEntity::Playlists, SyncEntity::PlaylistItems],
    )
    .await;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    post,
    path = "/api/playlists/{id}/items/remove",
    tag = "library",
    security(("session" = []), ("device" = [])),
    params(("id" = Uuid, Path, description = "id плейлиста")),
    request_body = RemovePlaylistItems,
    responses(
        (status = 204, description = "Позиции убраны; чужих и уже убранных среди них не было или они пропущены"),
        (status = 400, description = "Пустой список или больше 500 позиций", body = ErrorBody),
        (status = 404, description = "Нет такого плейлиста", body = ErrorBody),
        (status = 409, description = "Плейлист собирается сам (микс)", body = ErrorBody),
    ),
)]
pub async fn remove_items(
    State(st): State<AppState>,
    principal: Principal,
    Path(id): Path<Uuid>,
    Json(body): Json<RemovePlaylistItems>,
) -> AppResult<StatusCode> {
    let user = principal.user.id;
    if body.item_ids.is_empty() || body.item_ids.len() > MAX_ADD {
        return Err(bad(format!("от 1 до {MAX_ADD} позиций за раз")));
    }
    let mut tx = st.db.begin().await?;
    lock_editable(&mut tx, user, id).await?;
    sqlx::query!(
        "DELETE FROM playlist_items WHERE playlist_id = $1 AND id = ANY($2)",
        id,
        &body.item_ids
    )
    .execute(&mut *tx)
    .await?;
    touch(&mut tx, id).await?;
    changed(
        &mut tx,
        user,
        vec![SyncEntity::Playlists, SyncEntity::PlaylistItems],
    )
    .await;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    post,
    path = "/api/playlists/{id}/items/{itemId}/move",
    tag = "library",
    security(("session" = []), ("device" = [])),
    params(
        ("id" = Uuid, Path, description = "id плейлиста"),
        ("itemId" = Uuid, Path, description = "id позиции"),
    ),
    request_body = MovePlaylistItem,
    responses(
        (status = 200, description = "Новая позиция", body = MovedPlaylistItem),
        (status = 400, description = "Нет позиции `afterItemId`", body = ErrorBody),
        (status = 404, description = "Нет такого плейлиста или позиции", body = ErrorBody),
    ),
)]
pub async fn move_item(
    State(st): State<AppState>,
    principal: Principal,
    Path((id, item_id)): Path<(Uuid, Uuid)>,
    JsonOrDefault(body): JsonOrDefault<MovePlaylistItem>,
) -> AppResult<Json<MovedPlaylistItem>> {
    let user = principal.user.id;
    let mut tx = st.db.begin().await?;
    lock_editable(&mut tx, user, id).await?;
    let current = sqlx::query_scalar!(
        "SELECT position FROM playlist_items WHERE id = $1 AND playlist_id = $2",
        item_id,
        id
    )
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(AppError::NotFound)?;
    if body.after_item_id == Some(item_id) {
        return Ok(Json(MovedPlaylistItem {
            id: item_id,
            position: current,
            rebalanced: false,
        }));
    }
    let (before, after) = neighbours(&mut tx, id, body.after_item_id, Some(item_id)).await?;
    let mut rebalanced = false;
    let position = match positions_between(before, after, 1) {
        Some(p) => p[0],
        None => {
            rebalance(&mut tx, id).await?;
            rebalanced = true;
            let (before, after) =
                neighbours(&mut tx, id, body.after_item_id, Some(item_id)).await?;
            positions_between(before, after, 1)
                .map(|p| p[0])
                .ok_or_else(|| AppError::Internal(anyhow::anyhow!("no room after rebalance")))?
        }
    };
    sqlx::query!(
        "UPDATE playlist_items SET position = $2 WHERE id = $1",
        item_id,
        position
    )
    .execute(&mut *tx)
    .await?;
    touch(&mut tx, id).await?;
    changed(
        &mut tx,
        user,
        vec![SyncEntity::Playlists, SyncEntity::PlaylistItems],
    )
    .await;
    tx.commit().await?;
    Ok(Json(MovedPlaylistItem {
        id: item_id,
        position,
        rebalanced,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn positions_fill_gaps() {
        assert_eq!(positions_between(None, None, 2), Some(vec![1024.0, 2048.0]));
        assert_eq!(positions_between(Some(1024.0), None, 1), Some(vec![2048.0]));
        assert_eq!(
            positions_between(None, Some(1024.0), 2),
            Some(vec![-1024.0, 0.0])
        );
        assert_eq!(
            positions_between(Some(0.0), Some(3.0), 2),
            Some(vec![1.0, 2.0])
        );
        assert_eq!(positions_between(Some(1.0), Some(1.0 + 1e-7), 1), None);
        assert_eq!(positions_between(Some(2.0), Some(1.0), 1), None);
    }

    #[test]
    fn repeated_inserts_run_out_of_room() {
        // Вставка всё время в одно место делит промежуток пополам: примерно через 30 раз нужна перенумерация
        let (a, mut b) = (1024.0, 2048.0);
        let mut inserts = 0;
        while let Some(p) = positions_between(Some(a), Some(b), 1) {
            assert!(p[0] > a && p[0] < b);
            b = p[0];
            inserts += 1;
        }
        assert!((25..=40).contains(&inserts), "{inserts}");
    }
}
