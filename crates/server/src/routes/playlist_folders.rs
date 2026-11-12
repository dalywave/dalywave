//! Папки плейлистов и «Свой порядок». Вне папок папки и плейлисты делят одну шкалу позиций, внутри
//! папки — своя. Позиции дробные: перестановка меняет одну строку, а когда соседи сходятся слишком
//! близко, уровень перенумеровывается. Удалённая папка не уносит плейлисты — они встают на её место.

use axum::{
    Router,
    extract::State,
    http::StatusCode,
    routing::{patch, post},
};
use dalywave_protocol::{
    ErrorBody,
    library::{
        MovePlaylist, MovePlaylistFolder, NewPlaylistFolder, PlaylistFolder, PlaylistFolderPatch,
        PlaylistList,
    },
    realtime::{LibraryChange, ServerEvent},
    sync::SyncEntity,
};
use sqlx::{PgConnection, PgExecutor};
use uuid::Uuid;

use super::playlists::{STEP, positions_between, summaries};
use crate::{
    auth::Principal,
    error::{AppError, AppResult, bad},
    extract::{Json, Path},
    realtime::{self, Audience},
    state::AppState,
};

const MAX_TITLE: usize = 100;
/// Больше папок — это уже не порядок, а склад.
pub const MAX_FOLDERS: i64 = 100;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/playlist-folders", post(create))
        .route("/playlist-folders/{id}", patch(update).delete(remove))
        .route("/playlist-folders/{id}/move", post(move_folder))
        .route("/playlists/{id}/move", post(move_playlist))
}

/// Папки человека по «Своему порядку».
pub async fn folders(
    db: impl PgExecutor<'_>,
    user: Uuid,
) -> Result<Vec<PlaylistFolder>, sqlx::Error> {
    sqlx::query_as!(
        PlaylistFolder,
        "SELECT id, title, position, collapsed, created_at, updated_at FROM playlist_folders
         WHERE user_id = $1 ORDER BY position, id",
        user,
    )
    .fetch_all(db)
    .await
}

fn clean_title(title: &str) -> AppResult<String> {
    let title = title.split_whitespace().collect::<Vec<_>>().join(" ");
    if title.is_empty() || title.chars().count() > MAX_TITLE {
        return Err(bad("название папки — от 1 до 100 символов"));
    }
    Ok(title)
}

async fn changed(conn: &mut PgConnection, user: Uuid) {
    realtime::publish(
        conn,
        Audience::User(user),
        ServerEvent::LibraryChanged(LibraryChange {
            entities: vec![SyncEntity::Playlists, SyncEntity::PlaylistFolders],
        }),
    )
    .await;
}

/// Перестановки одного человека — по очереди: соседи и перенумерация считаются без гонок.
async fn lock_user(conn: &mut PgConnection, user: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query!("SELECT id FROM users WHERE id = $1 FOR UPDATE", user)
        .fetch_one(conn)
        .await?;
    Ok(())
}

async fn own_folder(conn: &mut PgConnection, user: Uuid, id: Uuid) -> AppResult<f64> {
    sqlx::query_scalar!(
        "SELECT position FROM playlist_folders WHERE id = $1 AND user_id = $2",
        id,
        user
    )
    .fetch_optional(conn)
    .await?
    .ok_or(AppError::NotFound)
}

/// Уровень по порядку: вне папок (`None`) — папки и плейлисты без папки, иначе плейлисты папки.
async fn level(
    conn: &mut PgConnection,
    user: Uuid,
    folder: Option<Uuid>,
) -> Result<Vec<(Uuid, f64)>, sqlx::Error> {
    let rows = match folder {
        None => sqlx::query!(
            r#"SELECT id AS "id!", position AS "position!" FROM (
                   SELECT id, position FROM playlists WHERE user_id = $1 AND folder_id IS NULL
                   UNION ALL
                   SELECT id, position FROM playlist_folders WHERE user_id = $1) x
               ORDER BY position, id"#,
            user,
        )
        .fetch_all(conn)
        .await?
        .into_iter()
        .map(|r| (r.id, r.position))
        .collect(),
        Some(f) => sqlx::query!(
            "SELECT id, position FROM playlists WHERE user_id = $1 AND folder_id = $2 ORDER BY position, id",
            user,
            f,
        )
        .fetch_all(conn)
        .await?
        .into_iter()
        .map(|r| (r.id, r.position))
        .collect(),
    };
    Ok(rows)
}

