//! Вход: провайдеры OIDC и свои аккаунты на сайте, аварийный пароль и вход приложений через
//! системный браузер.
//!
//! Сайт: `/api/auth/oidc/{provider}/start` → провайдер → `/api/auth/oidc/{provider}/callback` → cookie
//! `dw_session` → `next`. У daly.rest из прежних переменных адрес возврата старый: `/api/auth/oidc/callback`.
//! Свои аккаунты — [`crate::routes::accounts`].
//!
//! Приложение: `/api/auth/app/start` → экран входа сайта в системном браузере (`/login?app=1`; запрос
//! приложения — в cookie `dw_app` этого браузера) → любой способ входа → одноразовый код на redirect_uri
//! приложения → `POST /api/auth/app/token` → токен устройства `dwd_…`. Если способ входа один — провайдер,
//! экран входа пропускается, как раньше. Код провайдера меняется на токены только здесь, на сервере.
//! Часы: своего входа нет — вошедший iPhone просит `POST /api/auth/app/pair` и передаёт им токен.

use std::{net::IpAddr, sync::Arc, time::Duration};

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
    accounts, audit,
    auth::{self, Principal, permissions},
    crypto::{pkce_challenge, random_token, secrets_match, sha256, valid_pkce},
    devices,
    error::{AppError, AppResult, bad},
    extract::{Json, Path, Query},
    oidc::{Oidc, OidcError},
    session::{self, Method, NewSession, SESSION_COOKIE},
    state::{AppState, ClientIp},
    users::{self, Intent, SignInError, UserRow},
};

pub const OIDC_COOKIE: &str = "dw_oidc";
/// Вход приложения, начатый в этом браузере: id запроса `app_login_requests`
pub const APP_COOKIE: &str = "dw_app";
pub const OIDC_COOKIE_PATH: &str = "/api/auth";
/// Сколько ждём возврата от провайдера
pub const OIDC_TTL_S: i64 = 600;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/providers", get(providers))
        .route("/oidc/start", get(oidc_start))
        .route("/oidc/callback", get(oidc_callback))
        .route("/oidc/{provider}/start", get(provider_start))
        .route("/oidc/{provider}/callback", get(provider_callback))
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
    /// id провайдера; пусто — вход, начатый прежней версией (daly.rest)
    #[serde(default)]
    pub provider: String,
    /// Привязать вход к этому аккаунту (настройки)
    #[serde(default)]
    pub link: Option<Uuid>,
    /// Токен приглашения
    #[serde(default)]
    pub invite: Option<String>,
}

/// Вход приложения, начатый через `/api/auth/app/start` и продолженный на экране входа.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppPending {
    pub id: Uuid,
    pub exp: i64,
}

pub(crate) fn found(location: &str) -> Response {
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

pub(crate) fn me_for(user: &UserRow, via: AuthVia, session_id: Uuid) -> Me {
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

fn provider_view(p: &Oidc) -> OidcProvider {
    OidcProvider {
        id: p.config.id.clone(),
        title: p.config.title.clone(),
        icon: p.config.icon.clone(),
    }
}

fn private_cookie(st: &AppState, name: &'static str, value: String) -> Cookie<'static> {
    Cookie::build((name, value))
        .http_only(true)
        .secure(st.config.cookie_secure())
        .same_site(SameSite::Lax)
        .path(OIDC_COOKIE_PATH)
        .max_age(time::Duration::seconds(OIDC_TTL_S))
        .build()
}

// ---------- способы входа ----------

#[utoipa::path(
    get,
    path = "/api/auth/providers",
    tag = "auth",
    responses((status = 200, description = "Какие способы входа включены", body = AuthProviders)),
)]
pub async fn providers(State(st): State<AppState>) -> AppResult<Json<AuthProviders>> {
    let list: Vec<OidcProvider> = st.oidc.iter().map(|p| provider_view(p)).collect();
    let setup = st.config.local_auth && accounts::first_run(&st.db).await?;
    Ok(Json(AuthProviders {
        oidc: list.first().cloned(),
        providers: list,
        password: st.config.admin_password.is_some(),
        local: st.config.local_auth,
        setup,
    }))
}

// ---------- провайдеры OIDC ----------

#[utoipa::path(
    get,
    path = "/api/auth/oidc/start",
    tag = "auth",
    params(OidcStartQuery),
    responses(
        (status = 302, description = "Вход через первого провайдера (прежний адрес): как `/api/auth/oidc/{provider}/start`"),
        (status = 400, description = "`next` — не путь на этом сайте", body = ErrorBody),
        (status = 404, description = "Провайдеры не настроены", body = ErrorBody),
    ),
)]
pub async fn oidc_start(
    State(st): State<AppState>,
    headers: HeaderMap,
    jar: PrivateCookieJar,
    Query(q): Query<OidcStartQuery>,
) -> AppResult<(PrivateCookieJar, Response)> {
    let provider = st.oidc.first().cloned().ok_or(AppError::NotFound)?;
    start(&st, &headers, jar, provider, q).await
}

