//! Обучение сети вкуса на этом компьютере (macOS и Windows): приложение берёт у сервера задание
//! (`POST /api/taste/training/claim`), учит сеть тем же `dalywave-taste`, что и сервер, и отправляет
//! веса. Сервер сам проверяет их на своих отложенных данных и включает, только если сеть точнее
//! (crates/server/src/reco/training.rs).
//!
//! «Помогать учить рекомендации на этом компьютере»: не задано — участвуют только компьютеры
//! администраторов (сервер решает по роли, задание — общая сеть); включено — ещё и своя сеть
//! слушателя; выключено — ничего. Учит, только когда компьютер на питании от сети и не занят,
//! в потоке с низким приоритетом. Состояние — интерфейсу событием `trainer:state`.

use std::{
    sync::Mutex,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use dalywave_taste::{TrainReport, wire};
use serde::Serialize;
use serde_json::{Value, json};
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::Notify;

use crate::{
    MAIN, auth,
    error::{ShellError, ShellResult},
    settings::Settings,
};

pub const EVENT_STATE: &str = "trainer:state";
/// Первая попытка — не сразу после запуска: приложению есть чем заняться.
const FIRST_DELAY: Duration = Duration::from_secs(5 * 60);
/// Учить нечего — следующий вопрос серверу через столько.
const IDLE_DELAY: Duration = Duration::from_secs(60 * 60);
/// Компьютер на батарее или занят — проверить снова через столько.
const BUSY_DELAY: Duration = Duration::from_secs(10 * 60);
/// Загрузка процессора выше этой доли ядер — компьютер занят.
const BUSY_LOAD: f64 = 0.35;

/// Чем занят обучатель.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Phase {
    /// Выключено в настройках (или не задано, а компьютер не администратора)
    Off,
    /// Ждёт своего часа: следующая проверка по расписанию
    Idle,
    /// Компьютер на батарее
    OnBattery,
    /// Компьютер занят
    Busy,
    /// Вход не выполнен
    SignedOut,
    /// Получает набор
    Fetching,
    /// Учит
    Training,
    /// Отправляет веса и ждёт решения сервера
    Sending,
}

/// Последний раз: чем кончилось.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LastRun {
    pub at_ms: u64,
    /// `active` — сеть с этого компьютера теперь подбирает музыку, `kept` — проверку не прошла,
    /// `failed` — обучение не удалось.
    pub outcome: String,
    pub rows: i64,
    pub took_ms: u64,
    /// Решение сервера или ошибка — для журнала, интерфейс показывает короткое.
    pub note: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    /// Выбор человека: `null` — по умолчанию (участвуют только администраторы).
    pub enabled: Option<bool>,
    pub phase: Phase,
    pub last: Option<LastRun>,
}

pub struct Trainer {
    inner: Mutex<(Phase, Option<LastRun>)>,
    wake: Notify,
    http: reqwest::Client,
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
}

pub fn init(app: &AppHandle) {
    let http = reqwest::Client::builder()
        .user_agent(format!(
            "DalyWave/{} ({})",
            app.package_info().version,
            crate::platform::OS
        ))
        .connect_timeout(Duration::from_secs(10))
        // Набор — до нескольких десятков мегабайт, проверка весов на сервере — до минуты
        .timeout(Duration::from_secs(10 * 60))
        .build()
        .unwrap_or_default();
    app.manage(Trainer {
        inner: Mutex::new((Phase::Idle, None)),
        wake: Notify::new(),
        http,
    });
    tauri::async_runtime::spawn(supervise(app.clone()));
}

pub fn snapshot(app: &AppHandle) -> Option<Snapshot> {
    let t = app.try_state::<Trainer>()?;
    let (phase, last) = t.inner.lock().unwrap_or_else(|e| e.into_inner()).clone();
    Some(Snapshot {
        enabled: app.state::<Settings>().taste_training(),
        phase,
        last,
    })
}

fn set(app: &AppHandle, phase: Phase, last: Option<LastRun>) {
    let t = app.state::<Trainer>();
    {
        let mut inner = t.inner.lock().unwrap_or_else(|e| e.into_inner());
        if inner.0 != phase {
            log::info!("trainer: {:?} → {:?}", inner.0, phase);
        }
        inner.0 = phase;
        if last.is_some() {
            inner.1 = last;
        }
    }
    if let Some(s) = snapshot(app) {
        let _ = app.emit_to(MAIN, EVENT_STATE, &s);
    }
}