/// Где встать на уровне без самого `skip`: после `after`, в начало или в конец.
/// `Ok(None)` — между соседями места нет, уровень надо перенумеровать.
fn slot(
    level: &[(Uuid, f64)],
    skip: Uuid,
    after: Option<Uuid>,
    at_start: bool,
) -> AppResult<Option<f64>> {
    let rest: Vec<&(Uuid, f64)> = level.iter().filter(|(id, _)| *id != skip).collect();
    let (before, next) = match after {
        Some(a) => {
            let i = rest
                .iter()
                .position(|(id, _)| *id == a)
                .ok_or_else(|| bad("соседа нет на этом уровне медиатеки"))?;
            (Some(rest[i].1), rest.get(i + 1).map(|r| r.1))
        }
        None if at_start => (None, rest.first().map(|r| r.1)),
        None => (rest.last().map(|r| r.1), None),
    };
    Ok(positions_between(before, next, 1).map(|p| p[0]))
}

/// Перенумеровать уровень: 1024, 2048, … в текущем порядке.
async fn rebalance(
    conn: &mut PgConnection,
    user: Uuid,
    folder: Option<Uuid>,
) -> Result<(), sqlx::Error> {
    let rows = level(conn, user, folder).await?;
    for (i, (id, _)) in rows.iter().enumerate() {
        let position = (i + 1) as f64 * STEP;
        sqlx::query!(
            "UPDATE playlists SET position = $2 WHERE id = $1 AND position <> $2",
            id,
            position
        )
        .execute(&mut *conn)
        .await?;
        if folder.is_none() {
            sqlx::query!(
                "UPDATE playlist_folders SET position = $2 WHERE id = $1 AND position <> $2",
                id,
                position
            )
            .execute(&mut *conn)
            .await?;
        }
    }
    Ok(())
}

/// Место на уровне: с перенумерацией, если соседи сошлись.
async fn place(
    conn: &mut PgConnection,
    user: Uuid,
    folder: Option<Uuid>,
    skip: Uuid,
    after: Option<Uuid>,
    at_start: bool,
) -> AppResult<f64> {
    if let Some(p) = slot(&level(conn, user, folder).await?, skip, after, at_start)? {
        return Ok(p);
    }
    rebalance(conn, user, folder).await?;
    slot(&level(conn, user, folder).await?, skip, after, at_start)?
        .ok_or_else(|| AppError::Internal(anyhow::anyhow!("no room after rebalancing")))
}

async fn list_of(st: &AppState, user: Uuid) -> AppResult<PlaylistList> {
    Ok(PlaylistList {
        items: summaries(&st.db, user, None, false).await?,
        folders: folders(&st.db, user).await?,
    })
}

#[utoipa::path(
    post,
    path = "/api/playlist-folders",
    tag = "library",
    security(("session" = []), ("device" = [])),
    request_body = NewPlaylistFolder,
    responses(
        (status = 201, description = "Папка создана первой вне папок; названные плейлисты уже в ней", body = PlaylistList),
        (status = 400, description = "Пустое или длинное название, папок уже 100", body = ErrorBody),
        (status = 404, description = "Нет такого плейлиста", body = ErrorBody),
    ),
)]
pub async fn create(
    State(st): State<AppState>,
    principal: Principal,
    Json(body): Json<NewPlaylistFolder>,
) -> AppResult<(StatusCode, Json<PlaylistList>)> {
    let user = principal.user.id;
    let title = clean_title(&body.title)?;
    let mut tx = st.db.begin().await?;
    lock_user(&mut tx, user).await?;
    let have = sqlx::query_scalar!(
        r#"SELECT count(*) AS "n!" FROM playlist_folders WHERE user_id = $1"#,
        user
    )
    .fetch_one(&mut *tx)
    .await?;
    if have >= MAX_FOLDERS {
        return Err(bad("папок не больше 100"));
    }
    let position = place(&mut tx, user, None, Uuid::nil(), None, true).await?;
    let id = sqlx::query_scalar!(
        "INSERT INTO playlist_folders (user_id, title, position) VALUES ($1, $2, $3) RETURNING id",
        user,
        title,
        position,
    )
    .fetch_one(&mut *tx)
    .await?;
    for playlist in &body.playlist_ids {
        put(&mut tx, user, *playlist, Some(id), None, false).await?;
    }
    changed(&mut tx, user).await;
    tx.commit().await?;
    Ok((StatusCode::CREATED, Json(list_of(&st, user).await?)))
}

