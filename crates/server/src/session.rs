//! Сессии: cookie `dw_session` у сайта и токен `dwd_…` у приложений. В базе — только SHA-256 токена.
//! Срок скользящий: заход продлевает его, но в базу это пишется не чаще раза в минуту.

use std::net::IpAddr;

use axum_extra::extract::cookie::{Cookie, SameSite};
use chrono::{DateTime, Duration, Utc};
use dalywave_protocol::auth::SessionKind;
use ipnet::IpNet;
use sqlx::{PgConnection, PgPool};
use uuid::Uuid;

use crate::{
    config::Config,
    crypto::{random_token, sha256},
    error::AppResult,
};

pub const SESSION_COOKIE: &str = "dw_session";
pub const WEB_PREFIX: &str = "dws_";
pub const DEVICE_PREFIX: &str = "dwd_";
pub const WEB_TTL_DAYS: i32 = 90;
pub const DEVICE_TTL_DAYS: i32 = 365;

/// Как человек подтвердил вход.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Method {
    Oidc,
    Password,
}

impl Method {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Oidc => "oidc",
            Self::Password => "password",
        }
    }
}

pub struct NewSession<'a> {
    pub user_id: Uuid,
    /// Есть у сессии приложения, нет у сайта
    pub device_id: Option<Uuid>,
    pub method: Method,
    pub user_agent: Option<&'a str>,
    pub ip: IpAddr,
}

fn ttl_days(kind: SessionKind) -> i32 {
    match kind {
        SessionKind::Web => WEB_TTL_DAYS,
        SessionKind::Device => DEVICE_TTL_DAYS,
    }
}

/// Открывает сессию. Токен возвращается один раз: на сервере остаётся только его хеш.
pub async fn create(conn: &mut PgConnection, new: NewSession<'_>) -> AppResult<(Uuid, String)> {
    let (kind, prefix) = match new.device_id {
        Some(_) => (SessionKind::Device, DEVICE_PREFIX),
        None => (SessionKind::Web, WEB_PREFIX),
    };
    let token = format!("{prefix}{}", random_token()?);
    let user_agent: Option<String> = new.user_agent.map(|ua| ua.chars().take(300).collect());
    // Заодно убираем давно истёкшие и отозванные: в разделе «Доступ» они уже не видны
    sqlx::query!(
        "DELETE FROM sessions WHERE expires_at < now() - interval '30 days' OR revoked_at < now() - interval '30 days'"
    )
    .execute(&mut *conn)
    .await?;
    let id = sqlx::query_scalar!(
        "INSERT INTO sessions (user_id, device_id, kind, auth_method, token_hash, expires_at, user_agent, ip)
         VALUES ($1, $2, $3, $4, $5, now() + make_interval(days => $6), $7, $8)
         RETURNING id",
        new.user_id,
        new.device_id,
        kind.as_str(),
        new.method.as_str(),
        &sha256(&token)[..],
        ttl_days(kind),
        user_agent,
        IpNet::from(new.ip),
    )
    .fetch_one(&mut *conn)
    .await?;
    sqlx::query!(
        "UPDATE users SET last_login_at = now() WHERE id = $1",
        new.user_id
    )
    .execute(&mut *conn)
    .await?;
    Ok((id, token))
}

/// Живая сессия с данными человека.
pub struct Found {
    pub id: Uuid,
    pub user_id: Uuid,
    pub device_id: Option<Uuid>,
    pub auth_method: String,
    pub last_seen_at: DateTime<Utc>,
    pub user_name: String,
    pub email: Option<String>,
    pub avatar_url: Option<String>,
    pub roles: Vec<String>,
    pub disabled: bool,
}

