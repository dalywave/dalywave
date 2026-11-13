//! Раздел «Люди» админки: список пользователей с активностью, «Сейчас», страница человека,
//! история прослушиваний, «выйти везде» и удаление аккаунта. Права admin — слоем в `routes::router`.
//! Изменение имени, ролей и блокировка — `PATCH /api/admin/users/{id}` (`routes::admin`).

use std::collections::HashMap;

use axum::{
    Router,
    extract::State,
    http::StatusCode,
    routing::{delete, get},
};
use chrono::{DateTime, Utc};
use dalywave_protocol::{
    ErrorBody,
    admin::InviteStatus,
    admin_users::{
        LiveOverview, PersonBrief, RevokedSessions, StatArtist, StatDay, StatGenre, StatTrack,
        UserAcquisition, UserAcquisitions, UserCounts, UserDeletion, UserDetail, UserDirectory,
        UserFollowedArtist, UserHistoryItem, UserHistoryPage, UserHistoryQuery, UserIdentityInfo,
        UserInvite, UserStats, UserSummary,
    },
    auth::Role,
    events::ContextType,
};
use serde_json::json;
use sqlx::PgConnection;
use uuid::Uuid;

use crate::{
    admin_live, audit,
    auth::Principal,
    catalog::views,
    error::{AppError, AppResult, bad},
    extract::{Json, Path, Query},
    reco::profile::genre_ru,
    session,
    state::AppState,
    users::{self, UserRow, parse_roles},
};

/// Прослушивание засчитывается в топы с 30 секунд, как у друзей и «Итогов».
const PLAYED_MS: i32 = 30_000;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/users/table", get(directory))
        .route("/live", get(live))
        .route("/users/{id}/history", get(history))
        .route("/users/{id}/sessions", delete(revoke_all))
}

pub fn last_admin_error() -> AppError {
    AppError::Conflict {
        code: "last_admin",
        message: "это единственный администратор: сначала выдайте роль кому-нибудь ещё".into(),
    }
}

/// Человек — единственный незаблокированный администратор.
pub async fn is_last_admin(conn: &mut PgConnection, user: &UserRow) -> AppResult<bool> {
    if user.disabled || !user.roles().contains(&Role::Admin) {
        return Ok(false);
    }
    let others = sqlx::query_scalar!(
        r#"SELECT count(*) AS "n!" FROM users WHERE 'admin' = ANY(roles) AND NOT disabled AND id <> $1"#,
        user.id
    )
    .fetch_one(&mut *conn)
    .await?;
    Ok(others == 0)
}

struct Activity {
    online: bool,
    last_seen_at: Option<DateTime<Utc>>,
    listened: i64,
    plays: i64,
}

/// В сети ли, когда был и сколько слушал за 30 дней — по каждому из `ids`.
async fn activity(st: &AppState, ids: &[Uuid]) -> AppResult<HashMap<Uuid, Activity>> {
    let rows = sqlx::query!(
        r#"SELECT u.id,
                  EXISTS (SELECT 1 FROM connect_devices c
                          WHERE c.user_id = u.id AND c.connection IS NOT NULL
                            AND c.last_seen_at > now() - make_interval(secs => $2)) AS "online!",
                  GREATEST(u.last_login_at,
                           (SELECT max(s.last_seen_at) FROM sessions s WHERE s.user_id = u.id),
                           (SELECT max(c.last_seen_at) FROM connect_devices c WHERE c.user_id = u.id)) AS last_seen_at,
                  COALESCE(p.listened, 0)::bigint AS "listened!",
                  COALESCE(p.plays, 0)::bigint AS "plays!"
           FROM users u
           LEFT JOIN LATERAL (SELECT sum(e.played_ms) AS listened, count(*) AS plays FROM play_events e
                              WHERE e.user_id = u.id AND e.started_at > now() - interval '30 days') p ON true
           WHERE u.id = ANY($1)"#,
        ids,
        dalywave_protocol::connect::ONLINE_WINDOW_SECS as f64,
    )
    .fetch_all(&st.db)
    .await?;
    Ok(rows
        .into_iter()
        .map(|r| {
            (
                r.id,
                Activity {
                    online: r.online,
                    last_seen_at: r.last_seen_at,
                    listened: r.listened,
                    plays: r.plays,
                },
            )
        })
        .collect())
}

