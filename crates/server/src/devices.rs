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

pub async fn get(db: &PgPool, id: Uuid) -> AppResult<Option<DeviceRow>> {
    Ok(sqlx::query_as!(
        DeviceRow,
        "SELECT id, name, platform, app_version, created_at, last_seen_at FROM devices WHERE id = $1",
        id,
    )
    .fetch_optional(db)
    .await?)
}
