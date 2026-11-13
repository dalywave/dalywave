//! Совместные плейлисты: владелец делает плейлист совместным и зовёт друзей (или даёт ссылку),
//! участники добавляют, убирают и переставляют треки (это обычные обработчики плейлиста —
//! [`super::playlists::lock_editable`] пускает участников), владелец может убрать участника, участник —
//! выйти. Название, обложка, «Собрать как сет» и удаление — только у владельца.
//!
//! На устройства участника совместный плейлист приходит синхронизацией: при входе номера изменения
//! его позиций поднимаются, при выходе ему уходит tombstone плейлиста (триггер `playlist_members_gone`).
//!
//! Тут же — «Показывать друзьям» для своего плейлиста.

use axum::{
    Router,
    extract::State,
    http::StatusCode,
    routing::{delete, get, post, put},
};
use dalywave_protocol::{
    ErrorBody,
    library::{PlaylistKind, PlaylistSummary},
    notifications::{NotificationItem, NotificationKind},
    social::{AddPlaylistMembers, InviteLink, PlaylistInvite, PlaylistVisibility},
    sync::SyncEntity,
};
use sqlx::PgConnection;
use uuid::Uuid;

use super::playlists::{
    Scope, changed, changed_playlist, lock_any, owner_only, participants, summaries_in, touch,
};
use crate::{
    auth::Principal,
    catalog::views::{ImageKind, image_set},
    error::{AppError, AppResult, bad},
    extract::{Json, Path},
    notify::{self, Draft},
    social,
    state::AppState,
};

/// Участников в одном плейлисте — не больше.
pub const MAX_MEMBERS: i64 = 50;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route(
            "/playlists/{id}/collaborative",
            put(make_collaborative).delete(stop_collaborative),
        )
        .route("/playlists/{id}/visibility", put(set_visibility))
        .route("/playlists/{id}/invite", get(invite).post(reset_invite))
        .route("/playlists/{id}/members", post(add_members))
        .route("/playlists/{id}/members/{userId}", delete(remove_member))
        .route("/playlists/join/{code}", get(join_preview).post(join))
}

/// Свой плейлист вида `user` под замком.
async fn lock_mine(conn: &mut PgConnection, user: Uuid, id: Uuid) -> AppResult<bool> {
    let locked = lock_any(conn, user, id).await?;
    if !locked.is_owner(user) {
        return Err(owner_only());
    }
    if locked.kind != PlaylistKind::User.as_str() {
        return Err(AppError::Conflict {
            code: "read_only",
            message: "Этот плейлист собирается сам — сохраните его как свой, чтобы позвать друзей"
                .into(),
        });
    }
    Ok(locked.collaborative)
}

async fn summary_of(st: &AppState, user: Uuid, id: Uuid) -> AppResult<PlaylistSummary> {
    summaries_in(&st.db, user, Scope::Readable, Some(&[id]), false)
        .await?
        .into_iter()
        .next()
        .ok_or(AppError::NotFound)
}

/// Делает плейлист совместным (и даёт ему код ссылки). Повтор ничего не меняет.
async fn enable(conn: &mut PgConnection, id: Uuid) -> AppResult<()> {
    let code = social::new_code()?;
    sqlx::query!(
        "UPDATE playlists SET collaborative = true, invite_code = COALESCE(invite_code, $2), updated_at = now()
         WHERE id = $1 AND (NOT collaborative OR invite_code IS NULL)",
        id,
        code
    )
    .execute(&mut *conn)
    .await?;
    Ok(())
}

/// Человек входит в совместный плейлист: строка участника (место — первым в его медиатеке) и новые
/// номера изменения позиций, чтобы они пришли на его устройства. `false` — уже участник.
async fn add_member(conn: &mut PgConnection, id: Uuid, user: Uuid) -> AppResult<bool> {
    let count = sqlx::query_scalar!(
        r#"SELECT count(*) AS "n!" FROM playlist_members WHERE playlist_id = $1"#,
        id
    )
    .fetch_one(&mut *conn)
    .await?;
    if count >= MAX_MEMBERS {
        return Err(AppError::Conflict {
            code: "too_many_members",
            message: format!("В совместном плейлисте — не больше {MAX_MEMBERS} участников"),
        });
    }
    let inserted = sqlx::query!(
        "INSERT INTO playlist_members (playlist_id, user_id, position)
         VALUES ($1, $2, COALESCE(LEAST(
             (SELECT min(position) FROM playlists WHERE user_id = $2 AND folder_id IS NULL),
             (SELECT min(position) FROM playlist_folders WHERE user_id = $2),
             (SELECT min(position) FROM playlist_members WHERE user_id = $2)), 1024) - 1024)
         ON CONFLICT DO NOTHING",
        id,
        user
    )
    .execute(&mut *conn)
    .await?
    .rows_affected()
        > 0;
    if inserted {
        sqlx::query!(
            "UPDATE playlist_items SET position = position WHERE playlist_id = $1",
            id
        )
        .execute(&mut *conn)
        .await?;
        touch(conn, id).await?;
    }
    Ok(inserted)
}

