//! Раздел «Доступ» и «Журнал» админки: люди и роли, сессии с отзывом, журнал событий.
//! Весь роутер закрыт правом admin (см. `routes::router`).

use axum::{
    Router,
    extract::State,
    http::StatusCode,
    routing::{delete, get},
};
use chrono::{DateTime, Utc};
use dalywave_protocol::{
    ErrorBody,
    admin::{
        AdminUser, AuditActor, AuditEvent, AuditPage, AuditQuery, SessionInfo, SessionList,
        SessionsQuery, UserList, UserPatch,
    },
    auth::{AuthVia, Device, Platform, Role, SessionKind},
};
use ipnet::IpNet;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::{
    audit,
    auth::Principal,
    error::{AppError, AppResult, bad},
    extract::{Json, Path, Query},
    session,
    state::AppState,
    users::{self, UserRow, role_names},
};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/users", get(list_users))
        .route(
            "/users/{id}",
            get(super::admin_users::detail)
                .patch(update_user)
                .delete(super::admin_users::delete_user),
        )
        .merge(super::admin_users::routes())
        .route("/sessions", get(list_sessions))
        .route("/sessions/{id}", delete(revoke_session))
        .route("/audit", get(audit_log))
}

#[utoipa::path(
    get,
    path = "/api/admin/users",
    tag = "admin",
    security(("session" = []), ("device" = [])),
    responses(
        (status = 200, description = "Все люди; ждущие доступа — первыми", body = UserList),
        (status = 403, description = "Нет права admin", body = ErrorBody),
    ),
)]
pub async fn list_users(State(st): State<AppState>) -> AppResult<Json<UserList>> {
    let rows = sqlx::query_as!(
        UserRow,
        r#"SELECT id, issuer, email, email_verified, name, avatar_url, roles, idp_admin, disabled, created_at,
                  last_login_at, username, password_hash IS NOT NULL AS "has_password!"
           FROM users
           ORDER BY (cardinality(roles) = 0 AND NOT disabled) DESC, last_login_at DESC NULLS LAST, created_at DESC"#,
    )
    .fetch_all(&st.db)
    .await?;
    let ids: Vec<Uuid> = rows.iter().map(|u| u.id).collect();
    let mut providers = users::provider_titles(&st, &ids).await?;
    Ok(Json(UserList {
        items: rows
            .iter()
            .map(|u| u.admin_view(providers.remove(&u.id).unwrap_or_default()))
            .collect(),
        next_cursor: None,
    }))
}

