//! Большой экран: вход на телевизоре кодом с экрана (`/api/auth/tv/*`) и режим вечеринки (`/api/party`).
//! Типы — [`dalywave_protocol::tv`], логика вечеринки для волны — [`crate::party`].
//!
//! Вход — как device flow (RFC 8628), но без своего пароля: подтверждает человек, который уже вошёл
//! через daly.rest на сайте или в приложении. Телевизор получает дочернюю сессию — отдельную строку
//! в «Доступе»; у Apple TV это устройство с токеном `dwd_…`, у браузера телевизора — cookie сайта.

use std::time::Duration;

use axum::{
    Router,
    extract::State,
    http::{HeaderMap, StatusCode, header},
    routing::{get, post},
};
use axum_extra::extract::cookie::CookieJar;
use chrono::Utc;
use dalywave_protocol::{
    ErrorBody,
    auth::{AppToken, Platform},
    realtime::ServerEvent,
    tv::{
        PARTY_HOURS, Party, PartyPatch, TV_CODE_TTL_SECS, TV_POLL_SECS, TvApprove, TvCode,
        TvCodeRequest, TvLogin, TvSignedIn, TvToken, TvTokenRequest,
    },
};
use serde_json::json;
use uuid::Uuid;

use crate::{
    audit,
    auth::Principal,
    crypto::{random_bytes, random_token, sha256},
    devices,
    error::{AppError, AppResult, bad},
    extract::{Json, Path},
    jam, party,
    realtime::{self, Audience},
    routes::auth::open_web_session,
    session::{self, Method, NewSession},
    state::{AppState, ClientIp},
    users,
};

/// Знаки кода с экрана: без 0/O, 1/I/L — их путают, читая с телевизора.
const ALPHABET: &[u8] = b"ABCDEFGHJKMNPQRSTUVWXYZ23456789";
const CODE_LEN: usize = 8;
const MINUTE: Duration = Duration::from_secs(60);

/// Под `/api/auth`: код и опрос — без входа, подтверждение — у вошедшего.
pub fn auth_routes() -> Router<AppState> {
    Router::new()
        .route("/tv/code", post(code))
        .route("/tv/token", post(token))
        .route("/tv/approve", post(approve))
        .route("/tv/{code}", get(lookup))
}

/// Под правом play.
pub fn party_routes() -> Router<AppState> {
    Router::new().route("/party", get(get_party).patch(update_party))
}

/// Новый код: 8 случайных знаков из [`ALPHABET`] (31⁸ ≈ 8,5·10¹¹ вариантов).
fn new_user_code() -> AppResult<String> {
    let bytes = random_bytes::<16>()?;
    // Отбрасываем байты за кратным длине алфавита: так знаки равновероятны
    let limit = 256 - 256 % ALPHABET.len();
    let code: String = bytes
        .iter()
        .filter(|b| usize::from(**b) < limit)
        .take(CODE_LEN)
        .map(|b| ALPHABET[usize::from(*b) % ALPHABET.len()] as char)
        .collect();
    if code.len() < CODE_LEN {
        // 16 байт почти всегда дают 8 годных; если нет — ещё раз
        return new_user_code();
    }
    Ok(code)
}

/// Код, как его ввёл человек: регистр, пробелы и дефис не важны.
pub fn normalize_code(raw: &str) -> Option<String> {
    let code: String = raw
        .chars()
        .filter(|c| !c.is_whitespace() && *c != '-')
        .map(|c| c.to_ascii_uppercase())
        .collect();
    (code.len() == CODE_LEN && code.bytes().all(|b| ALPHABET.contains(&b))).then_some(code)
}

/// `KQ7M4TXP` → `KQ7M-4TXP`: так его легче прочитать с экрана.
pub fn display_code(code: &str) -> String {
    let (a, b) = code.split_at(code.len() / 2);
    format!("{a}-{b}")
}

fn unknown_code() -> AppError {
    AppError::Missing {
        code: "tv_code_unknown",
        message: "Код не подошёл: проверьте его на экране телевизора".into(),
    }
}

fn user_agent(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(header::USER_AGENT)
        .and_then(|v| v.to_str().ok())
}

