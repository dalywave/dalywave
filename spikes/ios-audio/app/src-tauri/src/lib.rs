//! Оболочка спайка: тестовая страница + движок dw-audio + интеграция с системой (iOS, macOS).
//!
//! Сценарии для автоматической проверки запускаются переменной окружения `DW_SCENARIO`
//! (или аргументом `--scenario=…`): background, ttfa, selftest, crossfade.
//! Адрес тестового сервера — `DW_SERVER` (по умолчанию http://127.0.0.1:8420) или поле на странице.

mod applog;
mod stats;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use dw_audio::{ChangeReason, Engine, EngineConfig, Event, PlaybackState, TrackSpec, Transition};
use serde::Serialize;
use serde_json::{Value, json};
use tauri::{AppHandle, Emitter, Manager};

struct AppState {
    engine: Engine,
    server: Mutex<String>,
    manifest: Mutex<Option<Value>>,
    selftest_running: AtomicBool,
}

fn server_url(app: &AppHandle) -> String {
    app.state::<AppState>()
        .server
        .lock()
        .map(|s| s.clone())
        .unwrap_or_default()
}

fn http_get(url: &str) -> Result<Vec<u8>, String> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(10)))
        .build()
        .into();
    let mut resp = agent.get(url).call().map_err(|e| format!("{url}: {e}"))?;
    resp.body_mut()
        .with_config()
        .limit(32 * 1024 * 1024)
        .read_to_vec()
        .map_err(|e| format!("{url}: {e}"))
}

fn load_manifest(app: &AppHandle) -> Result<Value, String> {
    let url = format!("{}/manifest.json", server_url(app));
    let body = http_get(&url)?;
    let v: Value = serde_json::from_slice(&body).map_err(|e| e.to_string())?;
    if let Ok(mut m) = app.state::<AppState>().manifest.lock() {
        *m = Some(v.clone());
    }
    Ok(v)
}

fn manifest(app: &AppHandle) -> Result<Value, String> {
    if let Some(v) = app
        .state::<AppState>()
        .manifest
        .lock()
        .ok()
        .and_then(|m| m.clone())
    {
        return Ok(v);
    }
    load_manifest(app)
}

fn album_tracks(server: &str, album: &Value) -> Vec<TrackSpec> {
    let mut out = Vec::new();
    let title = album["title"].as_str().unwrap_or("");
    let artist = album["artist"].as_str().unwrap_or("");
    let cover = album["cover"].as_str().map(|c| format!("{server}/{c}"));
    for t in album["tracks"].as_array().into_iter().flatten() {
        let file = t["file"].as_str().unwrap_or("");
        let mut spec = TrackSpec::url(file.to_string(), format!("{server}/{file}"));
        spec.title = format!(
            "{} · {}",
            t["title"].as_str().unwrap_or(""),
            album["format"].as_str().unwrap_or("demo")
        );
        spec.artist = artist.to_string();
        spec.album = title.to_string();
        spec.artwork_url = cover.clone();
        spec.duration_ms = t["durationMs"].as_u64();
        spec.loudness_lufs = t["lufs"].as_f64().map(|v| v as f32);
        spec.peak_db = t["peakDb"].as_f64().map(|v| v as f32);
        out.push(spec);
    }
    out
}

/// Очередь для фоновой проверки: все наборы подряд (разные кодеки и частоты) и демо-пьесы.
fn long_queue(server: &str, m: &Value) -> Vec<TrackSpec> {
    let order = [
        "gapless-opus_mp4",
        "gapless-opus_ogg",
        "gapless-flac16_44",
        "gapless-flac24_96",
        "gapless-mp3_320",
        "gapless-aac_256",
        "demo",
    ];
    let albums = m["albums"].as_array().cloned().unwrap_or_default();
    order
        .iter()
        .filter_map(|id| albums.iter().find(|a| a["id"] == *id))
        .flat_map(|a| album_tracks(server, a))
        .collect()
}

// ---- команды страницы ----