async fn summaries(st: &AppState, rows: &[UserRow]) -> AppResult<Vec<UserSummary>> {
    let ids: Vec<Uuid> = rows.iter().map(|u| u.id).collect();
    let mut providers = users::provider_titles(st, &ids).await?;
    let mut act = activity(st, &ids).await?;
    Ok(rows
        .iter()
        .map(|u| {
            let a = act.remove(&u.id);
            UserSummary {
                user: u.admin_view(providers.remove(&u.id).unwrap_or_default()),
                online: a.as_ref().is_some_and(|a| a.online),
                last_seen_at: a.as_ref().and_then(|a| a.last_seen_at).or(u.last_login_at),
                listened_ms_30d: a.as_ref().map_or(0, |a| a.listened),
                plays_30d: a.as_ref().map_or(0, |a| a.plays),
            }
        })
        .collect())
}

#[utoipa::path(
    get,
    path = "/api/admin/users/table",
    tag = "admin",
    security(("session" = []), ("device" = [])),
    responses(
        (status = 200, description = "Все люди с активностью: в сети, когда был, часы за 30 дней", body = UserDirectory),
        (status = 403, description = "Нет права admin", body = ErrorBody),
    ),
)]
pub async fn directory(State(st): State<AppState>) -> AppResult<Json<UserDirectory>> {
    let rows = sqlx::query_as!(
        UserRow,
        r#"SELECT id, issuer, email, email_verified, name, avatar_url, roles, idp_admin, disabled, created_at,
                  last_login_at, username, password_hash IS NOT NULL AS "has_password!"
           FROM users ORDER BY created_at DESC"#,
    )
    .fetch_all(&st.db)
    .await?;
    Ok(Json(UserDirectory {
        items: summaries(&st, &rows).await?,
    }))
}

#[utoipa::path(
    get,
    path = "/api/admin/live",
    tag = "admin",
    security(("session" = []), ("device" = [])),
    responses(
        (status = 200, description = "Кто в сети и что слушает; обновления — событием `admin.presence`", body = LiveOverview),
        (status = 403, description = "Нет права admin", body = ErrorBody),
    ),
)]
pub async fn live(State(st): State<AppState>) -> AppResult<Json<LiveOverview>> {
    Ok(Json(admin_live::overview(&st.db).await?))
}

struct InviteRow {
    id: Uuid,
    roles: Vec<String>,
    note: Option<String>,
    created_at: DateTime<Utc>,
    expires_at: DateTime<Utc>,
    used_at: Option<DateTime<Utc>>,
    revoked_at: Option<DateTime<Utc>>,
    person_id: Option<Uuid>,
    person_name: Option<String>,
    person_avatar: Option<String>,
}

impl InviteRow {
    fn view(self) -> UserInvite {
        let status = if self.used_at.is_some() {
            InviteStatus::Used
        } else if self.revoked_at.is_some() {
            InviteStatus::Revoked
        } else if self.expires_at <= Utc::now() {
            InviteStatus::Expired
        } else {
            InviteStatus::Active
        };
        UserInvite {
            id: self.id,
            roles: parse_roles(&self.roles),
            note: self.note,
            status,
            created_at: self.created_at,
            person: self
                .person_id
                .zip(self.person_name)
                .map(|(id, name)| PersonBrief {
                    id,
                    name,
                    avatar_url: self.person_avatar,
                }),
            used_at: self.used_at,
        }
    }
}

