//! Вход: daly.rest на сайте, аварийный пароль и вход приложений через системный браузер.
//!
//! Сайт: `/api/auth/oidc/start` → daly.rest → `/api/auth/oidc/callback` → cookie `dw_session` → `next`.
//! Приложение: `/api/auth/app/start` → тот же круг через daly.rest → одноразовый код на redirect_uri
//! приложения → `POST /api/auth/app/token` → токен устройства `dwd_…`. Код от daly.rest меняется
//! на токены только здесь, на сервере: секрет клиента есть только у него.
//! Часы: своего входа нет — вошедший iPhone просит `POST /api/auth/app/pair` и передаёт им токен.

use std::{net::IpAddr, time::Duration};

use axum::{
    Router,
    extract::State,
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use axum_extra::extract::cookie::{Cookie, CookieJar, PrivateCookieJar, SameSite};
use chrono::Utc;
use dalywave_protocol::{
    ErrorBody,
    auth::{
        AppStartQuery, AppToken, AppTokenRequest, AuthProviders, AuthVia, Me, OidcProvider,
        OidcStartQuery, PairRequest, PasswordLogin, Platform, SessionKind,
    },
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use utoipa::IntoParams;
use uuid::Uuid;

use crate::{
    audit,
    auth::{self, Principal, permissions},
    crypto::{pkce_challenge, random_token, secrets_match, sha256, valid_pkce},
    devices,
    error::{AppError, AppResult, bad},
    extract::{Json, Query},
    oidc::OidcError,
    session::{self, Method, NewSession, SESSION_COOKIE},
    state::{AppState, ClientIp},
    users::{self, UserRow},
};

pub const OIDC_COOKIE: &str = "dw_oidc";
pub const OIDC_COOKIE_PATH: &str = "/api/auth";
/// Сколько ждём возврата от daly.rest
pub const OIDC_TTL_S: i64 = 600;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/providers", get(providers))
        .route("/oidc/start", get(oidc_start))
        .route("/oidc/callback", get(oidc_callback))
        .route("/password", post(password_login))
        .route("/logout", post(logout))
        .route("/me", get(me))
        .route("/app/start", get(app_start))
        .route("/app/token", post(app_token))
        .route("/app/pair", post(app_pair))
}

/// Незаконченный вход. Лежит в зашифрованной cookie `dw_oidc`: state, nonce и PKCE verifier
/// известны только этому браузеру, поэтому ссылку возврата нельзя доиграть в чужом.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pending {
    pub state: String,
    pub nonce: String,
    pub verifier: String,
    pub next: String,
    /// Запрос приложения, если вход начат из него
    pub app: Option<Uuid>,
    /// Unix-время, после которого вход надо начинать заново
    pub exp: i64,
}

fn found(location: &str) -> Response {
    (StatusCode::FOUND, [(header::LOCATION, location)]).into_response()
}

/// Добавляет параметры к адресу возврата приложения.
fn with_params(base: &str, params: &[(&str, &str)]) -> String {
    match url::Url::parse(base) {
        Ok(mut url) => {
            url.query_pairs_mut().extend_pairs(params);
            url.into()
        }
        Err(_) => base.to_string(),
    }
}

/// `next` — только путь на этом же сайте: иначе вход можно превратить в открытый редирект.
pub fn local_path(next: &str) -> Option<&str> {
    let ok = next.starts_with('/')
        && !next.starts_with("//")
        && !next.contains('\\')
        && !next.chars().any(char::is_control)
        && next.len() <= 512;
    ok.then_some(next)
}

/// Куда можно вернуть код приложения: своя схема или порт на этой же машине.
pub fn app_redirect_allowed(uri: &str) -> bool {
    if uri == "dalywave://auth" {
        return true;
    }
    let Ok(url) = url::Url::parse(uri) else {
        return false;
    };
    // Адрес должен быть уже в каноническом виде: так приложение получит ровно то, что прислало
    url.as_str() == uri
        && url.scheme() == "http"
        && matches!(url.host_str(), Some("127.0.0.1" | "localhost"))
        && url.port().is_some()
        && url.path() == "/callback"
        && url.query().is_none()
        && url.fragment().is_none()
        && url.username().is_empty()
        && url.password().is_none()
}