#[tauri::command]
fn get_config(app: AppHandle) -> Value {
    let snap = app.state::<AppState>().engine.snapshot();
    json!({
        "server": server_url(&app),
        "platform": std::env::consts::OS,
        "arch": std::env::consts::ARCH,
        "deviceRate": snap.device_rate,
        "deviceChannels": snap.device_channels,
        "log": applog::path().map(|p| p.display().to_string()),
    })
}

/// Синхронные команды Tauri выполняются на главном потоке: всё, что ходит в сеть или ждёт
/// главный поток, уводим в пул блокирующих задач.
async fn blocking<T: Send + 'static>(
    f: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn set_server(app: AppHandle, url: String) -> Result<Value, String> {
    blocking(move || {
        let url = url.trim().trim_end_matches('/').to_string();
        if let Ok(mut s) = app.state::<AppState>().server.lock() {
            *s = url.clone();
        }
        applog::save_setting("server", &url);
        if let Ok(mut m) = app.state::<AppState>().manifest.lock() {
            *m = None;
        }
        load_manifest(&app)
    })
    .await
}

#[tauri::command]
async fn get_manifest(app: AppHandle) -> Result<Value, String> {
    blocking(move || manifest(&app)).await
}

#[tauri::command]
async fn load_album(app: AppHandle, album_id: String, autoplay: bool) -> Result<usize, String> {
    blocking(move || load_album_blocking(&app, album_id, autoplay)).await
}

fn load_album_blocking(app: &AppHandle, album_id: String, autoplay: bool) -> Result<usize, String> {
    let m = manifest(app)?;
    let server = server_url(app);
    let tracks = if album_id == "long" {
        long_queue(&server, &m)
    } else {
        let album = m["albums"]
            .as_array()
            .and_then(|a| a.iter().find(|x| x["id"] == album_id.as_str()))
            .ok_or("нет такого альбома")?;
        album_tracks(&server, album)
    };
    let n = tracks.len();
    log::info!("load queue '{album_id}': {n} tracks");
    app.state::<AppState>()
        .engine
        .load_queue(tracks, 0, autoplay);
    Ok(n)
}

#[tauri::command]
fn transport(app: AppHandle, action: String, value: Option<f64>) {
    let e = &app.state::<AppState>().engine;
    match action.as_str() {
        "play" => e.play(),
        "pause" => e.pause(),
        "toggle" => e.toggle(),
        "next" => e.next(),
        "previous" => e.previous(),
        "seek" => e.seek(value.unwrap_or(0.0).max(0.0) as u64),
        "volume" => e.set_volume(value.unwrap_or(1.0) as f32),
        "gapless" => e.set_transition(Transition::Gapless),
        "crossfade" => e.set_transition(Transition::Crossfade {
            ms: value.unwrap_or(6000.0) as u32,
        }),
        "stop" => e.stop(),
        _ => log::warn!("unknown action {action}"),
    }
    log::info!(
        "ui: {action} {}",
        value.map(|v| v.to_string()).unwrap_or_default()
    );
}

#[derive(Serialize)]
struct StatsDto {
    cpu_percent: f64,
    footprint_mb: f64,
    peak_footprint_mb: f64,
    callback_load_avg: f32,
    callback_load_max: f32,
    buffer_frames: u32,
    underruns: u64,
    device_rate: u32,
    state: String,
    index: Option<usize>,
    position_ms: u64,
    duration_ms: Option<u64>,
    uptime_s: f64,
}

fn collect_stats(app: &AppHandle) -> StatsDto {
    let s = app.state::<AppState>().engine.snapshot();
    let p = stats::sample();
    StatsDto {
        cpu_percent: p.cpu_percent,
        footprint_mb: p.footprint_mb,
        peak_footprint_mb: p.peak_footprint_mb,
        callback_load_avg: s.callback_load_avg,
        callback_load_max: s.callback_load_max,
        buffer_frames: s.buffer_frames,
        underruns: s.underruns,
        device_rate: s.device_rate,
        state: format!("{:?}", s.state),
        index: s.index,
        position_ms: s.position_ms,
        duration_ms: s.duration_ms,
        uptime_s: applog::uptime(),
    }
}

