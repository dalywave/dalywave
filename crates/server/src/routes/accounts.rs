//! Свои аккаунты (`DALYWAVE_LOCAL_AUTH`): вход логином и паролем, мастер первого запуска,
//! приглашения, одноразовые ссылки сброса пароля, «Вход» в настройках. Админке — способы входа,
//! новые аккаунты, приглашения и ссылки сброса (`/api/admin/*`, право admin).
//!
//! Вход приложения через системный браузер работает и здесь: если в браузере есть cookie `dw_app`
//! (её ставит `/api/auth/app/start`) и запрос пришёл с `app: true`, вместо сессии сайта ответ несёт
//! адрес возврата в приложение с одноразовым кодом.

use std::{net::IpAddr, time::Duration};

use axum::{
    Router,
    extract::State,
    http::{HeaderMap, StatusCode},
    routing::{delete, get, post},
};
use axum_extra::extract::cookie::{Cookie, CookieJar, PrivateCookieJar};
use chrono::Utc;
use dalywave_protocol::{
    ErrorBody,
    admin::{
        AuthOverview, CreatedInvite, CreatedUser, Invite, InviteList, InviteStatus, NewInvite,
        NewUser, OneTimeLink, ProviderInfo,
    },
    auth::{
        Account, AuthVia, InviteInfo, LinkedIdentity, LocalLogin, NewAccount, OidcProvider,
        PasswordChange, ResetInfo, ResetPassword, Role, SignInResult,
    },
};
use serde_json::json;
use uuid::Uuid;

use crate::{
    accounts::{
        self, INVITE_DEFAULT_DAYS, INVITE_MAX_DAYS, RESET_HOURS, check_password,
        normalize_username, username_conflict,
    },
    audit,
    auth::Principal,
    crypto::{random_token, sha256},
    error::{AppError, AppResult},
    extract::{Json, Path},
    routes::auth::{
        APP_COOKIE, OIDC_COOKIE_PATH, app_cookie, issue_app_code, me_for, open_web_session,
    },
    session::{self, Method},
    state::{AppState, ClientIp},
    users::{self, UserRow, parse_roles, role_names},
};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/local/login", post(local_login))
        .route("/setup", post(setup))
        .route("/invites/{token}", get(invite_info).post(accept_invite))
        .route("/reset/{token}", get(reset_info).post(reset_password))
        .route("/account", get(account))
        .route("/account/password", post(change_password))
        .route("/account/identities/{id}", delete(unlink_identity))
}

pub fn admin_routes() -> Router<AppState> {
    Router::new()
        .route("/auth", get(auth_overview))
        .route("/users", post(create_user))
        .route("/users/{id}/reset", post(reset_link))
        .route("/invites", get(list_invites).post(create_invite))
        .route("/invites/{id}", delete(revoke_invite))
}

fn local_enabled(st: &AppState) -> AppResult<()> {
    if st.config.local_auth {
        Ok(())
    } else {
        Err(AppError::PasswordDisabled)
    }
}

fn limited(ok: bool) -> AppResult<()> {
    if ok {
        Ok(())
    } else {
        Err(AppError::TooManyRequests)
    }
}

fn invite_invalid() -> AppError {
    AppError::Missing {
        code: "invite_invalid",
        message: "приглашение не найдено, уже использовано или истекло".into(),
    }
}

fn reset_invalid() -> AppError {
    AppError::Missing {
        code: "reset_invalid",
        message: "ссылка не найдена, уже использована или истекла".into(),
    }
}

/// Завершает вход своим аккаунтом: сессия сайта или (вход приложения) код на адрес приложения.
async fn finish(
    st: &AppState,
    ip: IpAddr,
    headers: &HeaderMap,
    jar: CookieJar,
    private: PrivateCookieJar,
    user: &UserRow,
    app: bool,
) -> AppResult<(PrivateCookieJar, CookieJar, Json<SignInResult>)> {
    if user.disabled {
        return Err(AppError::Forbidden("аккаунт заблокирован".into()));
    }
    if app {
        let expired = || AppError::Unprocessable {
            code: "app_expired",
            message: "вход приложения устарел: начните его заново в приложении".into(),
        };
        let id = app_cookie(&private).ok_or_else(expired)?;
        let location = issue_app_code(st, id, user.id, Method::Local)
            .await?
            .ok_or_else(expired)?;
        let private = private.remove(Cookie::build(APP_COOKIE).path(OIDC_COOKIE_PATH));
        return Ok((
            private,
            jar,
            Json(SignInResult {
                me: None,
                redirect: Some(location),
            }),
        ));
    }
    let (jar, session_id) = open_web_session(st, jar, headers, ip, user.id, Method::Local).await?;
    Ok((
        private,
        jar,
        Json(SignInResult {
            me: Some(me_for(user, AuthVia::Local, session_id)),
            redirect: None,
        }),
    ))
}

