//! Автообновление: сервер отвечает на `/api/updates/{target}/{arch}/{current_version}`
//! (204 — обновлений нет), архив проверяется подписью из `plugins.updater.pubkey`.
//!
//! Проверка — при запуске (через 20 с) и раз в 6 часов. Найденное обновление сразу скачивается в
//! фоне в папку кэша и проверяется подписью, затем — тихое «Обновление готово — Перезапустить» в
//! настройках и в трее. Сами мы не перезапускаемся никогда: музыка не должна оборваться. Ставится
//! обновление, когда человек нажмёт «Перезапустить», а на macOS — ещё и при обычном выходе (замена
//! `.app` атомарна, при следующем запуске уже новая версия). На Windows при выходе — нет: установщик
//! NSIS показывает окно и запускает приложение заново, а выход при выключении ПК оборвал бы его.

use std::{
    path::{Path, PathBuf},
    sync::Mutex,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_updater::{Update, UpdaterExt};

use crate::{
    MAIN,
    error::{ShellError, ShellResult},
    tray,
};

pub const EVENT_AVAILABLE: &str = "update:available";
pub const EVENT_PROGRESS: &str = "update:progress";
pub const EVENT_READY: &str = "update:ready";
pub const EVENT_STATE: &str = "update:state";
const FIRST_CHECK: Duration = Duration::from_secs(20);
const CHECK_EVERY: Duration = Duration::from_secs(6 * 60 * 60);
/// Ход скачивания интерфейсу — не чаще 4 раз в секунду
const PROGRESS_EVERY: Duration = Duration::from_millis(250);
/// Установка при выходе ждёт не дольше: распаковка `.app` — секунды, дольше — что-то не так
#[cfg(target_os = "macos")]
const EXIT_INSTALL_WAIT: Duration = Duration::from_secs(30);
const CACHE_DIR: &str = "updates";

/// - `idle` — обновлений нет (или ещё не проверяли); `checking` — спрашиваем сервер;
/// - `downloading` — скачиваем найденное, ход — `downloadedBytes`/`totalBytes`;
/// - `ready` — скачано и проверено: «Перезапустить»; `installing` — ставим;
/// - `failed` — проверка или скачивание не удались, текст — `error`; следующая попытка — по расписанию.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Status {
    #[default]
    Idle,
    Checking,
    Downloading,
    Ready,
    Installing,
    Failed,
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

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateState {
    pub status: Status,
    pub current_version: String,
    /// Найденная версия: при `downloading`, `ready`, `installing` и сбое скачивания
    pub update: Option<UpdateInfo>,
    pub downloaded_bytes: Option<u64>,
    pub total_bytes: Option<u64>,
    /// Последняя удачная проверка, мс от эпохи Unix
    pub checked_at_ms: Option<u64>,
    pub error: Option<String>,
    /// Готовое обновление ставится и при обычном выходе (macOS)
    pub install_on_quit: bool,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Progress {
    downloaded_bytes: u64,
    total_bytes: Option<u64>,
}

#[derive(Default)]
struct Inner {
    status: Status,
    found: Option<Update>,
    /// Скачанный и проверенный подписью файл обновления
    file: Option<PathBuf>,
    downloaded: Option<u64>,
    total: Option<u64>,
    checked_at_ms: Option<u64>,
    error: Option<String>,
}

#[derive(Default)]
pub struct Updates {
    inner: Mutex<Inner>,
    /// Одна проверка со скачиванием за раз: расписание и кнопка «Проверить» не качают дважды
    job: tokio::sync::Mutex<()>,
}

impl Updates {
    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }
}

fn info(update: &Update) -> UpdateInfo {
    UpdateInfo {
        version: update.version.clone(),
        current_version: update.current_version.clone(),
        notes: update.body.clone(),
        date: update.date.and_then(|d| {
            d.format(&time::format_description::well_known::Rfc3339)
                .ok()
        }),
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64)
}

fn snapshot(app: &AppHandle) -> UpdateState {
    let current_version = app.package_info().version.to_string();
    let inner = app.state::<Updates>();
    let inner = inner.lock();
    UpdateState {
        status: inner.status,
        current_version,
        update: inner.found.as_ref().map(info),
        downloaded_bytes: inner.downloaded,
        total_bytes: inner.total,
        checked_at_ms: inner.checked_at_ms,
        error: inner.error.clone(),
        install_on_quit: cfg!(target_os = "macos"),
    }
}

/// Готовая к установке версия — для пункта в трее.
pub fn ready_version(app: &AppHandle) -> Option<String> {
    let updates = app.try_state::<Updates>()?;
    let inner = updates.lock();
    (inner.status == Status::Ready)
        .then(|| inner.found.as_ref().map(|u| u.version.clone()))
        .flatten()
}

/// Поменять состояние и сообщить интерфейсу и трею.
fn set(app: &AppHandle, change: impl FnOnce(&mut Inner)) {
    change(&mut app.state::<Updates>().lock());
    let _ = app.emit_to(MAIN, EVENT_STATE, snapshot(app));
    let handle = app.clone();
    // Меню трея меняется на главном потоке
    let _ = app.run_on_main_thread(move || tray::refresh_update(&handle));
}

fn cache_dir(app: &AppHandle) -> ShellResult<PathBuf> {
    Ok(app.path().app_cache_dir()?.join(CACHE_DIR))
}

fn cache_file(dir: &Path, update: &Update) -> PathBuf {
    dir.join(format!(
        "DalyWave-{}-{}.update",
        update.version, update.target
    ))
}

/// Убрать скачанные раньше версии, кроме `keep`.
fn clean_cache(dir: &Path, keep: Option<&Path>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if Some(path.as_path()) != keep {
            let _ = std::fs::remove_file(&path);
        }
    }
}