/// Вход, выход, другой сервер, перемена в настройках — проверить сейчас.
pub fn changed(app: &AppHandle) {
    if let Some(t) = app.try_state::<Trainer>() {
        t.wake.notify_one();
    }
}

async fn supervise(app: AppHandle) {
    let mut delay = FIRST_DELAY;
    loop {
        let t = app.state::<Trainer>();
        tokio::select! {
            () = tokio::time::sleep(delay) => {}
            () = t.wake.notified() => {}
        }
        delay = match round(&app).await {
            Ok(next) => next,
            Err(e) => {
                log::warn!("trainer: {e}");
                IDLE_DELAY
            }
        };
    }
}

/// Один заход: можно ли учить, есть ли задание, обучение и отправка. Возвращает паузу до следующего.
async fn round(app: &AppHandle) -> ShellResult<Duration> {
    let choice = app.state::<Settings>().taste_training();
    if choice == Some(false) {
        set(app, Phase::Off, None);
        return Ok(IDLE_DELAY);
    }
    if let Some(busy) = machine_busy().await {
        set(app, busy, None);
        return Ok(BUSY_DELAY);
    }
    let server = app.state::<Settings>().server_url();
    let key = server.clone();
    let Some(token) = tauri::async_runtime::spawn_blocking(move || auth::load_token(&key))
        .await
        .map_err(ShellError::internal)??
    else {
        set(app, Phase::SignedOut, None);
        return Ok(IDLE_DELAY);
    };
    let t = app.state::<Trainer>();
    set(app, Phase::Fetching, None);
    // По умолчанию — только то, что сервер даст сам (администратору — общую сеть); включено — и свою
    let mut claim = claim(&t.http, &server, &token, "auto").await?;
    if claim.is_none() && choice == Some(true) {
        claim = self::claim(&t.http, &server, &token, "personal").await?;
    }
    let Some(claim) = claim else {
        set(
            app,
            if choice.is_none() {
                Phase::Off
            } else {
                Phase::Idle
            },
            None,
        );
        return Ok(IDLE_DELAY);
    };
    let task = claim["taskId"].as_str().unwrap_or_default().to_string();
    let rows = claim["rows"].as_i64().unwrap_or(0);
    log::info!(
        "trainer: task {task} ({}, {rows} rows)",
        claim["scope"].as_str().unwrap_or("?")
    );
    set(app, Phase::Training, None);
    let package = claim["package"].clone();
    let trained = tauri::async_runtime::spawn_blocking(move || train(package))
        .await
        .map_err(ShellError::internal)?;
    let (params, report) = match trained {
        Ok(x) => x,
        Err(e) => {
            log::warn!("trainer: training failed: {e}");
            let _ = post(
                &t.http,
                &server,
                &token,
                &format!("/api/taste/training/{task}/fail"),
                &json!({ "message": e }),
            )
            .await;
            set(app, Phase::Idle, Some(last("failed", rows, 0, &e)));
            return Ok(IDLE_DELAY);
        }
    };
    log::info!(
        "trainer: trained in {} ms, best epoch {} of {}",
        report.took_ms,
        report.best_epoch,
        report.epochs
    );
    set(app, Phase::Sending, None);
    let body = json!({ "params": wire::WireParams::new(&params), "report": report });
    let res = post(
        &t.http,
        &server,
        &token,
        &format!("/api/taste/training/{task}/result"),
        &body,
    )
    .await?;
    let active = res["active"].as_bool().unwrap_or(false);
    let note = res["note"].as_str().unwrap_or_default().to_string();
    log::info!(
        "trainer: server checked the weights — {} ({note})",
        if active { "switched on" } else { "kept off" }
    );
    set(
        app,
        Phase::Idle,
        Some(last(
            if active { "active" } else { "kept" },
            rows,
            report.took_ms,
            &note,
        )),
    );
    Ok(IDLE_DELAY)
}

fn last(outcome: &str, rows: i64, took_ms: u64, note: &str) -> LastRun {
    LastRun {
        at_ms: now_ms(),
        outcome: outcome.into(),
        rows,
        took_ms,
        note: note.into(),
    }
}

/// Набор из ответа сервера → обучение в потоке с низким приоритетом.
fn train(package: Value) -> Result<(dalywave_taste::Params, TrainReport), String> {
    let pkg: wire::Package = serde_json::from_value(package).map_err(|e| e.to_string())?;
    let (spec, set) = pkg.decode()?;
    std::thread::Builder::new()
        .name("taste-trainer".into())
        .spawn(move || {
            lower_priority();
            dalywave_taste::train(&set, &spec)
        })
        .map_err(|e| e.to_string())?
        .join()
        .map_err(|_| "обучение упало".to_string())?
}