#[utoipa::path(
    patch,
    path = "/api/admin/users/{id}",
    tag = "admin",
    security(("session" = []), ("device" = [])),
    params(("id" = Uuid, Path, description = "id пользователя")),
    request_body = UserPatch,
    responses(
        (status = 200, description = "Пользователь после изменения", body = AdminUser),
        (status = 404, description = "Нет такого пользователя", body = ErrorBody),
        (status = 400, description = "Пустое имя", body = ErrorBody),
        (status = 403, description = "Роль admin для общего аккаунта демо (`demo_account`)", body = ErrorBody),
        (status = 409, description = "Нельзя снять admin с себя (`cannot_demote_self`), заблокировать себя \
                                      (`cannot_disable_self`) или последнего администратора (`last_admin`)", body = ErrorBody),
    ),
)]
pub async fn update_user(
    State(st): State<AppState>,
    principal: Principal,
    Path(id): Path<Uuid>,
    Json(patch): Json<UserPatch>,
) -> AppResult<Json<AdminUser>> {
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
    let myself = user.id == principal.user.id;
    let actor = Some(principal.user.id);
    let subject = Some(user.id.to_string());
    // Общий аккаунт демо открыт всем гостям: админом он не становится
    if st.config.is_demo(user.username.as_deref())
        && patch
            .roles
            .as_ref()
            .is_some_and(|r| r.contains(&Role::Admin))
    {
        return Err(AppError::DemoAccount);
    }
    // Единственный незаблокированный администратор: без него админка останется без хозяина
    let last_admin = super::admin_users::is_last_admin(&mut tx, &user).await?;
    let wants_demote = patch
        .roles
        .as_ref()
        .is_some_and(|r| !r.contains(&Role::Admin));
    if last_admin && (wants_demote || patch.disabled == Some(true)) && !myself {
        return Err(super::admin_users::last_admin_error());
    }

    let before = user.roles();
    let mut roles = before.clone();
    let mut idp_admin = user.idp_admin;
    if let Some(wanted) = patch.roles {
        roles = Role::ALL
            .iter()
            .copied()
            .filter(|r| wanted.contains(r))
            .collect();
        if myself && before.contains(&Role::Admin) && !roles.contains(&Role::Admin) {
            return Err(AppError::Conflict {
                code: "cannot_demote_self",
                message: "нельзя снять роль администратора с самого себя".into(),
            });
        }
        // Снятый вручную admin больше не считается выданным провайдером: при входе провайдер вернёт его,
        // только если там он по-прежнему админ
        if !roles.contains(&Role::Admin) {
            idp_admin = false;
        }
        if roles != before {
            let payload =
                json!({ "from": role_names(&before), "to": role_names(&roles), "reason": "admin" });
            audit::record(
                &mut *tx,
                actor,
                audit::ROLES_CHANGED,
                subject.clone(),
                payload,
            )
            .await?;
        }
    }
    let mut disabled = user.disabled;
    if let Some(wanted) = patch.disabled {
        if myself && wanted {
            return Err(AppError::Conflict {
                code: "cannot_disable_self",
                message: "нельзя заблокировать самого себя".into(),
            });
        }
        if wanted != disabled {
            disabled = wanted;
            let mut payload = json!({});
            if disabled {
                // Блокировка действует сразу: все сессии и устройства человека отзываются
                let revoked = session::revoke_all(&mut tx, user.id).await?;
                payload = json!({ "revokedSessions": revoked });
            }
            let kind = if disabled {
                audit::USER_DISABLED
            } else {
                audit::USER_ENABLED
            };
            audit::record(&mut *tx, actor, kind, subject.clone(), payload).await?;
        }
    }

    let mut name = None;
    if let Some(wanted) = patch.name {
        let wanted: String = wanted
            .trim()
            .chars()
            .filter(|c| !c.is_control())
            .take(128)
            .collect();
        if wanted.is_empty() {
            return Err(bad("имя не может быть пустым"));
        }
        if wanted != user.name {
            let payload = json!({ "from": user.name, "to": wanted });
            audit::record(
                &mut *tx,
                actor,
                audit::USER_RENAMED,
                subject.clone(),
                payload,
            )
            .await?;
            name = Some(wanted);
        }
    }

    let updated = sqlx::query_as!(
        UserRow,
        r#"UPDATE users SET roles = $2, idp_admin = $3, disabled = $4, updated_at = now(),
               idp_admin_issuer = CASE WHEN $3 THEN idp_admin_issuer END,
               disabled_at = CASE WHEN NOT $4 THEN NULL WHEN disabled THEN disabled_at ELSE now() END,
               name = COALESCE($5, name), name_locked = name_locked OR $5::text IS NOT NULL
           WHERE id = $1
           RETURNING id, issuer, email, email_verified, name, avatar_url, roles, idp_admin, disabled, created_at,
                     last_login_at, username, password_hash IS NOT NULL AS "has_password!""#,
        user.id,
        &role_names(&roles),
        idp_admin,
        disabled,
        name,
    )
    .fetch_one(&mut *tx)
    .await?;
    tx.commit().await?;
    let mut providers = users::provider_titles(&st, &[updated.id]).await?;
    Ok(Json(updated.admin_view(
        providers.remove(&updated.id).unwrap_or_default(),
    )))
}

struct SessionRow {
    id: Uuid,
    user_id: Uuid,
    user_name: String,
    kind: String,
    auth_method: String,
    created_at: DateTime<Utc>,
    last_seen_at: DateTime<Utc>,
    expires_at: DateTime<Utc>,
    user_agent: Option<String>,
    ip: Option<IpNet>,
    device_id: Option<Uuid>,
    device_name: Option<String>,
    device_platform: Option<String>,
    device_app_version: Option<String>,
    device_created_at: Option<DateTime<Utc>>,
    device_last_seen_at: Option<DateTime<Utc>>,
}

impl SessionRow {
    fn view(self, current: Uuid) -> SessionInfo {
        let kind = if self.kind == "device" {
            SessionKind::Device
        } else {
            SessionKind::Web
        };
        let via = match (kind, self.auth_method.as_str()) {
            (SessionKind::Device, _) => AuthVia::Device,
            (_, "password") => AuthVia::Password,
            (_, "local") => AuthVia::Local,
            _ => AuthVia::Oidc,
        };
        let device = match (self.device_id, self.device_name, self.device_created_at) {
            (Some(id), Some(name), Some(created_at)) => Some(Device {
                id,
                name,
                platform: self
                    .device_platform
                    .and_then(|p| p.parse().ok())
                    .unwrap_or(Platform::Web),
                app_version: self.device_app_version,
                created_at,
                last_seen_at: self.device_last_seen_at,
            }),
            _ => None,
        };
        SessionInfo {
            id: self.id,
            user_id: self.user_id,
            user_name: self.user_name,
            kind,
            via,
            device,
            created_at: self.created_at,
            last_seen_at: self.last_seen_at,
            expires_at: self.expires_at,
            user_agent: self.user_agent,
            ip: self.ip.map(|ip| ip.addr().to_string()),
            current: self.id == current,
        }
    }
}

