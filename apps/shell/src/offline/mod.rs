//! Умный офлайн: локальная база медиатеки, загрузки на устройство и волна без сети.
//!
//! - **База** (`dalywave-store`, SQLite) — своя на каждый сервер: медиатека приходит синхронизацией
//!   (`dalywave-sync`), экраны в приложении читают её первой (`offline_read`), сайт ходит в сеть.
//! - **Загрузки** (`dalywave-downloads`): закреплённое человеком и умный запас (набор волны устройства
//!   с сервера) в пределах места; файлы — в `Application Support/rest.daly.wave/offline/files`
//!   (на iPhone — без резервной копии iCloud). Компьютер качает сам с докачкой, iPhone — фоновой
//!   сессией URLSession (`ios_bg.rs`): загрузка идёт и когда приложение усыплено.
//! - **Волна без сети** (`dalywave-core`): подбор из скачанного по настройкам и пропускам.
//!
//! Плеер играет скачанное первым (`prefer_local`): ссылка на сервер не нужна вовсе.

#[cfg(target_os = "ios")]
mod ios_bg;
mod net;
mod smoke;

use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

use chrono::{DateTime, Utc};
use dalywave_audio::{SourceSpec, TrackSpec};
use dalywave_core::wave::{Candidate, Reaction, Session as WaveSession};
use dalywave_downloads::{ESTIMATED_TRACK_BYTES, FetchError, PlanInput};
#[cfg(not(target_os = "ios"))]
use dalywave_protocol::offline::FileLink;
use dalywave_protocol::{catalog::Track, wave::WaveSettings};
use dalywave_store::{FileState, LocalSearch, NewFile, PinState, PlayRecord, Store, Usage};
use dalywave_sync::{Client, SyncError};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::Notify;
use uuid::Uuid;

use crate::{
    MAIN,
    error::{ShellError, ShellResult},
    settings::Settings,
};

pub const EVENT_STATE: &str = "offline:state";
pub const EVENT_PROGRESS: &str = "downloads:progress";
/// Медиатека в базе изменилась после синхронизации — экраны перечитывают свои списки.
pub const EVENT_SYNCED: &str = "offline:synced";

/// Предел места по умолчанию: телефон — 4 ГБ, компьютер — 20 ГБ.
const DEFAULT_LIMIT: u64 = if cfg!(target_os = "ios") {
    4 * 1024 * 1024 * 1024
} else {
    20 * 1024 * 1024 * 1024
};
const MIN_LIMIT: u64 = 512 * 1024 * 1024;
/// Синхронизация без поводов — раз в 10 минут; поводы (вход, лайк, закрепление) будят сразу.
const SYNC_EVERY: Duration = Duration::from_secs(600);
/// Набор волны пересобирается так часто, а сыгранное новое — сразу, как его наберётся столько.
const SET_EVERY_HOURS: i64 = 12;
const SET_PLAYED_FRESH: usize = 8;
/// Ссылок за запрос.
const LINK_BATCH: usize = 20;
/// iPhone: сколько файлов одновременно в фоновой сессии.
#[cfg(target_os = "ios")]
const IOS_PARALLEL: usize = 4;
/// Ход загрузки трека интерфейсу — не чаще.
#[cfg(not(target_os = "ios"))]
const PROGRESS_EVERY: Duration = Duration::from_millis(250);

/// Настройки загрузок этого устройства (хранятся в базе сервера).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct OfflineSettings {
    pub limit_bytes: u64,
    /// Качать только по Wi-Fi (на компьютере сотовой сети нет — не влияет).
    pub wifi_only: bool,
    /// Умный запас: набор волны с сервера докачивается сам.
    pub reserve_enabled: bool,
    /// «Без сети»: приложение не ходит на сервер — медиатека, плеер и волна только из скачанного.
    pub force_offline: bool,
}