#[tauri::command]
fn get_stats(app: AppHandle) -> StatsDto {
    collect_stats(&app)
}

#[tauri::command]
fn get_log() -> Vec<String> {
    applog::tail(200)
}

#[tauri::command]
async fn now_playing(app: AppHandle) -> Result<String, String> {
    blocking(move || Ok(now_playing_text(&app))).await
}

#[cfg(target_vendor = "apple")]
fn now_playing_text(app: &AppHandle) -> String {
    let (tx, rx) = std::sync::mpsc::channel();
    let _ = app.run_on_main_thread(move || {
        let _ = tx.send(dw_audio::apple::now_playing_description());
    });
    rx.recv_timeout(Duration::from_secs(2))
        .unwrap_or_else(|_| "main thread busy".into())
}

#[cfg(not(target_vendor = "apple"))]
fn now_playing_text(_app: &AppHandle) -> String {
    "нет системной панели на этой платформе".into()
}

#[tauri::command]
fn simulate(kind: String) -> Result<(), String> {
    log::info!("simulate {kind}");
    #[cfg(target_os = "ios")]
    {
        match kind.as_str() {
            "interruption-begin" => dw_audio::apple::simulate_interruption(true, false),
            "interruption-end" => dw_audio::apple::simulate_interruption(false, true),
            "unplug" => dw_audio::apple::simulate_headphones_unplugged(),
            _ => return Err("неизвестно".into()),
        }
        Ok(())
    }
    #[cfg(not(target_os = "ios"))]
    {
        let _ = kind;
        Err("только на iOS".into())
    }
}

#[tauri::command]
fn run_selftest(app: AppHandle) -> Result<(), String> {
    let st = app.state::<AppState>();
    if st.selftest_running.swap(true, Ordering::SeqCst) {
        return Err("самопроверка уже идёт".into());
    }
    let app2 = app.clone();
    std::thread::spawn(move || {
        let res = selftest(&app2);
        if let Err(e) = res {
            log::error!("selftest: {e}");
            let _ = app2.emit("selftest", json!({ "error": e }));
        }
        app2.state::<AppState>()
            .selftest_running
            .store(false, Ordering::SeqCst);
    });
    Ok(())
}

/// Те же проверки стыков, что в тестах движка, но по HTTP и на этом устройстве.
fn selftest(app: &AppHandle) -> Result<(), String> {
    use dw_audio::SourceSpec;
    use dw_audio::testkit::{analyze_joins, compare_with_concat, decode_all, render_queue};
    let server = server_url(app);
    let m = manifest(app)?;
    let mut refs: Vec<(u32, Vec<f32>)> = Vec::new();
    for album in m["albums"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|a| a["kind"] == "gapless")
    {
        let name = album["format"].as_str().unwrap_or("?").to_string();
        let rate = album["sourceRate"].as_u64().unwrap_or(48000) as u32;
        let bounds: Vec<usize> = album["bounds"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|v| v.as_u64())
            .map(|v| v as usize)
            .collect();
        if !refs.iter().any(|(r, _)| *r == rate) {
            let d = decode_all(&SourceSpec::Url(format!("{server}/ref/ref_{rate}.wav")))
                .map_err(|e| e.to_string())?;
            refs.push((rate, d.samples));
        }
        let reference = &refs.iter().find(|(r, _)| *r == rate).ok_or("ref")?.1;
        let tracks = album_tracks(&server, album);
        let parts: Vec<SourceSpec> = tracks.iter().map(|t| t.source.clone()).collect();
        let t0 = Instant::now();
        let res = render_queue(tracks, rate, Transition::Gapless, Duration::from_secs(120))
            .map_err(|e| e.to_string())?;
        let render_ms = t0.elapsed().as_secs_f64() * 1000.0;
        let r = analyze_joins(&res.samples, reference, &bounds);
        let (diff, _, len) =
            compare_with_concat(&res.samples, &parts).map_err(|e| e.to_string())?;
        let line = format!(
            "{name}: vs concat {diff} diff, len {len:+}; {} [{render_ms:.0} ms]",
            r.summary()
        );
        log::info!("selftest {line}");
        let _ = app.emit(
            "selftest",
            json!({
                "format": name,
                "exact": diff == 0 && len == 0,
                "worstShift": r.worst_shift(),
                "zeroRun": r.max_zero_run,
                "frames": [r.got_frames, r.expected_frames],
                "renderMs": render_ms,
                "summary": line,
            }),
        );
    }
    let _ = app.emit("selftest", json!({ "done": true }));
    Ok(())
}