async fn stats(st: &AppState, id: Uuid) -> AppResult<UserStats> {
    let sums = sqlx::query!(
        r#"SELECT COALESCE(sum(played_ms) FILTER (WHERE started_at > now() - interval '7 days'), 0)::bigint AS "l7!",
                  COALESCE(sum(played_ms) FILTER (WHERE started_at > now() - interval '30 days'), 0)::bigint AS "l30!",
                  count(*) FILTER (WHERE started_at > now() - interval '7 days') AS "p7!",
                  count(*) FILTER (WHERE started_at > now() - interval '30 days') AS "p30!",
                  count(*) AS "total!"
           FROM play_events WHERE user_id = $1"#,
        id
    )
    .fetch_one(&st.db)
    .await?;

    let artists = sqlx::query!(
        r#"SELECT ta.artist_id, count(*) AS "n!"
           FROM play_events e
           JOIN track_artists ta ON ta.track_id = e.track_id AND ta.role = 'main'
           WHERE e.user_id = $1 AND e.started_at > now() - interval '30 days'
             AND e.played_ms >= $2 AND NOT played_privately(e.user_id, e.started_at)
           GROUP BY ta.artist_id ORDER BY 2 DESC, ta.artist_id LIMIT 8"#,
        id,
        PLAYED_MS
    )
    .fetch_all(&st.db)
    .await?;
    let ids: Vec<Uuid> = artists.iter().map(|a| a.artist_id).collect();
    let plays: HashMap<Uuid, i64> = artists.iter().map(|a| (a.artist_id, a.n)).collect();
    let top_artists = views::artists(&st.db, &ids, id)
        .await?
        .into_iter()
        .map(|a| StatArtist {
            plays: plays.get(&a.id).copied().unwrap_or(0),
            id: a.id,
            name: a.name,
            image: a.image,
        })
        .collect();

    let tracks = sqlx::query!(
        r#"SELECT e.track_id, count(*) AS "n!"
           FROM play_events e
           WHERE e.user_id = $1 AND e.started_at > now() - interval '30 days'
             AND e.played_ms >= $2 AND NOT played_privately(e.user_id, e.started_at)
           GROUP BY e.track_id ORDER BY 2 DESC, e.track_id LIMIT 8"#,
        id,
        PLAYED_MS
    )
    .fetch_all(&st.db)
    .await?;
    let ids: Vec<Uuid> = tracks.iter().map(|t| t.track_id).collect();
    let plays: HashMap<Uuid, i64> = tracks.iter().map(|t| (t.track_id, t.n)).collect();
    let top_tracks = views::tracks(&st.db, &ids, id)
        .await?
        .into_iter()
        .map(|t| StatTrack {
            plays: plays.get(&t.id).copied().unwrap_or(0),
            track: t,
        })
        .collect();

    let genres = sqlx::query!(
        r#"SELECT g AS "genre!", count(*) AS "n!"
           FROM play_events e
           JOIN tracks t ON t.id = e.track_id
           JOIN albums al ON al.id = t.album_id
           CROSS JOIN LATERAL unnest(al.genres) AS g
           WHERE e.user_id = $1 AND e.started_at > now() - interval '30 days'
             AND e.played_ms >= $2 AND NOT played_privately(e.user_id, e.started_at)
           GROUP BY g ORDER BY 2 DESC, g LIMIT 8"#,
        id,
        PLAYED_MS
    )
    .fetch_all(&st.db)
    .await?
    .into_iter()
    .map(|g| StatGenre {
        name: genre_ru(&g.genre),
        plays: g.n,
    })
    .collect();

    let days = sqlx::query!(
        r#"SELECT d::date AS "day!", COALESCE(sum(e.played_ms), 0)::bigint AS "listened!", count(e.id) AS "plays!"
           FROM generate_series(current_date - 29, current_date, interval '1 day') AS d
           LEFT JOIN play_events e
                  ON e.user_id = $1 AND e.started_at >= d AND e.started_at < d + interval '1 day'
           GROUP BY d ORDER BY d"#,
        id
    )
    .fetch_all(&st.db)
    .await?
    .into_iter()
    .map(|d| StatDay {
        day: d.day,
        listened_ms: d.listened,
        plays: d.plays,
    })
    .collect();

    Ok(UserStats {
        listened_ms_7d: sums.l7,
        listened_ms_30d: sums.l30,
        plays_7d: sums.p7,
        plays_30d: sums.p30,
        plays_total: sums.total,
        top_artists,
        top_tracks,
        genres,
        days,
    })
}

