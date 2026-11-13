use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum HealthStatus {
    Ok,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Health {
    pub status: HealthStatus,
    pub version: String,
    pub db: HealthStatus,
    /// Сколько секунд работает процесс API.
    pub uptime_s: u64,
}

/// Где сервер хранит файлы.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum StorageKind {
    S3,
    Fs,
    Memory,
}

/// Что умеет этот сервер: интерфейс прячет выключенное. Отвечает без входа — экрану входа тоже нужно.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ServerConfig {
    pub version: String,
    /// Каталог Deezer: поиск по каталогу, чарты, новинки, похожие, страницы артистов целиком.
    pub deezer: bool,
    /// Загрузчик: скачивание записей, «Спорные», загрузчик на Mac, подписки с докачкой.
    pub downloader: bool,
    /// Своя музыка из папок на сервере подключена.
    pub library: bool,
    pub storage: StorageKind,
}
