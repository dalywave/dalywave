//! Домашние загрузчики: токен `dwk_…` (в базе — SHA-256), вход по нему и учёт «на связи».
//!
//! Токен даёт одно право — `downloader`: брать задания и отчитываться через `/api/fetcher/*`.
//! Сессии людей и токены приложений там не принимаются, а токен загрузчика не открывает ничего,
//! кроме этих маршрутов: `auth::authenticate` знает только `dwd_…` и cookie.

use std::net::IpAddr;

use axum::{extract::FromRequestParts, http::request::Parts};
use chrono::{DateTime, Utc};
use dalywave_protocol::fetcher::{Capabilities, ClaimRequest, Downloader};
use ipnet::IpNet;
use serde_json::Value;
use sqlx::{PgConnection, PgExecutor, PgPool};
use uuid::Uuid;

use crate::{
    auth,
    crypto::{random_token, sha256},
    error::{AppError, AppResult},
    state::AppState,
};

pub const PREFIX: &str = "dwk_";
/// Исполнитель в `jobs.locked_by`, `acquisitions.worker` и `audio_files.acquired_by`.
const EXECUTOR_PREFIX: &str = "device:";

pub fn executor(id: Uuid) -> String {
    format!("{EXECUTOR_PREFIX}{id}")
}

/// Загрузчик из `Authorization: Bearer dwk_…`.
#[derive(Debug, Clone)]
pub struct DownloaderAuth {
    pub id: Uuid,
    pub name: String,
}

impl DownloaderAuth {
    pub fn executor(&self) -> String {
        executor(self.id)
    }
}

impl FromRequestParts<AppState> for DownloaderAuth {
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, st: &AppState) -> Result<Self, Self::Rejection> {
        if let Some(found) = parts.extensions.get::<DownloaderAuth>() {
            return Ok(found.clone());
        }
        let token = auth::bearer(&parts.headers)
            .filter(|t| t.starts_with(PREFIX))
            .ok_or(AppError::Unauthorized)?;
        let found = sqlx::query_as!(
            DownloaderAuth,
            "SELECT id, name FROM downloaders WHERE token_hash = $1 AND revoked_at IS NULL",
            &sha256(token)[..],
        )
        .fetch_optional(&st.db)
        .await?
        .ok_or(AppError::Unauthorized)?;
        parts.extensions.insert(found.clone());
        Ok(found)
    }
}

/// Новый загрузчик. Токен возвращается один раз: на сервере остаётся только его хеш.
pub async fn create(
    conn: &mut PgConnection,
    name: &str,
    created_by: Uuid,
) -> AppResult<(Uuid, String)> {
    let token = format!("{PREFIX}{}", random_token()?);
    let id = sqlx::query_scalar!(
        "INSERT INTO downloaders (name, token_hash, created_by) VALUES ($1, $2, $3) RETURNING id",
        name,
        &sha256(&token)[..],
        created_by,
    )
    .fetch_one(&mut *conn)
    .await?;
    Ok((id, token))
}

/// Загрузчик обратился: «на связи». С запросом задания — заодно его версия и возможности.
pub async fn seen(
    db: &PgPool,
    id: Uuid,
    ip: IpAddr,
    hello: Option<&ClaimRequest>,
) -> Result<(), sqlx::Error> {
    let worker_id = hello.map(|h| h.worker_id.chars().take(100).collect::<String>());
    let version = hello.and_then(|h| {
        h.version
            .as_ref()
            .map(|v| v.chars().take(40).collect::<String>())
    });
    let yt_dlp = hello.and_then(|h| {
        h.capabilities
            .yt_dlp
            .as_ref()
            .map(|v| v.chars().take(40).collect::<String>())
    });
    let capabilities = hello.and_then(|h| serde_json::to_value(&h.capabilities).ok());
    sqlx::query!(
        "UPDATE downloaders SET last_seen_at = now(), last_ip = $2,
                last_claim_at = CASE WHEN $3 THEN now() ELSE last_claim_at END,
                worker_id = COALESCE($4, worker_id), version = COALESCE($5, version),
                yt_dlp = COALESCE($6, yt_dlp), capabilities = COALESCE($7, capabilities)
         WHERE id = $1",
        id,
        IpNet::from(ip),
        hello.is_some(),
        worker_id,
        version,
        yt_dlp,
        capabilities,
    )
    .execute(db)
    .await?;
    Ok(())
}