#[utoipa::path(
    get,
    path = "/api/auth/oidc/{provider}/start",
    tag = "auth",
    params(("provider" = String, Path, description = "id провайдера из `/api/auth/providers`"), OidcStartQuery),
    responses(
        (status = 302, description = "На страницу входа провайдера; ставит cookie `dw_oidc` на 10 минут"),
        (status = 400, description = "`next` — не путь на этом сайте", body = ErrorBody),
        (status = 401, description = "`link=1` без входа на сайте", body = ErrorBody),
        (status = 404, description = "Нет такого провайдера", body = ErrorBody),
    ),
)]
pub async fn provider_start(
    State(st): State<AppState>,
    headers: HeaderMap,
    jar: PrivateCookieJar,
    Path(provider): Path<String>,
    Query(q): Query<OidcStartQuery>,
) -> AppResult<(PrivateCookieJar, Response)> {
    let provider = st.provider(&provider).cloned().ok_or(AppError::NotFound)?;
    start(&st, &headers, jar, provider, q).await
}

async fn start(
    st: &AppState,
    headers: &HeaderMap,
    jar: PrivateCookieJar,
    provider: Arc<Oidc>,
    q: OidcStartQuery,
) -> AppResult<(PrivateCookieJar, Response)> {
    let next = match q.next.as_deref() {
        None | Some("") => "/",
        Some(next) => local_path(next)
            .ok_or_else(|| bad("next должен быть путём на этом сайте, например /library"))?,
    };
    let on = |v: &Option<String>| v.as_deref().is_some_and(|v| v == "1" || v == "true");
    // Вход приложения: запрос лежит в cookie этого браузера, адрес его не передаёт
    let app = if on(&q.app) {
        let pending: Option<AppPending> = jar
            .get(APP_COOKIE)
            .and_then(|c| serde_json::from_str(c.value()).ok());
        match pending.filter(|p| p.exp >= Utc::now().timestamp()) {
            Some(p) => Some(p.id),
            None => return Ok((jar, found(&st.config.login_error("expired", next)))),
        }
    } else {
        None
    };
    let link = if on(&q.link) {
        let principal = auth::authenticate(st, headers)
            .await?
            .filter(|p| p.kind == SessionKind::Web)
            .ok_or(AppError::Unauthorized)?;
        Some(principal.user.id)
    } else {
        None
    };
    let invite = q.invite.filter(|t| !t.is_empty() && t.len() <= 128);
    begin(st, jar, provider, next.to_string(), app, link, invite).await
}