/// Файл уже скачан (например, до перезапуска приложения) и подпись сходится.
async fn cached(path: PathBuf, update: &Update) -> bool {
    let read = tauri::async_runtime::spawn_blocking(move || std::fs::read(path)).await;
    match read {
        Ok(Ok(bytes)) => update.verify(&bytes).is_ok(),
        _ => false,
    }
}

async fn download(app: &AppHandle, update: &Update, path: PathBuf) -> ShellResult<()> {
    let progress = app.clone();
    let mut downloaded = 0u64;
    let mut last = Instant::now() - PROGRESS_EVERY;
    // `download` проверяет подпись сам: в кэш попадает только проверенный файл
    let bytes = update
        .download(
            move |chunk, total| {
                downloaded += chunk as u64;
                if last.elapsed() < PROGRESS_EVERY && Some(downloaded) != total {
                    return;
                }
                last = Instant::now();
                {
                    let updates = progress.state::<Updates>();
                    let mut inner = updates.lock();
                    inner.downloaded = Some(downloaded);
                    inner.total = total;
                }
                let _ = progress.emit_to(
                    MAIN,
                    EVENT_PROGRESS,
                    Progress {
                        downloaded_bytes: downloaded,
                        total_bytes: total,
                    },
                );
                let _ = progress.emit_to(MAIN, EVENT_STATE, snapshot(&progress));
            },
            || log::info!("update downloaded"),
        )
        .await
        .map_err(|e| ShellError::Update(e.to_string()))?;
    tauri::async_runtime::spawn_blocking(move || {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        // Сначала во временный файл: оборванная запись не выдаст себя за готовое обновление
        let partial = path.with_extension("part");
        std::fs::write(&partial, bytes)?;
        std::fs::rename(&partial, &path)
    })
    .await
    .map_err(ShellError::internal)?
    .map_err(|e| ShellError::Update(format!("не записать обновление на диск: {e}")))
}