async fn user_row(st: &AppState, id: Uuid) -> AppResult<UserRow> {
    users::get(st, id).await?.ok_or(AppError::NotFound)
}

// ---------- вход ----------

#[utoipa::path(
    post,
    path = "/api/auth/local/login",
    tag = "auth",
    request_body = LocalLogin,
    responses(
        (status = 200, description = "Сессия сайта (`me`, cookie `dw_session`) или адрес возврата в приложение", body = SignInResult),
        (status = 401, description = "Логин или пароль не подошли (`wrong_password`)", body = ErrorBody),
        (status = 403, description = "Свои аккаунты выключены (`password_disabled`) или аккаунт заблокирован", body = ErrorBody),
        (status = 422, description = "Вход приложения устарел (`app_expired`)", body = ErrorBody),
        (status = 429, description = "Больше 5 попыток в минуту для логина с одного адреса", body = ErrorBody),
    ),
)]
pub async fn local_login(
    State(st): State<AppState>,
    ClientIp(ip): ClientIp,
    headers: HeaderMap,
    jar: CookieJar,
    private: PrivateCookieJar,
    Json(body): Json<LocalLogin>,
) -> AppResult<(PrivateCookieJar, CookieJar, Json<SignInResult>)> {
    local_enabled(&st)?;
    let username = body.username.trim().to_lowercase();
    limited(
        st.limiter
            .check("local-login", ip, 30, Duration::from_secs(60)),
    )?;
    limited(st.limiter.check_key(
        "local-login-user",
        &format!("{ip}|{username}"),
        5,
        Duration::from_secs(60),
    ))?;
    let found = accounts::find_by_username(&st.db, &username).await?;
    let hash = found.as_ref().and_then(|u| u.password_hash.clone());
    if !accounts::verify_password(body.password, hash).await {
        let payload = json!({ "via": "local", "username": username, "ip": ip.to_string() });
        audit::record(&st.db, None, audit::SIGN_IN_FAILED, None, payload).await?;
        return Err(AppError::WrongPassword);
    }
    let Some(found) = found else {
        return Err(AppError::WrongPassword);
    };
    if found.disabled {
        let payload = json!({ "via": "local", "reason": "disabled", "ip": ip.to_string() });
        audit::record(
            &st.db,
            None,
            audit::SIGN_IN_DENIED,
            Some(found.id.to_string()),
            payload,
        )
        .await?;
    }
    let user = user_row(&st, found.id).await?;
    finish(&st, ip, &headers, jar, private, &user, body.app).await
}