fn user_agent(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(header::USER_AGENT)
        .and_then(|v| v.to_str().ok())
}

fn me_for(user: &UserRow, via: AuthVia, session_id: Uuid) -> Me {
    let roles = user.roles();
    Me {
        user: user.public(),
        permissions: permissions(&roles),
        roles,
        via,
        session_id,
        device: None,
    }
}

// ---------- способы входа ----------

#[utoipa::path(
    get,
    path = "/api/auth/providers",
    tag = "auth",
    responses((status = 200, description = "Какие способы входа включены", body = AuthProviders)),
)]
pub async fn providers(State(st): State<AppState>) -> Json<AuthProviders> {
    Json(AuthProviders {
        oidc: st.oidc.as_ref().map(|o| OidcProvider {
            title: o.config.title.clone(),
        }),
        password: st.config.admin_password.is_some(),
    })
}

// ---------- daly.rest ----------

#[utoipa::path(
    get,
    path = "/api/auth/oidc/start",
    tag = "auth",
    params(OidcStartQuery),
    responses(
        (status = 302, description = "На страницу входа daly.rest; ставит cookie `dw_oidc` на 10 минут"),
        (status = 400, description = "`next` — не путь на этом сайте", body = ErrorBody),
        (status = 404, description = "Вход через daly.rest не настроен", body = ErrorBody),
    ),
)]
pub async fn oidc_start(
    State(st): State<AppState>,
    jar: PrivateCookieJar,
    Query(q): Query<OidcStartQuery>,
) -> AppResult<(PrivateCookieJar, Response)> {
    let next = match q.next.as_deref() {
        None | Some("") => "/",
        Some(next) => local_path(next)
            .ok_or_else(|| bad("next должен быть путём на этом сайте, например /library"))?,
    };
    begin(&st, jar, next.to_string(), None).await
}

/// Уводит браузер на daly.rest. state, nonce и verifier уходят в cookie этого браузера.
async fn begin(
    st: &AppState,
    jar: PrivateCookieJar,
    next: String,
    app: Option<Uuid>,
) -> AppResult<(PrivateCookieJar, Response)> {
    let oidc = st.oidc.as_ref().ok_or(AppError::NotFound)?;
    let doc = match oidc.discovery().await {
        Ok(doc) => doc,
        Err(e) => {
            tracing::warn!(error = %e, "OIDC discovery failed");
            return Ok((jar, found(&st.config.login_error("provider", &next))));
        }
    };
    let pending = Pending {
        state: random_token()?,
        nonce: random_token()?,
        verifier: random_token()?,
        next,
        app,
        exp: Utc::now().timestamp() + OIDC_TTL_S,
    };
    let url = oidc.authorize_url(
        &doc,
        &st.config.oidc_redirect_uri(),
        &pending.state,
        &pending.nonce,
        &pkce_challenge(&pending.verifier),
    )?;
    let value = serde_json::to_string(&pending).map_err(anyhow::Error::from)?;
    let cookie = Cookie::build((OIDC_COOKIE, value))
        .http_only(true)
        .secure(st.config.cookie_secure())
        .same_site(SameSite::Lax)
        .path(OIDC_COOKIE_PATH)
        .max_age(time::Duration::seconds(OIDC_TTL_S))
        .build();
    Ok((jar.add(cookie), found(&url)))
}

#[derive(Debug, Default, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct CallbackQuery {
    pub code: Option<String>,
    pub state: Option<String>,
    /// Ошибка провайдера, например `access_denied`, если вход отменили
    pub error: Option<String>,
    /// RFC 9207: кто выдал ответ
    pub iss: Option<String>,
}

/// Почему вход не удался. Код уходит в `/login?error=…` или приложению.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Fail {
    Cancelled,
    Expired,
    State,
    Disabled,
    Provider,
}