#[utoipa::path(
    post,
    path = "/api/auth/tv/code",
    tag = "auth",
    request_body = TvCodeRequest,
    responses(
        (status = 200, description = "Код для экрана телевизора и секрет для опроса; живёт 10 минут", body = TvCode),
        (status = 400, description = "Платформа — не телевизор (`tvos` или `web`)", body = ErrorBody),
        (status = 429, description = "Больше 10 кодов в минуту с одного адреса", body = ErrorBody),
    ),
)]
pub async fn code(
    State(st): State<AppState>,
    ClientIp(ip): ClientIp,
    Json(body): Json<TvCodeRequest>,
) -> AppResult<Json<TvCode>> {
    if !st.limiter.check("tv-code", ip, 10, MINUTE) {
        return Err(AppError::TooManyRequests);
    }
    let default_name = match body.platform {
        Platform::Tvos => devices::default_name(Platform::Tvos),
        Platform::Web => "Телевизор",
        _ => {
            return Err(bad(
                "platform: вход кодом — только для телевизора (tvos или web)",
            ));
        }
    };
    let name = body
        .device_name
        .as_deref()
        .map(|n| n.trim().chars().take(64).collect::<String>())
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| default_name.to_string());
    sqlx::query!("DELETE FROM tv_logins WHERE expires_at < now() - interval '1 day'")
        .execute(&st.db)
        .await?;
    let device_code = random_token()?;
    // Совпадение живого кода почти невероятно, но уникальный индекс его не пропустит — пробуем ещё
    let mut tries = 0;
    let (user_code, expires_at) = loop {
        let user_code = new_user_code()?;
        let row = sqlx::query_scalar!(
            "INSERT INTO tv_logins (device_code_hash, user_code, platform, device_name, expires_at)
             VALUES ($1, $2, $3, $4, now() + make_interval(secs => $5))
             ON CONFLICT (user_code) WHERE used_at IS NULL DO NOTHING
             RETURNING expires_at",
            &sha256(&device_code)[..],
            user_code,
            body.platform.as_str(),
            name,
            TV_CODE_TTL_SECS as f64,
        )
        .fetch_optional(&st.db)
        .await?;
        if let Some(at) = row {
            break (user_code, at);
        }
        // Занят живым кодом: истёкшие освобождаем, следующий — другой
        sqlx::query!(
            "UPDATE tv_logins SET used_at = now() WHERE user_code = $1 AND used_at IS NULL AND expires_at < now()",
            user_code
        )
        .execute(&st.db)
        .await?;
        tries += 1;
        if tries > 5 {
            return Err(AppError::Unavailable(
                "не удалось выдать код, попробуйте ещё раз".into(),
            ));
        }
    };
    let shown = display_code(&user_code);
    let verification_url = format!("{}/tv", st.config.public_url);
    Ok(Json(TvCode {
        device_code,
        verification_url_complete: format!("{verification_url}?code={user_code}"),
        verification_url,
        user_code: shown,
        expires_at,
        interval_secs: TV_POLL_SECS,
    }))
}