#[utoipa::path(
    get,
    path = "/api/admin/users/{id}",
    tag = "admin",
    security(("session" = []), ("device" = [])),
    params(("id" = Uuid, Path, description = "id пользователя")),
    responses(
        (status = 200, description = "Профиль, входы, «сейчас слушает», статистика, приглашения, подписки, скачивания", body = UserDetail),
        (status = 404, description = "Нет такого пользователя", body = ErrorBody),
    ),
)]
pub async fn detail(
    State(st): State<AppState>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<UserDetail>> {
    let user = users::get(&st, id).await?.ok_or(AppError::NotFound)?;
    let summary = summaries(&st, std::slice::from_ref(&user))
        .await?
        .pop()
        .ok_or(AppError::NotFound)?;
    let extra = sqlx::query!(
        "SELECT disabled_at, name_locked FROM users WHERE id = $1",
        id
    )
    .fetch_one(&st.db)
    .await?;
    let mut conn = st.db.acquire().await?;
    let last_admin = is_last_admin(&mut conn, &user).await?;
    drop(conn);

    let identities = sqlx::query!(
        "SELECT id, issuer, email, created_at, last_login_at FROM user_identities
         WHERE user_id = $1 ORDER BY created_at",
        id
    )
    .fetch_all(&st.db)
    .await?
    .into_iter()
    .map(|r| UserIdentityInfo {
        provider: users::provider_title(&st, &r.issuer),
        id: r.id,
        issuer: r.issuer,
        email: r.email,
        created_at: r.created_at,
        last_login_at: r.last_login_at,
    })
    .collect();

    let live = admin_live::people(&st.db, Some(id)).await?.pop();

    let c = sqlx::query!(
        r#"SELECT (SELECT count(*) FROM playlists WHERE user_id = $1 AND kind = 'user') AS "playlists!",
                  (SELECT count(*) FROM liked_tracks WHERE user_id = $1) AS "liked!",
                  (SELECT count(*) FROM saved_albums WHERE user_id = $1) AS "albums!",
                  (SELECT count(*) FROM followed_artists WHERE user_id = $1) AS "followed!",
                  (SELECT count(*) FROM friendships WHERE user_id = $1) AS "friends!",
                  (SELECT count(*) FROM imports WHERE user_id = $1) AS "imports!""#,
        id
    )
    .fetch_one(&st.db)
    .await?;
    let counts = UserCounts {
        playlists: c.playlists,
        liked_tracks: c.liked,
        saved_albums: c.albums,
        followed_artists: c.followed,
        friends: c.friends,
        imports: c.imports,
    };

    let invited_by = sqlx::query_as!(
        InviteRow,
        r#"SELECT i.id, i.roles, i.note, i.created_at, i.expires_at, i.used_at, i.revoked_at,
                  c.id AS "person_id?", c.name AS "person_name?", c.avatar_url AS person_avatar
           FROM invites i LEFT JOIN users c ON c.id = i.created_by
           WHERE i.used_by = $1 ORDER BY i.used_at DESC LIMIT 1"#,
        id
    )
    .fetch_optional(&st.db)
    .await?
    .map(InviteRow::view);
    let invites_sent = sqlx::query_as!(
        InviteRow,
        r#"SELECT i.id, i.roles, i.note, i.created_at, i.expires_at, i.used_at, i.revoked_at,
                  c.id AS "person_id?", c.name AS "person_name?", c.avatar_url AS person_avatar
           FROM invites i LEFT JOIN users c ON c.id = i.used_by
           WHERE i.created_by = $1 ORDER BY i.created_at DESC LIMIT 50"#,
        id
    )
    .fetch_all(&st.db)
    .await?
    .into_iter()
    .map(InviteRow::view)
    .collect();

    let followed_ids = sqlx::query_scalar!(
        "SELECT artist_id FROM followed_artists WHERE user_id = $1 ORDER BY created_at DESC LIMIT 12",
        id
    )
    .fetch_all(&st.db)
    .await?;
    let followed = views::artists(&st.db, &followed_ids, id)
        .await?
        .into_iter()
        .map(|a| UserFollowedArtist {
            id: a.id,
            name: a.name,
            image: a.image,
        })
        .collect();

    let acq = sqlx::query!(
        r#"SELECT count(*) AS "total!",
                  count(*) FILTER (WHERE status = 'ready') AS "ready!",
                  count(*) FILTER (WHERE status IN ('failed', 'unavailable', 'disputed')) AS "failed!",
                  count(*) FILTER (WHERE status IN ('queued', 'matching', 'downloading', 'verifying', 'processing')) AS "active!"
           FROM acquisitions WHERE requested_by = $1"#,
        id
    )
    .fetch_one(&st.db)
    .await?;
    let recent = sqlx::query!(
        r#"SELECT a.id, a.track_id, t.title, a.status, a.reason, a.created_at,
                  COALESCE((SELECT string_agg(ar.name, ', ' ORDER BY ta.position)
                            FROM track_artists ta JOIN artists ar ON ar.id = ta.artist_id
                            WHERE ta.track_id = t.id AND ta.role = 'main'), '') AS "artist!"
           FROM acquisitions a JOIN tracks t ON t.id = a.track_id
           WHERE a.requested_by = $1 ORDER BY a.created_at DESC LIMIT 10"#,
        id
    )
    .fetch_all(&st.db)
    .await?
    .into_iter()
    .map(|r| UserAcquisition {
        id: r.id,
        track_id: r.track_id,
        title: r.title,
        artist: r.artist,
        status: r.status,
        reason: r.reason,
        created_at: r.created_at,
    })
    .collect();

    Ok(Json(UserDetail {
        summary,
        identities,
        disabled_at: extra.disabled_at,
        name_locked: extra.name_locked,
        last_admin,
        live,
        stats: stats(&st, id).await?,
        counts,
        invited_by,
        invites_sent,
        followed,
        acquisitions: UserAcquisitions {
            total: acq.total,
            ready: acq.ready,
            failed: acq.failed,
            active: acq.active,
            recent,
        },
    }))
}

#[utoipa::path(
    get,
    path = "/api/admin/users/{id}/history",
    tag = "admin",
    security(("session" = []), ("device" = [])),
    params(("id" = Uuid, Path, description = "id пользователя"), UserHistoryQuery),
    responses(
        (status = 200, description = "Прослушивания, новые первыми; в приватной сессии — без трека", body = UserHistoryPage),
        (status = 400, description = "Неверный cursor или limit", body = ErrorBody),
    ),
)]
pub async fn history(
    State(st): State<AppState>,
    Path(id): Path<Uuid>,
    Query(q): Query<UserHistoryQuery>,
) -> AppResult<Json<UserHistoryPage>> {
    let limit = q.limit.unwrap_or(50);
    if !(1..=200).contains(&limit) {
        return Err(bad("limit должен быть от 1 до 200"));
    }
    let cursor = match q.cursor.as_deref() {
        Some(c) => Some(c.parse::<Uuid>().map_err(|_| bad("неверный cursor"))?),
        None => None,
    };
    let mut rows = sqlx::query!(
        r#"SELECT e.id, e.track_id, e.started_at, e.played_ms, e.duration_ms, e.end_reason, e.context_type,
                  d.name AS "device_name?", played_privately(e.user_id, e.started_at) AS "private!"
           FROM play_events e
           LEFT JOIN devices d ON d.id = e.device_id
           WHERE e.user_id = $1
             AND ($2::uuid IS NULL OR (e.started_at, e.id) < (SELECT started_at, id FROM play_events WHERE id = $2))
           ORDER BY e.started_at DESC, e.id DESC
           LIMIT $3"#,
        id,
        cursor,
        i64::from(limit) + 1,
    )
    .fetch_all(&st.db)
    .await?;
    let next_cursor = if rows.len() > limit as usize {
        rows.truncate(limit as usize);
        rows.last().map(|r| r.id.to_string())
    } else {
        None
    };
    let ids: Vec<Uuid> = rows
        .iter()
        .filter(|r| !r.private)
        .map(|r| r.track_id)
        .collect();
    let tracks: HashMap<Uuid, _> = views::tracks(&st.db, &ids, id)
        .await?
        .into_iter()
        .map(|t| (t.id, t))
        .collect();
    let items = rows
        .into_iter()
        .map(|r| UserHistoryItem {
            id: r.id,
            private: r.private,
            track: if r.private {
                None
            } else {
                tracks.get(&r.track_id).cloned()
            },
            started_at: r.started_at,
            played_ms: r.played_ms,
            duration_ms: r.duration_ms,
            end_reason: r.end_reason,
            context_type: r
                .context_type
                .filter(|_| !r.private)
                .and_then(|c| c.parse::<ContextType>().ok()),
            device_name: r.device_name,
        })
        .collect();
    Ok(Json(UserHistoryPage { items, next_cursor }))
}