impl Fail {
    fn code(self) -> &'static str {
        match self {
            Self::Cancelled => "cancelled",
            Self::Expired => "expired",
            Self::State => "state",
            Self::Disabled => "disabled",
            Self::Provider => "provider",
        }
    }
}

struct AppRequest {
    id: Uuid,
    redirect_uri: String,
    state: String,
}

#[utoipa::path(
    get,
    path = "/api/auth/oidc/callback",
    tag = "auth",
    params(CallbackQuery),
    responses((
        status = 302,
        description = "Успех: cookie `dw_session` и переход на `next` (или код приложению). \
                       Ошибка: `/login?error=cancelled|expired|state|disabled|provider`",
    )),
)]
pub async fn oidc_callback(
    State(st): State<AppState>,
    ClientIp(ip): ClientIp,
    headers: HeaderMap,
    jar: PrivateCookieJar,
    sessions: CookieJar,
    Query(q): Query<CallbackQuery>,
) -> (PrivateCookieJar, Response) {
    let pending: Option<Pending> = jar
        .get(OIDC_COOKIE)
        .and_then(|c| serde_json::from_str(c.value()).ok());
    let jar = jar.remove(Cookie::build(OIDC_COOKIE).path(OIDC_COOKIE_PATH));
    let next = pending
        .as_ref()
        .map_or_else(|| "/".to_string(), |p| p.next.clone());
    let response = match complete(&st, ip, &headers, sessions, pending, q).await {
        Ok(response) => response,
        Err(e) => {
            tracing::error!(error = %e, "OIDC callback failed");
            found(&st.config.login_error(Fail::Provider.code(), &next))
        }
    };
    (jar, response)
}

async fn complete(
    st: &AppState,
    ip: IpAddr,
    headers: &HeaderMap,
    sessions: CookieJar,
    pending: Option<Pending>,
    q: CallbackQuery,
) -> AppResult<Response> {
    let next = pending
        .as_ref()
        .map_or_else(|| "/".to_string(), |p| p.next.clone());
    let app = match pending.as_ref().and_then(|p| p.app) {
        Some(id) => {
            let req = sqlx::query_as!(
                AppRequest,
                "SELECT id, redirect_uri, state FROM app_login_requests
                 WHERE id = $1 AND expires_at > now() AND code_hash IS NULL",
                id,
            )
            .fetch_optional(&st.db)
            .await?;
            match req {
                Some(req) => Some(req),
                None => return Ok(found(&st.config.login_error(Fail::Expired.code(), &next))),
            }
        }
        None => None,
    };
    let fail = |reason: Fail| match &app {
        Some(req) => found(&with_params(
            &req.redirect_uri,
            &[("error", reason.code()), ("state", &req.state)],
        )),
        None => found(&st.config.login_error(reason.code(), &next)),
    };

    let user = match sign_in(st, ip, pending, &q).await {
        Ok(user) => user,
        Err(reason) => return Ok(fail(reason)),
    };

    match &app {
        Some(req) => {
            // Код живёт 60 секунд и привязан к code_challenge, присланному приложением в начале
            let code = random_token()?;
            let issued = sqlx::query!(
                "UPDATE app_login_requests
                 SET user_id = $2, code_hash = $3, code_expires_at = now() + interval '60 seconds'
                 WHERE id = $1 AND code_hash IS NULL AND expires_at > now()",
                req.id,
                user.id,
                &sha256(&code)[..],
            )
            .execute(&st.db)
            .await?;
            if issued.rows_affected() == 0 {
                return Ok(fail(Fail::Expired));
            }
            Ok(found(&with_params(
                &req.redirect_uri,
                &[("code", &code), ("state", &req.state)],
            )))
        }
        None => {
            let (sessions, _) =
                open_web_session(st, sessions, headers, ip, user.id, Method::Oidc).await?;
            Ok((sessions, found(&st.config.page(&next))).into_response())
        }
    }
}

