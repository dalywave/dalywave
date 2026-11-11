//! Синхронизация локальной базы с сервером: медиатека (`/api/sync`, курсор и tombstones), закреплённое
//! на устройстве и набор волны (`/api/devices/current/*`), ссылки на файлы для загрузки.
//!
//! База держится под `Mutex` только между запросами: сеть не ждёт с захваченной базой, а экраны читают
//! её параллельно.

use std::sync::Mutex;

use dalywave_protocol::{
    offline::{
        DeviceDownload, DeviceSet, DeviceSetRequest, DownloadKind, FileLinks, FileLinksRequest,
        NewDeviceDownload,
    },
    sync::SyncPage,
};
use dalywave_store::{Applied, Store, StoreError};
use uuid::Uuid;

/// Изменений в странице: больше — меньше запросов на первой синхронизации.
const PAGE: u32 = 1000;
/// Страниц за один проход — страховка от бесконечного цикла при сбое сервера.
const MAX_PAGES: usize = 500;

#[derive(Debug, thiserror::Error)]
pub enum SyncError {
    #[error("сервер недоступен: {0}")]
    Network(String),
    #[error("нужен вход")]
    Unauthorized,
    #[error("сервер ответил {0}")]
    Server(u16),
    #[error("неожиданный ответ сервера: {0}")]
    Decode(String),
    #[error(transparent)]
    Store(#[from] StoreError),
}

impl From<reqwest::Error> for SyncError {
    fn from(e: reqwest::Error) -> Self {
        if e.is_decode() {
            Self::Decode(e.to_string())
        } else {
            Self::Network(e.to_string())
        }
    }
}

pub type SyncResult<T> = Result<T, SyncError>;

fn lock(store: &Mutex<Store>) -> std::sync::MutexGuard<'_, Store> {
    store.lock().unwrap_or_else(|e| e.into_inner())
}

/// Клиент сервера с токеном устройства.
#[derive(Clone)]
pub struct Client {
    http: reqwest::Client,
    server: String,
    token: String,
}

/// Итог прохода синхронизации.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Pulled {
    pub pages: usize,
    pub changes: Applied,
}

impl Client {
    pub fn new(http: reqwest::Client, server: &str, token: &str) -> Self {
        Self {
            http,
            server: server.trim_end_matches('/').to_string(),
            token: token.to_string(),
        }
    }

    fn url(&self, path: &str) -> String {
        format!("{}{path}", self.server)
    }

    async fn check(res: reqwest::Response) -> SyncResult<reqwest::Response> {
        match res.status().as_u16() {
            200..=299 => Ok(res),
            401 => Err(SyncError::Unauthorized),
            s => Err(SyncError::Server(s)),
        }
    }

    /// Все изменения после сохранённого курсора — страница за страницей, каждая применяется сразу.
    pub async fn pull(&self, store: &Mutex<Store>) -> SyncResult<Pulled> {
        let mut out = Pulled::default();
        for _ in 0..MAX_PAGES {
            let cursor = lock(store).cursor()?;
            let mut req = self
                .http
                .get(self.url("/api/sync"))
                .bearer_auth(&self.token)
                .query(&[("limit", PAGE.to_string())]);
            if let Some(c) = &cursor {
                req = req.query(&[("since", c)]);
            }
            let res = Self::check(req.send().await?).await?;
            let page: SyncPage = res.json().await?;
            let a = lock(store).apply(&page)?;
            out.pages += 1;
            out.changes.rows += a.rows;
            out.changes.library |= a.library;
            out.changes.reserve |= a.reserve;
            out.changes.pins |= a.pins;
            if !page.has_more {
                break;
            }
        }
        Ok(out)
    }

    /// Отправить закреплённое и открепленное без сети. Возвращает, сколько изменений дошло.
    pub async fn push_pins(&self, store: &Mutex<Store>) -> SyncResult<usize> {
        let pending = lock(store).pending_pins()?;
        let mut sent = 0;
        for pin in pending {
            let Ok(kind) = pin.kind.parse::<DownloadKind>() else {
                continue;
            };
            match pin.pending.as_deref() {
                Some("add") => {
                    let res = self
                        .http
                        .post(self.url("/api/devices/current/downloads"))
                        .bearer_auth(&self.token)
                        .json(&NewDeviceDownload {
                            kind,
                            ref_id: pin.ref_id,
                        })
                        .send()
                        .await?;
                    // Нет такого альбома или плейлиста на сервере — закреплять нечего
                    if res.status().as_u16() == 404 {
                        let s = lock(store);
                        s.remove_pin(&pin.kind, pin.ref_id)?;
                        s.forget_pin(&pin.kind, pin.ref_id)?;
                        continue;
                    }
                    let d: DeviceDownload = Self::check(res).await?.json().await?;
                    lock(store).confirm_pin(&pin.kind, pin.ref_id, d.id)?;
                }
                Some("remove") => {
                    if let Some(id) = pin.server_id {
                        let res = self
                            .http
                            .delete(self.url(&format!("/api/devices/current/downloads/{id}")))
                            .bearer_auth(&self.token)
                            .send()
                            .await?;
                        Self::check(res).await?;
                    }
                    lock(store).forget_pin(&pin.kind, pin.ref_id)?;
                }
                _ => continue,
            }
            sent += 1;
        }
        Ok(sent)
    }

    /// Пересобрать набор волны устройства под запас: строки придут следующей синхронизацией.
    pub async fn rebuild_set(&self, favorites: u32, fresh: u32) -> SyncResult<DeviceSet> {
        let res = self
            .http
            .post(self.url("/api/devices/current/set"))
            .bearer_auth(&self.token)
            .json(&DeviceSetRequest { favorites, fresh })
            .send()
            .await?;
        Ok(Self::check(res).await?.json().await?)
    }

    /// Ссылки на файлы (до 200 за раз).
    pub async fn files(&self, track_ids: &[Uuid]) -> SyncResult<FileLinks> {
        let res = self
            .http
            .post(self.url("/api/devices/current/files"))
            .bearer_auth(&self.token)
            .json(&FileLinksRequest {
                track_ids: track_ids.to_vec(),
            })
            .send()
            .await?;
        Ok(Self::check(res).await?.json().await?)
    }
}