// ---- события движка → страница, журнал, «Сейчас играет» ----

fn forward_events(app: AppHandle, events: crossbeam_channel::Receiver<Event>) {
    #[cfg(target_vendor = "apple")]
    let mut np = NowPlayingState::default();
    let mut last_logged_s = u64::MAX;
    for ev in events.iter() {
        if let Event::FirstAudio { latency_ms, .. } = &ev
            && let Some(p) = TTFA_PROBE.lock().ok().and_then(|g| g.clone())
        {
            let _ = p.send(*latency_ms);
        }
        match &ev {
            Event::Position {
                index,
                position_ms,
                duration_ms,
            } => {
                let s = position_ms / 1000;
                if s != last_logged_s {
                    last_logged_s = s;
                    log::info!(
                        "pos #{index} {:.1}s / {}",
                        *position_ms as f64 / 1000.0,
                        duration_ms
                            .map(|d| format!("{:.1}s", d as f64 / 1000.0))
                            .unwrap_or("?".into())
                    );
                }
            }
            Event::TrackChanged {
                index,
                track,
                reason,
            } => log::info!("track #{index} '{}' ({:?})", track.title, reason),
            other => log::info!("event {other:?}"),
        }
        let payload = match &ev {
            Event::State(s) => json!({ "type": "state", "state": format!("{s:?}") }),
            Event::Position {
                index,
                position_ms,
                duration_ms,
            } => {
                json!({ "type": "position", "index": index, "positionMs": position_ms, "durationMs": duration_ms })
            }
            Event::TrackChanged {
                index,
                track,
                reason,
            } => json!({
                "type": "track", "index": index, "title": track.title, "artist": track.artist, "album": track.album,
                "artwork": track.artwork_url, "durationMs": track.duration_ms, "auto": *reason == ChangeReason::Auto,
            }),
            Event::Buffering(b) => json!({ "type": "buffering", "on": b }),
            Event::Error { index, message } => {
                json!({ "type": "error", "index": index, "message": message })
            }
            Event::QueueEnded => json!({ "type": "ended" }),
            Event::FirstAudio { index, latency_ms } => {
                json!({ "type": "firstAudio", "index": index, "latencyMs": latency_ms })
            }
            Event::TrackInfo { index, info } => {
                json!({ "type": "info", "index": index, "info": info })
            }
        };
        let _ = app.emit("engine", payload);
        #[cfg(target_vendor = "apple")]
        np.on_event(&app, &ev);
    }
}

#[cfg(target_vendor = "apple")]
#[derive(Default)]
struct NowPlayingState {
    info: dw_audio::apple::NowPlaying,
    artwork: Option<Arc<dw_audio::apple::Artwork>>,
    artwork_url: Option<String>,
    last_sync: Option<Instant>,
}

#[cfg(target_vendor = "apple")]
impl NowPlayingState {
    fn push(&mut self, app: &AppHandle) {
        let info = self.info.clone();
        let art = self.artwork.clone();
        self.last_sync = Some(Instant::now());
        let _ =
            app.run_on_main_thread(move || dw_audio::apple::set_now_playing(&info, art.as_deref()));
    }

