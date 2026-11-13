//! Звук приложения: аудиодвижок dalywave-audio (crates/client/audio) играет сам, без WebView.
//!
//! Интерфейс передаёт очередь — текущий трек и следующие, с готовыми ссылками или без них, —
//! команды и настройки; движок отвечает событиями `player:*`. Ссылку на файл, если её ещё нет
//! или она истекла посреди трека, движок просит у сервера сам (`GET /api/tracks/{id}/stream` с
//! токеном устройства), поэтому очередь доигрывается и в фоне на iPhone, когда WebView усыплён.
//! «Сейчас играет», медиаклавиши, экран блокировки и трей получают состояние отсюда же (media.rs).
//!
//! Движок создаётся в фоновом потоке: на iOS сначала активируется аудиосессия (до 200 мс), на
//! macOS первое открытие устройства — до 200 мс; главный поток это не задерживает.

use std::sync::{
    Mutex, OnceLock,
    atomic::{AtomicBool, Ordering},
};
use std::time::{Duration, Instant};

use crossbeam_channel::Receiver;
use dalywave_audio::{
    ChangeReason, EQ_BANDS, Engine, EngineConfig, EqPreset, EqSettings, Event, PlaybackState,
    Resolved, Resolver, SleepStatus, SleepTimer, SourceSpec, TrackSpec, Transition,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::{AppHandle, Emitter, Manager};

use crate::{
    MAIN, MINI,
    error::{ShellError, ShellResult},
    media::{self, Command, Media, RepeatMode, Source},
    settings::{Settings, StoredEq},
};

pub const EVENT_STATE: &str = "player:state";
pub const EVENT_POSITION: &str = "player:position";
pub const EVENT_TRACK: &str = "player:track";
pub const EVENT_BUFFERING: &str = "player:buffering";
pub const EVENT_ERROR: &str = "player:error";
pub const EVENT_ENDED: &str = "player:ended";
pub const EVENT_LEVEL: &str = "player:level";
pub const EVENT_SLEEP: &str = "player:sleep";
/// Системный вывод не отвечает (аудиосервер завис) или снова работает
pub const EVENT_SOUND: &str = "player:sound";
/// Мини-плеер: что играет (название, обложка, пауза) — после смены трека и паузы
pub const EVENT_NOW: &str = "player:now";

/// Громкость до первой команды интерфейса — как `dw.volume` в интерфейсе по умолчанию
const DEFAULT_VOLUME: f32 = 0.72;
/// Сколько команда ждёт движок, пока он создаётся при запуске
const INIT_WAIT: Duration = Duration::from_secs(10);
/// Позиция в журнале во время воспроизведения — не чаще
const LOG_POSITION_EVERY: Duration = Duration::from_secs(10);

pub struct Player {
    engine: OnceLock<Engine>,
    init_error: OnceLock<String>,
    /// `DW_VOLUME` — множитель громкости для проверок: 0 — движок работает, но без звука
    volume_cap: f32,
    ui_volume: Mutex<f32>,
    /// iOS: приложение в фоне, WebView усыплён — позиция и уровень ему не нужны
    background: AtomicBool,
    smoke: AtomicBool,
    /// Токен устройства для сервера: (адрес, токен) — чтобы не ходить в связку ключей на каждый трек
    token: Mutex<Option<(String, Option<String>)>>,
    http: reqwest::Client,
}

impl Player {
    fn new(app_version: &str) -> Self {
        let volume_cap = std::env::var("DW_VOLUME")
            .ok()
            .and_then(|v| v.parse::<f32>().ok())
            .map_or(1.0, |v| v.clamp(0.0, 1.0));
        let http = reqwest::Client::builder()
            .user_agent(format!("DalyWave/{app_version} ({})", crate::platform::OS))
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(20))
            .build()
            .unwrap_or_default();
        Self {
            engine: OnceLock::new(),
            init_error: OnceLock::new(),
            volume_cap,
            ui_volume: Mutex::new(DEFAULT_VOLUME),
            background: AtomicBool::new(false),
            smoke: AtomicBool::new(false),
            token: Mutex::new(None),
            http,
        }
    }

    pub fn engine(&self) -> Option<Engine> {
        self.engine.get().cloned()
    }

    fn engine_volume(&self) -> f32 {
        *self.ui_volume.lock().unwrap_or_else(|e| e.into_inner()) * self.volume_cap
    }

    /// Вход сменился или сменили сервер — токен перечитать из связки ключей.
    pub fn forget_token(&self) {
        *self.token.lock().unwrap_or_else(|e| e.into_inner()) = None;
    }

    fn token_for(&self, server: &str) -> Option<String> {
        if let Some((s, t)) = self.token.lock().unwrap_or_else(|e| e.into_inner()).clone()
            && s == server
        {
            return t;
        }
        let token = match crate::auth::load_token(server) {
            Ok(t) => t,
            Err(e) => {
                log::warn!("player: no device token: {e}");
                None
            }
        };
        *self.token.lock().unwrap_or_else(|e| e.into_inner()) =
            Some((server.to_string(), token.clone()));
        token
    }

    /// HTTP-клиент плеера и токен устройства для сервера (beat.rs берёт анализ трека так же, как движок).
    pub(crate) fn client_for(&self, server: &str) -> (reqwest::Client, Option<String>) {
        (self.http.clone(), self.token_for(server))
    }

    /// iOS: приложение в фоне (WebView усыплён) — Siri включает музыку сама, не через интерфейс.
    #[cfg_attr(not(target_os = "ios"), allow(dead_code))]
    pub fn in_background(&self) -> bool {
        self.background.load(Ordering::Relaxed)
    }

    /// iOS: приложение ушло в фон или вернулось.
    #[cfg_attr(not(target_os = "ios"), allow(dead_code))]
    pub fn set_background(&self, app: &AppHandle, background: bool) {
        self.background.store(background, Ordering::Relaxed);
        if !background && let Some(engine) = self.engine() {
            // WebView проснулся: свежее состояние сразу, без ожидания следующего события
            emit_snapshot(app, &engine);
        }
    }
}