/// Ищет живую сессию по токену и сдвигает её срок. Токен сайта не годится как токен приложения и наоборот.
pub async fn find(db: &PgPool, token: &str, kind: SessionKind) -> AppResult<Option<Found>> {
    let found = sqlx::query_as!(
        Found,
        r#"SELECT s.id, s.user_id, s.device_id, s.auth_method, s.last_seen_at,
                  u.name AS user_name, u.email, u.avatar_url, u.roles, u.disabled
           FROM sessions s
           JOIN users u ON u.id = s.user_id
           LEFT JOIN devices d ON d.id = s.device_id
           WHERE s.token_hash = $1 AND s.kind = $2
             AND s.revoked_at IS NULL AND s.expires_at > now() AND d.revoked_at IS NULL"#,
        &sha256(token)[..],
        kind.as_str(),
    )
    .fetch_optional(db)
    .await?;
    if let Some(s) = &found
        && s.last_seen_at < Utc::now() - Duration::minutes(1)
    {
        sqlx::query!(
            "WITH touched AS (
                UPDATE sessions SET last_seen_at = now(), expires_at = now() + make_interval(days => $2)
                WHERE id = $1 AND last_seen_at < now() - interval '1 minute'
                RETURNING device_id
             )
             UPDATE devices SET last_seen_at = now() WHERE id = (SELECT device_id FROM touched)",
            s.id,
            ttl_days(kind),
        )
        .execute(db)
        .await?;
    }
    Ok(found)
}

/// Что отозвано: владелец и устройство.
pub struct Revoked {
    pub id: Uuid,
    pub user_id: Uuid,
    pub device_id: Option<Uuid>,
}

/// Отзывает сессию. У приложения отзывается и устройство: без сессии его токен ничего не значит.
pub async fn revoke(conn: &mut PgConnection, id: Uuid) -> AppResult<Option<Revoked>> {
    let revoked = sqlx::query_as!(
        Revoked,
        "UPDATE sessions SET revoked_at = now() WHERE id = $1 AND revoked_at IS NULL RETURNING id, user_id, device_id",
        id,
    )
    .fetch_optional(&mut *conn)
    .await?;
    if let Some(device) = revoked.as_ref().and_then(|r| r.device_id) {
        sqlx::query!(
            "UPDATE devices SET revoked_at = now() WHERE id = $1 AND revoked_at IS NULL",
            device
        )
        .execute(&mut *conn)
        .await?;
        crate::devices::revoke_paired(conn, device).await?;
    }
    Ok(revoked)
}

pub async fn revoke_token(conn: &mut PgConnection, token: &str) -> AppResult<Option<Revoked>> {
    let id = sqlx::query_scalar!(
        "SELECT id FROM sessions WHERE token_hash = $1 AND revoked_at IS NULL",
        &sha256(token)[..]
    )
    .fetch_optional(&mut *conn)
    .await?;
    match id {
        Some(id) => revoke(conn, id).await,
        None => Ok(None),
    }
}

/// Все сессии и устройства человека, например при блокировке.
pub async fn revoke_all(conn: &mut PgConnection, user_id: Uuid) -> AppResult<u64> {
    let res = sqlx::query!(
        "UPDATE sessions SET revoked_at = now() WHERE user_id = $1 AND revoked_at IS NULL",
        user_id
    )
    .execute(&mut *conn)
    .await?;
    sqlx::query!(
        "UPDATE devices SET revoked_at = now() WHERE user_id = $1 AND revoked_at IS NULL",
        user_id
    )
    .execute(&mut *conn)
    .await?;
    Ok(res.rows_affected())
}

/// Cookie живёт долго, срок задаёт запись в базе: так он скользит без перевыпуска cookie.
pub fn cookie(config: &Config, token: String) -> Cookie<'static> {
    Cookie::build((SESSION_COOKIE, token))
        .http_only(true)
        .secure(config.cookie_secure())
        .same_site(SameSite::Lax)
        .path("/")
        .max_age(time::Duration::days(400))
        .build()
}

pub fn removal_cookie(config: &Config) -> Cookie<'static> {
    Cookie::build((SESSION_COOKIE, ""))
        .http_only(true)
        .secure(config.cookie_secure())
        .same_site(SameSite::Lax)
        .path("/")
        .build()
}