    fn on_event(&mut self, app: &AppHandle, ev: &Event) {
        match ev {
            Event::TrackChanged { track, .. } => {
                self.info.title = track.title.clone();
                self.info.artist = track.artist.clone();
                self.info.album = track.album.clone();
                self.info.duration_s = track.duration_ms.map(|d| d as f64 / 1000.0);
                self.info.elapsed_s = 0.0;
                if self.artwork_url != track.artwork_url {
                    self.artwork_url = track.artwork_url.clone();
                    self.artwork = None;
                    if let Some(url) = track.artwork_url.clone() {
                        let app2 = app.clone();
                        std::thread::spawn(move || match http_get(&url) {
                            Ok(bytes) => {
                                let _ = app2.run_on_main_thread(move || {
                                    if let Some(a) = dw_audio::apple::Artwork::from_bytes(&bytes) {
                                        ARTWORK.lock().map(|mut g| *g = Some(Arc::new(a))).ok();
                                        log::info!("artwork ready ({} bytes)", bytes.len());
                                    }
                                });
                            }
                            Err(e) => log::warn!("artwork: {e}"),
                        });
                    }
                }
                self.push(app);
            }
            Event::State(s) => {
                self.info.rate = if *s == PlaybackState::Playing {
                    1.0
                } else {
                    0.0
                };
                self.push(app);
            }
            Event::Position {
                position_ms,
                duration_ms,
                ..
            } => {
                self.info.elapsed_s = *position_ms as f64 / 1000.0;
                if let Some(d) = duration_ms {
                    self.info.duration_s = Some(*d as f64 / 1000.0);
                }
                // Обложка догрузилась — подхватываем.
                if self.artwork.is_none()
                    && let Some(a) = ARTWORK.lock().ok().and_then(|g| g.clone())
                {
                    self.artwork = Some(a);
                    self.push(app);
                    return;
                }
                // Система сама ведёт время по rate; сверяемся раз в 5 с и после перемотки.
                if self
                    .last_sync
                    .is_none_or(|t| t.elapsed() > Duration::from_secs(5))
                {
                    self.push(app);
                }
            }
            _ => {}
        }
    }
}

#[cfg(target_vendor = "apple")]
static ARTWORK: Mutex<Option<Arc<dw_audio::apple::Artwork>>> = Mutex::new(None);

// ---- интеграция с системой ----

#[cfg(target_vendor = "apple")]
fn setup_apple(app: &AppHandle, engine: &Engine) {
    use dw_audio::apple::{RemoteCommand, register_remote_commands};
    let e = engine.clone();
    let cmds = register_remote_commands(move |c| {
        log::info!("remote command: {c:?}");
        match c {
            RemoteCommand::Play => e.play(),
            RemoteCommand::Pause => e.pause(),
            RemoteCommand::Toggle => e.toggle(),
            RemoteCommand::Next => e.next(),
            RemoteCommand::Previous => e.previous(),
            RemoteCommand::Seek(t) => e.seek((t * 1000.0) as u64),
        }
    });
    // Регистрации живут всё время работы приложения.
    std::mem::forget(cmds);

    #[cfg(target_os = "ios")]
    {
        use dw_audio::apple::{SessionEvent, observe_app_lifecycle, observe_session};
        let e = engine.clone();
        let was_playing = Arc::new(AtomicBool::new(false));
        let obs = observe_session(move |ev| {
            log::info!("session: {ev:?}");
            match ev {
                SessionEvent::InterruptionBegan => {
                    was_playing.store(
                        e.snapshot().state == PlaybackState::Playing,
                        Ordering::SeqCst,
                    );
                    e.pause();
                }
                SessionEvent::InterruptionEnded { should_resume } => {
                    if should_resume && was_playing.swap(false, Ordering::SeqCst) {
                        if let Err(err) = dw_audio::apple::activate_audio_session() {
                            log::warn!("reactivate session: {err}");
                        }
                        e.play();
                    }
                }
                SessionEvent::HeadphonesUnplugged => e.pause(),
            }
        });
        std::mem::forget(obs);
        let lifecycle = observe_app_lifecycle(|background| {
            log::info!(
                "app {}",
                if background {
                    "entered BACKGROUND"
                } else {
                    "entered FOREGROUND"
                }
            )
        });
        std::mem::forget(lifecycle);
    }
    let _ = app;
}