#[utoipa::path(
    post,
    path = "/api/auth/tv/token",
    tag = "auth",
    request_body = TvTokenRequest,
    responses(
        (status = 200, description = "`pending` — ещё не подтвердили; `ready` — токен Apple TV (один раз); \
                                      `signed_in` — браузеру поставлена cookie `dw_session`", body = TvToken),
        (status = 400, description = "Код неверный, истёк или уже использован (`invalid_grant`)", body = ErrorBody),
        (status = 403, description = "Аккаунт заблокирован", body = ErrorBody),
        (status = 429, description = "Больше 60 запросов в минуту с одного адреса", body = ErrorBody),
    ),
)]
pub async fn token(
    State(st): State<AppState>,
    ClientIp(ip): ClientIp,
    headers: HeaderMap,
    jar: CookieJar,
    Json(body): Json<TvTokenRequest>,
) -> AppResult<(CookieJar, Json<TvToken>)> {
    if !st.limiter.check("tv-token", ip, 60, MINUTE) {
        return Err(AppError::TooManyRequests);
    }
    let gone = || AppError::InvalidGrant("код неверный, истёк или уже использован".into());
    let hash = sha256(&body.device_code);
    let mut tx = st.db.begin().await?;
    // Подтверждённый живой код гасится здесь же: сессия выдаётся один раз
    let claimed = sqlx::query!(
        "UPDATE tv_logins SET used_at = now()
         WHERE device_code_hash = $1 AND used_at IS NULL AND approved_at IS NOT NULL AND expires_at > now()
         RETURNING id, user_id, auth_method, platform, device_name",
        &hash[..],
    )
    .fetch_optional(&mut *tx)
    .await?;
    let Some(row) = claimed else {
        let waiting = sqlx::query_scalar!(
            "SELECT id FROM tv_logins
             WHERE device_code_hash = $1 AND used_at IS NULL AND approved_at IS NULL AND expires_at > now()",
            &hash[..],
        )
        .fetch_optional(&mut *tx)
        .await?;
        tx.commit().await?;
        return match waiting {
            Some(_) => Ok((jar, Json(TvToken::Pending))),
            None => Err(gone()),
        };
    };
    let user = match row.user_id {
        Some(id) => users::get(&st, id).await?,
        None => None,
    }
    .ok_or_else(gone)?;
    if user.disabled {
        tx.commit().await?;
        return Err(AppError::Forbidden("аккаунт заблокирован".into()));
    }
    let method = Method::parse(row.auth_method.as_deref().unwrap_or_default());
    let platform: Platform = row.platform.parse().map_err(anyhow::Error::from)?;
    if platform == Platform::Web {
        tx.commit().await?;
        let (jar, session_id) = open_web_session(&st, jar, &headers, ip, user.id, method).await?;
        sqlx::query!(
            "UPDATE tv_logins SET session_id = $2 WHERE id = $1",
            row.id,
            session_id
        )
        .execute(&st.db)
        .await?;
        return Ok((
            jar,
            Json(TvToken::SignedIn(TvSignedIn {
                user: user.public(),
            })),
        ));
    }
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
        method,
        user_agent: user_agent(&headers),
        ip,
    };
    let (session_id, token) = session::create(&mut tx, new).await?;
    sqlx::query!(
        "UPDATE tv_logins SET session_id = $2 WHERE id = $1",
        row.id,
        session_id
    )
    .execute(&mut *tx)
    .await?;
    let payload = json!({
        "via": "tv",
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
    Ok((
        jar,
        Json(TvToken::Ready(AppToken {
            token,
            device: device.view(),
            user: user.public(),
        })),
    ))
}

struct Pending {
    user_code: String,
    platform: String,
    device_name: String,
    expires_at: chrono::DateTime<Utc>,
}

async fn pending(st: &AppState, code: &str) -> AppResult<Pending> {
    let code = normalize_code(code).ok_or_else(unknown_code)?;
    sqlx::query_as!(
        Pending,
        "SELECT user_code, platform, device_name, expires_at FROM tv_logins
         WHERE user_code = $1 AND used_at IS NULL AND approved_at IS NULL AND expires_at > now()",
        code,
    )
    .fetch_optional(&st.db)
    .await?
    .ok_or_else(unknown_code)
}

#[utoipa::path(
    get,
    path = "/api/auth/tv/{code}",
    tag = "auth",
    security(("session" = []), ("device" = [])),
    params(("code" = String, Path, description = "Код с экрана телевизора")),
    responses(
        (status = 200, description = "Какой телевизор просит вход", body = TvLogin),
        (status = 401, description = "Нет сессии", body = ErrorBody),
        (status = 404, description = "Кода нет, он истёк или уже подтверждён (`tv_code_unknown`)", body = ErrorBody),
        (status = 429, description = "Больше 20 проверок в минуту с одного адреса", body = ErrorBody),
    ),
)]
pub async fn lookup(
    State(st): State<AppState>,
    ClientIp(ip): ClientIp,
    principal: Principal,
    Path(code): Path<String>,
) -> AppResult<Json<TvLogin>> {
    principal.need(dalywave_protocol::auth::Permission::Play)?;
    if !st.limiter.check("tv-lookup", ip, 20, MINUTE) {
        return Err(AppError::TooManyRequests);
    }
    let p = pending(&st, &code).await?;
    Ok(Json(TvLogin {
        user_code: display_code(&p.user_code),
        platform: p.platform.parse().map_err(anyhow::Error::from)?,
        device_name: p.device_name,
        expires_at: p.expires_at,
    }))
}

#[utoipa::path(
    post,
    path = "/api/auth/tv/approve",
    tag = "auth",
    security(("session" = []), ("device" = [])),
    request_body = TvApprove,
    responses(
        (status = 204, description = "Вход подтверждён: телевизор получит сессию при следующем опросе"),
        (status = 401, description = "Нет сессии", body = ErrorBody),
        (status = 404, description = "Кода нет, он истёк или уже подтверждён (`tv_code_unknown`)", body = ErrorBody),
        (status = 429, description = "Больше 10 попыток в минуту с одного адреса", body = ErrorBody),
    ),
)]
pub async fn approve(
    State(st): State<AppState>,
    ClientIp(ip): ClientIp,
    principal: Principal,
    Json(body): Json<TvApprove>,
) -> AppResult<StatusCode> {
    principal.need(dalywave_protocol::auth::Permission::Play)?;
    if !st.limiter.check("tv-approve", ip, 10, MINUTE) {
        return Err(AppError::TooManyRequests);
    }
    let code = normalize_code(&body.user_code).ok_or_else(unknown_code)?;
    // Способ входа подтвердившего: сессия телевизора не «сильнее» его собственной
    let method = sqlx::query_scalar!(
        "SELECT auth_method FROM sessions WHERE id = $1",
        principal.session_id
    )
    .fetch_optional(&st.db)
    .await?
    .unwrap_or_else(|| "oidc".into());
    let approved = sqlx::query!(
        "UPDATE tv_logins SET user_id = $2, auth_method = $3, approved_at = now()
         WHERE user_code = $1 AND used_at IS NULL AND approved_at IS NULL AND expires_at > now()",
        code,
        principal.user.id,
        method,
    )
    .execute(&st.db)
    .await?;
    if approved.rows_affected() == 0 {
        return Err(unknown_code());
    }
    Ok(StatusCode::NO_CONTENT)
}