fn invite_url(st: &AppState, code: &str) -> String {
    format!("{}/playlist/join/{code}", st.config.public_url)
}

#[utoipa::path(
    put,
    path = "/api/playlists/{id}/collaborative",
    tag = "library",
    security(("session" = []), ("device" = [])),
    params(("id" = Uuid, Path, description = "id плейлиста")),
    responses(
        (status = 200, description = "Плейлист совместный: друзья, которых позовут, смогут добавлять треки", body = PlaylistSummary),
        (status = 403, description = "Не свой плейлист", body = ErrorBody),
        (status = 404, description = "Нет такого плейлиста", body = ErrorBody),
        (status = 409, description = "Микс — сначала сохранить копию", body = ErrorBody),
    ),
)]
pub async fn make_collaborative(
    State(st): State<AppState>,
    principal: Principal,
    Path(id): Path<Uuid>,
) -> AppResult<Json<PlaylistSummary>> {
    let user = principal.user.id;
    let mut tx = st.db.begin().await?;
    lock_mine(&mut tx, user, id).await?;
    enable(&mut tx, id).await?;
    changed(&mut tx, user, vec![SyncEntity::Playlists]).await;
    tx.commit().await?;
    Ok(Json(summary_of(&st, user, id).await?))
}

#[utoipa::path(
    delete,
    path = "/api/playlists/{id}/collaborative",
    tag = "library",
    security(("session" = []), ("device" = [])),
    params(("id" = Uuid, Path, description = "id плейлиста")),
    responses(
        (status = 200, description = "Плейлист снова только свой: участники его больше не видят, ссылка не работает", body = PlaylistSummary),
        (status = 403, description = "Не свой плейлист", body = ErrorBody),
        (status = 404, description = "Нет такого плейлиста", body = ErrorBody),
    ),
)]
pub async fn stop_collaborative(
    State(st): State<AppState>,
    principal: Principal,
    Path(id): Path<Uuid>,
) -> AppResult<Json<PlaylistSummary>> {
    let user = principal.user.id;
    let mut tx = st.db.begin().await?;
    lock_mine(&mut tx, user, id).await?;
    let users = participants(&mut tx, id).await?;
    sqlx::query!("DELETE FROM playlist_members WHERE playlist_id = $1", id)
        .execute(&mut *tx)
        .await?;
    sqlx::query!(
        "UPDATE playlists SET collaborative = false, invite_code = NULL, updated_at = now() WHERE id = $1",
        id
    )
    .execute(&mut *tx)
    .await?;
    for u in users {
        changed(
            &mut tx,
            u,
            vec![SyncEntity::Playlists, SyncEntity::PlaylistItems],
        )
        .await;
    }
    tx.commit().await?;
    Ok(Json(summary_of(&st, user, id).await?))
}

#[utoipa::path(
    put,
    path = "/api/playlists/{id}/visibility",
    tag = "library",
    security(("session" = []), ("device" = [])),
    params(("id" = Uuid, Path, description = "id плейлиста")),
    request_body = PlaylistVisibility,
    responses(
        (status = 200, description = "Видят ли друзья плейлист на странице владельца", body = PlaylistSummary),
        (status = 403, description = "Не свой плейлист", body = ErrorBody),
        (status = 404, description = "Нет такого плейлиста", body = ErrorBody),
    ),
)]
pub async fn set_visibility(
    State(st): State<AppState>,
    principal: Principal,
    Path(id): Path<Uuid>,
    Json(body): Json<PlaylistVisibility>,
) -> AppResult<Json<PlaylistSummary>> {
    let user = principal.user.id;
    let mut tx = st.db.begin().await?;
    if !lock_any(&mut tx, user, id).await?.is_owner(user) {
        return Err(owner_only());
    }
    sqlx::query!(
        "UPDATE playlists SET friends_can_see = $2 WHERE id = $1",
        id,
        body.friends_can_see
    )
    .execute(&mut *tx)
    .await?;
    changed_playlist(&mut tx, id, vec![SyncEntity::Playlists]).await;
    tx.commit().await?;
    Ok(Json(summary_of(&st, user, id).await?))
}