/// Проверяет возврат от daly.rest, меняет код на профиль и находит человека.
async fn sign_in(
    st: &AppState,
    ip: IpAddr,
    pending: Option<Pending>,
    q: &CallbackQuery,
) -> Result<UserRow, Fail> {
    if q.error.is_some() {
        return Err(Fail::Cancelled);
    }
    let Some(pending) = pending else {
        return Err(Fail::Expired);
    };
    if pending.exp < Utc::now().timestamp() {
        return Err(Fail::Expired);
    }
    let (Some(code), Some(state)) = (q.code.as_deref(), q.state.as_deref()) else {
        return Err(Fail::Cancelled);
    };
    // Чужой state — ссылку возврата открыли не в том браузере, где начинали вход
    if !secrets_match(state, &pending.state) {
        return Err(Fail::State);
    }
    let oidc = st.oidc.as_ref().ok_or(Fail::Provider)?;
    if let Some(iss) = q.iss.as_deref() {
        let expected = oidc
            .discovery()
            .await
            .map(|d| d.issuer.clone())
            .map_err(|e| {
                tracing::warn!(error = %e, "OIDC discovery failed");
                Fail::Provider
            })?;
        if iss != expected {
            tracing::warn!(iss, "callback from an unexpected issuer");
            return Err(Fail::Provider);
        }
    }
    let identity = oidc
        .exchange(
            code,
            &pending.verifier,
            &st.config.oidc_redirect_uri(),
            &pending.nonce,
        )
        .await
        .map_err(|e| match e {
            OidcError::Nonce => {
                tracing::warn!("id_token nonce mismatch");
                Fail::State
            }
            OidcError::Provider(e) => {
                tracing::warn!(error = %e, "OIDC code exchange failed");
                Fail::Provider
            }
        })?;
    let user = users::sign_in(st, identity).await.map_err(|e| {
        tracing::error!(error = %e, "could not save the user after OIDC sign-in");
        Fail::Provider
    })?;
    if user.disabled {
        let payload = json!({ "via": "oidc", "reason": "disabled", "ip": ip.to_string() });
        if let Err(e) = audit::record(
            &st.db,
            None,
            audit::SIGN_IN_DENIED,
            Some(user.id.to_string()),
            payload,
        )
        .await
        {
            tracing::error!(error = %e, "could not write the audit event");
        }
        return Err(Fail::Disabled);
    }
    Ok(user)
}

/// Новая сессия сайта. Прежняя сессия этого браузера закрывается.
pub(crate) async fn open_web_session(
    st: &AppState,
    jar: CookieJar,
    headers: &HeaderMap,
    ip: IpAddr,
    user_id: Uuid,
    method: Method,
) -> AppResult<(CookieJar, Uuid)> {
    let mut tx = st.db.begin().await?;
    if let Some(old) = jar.get(SESSION_COOKIE) {
        session::revoke_token(&mut tx, old.value()).await?;
    }
    let new = NewSession {
        user_id,
        device_id: None,
        method,
        user_agent: user_agent(headers),
        ip,
    };
    let (session_id, token) = session::create(&mut tx, new).await?;
    let payload = json!({ "via": method.as_str(), "sessionId": session_id, "ip": ip.to_string() });
    audit::record(
        &mut *tx,
        Some(user_id),
        audit::SIGN_IN,
        Some(user_id.to_string()),
        payload,
    )
    .await?;
    tx.commit().await?;
    Ok((jar.add(session::cookie(&st.config, token)), session_id))
}

// ---------- аварийный пароль ----------

