//! Загрузчик музыки на этом компьютере (macOS и Windows) — sidecar `dalywave-fetcher`
//! (договор: services/fetcher/SIDECAR.md).
//!
//! Приложение запущено (оно живёт в строке меню или в трее) — загрузчик берёт задания у сервера
//! и качает со своего адреса; приложение закрыто — не качает ничего. Включён по умолчанию и
//! работает, пока вход выполнен. Токен `dwk_…` приложение заводит себе само
//! (`POST /api/admin/downloaders` с токеном устройства — только администратору) и хранит в
//! связке ключей (Windows — «Диспетчер учётных данных»). Состояние уходит интерфейсу событием
//! `downloader:state` и в меню трея.
//!
//! Всем управляет одна задача-надзиратель: решает, должен ли загрузчик работать, запускает его,
//! читает события (stdout — строки JSON, stderr — тот же текстовый лог, что в файле),
//! перезапускает после сбоя с растущей паузой и останавливает: `{"cmd":"stop"}`, до 15 с на
//! возврат задания серверу, затем убивает всю группу процессов (загрузчик PyInstaller, Python,
//! Deno сервиса PO-токенов): на macOS — SIGKILL группе, на Windows — Job Object. Окон консоли
//! на Windows нет: плагин запускает с `CREATE_NO_WINDOW`, дочерние программы наследуют скрытую
//! консоль.

use std::{
    collections::VecDeque,
    ffi::OsString,
    path::PathBuf,
    sync::{Mutex, MutexGuard},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_opener::OpenerExt;
use tauri_plugin_shell::{
    ShellExt,
    process::{CommandChild, CommandEvent},
};
use tokio::sync::{Notify, watch};

use crate::{
    MAIN, auth,
    error::{ShellError, ShellResult},
    settings::Settings,
};

pub const EVENT_STATE: &str = "downloader:state";
/// Имя в `bundle.externalBin` (tauri.macos.conf.json, tauri.windows.conf.json)
const SIDECAR: &str = "dalywave-fetcher";
/// Начало имени загрузчика в админке: «Mac MacBook Pro», «PC DESKTOP-7GH2K1L»
const MACHINE: &str = if cfg!(target_os = "macos") {
    "Mac"
} else if cfg!(windows) {
    "PC"
} else {
    "Linux"
};
/// Сколько ждать загрузчик после `stop`: возврат задания серверу занимает до 10 с (SIDECAR.md)
const STOP_TIMEOUT: Duration = Duration::from_secs(15);
/// Сколько ждать конца процессов после SIGKILL
const KILL_GRACE: Duration = Duration::from_secs(2);
/// Проработал столько — следующая пауза перед перезапуском снова короткая
const STABLE_RUN: Duration = Duration::from_secs(120);
/// Паузы перед перезапуском после сбоя, секунды
const BACKOFF: [u64; 8] = [2, 5, 10, 20, 40, 80, 160, 300];
/// Имя загрузчика на сервере — до 60 символов
const NAME_MAX: usize = 60;
/// Последние строки stderr — в журнал приложения, если загрузчик упал
const STDERR_TAIL: usize = 20;

/// Что сейчас с загрузчиком с точки зрения приложения. Пока процесс работает (`running`),
/// подробности — в последнем `status` самого загрузчика.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Phase {
    /// Выключен в настройках или в трее
    Disabled,
    /// Вход в приложение не выполнен
    SignedOut,
    /// Токен загрузчика заводит только администратор
    Forbidden,
    /// Заводим токен загрузчика на сервере
    Provisioning,
    /// Процесс запущен, своего состояния ещё не прислал
    Starting,
    /// Работает: подробности — в `status`
    Running,
    /// Останавливается: выключили, вышли, сменили сервер, выход из приложения
    Stopping,
    /// Упал или сервер не выдал токен — новая попытка в `retryAtMs`
    Restarting,
    /// Сервер не принял токен (его отозвали) — нужен новый: `downloader_recreate_token`
    TokenRejected,
    /// Программы загрузчика не нашлись или не запускаются
    Broken,
}

impl Phase {
    fn name(self) -> &'static str {
        match self {
            Self::Disabled => "disabled",
            Self::SignedOut => "signed-out",
            Self::Forbidden => "forbidden",
            Self::Provisioning => "provisioning",
            Self::Starting => "starting",
            Self::Running => "running",
            Self::Stopping => "stopping",
            Self::Restarting => "restarting",
            Self::TokenRejected => "token-rejected",
            Self::Broken => "broken",
        }
    }
}

/// Почему загрузчик не работает: `code` — для текста в интерфейсе, `message` — по-русски, для журнала.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Problem {
    pub code: String,
    pub message: String,
}

impl Problem {
    fn new(code: &str, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
        }
    }
}

/// Всё о загрузчике для интерфейса — событие `downloader:state` и ответ команд.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    /// Включён ли («Скачивать на этом Mac»)
    pub enabled: bool,
    /// Пауза, которую оставил пользователь
    pub paused: bool,
    pub phase: Phase,
    /// Последнее событие `status` загрузчика как есть (SIDECAR.md, «Протокол»); только пока он работает
    pub status: Option<Value>,
    pub server: String,
    pub worker_name: String,
    pub log_file: String,
    /// Версия загрузчика и yt-dlp из `hello`
    pub version: Option<String>,
    pub yt_dlp: Option<String>,
    pub error: Option<Problem>,
    /// `restarting`: когда следующая попытка (мс от 1970 года)
    pub retry_at_ms: Option<u64>,
}