// ---------- режим вечеринки ----------

#[utoipa::path(
    get,
    path = "/api/party",
    tag = "jam",
    security(("session" = []), ("device" = [])),
    responses((status = 200, description = "Идёт ли вечеринка и её настройки", body = Party)),
)]
pub async fn get_party(State(st): State<AppState>, principal: Principal) -> AppResult<Json<Party>> {
    Ok(Json(party::of(&st.db, principal.user.id).await?))
}

#[utoipa::path(
    patch,
    path = "/api/party",
    tag = "jam",
    security(("session" = []), ("device" = [])),
    request_body = PartyPatch,
    responses((
        status = 200,
        description = "Вечеринка с новыми настройками. Включение открывает «Слушать вместе» (QR для гостей) \
                       и продлевает вечеринку на 12 часов; всем устройствам — `party.changed`",
        body = Party,
    )),
)]
pub async fn update_party(
    State(st): State<AppState>,
    principal: Principal,
    Json(patch): Json<PartyPatch>,
) -> AppResult<Json<Party>> {
    let user = principal.user.id;
    let until = match patch.active {
        Some(true) => Some(Some(Utc::now() + chrono::Duration::hours(PARTY_HOURS))),
        Some(false) => Some(None),
        None => None,
    };
    sqlx::query!(
        "INSERT INTO parties (user_id, active_until, energetic, no_artist_repeat, dj)
         VALUES ($1, CASE WHEN $2 THEN $3::timestamptz ELSE NULL END,
                 coalesce($4, false), coalesce($5, true), coalesce($6, true))
         ON CONFLICT (user_id) DO UPDATE SET
             active_until = CASE WHEN $2 THEN $3::timestamptz ELSE parties.active_until END,
             energetic = coalesce($4, parties.energetic),
             no_artist_repeat = coalesce($5, parties.no_artist_repeat),
             dj = coalesce($6, parties.dj),
             updated_at = now()",
        user,
        until.is_some(),
        until.flatten(),
        patch.energetic,
        patch.no_artist_repeat,
        patch.dj,
    )
    .execute(&st.db)
    .await?;
    if patch.active == Some(true) {
        open_jam(&st, user).await?;
    }
    let out = party::of(&st.db, user).await?;
    realtime::deliver(
        &st.hub,
        Audience::User(user),
        ServerEvent::PartyChanged(out.clone()),
    );
    Ok(Json(out))
}

/// «Слушать вместе» для QR на сцене: открытая сессия или новая.
async fn open_jam(st: &AppState, user: Uuid) -> AppResult<()> {
    if let Some(s) = jam::open_of(&st.db, user).await? {
        jam::touch(&st.db, s.id, None).await?;
        return Ok(());
    }
    sqlx::query!(
        "INSERT INTO jam_sessions (user_id, code) VALUES ($1, $2) ON CONFLICT DO NOTHING",
        user,
        jam::new_code()?,
    )
    .execute(&st.db)
    .await?;
    if let Some(s) = jam::open_of(&st.db, user).await? {
        jam::changed(st, &s, false, None);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_are_readable_and_forgiving() {
        let code = new_user_code().unwrap();
        assert_eq!(code.len(), CODE_LEN);
        assert!(code.bytes().all(|b| ALPHABET.contains(&b)));
        assert_eq!(normalize_code(" kq7m-4txp "), Some("KQ7M4TXP".into()));
        assert_eq!(normalize_code("KQ7M 4TXP"), Some("KQ7M4TXP".into()));
        assert_eq!(normalize_code("KQ7M4TX0"), None, "ноль в коде не бывает");
        assert_eq!(normalize_code("KQ7M4T"), None);
        assert_eq!(display_code("KQ7M4TXP"), "KQ7M-4TXP");
    }
}