#[utoipa::path(
    post,
    path = "/api/auth/password",
    tag = "auth",
    request_body = PasswordLogin,
    responses(
        (status = 200, description = "Вход как владелец; ставит cookie `dw_session`", body = Me),
        (status = 401, description = "Неверный пароль (`wrong_password`)", body = ErrorBody),
        (status = 403, description = "Вход паролем выключен (`password_disabled`)", body = ErrorBody),
        (status = 429, description = "Больше 5 попыток в минуту с одного адреса", body = ErrorBody),
    ),
)]
pub async fn password_login(
    State(st): State<AppState>,
    ClientIp(ip): ClientIp,
    headers: HeaderMap,
    jar: CookieJar,
    Json(body): Json<PasswordLogin>,
) -> AppResult<(CookieJar, Json<Me>)> {
    let Some(expected) = st.config.admin_password.as_deref() else {
        return Err(AppError::PasswordDisabled);
    };
    if !st.limiter.check("password", ip, 5, Duration::from_secs(60)) {
        return Err(AppError::TooManyRequests);
    }
    if !secrets_match(&body.password, expected) {
        let payload = json!({ "via": "password", "ip": ip.to_string() });
        audit::record(&st.db, None, audit::SIGN_IN_FAILED, None, payload).await?;
        return Err(AppError::WrongPassword);
    }
    let owner = users::owner(&st).await?;
    let (jar, session_id) =
        open_web_session(&st, jar, &headers, ip, owner.id, Method::Password).await?;
    Ok((jar, Json(me_for(&owner, AuthVia::Password, session_id))))
}

// ---------- сессия ----------

#[utoipa::path(
    post,
    path = "/api/auth/logout",
    tag = "auth",
    security(("session" = []), ("device" = [])),
    responses((status = 204, description = "Сессия отозвана, cookie `dw_session` стёрта")),
)]
pub async fn logout(
    State(st): State<AppState>,
    headers: HeaderMap,
    jar: CookieJar,
) -> AppResult<(CookieJar, StatusCode)> {
    let token = match auth::bearer(&headers) {
        Some(token) => Some(token.to_string()),
        None => jar.get(SESSION_COOKIE).map(|c| c.value().to_string()),
    };
    if let Some(token) = token {
        let mut tx = st.db.begin().await?;
        if let Some(revoked) = session::revoke_token(&mut tx, &token).await? {
            let payload = json!({ "sessionId": revoked.id, "deviceId": revoked.device_id });
            audit::record(
                &mut *tx,
                Some(revoked.user_id),
                audit::LOGOUT,
                Some(revoked.id.to_string()),
                payload,
            )
            .await?;
        }
        tx.commit().await?;
    }
    Ok((
        jar.remove(session::removal_cookie(&st.config)),
        StatusCode::NO_CONTENT,
    ))
}

#[utoipa::path(
    get,
    path = "/api/auth/me",
    tag = "auth",
    security(("session" = []), ("device" = [])),
    responses(
        (status = 200, description = "Кто вошёл; у людей без ролей `roles` пустой", body = Me),
        (status = 401, description = "Нет сессии", body = ErrorBody),
    ),
)]
pub async fn me(State(st): State<AppState>, principal: Principal) -> AppResult<Json<Me>> {
    let device = match principal.device_id {
        Some(id) => devices::get(&st.db, id).await?.map(|d| d.view()),
        None => None,
    };
    Ok(Json(Me {
        permissions: principal.permissions(),
        user: principal.user,
        roles: principal.roles,
        via: principal.via,
        session_id: principal.session_id,
        device,
    }))
}

// ---------- вход приложений ----------