/// Проверить и, если есть новая версия, скачать её в фоне. `None` — версия последняя.
async fn check_and_prepare(app: &AppHandle) -> ShellResult<Option<UpdateInfo>> {
    let updates = app.state::<Updates>();
    // Проверка уже идёт: её итог придёт событием
    let Ok(_job) = updates.job.try_lock() else {
        return Ok(snapshot(app).update);
    };
    let was = updates.lock().status;
    if was == Status::Installing {
        return Ok(snapshot(app).update);
    }
    // Готовое обновление не прячем на время проверки
    if was != Status::Ready {
        set(app, |i| {
            i.status = Status::Checking;
            i.error = None;
        });
    }
    let checked = match app.updater() {
        Ok(updater) => updater.check().await,
        Err(e) => Err(e),
    };
    let found = match checked {
        Ok(found) => found,
        Err(e) => {
            let message = e.to_string();
            set(app, |i| {
                if i.status == Status::Checking {
                    i.status = Status::Failed;
                    i.error = Some(message.clone());
                }
            });
            return Err(ShellError::Update(message));
        }
    };
    let dir = cache_dir(app)?;
    let Some(update) = found else {
        // Выпуск могли отозвать: скачанное больше не ставим
        set(app, |i| {
            *i = Inner {
                checked_at_ms: Some(now_ms()),
                ..Inner::default()
            };
        });
        clean_cache(&dir, None);
        return Ok(None);
    };
    let found_info = info(&update);
    let path = cache_file(&dir, &update);
    let same_ready = {
        let inner = updates.lock();
        inner.status == Status::Ready
            && inner
                .found
                .as_ref()
                .is_some_and(|u| u.version == update.version)
            && inner.file.as_deref() == Some(path.as_path())
    };
    if same_ready {
        set(app, |i| i.checked_at_ms = Some(now_ms()));
        return Ok(Some(found_info));
    }

    log::info!("update {} is available", update.version);
    let _ = app.emit_to(MAIN, EVENT_AVAILABLE, found_info.clone());
    set(app, |i| {
        i.status = Status::Downloading;
        i.found = Some(update.clone());
        i.file = None;
        i.downloaded = None;
        i.total = None;
        i.error = None;
        i.checked_at_ms = Some(now_ms());
    });
    let result = if cached(path.clone(), &update).await {
        log::info!("update {} is already downloaded", update.version);
        Ok(())
    } else {
        download(app, &update, path.clone()).await
    };
    if let Err(e) = result {
        log::warn!("update download failed: {e}");
        let message = e.to_string();
        set(app, |i| {
            i.status = Status::Failed;
            i.error = Some(message);
        });
        return Err(e);
    }
    clean_cache(&dir, Some(&path));
    set(app, |i| {
        i.status = Status::Ready;
        i.file = Some(path);
        i.error = None;
    });
    log::info!("update {} is ready to install", update.version);
    let _ = app.emit_to(MAIN, EVENT_READY, found_info.clone());
    Ok(Some(found_info))
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
            match check_and_prepare(&app).await {
                Ok(Some(_)) => {}
                Ok(None) => log::debug!("no updates"),
                Err(e) => log::info!("update check failed: {e}"),
            }
            tokio::time::sleep(CHECK_EVERY).await;
        }
    });
}