#[utoipa::path(
    delete,
    path = "/api/admin/users/{id}/sessions",
    tag = "admin",
    security(("session" = []), ("device" = [])),
    params(("id" = Uuid, Path, description = "id пользователя")),
    responses(
        (status = 200, description = "Все сессии и устройства человека отозваны («выйти везде»)", body = RevokedSessions),
        (status = 404, description = "Нет такого пользователя", body = ErrorBody),
        (status = 409, description = "Свои сессии так не закрываются (`cannot_revoke_self`)", body = ErrorBody),
    ),
)]
pub async fn revoke_all(
    State(st): State<AppState>,
    principal: Principal,
    Path(id): Path<Uuid>,
) -> AppResult<Json<RevokedSessions>> {
    if id == principal.user.id {
        return Err(AppError::Conflict {
            code: "cannot_revoke_self",
            message: "свои сессии закрываются в настройках".into(),
        });
    }
    users::get(&st, id).await?.ok_or(AppError::NotFound)?;
    let mut tx = st.db.begin().await?;
    let revoked = session::revoke_all(&mut tx, id).await?;
    audit::record(
        &mut *tx,
        Some(principal.user.id),
        audit::SESSIONS_REVOKED,
        Some(id.to_string()),
        json!({ "revokedSessions": revoked }),
    )
    .await?;
    tx.commit().await?;
    Ok(Json(RevokedSessions { revoked }))
}