#[utoipa::path(
    get,
    path = "/api/auth/app/start",
    tag = "auth",
    params(AppStartQuery),
    responses(
        (status = 302, description = "На daly.rest; после входа — на redirect_uri с `code` и `state` \
                                      или с `error` и `state`"),
        (status = 400, description = "redirect_uri не из разрешённых", body = ErrorBody),
    ),
)]
pub async fn app_start(
    State(st): State<AppState>,
    ClientIp(ip): ClientIp,
    jar: PrivateCookieJar,
    Query(q): Query<AppStartQuery>,
) -> AppResult<(PrivateCookieJar, Response)> {
    if !st
        .limiter
        .check("app-start", ip, 30, Duration::from_secs(60))
    {
        return Err(AppError::TooManyRequests);
    }
    // Чужой адрес не получает даже ошибку: иначе это открытый редирект
    let redirect_uri = q
        .redirect_uri
        .as_deref()
        .filter(|u| app_redirect_allowed(u))
        .ok_or_else(|| {
            bad("redirect_uri не разрешён: dalywave://auth или http://127.0.0.1:<порт>/callback")
        })?;
    let state = q.state.as_deref().unwrap_or("");
    let reject = |error: &str, description: &str| {
        found(&with_params(
            redirect_uri,
            &[
                ("error", error),
                ("error_description", description),
                ("state", state),
            ],
        ))
    };
    if state.is_empty() || state.len() > 512 {
        return Ok((
            jar,
            reject(
                "invalid_request",
                "state is required (up to 512 characters)",
            ),
        ));
    }
    let challenge = q.code_challenge.as_deref().unwrap_or("");
    if q.code_challenge_method.as_deref() != Some("S256") || !valid_pkce(challenge) {
        return Ok((
            jar,
            reject(
                "invalid_request",
                "PKCE is required: code_challenge with code_challenge_method=S256",
            ),
        ));
    }
    let Some(platform) = q.platform else {
        return Ok((jar, reject("invalid_request", "platform is required")));
    };
    if st.oidc.is_none() {
        return Ok((
            jar,
            reject("provider", "sign-in provider is not configured"),
        ));
    }
    let device_name = q
        .device_name
        .as_deref()
        .map(|n| n.trim().chars().take(64).collect::<String>())
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| devices::default_name(platform).to_string());

    sqlx::query!("DELETE FROM app_login_requests WHERE expires_at < now() - interval '1 day'")
        .execute(&st.db)
        .await?;
    let id = sqlx::query_scalar!(
        "INSERT INTO app_login_requests (redirect_uri, state, code_challenge, device_name, platform, expires_at)
         VALUES ($1, $2, $3, $4, $5, now() + interval '10 minutes')
         RETURNING id",
        redirect_uri,
        state,
        challenge,
        device_name,
        platform.as_str(),
    )
    .fetch_one(&st.db)
    .await?;
    begin(&st, jar, "/".into(), Some(id)).await
}