/// Создать движок в фоновом потоке и подключить его к системе.
pub fn init(app: &AppHandle) {
    app.manage(Player::new(&app.package_info().version.to_string()));
    let app = app.clone();
    let spawned = std::thread::Builder::new()
        .name("dw-player-init".into())
        .spawn(move || start_engine(&app));
    if let Err(e) = spawned {
        log::error!("player: could not start the engine thread: {e}");
    }
}

fn start_engine(app: &AppHandle) {
    let player = app.state::<Player>();
    #[cfg(target_os = "ios")]
    match dalywave_audio::apple::activate_audio_session() {
        Ok((rate, io)) => log::info!(
            "audio session active: {rate} Hz, IO buffer {:.1} ms, route policy {}, spatial: {}",
            io * 1000.0,
            dalywave_audio::apple::route_sharing_policy(),
            dalywave_audio::apple::spatial_description()
        ),
        Err(e) => log::error!("audio session: {e}"),
    }
    let cfg = EngineConfig {
        // Компьютер: устройство вывода, выбранное в шторке «Устройства» (audio.rs)
        #[cfg(desktop)]
        output: dalywave_audio::OutputConfig::Device {
            buffer_frames: None,
            device: app.state::<Settings>().audio_output(),
        },
        volume: player.engine_volume(),
        user_agent: format!(
            "DalyWave/{} ({})",
            app.package_info().version,
            crate::platform::OS
        ),
        resolver: Some(resolver(app)),
        analysis: Some(analysis_source(app)),
        // Эквалайзер этого устройства — с первого трека, ещё до интерфейса
        eq: app.state::<Settings>().eq().as_ref().and_then(eq_settings),
        // AirPlay — буферизованный вывод (AirPlay 2); `DALYWAVE_OUTPUT=buffered` — он же без колонок
        buffered: std::env::var("DALYWAVE_OUTPUT")
            .map(|v| dalywave_audio::BufferedOutput::from_env_value(&v))
            .unwrap_or_default(),
        ..EngineConfig::default()
    };
    let started = Instant::now();
    let (engine, events) = match Engine::new(cfg) {
        Ok(e) => e,
        Err(e) => {
            log::error!("player: could not open the audio output: {e}");
            let _ = player.init_error.set(e.to_string());
            return;
        }
    };
    let snap = engine.snapshot();
    log::info!(
        "player: engine ready in {:.0} ms, output {} Hz {} ch{}{}",
        started.elapsed().as_secs_f64() * 1000.0,
        snap.device_rate,
        snap.device_channels,
        if snap.output_buffered {
            ", buffered (AirPlay 2)"
        } else {
            ""
        },
        if player.volume_cap < 1.0 {
            format!(", volume ×{} (DW_VOLUME)", player.volume_cap)
        } else {
            String::new()
        }
    );
    let _ = player.engine.set(engine.clone());
    let forward_app = app.clone();
    let _ = std::thread::Builder::new()
        .name("dw-player-events".into())
        .spawn(move || forward_events(&forward_app, events));
    // Пульт системы и уведомления аудиосессии регистрируются на главном потоке
    let main_app = app.clone();
    let _ = app.run_on_main_thread(move || media::attach_engine(&main_app));
    if let Ok(server) = std::env::var("DALYWAVE_SMOKE")
        && !server.is_empty()
    {
        smoke(
            app.clone(),
            engine,
            server.trim_end_matches('/').to_string(),
        );
    }
}