#[utoipa::path(
    delete,
    path = "/api/admin/users/{id}",
    tag = "admin",
    security(("session" = []), ("device" = [])),
    params(("id" = Uuid, Path, description = "id пользователя")),
    request_body = UserDeletion,
    responses(
        (status = 204, description = "Аккаунт удалён со всеми данными: медиатекой, историей, сессиями"),
        (status = 422, description = "Имя для подтверждения не совпало (`confirm_mismatch`)", body = ErrorBody),
        (status = 404, description = "Нет такого пользователя", body = ErrorBody),
        (status = 409, description = "Нельзя удалить себя (`cannot_delete_self`) или последнего администратора (`last_admin`)", body = ErrorBody),
    ),
)]
pub async fn delete_user(
    State(st): State<AppState>,
    principal: Principal,
    Path(id): Path<Uuid>,
    Json(req): Json<UserDeletion>,
) -> AppResult<StatusCode> {
    if id == principal.user.id {
        return Err(AppError::Conflict {
            code: "cannot_delete_self",
            message: "нельзя удалить самого себя".into(),
        });
    }
    let mut tx = st.db.begin().await?;
    let user = sqlx::query_as!(
        UserRow,
        r#"SELECT id, issuer, email, email_verified, name, avatar_url, roles, idp_admin, disabled, created_at,
                  last_login_at, username, password_hash IS NOT NULL AS "has_password!"
           FROM users WHERE id = $1 FOR UPDATE"#,
        id,
    )
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(AppError::NotFound)?;
    if req.confirm_name.trim() != user.name.trim() {
        return Err(AppError::Unprocessable {
            code: "confirm_mismatch",
            message: "имя для подтверждения не совпадает".into(),
        });
    }
    if is_last_admin(&mut tx, &user).await? {
        return Err(last_admin_error());
    }
    let payload = json!({
        "name": user.name,
        "email": user.email,
        "username": user.username,
        "roles": user.roles,
    });
    audit::record(
        &mut *tx,
        Some(principal.user.id),
        audit::USER_DELETED,
        Some(user.id.to_string()),
        payload,
    )
    .await?;
    // Всё личное уходит каскадом (ON DELETE CASCADE); общее — приглашения, скачивания, журнал — теряет
    // автора. Заметки синхронизации об удалённом тоже больше некому читать
    sqlx::query!("DELETE FROM users WHERE id = $1", user.id)
        .execute(&mut *tx)
        .await?;
    sqlx::query!("DELETE FROM tombstones WHERE user_id = $1", user.id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    admin_live::notify(&st.hub, user.id);
    Ok(StatusCode::NO_CONTENT)
}