pub struct Downloader {
    inner: Mutex<Inner>,
    /// Будит надзирателя: включили или выключили, вход, выход, другой сервер, новый токен
    wake: Notify,
    /// Надзиратель закончил работу — выход из приложения
    done: watch::Sender<bool>,
    http: reqwest::Client,
    worker_name: String,
    data_dir: PathBuf,
    log_file: PathBuf,
}

struct Inner {
    phase: Phase,
    status: Option<Value>,
    version: Option<String>,
    yt_dlp: Option<String>,
    error: Option<Problem>,
    retry_at_ms: Option<u64>,
    /// С каким сервером работает (или будет работать) загрузчик
    server: String,
    child: Option<CommandChild>,
    quitting: bool,
    /// Пользователь попросил новый токен
    recreate: bool,
    /// Что уже записано в журнал: состояние загрузчика и сервиса PO-токенов
    logged_state: Option<String>,
    logged_pot: Option<String>,
}

impl Downloader {
    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn snapshot(&self, settings: &Settings) -> Snapshot {
        let inner = self.lock();
        Snapshot {
            enabled: settings.downloader_enabled(),
            paused: settings.downloader_paused(),
            phase: inner.phase,
            status: inner.status.clone(),
            server: inner.server.clone(),
            worker_name: self.worker_name.clone(),
            log_file: self.log_file.display().to_string(),
            version: inner.version.clone(),
            yt_dlp: inner.yt_dlp.clone(),
            error: inner.error.clone(),
            retry_at_ms: inner.retry_at_ms,
        }
    }

    fn quitting(&self) -> bool {
        self.lock().quitting
    }

    /// Команда загрузчику — строка JSON в stdin. `false` — процесса нет.
    fn send(&self, cmd: &Value) -> bool {
        let mut inner = self.lock();
        let Some(child) = inner.child.as_mut() else {
            return false;
        };
        let line = format!("{cmd}\n");
        match child.write(line.as_bytes()) {
            Ok(()) => true,
            Err(e) => {
                log::warn!("downloader: could not send {cmd}: {e}");
                false
            }
        }
    }
}

/// Папка данных загрузчика (кэш yt-dlp и Deno, файлы заданий, счётчики) и его журнал:
/// macOS — `~/Library/Application Support/DalyWave/fetcher` и `~/Library/Logs/DalyWave/fetcher.log`;
/// Windows — `%LOCALAPPDATA%\DalyWave\fetcher` и `%LOCALAPPDATA%\DalyWave\logs\fetcher.log`.
fn dirs(app: &AppHandle) -> (PathBuf, PathBuf) {
    #[cfg(target_os = "macos")]
    {
        let home = app
            .path()
            .home_dir()
            .unwrap_or_else(|_| std::env::temp_dir());
        (
            home.join("Library/Application Support/DalyWave/fetcher"),
            home.join("Library/Logs/DalyWave/fetcher.log"),
        )
    }
    #[cfg(not(target_os = "macos"))]
    {
        let base = app
            .path()
            .local_data_dir()
            .unwrap_or_else(|_| std::env::temp_dir())
            .join("DalyWave");
        (base.join("fetcher"), base.join("logs").join("fetcher.log"))
    }
}

/// Завести загрузчик и его надзирателя — при запуске приложения на компьютере.
pub fn init(app: &AppHandle) {
    let settings = app.state::<Settings>();
    let (data_dir, log_file) = dirs(app);
    let http = reqwest::Client::builder()
        .user_agent(format!(
            "DalyWave/{} ({})",
            app.package_info().version,
            crate::platform::OS
        ))
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(20))
        .build()
        .unwrap_or_default();
    let phase = if settings.downloader_enabled() {
        Phase::Starting
    } else {
        Phase::Disabled
    };
    app.manage(Downloader {
        inner: Mutex::new(Inner {
            phase,
            status: None,
            version: None,
            yt_dlp: None,
            error: None,
            retry_at_ms: None,
            server: settings.server_url(),
            child: None,
            quitting: false,
            recreate: false,
            logged_state: None,
            logged_pot: None,
        }),
        wake: Notify::new(),
        done: watch::channel(false).0,
        http,
        worker_name: worker_name(&auth::device_name()),
        data_dir,
        log_file,
    });
    tauri::async_runtime::spawn(supervise(app.clone()));
}

/// «Mac <имя компьютера>», «PC <имя компьютера>» — так загрузчик приложения не путается в
/// админке с загрузчиком из launchd.
fn worker_name(computer: &str) -> String {
    let name = match computer.trim() {
        // Имя компьютера не узнали — `auth::device_name` вернул запасное
        "" | "Mac" | "Windows" | "Linux" => MACHINE.to_string(),
        computer => format!("{MACHINE} {computer}"),
    };
    name.chars()
        .take(NAME_MAX)
        .collect::<String>()
        .trim_end()
        .to_string()
}

/// Что-то поменялось (вход, выход, адрес сервера) — надзиратель решит заново.
pub fn changed(app: &AppHandle) {
    if let Some(dl) = app.try_state::<Downloader>() {
        dl.wake.notify_one();
    }
}

pub fn snapshot(app: &AppHandle) -> Option<Snapshot> {
    let dl = app.try_state::<Downloader>()?;
    Some(dl.snapshot(&app.state::<Settings>()))
}

