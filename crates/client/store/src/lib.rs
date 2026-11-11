//! Локальная база приложений DalyWave (SQLite).
//!
//! Медиатека без аудио приходит синхронизацией (`/api/sync`, курсор и tombstones — `docs/data-model.md`):
//! просмотр и поиск работают без сети, а экраны получают те же типы, что и от сервера
//! (`dalywave_protocol`). Рядом — то, что знает только устройство: что закреплено скачанным, какие
//! файлы уже лежат на диске, набор волны устройства и история прослушиваний для волны без сети.
//!
//! Один файл базы на сервер и вход; доступ — из одного потока за раз (обёртка держит `Mutex`).

mod library;
mod offline;
mod schema;
mod sync;
mod text;

use std::path::Path;

use rusqlite::{Connection, OptionalExtension, params};

pub use library::LocalSearch;
pub use offline::{
    FileRecord, FileState, NewFile, Pin, PinState, PlayRecord, ReserveTrack, StoredFile,
    TrackFeatures, Usage,
};
pub use sync::Applied;
pub use text::normalize;

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("база: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("данные: {0}")]
    Json(#[from] serde_json::Error),
}

pub type StoreResult<T> = Result<T, StoreError>;

/// Локальная база.
pub struct Store {
    conn: Connection,
}

impl Store {
    /// Открыть (и при необходимости создать) базу в файле.
    pub fn open(path: &Path) -> StoreResult<Self> {
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let conn = Connection::open(path)?;
        Self::init(conn)
    }

    /// База в памяти — для тестов.
    pub fn memory() -> StoreResult<Self> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(conn: Connection) -> StoreResult<Self> {
        // WAL: чтение экранов не ждёт записи синхронизации
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        conn.pragma_update(None, "foreign_keys", "OFF")?;
        schema::migrate(&conn)?;
        Ok(Self { conn })
    }

    pub(crate) fn conn(&self) -> &Connection {
        &self.conn
    }

    /// Значение из служебной таблицы: курсор синхронизации, id устройства, настройки.
    pub fn meta(&self, key: &str) -> StoreResult<Option<String>> {
        Ok(self
            .conn
            .query_row("SELECT value FROM meta WHERE key = ?1", [key], |r| r.get(0))
            .optional()?)
    }

    pub fn set_meta(&self, key: &str, value: Option<&str>) -> StoreResult<()> {
        match value {
            Some(v) => self.conn.execute(
                "INSERT INTO meta (key, value) VALUES (?1, ?2)
                 ON CONFLICT (key) DO UPDATE SET value = excluded.value",
                params![key, v],
            )?,
            None => self
                .conn
                .execute("DELETE FROM meta WHERE key = ?1", [key])?,
        };
        Ok(())
    }

    /// Курсор синхронизации; `None` — полная синхронизация с начала.
    pub fn cursor(&self) -> StoreResult<Option<String>> {
        self.meta("sync.cursor")
    }

    /// Медиатека уже приходила хоть раз целиком: экраны можно показывать из базы.
    pub fn synced(&self) -> StoreResult<bool> {
        Ok(self.meta("sync.completed_at")?.is_some())
    }

    /// Устройство этого входа: набор волны и закреплённое других устройств сюда не относятся.
    pub fn device_id(&self) -> StoreResult<Option<uuid::Uuid>> {
        Ok(self.meta("device.id")?.and_then(|s| s.parse().ok()))
    }

    pub fn set_device_id(&self, id: Option<uuid::Uuid>) -> StoreResult<()> {
        self.set_meta("device.id", id.map(|i| i.to_string()).as_deref())
    }

    /// Забыть медиатеку (выход, другой сервер). Файлы и закреплённое остаются.
    pub fn reset_library(&self) -> StoreResult<()> {
        self.conn.execute_batch(schema::RESET_LIBRARY)?;
        Ok(())
    }
}