/// Поток обучения уступает всему остальному: фоновый класс QoS на macOS, «простаивающий» на Windows.
fn lower_priority() {
    #[cfg(target_os = "macos")]
    {
        // QOS_CLASS_BACKGROUND из <sys/qos.h>
        const QOS_CLASS_BACKGROUND: u32 = 0x09;
        unsafe extern "C" {
            fn pthread_set_qos_class_self_np(qos: u32, relative: i32) -> i32;
        }
        // SAFETY: функция только меняет класс текущего потока
        unsafe {
            pthread_set_qos_class_self_np(QOS_CLASS_BACKGROUND, 0);
        }
    }
    #[cfg(windows)]
    {
        use windows_sys::Win32::System::Threading::{
            GetCurrentThread, SetThreadPriority, THREAD_PRIORITY_IDLE,
        };
        // SAFETY: псевдодескриптор текущего потока
        unsafe {
            SetThreadPriority(GetCurrentThread(), THREAD_PRIORITY_IDLE);
        }
    }
}

async fn claim(
    http: &reqwest::Client,
    server: &str,
    token: &str,
    scope: &str,
) -> ShellResult<Option<Value>> {
    let res = http
        .post(format!("{server}/api/taste/training/claim"))
        .bearer_auth(token)
        .json(&json!({ "scope": scope }))
        .send()
        .await
        .map_err(|e| ShellError::Network(e.to_string()))?;
    match res.status().as_u16() {
        204 => Ok(None),
        200 => Ok(Some(
            res.json()
                .await
                .map_err(|e| ShellError::Network(e.to_string()))?,
        )),
        // Общую сеть просит не администратор, или устройство не компьютер — учить нечего
        403 => Ok(None),
        status => Err(server_error(status, res).await),
    }
}

async fn post(
    http: &reqwest::Client,
    server: &str,
    token: &str,
    path: &str,
    body: &Value,
) -> ShellResult<Value> {
    let res = http
        .post(format!("{server}{path}"))
        .bearer_auth(token)
        .json(body)
        .send()
        .await
        .map_err(|e| ShellError::Network(e.to_string()))?;
    let status = res.status().as_u16();
    match status {
        200 => res
            .json()
            .await
            .map_err(|e| ShellError::Network(e.to_string())),
        204 => Ok(Value::Null),
        _ => Err(server_error(status, res).await),
    }
}

async fn server_error(status: u16, res: reqwest::Response) -> ShellError {
    let message = res
        .json::<Value>()
        .await
        .ok()
        .and_then(|v| v["error"]["message"].as_str().map(String::from))
        .unwrap_or_default();
    ShellError::Server { status, message }
}

/// Можно ли учить сейчас: на питании от сети и не занят. `Some(причина)` — нельзя.
async fn machine_busy() -> Option<Phase> {
    tauri::async_runtime::spawn_blocking(|| {
        if !on_ac_power() {
            return Some(Phase::OnBattery);
        }
        if cpu_load() > BUSY_LOAD {
            return Some(Phase::Busy);
        }
        None
    })
    .await
    .unwrap_or(Some(Phase::Busy))
}

/// macOS: `pmset -g batt` пишет «Now drawing from 'AC Power'»; у Mac без батареи — тоже.
#[cfg(target_os = "macos")]
fn on_ac_power() -> bool {
    std::process::Command::new("/usr/bin/pmset")
        .args(["-g", "batt"])
        .output()
        .map(|o| !String::from_utf8_lossy(&o.stdout).contains("'Battery Power'"))
        .unwrap_or(true)
}

/// Windows: питание от сети или неизвестно (настольный компьютер).
#[cfg(windows)]
fn on_ac_power() -> bool {
    use windows_sys::Win32::System::Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS};
    // SAFETY: структура заполняется вызовом
    unsafe {
        let mut s: SYSTEM_POWER_STATUS = std::mem::zeroed();
        GetSystemPowerStatus(&mut s) == 0 || s.ACLineStatus != 0
    }
}

#[cfg(not(any(target_os = "macos", windows)))]
fn on_ac_power() -> bool {
    true
}