/// Состояние — интерфейсу и в меню трея.
fn publish(app: &AppHandle) {
    let Some(snapshot) = snapshot(app) else {
        return;
    };
    let _ = app.emit_to(MAIN, EVENT_STATE, &snapshot);
    // При выходе главный поток ждёт остановки загрузчика: меню трея не трогаем
    if app.state::<Downloader>().quitting() {
        return;
    }
    let handle = app.clone();
    let _ = app.run_on_main_thread(move || crate::tray::refresh_downloader(&handle));
}

fn set_phase(app: &AppHandle, phase: Phase, error: Option<Problem>, retry_at_ms: Option<u64>) {
    let dl = app.state::<Downloader>();
    {
        let mut inner = dl.lock();
        if inner.phase == phase && inner.error == error && inner.retry_at_ms == retry_at_ms {
            return;
        }
        if inner.phase != phase {
            log::info!("downloader: {} → {}", inner.phase.name(), phase.name());
        }
        inner.phase = phase;
        inner.error = error;
        inner.retry_at_ms = retry_at_ms;
        if !matches!(phase, Phase::Running | Phase::Stopping) {
            inner.status = None;
            inner.logged_state = None;
            inner.logged_pot = None;
        }
    }
    publish(app);
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
}

/// Связка ключей может спросить разрешение — не на главном потоке и не в потоках tokio.
async fn blocking<T: Send + 'static>(
    f: impl FnOnce() -> ShellResult<T> + Send + 'static,
) -> ShellResult<T> {
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(ShellError::internal)?
}

/// Токен загрузчика `dwk_…` в связке ключей: отдельная запись на каждый сервер.
mod token_store {
    use crate::{auth::KEYCHAIN_SERVICE, error::ShellResult};

    fn entry(server: &str) -> ShellResult<keyring_core::Entry> {
        Ok(keyring::Entry::new(KEYCHAIN_SERVICE, &format!("downloader-token {server}"))?.inner)
    }