// ---- сценарии ----

fn scenario_name() -> Option<String> {
    std::env::var("DW_SCENARIO")
        .ok()
        .or_else(|| {
            std::env::args().find_map(|a| a.strip_prefix("--scenario=").map(str::to_string))
        })
        .filter(|s| !s.is_empty())
}

fn run_scenario(app: AppHandle, name: String) {
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(2));
        log::info!("scenario '{name}' start");
        let m = match manifest(&app) {
            Ok(m) => m,
            Err(e) => {
                log::error!("scenario: manifest: {e}");
                return;
            }
        };
        let server = server_url(&app);
        let engine = app.state::<AppState>().engine.clone();
        match name.as_str() {
            "background" | "crossfade" => {
                if name == "crossfade" {
                    engine.set_transition(Transition::Crossfade { ms: 4000 });
                }
                let q = if name == "crossfade" {
                    m["albums"]
                        .as_array()
                        .and_then(|a| a.iter().find(|x| x["id"] == "demo"))
                        .map(|a| album_tracks(&server, a))
                        .unwrap_or_default()
                } else {
                    long_queue(&server, &m)
                };
                log::info!("scenario: queue of {} tracks", q.len());
                engine.load_queue(q, 0, true);
                loop {
                    std::thread::sleep(Duration::from_secs(10));
                    let s = collect_stats(&app);
                    log::info!(
                        "stats: cpu {:.1}% footprint {:.1} MB (peak {:.1}) cb-load avg {:.3}% max {:.3}% buf {} underruns {} state {} #{:?} {:.1}s",
                        s.cpu_percent,
                        s.footprint_mb,
                        s.peak_footprint_mb,
                        s.callback_load_avg * 100.0,
                        s.callback_load_max * 100.0,
                        s.buffer_frames,
                        s.underruns,
                        s.state,
                        s.index,
                        s.position_ms as f64 / 1000.0
                    );
                    log::info!("nowplaying: {}", now_playing_text(&app));
                }
            }
            "ttfa" => {
                let order = [
                    "gapless-opus_mp4",
                    "gapless-opus_ogg",
                    "gapless-flac16_44",
                    "gapless-flac24_96",
                    "gapless-mp3_320",
                    "gapless-aac_256",
                ];
                let (tx, rx) = crossbeam_channel::unbounded::<f64>();
                *TTFA_PROBE.lock().unwrap_or_else(|p| p.into_inner()) = Some(tx);
                for round in 0..3 {
                    for id in order {
                        let Some(album) = m["albums"]
                            .as_array()
                            .and_then(|a| a.iter().find(|x| x["id"] == id))
                        else {
                            continue;
                        };
                        // Второй трек, чтобы не попадать в уже скачанный кэш.
                        let tracks: Vec<TrackSpec> = album_tracks(&server, album)
                            .into_iter()
                            .skip(1 + round)
                            .take(1)
                            .collect();
                        while rx.try_recv().is_ok() {}
                        engine.load_queue(tracks, 0, true);
                        match rx.recv_timeout(Duration::from_secs(10)) {
                            Ok(ms) => log::info!("ttfa {id} round {round}: {ms:.1} ms"),
                            Err(_) => log::warn!("ttfa {id} round {round}: timeout"),
                        }
                        std::thread::sleep(Duration::from_millis(1500));
                    }
                }
                engine.stop();
                log::info!("scenario ttfa done");
            }
            "selftest" => {
                if let Err(e) = selftest(&app) {
                    log::error!("selftest: {e}");
                }
                log::info!("scenario selftest done");
            }
            #[cfg(target_os = "ios")]
            "session" => {
                // Те же уведомления, что шлёт система при звонке и отключении наушников.
                use dw_audio::apple::{simulate_headphones_unplugged, simulate_interruption};
                let state = |label: &str| {
                    let s = engine.snapshot();
                    log::info!(
                        "session check [{label}]: state {:?}, #{:?} {:.1}s",
                        s.state,
                        s.index,
                        s.position_ms as f64 / 1000.0
                    );
                };
                engine.load_queue(long_queue(&server, &m), 0, true);
                std::thread::sleep(Duration::from_secs(6));
                state("playing before call");
                simulate_interruption(true, false);
                std::thread::sleep(Duration::from_secs(3));
                state("call began → expect Paused");
                simulate_interruption(false, true);
                std::thread::sleep(Duration::from_secs(3));
                state("call ended with shouldResume → expect Playing");
                simulate_headphones_unplugged();
                std::thread::sleep(Duration::from_secs(3));
                state("headphones unplugged → expect Paused");
                engine.play();
                std::thread::sleep(Duration::from_secs(3));
                state("user pressed play → expect Playing");
                simulate_interruption(true, false);
                std::thread::sleep(Duration::from_secs(2));
                simulate_interruption(false, false);
                std::thread::sleep(Duration::from_secs(3));
                state("call ended without shouldResume → expect Paused");
                engine.play();
                std::thread::sleep(Duration::from_secs(3));
                state("play again → expect Playing");
                // Быстрое возобновление: система остановила поток вывода, наш play через 0,5 с
                // должен его перезапустить (а не только снять паузу микшера).
                simulate_interruption(true, false);
                std::thread::sleep(Duration::from_millis(500));
                simulate_interruption(false, false);
                std::thread::sleep(Duration::from_millis(500));
                let before = engine.snapshot().output_frames;
                engine.play();
                std::thread::sleep(Duration::from_secs(2));
                let s = engine.snapshot();
                let ms =
                    s.output_frames.saturating_sub(before) * 1000 / s.device_rate.max(1) as u64;
                log::info!(
                    "session check [quick resume]: output advanced {ms} ms in 2 s (expect ~2000), state {:?}",
                    s.state
                );
                log::info!("scenario session done");
            }
            other => log::warn!("unknown scenario {other}"),
        }
    });
}