impl Default for OfflineSettings {
    fn default() -> Self {
        Self {
            limit_bytes: DEFAULT_LIMIT,
            wifi_only: cfg!(target_os = "ios"),
            reserve_enabled: true,
            force_offline: false,
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsPatch {
    pub limit_bytes: Option<u64>,
    pub wifi_only: Option<bool>,
    pub reserve_enabled: Option<bool>,
    pub force_offline: Option<bool>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageView {
    pub pinned_bytes: u64,
    pub reserve_bytes: u64,
    pub cache_bytes: u64,
    pub total_bytes: u64,
    pub files: u64,
}

impl From<Usage> for UsageView {
    fn from(u: Usage) -> Self {
        Self {
            pinned_bytes: u.pinned,
            reserve_bytes: u.reserve,
            cache_bytes: u.cache,
            total_bytes: u.total(),
            files: u.files,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadsView {
    /// Качается сейчас.
    pub active: usize,
    /// Ждёт очереди.
    pub queued: usize,
    pub failed: usize,
    /// Закреплённое, которое не помещается в предел.
    pub over_limit: usize,
    /// Ждёт Wi-Fi (включено «только по Wi-Fi», а сеть сотовая).
    pub waiting_for_wifi: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReserveView {
    pub tracks: usize,
    pub ready: usize,
    pub built_at: Option<String>,
}

/// Всё о загрузках и офлайне для интерфейса.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OfflineStatus {
    pub online: bool,
    /// `wifi` | `cellular` | `none` | `unknown`
    pub network: &'static str,
    pub synced: bool,
    pub synced_at: Option<String>,
    pub syncing: bool,
    pub settings: OfflineSettings,
    pub usage: UsageView,
    pub downloads: DownloadsView,
    pub pins: Vec<PinState>,
    pub reserve: ReserveView,
}

/// Состояние загрузки трека.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackDownload {
    pub track_id: Uuid,
    /// `queued` | `downloading` | `ready` | `failed`
    pub state: &'static str,
    pub received: u64,
    pub total: u64,
}

/// База и папка файлов текущего сервера.
pub struct Ctx {
    server: String,
    store: Mutex<Store>,
    files: PathBuf,
}

impl Ctx {
    fn store(&self) -> std::sync::MutexGuard<'_, Store> {
        self.store.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn settings(&self) -> OfflineSettings {
        self.store()
            .meta("settings.offline")
            .ok()
            .flatten()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    fn abs(&self, rel: &str) -> PathBuf {
        self.files.join(rel)
    }
}

pub struct Offline {
    http: reqwest::Client,
    /// Компьютер качает сам (ureq) — с тем же User-Agent
    #[cfg_attr(target_os = "ios", allow(dead_code))]
    user_agent: String,
    root: Option<PathBuf>,
    ctx: Mutex<Option<Arc<Ctx>>>,
    wake_sync: Notify,
    wake_downloads: Notify,
    /// Сервер отвечал при последней попытке.
    reachable: AtomicBool,
    syncing: AtomicBool,
    /// Компьютер: остановить текущую загрузку (откреплено, «Удалить всё», «Без сети»).
    cancel: Arc<AtomicBool>,
    /// Что качается сейчас: получено и всего.
    active: Mutex<HashMap<Uuid, (u64, u64)>>,
    over_limit: Mutex<usize>,
    waves: Mutex<HashMap<String, WaveSession>>,
    token: Mutex<Option<(String, Option<String>)>>,
    last_state: Mutex<Option<Instant>>,
    /// Проверка без входа (`smoke.rs`): своя база вместо базы сервера
    smoke: OnceLock<String>,
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// Имя папки сервера: только буквы, цифры и точки.
fn server_key(server: &str) -> String {
    server
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '.' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

fn extension(container: &str) -> &'static str {
    match container {
        "mp4" => "m4a",
        "ogg" => "ogg",
        "webm" => "webm",
        "mp3" => "mp3",
        "flac" => "flac",
        "wav" => "wav",
        _ => "bin",
    }
}

impl Offline {
    fn new(app: &AppHandle) -> Self {
        let user_agent = format!(
            "DalyWave/{} ({})",
            app.package_info().version,
            crate::platform::OS
        );
        let http = reqwest::Client::builder()
            .user_agent(&user_agent)
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(60))
            .build()
            .unwrap_or_default();
        let root = app.path().app_data_dir().ok().map(|d| d.join("offline"));
        Self {
            http,
            user_agent,
            root,
            ctx: Mutex::new(None),
            wake_sync: Notify::new(),
            wake_downloads: Notify::new(),
            reachable: AtomicBool::new(true),
            syncing: AtomicBool::new(false),
            cancel: Arc::new(AtomicBool::new(false)),
            active: Mutex::new(HashMap::new()),
            over_limit: Mutex::new(0),
            waves: Mutex::new(HashMap::new()),
            token: Mutex::new(None),
            last_state: Mutex::new(None),
            smoke: OnceLock::new(),
        }
    }

    /// База текущего сервера; другой сервер — другая база.
    pub fn ctx(&self, app: &AppHandle) -> ShellResult<Arc<Ctx>> {
        let server = match self.smoke.get() {
            Some(s) => s.clone(),
            None => app.state::<Settings>().server_url(),
        };
        let mut slot = lock(&self.ctx);
        if let Some(c) = slot.as_ref()
            && c.server == server
        {
            return Ok(c.clone());
        }
        let root = self
            .root
            .clone()
            .ok_or_else(|| ShellError::Internal("нет папки данных приложения".into()))?;
        std::fs::create_dir_all(&root).map_err(ShellError::internal)?;
        #[cfg(target_os = "ios")]
        ios_bg::exclude_from_backup(&root);
        let key = server_key(&server);
        let store =
            Store::open(&root.join(format!("{key}.sqlite"))).map_err(ShellError::internal)?;
        let device = crate::auth::device_id(app, &server).and_then(|s| s.parse::<Uuid>().ok());
        if device.is_some() {
            let _ = store.set_device_id(device);
        }
        let ctx = Arc::new(Ctx {
            server,
            store: Mutex::new(store),
            files: root.join("files").join(key),
        });
        *slot = Some(ctx.clone());
        Ok(ctx)
    }

    /// Токен устройства; «нет входа» не запоминается — после входа он появится сам.
    fn token(&self, server: &str) -> Option<String> {
        if let Some((s, Some(t))) = lock(&self.token).clone()
            && s == server
        {
            return Some(t);
        }
        let t = crate::auth::load_token(server).ok().flatten();
        *lock(&self.token) = Some((server.to_string(), t.clone()));
        t
    }

    /// Вход сменился: токен перечитать.
    pub fn forget_token(&self) {
        *lock(&self.token) = None;
    }

    /// Сейчас без сети: «Без сети» включено, сети нет или сервер не отвечает.
    pub fn offline_now(&self) -> bool {
        let forced = lock(&self.ctx)
            .as_ref()
            .is_some_and(|c| c.settings().force_offline);
        forced || net::kind() == net::Kind::None || !self.reachable.load(Ordering::Relaxed)
    }

    fn status(&self, ctx: &Ctx) -> ShellResult<OfflineStatus> {
        let settings = ctx.settings();
        let kind = net::kind();
        let store = ctx.store();
        let pins = store.pin_states().map_err(ShellError::internal)?;
        let usage = store.usage().map_err(ShellError::internal)?;
        let files = store.files().map_err(ShellError::internal)?;
        let reserve = store.reserve().map_err(ShellError::internal)?;
        let ready: std::collections::HashSet<Uuid> = files
            .iter()
            .filter(|f| f.state == FileState::Ready)
            .map(|f| f.track_id)
            .collect();
        let active = lock(&self.active).len();
        let queued = files
            .iter()
            .filter(|f| matches!(f.state, FileState::Queued | FileState::Downloading))
            .count();
        Ok(OfflineStatus {
            online: !settings.force_offline
                && kind != net::Kind::None
                && self.reachable.load(Ordering::Relaxed),
            network: kind.as_str(),
            synced: store.synced().unwrap_or(false),
            synced_at: store.meta("sync.completed_at").ok().flatten(),
            syncing: self.syncing.load(Ordering::Relaxed),
            usage: usage.into(),
            downloads: DownloadsView {
                active,
                queued: queued.saturating_sub(active),
                failed: files
                    .iter()
                    .filter(|f| f.state == FileState::Failed)
                    .count(),
                over_limit: *lock(&self.over_limit),
                waiting_for_wifi: settings.wifi_only && kind == net::Kind::Cellular && queued > 0,
            },
            pins,
            reserve: ReserveView {
                tracks: reserve.len(),
                ready: reserve
                    .iter()
                    .filter(|r| ready.contains(&r.track_id))
                    .count(),
                built_at: store.meta("set.built_at").ok().flatten(),
            },
            settings,
        })
    }

    /// Состояние — интерфейсу; частые перемены (ход загрузки) — не чаще раза в секунду.
    fn emit_state(&self, app: &AppHandle, force: bool) {
        {
            let mut last = lock(&self.last_state);
            if !force && last.is_some_and(|t| t.elapsed() < Duration::from_secs(1)) {
                return;
            }
            *last = Some(Instant::now());
        }
        if let Ok(ctx) = self.ctx(app)
            && let Ok(s) = self.status(&ctx)
        {
            let _ = app.emit_to(MAIN, EVENT_STATE, s);
        }
    }

    fn emit_progress(
        &self,
        app: &AppHandle,
        track_id: Uuid,
        state: FileState,
        received: u64,
        total: u64,
    ) {
        let _ = app.emit_to(
            MAIN,
            EVENT_PROGRESS,
            TrackDownload {
                track_id,
                state: state.as_str(),
                received,
                total,
            },
        );
    }

    /// Разбудить синхронизацию (после изменения медиатеки, входа, закрепления).
    pub fn poke(&self) {
        self.wake_sync.notify_one();
    }
}

// ---------- запуск ----------

pub fn init(app: &AppHandle) {
    app.manage(Offline::new(app));
    #[cfg(target_os = "ios")]
    ios_bg::init(app);
    let handle = app.clone();
    tauri::async_runtime::spawn(async move { sync_loop(handle).await });
    let handle = app.clone();
    tauri::async_runtime::spawn(async move { download_loop(handle).await });
    smoke::spawn(app);
}

/// Скачанный файл вместо ссылки: плеер играет его без сети и без запроса ссылки.
pub fn prefer_local(app: &AppHandle, mut spec: TrackSpec) -> TrackSpec {
    let Some(track_id) = spec
        .stream_id
        .as_deref()
        .and_then(|s| s.parse::<Uuid>().ok())
    else {
        return spec;
    };
    let Some(off) = app.try_state::<Offline>() else {
        return spec;
    };
    let Ok(ctx) = off.ctx(app) else { return spec };
    let file = ctx.store().ready_file(track_id).ok().flatten();
    if let Some(f) = file {
        let path = ctx.abs(&f.path);
        if path.exists() {
            spec.source = SourceSpec::File(path);
            spec.duration_ms = spec.duration_ms.or(f.duration_ms.map(|d| d as u64));
            spec.loudness_lufs = spec.loudness_lufs.or(f.loudness_lufs);
            spec.peak_db = spec.peak_db.or(f.true_peak_db);
            let _ = ctx.store().touch_file(track_id);
        }
    }
    spec
}

/// Ссылку у сервера спрашивать бессмысленно: «Без сети» или сети нет.
pub fn offline_now(app: &AppHandle) -> bool {
    app.try_state::<Offline>().is_some_and(|o| o.offline_now())
}

async fn sync_loop(app: AppHandle) {
    tokio::time::sleep(Duration::from_secs(2)).await;
    loop {
        if let Err(e) = sync_once(&app).await {
            log::warn!("offline: sync failed: {e}");
        }
        let off = app.state::<Offline>();
        tokio::select! {
            _ = off.wake_sync.notified() => {}
            _ = tokio::time::sleep(SYNC_EVERY) => {}
        }
    }
}

async fn sync_once(app: &AppHandle) -> ShellResult<()> {
    let off = app.state::<Offline>();
    let ctx = off.ctx(app)?;
    let settings = ctx.settings();
    if settings.force_offline || net::kind() == net::Kind::None {
        off.emit_state(app, true);
        return Ok(());
    }
    let server = ctx.server.clone();
    let token = {
        let off_server = server.clone();
        let handle = app.clone();
        tauri::async_runtime::spawn_blocking(move || handle.state::<Offline>().token(&off_server))
            .await
            .ok()
            .flatten()
    };
    let Some(token) = token else {
        return Ok(());
    };
    // Устройство могло появиться после входа
    if ctx.store().device_id().ok().flatten().is_none()
        && let Some(d) = crate::auth::device_id(app, &server).and_then(|s| s.parse().ok())
    {
        let _ = ctx.store().set_device_id(Some(d));
    }
    let client = Client::new(off.http.clone(), &server, &token);
    off.syncing.store(true, Ordering::Relaxed);
    off.emit_state(app, true);
    let started = Instant::now();
    let result = async {
        client.push_pins(&ctx.store).await?;
        let pulled = client.pull(&ctx.store).await?;
        if settings.reserve_enabled && set_due(&ctx) {
            let (fav, fresh) = set_size(&ctx, &settings);
            match client.rebuild_set(fav, fresh).await {
                Ok(set) => {
                    let _ = ctx
                        .store()
                        .set_meta("set.built_at", Some(&set.built_at.to_rfc3339()));
                    log::info!(
                        "offline: wave set rebuilt — {} favorites, {} fresh, {} removed",
                        set.favorites,
                        set.fresh,
                        set.removed
                    );
                    let more = client.pull(&ctx.store).await?;
                    return Ok::<_, SyncError>((
                        pulled.changes.library || more.changes.library,
                        true,
                    ));
                }
                Err(e) => log::warn!("offline: wave set not rebuilt: {e}"),
            }
        }
        Ok((
            pulled.changes.library,
            pulled.changes.reserve || pulled.changes.pins,
        ))
    }
    .await;
    off.syncing.store(false, Ordering::Relaxed);
    match result {
        Ok((library, downloads)) => {
            off.reachable.store(true, Ordering::Relaxed);
            log::info!(
                "offline: synced in {:.0} ms",
                started.elapsed().as_secs_f64() * 1000.0
            );
            if library {
                let _ = app.emit_to(MAIN, EVENT_SYNCED, Value::Null);
            }
            if downloads || library {
                off.wake_downloads.notify_one();
            }
        }
        Err(SyncError::Network(e)) => {
            off.reachable.store(false, Ordering::Relaxed);
            log::info!("offline: server unreachable: {e}");
        }
        Err(SyncError::Unauthorized) => off.forget_token(),
        Err(e) => log::warn!("offline: sync error: {e}"),
    }
    off.emit_state(app, true);
    Ok(())
}

/// Пора пересобрать набор волны: не собирался, устарел или новое из него уже сыграно.
fn set_due(ctx: &Ctx) -> bool {
    let store = ctx.store();
    let built = store
        .meta("set.built_at")
        .ok()
        .flatten()
        .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
        .map(|d| d.with_timezone(&Utc));
    match built {
        None => true,
        Some(at) => {
            Utc::now() - at > chrono::Duration::hours(SET_EVERY_HOURS)
                || store.played_fresh().unwrap_or(0) >= SET_PLAYED_FRESH
        }
    }
}

/// Сколько любимого и нового поместится в запас: место за вычетом закреплённого, треком по ~6 МБ.
fn set_size(ctx: &Ctx, s: &OfflineSettings) -> (u32, u32) {
    let pinned = ctx.store().usage().map(|u| u.pinned).unwrap_or(0);
    let tracks = (s.limit_bytes.saturating_sub(pinned) / ESTIMATED_TRACK_BYTES).min(2000) as u32;
    let fresh = (tracks * 2 / 5).min(400);
    (tracks - fresh, fresh)
}

// ---------- загрузки ----------

async fn download_loop(app: AppHandle) {
    tokio::time::sleep(Duration::from_secs(3)).await;
    #[cfg(target_os = "ios")]
    let mut events = ios_bg::events();
    loop {
        let more = match downloads_once(&app).await {
            Ok(more) => more,
            Err(e) => {
                log::warn!("offline: downloads: {e}");
                false
            }
        };
        let off = app.state::<Offline>();
        let wait = if more {
            Duration::from_secs(20)
        } else {
            Duration::from_secs(300)
        };
        #[cfg(target_os = "ios")]
        {
            tokio::select! {
                _ = off.wake_downloads.notified() => {}
                _ = tokio::time::sleep(wait) => {}
                Some(ev) = events.recv() => ios_finished(&app, ev),
            }
            while let Ok(ev) = events.try_recv() {
                ios_finished(&app, ev);
            }
        }
        #[cfg(not(target_os = "ios"))]
        tokio::select! {
            _ = off.wake_downloads.notified() => {}
            _ = tokio::time::sleep(wait) => {}
        }
    }
}

fn remove_files(ctx: &Ctx, rel: &str) {
    let path = ctx.abs(rel);
    for p in [
        path.clone(),
        dalywave_downloads::part_path(&path),
        path.with_extension("resume"),
    ] {
        let _ = std::fs::remove_file(p);
    }
}

/// Один проход: вытеснить лишнее, получить ссылки и качать. `true` — осталось что качать.
async fn downloads_once(app: &AppHandle) -> ShellResult<bool> {
    let off = app.state::<Offline>();
    let ctx = off.ctx(app)?;
    let settings = ctx.settings();
    let plan = {
        let store = ctx.store();
        let pinned = store.pinned_tracks().map_err(ShellError::internal)?;
        let reserve: Vec<Uuid> = store
            .reserve()
            .map_err(ShellError::internal)?
            .into_iter()
            .map(|r| r.track_id)
            .collect();
        let files = store.files().map_err(ShellError::internal)?;
        dalywave_downloads::plan(PlanInput {
            pinned: &pinned,
            reserve: &reserve,
            files: &files,
            limit: settings.limit_bytes,
            reserve_enabled: settings.reserve_enabled,
        })
    };
    *lock(&off.over_limit) = plan.over_limit.len();
    if !plan.evict.is_empty() {
        let store = ctx.store();
        for id in &plan.evict {
            if let Ok(Some(f)) = store.file(*id) {
                #[cfg(target_os = "ios")]
                ios_bg::cancel(*id);
                remove_files(&ctx, &f.path);
                let _ = store.delete_file(*id);
            }
        }
        drop(store);
        log::info!("offline: evicted {} files", plan.evict.len());
        off.emit_state(app, true);
    }
    let kind = net::kind();
    let allowed = !settings.force_offline
        && kind != net::Kind::None
        && off.reachable.load(Ordering::Relaxed)
        && !(settings.wifi_only && kind == net::Kind::Cellular);
    if plan.fetch.is_empty() {
        off.emit_state(app, false);
        return Ok(false);
    }
    if !allowed {
        off.emit_state(app, true);
        return Ok(false);
    }
    let Some(token) = off.token(&ctx.server) else {
        return Ok(false);
    };
    let client = Client::new(off.http.clone(), &ctx.server, &token);
    off.cancel.store(false, Ordering::Relaxed);
    #[cfg(target_os = "ios")]
    let todo: Vec<Uuid> = {
        let busy = ios_bg::in_flight();
        let room = IOS_PARALLEL.saturating_sub(busy.len());
        plan.fetch
            .iter()
            .filter(|id| !busy.contains(id))
            .take(room)
            .copied()
            .collect()
    };
    #[cfg(not(target_os = "ios"))]
    let todo = plan.fetch.clone();
    for chunk in todo.chunks(LINK_BATCH) {
        let links = match client.files(chunk).await {
            Ok(l) => l,
            Err(SyncError::Network(e)) => {
                off.reachable.store(false, Ordering::Relaxed);
                log::info!("offline: no links, server unreachable: {e}");
                return Ok(true);
            }
            Err(e) => return Err(ShellError::Internal(e.to_string())),
        };
        for link in links.items {
            let rel = {
                let t = link.track_id.to_string();
                format!(
                    "{}/{}.{}",
                    &t[t.len() - 2..],
                    t,
                    extension(link.container.as_str())
                )
            };
            let new = NewFile {
                track_id: link.track_id,
                recording_id: link.recording_id,
                file_id: link.file_id,
                path: rel.clone(),
                size_bytes: link.size_bytes.max(0) as u64,
                sha256: link.sha256.clone(),
                codec: link.codec.as_str().into(),
                container: link.container.as_str().into(),
                duration_ms: i64::from(link.duration_ms),
                loudness_lufs: link.loudness_lufs,
                true_peak_db: link.true_peak_db,
                waveform: link.waveform.clone(),
            };
            {
                let store = ctx.store();
                // Другой файл записи (сервер заменил копию лучшей) — недокачанное старое не годится
                if let Ok(Some(old)) = store.file(link.track_id)
                    && old.file_id != Some(link.file_id)
                {
                    remove_files(&ctx, &old.path);
                }
                store.queue_file(&new).map_err(ShellError::internal)?;
            }
            #[cfg(target_os = "ios")]
            {
                ios_bg::start(
                    link.track_id,
                    &link.url,
                    &dalywave_downloads::part_path(&ctx.abs(&rel)),
                    !settings.wifi_only,
                );
                let _ = ctx
                    .store()
                    .set_file_state(link.track_id, FileState::Downloading, 0, None);
                off.emit_progress(
                    app,
                    link.track_id,
                    FileState::Downloading,
                    0,
                    new.size_bytes,
                );
            }
            #[cfg(not(target_os = "ios"))]
            {
                let stop = desktop_fetch(app, &ctx, &link, &rel).await;
                if stop {
                    return Ok(true);
                }
            }
        }
        off.emit_state(app, true);
    }
    Ok(true)
}

/// Компьютер: скачать один файл. `true` — дальше не качать (сеть пропала, остановлено).
#[cfg(not(target_os = "ios"))]
async fn desktop_fetch(app: &AppHandle, ctx: &Arc<Ctx>, link: &FileLink, rel: &str) -> bool {
    let off = app.state::<Offline>();
    let id = link.track_id;
    let size = link.size_bytes.max(0) as u64;
    let _ = ctx
        .store()
        .set_file_state(id, FileState::Downloading, 0, None);
    lock(&off.active).insert(id, (0, size));
    off.emit_progress(app, id, FileState::Downloading, 0, size);
    let handle = app.clone();
    let dest = ctx.abs(rel);
    let url = link.url.clone();
    let sha = link.sha256.clone();
    let ua = off.user_agent.clone();
    let cancel = off.cancel.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        let mut last = Instant::now() - PROGRESS_EVERY;
        let req = dalywave_downloads::Request {
            url: &url,
            dest: &dest,
            size,
            sha256: sha.as_deref(),
            user_agent: &ua,
        };
        dalywave_downloads::fetch(&req, &cancel, |got, total| {
            if last.elapsed() >= PROGRESS_EVERY {
                last = Instant::now();
                let off = handle.state::<Offline>();
                lock(&off.active).insert(id, (got, total));
                off.emit_progress(&handle, id, FileState::Downloading, got, total);
                off.emit_state(&handle, false);
            }
        })
    })
    .await
    .unwrap_or_else(|e| Err(FetchError::Network(e.to_string())));
    lock(&off.active).remove(&id);
    let store = ctx.store();
    match result {
        Ok(n) => {
            let _ = store.set_file_state(id, FileState::Ready, n, None);
            drop(store);
            off.emit_progress(app, id, FileState::Ready, n, n);
            fetch_analysis(app, ctx, id);
            false
        }
        Err(FetchError::Cancelled) => {
            let _ = store.set_file_state(id, FileState::Queued, 0, None);
            off.emit_progress(app, id, FileState::Queued, 0, size);
            true
        }
        Err(e @ FetchError::Network(_)) => {
            log::info!("offline: download of {id} interrupted: {e}");
            let _ = store.set_file_state(id, FileState::Queued, 0, Some(&e.to_string()));
            off.emit_progress(app, id, FileState::Queued, 0, size);
            true
        }
        Err(e @ FetchError::Expired(_)) => {
            let _ = store.set_file_state(id, FileState::Queued, 0, Some(&e.to_string()));
            false
        }
        Err(e) => {
            log::warn!("offline: download of {id} failed: {e}");
            let _ = store.set_file_state(id, FileState::Failed, 0, Some(&e.to_string()));
            off.emit_progress(app, id, FileState::Failed, 0, size);
            false
        }
    }
}

/// Анализ трека для «Диджея» — рядом с файлом: переходы по долям работают и без сети.
fn fetch_analysis(app: &AppHandle, ctx: &Arc<Ctx>, track_id: Uuid) {
    let off = app.state::<Offline>();
    let Some(token) = off.token(&ctx.server) else {
        return;
    };
    let Ok(Some(f)) = ctx.store().file(track_id) else {
        return;
    };
    let dest = ctx.abs(&f.path).with_extension("analysis.json");
    if dest.exists() {
        return;
    }
    let url = format!("{}/api/tracks/{track_id}/analysis", ctx.server);
    let http = off.http.clone();
    tauri::async_runtime::spawn(async move {
        let Ok(res) = http.get(url).bearer_auth(token).send().await else {
            return;
        };
        if res.status().is_success()
            && let Ok(bytes) = res.bytes().await
        {
            let _ = std::fs::write(dest, bytes);
        }
    });
}

/// Анализ скачанного трека для «Диджея», если он сохранён рядом с файлом.
pub fn local_analysis(app: &AppHandle, track_id: &str) -> Option<Vec<u8>> {
    let id = track_id.parse::<Uuid>().ok()?;
    let off = app.try_state::<Offline>()?;
    let ctx = off.ctx(app).ok()?;
    let f = ctx.store().ready_file(id).ok().flatten()?;
    std::fs::read(ctx.abs(&f.path).with_extension("analysis.json")).ok()
}

/// iPhone: фоновая сессия закончила файл (или не смогла).
#[cfg(target_os = "ios")]
fn ios_finished(app: &AppHandle, ev: ios_bg::Event) {
    let off = app.state::<Offline>();
    let Ok(ctx) = off.ctx(app) else { return };
    match ev {
        ios_bg::Event::Progress {
            track_id,
            received,
            total,
        } => {
            lock(&off.active).insert(track_id, (received, total));
            off.emit_progress(app, track_id, FileState::Downloading, received, total);
            off.emit_state(app, false);
        }
        ios_bg::Event::Finished { track_id } => {
            lock(&off.active).remove(&track_id);
            let Ok(Some(f)) = ctx.store().file(track_id) else {
                return;
            };
            let dest = ctx.abs(&f.path);
            let part = dalywave_downloads::part_path(&dest);
            match dalywave_downloads::verify(&part, f.size_bytes, f.sha256.as_deref())
                .and_then(|()| std::fs::rename(&part, &dest).map_err(FetchError::from))
            {
                Ok(()) => {
                    let _ =
                        ctx.store()
                            .set_file_state(track_id, FileState::Ready, f.size_bytes, None);
                    off.emit_progress(app, track_id, FileState::Ready, f.size_bytes, f.size_bytes);
                    fetch_analysis(app, &ctx, track_id);
                }
                Err(e) => {
                    let _ = std::fs::remove_file(&part);
                    log::warn!("offline: file of {track_id} failed the check: {e}");
                    let _ = ctx.store().set_file_state(
                        track_id,
                        FileState::Failed,
                        0,
                        Some(&e.to_string()),
                    );
                    off.emit_progress(app, track_id, FileState::Failed, 0, f.size_bytes);
                }
            }
            off.emit_state(app, true);
            off.wake_downloads.notify_one();
        }
        ios_bg::Event::Failed { track_id, error } => {
            lock(&off.active).remove(&track_id);
            log::info!("offline: background download of {track_id}: {error}");
            let _ = ctx
                .store()
                .set_file_state(track_id, FileState::Queued, 0, Some(&error));
            off.emit_progress(app, track_id, FileState::Queued, 0, 0);
            off.emit_state(app, true);
        }
    }
}

// ---------- команды ----------

#[tauri::command]
pub async fn offline_status(app: AppHandle) -> ShellResult<OfflineStatus> {
    let off = app.state::<Offline>();
    let ctx = off.ctx(&app)?;
    off.status(&ctx)
}

#[tauri::command]
pub async fn offline_set_settings(
    app: AppHandle,
    patch: SettingsPatch,
) -> ShellResult<OfflineStatus> {
    let off = app.state::<Offline>();
    let ctx = off.ctx(&app)?;
    let mut s = ctx.settings();
    let was_forced = s.force_offline;
    if let Some(l) = patch.limit_bytes {
        s.limit_bytes = l.max(MIN_LIMIT);
    }
    if let Some(w) = patch.wifi_only {
        s.wifi_only = w;
    }
    if let Some(r) = patch.reserve_enabled {
        s.reserve_enabled = r;
    }
    if let Some(f) = patch.force_offline {
        s.force_offline = f;
    }
    ctx.store()
        .set_meta(
            "settings.offline",
            Some(&serde_json::to_string(&s).map_err(ShellError::internal)?),
        )
        .map_err(ShellError::internal)?;
    if s.force_offline {
        off.cancel.store(true, Ordering::Relaxed);
    }
    if was_forced && !s.force_offline {
        off.reachable.store(true, Ordering::Relaxed);
        off.poke();
    }
    // Другой предел — другой запас
    if patch.limit_bytes.is_some() || patch.reserve_enabled == Some(true) {
        let _ = ctx.store().set_meta("set.built_at", None);
        off.poke();
    }
    off.wake_downloads.notify_one();
    let status = off.status(&ctx)?;
    let _ = app.emit_to(MAIN, EVENT_STATE, status.clone());
    Ok(status)
}

/// Синхронизировать сейчас (после изменения медиатеки): ждёт прохода не дольше 10 с.
#[tauri::command]
pub async fn offline_sync(app: AppHandle) -> ShellResult<()> {
    let _ = tokio::time::timeout(Duration::from_secs(10), sync_once(&app)).await;
    Ok(())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadQuery {
    /// `overview` | `liked` | `albums` | `artists` | `playlists` | `playlist` | `album` | `artist` |
    /// `search` | `tracks`
    what: String,
    #[serde(default)]
    id: Option<Uuid>,
    #[serde(default)]
    ids: Vec<Uuid>,
    #[serde(default)]
    cursor: Option<String>,
    #[serde(default)]
    limit: Option<usize>,
    #[serde(default)]
    q: Option<String>,
}

/// Медиатека из локальной базы в типах API. `null` — в базе этого нет (или она ещё не синхронизирована):
/// интерфейс идёт на сервер.
#[tauri::command]
pub async fn offline_read(app: AppHandle, query: ReadQuery) -> ShellResult<Value> {
    let off = app.state::<Offline>();
    let ctx = off.ctx(&app)?;
    let store = ctx.store();
    if !store.synced().unwrap_or(false) && !matches!(query.what.as_str(), "tracks") {
        return Ok(Value::Null);
    }
    let v = |r: Result<Value, serde_json::Error>| r.map_err(ShellError::internal);
    let e = ShellError::internal;
    Ok(match query.what.as_str() {
        "overview" => v(serde_json::to_value(store.overview().map_err(e)?))?,
        "liked" => v(serde_json::to_value(
            store
                .liked(query.cursor.as_deref(), query.limit.unwrap_or(200))
                .map_err(e)?,
        ))?,
        "albums" => v(serde_json::to_value(store.saved_albums().map_err(e)?))?,
        "artists" => v(serde_json::to_value(store.followed().map_err(e)?))?,
        "playlists" => v(serde_json::to_value(store.playlists().map_err(e)?))?,
        "playlist" => match query.id {
            Some(id) => v(serde_json::to_value(store.playlist(id).map_err(e)?))?,
            None => Value::Null,
        },
        "album" => match query.id {
            Some(id) => v(serde_json::to_value(store.album(id).map_err(e)?))?,
            None => Value::Null,
        },
        "artist" => match query.id {
            Some(id) => v(serde_json::to_value(store.artist(id).map_err(e)?))?,
            None => Value::Null,
        },
        "search" => v(serde_json::to_value(
            store
                .search(LocalSearch {
                    q: query.q.as_deref().unwrap_or(""),
                    limit: query.limit.unwrap_or(20),
                })
                .map_err(e)?,
        ))?,
        "tracks" => v(serde_json::to_value(store.tracks(&query.ids).map_err(e)?))?,
        _ => {
            return Err(ShellError::Invalid(format!(
                "нет такого раздела: {}",
                query.what
            )));
        }
    })
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PinArgs {
    kind: String,
    #[serde(default)]
    ref_id: Option<Uuid>,
}

fn check_kind(kind: &str) -> ShellResult<()> {
    match kind {
        "track" | "album" | "playlist" | "artist" | "liked" => Ok(()),
        other => Err(ShellError::Invalid(format!("нельзя закрепить «{other}»"))),
    }
}

/// Закрепить на устройстве: альбом, плейлист, трек, все лайки, артиста из подписок.
#[tauri::command]
pub async fn offline_pin(app: AppHandle, pin: PinArgs) -> ShellResult<OfflineStatus> {
    check_kind(&pin.kind)?;
    let off = app.state::<Offline>();
    let ctx = off.ctx(&app)?;
    let ref_id = if pin.kind == "liked" {
        None
    } else {
        pin.ref_id
    };
    ctx.store()
        .add_pin(&pin.kind, ref_id)
        .map_err(ShellError::internal)?;
    log::info!("offline: pinned {} {:?}", pin.kind, ref_id);
    // Треков альбома или артиста может ещё не быть в базе: синхронизация приведёт их вместе с закреплением
    off.poke();
    off.wake_downloads.notify_one();
    let s = off.status(&ctx)?;
    let _ = app.emit_to(MAIN, EVENT_STATE, s.clone());
    Ok(s)
}

#[tauri::command]
pub async fn offline_unpin(app: AppHandle, pin: PinArgs) -> ShellResult<OfflineStatus> {
    check_kind(&pin.kind)?;
    let off = app.state::<Offline>();
    let ctx = off.ctx(&app)?;
    let ref_id = if pin.kind == "liked" {
        None
    } else {
        pin.ref_id
    };
    ctx.store()
        .remove_pin(&pin.kind, ref_id)
        .map_err(ShellError::internal)?;
    log::info!("offline: unpinned {} {:?}", pin.kind, ref_id);
    off.cancel.store(true, Ordering::Relaxed);
    off.poke();
    off.wake_downloads.notify_one();
    let s = off.status(&ctx)?;
    let _ = app.emit_to(MAIN, EVENT_STATE, s.clone());
    Ok(s)
}

/// «Удалить всё»: файлы, закреплённое и запас; запас вернётся со следующей пересборкой, если он включён.
#[tauri::command]
pub async fn offline_clear(app: AppHandle) -> ShellResult<OfflineStatus> {
    let off = app.state::<Offline>();
    let ctx = off.ctx(&app)?;
    off.cancel.store(true, Ordering::Relaxed);
    {
        let store = ctx.store();
        store.clear_pins().map_err(ShellError::internal)?;
        for f in store.files().map_err(ShellError::internal)? {
            #[cfg(target_os = "ios")]
            ios_bg::cancel(f.track_id);
            let _ = store.delete_file(f.track_id);
        }
    }
    let _ = std::fs::remove_dir_all(&ctx.files);
    let mut s = ctx.settings();
    // Запас не накачивается обратно сразу: человек хотел освободить место
    s.reserve_enabled = false;
    let _ = ctx.store().set_meta(
        "settings.offline",
        serde_json::to_string(&s).ok().as_deref(),
    );
    log::info!("offline: all downloads removed");
    off.poke();
    let status = off.status(&ctx)?;
    let _ = app.emit_to(MAIN, EVENT_STATE, status.clone());
    Ok(status)
}

/// Состояние загрузки всех треков на устройстве — для значков в списках.
#[tauri::command]
pub async fn offline_downloads(app: AppHandle) -> ShellResult<Vec<TrackDownload>> {
    let off = app.state::<Offline>();
    let ctx = off.ctx(&app)?;
    let active = lock(&off.active).clone();
    Ok(ctx
        .store()
        .files()
        .map_err(ShellError::internal)?
        .into_iter()
        .map(|f| {
            let (received, total) = active.get(&f.track_id).copied().unwrap_or((
                if f.state == FileState::Ready {
                    f.size_bytes
                } else {
                    f.received
                },
                f.size_bytes,
            ));
            TrackDownload {
                track_id: f.track_id,
                state: f.state.as_str(),
                received,
                total,
            }
        })
        .collect())
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalSource {
    pub duration_ms: Option<i64>,
    pub loudness_lufs: Option<f32>,
    pub true_peak_db: Option<f32>,
    pub waveform: Option<Vec<i16>>,
}

/// Скачанный трек для плеера: громкость и огибающая; `null` — файла нет.
#[tauri::command]
pub async fn offline_source(app: AppHandle, track_id: Uuid) -> ShellResult<Option<LocalSource>> {
    let off = app.state::<Offline>();
    let ctx = off.ctx(&app)?;
    let f = ctx
        .store()
        .ready_file(track_id)
        .map_err(ShellError::internal)?;
    Ok(f.filter(|f| ctx.abs(&f.path).exists())
        .map(|f| LocalSource {
            duration_ms: f.duration_ms,
            loudness_lufs: f.loudness_lufs,
            true_peak_db: f.true_peak_db,
            waveform: f.waveform,
        }))
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OfflineWaveTrack {
    pub track: Track,
    pub reason: String,
}

fn candidates(ctx: &Ctx) -> ShellResult<Vec<Candidate>> {
    Ok(ctx
        .store()
        .playable_features()
        .map_err(ShellError::internal)?
        .into_iter()
        .map(|f| Candidate {
            track_id: f.track_id,
            title: f.title,
            artist_ids: f.artist_ids,
            artist_names: f.artist_names,
            energy: f.energy,
            valence: f.valence,
            danceability: f.danceability,
            bpm: f.bpm,
            key_camelot: f.key_camelot,
            voice: f.voice,
            language: f.language,
            approachability: f.approachability,
            liked: f.liked,
            set_kind: f.set_kind,
            set_score: f.set_score,
            plays_30d: f.plays_30d,
            skips_30d: f.skips_30d,
            completed: f.completed,
            last_played: f.last_played,
            blocked: f.blocked,
        })
        .collect())
}

/// Следующие треки волны из скачанного. Новая сессия — по новому `session`.
#[tauri::command]
pub async fn offline_wave_next(
    app: AppHandle,
    session: String,
    settings: WaveSettings,
    count: usize,
    exclude: Vec<Uuid>,
) -> ShellResult<Vec<OfflineWaveTrack>> {
    let off = app.state::<Offline>();
    let ctx = off.ctx(&app)?;
    let cands = candidates(&ctx)?;
    let picks = {
        let mut waves = lock(&off.waves);
        let seed = Uuid::now_v7().as_u128() as u64;
        let s = waves.entry(session.clone()).or_insert_with(|| {
            log::info!(
                "offline: wave without network from {} downloaded tracks",
                cands.len()
            );
            WaveSession::new(settings.clone(), seed)
        });
        // Настройки сменились посреди сессии — подбор по новым, сыгранное не повторяется
        if s.settings != settings {
            s.settings = settings;
        }
        s.exclude(exclude);
        s.next(&cands, count.clamp(1, 30), Utc::now())
    };
    let ids: Vec<Uuid> = picks.iter().map(|p| p.track_id).collect();
    let tracks = ctx.store().tracks(&ids).map_err(ShellError::internal)?;
    Ok(picks
        .into_iter()
        .filter_map(|p| {
            Some(OfflineWaveTrack {
                track: tracks.iter().find(|t| t.id == p.track_id)?.clone(),
                reason: p.reason,
            })
        })
        .collect())
}

/// Реакция в волне без сети: пропуск, лайк, «не нравится», дослушан.
#[tauri::command]
pub async fn offline_wave_react(
    app: AppHandle,
    session: String,
    track_id: Uuid,
    kind: String,
    position_ms: i64,
) -> ShellResult<()> {
    let off = app.state::<Offline>();
    let ctx = off.ctx(&app)?;
    let reaction = match kind.as_str() {
        "like" | "more_like_this" => Reaction::Like,
        "dislike" => Reaction::Dislike,
        "skip" => Reaction::Skip { position_ms },
        "completed" => Reaction::Completed,
        _ => Reaction::NotNow,
    };
    let cand = candidates(&ctx)?
        .into_iter()
        .find(|c| c.track_id == track_id);
    if let Some(c) = cand
        && let Some(s) = lock(&off.waves).get_mut(&session)
    {
        s.react(&c, reaction);
    }
    Ok(())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Played {
    track_id: Uuid,
    played_ms: i64,
    duration_ms: i64,
    /// `completed` | `skipped` | `previous` | …
    end_reason: String,
    #[serde(default)]
    wave: bool,
}

/// Прослушивание — в историю устройства: волна без сети учитывает пропуски и отдых заслушанного.
#[tauri::command]
pub async fn offline_played(app: AppHandle, play: Played) -> ShellResult<()> {
    let off = app.state::<Offline>();
    let ctx = off.ctx(&app)?;
    if play.played_ms < 1000 {
        return Ok(());
    }
    let skipped = play.end_reason != "completed" && play.played_ms < play.duration_ms * 8 / 10;
    ctx.store()
        .record_play(&PlayRecord {
            track_id: play.track_id,
            at: Utc::now(),
            played_ms: play.played_ms,
            duration_ms: play.duration_ms,
            skipped,
            wave: play.wave,
        })
        .map_err(ShellError::internal)?;
    // Новое из запаса сыграно — набор пора освежить
    if ctx.store().played_fresh().unwrap_or(0) >= SET_PLAYED_FRESH {
        off.poke();
    }
    Ok(())
}
