//! «Система» админки: здоровье сервера, резервные копии базы, выпуски приложений.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

// ───────────── Здоровье ─────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum CheckState {
    Ok,
    /// Работает, но есть на что посмотреть.
    Warn,
    Down,
    /// На этом сервере не включено.
    Off,
}

/// Проверка одной части сервера.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct HealthCheck {
    /// `database`, `storage`, `realtime`, `worker`, `ml`, `fetcher`, `backups`.
    pub key: String,
    pub state: CheckState,
    /// Сколько ответа ждали, мс.
    #[schema(required = true)]
    pub latency_ms: Option<i64>,
    /// Короткое пояснение для человека или текст ошибки.
    #[schema(required = true)]
    pub detail: Option<String>,
}

/// Задача, упавшая за сутки.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct HealthFailure {
    pub job_id: Uuid,
    pub kind: String,
    pub lane: String,
    #[schema(required = true)]
    pub error: Option<String>,
    pub at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AdminHealth {
    pub version: String,
    pub uptime_s: u64,
    pub checks: Vec<HealthCheck>,
    /// Размер базы, байт.
    pub db_bytes: i64,
    /// Открытых соединений WebSocket.
    pub connections: u64,
    pub queued: i64,
    pub running: i64,
    pub failed_24h: i64,
    /// Последние упавшие задачи (до 20).
    pub failures: Vec<HealthFailure>,
}

// ───────────── Резервные копии ─────────────

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct BackupFile {
    pub name: String,
    pub size_bytes: i64,
    pub created_at: DateTime<Utc>,
}

/// Итог пробной проверки копии.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct BackupCheck {
    pub name: String,
    pub ok: bool,
    /// `sql` — дамп распакован целиком (контрольная сумма gzip), проверены начало и конец дампа.
    pub method: String,
    /// Сколько объектов создаёт копия (таблицы, индексы, функции…).
    #[schema(required = true)]
    pub entries: Option<i64>,
    /// Таблиц с данными в копии.
    #[schema(required = true)]
    pub tables: Option<i64>,
    #[schema(required = true)]
    pub error: Option<String>,
    pub checked_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Backups {
    /// Папка с копиями задана (`DALYWAVE_BACKUP_DIR`) и читается.
    pub configured: bool,
    #[schema(required = true)]
    pub dir: Option<String>,
    /// Свежие первыми.
    pub files: Vec<BackupFile>,
    pub total_bytes: i64,
    /// Последняя проверка копии.
    #[schema(required = true)]
    pub last_check: Option<BackupCheck>,
}

// ───────────── Выпуски ─────────────

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseItem {
    pub id: Uuid,
    /// `darwin`, `windows`, `linux`, `streamdeck`.
    pub target: String,
    pub arch: String,
    pub version: String,
    pub size_bytes: i64,
    /// Есть установщик для сайта.
    pub installer: bool,
    #[schema(required = true)]
    pub notes: Option<String>,
    pub published_at: DateTime<Utc>,
}

/// Сколько устройств на этой версии.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct VersionShare {
    /// Платформа устройства: `macos`, `windows`, `ios`, `streamdeck`…
    pub platform: String,
    /// `null` — версия не сообщена (старое приложение).
    #[schema(required = true)]
    pub version: Option<String>,
    pub devices: i64,
    pub people: i64,
    /// Есть выпуск новее для этой платформы.
    pub outdated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AppReleases {
    /// Версия сервера (и сайта — он собирается вместе с ним).
    pub server_version: String,
    /// Последний выпуск по каждой цели, затем остальные — свежие первыми (до 40).
    pub items: Vec<ReleaseItem>,
    /// Последняя версия по цели: `darwin` → `0.1.16`.
    pub latest: Vec<LatestRelease>,
    /// Приложения, заходившие за 60 дней, по версиям.
    pub installs: Vec<VersionShare>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct LatestRelease {
    pub target: String,
    pub version: String,
    pub published_at: DateTime<Utc>,
}