/// Движок для команды; при запуске подождать, пока он создаётся.
pub(crate) async fn engine(app: &AppHandle) -> ShellResult<Engine> {
    let player = app.state::<Player>();
    let deadline = Instant::now() + INIT_WAIT;
    loop {
        if let Some(e) = player.engine() {
            return Ok(e);
        }
        if let Some(e) = player.init_error.get() {
            return Err(ShellError::Audio(e.clone()));
        }
        if Instant::now() > deadline {
            return Err(ShellError::Audio("аудиодвижок не запустился".into()));
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
}

// ---- ссылки на файлы ----

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct StreamReady {
    url: String,
    #[serde(default)]
    loudness_lufs: Option<f32>,
    #[serde(default)]
    true_peak_db: Option<f32>,
    #[serde(default)]
    duration_ms: Option<u64>,
}

/// Ссылка на файл трека с сервера: для очереди без ссылок и когда ссылка истекла.
/// Вызывается в потоках движка — блокирует их, но не главный поток и не интерфейс.
fn resolver(app: &AppHandle) -> Resolver {
    let app = app.clone();
    Resolver::new(move |track: &TrackSpec| {
        let track_id = track.stream_id.clone().ok_or("у трека нет id на сервере")?;
        // Без сети ссылку не получить: не ждать таймаута, следующий трек — из скачанного
        if crate::offline::offline_now(&app) {
            return Err("нет сети".into());
        }
        let server = app.state::<Settings>().server_url();
        let player = app.state::<Player>();
        let token = player.token_for(&server);
        let http = player.http.clone();
        let started = Instant::now();
        let resolved = tauri::async_runtime::block_on(stream_link(
            &http,
            &server,
            &track_id,
            token.as_deref(),
        ));
        match &resolved {
            Ok(_) => log::info!(
                "player: link for track {track_id} in {:.0} ms",
                started.elapsed().as_secs_f64() * 1000.0
            ),
            Err(e) => log::warn!("player: no link for track {track_id}: {e}"),
        }
        resolved
    })
}

/// Анализ трека для «Диджея»: `GET /api/tracks/{id}/analysis`. Движок берёт его заранее — для
/// текущего и следующего трека; нет анализа (404) — переход будет «Умным».
fn analysis_source(app: &AppHandle) -> dalywave_audio::AnalysisSource {
    let app = app.clone();
    dalywave_audio::AnalysisSource::new(move |track: &TrackSpec| {
        let track_id = track.stream_id.clone().ok_or("у трека нет id на сервере")?;
        // Скачанный трек — с анализом рядом (умный офлайн): переходы по долям и без сети
        if let Some(a) = crate::offline::local_analysis(&app, &track_id)
            .and_then(|b| serde_json::from_slice::<dalywave_audio::Analysis>(&b).ok())
        {
            return Ok(Some(a));
        }
        if crate::offline::offline_now(&app) {
            return Ok(None);
        }
        let server = app.state::<Settings>().server_url();
        let player = app.state::<Player>();
        let token = player.token_for(&server);
        let http = player.http.clone();
        tauri::async_runtime::block_on(async move {
            let mut req = http.get(format!("{server}/api/tracks/{track_id}/analysis"));
            if let Some(token) = token {
                req = req.bearer_auth(token);
            }
            let res = req
                .send()
                .await
                .map_err(|e| format!("сервер недоступен: {e}"))?;
            match res.status().as_u16() {
                200 => res
                    .json::<dalywave_audio::Analysis>()
                    .await
                    .map(Some)
                    .map_err(|e| format!("неожиданный ответ сервера: {e}")),
                404 => Ok(None),
                status => Err(format!("сервер ответил {status}")),
            }
        })
    })
}

async fn stream_link(
    http: &reqwest::Client,
    server: &str,
    track_id: &str,
    token: Option<&str>,
) -> Result<Resolved, String> {
    // Качество — по сети устройства и настройкам слушателя: на сотовой с «Экономией» сервер даёт Opus 64
    let network = crate::offline::network_param();
    // 202 «preparing» — сервер делает копию файла своей музыки: она готова за пару секунд
    let mut tries = 0;
    let res = loop {
        let mut req = http.get(format!(
            "{server}/api/tracks/{track_id}/stream?network={network}"
        ));
        if let Some(token) = token {
            req = req.bearer_auth(token);
        }
        let res = req
            .send()
            .await
            .map_err(|e| format!("сервер недоступен: {e}"))?;
        let preparing = res.status().as_u16() == 202
            && res
                .headers()
                .get(reqwest::header::RETRY_AFTER)
                .is_some();
        if !preparing || tries >= 15 {
            break res;
        }
        tries += 1;
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    };
    match res.status().as_u16() {
        200 => {
            let body: StreamReady = res
                .json()
                .await
                .map_err(|e| format!("неожиданный ответ сервера: {e}"))?;
            let url = if body.url.starts_with('/') {
                format!("{server}{}", body.url)
            } else {
                body.url
            };
            Ok(Resolved {
                url,
                loudness_lufs: body.loudness_lufs,
                peak_db: body.true_peak_db,
                duration_ms: body.duration_ms,
            })
        }
        202 => Err("трек ещё скачивается на сервер".into()),
        401 => Err("нужен вход".into()),
        404 => Err("трека нет на сервере".into()),
        status => Err(format!("сервер ответил {status}")),
    }
}

// ---- события движка → интерфейс, журнал, система ----

fn state_name(s: PlaybackState) -> &'static str {
    match s {
        PlaybackState::Idle => "idle",
        PlaybackState::Loading => "loading",
        PlaybackState::Playing => "playing",
        PlaybackState::Paused => "paused",
        PlaybackState::Ended => "ended",
    }
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct StatePayload {
    state: &'static str,
    buffering: bool,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct PositionPayload {
    id: String,
    position_ms: u64,
    duration_ms: Option<u64>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct TrackPayload {
    id: String,
    index: usize,
    /// `auto` — движок перешёл сам (без пауз, кроссфейдом, пропустив сбойный трек);
    /// `user` — команда: загрузка, следующий, предыдущий (в том числе с пульта системы)
    reason: &'static str,
    duration_ms: Option<u64>,
}

#[derive(Clone, Serialize)]
struct BufferingPayload {
    buffering: bool,
}

/// Системный вывод не отвечает: интерфейс показывает «звук недоступен — повторяем»
#[derive(Clone, Serialize)]
struct SoundPayload {
    available: bool,
}

#[derive(Clone, Serialize)]
struct ErrorPayload {
    id: Option<String>,
    message: String,
}

#[derive(Clone, Serialize)]
struct LevelPayload {
    rms: f32,
    peak: f32,
    bands: [f32; 3],
}

fn emit<S: Serialize + Clone>(app: &AppHandle, event: &str, payload: S) {
    let _ = app.emit_to(MAIN, event, payload.clone());
    // Мини-плеер — ещё один вид того же плеера: те же события
    if app.get_webview_window(MINI).is_some() {
        let _ = app.emit_to(MINI, event, payload);
    }
}

#[cfg_attr(not(target_os = "ios"), allow(dead_code))]
fn emit_snapshot(app: &AppHandle, engine: &Engine) {
    let s = engine.snapshot();
    emit(
        app,
        EVENT_STATE,
        StatePayload {
            state: state_name(s.state),
            buffering: s.buffering,
        },
    );
    emit(
        app,
        EVENT_SOUND,
        SoundPayload {
            available: !s.output_stalled,
        },
    );
    if let Some(id) = s.id {
        emit(
            app,
            EVENT_POSITION,
            PositionPayload {
                id,
                position_ms: s.position_ms,
                duration_ms: s.duration_ms,
            },
        );
    }
}

fn forward_events(app: &AppHandle, events: Receiver<Event>) {
    let player = app.state::<Player>();
    let mut playing = false;
    let mut buffering = false;
    let mut last_position_log: Option<Instant> = None;
    let mut last_level_log: Option<Instant> = None;
    for ev in events.iter() {
        let background = player.background.load(Ordering::Relaxed);
        match &ev {
            Event::State(s) => {
                playing = *s == PlaybackState::Playing;
                log::info!("player: {}", state_name(*s));
                emit(
                    app,
                    EVENT_STATE,
                    StatePayload {
                        state: state_name(*s),
                        buffering,
                    },
                );
            }
            Event::Position {
                id,
                position_ms,
                duration_ms,
                ..
            } => {
                if playing && last_position_log.is_none_or(|t| t.elapsed() >= LOG_POSITION_EVERY) {
                    last_position_log = Some(Instant::now());
                    log::info!(
                        "player: {id} at {:.1} s of {}",
                        *position_ms as f64 / 1000.0,
                        duration_ms.map_or("?".into(), |d| format!("{:.1} s", d as f64 / 1000.0))
                    );
                }
                if !background {
                    emit(
                        app,
                        EVENT_POSITION,
                        PositionPayload {
                            id: id.clone(),
                            position_ms: *position_ms,
                            duration_ms: *duration_ms,
                        },
                    );
                }
            }
            Event::TrackChanged {
                index,
                track,
                reason,
            } => {
                let reason = match reason {
                    ChangeReason::Auto => "auto",
                    ChangeReason::User => "user",
                };
                log::info!(
                    "player: track #{index} {} «{}» ({reason})",
                    track.id,
                    track.title
                );
                emit(
                    app,
                    EVENT_TRACK,
                    TrackPayload {
                        id: track.id.clone(),
                        index: *index,
                        reason,
                        duration_ms: track.duration_ms,
                    },
                );
            }
            Event::Buffering(b) => {
                buffering = *b;
                log::debug!("player: buffering {b}");
                emit(app, EVENT_BUFFERING, BufferingPayload { buffering: *b });
            }
            Event::Error { id, message, .. } => {
                log::warn!(
                    "player: error on {}: {message}",
                    id.as_deref().unwrap_or("?")
                );
                emit(
                    app,
                    EVENT_ERROR,
                    ErrorPayload {
                        id: id.clone(),
                        message: message.clone(),
                    },
                );
            }
            Event::QueueEnded => {
                log::info!("player: queue ended");
                emit(app, EVENT_ENDED, Value::Null);
            }
            Event::FirstAudio { latency_ms, .. } => {
                log::info!("player: sound in {latency_ms:.0} ms after the command");
            }
            Event::TrackInfo { info, .. } => log::info!("player: {info}"),
            Event::Level(l) => {
                if player.smoke.load(Ordering::Relaxed)
                    && last_level_log.is_none_or(|t| t.elapsed() >= Duration::from_secs(2))
                {
                    last_level_log = Some(Instant::now());
                    log::info!(
                        "player: level rms {:.3} peak {:.3} bands [{:.3}, {:.3}, {:.3}]",
                        l.rms,
                        l.peak,
                        l.bands[0],
                        l.bands[1],
                        l.bands[2]
                    );
                }
                if !background {
                    emit(
                        app,
                        EVENT_LEVEL,
                        LevelPayload {
                            rms: l.rms,
                            peak: l.peak,
                            bands: l.bands,
                        },
                    );
                }
            }
            Event::TransitionPlanned {
                index,
                next_index,
                technique,
                note,
                ..
            } => log::info!("player: dj #{index} → #{next_index}: {technique} ({note})"),
            Event::Output {
                sample_rate,
                channels,
                reason,
            } => {
                log::info!("player: output {sample_rate} Hz {channels} ch ({reason})");
                // Устройство могло смениться: шторке «Устройства» — свежий список
                crate::audio::changed(app);
            }
            Event::Sleep(status) => {
                log::info!("player: sleep timer {status:?}");
                emit(app, EVENT_SLEEP, SleepPayload::from(*status));
            }
            Event::OutputStalled {
                attempt,
                retry_in_ms,
            } => {
                log::warn!(
                    "player: the system audio does not respond, attempt {attempt}, next in {retry_in_ms} ms"
                );
                emit(app, EVENT_SOUND, SoundPayload { available: false });
            }
            Event::OutputRecovered => {
                log::info!("player: the system audio responds again");
                emit(app, EVENT_SOUND, SoundPayload { available: true });
            }
        }
        media::on_player_event(app, &ev);
        #[cfg(desktop)]
        crate::discord::on_player_event(app, &ev);
        if matches!(
            ev,
            Event::TrackChanged { .. } | Event::State(_) | Event::QueueEnded
        ) && app.get_webview_window(MINI).is_some()
        {
            let _ = app.emit_to(MINI, EVENT_NOW, now_payload(app));
        }
    }
}

// ---- пульт: медиаклавиши, экран блокировки, наушники, трей ----

/// Команда с пульта системы или из трея. Что умеет движок, делает он сам (WebView может спать);
/// остальное — интерфейсу: «играть», когда очередь пуста (запустить волну), «следующий» в конце
/// очереди движка, «предыдущий» в начале — история очереди живёт в интерфейсе.
pub fn remote(app: &AppHandle, command: Command, source: Source) {
    let Some(engine) = app.state::<Player>().engine() else {
        media::forward(app, command, source);
        return;
    };
    let s = engine.snapshot();
    let loaded = s.queue_len > 0 && s.index.is_some();
    log::info!("remote {command:?} from {source:?}");
    match command {
        Command::Play if loaded => engine.play(),
        Command::Toggle if loaded => engine.toggle(),
        Command::Pause | Command::Stop => engine.pause(),
        // Повтор одного трека «следующий» и «предыдущий» пропускают — это движок учитывает сам
        Command::Next if s.has_next => engine.next(),
        Command::Previous if loaded && (s.has_previous || s.position_ms > 3000) => {
            engine.previous()
        }
        Command::SetRepeat { mode } => {
            // Повтор одного трека движок включает сразу (WebView может спать), режим — интерфейсу
            engine.set_repeat_one(mode == RepeatMode::One);
            media::forward(app, command, source);
        }
        Command::Seek { position_ms } if loaded => engine.seek(position_ms),
        Command::SeekBy { offset_ms } if loaded => {
            engine.seek(u64::try_from(s.position_ms as i64 + offset_ms).unwrap_or(0))
        }
        other => media::forward(app, other, source),
    }
}

// ---- команды интерфейса ----

/// Трек очереди от интерфейса.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayerItem {
    /// Ключ элемента очереди интерфейса — возвращается в событиях
    pub id: String,
    /// Трек на сервере: по нему движок сам просит ссылку, когда её нет или она истекла
    pub track_id: String,
    /// Подписанная ссылка на файл; null — движок спросит сервер, когда трек понадобится
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub loudness_lufs: Option<f32>,
    #[serde(default)]
    pub true_peak_db: Option<f32>,
    #[serde(default)]
    pub duration_ms: Option<u64>,
    pub title: String,
    pub artist: String,
    #[serde(default)]
    pub album: Option<String>,
    /// Обложка для «Сейчас играет»: http(s)
    #[serde(default)]
    pub artwork_url: Option<String>,
}

impl PlayerItem {
    fn into_spec(self) -> TrackSpec {
        let source = match self.url.filter(|u| !u.is_empty()) {
            Some(url) => SourceSpec::Url(url),
            None => SourceSpec::Deferred,
        };
        TrackSpec {
            id: self.id,
            source,
            stream_id: Some(self.track_id),
            title: self.title,
            artist: self.artist,
            album: self.album.unwrap_or_default(),
            artwork_url: self.artwork_url,
            duration_ms: self.duration_ms,
            loudness_lufs: self.loudness_lufs,
            peak_db: self.true_peak_db,
            analysis: None,
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(tag = "mode", rename_all = "camelCase")]
pub enum TransitionDto {
    Gapless,
    Crossfade {
        ms: u32,
    },
    /// «Диджей»: `expressiveness` 0…1 — от незаметного до эффектного
    Dj {
        #[serde(default = "default_expressiveness")]
        expressiveness: f32,
    },
}

fn default_expressiveness() -> f32 {
    0.6
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayerSnapshot {
    pub state: &'static str,
    pub id: Option<String>,
    pub position_ms: u64,
    pub duration_ms: Option<u64>,
    pub buffering: bool,
    pub queue_len: usize,
    pub device_rate: u32,
}

/// Новая очередь: `items[start]` начинает играть с `startMs`.
#[tauri::command]
pub async fn player_load(
    app: AppHandle,
    items: Vec<PlayerItem>,
    start: Option<usize>,
    start_ms: Option<u64>,
    autoplay: Option<bool>,
) -> ShellResult<()> {
    let engine = engine(&app).await?;
    let tracks = items
        .into_iter()
        .map(|i| crate::offline::prefer_local(&app, i.into_spec()))
        .collect();
    engine.load_queue_at(
        tracks,
        start.unwrap_or(0),
        start_ms.unwrap_or(0),
        autoplay.unwrap_or(true),
    );
    Ok(())
}

/// Что играть после текущего: движок готовит следующий трек заранее и сводит сам.
#[tauri::command]
pub async fn player_set_upcoming(app: AppHandle, items: Vec<PlayerItem>) -> ShellResult<()> {
    let engine = engine(&app).await?;
    engine.set_upcoming(
        items
            .into_iter()
            .map(|i| crate::offline::prefer_local(&app, i.into_spec()))
            .collect(),
    );
    Ok(())
}

#[tauri::command]
pub async fn player_play(app: AppHandle) -> ShellResult<()> {
    engine(&app).await?.play();
    Ok(())
}

#[tauri::command]
pub async fn player_pause(app: AppHandle) -> ShellResult<()> {
    engine(&app).await?.pause();
    Ok(())
}

#[tauri::command]
pub async fn player_toggle(app: AppHandle) -> ShellResult<()> {
    engine(&app).await?.toggle();
    Ok(())
}

#[tauri::command]
pub async fn player_seek(app: AppHandle, position_ms: u64) -> ShellResult<()> {
    engine(&app).await?.seek(position_ms);
    Ok(())
}

#[tauri::command]
pub async fn player_next(app: AppHandle) -> ShellResult<()> {
    engine(&app).await?.next();
    Ok(())
}

#[tauri::command]
pub async fn player_previous(app: AppHandle) -> ShellResult<()> {
    engine(&app).await?.previous();
    Ok(())
}

#[tauri::command]
pub async fn player_stop(app: AppHandle) -> ShellResult<()> {
    engine(&app).await?.stop();
    Ok(())
}

/// 0…1; с `DW_VOLUME` умножается на него.
#[tauri::command]
pub async fn player_set_volume(app: AppHandle, volume: f32) -> ShellResult<()> {
    let engine = engine(&app).await?;
    let player = app.state::<Player>();
    *player.ui_volume.lock().unwrap_or_else(|e| e.into_inner()) = volume.clamp(0.0, 1.0);
    engine.set_volume(player.engine_volume());
    Ok(())
}

#[tauri::command]
pub async fn player_set_transition(app: AppHandle, transition: TransitionDto) -> ShellResult<()> {
    let engine = engine(&app).await?;
    engine.set_transition(match transition {
        TransitionDto::Gapless => Transition::Gapless,
        TransitionDto::Crossfade { ms } => Transition::Crossfade {
            ms: ms.clamp(100, 30_000),
        },
        TransitionDto::Dj { expressiveness } => Transition::Dj {
            expressiveness: (expressiveness.clamp(0.0, 1.0) * 100.0).round() as u8,
        },
    });
    Ok(())
}

/// Выравнивание громкости по LUFS.
#[tauri::command]
pub async fn player_set_normalize(app: AppHandle, enabled: bool) -> ShellResult<()> {
    engine(&app).await?.set_normalize(enabled);
    Ok(())
}

/// События `player:level` (~20 в секунду) — только пока волна их рисует.
#[tauri::command]
pub async fn player_set_levels(app: AppHandle, enabled: bool) -> ShellResult<()> {
    engine(&app).await?.set_levels(enabled);
    Ok(())
}

#[tauri::command]
pub async fn player_state(app: AppHandle) -> ShellResult<PlayerSnapshot> {
    let s = engine(&app).await?.snapshot();
    Ok(PlayerSnapshot {
        state: state_name(s.state),
        id: s.id,
        position_ms: s.position_ms,
        duration_ms: s.duration_ms,
        buffering: s.buffering,
        queue_len: s.queue_len,
        device_rate: s.device_rate,
    })
}

// ---- эквалайзер и таймер сна ----

/// Эквалайзер от интерфейса: `preset` — `off`, `bass-boost`, `bass-cut`, `vocal`, `acoustic`,
/// `electronic`, `loudness` или `custom`; `gains` — десять полос «Своего», дБ.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EqDto {
    pub preset: String,
    #[serde(default)]
    pub gains: Vec<f32>,
}

/// Что играет движок: пресет — его таблица, «Свой» — полосы как есть, `off` — ничего.
fn eq_settings(eq: &StoredEq) -> Option<EqSettings> {
    if eq.preset == "custom" {
        let mut gains = [0.0; EQ_BANDS];
        for (g, v) in gains.iter_mut().zip(&eq.gains) {
            *g = *v;
        }
        return Some(EqSettings::new(gains)).filter(|s| !s.is_flat());
    }
    EqPreset::from_id(&eq.preset)
        .filter(|p| *p != EqPreset::Off)
        .map(EqSettings::preset)
}

/// Эквалайзер этого устройства: запоминается и действует сразу. null — выключить.
#[tauri::command]
pub async fn player_set_eq(app: AppHandle, eq: Option<EqDto>) -> ShellResult<()> {
    let stored = eq.map(|e| StoredEq {
        preset: e.preset,
        gains: e.gains.into_iter().take(EQ_BANDS).collect(),
    });
    if let Some(e) = &stored
        && e.preset != "custom"
        && EqPreset::from_id(&e.preset).is_none()
    {
        return Err(ShellError::Invalid(format!(
            "нет такого пресета эквалайзера: «{}»",
            e.preset
        )));
    }
    app.state::<Settings>().set_eq(stored.clone())?;
    engine(&app)
        .await?
        .set_eq(stored.as_ref().and_then(eq_settings));
    Ok(())
}

/// Сохранённый эквалайзер этого устройства; null — не настраивали (выключен).
#[tauri::command]
pub fn player_eq(app: AppHandle) -> Option<EqDto> {
    app.state::<Settings>().eq().map(|e| EqDto {
        preset: e.preset,
        gains: e.gains,
    })
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(tag = "mode", rename_all = "camelCase")]
pub enum SleepDto {
    /// Через `ms` миллисекунд
    Minutes { ms: u64 },
    /// В конце текущего трека
    Track,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SleepPayload {
    /// `minutes`, `track`; null — таймера нет
    timer: Option<&'static str>,
    /// Когда пауза, мс от эпохи Unix (для конца трека — по позиции)
    ends_at_ms: Option<u64>,
    remaining_ms: Option<u64>,
    fading: bool,
    /// Таймер сработал: музыка на паузе
    finished: bool,
}

impl From<SleepStatus> for SleepPayload {
    fn from(s: SleepStatus) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_millis() as u64);
        Self {
            timer: s.timer.map(|t| match t {
                SleepTimer::After { .. } => "minutes",
                SleepTimer::EndOfTrack => "track",
            }),
            ends_at_ms: s.remaining_ms.map(|r| now + r),
            remaining_ms: s.remaining_ms,
            fading: s.fading,
            finished: s.finished,
        }
    }
}

/// Таймер сна: последние секунды звук затухает, затем пауза. null — снять. Ход — `player:sleep`.
#[tauri::command]
pub async fn player_set_sleep(app: AppHandle, timer: Option<SleepDto>) -> ShellResult<()> {
    let timer = timer.map(|t| match t {
        SleepDto::Minutes { ms } => SleepTimer::After {
            ms: ms.clamp(1_000, 24 * 3_600_000),
        },
        SleepDto::Track => SleepTimer::EndOfTrack,
    });
    engine(&app).await?.set_sleep(timer);
    Ok(())
}

/// Перемешивание и повтор плеера интерфейса: повтор одного трека ведёт движок (и в фоне),
/// оба режима показываются в Пункте управления.
#[tauri::command]
pub async fn player_set_modes(
    app: AppHandle,
    shuffle: bool,
    repeat: RepeatMode,
) -> ShellResult<()> {
    engine(&app)
        .await?
        .set_repeat_one(repeat == RepeatMode::One);
    media::set_modes(&app, shuffle, repeat);
    Ok(())
}

/// Таймер сна сейчас (при запуске интерфейса: таймер мог стоять, пока окно было закрыто).
#[tauri::command]
pub async fn player_sleep(app: AppHandle) -> ShellResult<SleepPayload> {
    let s = engine(&app).await?.snapshot().sleep.unwrap_or(SleepStatus {
        timer: None,
        remaining_ms: None,
        fading: false,
        finished: false,
    });
    Ok(s.into())
}

// ---- мини-плеер: другой вид того же плеера ----

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NowPayload {
    title: String,
    artist: String,
    album: Option<String>,
    artwork_url: Option<String>,
    duration_ms: Option<u64>,
    position_ms: u64,
    playing: bool,
    /// Ключ элемента очереди: с ним сверяются события `player:position`
    id: Option<String>,
}

fn now_payload(app: &AppHandle) -> Option<NowPayload> {
    let now = app.state::<Media>().current()?;
    let snap = app.state::<Player>().engine().map(|e| e.snapshot());
    Some(NowPayload {
        title: now.title,
        artist: now.artist,
        album: now.album,
        artwork_url: now.artwork_url,
        duration_ms: snap
            .as_ref()
            .and_then(|s| s.duration_ms)
            .or(now.duration_ms),
        position_ms: snap.as_ref().map_or(now.position_ms, |s| s.position_ms),
        playing: now.playing,
        id: snap.and_then(|s| s.id),
    })
}

/// Что играет — для мини-плеера; null — ничего.
#[tauri::command]
pub fn player_now(app: AppHandle) -> Option<NowPayload> {
    now_payload(&app)
}

/// Кнопки мини-плеера — как пульт: что движок не может сам (следующий в конце его очереди,
/// предыдущий из истории), уходит плееру главного окна.
#[tauri::command]
pub fn player_remote(app: AppHandle, action: String) -> ShellResult<()> {
    let command = match action.as_str() {
        "toggle" => Command::Toggle,
        "play" => Command::Play,
        "pause" => Command::Pause,
        "next" => Command::Next,
        "previous" => Command::Previous,
        other => {
            return Err(ShellError::Invalid(format!(
                "неизвестное действие пульта: «{other}»"
            )));
        }
    };
    remote(&app, command, Source::Tray);
    Ok(())
}

// ---- проверка звука без входа ----

/// `DALYWAVE_SMOKE=<адрес тестового сервера>` (crates/client/audio/tools/serve.py): очередь из
/// его manifest.json играет сразу после запуска, в журнал — смены треков, позиция, уровень и
/// нагрузка. `DALYWAVE_SMOKE_ALBUM` — id набора из manifest.json (по умолчанию все подряд).
/// Громкость — `DW_VOLUME`; `DALYWAVE_SMOKE_MINI=1` — сразу открыть мини-плеер.
fn smoke(app: AppHandle, engine: Engine, server: String) {
    app.state::<Player>().smoke.store(true, Ordering::Relaxed);
    let spawned = std::thread::Builder::new()
        .name("dw-smoke".into())
        .spawn(move || {
            let http = app.state::<Player>().http.clone();
            let url = format!("{server}/manifest.json");
            let manifest = tauri::async_runtime::block_on(async move {
                http.get(url).send().await?.json::<Value>().await
            });
            let manifest = match manifest {
                Ok(m) => m,
                Err(e) => {
                    log::error!("smoke: no manifest at {server}: {e}");
                    return;
                }
            };
            let album = std::env::var("DALYWAVE_SMOKE_ALBUM").ok();
            let tracks = smoke_queue(&server, &manifest, album.as_deref());
            log::info!("smoke: {} tracks from {server}", tracks.len());
            engine.set_levels(true);
            engine.load_queue(tracks, 0, true);
            // Проверка мини-плеера: `DALYWAVE_SMOKE_MINI=1` — окно открывается само
            #[cfg(desktop)]
            if std::env::var("DALYWAVE_SMOKE_MINI").is_ok_and(|v| v == "1") {
                let handle = app.clone();
                let _ = app.run_on_main_thread(move || {
                    if let Err(e) = crate::mini::open(&handle) {
                        log::warn!("smoke: mini player: {e}");
                    }
                });
            }
            loop {
                std::thread::sleep(Duration::from_secs(10));
                let s = engine.snapshot();
                log::info!(
                    "smoke: {} #{:?} {:.1} s, underruns {}, callback load {:.3}% (max {:.3}%), output {} Hz, reopened {}",
                    state_name(s.state),
                    s.index,
                    s.position_ms as f64 / 1000.0,
                    s.underruns,
                    s.callback_load_avg * 100.0,
                    s.callback_load_max * 100.0,
                    s.device_rate,
                    s.output_reopens
                );
                media::log_now_playing(&app);
            }
        });
    if let Err(e) = spawned {
        log::error!("smoke: {e}");
    }
}

fn smoke_queue(server: &str, manifest: &Value, album: Option<&str>) -> Vec<TrackSpec> {
    let albums = manifest["albums"].as_array().cloned().unwrap_or_default();
    let order = [
        "gapless-opus_mp4",
        "gapless-opus_ogg",
        "gapless-flac16_44",
        "gapless-flac24_96",
        "gapless-mp3_320",
        "gapless-aac_256",
        "demo",
    ];
    let wanted: Vec<&str> = match album {
        Some(a) => vec![a],
        None => order.to_vec(),
    };
    wanted
        .iter()
        .filter_map(|id| albums.iter().find(|a| a["id"] == *id))
        .flat_map(|a| {
            let cover = a["cover"].as_str().map(|c| format!("{server}/{c}"));
            a["tracks"]
                .as_array()
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .map(move |t| {
                    let file = t["file"].as_str().unwrap_or("").to_string();
                    let mut spec = TrackSpec::url(file.clone(), format!("{server}/{file}"));
                    spec.title = format!(
                        "{} · {}",
                        t["title"].as_str().unwrap_or(""),
                        a["format"].as_str().unwrap_or("demo")
                    );
                    spec.artist = a["artist"].as_str().unwrap_or("").to_string();
                    spec.album = a["title"].as_str().unwrap_or("").to_string();
                    spec.artwork_url = cover.clone();
                    spec.duration_ms = t["durationMs"].as_u64();
                    spec.loudness_lufs = t["lufs"].as_f64().map(|v| v as f32);
                    spec.peak_db = t["peakDb"].as_f64().map(|v| v as f32);
                    spec
                })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn item_without_url_is_resolved_later() {
        let item: PlayerItem = serde_json::from_value(serde_json::json!({
            "id": "uid-1", "trackId": "0193", "url": null, "title": "Float",
            "artist": "Call Me Karizma", "durationMs": 169000, "loudnessLufs": -9.5
        }))
        .unwrap();
        let spec = item.into_spec();
        assert_eq!(spec.source, SourceSpec::Deferred);
        assert_eq!(spec.stream_id.as_deref(), Some("0193"));
        assert_eq!(spec.loudness_lufs, Some(-9.5));
    }

    #[test]
    fn transition_json() {
        let t: TransitionDto =
            serde_json::from_value(serde_json::json!({ "mode": "crossfade", "ms": 6000 })).unwrap();
        assert!(matches!(t, TransitionDto::Crossfade { ms: 6000 }));
        let t: TransitionDto =
            serde_json::from_value(serde_json::json!({ "mode": "gapless" })).unwrap();
        assert!(matches!(t, TransitionDto::Gapless));
        let t: TransitionDto =
            serde_json::from_value(serde_json::json!({ "mode": "dj", "expressiveness": 0.25 }))
                .unwrap();
        assert!(matches!(t, TransitionDto::Dj { expressiveness } if expressiveness == 0.25));
    }

    #[test]
    fn eq_from_settings() {
        let off = StoredEq {
            preset: "off".into(),
            gains: vec![3.0; 10],
        };
        assert_eq!(eq_settings(&off), None);
        let vocal = StoredEq {
            preset: "vocal".into(),
            gains: vec![],
        };
        assert_eq!(
            eq_settings(&vocal),
            Some(EqSettings::preset(EqPreset::Vocal))
        );
        let custom = StoredEq {
            preset: "custom".into(),
            gains: vec![20.0, -3.0],
        };
        let s = eq_settings(&custom).unwrap();
        assert_eq!(s.gains_db[0], 12.0, "clamped");
        assert_eq!(s.gains_db[1], -3.0);
        assert_eq!(s.gains_db[9], 0.0);
        let flat = StoredEq {
            preset: "custom".into(),
            gains: vec![0.0; 10],
        };
        assert_eq!(eq_settings(&flat), None);
    }

    #[test]
    fn sleep_json() {
        let t: SleepDto =
            serde_json::from_value(serde_json::json!({ "mode": "minutes", "ms": 900000 })).unwrap();
        assert!(matches!(t, SleepDto::Minutes { ms: 900_000 }));
        let t: SleepDto = serde_json::from_value(serde_json::json!({ "mode": "track" })).unwrap();
        assert!(matches!(t, SleepDto::Track));
        let v = serde_json::to_value(SleepPayload::from(SleepStatus {
            timer: Some(SleepTimer::EndOfTrack),
            remaining_ms: Some(1000),
            fading: true,
            finished: false,
        }))
        .unwrap();
        assert_eq!(v["timer"], "track");
        assert_eq!(v["remainingMs"], 1000);
        assert_eq!(v["fading"], true);
        assert!(v["endsAtMs"].as_u64().unwrap() > 1000);
    }

    #[test]
    fn track_event_json() {
        let v = serde_json::to_value(TrackPayload {
            id: "uid".into(),
            index: 2,
            reason: "auto",
            duration_ms: Some(1000),
        })
        .unwrap();
        assert_eq!(
            v,
            serde_json::json!({ "id": "uid", "index": 2, "reason": "auto", "durationMs": 1000 })
        );
    }
}