/// Уводит браузер к провайдеру. state, nonce и verifier уходят в cookie этого браузера.
async fn begin(
    st: &AppState,
    jar: PrivateCookieJar,
    oidc: Arc<Oidc>,
    next: String,
    app: Option<Uuid>,
    link: Option<Uuid>,
    invite: Option<String>,
) -> AppResult<(PrivateCookieJar, Response)> {
    let doc = match oidc.discovery().await {
        Ok(doc) => doc,
        Err(e) => {
            tracing::warn!(provider = oidc.id(), error = %e, "OIDC discovery failed");
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
        provider: oidc.id().to_string(),
        link,
        invite,
    };
    let url = oidc.authorize_url(
        &doc,
        &st.config.oidc_redirect_uri(&oidc.config),
        &pending.state,
        &pending.nonce,
        &pkce_challenge(&pending.verifier),
    )?;
    let value = serde_json::to_string(&pending).map_err(anyhow::Error::from)?;
    Ok((jar.add(private_cookie(st, OIDC_COOKIE, value)), found(&url)))
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
    /// Провайдер не пускает новых людей, а приглашения нет
    NotRegistered,
    /// Привязка: этот вход уже у другого человека
    LinkedElsewhere,
}

impl Fail {
    fn code(self) -> &'static str {
        match self {
            Self::Cancelled => "cancelled",
            Self::Expired => "expired",
            Self::State => "state",
            Self::Disabled => "disabled",
            Self::Provider => "provider",
            Self::NotRegistered => "not_registered",
            Self::LinkedElsewhere => "linked_elsewhere",
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
        description = "Возврат от daly.rest из прежних переменных (адрес зарегистрирован там). \
                       Успех: cookie `dw_session` и переход на `next` (или код приложению). \
                       Ошибка: `/login?error=cancelled|expired|state|disabled|provider|not_registered`",
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
    callback(&st, ip, &headers, jar, sessions, None, q).await
}

#[utoipa::path(
    get,
    path = "/api/auth/oidc/{provider}/callback",
    tag = "auth",
    params(("provider" = String, Path, description = "id провайдера"), CallbackQuery),
    responses((
        status = 302,
        description = "Успех: cookie `dw_session` и переход на `next`, код приложению или `/settings?linked=…`. \
                       Ошибка: `/login?error=cancelled|expired|state|disabled|provider|not_registered` \
                       (привязка — `/settings?link_error=…`)",
    )),
)]
pub async fn provider_callback(
    State(st): State<AppState>,
    ClientIp(ip): ClientIp,
    headers: HeaderMap,
    jar: PrivateCookieJar,
    sessions: CookieJar,
    Path(provider): Path<String>,
    Query(q): Query<CallbackQuery>,
) -> (PrivateCookieJar, Response) {
    callback(&st, ip, &headers, jar, sessions, Some(provider), q).await
}

async fn callback(
    st: &AppState,
    ip: IpAddr,
    headers: &HeaderMap,
    jar: PrivateCookieJar,
    sessions: CookieJar,
    path_provider: Option<String>,
    q: CallbackQuery,
) -> (PrivateCookieJar, Response) {
    let pending: Option<Pending> = jar
        .get(OIDC_COOKIE)
        .and_then(|c| serde_json::from_str(c.value()).ok());
    let jar = jar.remove(Cookie::build(OIDC_COOKIE).path(OIDC_COOKIE_PATH));
    let next = pending
        .as_ref()
        .map_or_else(|| "/".to_string(), |p| p.next.clone());
    let linking = pending.as_ref().is_some_and(|p| p.link.is_some());
    let jar = if pending.as_ref().is_some_and(|p| p.app.is_some()) {
        jar.remove(Cookie::build(APP_COOKIE).path(OIDC_COOKIE_PATH))
    } else {
        jar
    };
    let response = match complete(st, ip, headers, sessions, pending, path_provider, q).await {
        Ok(response) => response,
        Err(e) => {
            tracing::error!(error = %e, "OIDC callback failed");
            if linking {
                found(&link_result(st, Err(Fail::Provider)))
            } else {
                found(&st.config.login_error(Fail::Provider.code(), &next))
            }
        }
    };
    (jar, response)
}