#[utoipa::path(
    post,
    path = "/api/auth/setup",
    tag = "auth",
    request_body = NewAccount,
    responses(
        (status = 200, description = "Первый запуск: администратор создан, вход выполнен", body = SignInResult),
        (status = 403, description = "Свои аккаунты выключены (`password_disabled`)", body = ErrorBody),
        (status = 409, description = "Люди уже есть (`already_set_up`)", body = ErrorBody),
        (status = 422, description = "Логин (`bad_username`) или пароль (`weak_password`) не подходят", body = ErrorBody),
    ),
)]
pub async fn setup(
    State(st): State<AppState>,
    ClientIp(ip): ClientIp,
    headers: HeaderMap,
    jar: CookieJar,
    private: PrivateCookieJar,
    Json(body): Json<NewAccount>,
) -> AppResult<(PrivateCookieJar, CookieJar, Json<SignInResult>)> {
    local_enabled(&st)?;
    limited(st.limiter.check("setup", ip, 10, Duration::from_secs(60)))?;
    let username = normalize_username(&body.username)?;
    check_password(&body.password)?;
    let name = accounts::display_name(body.name.as_deref(), &username);
    let hash = accounts::hash_password(body.password).await?;

    let mut tx = st.db.begin().await?;
    // Два первых запуска одновременно: администратором станет только один
    sqlx::query!("SELECT pg_advisory_xact_lock(hashtext('dalywave.setup'))")
        .execute(&mut *tx)
        .await?;
    let empty = sqlx::query_scalar!(r#"SELECT NOT EXISTS (SELECT 1 FROM users) AS "empty!""#)
        .fetch_one(&mut *tx)
        .await?;
    if !empty {
        return Err(AppError::Conflict {
            code: "already_set_up",
            message: "администратор уже создан".into(),
        });
    }
    let id = sqlx::query_scalar!(
        "INSERT INTO users (name, username, password_hash, password_changed_at, roles, last_login_at)
         VALUES ($1, $2, $3, now(), '{admin}', now()) RETURNING id",
        name,
        username,
        hash,
    )
    .fetch_one(&mut *tx)
    .await?;
    let payload =
        json!({ "name": name, "username": username, "reason": "setup", "ip": ip.to_string() });
    audit::record(
        &mut *tx,
        None,
        audit::USER_CREATED,
        Some(id.to_string()),
        payload,
    )
    .await?;
    tx.commit().await?;
    let user = user_row(&st, id).await?;
    finish(&st, ip, &headers, jar, private, &user, body.app).await
}

// ---------- приглашения ----------

#[utoipa::path(
    get,
    path = "/api/auth/invites/{token}",
    tag = "auth",
    params(("token" = String, Path, description = "Токен из ссылки-приглашения")),
    responses(
        (status = 200, description = "Приглашение действует", body = InviteInfo),
        (status = 404, description = "Нет, использовано или истекло (`invite_invalid`)", body = ErrorBody),
    ),
)]
pub async fn invite_info(
    State(st): State<AppState>,
    ClientIp(ip): ClientIp,
    Path(token): Path<String>,
) -> AppResult<Json<InviteInfo>> {
    limited(st.limiter.check("invite", ip, 30, Duration::from_secs(60)))?;
    let invite = accounts::find_invite(&st.db, &token)
        .await?
        .ok_or_else(invite_invalid)?;
    Ok(Json(InviteInfo {
        roles: invite.roles,
        expires_at: invite.expires_at,
    }))
}

#[utoipa::path(
    post,
    path = "/api/auth/invites/{token}",
    tag = "auth",
    params(("token" = String, Path, description = "Токен из ссылки-приглашения")),
    request_body = NewAccount,
    responses(
        (status = 200, description = "Аккаунт создан с ролями приглашения, вход выполнен", body = SignInResult),
        (status = 403, description = "Свои аккаунты выключены (`password_disabled`)", body = ErrorBody),
        (status = 404, description = "Приглашение не действует (`invite_invalid`)", body = ErrorBody),
        (status = 409, description = "Логин занят (`username_taken`)", body = ErrorBody),
        (status = 422, description = "Логин или пароль не подходят", body = ErrorBody),
    ),
)]
pub async fn accept_invite(
    State(st): State<AppState>,
    ClientIp(ip): ClientIp,
    headers: HeaderMap,
    jar: CookieJar,
    private: PrivateCookieJar,
    Path(token): Path<String>,
    Json(body): Json<NewAccount>,
) -> AppResult<(PrivateCookieJar, CookieJar, Json<SignInResult>)> {
    local_enabled(&st)?;
    limited(st.limiter.check("invite", ip, 30, Duration::from_secs(60)))?;
    let username = normalize_username(&body.username)?;
    check_password(&body.password)?;
    let name = accounts::display_name(body.name.as_deref(), &username);
    if accounts::find_invite(&st.db, &token).await?.is_none() {
        return Err(invite_invalid());
    }
    let hash = accounts::hash_password(body.password).await?;

    let mut tx = st.db.begin().await?;
    let invite = accounts::find_invite(&mut *tx, &token)
        .await?
        .ok_or_else(invite_invalid)?;
    if accounts::username_taken(&mut *tx, &username, None).await? {
        return Err(username_conflict());
    }
    let roles = role_names(&invite.roles);
    let id = sqlx::query_scalar!(
        "INSERT INTO users (name, username, password_hash, password_changed_at, roles, last_login_at)
         VALUES ($1, $2, $3, now(), $4, now()) RETURNING id",
        name,
        username,
        hash,
        &roles,
    )
    .fetch_one(&mut *tx)
    .await?;
    if !accounts::use_invite(&mut *tx, invite.id, id).await? {
        return Err(invite_invalid());
    }
    let payload = json!({ "name": name, "username": username, "reason": "invite", "roles": roles });
    audit::record(
        &mut *tx,
        None,
        audit::USER_CREATED,
        Some(id.to_string()),
        payload,
    )
    .await?;
    audit::record(
        &mut *tx,
        Some(id),
        audit::INVITE_USED,
        Some(invite.id.to_string()),
        json!({ "userId": id }),
    )
    .await?;
    tx.commit().await?;
    let user = user_row(&st, id).await?;
    finish(&st, ip, &headers, jar, private, &user, body.app).await
}