/// Загрузка процессора как доля ядер: средняя за минуту (macOS) или за секунду (Windows).
#[cfg(target_os = "macos")]
fn cpu_load() -> f64 {
    let cores = std::thread::available_parallelism().map_or(1, |n| n.get()) as f64;
    std::process::Command::new("/usr/sbin/sysctl")
        .args(["-n", "vm.loadavg"])
        .output()
        .ok()
        .and_then(|o| {
            String::from_utf8_lossy(&o.stdout)
                .split_whitespace()
                .nth(1)
                .and_then(|x| x.parse::<f64>().ok())
        })
        .map_or(0.0, |l| l / cores)
}

#[cfg(windows)]
fn cpu_load() -> f64 {
    use windows_sys::Win32::{Foundation::FILETIME, System::Threading::GetSystemTimes};
    fn ticks(f: &FILETIME) -> u64 {
        (u64::from(f.dwHighDateTime) << 32) | u64::from(f.dwLowDateTime)
    }
    let sample = || {
        let (mut idle, mut kernel, mut user) =
            unsafe { std::mem::zeroed::<(FILETIME, FILETIME, FILETIME)>() };
        // SAFETY: три структуры заполняются вызовом
        let ok = unsafe { GetSystemTimes(&mut idle, &mut kernel, &mut user) } != 0;
        ok.then(|| (ticks(&idle), ticks(&kernel) + ticks(&user)))
    };
    let Some((i0, t0)) = sample() else {
        return 0.0;
    };
    std::thread::sleep(Duration::from_secs(1));
    let Some((i1, t1)) = sample() else {
        return 0.0;
    };
    let total = t1.saturating_sub(t0).max(1) as f64;
    1.0 - i1.saturating_sub(i0) as f64 / total
}

#[cfg(not(any(target_os = "macos", windows)))]
fn cpu_load() -> f64 {
    0.0
}

// ---- команды интерфейса ----

#[tauri::command]
pub async fn trainer_state(app: AppHandle) -> ShellResult<Snapshot> {
    snapshot(&app).ok_or_else(|| ShellError::Internal("обучение не запущено".into()))
}

/// `null` — по умолчанию (только компьютеры администраторов).
#[tauri::command]
pub async fn trainer_set_enabled(app: AppHandle, enabled: Option<bool>) -> ShellResult<Snapshot> {
    app.state::<Settings>().set_taste_training(enabled)?;
    log::info!("trainer: helping is now {enabled:?}");
    if enabled == Some(false) {
        set(&app, Phase::Off, None);
    }
    changed(&app);
    trainer_state(app).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_is_camel_case() {
        let s = Snapshot {
            enabled: None,
            phase: Phase::OnBattery,
            last: Some(last("kept", 1200, 4300, "…")),
        };
        let v = serde_json::to_value(&s).unwrap();
        assert_eq!(v["phase"], "on-battery");
        assert_eq!(v["enabled"], Value::Null);
        assert_eq!(v["last"]["outcome"], "kept");
        assert_eq!(v["last"]["tookMs"], 4300);
    }

    #[test]
    fn trains_from_a_package_like_the_server_sends() {
        // Маленький набор в формате сервера → те же веса, что дал бы сервер с этим зерном
        let mut set = dalywave_taste::TrainSet {
            sound_dim: 8,
            semantic_dim: 4,
            scalar_dim: 3,
            ..Default::default()
        };
        for t in 0..10 {
            set.vectors
                .extend((0..12).map(|d| ((t * 7 + d) % 5) as f32 / 5.0));
        }
        for i in 0..200u32 {
            let t = i % 10;
            set.row_vector.push(t);
            set.scalars.extend([t as f32 / 10.0, (i % 3) as f32, 1.0]);
            let skip = t % 3 == 0;
            set.labels
                .extend([skip as u8 as f32, (!skip) as u8 as f32, 0.0]);
            set.weights.push(1.0);
            set.groups.push(i / 8);
        }
        let spec = dalywave_taste::Spec {
            max_epochs: 5,
            seed: 3,
            ..Default::default()
        };
        let pkg = wire::Package::new(spec.clone(), &set, &["a", "b", "c"]);
        let (params, report) = train(serde_json::to_value(&pkg).unwrap()).unwrap();
        assert!(report.epochs >= 1);
        let (spec2, set2) = pkg.decode().unwrap();
        let (same, _) = dalywave_taste::train(&set2, &spec2).unwrap();
        assert_eq!(params, same);
        assert_eq!(spec, spec2);
    }
}