#[utoipa::path(
    patch,
    path = "/api/playlist-folders/{id}",
    tag = "library",
    security(("session" = []), ("device" = [])),
    params(("id" = Uuid, Path, description = "id папки")),
    request_body = PlaylistFolderPatch,
    responses(
        (status = 200, description = "Папка после правки", body = PlaylistFolder),
        (status = 400, description = "Пустое или длинное название", body = ErrorBody),
        (status = 404, description = "Нет такой папки", body = ErrorBody),
    ),
)]
pub async fn update(
    State(st): State<AppState>,
    principal: Principal,
    Path(id): Path<Uuid>,
    Json(patch): Json<PlaylistFolderPatch>,
) -> AppResult<Json<PlaylistFolder>> {
    let user = principal.user.id;
    let title = patch.title.as_deref().map(clean_title).transpose()?;
    let mut tx = st.db.begin().await?;
    // Свернуть и развернуть — не правка папки: «Недавние» от этого не меняются
    let folder = sqlx::query_as!(
        PlaylistFolder,
        "UPDATE playlist_folders SET title = COALESCE($3, title), collapsed = COALESCE($4, collapsed),
                updated_at = CASE WHEN $3::text IS NULL THEN updated_at ELSE now() END
         WHERE id = $1 AND user_id = $2
         RETURNING id, title, position, collapsed, created_at, updated_at",
        id,
        user,
        title,
        patch.collapsed,
    )
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(AppError::NotFound)?;
    changed(&mut tx, user).await;
    tx.commit().await?;
    Ok(Json(folder))
}

#[utoipa::path(
    delete,
    path = "/api/playlist-folders/{id}",
    tag = "library",
    security(("session" = []), ("device" = [])),
    params(("id" = Uuid, Path, description = "id папки")),
    responses(
        (status = 200, description = "Папка удалена, её плейлисты встали на её место", body = PlaylistList),
        (status = 404, description = "Нет такой папки", body = ErrorBody),
    ),
)]
pub async fn remove(
    State(st): State<AppState>,
    principal: Principal,
    Path(id): Path<Uuid>,
) -> AppResult<Json<PlaylistList>> {
    let user = principal.user.id;
    let mut tx = st.db.begin().await?;
    lock_user(&mut tx, user).await?;
    own_folder(&mut tx, user, id).await?;
    // Плейлисты папки — по её порядку на место самой папки
    let inside = level(&mut tx, user, Some(id)).await?;
    let mut after = id;
    for (playlist, _) in &inside {
        let position = place(&mut tx, user, None, *playlist, Some(after), false).await?;
        sqlx::query!(
            "UPDATE playlists SET folder_id = NULL, position = $2 WHERE id = $1",
            playlist,
            position
        )
        .execute(&mut *tx)
        .await?;
        after = *playlist;
    }
    sqlx::query!(
        "DELETE FROM playlist_folders WHERE id = $1 AND user_id = $2",
        id,
        user
    )
    .execute(&mut *tx)
    .await?;
    changed(&mut tx, user).await;
    tx.commit().await?;
    Ok(Json(list_of(&st, user).await?))
}