// ---------- сброс пароля ----------

struct ResetRow {
    id: Uuid,
    user_id: Uuid,
    name: String,
    username: Option<String>,
    expires_at: chrono::DateTime<Utc>,
}

#[utoipa::path(
    get,
    path = "/api/auth/reset/{token}",
    tag = "auth",
    params(("token" = String, Path, description = "Токен из ссылки сброса")),
    responses(
        (status = 200, description = "Для кого ссылка", body = ResetInfo),
        (status = 404, description = "Нет, использована или истекла (`reset_invalid`)", body = ErrorBody),
    ),
)]
pub async fn reset_info(
    State(st): State<AppState>,
    ClientIp(ip): ClientIp,
    Path(token): Path<String>,
) -> AppResult<Json<ResetInfo>> {
    limited(st.limiter.check("reset", ip, 30, Duration::from_secs(60)))?;
    let row = sqlx::query_as!(
        ResetRow,
        "SELECT r.id, r.user_id, u.name, u.username, r.expires_at
         FROM password_resets r JOIN users u ON u.id = r.user_id
         WHERE r.token_hash = $1 AND r.used_at IS NULL AND r.expires_at > now()",
        &sha256(&token)[..],
    )
    .fetch_optional(&st.db)
    .await?
    .ok_or_else(reset_invalid)?;
    Ok(Json(ResetInfo {
        name: row.name,
        username: row.username,
        expires_at: row.expires_at,
    }))
}

#[utoipa::path(
    post,
    path = "/api/auth/reset/{token}",
    tag = "auth",
    params(("token" = String, Path, description = "Токен из ссылки сброса")),
    request_body = ResetPassword,
    responses(
        (status = 200, description = "Пароль задан, прежние сессии закрыты, вход выполнен", body = SignInResult),
        (status = 403, description = "Свои аккаунты выключены или аккаунт заблокирован", body = ErrorBody),
        (status = 404, description = "Ссылка не действует (`reset_invalid`)", body = ErrorBody),
        (status = 409, description = "Логин занят (`username_taken`)", body = ErrorBody),
        (status = 422, description = "Логин или пароль не подходят", body = ErrorBody),
    ),
)]
pub async fn reset_password(
    State(st): State<AppState>,
    ClientIp(ip): ClientIp,
    headers: HeaderMap,
    jar: CookieJar,
    private: PrivateCookieJar,
    Path(token): Path<String>,
    Json(body): Json<ResetPassword>,
) -> AppResult<(PrivateCookieJar, CookieJar, Json<SignInResult>)> {
    local_enabled(&st)?;
    limited(st.limiter.check("reset", ip, 30, Duration::from_secs(60)))?;
    check_password(&body.password)?;
    let hash = accounts::hash_password(body.password).await?;

    let mut tx = st.db.begin().await?;
    let row = sqlx::query_as!(
        ResetRow,
        "SELECT r.id, r.user_id, u.name, u.username, r.expires_at
         FROM password_resets r JOIN users u ON u.id = r.user_id
         WHERE r.token_hash = $1 AND r.used_at IS NULL AND r.expires_at > now()
         FOR UPDATE OF r",
        &sha256(&token)[..],
    )
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(reset_invalid)?;
    let username = match &row.username {
        Some(name) => name.clone(),
        None => {
            let name = normalize_username(body.username.as_deref().unwrap_or_default())?;
            if accounts::username_taken(&mut *tx, &name, Some(row.user_id)).await? {
                return Err(username_conflict());
            }
            name
        }
    };
    sqlx::query!(
        "UPDATE users SET username = $2, password_hash = $3, password_changed_at = now(), updated_at = now()
         WHERE id = $1",
        row.user_id,
        username,
        hash,
    )
    .execute(&mut *tx)
    .await?;
    sqlx::query!(
        "UPDATE password_resets SET used_at = now() WHERE id = $1",
        row.id
    )
    .execute(&mut *tx)
    .await?;
    // Ссылку выдают, когда пароль забыт или украден: все прежние сессии закрываются
    let revoked = session::revoke_all(&mut tx, row.user_id).await?;
    let payload = json!({ "reason": "reset", "revokedSessions": revoked, "ip": ip.to_string() });
    audit::record(
        &mut *tx,
        Some(row.user_id),
        audit::PASSWORD_CHANGED,
        Some(row.user_id.to_string()),
        payload,
    )
    .await?;
    tx.commit().await?;
    let user = user_row(&st, row.user_id).await?;
    finish(&st, ip, &headers, jar, private, &user, false).await
}