/// Версия yt-dlp из отчёта: у старых загрузчиков её нет в запросе задания.
pub async fn report_versions(
    db: &PgPool,
    id: Uuid,
    yt_dlp: Option<&str>,
) -> Result<(), sqlx::Error> {
    let Some(yt_dlp) = yt_dlp else {
        return Ok(());
    };
    sqlx::query!(
        "UPDATE downloaders SET yt_dlp = $2 WHERE id = $1",
        id,
        yt_dlp.chars().take(40).collect::<String>(),
    )
    .execute(db)
    .await?;
    Ok(())
}

/// Сколько загрузчиков на связи: обращались в последние две минуты. Ждущий задания переспрашивает
/// каждые ~25 секунд, работающий продлевает аренду раз в минуту.
pub async fn online(db: impl PgExecutor<'_>) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar!(
        r#"SELECT count(*) AS "n!" FROM downloaders
           WHERE revoked_at IS NULL AND last_seen_at > now() - interval '2 minutes'"#
    )
    .fetch_one(db)
    .await
}

struct Row {
    id: Uuid,
    name: String,
    created_at: DateTime<Utc>,
    revoked_at: Option<DateTime<Utc>>,
    last_seen_at: Option<DateTime<Utc>>,
    last_claim_at: Option<DateTime<Utc>>,
    online: bool,
    worker_id: Option<String>,
    version: Option<String>,
    yt_dlp: Option<String>,
    capabilities: Value,
    running: i64,
    ok: i64,
    failed: i64,
}

impl Row {
    fn view(self) -> Downloader {
        Downloader {
            id: self.id,
            name: self.name,
            created_at: self.created_at,
            revoked_at: self.revoked_at,
            last_seen_at: self.last_seen_at,
            last_claim_at: self.last_claim_at,
            online: self.online,
            worker_id: self.worker_id,
            version: self.version,
            yt_dlp_version: self.yt_dlp,
            capabilities: serde_json::from_value::<Capabilities>(self.capabilities)
                .unwrap_or_default(),
            running: self.running,
            ok_24h: self.ok,
            failed_24h: self.failed,
        }
    }
}

/// Загрузчики с работой за сутки: живые первыми, отозванные в конце.
pub async fn list(db: &PgPool, only: Option<Uuid>) -> Result<Vec<Downloader>, sqlx::Error> {
    let rows = sqlx::query_as!(
        Row,
        r#"SELECT d.id, d.name, d.created_at, d.revoked_at, d.last_seen_at, d.last_claim_at,
                  COALESCE(d.revoked_at IS NULL AND d.last_seen_at > now() - interval '2 minutes', false) AS "online!",
                  d.worker_id, d.version, d.yt_dlp, d.capabilities,
                  (SELECT count(*) FROM jobs j WHERE j.status = 'running' AND j.locked_by = 'device:' || d.id) AS "running!",
                  (SELECT count(*) FROM acquisition_attempts a
                   WHERE a.executor = 'device:' || d.id AND a.created_at > now() - interval '24 hours'
                     AND a.outcome = 'ready') AS "ok!",
                  (SELECT count(*) FROM acquisition_attempts a
                   WHERE a.executor = 'device:' || d.id AND a.created_at > now() - interval '24 hours'
                     AND a.outcome IN ('retry', 'failed', 'postponed')) AS "failed!"
           FROM downloaders d
           WHERE $1::uuid IS NULL OR d.id = $1
           ORDER BY d.revoked_at IS NOT NULL, d.last_seen_at DESC NULLS LAST, d.created_at DESC"#,
        only,
    )
    .fetch_all(db)
    .await?;
    Ok(rows.into_iter().map(Row::view).collect())
}

pub async fn get(db: &PgPool, id: Uuid) -> Result<Option<Downloader>, sqlx::Error> {
    Ok(list(db, Some(id)).await?.into_iter().next())
}

/// Отзыв: токен перестаёт действовать сразу. `false` — нет такого живого загрузчика.
pub async fn revoke(conn: &mut PgConnection, id: Uuid) -> Result<bool, sqlx::Error> {
    let res = sqlx::query!(
        "UPDATE downloaders SET revoked_at = now() WHERE id = $1 AND revoked_at IS NULL",
        id
    )
    .execute(&mut *conn)
    .await?;
    Ok(res.rows_affected() == 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn executor_names() {
        let id = Uuid::nil();
        assert_eq!(executor(id), format!("device:{id}"));
    }
}