static TTFA_PROBE: Mutex<Option<crossbeam_channel::Sender<f64>>> = Mutex::new(None);

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let handle = app.handle().clone();
            let data_dir = app.path().app_data_dir().ok();
            applog::init(data_dir.clone());
            log::info!(
                "DalyWave spike start: {} {} (pid {})",
                std::env::consts::OS,
                std::env::consts::ARCH,
                std::process::id()
            );

            #[cfg(target_os = "ios")]
            match dw_audio::apple::activate_audio_session() {
                Ok((rate, io)) => log::info!(
                    "AVAudioSession playback active: {rate} Hz, IO buffer {:.1} ms",
                    io * 1000.0
                ),
                Err(e) => log::error!("AVAudioSession: {e}"),
            }

            let server = std::env::var("DW_SERVER")
                .ok()
                .or_else(|| applog::load_setting("server"))
                .unwrap_or_else(|| "http://127.0.0.1:8420".into());
            let volume = std::env::var("DW_VOLUME")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(1.0);
            let cfg = EngineConfig {
                volume,
                ..EngineConfig::default()
            };
            let (engine, events) = Engine::new(cfg).map_err(|e| e.to_string())?;
            log::info!(
                "engine: device {} Hz, volume {volume}, server {server}",
                engine.snapshot().device_rate
            );

            #[cfg(target_vendor = "apple")]
            setup_apple(&handle, &engine);

            app.manage(AppState {
                engine,
                server: Mutex::new(server),
                manifest: Mutex::new(None),
                selftest_running: AtomicBool::new(false),
            });

            let h2 = handle.clone();
            std::thread::Builder::new()
                .name("dw-events".into())
                .spawn(move || forward_events(h2, events))
                .map_err(|e| e.to_string())?;
            if let Some(name) = scenario_name() {
                run_scenario(handle, name);
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_config,
            set_server,
            get_manifest,
            load_album,
            transport,
            get_stats,
            get_log,
            now_playing,
            simulate,
            run_selftest
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