// ---------- свой аккаунт ----------

struct IdentityRow {
    id: Uuid,
    issuer: String,
    email: Option<String>,
    created_at: chrono::DateTime<Utc>,
    last_login_at: Option<chrono::DateTime<Utc>>,
}

#[utoipa::path(
    get,
    path = "/api/auth/account",
    tag = "auth",
    security(("session" = []), ("device" = [])),
    responses(
        (status = 200, description = "Логин, пароль и привязанные входы", body = Account),
        (status = 401, description = "Нет сессии", body = ErrorBody),
    ),
)]
pub async fn account(State(st): State<AppState>, principal: Principal) -> AppResult<Json<Account>> {
    let user = user_row(&st, principal.user.id).await?;
    let rows = sqlx::query_as!(
        IdentityRow,
        "SELECT id, issuer, email, created_at, last_login_at FROM user_identities
         WHERE user_id = $1 ORDER BY created_at",
        user.id,
    )
    .fetch_all(&st.db)
    .await?;
    let identities: Vec<LinkedIdentity> = rows
        .into_iter()
        .map(|r| {
            let provider = st.provider_by_issuer(&r.issuer);
            LinkedIdentity {
                id: r.id,
                provider: provider.map(|p| p.config.id.clone()),
                title: users::provider_title(&st, &r.issuer),
                icon: provider.and_then(|p| p.config.icon.clone()),
                email: r.email,
                created_at: r.created_at,
                last_login_at: r.last_login_at,
            }
        })
        .collect();
    let linkable = st
        .oidc
        .iter()
        .filter(|p| {
            !identities
                .iter()
                .any(|i| i.provider.as_deref() == Some(p.id()))
        })
        .map(|p| OidcProvider {
            id: p.config.id.clone(),
            title: p.config.title.clone(),
            icon: p.config.icon.clone(),
        })
        .collect();
    Ok(Json(Account {
        username: user.username,
        has_password: user.has_password,
        local: st.config.local_auth,
        identities,
        linkable,
    }))
}