#[utoipa::path(
    post,
    path = "/api/playlist-folders/{id}/move",
    tag = "library",
    security(("session" = []), ("device" = [])),
    params(("id" = Uuid, Path, description = "id папки")),
    request_body = MovePlaylistFolder,
    responses(
        (status = 200, description = "Папка на новом месте", body = PlaylistList),
        (status = 400, description = "Соседа нет вне папок", body = ErrorBody),
        (status = 404, description = "Нет такой папки", body = ErrorBody),
    ),
)]
pub async fn move_folder(
    State(st): State<AppState>,
    principal: Principal,
    Path(id): Path<Uuid>,
    Json(body): Json<MovePlaylistFolder>,
) -> AppResult<Json<PlaylistList>> {
    let user = principal.user.id;
    let mut tx = st.db.begin().await?;
    lock_user(&mut tx, user).await?;
    own_folder(&mut tx, user, id).await?;
    let position = place(&mut tx, user, None, id, body.after_id, body.at_start).await?;
    sqlx::query!(
        "UPDATE playlist_folders SET position = $2 WHERE id = $1",
        id,
        position
    )
    .execute(&mut *tx)
    .await?;
    changed(&mut tx, user).await;
    tx.commit().await?;
    Ok(Json(list_of(&st, user).await?))
}

/// Переложить плейлист на уровень `folder` и на место там.
async fn put(
    conn: &mut PgConnection,
    user: Uuid,
    playlist: Uuid,
    folder: Option<Uuid>,
    after: Option<Uuid>,
    at_start: bool,
) -> AppResult<()> {
    sqlx::query_scalar!(
        "SELECT id FROM playlists WHERE id = $1 AND user_id = $2",
        playlist,
        user
    )
    .fetch_optional(&mut *conn)
    .await?
    .ok_or(AppError::NotFound)?;
    if let Some(f) = folder {
        own_folder(conn, user, f).await?;
    }
    let position = place(conn, user, folder, playlist, after, at_start).await?;
    // Перекладка — не правка плейлиста: updated_at и «Недавние» не трогаются
    sqlx::query!(
        "UPDATE playlists SET folder_id = $2, position = $3 WHERE id = $1",
        playlist,
        folder,
        position,
    )
    .execute(conn)
    .await?;
    Ok(())
}

#[utoipa::path(
    post,
    path = "/api/playlists/{id}/move",
    tag = "library",
    security(("session" = []), ("device" = [])),
    params(("id" = Uuid, Path, description = "id плейлиста")),
    request_body = MovePlaylist,
    responses(
        (status = 200, description = "Плейлист в папке (или вне папок) на новом месте", body = PlaylistList),
        (status = 400, description = "Соседа нет на этом уровне", body = ErrorBody),
        (status = 404, description = "Нет такого плейлиста или папки", body = ErrorBody),
    ),
)]
pub async fn move_playlist(
    State(st): State<AppState>,
    principal: Principal,
    Path(id): Path<Uuid>,
    Json(body): Json<MovePlaylist>,
) -> AppResult<Json<PlaylistList>> {
    let user = principal.user.id;
    let mut tx = st.db.begin().await?;
    lock_user(&mut tx, user).await?;
    put(
        &mut tx,
        user,
        id,
        body.folder_id,
        body.after_id,
        body.at_start,
    )
    .await?;
    changed(&mut tx, user).await;
    tx.commit().await?;
    Ok(Json(list_of(&st, user).await?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slots_between_neighbours() {
        let [a, b, c, m] = [1, 2, 3, 9].map(Uuid::from_u128);
        let level = [(a, 1024.0), (m, 1536.0), (b, 2048.0), (c, 3072.0)];
        assert_eq!(slot(&level, m, Some(b), false).unwrap(), Some(2560.0));
        assert_eq!(slot(&level, m, None, true).unwrap(), Some(0.0));
        assert_eq!(slot(&level, m, None, false).unwrap(), Some(4096.0));
        // Сам себе не сосед: встать после своего же соседа сверху — на прежнее место
        assert_eq!(slot(&level, m, Some(a), false).unwrap(), Some(1536.0));
        assert!(slot(&level, m, Some(Uuid::from_u128(7)), false).is_err());
        let tight = [(a, 1.0), (b, 1.0 + 1e-7)];
        assert_eq!(slot(&tight, m, Some(a), false).unwrap(), None);
    }

    #[test]
    fn folder_titles_are_checked() {
        assert_eq!(clean_title("  Для  бега ").unwrap(), "Для бега");
        assert!(clean_title(" ").is_err());
        assert!(clean_title(&"п".repeat(101)).is_err());
    }
}