#[utoipa::path(
    post,
    path = "/api/auth/app/token",
    tag = "auth",
    request_body = AppTokenRequest,
    responses(
        (status = 200, description = "Устройство и его токен; токен показывается один раз", body = AppToken),
        (status = 400, description = "Код неверный, истёк, уже использован или не сходится с PKCE \
                                      (`invalid_grant`)", body = ErrorBody),
        (status = 403, description = "Аккаунт заблокирован", body = ErrorBody),
    ),
)]
pub async fn app_token(
    State(st): State<AppState>,
    ClientIp(ip): ClientIp,
    headers: HeaderMap,
    Json(body): Json<AppTokenRequest>,
) -> AppResult<Json<AppToken>> {
    if !st
        .limiter
        .check("app-token", ip, 30, Duration::from_secs(60))
    {
        return Err(AppError::TooManyRequests);
    }
    let unknown = || AppError::InvalidGrant("код неверный, истёк или уже использован".into());
    let code_hash = sha256(&body.code);
    let mut tx = st.db.begin().await?;
    // Код гасится сразу: даже неудачная попытка не даёт перебирать verifier
    let row = sqlx::query!(
        "UPDATE app_login_requests SET used_at = now()
         WHERE code_hash = $1 AND used_at IS NULL
         RETURNING id, user_id, code_challenge, code_expires_at, device_name, platform",
        &code_hash[..],
    )
    .fetch_optional(&mut *tx)
    .await?;
    let Some(row) = row else {
        // Повторное предъявление (RFC 6749 4.1.2): сессию, выданную по этому коду, отзываем
        let issued = sqlx::query_scalar!(
            "SELECT session_id FROM app_login_requests WHERE code_hash = $1 AND used_at IS NOT NULL",
            &code_hash[..],
        )
        .fetch_optional(&mut *tx)
        .await?
        .flatten();
        if let Some(session_id) = issued
            && let Some(revoked) = session::revoke(&mut tx, session_id).await?
        {
            let payload = json!({ "sessionId": revoked.id, "deviceId": revoked.device_id, "ip": ip.to_string() });
            audit::record(
                &mut *tx,
                None,
                audit::APP_CODE_REUSED,
                Some(revoked.user_id.to_string()),
                payload,
            )
            .await?;
        }
        tx.commit().await?;
        return Err(unknown());
    };
    let expired = row.code_expires_at.is_none_or(|at| at < Utc::now());
    let pkce_ok = valid_pkce(&body.code_verifier)
        && secrets_match(&pkce_challenge(&body.code_verifier), &row.code_challenge);
    let user = match row.user_id {
        Some(id) => users::get(&st, id).await?,
        None => None,
    };
    let user = match user {
        Some(user) if !expired && pkce_ok => user,
        _ => {
            tx.commit().await?;
            return Err(if pkce_ok {
                unknown()
            } else {
                AppError::InvalidGrant("code_verifier не подходит".into())
            });
        }
    };
    if user.disabled {
        tx.commit().await?;
        return Err(AppError::Forbidden("аккаунт заблокирован".into()));
    }

    let platform: Platform = row.platform.parse().map_err(anyhow::Error::from)?;
    let app_version: Option<String> = body
        .app_version
        .as_deref()
        .map(|v| v.trim().chars().take(32).collect())
        .filter(|v: &String| !v.is_empty());
    let device = devices::create(
        &mut tx,
        user.id,
        &row.device_name,
        platform.as_str(),
        app_version.as_deref(),
    )
    .await?;
    let new = NewSession {
        user_id: user.id,
        device_id: Some(device.id),
        method: Method::Oidc,
        user_agent: user_agent(&headers),
        ip,
    };
    let (session_id, token) = session::create(&mut tx, new).await?;
    sqlx::query!(
        "UPDATE app_login_requests SET session_id = $2 WHERE id = $1",
        row.id,
        session_id
    )
    .execute(&mut *tx)
    .await?;
    let payload = json!({
        "via": "device",
        "sessionId": session_id,
        "deviceId": device.id,
        "platform": platform,
        "ip": ip.to_string(),
    });
    audit::record(
        &mut *tx,
        Some(user.id),
        audit::SIGN_IN,
        Some(user.id.to_string()),
        payload,
    )
    .await?;
    tx.commit().await?;
    Ok(Json(AppToken {
        token,
        device: device.view(),
        user: user.public(),
    }))
}

// ---------- сопряжённые устройства ----------