    pub fn load(server: &str) -> ShellResult<Option<String>> {
        match entry(server)?.get_password() {
            Ok(token) if !token.is_empty() => Ok(Some(token)),
            Ok(_) | Err(keyring_core::Error::NoEntry) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    pub fn store(server: &str, token: &str) -> ShellResult<()> {
        Ok(entry(server)?.set_password(token)?)
    }

    pub fn delete(server: &str) -> ShellResult<()> {
        match entry(server)?.delete_credential() {
            Ok(()) | Err(keyring_core::Error::NoEntry) => Ok(()),
            Err(e) => Err(e.into()),
        }
    }
}

// ---- надзиратель ----

enum Prepared {
    /// Работать не нужно или нельзя — ждать перемен
    Wait(Phase),
    /// Не вышло (сеть, связка ключей) — попробовать позже
    Retry(Problem),
    Run {
        server: String,
        token: String,
    },
}

enum End {
    /// Остановили мы: выключили, вышли, выход из приложения
    Stopped,
    Failed(Problem),
    TokenRejected(Problem),
    Broken(Problem),
}

async fn supervise(app: AppHandle) {
    let dl = app.state::<Downloader>();
    let mut failures = 0usize;
    loop {
        if dl.quitting() {
            break;
        }
        match prepare(&app).await {
            Prepared::Wait(phase) => {
                failures = 0;
                set_phase(&app, phase, None, None);
                dl.wake.notified().await;
            }
            Prepared::Retry(problem) => backoff(&app, &mut failures, problem).await,
            Prepared::Run { server, token } => {
                let started = Instant::now();
                let end = run(&app, &server, &token).await;
                if started.elapsed() >= STABLE_RUN {
                    failures = 0;
                }
                match end {
                    End::Stopped => {}
                    End::Failed(problem) => {
                        if dl.quitting() {
                            break;
                        }
                        backoff(&app, &mut failures, problem).await;
                    }
                    End::TokenRejected(problem) => {
                        log::warn!("downloader: {}", problem.message);
                        set_phase(&app, Phase::TokenRejected, Some(problem), None);
                        dl.wake.notified().await;
                    }
                    End::Broken(problem) => {
                        log::error!("downloader: {}", problem.message);
                        set_phase(&app, Phase::Broken, Some(problem), None);
                        dl.wake.notified().await;
                    }
                }
            }
        }
    }
    log::info!("downloader: supervisor finished");
    dl.done.send_replace(true);
}

/// Пауза перед новой попыткой; перемены (выключили, вышли) её прерывают.
async fn backoff(app: &AppHandle, failures: &mut usize, problem: Problem) {
    let delay = Duration::from_secs(BACKOFF[(*failures).min(BACKOFF.len() - 1)]);
    *failures += 1;
    log::warn!(
        "downloader: {} — next attempt in {} s",
        problem.message,
        delay.as_secs()
    );
    let retry_at = now_ms() + u64::try_from(delay.as_millis()).unwrap_or(0);
    set_phase(app, Phase::Restarting, Some(problem), Some(retry_at));
    let dl = app.state::<Downloader>();
    tokio::select! {
        () = tokio::time::sleep(delay) => {}
        () = dl.wake.notified() => {}
    }
}

/// Должен ли загрузчик работать, и с каким токеном.
async fn prepare(app: &AppHandle) -> Prepared {
    let dl = app.state::<Downloader>();
    let settings = app.state::<Settings>();
    let server = settings.server_url();
    dl.lock().server.clone_from(&server);
    if !settings.downloader_enabled() {
        return Prepared::Wait(Phase::Disabled);
    }
    let key = server.clone();
    let device = match blocking(move || auth::load_token(&key)).await {
        Ok(Some(token)) => token,
        Ok(None) => return Prepared::Wait(Phase::SignedOut),
        Err(e) => return Prepared::Retry(Problem::new("keychain", e.to_string())),
    };
    let recreate = std::mem::take(&mut dl.lock().recreate);
    let key = server.clone();
    let stored = if recreate {
        log::info!("downloader: forgetting the rejected token");
        blocking(move || token_store::delete(&key).map(|()| None)).await
    } else {
        blocking(move || token_store::load(&key)).await
    };
    match stored {
        Ok(Some(token)) => return Prepared::Run { server, token },
        Ok(None) => {}
        Err(e) => return Prepared::Retry(Problem::new("keychain", e.to_string())),
    }

    set_phase(app, Phase::Provisioning, None, None);
    match create_token(&dl.http, &server, &device, &dl.worker_name).await {
        Ok(token) => {
            let (key, value) = (server.clone(), token.clone());
            if let Err(e) = blocking(move || token_store::store(&key, &value)).await {
                return Prepared::Retry(Problem::new("keychain", e.to_string()));
            }
            log::info!(
                "downloader: token for «{}» created on {server} and stored in the keychain",
                dl.worker_name
            );
            Prepared::Run { server, token }
        }
        Err(Provision::Forbidden) => {
            log::info!("downloader: only admins can create a downloader token");
            Prepared::Wait(Phase::Forbidden)
        }
        Err(Provision::SignedOut) => Prepared::Wait(Phase::SignedOut),
        Err(Provision::Failed(problem)) => Prepared::Retry(problem),
    }
}

enum Provision {
    Forbidden,
    SignedOut,
    Failed(Problem),
}

#[derive(Deserialize)]
struct Created {
    token: String,
}

/// Новый загрузчик на сервере: токен показывается один раз, на сервере остаётся хеш.
async fn create_token(
    http: &reqwest::Client,
    server: &str,
    device_token: &str,
    name: &str,
) -> Result<String, Provision> {
    let res = http
        .post(format!("{server}/api/admin/downloaders"))
        .bearer_auth(device_token)
        .json(&json!({ "name": name }))
        .send()
        .await
        .map_err(|e| {
            Provision::Failed(Problem::new(
                "server_unreachable",
                format!("сервер недоступен: {e}"),
            ))
        })?;
    let status = res.status().as_u16();
    match status {
        200 | 201 => {
            let body: Created = res.json().await.map_err(|e| {
                Provision::Failed(Problem::new(
                    "server_error",
                    format!("неожиданный ответ сервера: {e}"),
                ))
            })?;
            if body.token.starts_with("dwk_") {
                Ok(body.token)
            } else {
                Err(Provision::Failed(Problem::new(
                    "server_error",
                    "сервер выдал токен неизвестного вида",
                )))
            }
        }
        401 => Err(Provision::SignedOut),
        403 => Err(Provision::Forbidden),
        _ => {
            let message = res
                .json::<Value>()
                .await
                .ok()
                .and_then(|v| v["error"]["message"].as_str().map(String::from))
                .unwrap_or_else(|| format!("сервер ответил {status}"));
            Err(Provision::Failed(Problem::new("server_error", message)))
        }
    }
}

/// Почему пора остановить работающий загрузчик; `None` — пусть работает.
async fn stop_reason(app: &AppHandle, server: &str) -> Option<&'static str> {
    let dl = app.state::<Downloader>();
    {
        let inner = dl.lock();
        if inner.quitting {
            return Some("the app is quitting");
        }
        if inner.recreate {
            return Some("a new token was requested");
        }
    }
    let settings = app.state::<Settings>();
    if !settings.downloader_enabled() {
        return Some("turned off");
    }
    if settings.server_url() != server {
        return Some("the server changed");
    }
    let key = server.to_string();
    if matches!(blocking(move || auth::load_token(&key)).await, Ok(None)) {
        return Some("signed out");
    }
    None
}

/// Windows: canvas.node сервиса PO-токенов тянет cairo и pango из своей папки, а Deno ищет
/// зависимые DLL не рядом с модулем, а по PATH — ставим её первой. Окружение загрузчика
/// наследуют Python и Deno.
#[cfg(windows)]
fn canvas_dll_path(pot_dir: &std::path::Path) -> Option<OsString> {
    let release = pot_dir.join("node_modules/canvas/build/Release");
    let current = std::env::var_os("PATH").unwrap_or_default();
    let paths = std::iter::once(release).chain(std::env::split_paths(&current));
    std::env::join_paths(paths).ok()
}

/// Последняя строка `exit` загрузчика.
struct ExitInfo {
    reason: String,
    message: Option<String>,
}

/// Один запуск: от старта процесса до его конца.
async fn run(app: &AppHandle, server: &str, token: &str) -> End {
    let dl = app.state::<Downloader>();
    let paused = app.state::<Settings>().downloader_paused();
    let pot_dir = match app.path().resource_dir() {
        Ok(dir) => dir.join("bgutil-server"),
        Err(e) => return End::Broken(Problem::new("not_installed", e.to_string())),
    };
    for dir in [Some(dl.data_dir.as_path()), dl.log_file.parent()]
        .into_iter()
        .flatten()
    {
        if let Err(e) = std::fs::create_dir_all(dir) {
            log::warn!("downloader: could not create {}: {e}", dir.display());
        }
    }
    let mut args: Vec<OsString> = vec![
        "sidecar".into(),
        "--server".into(),
        server.into(),
        "--worker-name".into(),
        dl.worker_name.clone().into(),
        "--data-dir".into(),
        dl.data_dir.clone().into(),
        "--log-file".into(),
        dl.log_file.clone().into(),
        "--pot-server-dir".into(),
        pot_dir.clone().into(),
    ];
    if paused {
        args.push("--paused".into());
    }
    #[cfg(windows)]
    let dll_path = canvas_dll_path(&pot_dir);
    let spawned = app.shell().sidecar(SIDECAR).and_then(|command| {
        let command = command
            .args(args)
            // Только переменной окружения: аргументы видны в `ps`
            .env("DALYWAVE_DOWNLOADER_TOKEN", token)
            // Своя группа процессов (Windows — Job Object): при остановке силой достаётся и
            // Python внутри PyInstaller, и Deno
            .set_process_group(true);
        #[cfg(windows)]
        let command = match dll_path {
            Some(path) => command.env("PATH", path),
            None => command,
        };
        command.spawn()
    });
    let (mut rx, child) = match spawned {
        Ok(spawned) => spawned,
        Err(e) => {
            return End::Broken(Problem::new(
                "not_installed",
                format!("загрузчик не запустился: {e}"),
            ));
        }
    };
    log::info!(
        "downloader: started {SIDECAR} (pid {}) for {server} as «{}»{}",
        child.pid(),
        dl.worker_name,
        if paused { ", paused" } else { "" }
    );
    dl.lock().child = Some(child);
    set_phase(app, Phase::Starting, None, None);

    let mut stop_deadline: Option<tokio::time::Instant> = None;
    let mut kill_deadline: Option<tokio::time::Instant> = None;
    let mut exit: Option<ExitInfo> = None;
    let mut tail: VecDeque<String> = VecDeque::with_capacity(STDERR_TAIL);
    let (code, signal) = loop {
        let now = tokio::time::Instant::now();
        tokio::select! {
            event = rx.recv() => match event {
                Some(CommandEvent::Stdout(line)) => {
                    if let Some(info) = on_event(app, &line) {
                        exit = Some(info);
                    }
                }
                Some(CommandEvent::Stderr(line)) => {
                    let text = String::from_utf8_lossy(&line).trim_end().to_string();
                    log::debug!("downloader: {text}");
                    if tail.len() == STDERR_TAIL {
                        tail.pop_front();
                    }
                    tail.push_back(text);
                }
                Some(CommandEvent::Error(e)) => log::warn!("downloader: {e}"),
                Some(CommandEvent::Terminated(t)) => break (t.code, t.signal),
                Some(_) => {}
                None => break (None, None),
            },
            () = dl.wake.notified(), if stop_deadline.is_none() => {
                if let Some(reason) = stop_reason(app, server).await {
                    log::info!("downloader: stopping — {reason}");
                    dl.send(&json!({ "cmd": "stop" }));
                    stop_deadline = Some(tokio::time::Instant::now() + STOP_TIMEOUT);
                    set_phase(app, Phase::Stopping, None, None);
                }
            },
            () = tokio::time::sleep_until(stop_deadline.unwrap_or(now)), if stop_deadline.is_some() && kill_deadline.is_none() => {
                log::warn!("downloader: did not stop in {} s — killing it", STOP_TIMEOUT.as_secs());
                let child = dl.lock().child.take();
                if let Some(child) = child
                    && let Err(e) = child.kill()
                {
                    log::warn!("downloader: kill failed: {e}");
                }
                kill_deadline = Some(tokio::time::Instant::now() + KILL_GRACE);
            },
            // Процесс убит, а трубы держит кто-то вне группы: дальше не ждём
            () = tokio::time::sleep_until(kill_deadline.unwrap_or(now)), if kill_deadline.is_some() => {
                break (None, Some(9));
            },
        }
    };
    let stop_requested = stop_deadline.is_some();
    dl.lock().child = None;
    let reason = exit.as_ref().map_or("—", |e| e.reason.as_str());
    match (code, signal) {
        (Some(code), _) => log::info!("downloader: exited with code {code} ({reason})"),
        (None, Some(signal)) => log::info!("downloader: killed by signal {signal}"),
        (None, None) => log::info!("downloader: output closed"),
    }
    let end = classify(code, signal, stop_requested, exit);
    if matches!(end, End::Failed(_) | End::Broken(_)) && !tail.is_empty() {
        log::warn!(
            "downloader: last lines of its log:\n{}",
            tail.into_iter().collect::<Vec<_>>().join("\n")
        );
    }
    end
}

/// Код выхода → что дальше (SIDECAR.md, «Остановка и коды выхода»).
fn classify(
    code: Option<i32>,
    signal: Option<i32>,
    stop_requested: bool,
    exit: Option<ExitInfo>,
) -> End {
    if stop_requested {
        return End::Stopped;
    }
    let said = exit.is_some();
    let message = exit.and_then(|e| e.message);
    match code {
        // Без строки `exit` код 2 — от разбора аргументов: приложение и загрузчик разных версий
        Some(2) if !said => End::Broken(Problem::new(
            "bad_arguments",
            "загрузчик не принял аргументы запуска — версии приложения и загрузчика не совпадают",
        )),
        Some(2) => End::TokenRejected(Problem::new(
            "no_token",
            message.unwrap_or_else(|| "загрузчик запущен без токена".into()),
        )),
        Some(3) => End::TokenRejected(Problem::new(
            "token_rejected",
            message.unwrap_or_else(|| "сервер не принял токен загрузчика — его отозвали".into()),
        )),
        Some(4) => End::Broken(Problem::new(
            "tools_missing",
            message.unwrap_or_else(|| "программы загрузчика не нашлись или не запускаются".into()),
        )),
        Some(0) => End::Failed(Problem::new("exited", "загрузчик остановился сам")),
        Some(c) => End::Failed(Problem::new(
            "crashed",
            message.unwrap_or_else(|| format!("загрузчик упал (код {c})")),
        )),
        None => End::Failed(Problem::new(
            "crashed",
            format!("загрузчик завершён сигналом {}", signal.unwrap_or(0)),
        )),
    }
}

/// Строка stdout загрузчика: hello, status, job, ack, exit. Неизвестное пропускается.
fn on_event(app: &AppHandle, line: &[u8]) -> Option<ExitInfo> {
    let Ok(event) = serde_json::from_slice::<Value>(line) else {
        log::debug!(
            "downloader: not an event: {}",
            String::from_utf8_lossy(line).trim_end()
        );
        return None;
    };
    let dl = app.state::<Downloader>();
    let text = |v: &Value| v.as_str().unwrap_or("—").to_string();
    match event["event"].as_str() {
        Some("hello") => {
            log::info!(
                "downloader: hello — fetcher {}, yt-dlp {}, protocol {}, pid {}, server {}",
                text(&event["version"]),
                text(&event["ytDlp"]),
                event["protocol"],
                event["pid"],
                text(&event["server"]),
            );
            {
                let mut inner = dl.lock();
                inner.version = event["version"].as_str().map(String::from);
                inner.yt_dlp = event["ytDlp"].as_str().map(String::from);
            }
            publish(app);
        }
        Some("status") => {
            {
                let mut inner = dl.lock();
                let state = text(&event["state"]);
                if inner.logged_state.as_deref() != Some(state.as_str()) {
                    let error = &event["error"];
                    if error.is_object() {
                        log::info!(
                            "downloader: {state} — {} ({}), retry at {}",
                            text(&error["message"]),
                            text(&error["code"]),
                            text(&event["retryAt"]),
                        );
                    } else {
                        log::info!("downloader: {state}");
                    }
                    inner.logged_state = Some(state);
                }
                let pot = &event["pot"];
                let pot_state = text(&pot["state"]);
                if inner.logged_pot.as_deref() != Some(pot_state.as_str()) {
                    match pot["message"].as_str() {
                        Some(message) => {
                            log::info!("downloader: PO tokens {pot_state} — {message}")
                        }
                        None => log::info!(
                            "downloader: PO tokens {pot_state}{}",
                            pot["version"]
                                .as_str()
                                .map(|v| format!(" {v}"))
                                .unwrap_or_default()
                        ),
                    }
                    inner.logged_pot = Some(pot_state);
                }
                if inner.phase != Phase::Stopping {
                    inner.phase = Phase::Running;
                    inner.error = None;
                    inner.retry_at_ms = None;
                }
                inner.status = Some(event);
            }
            publish(app);
        }
        Some("job") => {
            let elapsed = event["elapsedMs"].as_f64().unwrap_or(0.0) / 1000.0;
            log::info!(
                "downloader: job «{}» — {}: {} ({}) in {elapsed:.1} s{}",
                text(&event["track"]),
                text(&event["artist"]),
                text(&event["outcome"]),
                text(&event["status"]),
                event["error"]["message"]
                    .as_str()
                    .map(|m| format!(" — {m}"))
                    .unwrap_or_default(),
            );
        }
        Some("ack") => {
            if event["ok"].as_bool() == Some(false) {
                log::warn!(
                    "downloader: command {} refused: {}",
                    text(&event["cmd"]),
                    text(&event["message"])
                );
            } else {
                log::debug!("downloader: {} acknowledged", text(&event["cmd"]));
            }
        }
        Some("exit") => {
            return Some(ExitInfo {
                reason: text(&event["reason"]),
                message: event["message"].as_str().map(String::from),
            });
        }
        _ => {}
    }
    None
}

// ---- выход из приложения ----

/// Выход из приложения (⌘Q, «Выйти» в трее, перезапуск после обновления): загрузчик возвращает
/// текущее задание серверу и останавливается — до 15 с, дальше SIGKILL всей группе. Вызывается
/// на главном потоке из `RunEvent::Exit` и ждёт; окно к этому времени спрятано.
pub fn shutdown(app: &AppHandle) {
    let Some(dl) = app.try_state::<Downloader>() else {
        return;
    };
    let running = {
        let mut inner = dl.lock();
        inner.quitting = true;
        inner.child.is_some()
    };
    dl.wake.notify_one();
    if !running {
        return;
    }
    let started = Instant::now();
    let mut done = dl.done.subscribe();
    let finished = tauri::async_runtime::block_on(async {
        tokio::time::timeout(
            STOP_TIMEOUT + KILL_GRACE + Duration::from_secs(1),
            done.wait_for(|d| *d),
        )
        .await
        .is_ok()
    });
    let child = dl.lock().child.take();
    if let Some(child) = child {
        log::warn!("downloader: still running at exit — killing it");
        let _ = child.kill();
    }
    log::info!(
        "downloader: {} for exit in {:.2} s",
        if finished { "stopped" } else { "abandoned" },
        started.elapsed().as_secs_f64()
    );
}

// ---- трей ----

/// Что показать в меню трея.
#[derive(Debug, Clone, PartialEq)]
pub enum Summary {
    Disabled,
    SignedOut,
    Forbidden,
    Starting,
    Idle,
    Working {
        track: Option<String>,
        artist: Option<String>,
        /// Ход всего задания, 0…100
        percent: u32,
        /// Пауза попрошена: доделывает текущий трек
        finishing: bool,
    },
    Paused,
    Offline,
    /// Отдых после бот-проверки YouTube и подобного — продолжит сам
    Resting,
    Stopping,
    Restarting,
    TokenRejected,
    Broken,
}

pub struct TrayState {
    pub summary: Summary,
    pub enabled: bool,
    pub paused: bool,
}

impl TrayState {
    /// Пауза имеет смысл, пока загрузчик включён и не сломан
    pub fn can_pause(&self) -> bool {
        self.enabled
            && !matches!(
                self.summary,
                Summary::Disabled
                    | Summary::SignedOut
                    | Summary::Forbidden
                    | Summary::TokenRejected
                    | Summary::Broken
            )
    }
}

pub fn tray_state(app: &AppHandle) -> Option<TrayState> {
    let s = snapshot(app)?;
    Some(TrayState {
        summary: summarize(s.phase, s.status.as_ref()),
        enabled: s.enabled,
        paused: s.paused,
    })
}

fn summarize(phase: Phase, status: Option<&Value>) -> Summary {
    match phase {
        Phase::Disabled => Summary::Disabled,
        Phase::SignedOut => Summary::SignedOut,
        Phase::Forbidden => Summary::Forbidden,
        Phase::Provisioning | Phase::Starting => Summary::Starting,
        Phase::Stopping => Summary::Stopping,
        Phase::Restarting => Summary::Restarting,
        Phase::TokenRejected => Summary::TokenRejected,
        Phase::Broken => Summary::Broken,
        Phase::Running => {
            let Some(status) = status else {
                return Summary::Starting;
            };
            match status["state"].as_str() {
                Some("idle") => Summary::Idle,
                Some("paused") => Summary::Paused,
                Some("offline") => Summary::Offline,
                Some("stopping") => Summary::Stopping,
                Some("error") => match status["error"]["code"].as_str() {
                    Some("token_rejected" | "no_token") => Summary::TokenRejected,
                    Some("tools_missing") => Summary::Broken,
                    _ => Summary::Resting,
                },
                Some("working") => {
                    let job = &status["job"];
                    Summary::Working {
                        track: job["track"].as_str().map(String::from),
                        artist: job["artist"].as_str().map(String::from),
                        percent: overall_percent(
                            job["stage"].as_str().unwrap_or(""),
                            job["progress"].as_f64().unwrap_or(0.0),
                        ),
                        finishing: status["paused"].as_bool() == Some(true),
                    }
                }
                _ => Summary::Starting,
            }
        }
    }
}

/// Ход всего задания по этапу и доле внутри него: поиск — до 10 %, скачивание — до 65 %,
/// проверка — до 80 %, загрузка в хранилище — до 100 %. Такие же доли — в интерфейсе.
pub fn overall_percent(stage: &str, progress: f64) -> u32 {
    let (from, to) = match stage {
        "matching" => (0.0, 0.10),
        "downloading" => (0.10, 0.65),
        "verifying" => (0.65, 0.80),
        "uploading" => (0.80, 1.0),
        _ => (0.0, 0.0),
    };
    let p = if progress.is_finite() {
        progress.clamp(0.0, 1.0)
    } else {
        0.0
    };
    // Доли ограничены 0…100 — приведение без потерь
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let percent = ((from + (to - from) * p) * 100.0).floor() as u32;
    percent.min(100)
}

// ---- команды интерфейса ----

fn current(app: &AppHandle) -> ShellResult<Snapshot> {
    snapshot(app).ok_or_else(|| ShellError::Internal("загрузчик не запущен".into()))
}

/// Включить или выключить загрузчик на этом Mac (запоминается).
pub fn set_enabled(app: &AppHandle, enabled: bool) -> ShellResult<()> {
    let settings = app.state::<Settings>();
    if settings.downloader_enabled() != enabled {
        settings.set_downloader_enabled(enabled)?;
        log::info!("downloader: turned {}", if enabled { "on" } else { "off" });
        changed(app);
    }
    publish(app);
    Ok(())
}

/// Пауза: текущий трек загрузчик доделывает, новых заданий не берёт. Запоминается — после
/// перезапуска приложения загрузчик стартует на паузе.
pub fn set_paused(app: &AppHandle, paused: bool) -> ShellResult<()> {
    app.state::<Settings>().set_downloader_paused(paused)?;
    let sent = app
        .state::<Downloader>()
        .send(&json!({ "cmd": if paused { "pause" } else { "resume" } }));
    log::info!(
        "downloader: {}{}",
        if paused { "pause" } else { "resume" },
        if sent {
            ""
        } else {
            " (applies at the next start)"
        }
    );
    publish(app);
    Ok(())
}

/// Трей: «Пауза» / «Продолжить».
pub fn toggle_paused(app: &AppHandle) {
    let paused = !app.state::<Settings>().downloader_paused();
    if let Err(e) = set_paused(app, paused) {
        log::warn!("downloader: {e}");
    }
}

/// Трей: «Скачивать на этом Mac».
pub fn toggle_enabled(app: &AppHandle) {
    let enabled = !app.state::<Settings>().downloader_enabled();
    if let Err(e) = set_enabled(app, enabled) {
        log::warn!("downloader: {e}");
    }
}

#[tauri::command]
pub async fn downloader_state(app: AppHandle) -> ShellResult<Snapshot> {
    current(&app)
}

#[tauri::command]
pub async fn downloader_set_enabled(app: AppHandle, enabled: bool) -> ShellResult<Snapshot> {
    set_enabled(&app, enabled)?;
    current(&app)
}

#[tauri::command]
pub async fn downloader_set_paused(app: AppHandle, paused: bool) -> ShellResult<Snapshot> {
    set_paused(&app, paused)?;
    current(&app)
}

/// Сервер не принял токен (его отозвали в админке): забыть его и завести новый.
#[tauri::command]
pub async fn downloader_recreate_token(app: AppHandle) -> ShellResult<Snapshot> {
    {
        let dl = app.state::<Downloader>();
        let mut inner = dl.lock();
        if inner.phase != Phase::TokenRejected {
            return Err(ShellError::Invalid(
                "токен загрузчика действует — новый не нужен".into(),
            ));
        }
        inner.recreate = true;
    }
    log::info!("downloader: a new token was requested");
    changed(&app);
    current(&app)
}

/// Журнал загрузчика — в «Консоли» (или папка, если журнала ещё нет).
#[tauri::command]
pub async fn downloader_open_log(app: AppHandle) -> ShellResult<()> {
    let log_file = app.state::<Downloader>().log_file.clone();
    let target = if log_file.exists() {
        log_file
    } else {
        let dir = log_file
            .parent()
            .map(PathBuf::from)
            .ok_or_else(|| ShellError::Internal("нет папки журнала".into()))?;
        std::fs::create_dir_all(&dir).map_err(ShellError::internal)?;
        dir
    };
    app.opener()
        .open_path(target.to_string_lossy(), None::<&str>)
        .map_err(ShellError::internal)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn worker_names() {
        assert_eq!(
            worker_name("MacBook Pro von Zakhar"),
            format!("{MACHINE} MacBook Pro von Zakhar")
        );
        assert_eq!(worker_name("Mac"), MACHINE);
        assert_eq!(worker_name("Windows"), MACHINE);
        let long = worker_name(&"Очень длинное имя компьютера ".repeat(4));
        assert_eq!(long.chars().count(), NAME_MAX.min(long.chars().count()));
        assert!(long.chars().count() <= NAME_MAX);
        assert!(!long.ends_with(' '));
    }

    #[test]
    fn overall_progress_grows_through_stages() {
        assert_eq!(overall_percent("matching", 0.0), 0);
        assert_eq!(overall_percent("matching", 1.0), 10);
        assert_eq!(overall_percent("downloading", 0.5), 37);
        assert_eq!(overall_percent("verifying", 0.0), 65);
        assert_eq!(overall_percent("uploading", 1.0), 100);
        assert_eq!(overall_percent("uploading", f64::NAN), 80);
        assert_eq!(overall_percent("downloading", 7.0), 65);
    }

    #[test]
    fn status_summaries() {
        let working = json!({
            "state": "working", "paused": false,
            "job": { "track": "Another World", "artist": "CONTROL Resonant", "stage": "downloading", "progress": 0.42 }
        });
        assert_eq!(
            summarize(Phase::Running, Some(&working)),
            Summary::Working {
                track: Some("Another World".into()),
                artist: Some("CONTROL Resonant".into()),
                percent: 33,
                finishing: false,
            }
        );
        let resting = json!({ "state": "error", "error": { "code": "bot_check", "message": "…" } });
        assert_eq!(summarize(Phase::Running, Some(&resting)), Summary::Resting);
        let rejected = json!({ "state": "error", "error": { "code": "token_rejected" } });
        assert_eq!(
            summarize(Phase::Running, Some(&rejected)),
            Summary::TokenRejected
        );
        assert_eq!(summarize(Phase::Running, None), Summary::Starting);
        assert_eq!(summarize(Phase::Provisioning, None), Summary::Starting);
        assert_eq!(
            summarize(Phase::Disabled, Some(&working)),
            Summary::Disabled
        );
    }

    #[test]
    fn exit_codes() {
        let said = |reason: &str| {
            Some(ExitInfo {
                reason: reason.into(),
                message: None,
            })
        };
        assert!(matches!(
            classify(Some(0), None, true, said("stop")),
            End::Stopped
        ));
        assert!(matches!(classify(None, Some(9), true, None), End::Stopped));
        assert!(matches!(
            classify(Some(3), None, false, said("token_rejected")),
            End::TokenRejected(p) if p.code == "token_rejected"
        ));
        assert!(matches!(
            classify(Some(2), None, false, said("no_token")),
            End::TokenRejected(p) if p.code == "no_token"
        ));
        assert!(matches!(
            classify(Some(2), None, false, None),
            End::Broken(p) if p.code == "bad_arguments"
        ));
        assert!(matches!(
            classify(Some(4), None, false, said("tools_missing")),
            End::Broken(_)
        ));
        assert!(matches!(
            classify(Some(1), None, false, said("crashed")),
            End::Failed(_)
        ));
        assert!(matches!(
            classify(None, Some(9), false, None),
            End::Failed(_)
        ));
    }

    #[test]
    fn snapshot_json_is_camel_case() {
        let s = Snapshot {
            enabled: true,
            paused: false,
            phase: Phase::TokenRejected,
            status: None,
            server: "https://wave.daly.rest".into(),
            worker_name: "Mac MacBook".into(),
            log_file: "/tmp/fetcher.log".into(),
            version: Some("0.2.0".into()),
            yt_dlp: None,
            error: Some(Problem::new("token_rejected", "…")),
            retry_at_ms: None,
        };
        let v = serde_json::to_value(&s).unwrap();
        assert_eq!(v["phase"], "token-rejected");
        assert_eq!(v["workerName"], "Mac MacBook");
        assert_eq!(v["retryAtMs"], Value::Null);
        assert_eq!(v["error"]["code"], "token_rejected");
    }
}