async fn invite_link(st: &AppState, user: Uuid, id: Uuid, reset: bool) -> AppResult<InviteLink> {
    let mut tx = st.db.begin().await?;
    lock_mine(&mut tx, user, id).await?;
    enable(&mut tx, id).await?;
    if reset {
        sqlx::query!(
            "UPDATE playlists SET invite_code = $2 WHERE id = $1",
            id,
            social::new_code()?
        )
        .execute(&mut *tx)
        .await?;
    }
    let code = sqlx::query_scalar!("SELECT invite_code FROM playlists WHERE id = $1", id)
        .fetch_one(&mut *tx)
        .await?
        .ok_or_else(|| {
            AppError::Internal(anyhow::anyhow!("collaborative playlist without code"))
        })?;
    changed(&mut tx, user, vec![SyncEntity::Playlists]).await;
    tx.commit().await?;
    Ok(InviteLink {
        url: invite_url(st, &code),
        code,
    })
}

#[utoipa::path(
    get,
    path = "/api/playlists/{id}/invite",
    tag = "library",
    security(("session" = []), ("device" = [])),
    params(("id" = Uuid, Path, description = "id плейлиста")),
    responses(
        (status = 200, description = "Ссылка-приглашение (плейлист становится совместным)", body = InviteLink),
        (status = 403, description = "Не свой плейлист", body = ErrorBody),
        (status = 404, description = "Нет такого плейлиста", body = ErrorBody),
    ),
)]
pub async fn invite(
    State(st): State<AppState>,
    principal: Principal,
    Path(id): Path<Uuid>,
) -> AppResult<Json<InviteLink>> {
    Ok(Json(invite_link(&st, principal.user.id, id, false).await?))
}

#[utoipa::path(
    post,
    path = "/api/playlists/{id}/invite",
    tag = "library",
    security(("session" = []), ("device" = [])),
    params(("id" = Uuid, Path, description = "id плейлиста")),
    responses(
        (status = 200, description = "Новая ссылка; прежняя больше не работает", body = InviteLink),
        (status = 403, description = "Не свой плейлист", body = ErrorBody),
        (status = 404, description = "Нет такого плейлиста", body = ErrorBody),
    ),
)]
pub async fn reset_invite(
    State(st): State<AppState>,
    principal: Principal,
    Path(id): Path<Uuid>,
) -> AppResult<Json<InviteLink>> {
    Ok(Json(invite_link(&st, principal.user.id, id, true).await?))
}

#[utoipa::path(
    post,
    path = "/api/playlists/{id}/members",
    tag = "library",
    security(("session" = []), ("device" = [])),
    params(("id" = Uuid, Path, description = "id плейлиста")),
    request_body = AddPlaylistMembers,
    responses(
        (status = 200, description = "Друзья — участники (плейлист становится совместным)", body = PlaylistSummary),
        (status = 400, description = "Среди людей есть не друзья", body = ErrorBody),
        (status = 403, description = "Не свой плейлист", body = ErrorBody),
        (status = 404, description = "Нет такого плейлиста", body = ErrorBody),
    ),
)]
pub async fn add_members(
    State(st): State<AppState>,
    principal: Principal,
    Path(id): Path<Uuid>,
    Json(body): Json<AddPlaylistMembers>,
) -> AppResult<Json<PlaylistSummary>> {
    let user = principal.user.id;
    if body.user_ids.is_empty() || body.user_ids.len() > MAX_MEMBERS as usize {
        return Err(bad(format!("от 1 до {MAX_MEMBERS} человек за раз")));
    }
    let mut tx = st.db.begin().await?;
    lock_mine(&mut tx, user, id).await?;
    let friends = social::friend_ids(&mut *tx, user).await?;
    if body.user_ids.iter().any(|u| !friends.contains(u)) {
        return Err(bad("позвать в плейлист можно только друзей"));
    }
    enable(&mut tx, id).await?;
    let title = sqlx::query_scalar!("SELECT title FROM playlists WHERE id = $1", id)
        .fetch_one(&mut *tx)
        .await?;
    let mut added = Vec::new();
    for &member in &body.user_ids {
        if add_member(&mut tx, id, member).await? {
            added.push(member);
        }
    }
    for &member in &added {
        notify::emit_in(
            &mut tx,
            member,
            Draft::new(
                NotificationKind::SharedPlaylist,
                format!("{} зовёт вас в «{title}»", principal.user.name),
                "Совместный плейлист: добавляйте свои треки",
            )
            .link(format!("/playlist/{id}"))
            .image(principal.user.avatar_url.clone())
            .once(format!("pl_invite:{id}")),
        )
        .await?;
    }
    changed_playlist(
        &mut tx,
        id,
        vec![SyncEntity::Playlists, SyncEntity::PlaylistItems],
    )
    .await;
    tx.commit().await?;
    Ok(Json(summary_of(&st, user, id).await?))
}