#[utoipa::path(
    post,
    path = "/api/auth/account/password",
    tag = "auth",
    security(("session" = []), ("device" = [])),
    request_body = PasswordChange,
    responses(
        (status = 204, description = "Пароль задан или сменён"),
        (status = 401, description = "Нынешний пароль не подошёл (`wrong_password`)", body = ErrorBody),
        (status = 403, description = "Свои аккаунты выключены (`password_disabled`)", body = ErrorBody),
        (status = 409, description = "Логин занят (`username_taken`)", body = ErrorBody),
        (status = 422, description = "Логин или пароль не подходят", body = ErrorBody),
        (status = 429, description = "Больше 5 попыток в минуту", body = ErrorBody),
    ),
)]
pub async fn change_password(
    State(st): State<AppState>,
    principal: Principal,
    Json(body): Json<PasswordChange>,
) -> AppResult<StatusCode> {
    local_enabled(&st)?;
    let user_id = principal.user.id;
    limited(
        st.limiter
            .check_id("password-change", user_id, 5, Duration::from_secs(60)),
    )?;
    check_password(&body.password)?;
    let current = sqlx::query!(
        "SELECT username, password_hash FROM users WHERE id = $1",
        user_id
    )
    .fetch_one(&st.db)
    .await?;
    if current.password_hash.is_some()
        && !accounts::verify_password(
            body.current.unwrap_or_default(),
            current.password_hash.clone(),
        )
        .await
    {
        return Err(AppError::WrongPassword);
    }
    let username = match current.username {
        Some(name) => name,
        None => normalize_username(body.username.as_deref().unwrap_or_default())?,
    };
    let hash = accounts::hash_password(body.password).await?;
    let mut tx = st.db.begin().await?;
    if accounts::username_taken(&mut *tx, &username, Some(user_id)).await? {
        return Err(username_conflict());
    }
    sqlx::query!(
        "UPDATE users SET username = $2, password_hash = $3, password_changed_at = now(), updated_at = now()
         WHERE id = $1",
        user_id,
        username,
        hash,
    )
    .execute(&mut *tx)
    .await?;
    let reason = if current.password_hash.is_some() {
        "changed"
    } else {
        "set"
    };
    audit::record(
        &mut *tx,
        Some(user_id),
        audit::PASSWORD_CHANGED,
        Some(user_id.to_string()),
        json!({ "reason": reason }),
    )
    .await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    delete,
    path = "/api/auth/account/identities/{id}",
    tag = "auth",
    security(("session" = []), ("device" = [])),
    params(("id" = Uuid, Path, description = "id входа из `/api/auth/account`")),
    responses(
        (status = 204, description = "Вход отвязан"),
        (status = 404, description = "Нет такого входа у этого аккаунта", body = ErrorBody),
        (status = 409, description = "Это последний способ войти (`last_sign_in_method`)", body = ErrorBody),
    ),
)]
pub async fn unlink_identity(
    State(st): State<AppState>,
    principal: Principal,
    Path(id): Path<Uuid>,
) -> AppResult<StatusCode> {
    let user_id = principal.user.id;
    let mut tx = st.db.begin().await?;
    let row = sqlx::query!(
        "SELECT i.issuer, i.subject, u.password_hash IS NOT NULL AS \"has_password!\"
         FROM user_identities i JOIN users u ON u.id = i.user_id
         WHERE i.id = $1 AND i.user_id = $2 FOR UPDATE OF u",
        id,
        user_id,
    )
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(AppError::NotFound)?;
    let others = sqlx::query_scalar!(
        r#"SELECT count(*) AS "n!" FROM user_identities WHERE user_id = $1 AND id <> $2"#,
        user_id,
        id,
    )
    .fetch_one(&mut *tx)
    .await?;
    if others == 0 && !(row.has_password && st.config.local_auth) {
        return Err(AppError::Conflict {
            code: "last_sign_in_method",
            message: "это последний способ войти: сначала задайте пароль или привяжите другой вход"
                .into(),
        });
    }
    sqlx::query!("DELETE FROM user_identities WHERE id = $1", id)
        .execute(&mut *tx)
        .await?;
    // Прежний «основной» вход в самой строке человека тоже забывается: иначе отвязанный вход
    // нашёлся бы по старым колонкам
    sqlx::query!(
        "UPDATE users SET issuer = NULL, subject = NULL, updated_at = now()
         WHERE id = $1 AND issuer = $2 AND subject = $3",
        user_id,
        row.issuer,
        row.subject,
    )
    .execute(&mut *tx)
    .await?;
    audit::record(
        &mut *tx,
        Some(user_id),
        audit::IDENTITY_UNLINKED,
        Some(user_id.to_string()),
        json!({ "issuer": row.issuer }),
    )
    .await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

// ---------- админка ----------

#[utoipa::path(
    get,
    path = "/api/admin/auth",
    tag = "admin",
    security(("session" = []), ("device" = [])),
    responses(
        (status = 200, description = "Способы входа (только чтение: настраиваются переменными окружения)", body = AuthOverview),
        (status = 403, description = "Нет права admin", body = ErrorBody),
    ),
)]
pub async fn auth_overview(State(st): State<AppState>) -> AppResult<Json<AuthOverview>> {
    let counts = sqlx::query!(
        r#"SELECT issuer, count(DISTINCT user_id) AS "n!" FROM user_identities GROUP BY issuer"#
    )
    .fetch_all(&st.db)
    .await?;
    let providers = st
        .oidc
        .iter()
        .map(|p| {
            let c = &p.config;
            ProviderInfo {
                id: c.id.clone(),
                title: c.title.clone(),
                icon: c.icon.clone(),
                issuer: c.issuer.clone(),
                callback_url: st.config.oidc_redirect_uri(c),
                admin_claim: c.admin_claim.clone(),
                admin_values: c.admin_values.clone(),
                auto_register: c.auto_register,
                trust_email: c.trust_email,
                users: counts
                    .iter()
                    .filter(|r| r.issuer.trim_end_matches('/') == c.issuer)
                    .map(|r| r.n)
                    .sum(),
            }
        })
        .collect();
    Ok(Json(AuthOverview {
        providers,
        local: st.config.local_auth,
        emergency_password: st.config.admin_password.is_some(),
    }))
}