#[utoipa::path(
    get,
    path = "/api/admin/sessions",
    tag = "admin",
    security(("session" = []), ("device" = [])),
    params(SessionsQuery),
    responses(
        (status = 200, description = "Живые сессии, свежие первыми", body = SessionList),
        (status = 403, description = "Нет права admin", body = ErrorBody),
    ),
)]
pub async fn list_sessions(
    State(st): State<AppState>,
    principal: Principal,
    Query(q): Query<SessionsQuery>,
) -> AppResult<Json<SessionList>> {
    let rows = sqlx::query_as!(
        SessionRow,
        r#"SELECT s.id, s.user_id, u.name AS user_name, s.kind, s.auth_method,
                  s.created_at, s.last_seen_at, s.expires_at, s.user_agent, s.ip,
                  d.id AS "device_id?", d.name AS "device_name?", d.platform AS "device_platform?",
                  d.app_version AS device_app_version, d.created_at AS "device_created_at?",
                  d.last_seen_at AS device_last_seen_at
           FROM sessions s
           JOIN users u ON u.id = s.user_id
           LEFT JOIN devices d ON d.id = s.device_id
           WHERE s.revoked_at IS NULL AND s.expires_at > now() AND ($1::uuid IS NULL OR s.user_id = $1)
           ORDER BY s.last_seen_at DESC"#,
        q.user_id,
    )
    .fetch_all(&st.db)
    .await?;
    let items = rows
        .into_iter()
        .map(|r| r.view(principal.session_id))
        .collect();
    Ok(Json(SessionList {
        items,
        next_cursor: None,
    }))
}

#[utoipa::path(
    delete,
    path = "/api/admin/sessions/{id}",
    tag = "admin",
    security(("session" = []), ("device" = [])),
    params(("id" = Uuid, Path, description = "id сессии")),
    responses(
        (status = 204, description = "Сессия отозвана; у приложения отозвано и устройство"),
        (status = 404, description = "Нет такой живой сессии", body = ErrorBody),
    ),
)]
pub async fn revoke_session(
    State(st): State<AppState>,
    principal: Principal,
    Path(id): Path<Uuid>,
) -> AppResult<StatusCode> {
    let mut tx = st.db.begin().await?;
    let revoked = session::revoke(&mut tx, id)
        .await?
        .ok_or(AppError::NotFound)?;
    let payload = json!({ "userId": revoked.user_id, "deviceId": revoked.device_id });
    audit::record(
        &mut *tx,
        Some(principal.user.id),
        audit::SESSION_REVOKED,
        Some(revoked.id.to_string()),
        payload,
    )
    .await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

struct AuditRow {
    id: Uuid,
    at: DateTime<Utc>,
    actor_user_id: Option<Uuid>,
    actor_name: Option<String>,
    kind: String,
    subject: Option<String>,
    payload: Value,
}

#[utoipa::path(
    get,
    path = "/api/admin/audit",
    tag = "admin",
    security(("session" = []), ("device" = [])),
    params(AuditQuery),
    responses(
        (status = 200, description = "События журнала, новые первыми", body = AuditPage),
        (status = 400, description = "Неверный cursor или limit", body = ErrorBody),
    ),
)]
pub async fn audit_log(
    State(st): State<AppState>,
    Query(q): Query<AuditQuery>,
) -> AppResult<Json<AuditPage>> {
    let limit = q.limit.unwrap_or(50);
    if !(1..=200).contains(&limit) {
        return Err(bad("limit должен быть от 1 до 200"));
    }
    let cursor = match q.cursor.as_deref() {
        Some(c) => Some(c.parse::<Uuid>().map_err(|_| bad("неверный cursor"))?),
        None => None,
    };
    let mut rows = sqlx::query_as!(
        AuditRow,
        r#"SELECT e.id, e.at, e.actor_user_id, u.name AS "actor_name?", e.kind, e.subject, e.payload
           FROM audit_events e
           LEFT JOIN users u ON u.id = e.actor_user_id
           WHERE ($1::uuid IS NULL OR e.id < $1)
             AND ($3::text IS NULL OR e.kind = $3 OR (right($3, 1) = '.' AND starts_with(e.kind, $3)))
             AND ($4::uuid IS NULL OR e.actor_user_id = $4)
           ORDER BY e.id DESC
           LIMIT $2"#,
        cursor,
        i64::from(limit) + 1,
        q.kind.as_deref().map(str::trim).filter(|k| !k.is_empty()),
        q.actor,
    )
    .fetch_all(&st.db)
    .await?;
    // Лишняя строка говорит, что есть следующая страница
    let next_cursor = if rows.len() > limit as usize {
        rows.truncate(limit as usize);
        rows.last().map(|r| r.id.to_string())
    } else {
        None
    };
    let items = rows
        .into_iter()
        .map(|r| AuditEvent {
            id: r.id,
            at: r.at,
            actor: r
                .actor_user_id
                .zip(r.actor_name)
                .map(|(id, name)| AuditActor { id, name }),
            kind: r.kind,
            subject: r.subject,
            payload: r.payload,
        })
        .collect();
    Ok(Json(AuditPage { items, next_cursor }))
}