#[utoipa::path(
    post,
    path = "/api/auth/app/pair",
    tag = "auth",
    security(("device" = [])),
    request_body = PairRequest,
    responses(
        (status = 200, description = "Сессия часов: отдельное устройство в «Доступе», отзывается вместе \
                                      с iPhone. Токен показывается один раз", body = AppToken),
        (status = 400, description = "Платформа — не сопряжённое устройство", body = ErrorBody),
        (status = 401, description = "Нет сессии", body = ErrorBody),
        (status = 403, description = "Просит не приложение (сайт) или само сопряжённое устройство", body = ErrorBody),
        (status = 429, description = "Больше 10 запросов в минуту с одного адреса", body = ErrorBody),
    ),
)]
pub async fn app_pair(
    State(st): State<AppState>,
    ClientIp(ip): ClientIp,
    headers: HeaderMap,
    principal: Principal,
    Json(body): Json<PairRequest>,
) -> AppResult<Json<AppToken>> {
    if !st
        .limiter
        .check("app-pair", ip, 10, Duration::from_secs(60))
    {
        return Err(AppError::TooManyRequests);
    }
    // Выдаёт только приложение со своим входом: не сайт (cookie можно подставить чужой странице)
    // и не сами часы — иначе сопряжённые устройства плодили бы друг друга
    let parent = match (principal.kind, principal.device_id) {
        (SessionKind::Device, Some(id)) => id,
        _ => return Err(AppError::Forbidden("сессию часам выдаёт приложение".into())),
    };
    if !body.platform.is_paired_accessory() {
        return Err(bad("platform: сессию можно выдать только часам (watchos)"));
    }
    let mut tx = st.db.begin().await?;
    let parent_row = sqlx::query!(
        "SELECT d.paired_from, s.auth_method FROM devices d JOIN sessions s ON s.device_id = d.id
         WHERE d.id = $1 AND s.id = $2",
        parent,
        principal.session_id,
    )
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(AppError::Unauthorized)?;
    if parent_row.paired_from.is_some() {
        return Err(AppError::Forbidden(
            "сопряжённое устройство не выдаёт сессий".into(),
        ));
    }
    let method = match parent_row.auth_method.as_str() {
        "password" => Method::Password,
        _ => Method::Oidc,
    };
    let name = body
        .device_name
        .as_deref()
        .map(|n| n.trim().chars().take(64).collect::<String>())
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| devices::default_name(body.platform).to_string());
    let app_version: Option<String> = body
        .app_version
        .as_deref()
        .map(|v| v.trim().chars().take(32).collect())
        .filter(|v: &String| !v.is_empty());
    let user = principal.user.id;
    let device = devices::create_paired(
        &mut tx,
        user,
        parent,
        &name,
        body.platform.as_str(),
        app_version.as_deref(),
    )
    .await?;
    let new = NewSession {
        user_id: user,
        device_id: Some(device.id),
        method,
        user_agent: user_agent(&headers),
        ip,
    };
    let (session_id, token) = session::create(&mut tx, new).await?;
    let payload = json!({
        "via": "paired",
        "sessionId": session_id,
        "deviceId": device.id,
        "pairedFrom": parent,
        "platform": body.platform,
        "ip": ip.to_string(),
    });
    audit::record(
        &mut *tx,
        Some(user),
        audit::SIGN_IN,
        Some(user.to_string()),
        payload,
    )
    .await?;
    tx.commit().await?;
    Ok(Json(AppToken {
        token,
        device: device.view(),
        user: principal.user,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn next_must_stay_on_site() {
        assert_eq!(
            local_path("/library?tab=albums"),
            Some("/library?tab=albums")
        );
        assert_eq!(local_path("/"), Some("/"));
        assert_eq!(local_path("//evil.example"), None);
        assert_eq!(local_path("https://evil.example"), None);
        assert_eq!(local_path("/\\evil.example"), None);
        assert_eq!(local_path("library"), None);
        assert_eq!(local_path("/a\nb"), None);
    }

    #[test]
    fn app_redirects() {
        assert!(app_redirect_allowed("dalywave://auth"));
        assert!(app_redirect_allowed("http://127.0.0.1:53682/callback"));
        assert!(app_redirect_allowed("http://localhost:4100/callback"));
        assert!(
            !app_redirect_allowed("http://127.0.0.1/callback"),
            "порт обязателен"
        );
        assert!(!app_redirect_allowed("http://127.0.0.1:5000/other"));
        assert!(!app_redirect_allowed("http://127.0.0.1:5000/callback?x=1"));
        assert!(!app_redirect_allowed("http://evil.example:5000/callback"));
        assert!(!app_redirect_allowed("https://127.0.0.1:5000/callback"));
        assert!(!app_redirect_allowed("http://user@127.0.0.1:5000/callback"));
        assert!(!app_redirect_allowed("http://127.0.0.1:5000/x/../callback"));
        assert!(!app_redirect_allowed("dalywave://auth/extra"));
    }

    #[test]
    fn params_are_appended() {
        assert_eq!(
            with_params("dalywave://auth", &[("code", "a b"), ("state", "s")]),
            "dalywave://auth?code=a+b&state=s"
        );
        assert_eq!(
            with_params("http://127.0.0.1:5000/callback", &[("error", "state")]),
            "http://127.0.0.1:5000/callback?error=state"
        );
    }
}