/// Поставить готовое обновление и перезапуститься. Только по действию человека.
async fn install(app: &AppHandle) -> ShellResult<()> {
    if app.state::<Updates>().lock().status != Status::Ready {
        // Ещё не скачано (или скачивание сорвалось) — скачать сейчас
        check_and_prepare(app).await?;
    }
    let updates = app.state::<Updates>();
    let taken = {
        let mut inner = updates.lock();
        match (inner.status, inner.found.clone(), inner.file.clone()) {
            (Status::Ready, Some(update), Some(file)) => {
                inner.status = Status::Installing;
                Some((update, file))
            }
            _ => None,
        }
    };
    let Some((update, file)) = taken else {
        return Err(ShellError::Update(
            "обновление ещё не готово — сначала проверьте наличие".into(),
        ));
    };
    set(app, |_| {});
    log::info!("installing update {}", update.version);
    let handle = app.clone();
    let installed = tauri::async_runtime::spawn_blocking(move || {
        let bytes = std::fs::read(&file).map_err(|e| ShellError::Update(e.to_string()))?;
        // Windows: установщик NSIS закрывает приложение сам (`process::exit` внутри плагина) —
        // загрузчик возвращает задание серверу заранее, окно уже не видно
        #[cfg(windows)]
        {
            if let Some(main) = handle.get_webview_window(MAIN) {
                let _ = main.hide();
            }
            crate::downloader::shutdown(&handle);
        }
        #[cfg(not(windows))]
        let _ = &handle;
        let result = update
            .install(bytes)
            .map_err(|e| ShellError::Update(e.to_string()));
        if result.is_ok() {
            let _ = std::fs::remove_file(&file);
        }
        result
    })
    .await
    .map_err(ShellError::internal)
    .and_then(|r| r);
    if let Err(e) = installed {
        log::warn!("update install failed: {e}");
        let message = e.to_string();
        set(app, |i| {
            i.status = Status::Failed;
            i.file = None;
            i.error = Some(message);
        });
        return Err(e);
    }
    log::info!("update installed, restarting");
    app.restart()
}

/// «Перезапустить» в трее.
pub fn restart(app: &AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        if let Err(e) = install(&app).await {
            log::warn!("update from tray failed: {e}");
        }
    });
}

/// macOS: обычный выход (⌘Q, «Выйти» в трее) ставит готовое обновление — при следующем запуске
/// уже новая версия. Вызывается из `RunEvent::Exit` после остановки загрузчика.
#[cfg(target_os = "macos")]
pub fn install_on_exit(app: &AppHandle) {
    let Some(updates) = app.try_state::<Updates>() else {
        return;
    };
    let taken = {
        let mut inner = updates.lock();
        match (inner.status, inner.found.clone(), inner.file.clone()) {
            (Status::Ready, Some(update), Some(file)) => {
                inner.status = Status::Installing;
                Some((update, file))
            }
            _ => None,
        }
    };
    let Some((update, file)) = taken else {
        return;
    };
    // Отдельный поток с пределом ожидания: если замене понадобятся права администратора, плагин
    // спросит их через главный поток, а он здесь занят выходом — не повиснуть
    let (tx, rx) = std::sync::mpsc::channel();
    let version = update.version.clone();
    std::thread::spawn(move || {
        let result = std::fs::read(&file)
            .map_err(|e| e.to_string())
            .and_then(|bytes| update.install(bytes).map_err(|e| e.to_string()));
        if result.is_ok() {
            let _ = std::fs::remove_file(&file);
        }
        let _ = tx.send(result);
    });
    match rx.recv_timeout(EXIT_INSTALL_WAIT) {
        Ok(Ok(())) => log::info!("update {version} installed on exit"),
        Ok(Err(e)) => log::warn!("update {version} not installed on exit: {e}"),
        Err(_) => log::warn!("update {version}: install on exit timed out"),
    }
}

/// Проверить сейчас (и скачать найденное в фоне). `null` — версия актуальна.
#[tauri::command]
pub async fn update_check(app: AppHandle) -> ShellResult<Option<UpdateInfo>> {
    check_and_prepare(&app).await
}

/// Что с обновлением сейчас: для настроек «О приложении».
#[tauri::command]
pub fn update_state(app: AppHandle) -> UpdateState {
    snapshot(&app)
}

/// «Перезапустить»: поставить скачанное обновление и перезапустить приложение. Если оно ещё не
/// скачано — сначала скачать (ход — `update:progress`).
#[tauri::command]
pub async fn update_install(app: AppHandle) -> ShellResult<()> {
    install(&app).await
}
