//! Устройства с приложением. У каждого своя сессия и свой токен `dwd_…`.

use chrono::{DateTime, Utc};
use dalywave_protocol::auth::{Device, Platform};
use sqlx::{PgConnection, PgPool};
use uuid::Uuid;

use crate::error::AppResult;

pub struct DeviceRow {
    pub id: Uuid,
    pub name: String,
    pub platform: String,
    pub app_version: Option<String>,
    pub created_at: DateTime<Utc>,
    pub last_seen_at: Option<DateTime<Utc>>,
}

impl DeviceRow {
    pub fn view(&self) -> Device {
        Device {
            id: self.id,
            name: self.name.clone(),
            // Столбец ограничен CHECK теми же значениями, иного там не бывает
            platform: self.platform.parse().unwrap_or(Platform::Web),
            app_version: self.app_version.clone(),
            created_at: self.created_at,
            last_seen_at: self.last_seen_at,
        }
    }
}

/// Имя устройства, если приложение его не прислало.
pub fn default_name(platform: Platform) -> &'static str {
    match platform {
        Platform::Web => "Браузер",
        Platform::Macos => "Mac",
        Platform::Windows => "Компьютер с Windows",
        Platform::Linux => "Компьютер с Linux",
        Platform::Ios => "iPhone",
        Platform::Ipados => "iPad",
        Platform::Android => "Android",
        Platform::Watchos => "Apple Watch",
        Platform::Tvos => "Apple TV",
        Platform::Streamdeck => "Stream Deck",
    }
}

pub async fn create(
    conn: &mut PgConnection,
    user_id: Uuid,
    name: &str,
    platform: &str,
    app_version: Option<&str>,
) -> AppResult<DeviceRow> {
    Ok(sqlx::query_as!(
        DeviceRow,
        "INSERT INTO devices (user_id, name, platform, app_version, last_seen_at) VALUES ($1, $2, $3, $4, now())
         RETURNING id, name, platform, app_version, created_at, last_seen_at",
        user_id,
        name,
        platform,
        app_version,
    )
    .fetch_one(conn)
    .await?)
}

/// Сопряжённое устройство (часы) уже вошедшего приложения `parent`. Прежнее сопряжённое устройство
/// той же платформы у этого приложения отзывается вместе с сессией: часы просят ключ заново после
/// сброса, и отозванные «Apple Watch» не копятся в разделе «Доступ».
pub async fn create_paired(
    conn: &mut PgConnection,
    user_id: Uuid,
    parent: Uuid,
    name: &str,
    platform: &str,
    app_version: Option<&str>,
) -> AppResult<DeviceRow> {
    let replaced = sqlx::query_scalar!(
        "UPDATE devices SET revoked_at = now()
         WHERE paired_from = $1 AND platform = $2 AND revoked_at IS NULL
         RETURNING id",
        parent,
        platform,
    )
    .fetch_all(&mut *conn)
    .await?;
    if !replaced.is_empty() {
        sqlx::query!(
            "UPDATE sessions SET revoked_at = now() WHERE device_id = ANY($1) AND revoked_at IS NULL",
            &replaced,
        )
        .execute(&mut *conn)
        .await?;
    }
    Ok(sqlx::query_as!(
        DeviceRow,
        "INSERT INTO devices (user_id, name, platform, app_version, last_seen_at, paired_from)
         VALUES ($1, $2, $3, $4, now(), $5)
         RETURNING id, name, platform, app_version, created_at, last_seen_at",
        user_id,
        name,
        platform,
        app_version,
        parent,
    )
    .fetch_one(conn)
    .await?)
}

/// Отзывает устройства, сопряжённые с `parent`, и их сессии: без iPhone часы не остаются в системе.
pub async fn revoke_paired(conn: &mut PgConnection, parent: Uuid) -> AppResult<()> {
    let ids = sqlx::query_scalar!(
        "UPDATE devices SET revoked_at = now() WHERE paired_from = $1 AND revoked_at IS NULL RETURNING id",
        parent,
    )
    .fetch_all(&mut *conn)
    .await?;
    if !ids.is_empty() {
        sqlx::query!(
            "UPDATE sessions SET revoked_at = now() WHERE device_id = ANY($1) AND revoked_at IS NULL",
            &ids,
        )
        .execute(&mut *conn)
        .await?;
    }
    Ok(())
}

pub async fn get(db: &PgPool, id: Uuid) -> AppResult<Option<DeviceRow>> {
    Ok(sqlx::query_as!(
        DeviceRow,
        "SELECT id, name, platform, app_version, created_at, last_seen_at FROM devices WHERE id = $1",
        id,
    )
    .fetch_optional(db)
    .await?)
}