/// Новая одноразовая ссылка сброса; прежние неиспользованные ссылки человека гасятся.
async fn new_reset_link(
    st: &AppState,
    conn: &mut sqlx::PgConnection,
    user: Uuid,
    actor: Uuid,
) -> AppResult<OneTimeLink> {
    let token = random_token()?;
    sqlx::query!(
        "DELETE FROM password_resets WHERE user_id = $1 AND used_at IS NULL",
        user
    )
    .execute(&mut *conn)
    .await?;
    let expires_at = sqlx::query_scalar!(
        "INSERT INTO password_resets (user_id, token_hash, created_by, expires_at)
         VALUES ($1, $2, $3, now() + make_interval(hours => $4)) RETURNING expires_at",
        user,
        &sha256(&token)[..],
        actor,
        RESET_HOURS as i32,
    )
    .fetch_one(&mut *conn)
    .await?;
    audit::record(
        &mut *conn,
        Some(actor),
        audit::PASSWORD_RESET_LINK,
        Some(user.to_string()),
        json!({ "expiresAt": expires_at }),
    )
    .await?;
    Ok(OneTimeLink {
        url: st.config.page(&format!("/reset/{token}")),
        expires_at,
    })
}

#[utoipa::path(
    post,
    path = "/api/admin/users",
    tag = "admin",
    security(("session" = []), ("device" = [])),
    request_body = NewUser,
    responses(
        (status = 200, description = "Аккаунт создан; без пароля — со ссылкой, чтобы задать его", body = CreatedUser),
        (status = 403, description = "Свои аккаунты выключены (`password_disabled`)", body = ErrorBody),
        (status = 409, description = "Логин занят (`username_taken`)", body = ErrorBody),
        (status = 422, description = "Логин или пароль не подходят", body = ErrorBody),
    ),
)]
pub async fn create_user(
    State(st): State<AppState>,
    principal: Principal,
    Json(body): Json<NewUser>,
) -> AppResult<Json<CreatedUser>> {
    local_enabled(&st)?;
    let username = normalize_username(&body.username)?;
    let hash = match body.password {
        Some(password) => {
            check_password(&password)?;
            Some(accounts::hash_password(password).await?)
        }
        None => None,
    };
    let name = accounts::display_name(body.name.as_deref(), &username);
    let roles = role_names(&parse_roles(&role_names(&body.roles)));
    let actor = principal.user.id;
    let mut tx = st.db.begin().await?;
    if accounts::username_taken(&mut *tx, &username, None).await? {
        return Err(username_conflict());
    }
    let id = sqlx::query_scalar!(
        "INSERT INTO users (name, username, password_hash, password_changed_at, roles)
         VALUES ($1, $2, $3, CASE WHEN $3::text IS NULL THEN NULL ELSE now() END, $4) RETURNING id",
        name,
        username,
        hash,
        &roles,
    )
    .fetch_one(&mut *tx)
    .await?;
    audit::record(
        &mut *tx,
        Some(actor),
        audit::USER_CREATED,
        Some(id.to_string()),
        json!({ "name": name, "username": username, "roles": roles, "reason": "admin" }),
    )
    .await?;
    let link = match hash {
        Some(_) => None,
        None => Some(new_reset_link(&st, &mut tx, id, actor).await?),
    };
    tx.commit().await?;
    let user = user_row(&st, id).await?;
    Ok(Json(CreatedUser {
        user: user.admin_view(Vec::new()),
        link,
    }))
}

#[utoipa::path(
    post,
    path = "/api/admin/users/{id}/reset",
    tag = "admin",
    security(("session" = []), ("device" = [])),
    params(("id" = Uuid, Path, description = "id пользователя")),
    responses(
        (status = 200, description = "Одноразовая ссылка: задать пароль (и логин, если его нет)", body = OneTimeLink),
        (status = 403, description = "Свои аккаунты выключены (`password_disabled`)", body = ErrorBody),
        (status = 404, description = "Нет такого пользователя", body = ErrorBody),
    ),
)]
pub async fn reset_link(
    State(st): State<AppState>,
    principal: Principal,
    Path(id): Path<Uuid>,
) -> AppResult<Json<OneTimeLink>> {
    local_enabled(&st)?;
    let user = user_row(&st, id).await?;
    let mut tx = st.db.begin().await?;
    let link = new_reset_link(&st, &mut tx, user.id, principal.user.id).await?;
    tx.commit().await?;
    Ok(Json(link))
}

struct InviteRow {
    id: Uuid,
    roles: Vec<String>,
    note: Option<String>,
    created_at: chrono::DateTime<Utc>,
    expires_at: chrono::DateTime<Utc>,
    used_at: Option<chrono::DateTime<Utc>>,
    revoked_at: Option<chrono::DateTime<Utc>>,
    used_by: Option<String>,
}