#[utoipa::path(
    delete,
    path = "/api/playlists/{id}/members/{userId}",
    tag = "library",
    security(("session" = []), ("device" = [])),
    params(
        ("id" = Uuid, Path, description = "id плейлиста"),
        ("userId" = Uuid, Path, description = "Участник; свой id — выйти из плейлиста"),
    ),
    responses(
        (status = 204, description = "Участник убран (или вышел): плейлиста у него больше нет"),
        (status = 403, description = "Убрать другого может только владелец", body = ErrorBody),
        (status = 404, description = "Нет такого плейлиста или участника", body = ErrorBody),
    ),
)]
pub async fn remove_member(
    State(st): State<AppState>,
    principal: Principal,
    Path((id, member)): Path<(Uuid, Uuid)>,
) -> AppResult<StatusCode> {
    let user = principal.user.id;
    let mut tx = st.db.begin().await?;
    let locked = lock_any(&mut tx, user, id).await?;
    if member != user && !locked.is_owner(user) {
        return Err(owner_only());
    }
    let removed = sqlx::query!(
        "DELETE FROM playlist_members WHERE playlist_id = $1 AND user_id = $2",
        id,
        member
    )
    .execute(&mut *tx)
    .await?;
    if removed.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }
    touch(&mut tx, id).await?;
    changed(
        &mut tx,
        member,
        vec![SyncEntity::Playlists, SyncEntity::PlaylistItems],
    )
    .await;
    changed_playlist(&mut tx, id, vec![SyncEntity::Playlists]).await;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Плейлист по коду ссылки: id, владелец, название.
async fn by_code(st: &AppState, code: &str) -> AppResult<(Uuid, Uuid)> {
    if code.len() > 32 {
        return Err(AppError::NotFound);
    }
    let row = sqlx::query!(
        "SELECT id, user_id FROM playlists WHERE invite_code = $1 AND collaborative",
        code
    )
    .fetch_optional(&st.db)
    .await?
    .ok_or(AppError::Missing {
        code: "invite_expired",
        message: "Ссылка больше не работает — попросите новую".into(),
    })?;
    Ok((row.id, row.user_id))
}

/// Карточка приглашения: плейлист глазами владельца, но без его места и папки.
async fn invite_view(
    st: &AppState,
    user: Uuid,
    id: Uuid,
    owner: Uuid,
) -> AppResult<PlaylistInvite> {
    let joined = sqlx::query_scalar!(
        r#"SELECT ($2 = (SELECT user_id FROM playlists WHERE id = $1)
                  OR EXISTS (SELECT 1 FROM playlist_members WHERE playlist_id = $1 AND user_id = $2)) AS "j!""#,
        id,
        user
    )
    .fetch_one(&st.db)
    .await?;
    let mut playlist = summaries_in(&st.db, owner, Scope::Library, Some(&[id]), false)
        .await?
        .into_iter()
        .next()
        .ok_or(AppError::NotFound)?;
    playlist.folder_id = None;
    playlist.pinned = false;
    playlist.played_at = None;
    let owner = social::people(&st.db, &[owner])
        .await?
        .remove(&owner)
        .ok_or(AppError::NotFound)?;
    playlist.owner = Some(owner.clone());
    Ok(PlaylistInvite {
        playlist,
        owner,
        joined,
    })
}

