//! Автообновление: сервер отвечает на
//! `/api/updates/{target}/{arch}/{current_version}` (204 — обновлений нет), архив проверяется
//! подписью из `plugins.updater.pubkey`. Проверка — при запуске и раз в 6 часов; ставит
//! обновление интерфейс, когда пользователь согласится.

use std::{sync::Mutex, time::Duration};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_updater::{Update, UpdaterExt};

use crate::{
    MAIN,
    error::{ShellError, ShellResult},
};

pub const EVENT_AVAILABLE: &str = "update:available";
pub const EVENT_PROGRESS: &str = "update:progress";
const FIRST_CHECK: Duration = Duration::from_secs(20);
const CHECK_EVERY: Duration = Duration::from_secs(6 * 60 * 60);

#[derive(Default)]
pub struct Updates {
    found: Mutex<Option<Update>>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub version: String,
    pub current_version: String,
    /// Список изменений из ответа сервера
    pub notes: Option<String>,
    /// RFC 3339
    pub date: Option<String>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Progress {
    downloaded_bytes: u64,
    total_bytes: Option<u64>,
}

fn info(update: &Update) -> UpdateInfo {
    UpdateInfo {
        version: update.version.clone(),
        current_version: update.current_version.clone(),
        notes: update.body.clone(),
        date: update.date.map(|d| d.to_string()),
    }
}

async fn check(app: &AppHandle) -> ShellResult<Option<UpdateInfo>> {
    let updater = app
        .updater()
        .map_err(|e| ShellError::Update(e.to_string()))?;
    let found = updater
        .check()
        .await
        .map_err(|e| ShellError::Update(e.to_string()))?;
    let result = found.as_ref().map(info);
    *app.state::<Updates>()
        .found
        .lock()
        .unwrap_or_else(|e| e.into_inner()) = found;
    Ok(result)
}

/// Фоновая проверка в собранном приложении (в `cargo tauri dev` не нужна).
pub fn schedule(app: &AppHandle) {
    if tauri::is_dev() {
        return;
    }
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(FIRST_CHECK).await;
        loop {
            match check(&app).await {
                Ok(Some(update)) => {
                    log::info!("update {} is available", update.version);
                    let _ = app.emit_to(MAIN, EVENT_AVAILABLE, update);
                }
                Ok(None) => log::debug!("no updates"),
                Err(e) => log::info!("update check failed: {e}"),
            }
            tokio::time::sleep(CHECK_EVERY).await;
        }
    });
}

/// Проверить сейчас. `null` — версия актуальна.
#[tauri::command]
pub async fn update_check(app: AppHandle) -> ShellResult<Option<UpdateInfo>> {
    check(&app).await
}

/// Скачать и поставить найденное обновление, затем перезапустить приложение.
/// Прогресс приходит событием `update:progress`.
#[tauri::command]
pub async fn update_install(app: AppHandle) -> ShellResult<()> {
    let update = app
        .state::<Updates>()
        .found
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .take();
    let Some(update) = update else {
        return Err(ShellError::Update(
            "обновление не найдено — сначала проверьте наличие".into(),
        ));
    };
    let mut downloaded = 0u64;
    let progress = app.clone();
    update
        .download_and_install(
            move |chunk, total| {
                downloaded += chunk as u64;
                let _ = progress.emit_to(
                    MAIN,
                    EVENT_PROGRESS,
                    Progress {
                        downloaded_bytes: downloaded,
                        total_bytes: total,
                    },
                );
            },
            || log::info!("update downloaded"),
        )
        .await
        .map_err(|e| ShellError::Update(e.to_string()))?;
    log::info!("update installed, restarting");
    app.restart()
}