impl InviteRow {
    fn view(self) -> Invite {
        let status = if self.used_at.is_some() {
            InviteStatus::Used
        } else if self.revoked_at.is_some() {
            InviteStatus::Revoked
        } else if self.expires_at <= Utc::now() {
            InviteStatus::Expired
        } else {
            InviteStatus::Active
        };
        Invite {
            id: self.id,
            roles: parse_roles(&self.roles),
            note: self.note,
            status,
            created_at: self.created_at,
            expires_at: self.expires_at,
            used_by: self.used_by,
        }
    }
}

#[utoipa::path(
    get,
    path = "/api/admin/invites",
    tag = "admin",
    security(("session" = []), ("device" = [])),
    responses(
        (status = 200, description = "Приглашения: действующие и последние использованные", body = InviteList),
        (status = 403, description = "Нет права admin", body = ErrorBody),
    ),
)]
pub async fn list_invites(State(st): State<AppState>) -> AppResult<Json<InviteList>> {
    let rows = sqlx::query_as!(
        InviteRow,
        r#"SELECT i.id, i.roles, i.note, i.created_at, i.expires_at, i.used_at, i.revoked_at,
                  u.name AS "used_by?"
           FROM invites i LEFT JOIN users u ON u.id = i.used_by
           WHERE i.created_at > now() - interval '90 days'
           ORDER BY i.created_at DESC LIMIT 100"#,
    )
    .fetch_all(&st.db)
    .await?;
    Ok(Json(InviteList {
        items: rows.into_iter().map(InviteRow::view).collect(),
    }))
}

#[utoipa::path(
    post,
    path = "/api/admin/invites",
    tag = "admin",
    security(("session" = []), ("device" = [])),
    request_body = NewInvite,
    responses(
        (status = 200, description = "Ссылка-приглашение; показывается один раз", body = CreatedInvite),
        (status = 403, description = "Нет права admin", body = ErrorBody),
    ),
)]
pub async fn create_invite(
    State(st): State<AppState>,
    principal: Principal,
    Json(body): Json<NewInvite>,
) -> AppResult<Json<CreatedInvite>> {
    let days = body
        .days
        .unwrap_or(INVITE_DEFAULT_DAYS)
        .clamp(1, INVITE_MAX_DAYS);
    let roles: Vec<Role> = parse_roles(&role_names(&body.roles));
    let note = body
        .note
        .map(|n| n.trim().chars().take(120).collect::<String>())
        .filter(|n| !n.is_empty());
    let token = random_token()?;
    let mut tx = st.db.begin().await?;
    let row = sqlx::query_as!(
        InviteRow,
        r#"INSERT INTO invites (token_hash, roles, note, created_by, expires_at)
           VALUES ($1, $2, $3, $4, now() + make_interval(days => $5))
           RETURNING id, roles, note, created_at, expires_at, used_at, revoked_at, NULL::text AS "used_by?""#,
        &sha256(&token)[..],
        &role_names(&roles),
        note,
        principal.user.id,
        days as i32,
    )
    .fetch_one(&mut *tx)
    .await?;
    audit::record(
        &mut *tx,
        Some(principal.user.id),
        audit::INVITE_CREATED,
        Some(row.id.to_string()),
        json!({ "roles": role_names(&roles), "days": days }),
    )
    .await?;
    tx.commit().await?;
    Ok(Json(CreatedInvite {
        invite: row.view(),
        url: st.config.page(&format!("/invite/{token}")),
    }))
}

#[utoipa::path(
    delete,
    path = "/api/admin/invites/{id}",
    tag = "admin",
    security(("session" = []), ("device" = [])),
    params(("id" = Uuid, Path, description = "id приглашения")),
    responses(
        (status = 204, description = "Приглашение отозвано"),
        (status = 404, description = "Нет действующего приглашения", body = ErrorBody),
    ),
)]
pub async fn revoke_invite(
    State(st): State<AppState>,
    principal: Principal,
    Path(id): Path<Uuid>,
) -> AppResult<StatusCode> {
    let mut tx = st.db.begin().await?;
    let done = sqlx::query!(
        "UPDATE invites SET revoked_at = now() WHERE id = $1 AND used_at IS NULL AND revoked_at IS NULL",
        id,
    )
    .execute(&mut *tx)
    .await?;
    if done.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }
    audit::record(
        &mut *tx,
        Some(principal.user.id),
        audit::INVITE_REVOKED,
        Some(id.to_string()),
        json!({}),
    )
    .await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}