#[utoipa::path(
    get,
    path = "/api/playlists/join/{code}",
    tag = "library",
    security(("session" = []), ("device" = [])),
    params(("code" = String, Path, description = "Код из ссылки")),
    responses(
        (status = 200, description = "Куда зовут: плейлист, владелец, вошёл ли уже", body = PlaylistInvite),
        (status = 404, description = "Ссылка больше не работает (`invite_expired`)", body = ErrorBody),
    ),
)]
pub async fn join_preview(
    State(st): State<AppState>,
    principal: Principal,
    Path(code): Path<String>,
) -> AppResult<Json<PlaylistInvite>> {
    let (id, owner) = by_code(&st, &code).await?;
    Ok(Json(invite_view(&st, principal.user.id, id, owner).await?))
}

#[utoipa::path(
    post,
    path = "/api/playlists/join/{code}",
    tag = "library",
    security(("session" = []), ("device" = [])),
    params(("code" = String, Path, description = "Код из ссылки")),
    responses(
        (status = 200, description = "Человек — участник; плейлист у него в медиатеке", body = PlaylistSummary),
        (status = 404, description = "Ссылка больше не работает (`invite_expired`)", body = ErrorBody),
        (status = 409, description = "Участников уже слишком много", body = ErrorBody),
    ),
)]
pub async fn join(
    State(st): State<AppState>,
    principal: Principal,
    Path(code): Path<String>,
) -> AppResult<Json<PlaylistSummary>> {
    let user = principal.user.id;
    let (id, owner) = by_code(&st, &code).await?;
    if owner != user {
        let mut tx = st.db.begin().await?;
        lock_any(&mut tx, owner, id).await?;
        if add_member(&mut tx, id, user).await? {
            changed_playlist(
                &mut tx,
                id,
                vec![SyncEntity::Playlists, SyncEntity::PlaylistItems],
            )
            .await;
        }
        tx.commit().await?;
    }
    Ok(Json(summary_of(&st, user, id).await?))
}

/// Участник добавил треки — остальным (владельцу и участникам) одно уведомление на десять минут:
/// «Маша добавил 3 трека в «Вечер»».
pub async fn notify_added(
    conn: &mut PgConnection,
    id: Uuid,
    actor: Uuid,
    track_ids: &[Uuid],
) -> AppResult<()> {
    let meta = sqlx::query!(
        "SELECT p.title, u.name FROM playlists p, users u WHERE p.id = $1 AND u.id = $2",
        id,
        actor
    )
    .fetch_one(&mut *conn)
    .await?;
    let tracks = sqlx::query!(
        r#"SELECT t.id, t.title, al.id AS "album_id?", al.cover_key AS "cover_key?", al.cover_source AS "cover_source?",
                  (SELECT a.name FROM track_artists ta JOIN artists a ON a.id = ta.artist_id
                   WHERE ta.track_id = t.id ORDER BY ta.position LIMIT 1) AS artist
           FROM unnest($1::uuid[]) WITH ORDINALITY AS ids(id, ord)
           JOIN tracks t ON t.id = ids.id
           LEFT JOIN albums al ON al.id = t.album_id
           ORDER BY ids.ord LIMIT 20"#,
        track_ids
    )
    .fetch_all(&mut *conn)
    .await?;
    let items: Vec<NotificationItem> = tracks
        .into_iter()
        .map(|t| NotificationItem {
            image: t.album_id.and_then(|a| {
                image_set(
                    ImageKind::Albums,
                    a,
                    t.cover_key.as_deref(),
                    t.cover_source.as_deref(),
                )
                .map(|s| s.small)
            }),
            title: t.title,
            subtitle: t.artist.unwrap_or_default(),
            link: Some(format!("/playlist/{id}")),
        })
        .collect();
    // Число — по всем трекам, список — по первым двадцати
    let weight = track_ids.len().max(1) as i32;
    let image = items.iter().find_map(|i| i.image.clone());
    let body = items
        .first()
        .map(|i| {
            if i.subtitle.is_empty() {
                i.title.clone()
            } else {
                format!("{} — {}", i.subtitle, i.title)
            }
        })
        .unwrap_or_default();
    for user in participants(&mut *conn, id).await? {
        if user == actor {
            continue;
        }
        let mut draft = Draft::new(
            NotificationKind::SharedPlaylist,
            format!("{} добавил {{n}} в «{}»", meta.name, meta.title),
            body.clone(),
        )
        .link(format!("/playlist/{id}"))
        .image(image.clone())
        .group(format!("pl_add:{id}:{actor}"))
        .many(items.clone());
        draft.weight = weight;
        notify::emit_in(&mut *conn, user, draft).await?;
    }
    Ok(())
}