/// Куда вернуть после привязки входа в настройках.
fn link_result(st: &AppState, result: Result<&str, Fail>) -> String {
    match result {
        Ok(provider) => st.config.page(&format!("/settings?linked={provider}")),
        Err(fail) => st
            .config
            .page(&format!("/settings?link_error={}", fail.code())),
    }
}

/// Запрос приложения, ещё ждущий входа.
async fn app_request(st: &AppState, id: Uuid) -> AppResult<Option<AppRequest>> {
    Ok(sqlx::query_as!(
        AppRequest,
        "SELECT id, redirect_uri, state FROM app_login_requests
         WHERE id = $1 AND expires_at > now() AND code_hash IS NULL",
        id,
    )
    .fetch_optional(&st.db)
    .await?)
}

/// Выдаёт одноразовый код приложению: адрес возврата с `code` и `state`; `None` — запрос истёк.
/// Код живёт 60 секунд и привязан к code_challenge, присланному приложением в начале.
pub(crate) async fn issue_app_code(
    st: &AppState,
    app: Uuid,
    user: Uuid,
    method: Method,
) -> AppResult<Option<String>> {
    let Some(req) = app_request(st, app).await? else {
        return Ok(None);
    };
    let code = random_token()?;
    let issued = sqlx::query!(
        "UPDATE app_login_requests
         SET user_id = $2, code_hash = $3, code_expires_at = now() + interval '60 seconds', auth_method = $4
         WHERE id = $1 AND code_hash IS NULL AND expires_at > now()",
        req.id,
        user,
        &sha256(&code)[..],
        method.as_str(),
    )
    .execute(&st.db)
    .await?;
    if issued.rows_affected() == 0 {
        return Ok(None);
    }
    Ok(Some(with_params(
        &req.redirect_uri,
        &[("code", &code), ("state", &req.state)],
    )))
}

/// Запрос приложения из cookie `dw_app` этого браузера.
pub(crate) fn app_cookie(jar: &PrivateCookieJar) -> Option<Uuid> {
    jar.get(APP_COOKIE)
        .and_then(|c| serde_json::from_str::<AppPending>(c.value()).ok())
        .filter(|p| p.exp >= Utc::now().timestamp())
        .map(|p| p.id)
}

async fn complete(
    st: &AppState,
    ip: IpAddr,
    headers: &HeaderMap,
    sessions: CookieJar,
    pending: Option<Pending>,
    path_provider: Option<String>,
    q: CallbackQuery,
) -> AppResult<Response> {
    let next = pending
        .as_ref()
        .map_or_else(|| "/".to_string(), |p| p.next.clone());
    let link = pending.as_ref().and_then(|p| p.link);
    let app = match pending.as_ref().and_then(|p| p.app) {
        Some(id) => match app_request(st, id).await? {
            Some(req) => Some(req),
            None => return Ok(found(&st.config.login_error(Fail::Expired.code(), &next))),
        },
        None => None,
    };
    let fail = |reason: Fail| match &app {
        Some(req) => found(&with_params(
            &req.redirect_uri,
            &[("error", reason.code()), ("state", &req.state)],
        )),
        None if link.is_some() => found(&link_result(st, Err(reason))),
        None => found(&st.config.login_error(reason.code(), &next)),
    };

    let (user, provider) = match sign_in(st, ip, pending, path_provider.as_deref(), &q).await {
        Ok(found) => found,
        Err(reason) => return Ok(fail(reason)),
    };

    if link.is_some() {
        return Ok(found(&link_result(st, Ok(&provider))));
    }
    match &app {
        Some(req) => match issue_app_code(st, req.id, user.id, Method::Oidc).await? {
            Some(location) => Ok(found(&location)),
            None => Ok(fail(Fail::Expired)),
        },
        None => {
            let (sessions, _) =
                open_web_session(st, sessions, headers, ip, user.id, Method::Oidc).await?;
            Ok((sessions, found(&st.config.page(&next))).into_response())
        }
    }
}

/// Проверяет возврат от провайдера, меняет код на профиль и находит человека.
async fn sign_in(
    st: &AppState,
    ip: IpAddr,
    pending: Option<Pending>,
    path_provider: Option<&str>,
    q: &CallbackQuery,
) -> Result<(UserRow, String), Fail> {
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
    // Вход, начатый прежней версией, шёл через daly.rest — первого провайдера со старым адресом
    let oidc = if pending.provider.is_empty() {
        st.oidc.iter().find(|p| p.config.legacy_callback)
    } else {
        st.provider(&pending.provider)
    }
    .cloned()
    .ok_or(Fail::Provider)?;
    // Возврат пришёл на адрес другого провайдера: так не бывает, если ответ не подменили
    let path_ok = match path_provider {
        Some(id) => id == oidc.id() && !oidc.config.legacy_callback,
        None => oidc.config.legacy_callback,
    };
    if !path_ok {
        return Err(Fail::State);
    }
    if let Some(iss) = q.iss.as_deref() {
        let expected = oidc
            .discovery()
            .await
            .map(|d| d.issuer.clone())
            .map_err(|e| {
                tracing::warn!(provider = oidc.id(), error = %e, "OIDC discovery failed");
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
            &st.config.oidc_redirect_uri(&oidc.config),
            &pending.nonce,
        )
        .await
        .map_err(|e| match e {
            OidcError::Nonce => {
                tracing::warn!("id_token nonce mismatch");
                Fail::State
            }
            OidcError::Provider(e) => {
                tracing::warn!(provider = oidc.id(), error = %e, "OIDC code exchange failed");
                Fail::Provider
            }
        })?;

    let invite = match pending.invite.as_deref() {
        Some(token) if pending.link.is_none() => {
            accounts::find_invite(&st.db, token).await.map_err(|e| {
                tracing::error!(error = %e, "could not read the invite");
                Fail::Provider
            })?
        }
        _ => None,
    };
    let intent = Intent {
        link_to: pending.link,
        invite_roles: invite.as_ref().map(|i| i.roles.clone()),
    };
    let user = users::sign_in(st, &oidc.config, identity, intent)
        .await
        .map_err(|e| match e {
            SignInError::NotRegistered => Fail::NotRegistered,
            SignInError::LinkedElsewhere => Fail::LinkedElsewhere,
            SignInError::App(e) => {
                tracing::error!(error = %e, "could not save the user after OIDC sign-in");
                Fail::Provider
            }
        })?;
    if let Some(invite) = invite
        && let Err(e) = accounts::use_invite(&st.db, invite.id, user.id).await
    {
        tracing::error!(error = %e, "could not mark the invite as used");
    }
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
    Ok((user, oidc.id().to_string()))
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
        (status = 302, description = "На экран входа сайта (`/login?app=1`, запрос — в cookie `dw_app`) \
                                      или сразу к провайдеру, если способ входа один; после входа — \
                                      на redirect_uri с `code` и `state` или с `error` и `state`"),
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
    if st.oidc.is_empty() && !st.config.local_auth {
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
    // Способ входа один — провайдер: сразу к нему, как раньше
    if let ([provider], false) = (&st.oidc[..], st.config.local_auth) {
        return begin(&st, jar, provider.clone(), "/".into(), Some(id), None, None).await;
    }
    // Иначе — экран входа сайта в этом же браузере: там все провайдеры и форма пароля.
    // Запрос приложения едет в зашифрованной cookie, а не в адресе
    let pending = AppPending {
        id,
        exp: Utc::now().timestamp() + OIDC_TTL_S,
    };
    let value = serde_json::to_string(&pending).map_err(anyhow::Error::from)?;
    Ok((
        jar.add(private_cookie(&st, APP_COOKIE, value)),
        found(&st.config.page("/login?app=1")),
    ))
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
         RETURNING id, user_id, code_challenge, code_expires_at, device_name, platform, auth_method",
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
        method: Method::parse(row.auth_method.as_deref().unwrap_or_default()),
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
    let method = Method::parse(&parent_row.auth_method);
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
