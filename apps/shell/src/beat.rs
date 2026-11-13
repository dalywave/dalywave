//! «Почувствуй бит» (iPhone): лёгкие удары Taptic Engine на сильных долях того, что играет.
//!
//! Сетка — из анализа трека (`GET /api/tracks/{id}/analysis`, тот же, что у «Диджея»; у скачанного —
//! из локальной базы): сильные доли (`downbeatsMs`), а без них — каждая четвёртая доля. Сила удара —
//! от энергии трека в эту секунду (`energyCurve`).
//!
//! Время — по позиции движка: позиция — это то, что ушло в вывод, а слышно оно позже на задержку
//! маршрута (у AirPods — до 0,2 с), её Swift берёт у аудиосессии. Удар ставится в Core Haptics с
//! точным временем за 50 мс до доли (`DalyWaveBeat.tap`, `gen/apple/Sources/dalywave-shell/DalyWaveBeat.swift`),
//! поэтому опоздание потока здесь не важно. Пауза, буферизация, перемотка и смена трека снимают
//! поставленное.
//!
//! Только на экране: в фоне iOS вибрацию не даёт — поток засыпает, пока приложение не вернётся.
//! Выключатель хранится в интерфейсе (localStorage) и приходит сюда при запуске (`beat_set`).
#![cfg_attr(not(target_os = "ios"), allow(dead_code))]

use std::collections::{HashSet, VecDeque};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use dalywave_audio::{Analysis, PlaybackState};
use serde::Serialize;
use tauri::{AppHandle, Manager};

use crate::{error::ShellResult, player::Player, settings::Settings};

/// За столько до доли удар ставится в Core Haptics.
const LOOKAHEAD_MS: f64 = 50.0;
/// Мотор отзывается с задержкой — удар чуть раньше доли.
const MOTOR_LEAD_MS: f64 = 12.0;
/// Позиция ушла дальше ожидаемого — перемотка: поставленное снимается.
const JUMP_MS: f64 = 150.0;
/// Задержку маршрута перечитывать (наушники подключили, AirPlay выбрали).
const LATENCY_EVERY: Duration = Duration::from_secs(4);
/// Сеток в памяти: текущий трек, следующий и несколько недавних.
const GRIDS_MAX: usize = 8;

/// Сильные доли трека и сила удара на каждой.
#[derive(Debug, Clone, PartialEq)]
pub struct Pulses {
    at_ms: Vec<f64>,
    strength: Vec<f32>,
}

impl Pulses {
    /// Сетка из анализа; слишком короткой или пустой — нет.
    pub fn from_analysis(a: &Analysis) -> Option<Self> {
        let mut at: Vec<f64> = if a.downbeats_ms.len() >= 4 {
            a.downbeats_ms.clone()
        } else if a.beats_ms.len() >= 16 {
            a.beats_ms.iter().step_by(4).copied().collect()
        } else {
            return None;
        };
        at.retain(|t| t.is_finite() && *t >= 0.0);
        at.sort_by(f64::total_cmp);
        at.dedup_by(|a, b| (*a - *b).abs() < 1.0);
        if at.len() < 4 {
            return None;
        }
        let max = a
            .energy_curve
            .iter()
            .copied()
            .filter(|e| e.is_finite())
            .fold(0.0_f64, f64::max);
        let strength = at
            .iter()
            .map(|t| {
                let e = a
                    .energy_curve
                    .get((t / 1000.0) as usize)
                    .copied()
                    .filter(|_| max > 0.0)
                    .map_or(0.5, |e| (e / max).clamp(0.0, 1.0));
                (0.4 + 0.6 * e) as f32
            })
            .collect();
        Some(Self {
            at_ms: at,
            strength,
        })
    }

    /// Ближайшая доля строго после `after_ms`: время и сила.
    pub fn next(&self, after_ms: f64) -> Option<(f64, f32)> {
        let i = self.at_ms.partition_point(|t| *t <= after_ms);
        Some((*self.at_ms.get(i)?, *self.strength.get(i)?))
    }
}

#[derive(Default)]
struct Grids {
    /// (id трека на сервере, сетка; None — анализа нет)
    items: VecDeque<(String, Option<Arc<Pulses>>)>,
    fetching: HashSet<String>,
}

#[derive(Default)]
pub struct Beat {
    enabled: AtomicBool,
    started: AtomicBool,
    grids: Mutex<Grids>,
    wake: (Mutex<()>, Condvar),
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BeatState {
    /// Есть Taptic Engine (iPhone); на iPad, компьютерах и в симуляторе — нет
    supported: bool,
    enabled: bool,
}

pub fn init(app: &AppHandle) {
    app.manage(Beat::default());
}

impl Beat {
    fn wait(&self, timeout: Duration) {
        let guard = self.wake.0.lock().unwrap_or_else(|e| e.into_inner());
        let _ = self.wake.1.wait_timeout(guard, timeout);
    }

    fn notify(&self) {
        self.wake.1.notify_all();
    }

    /// Сетка трека: есть — сразу; нет в памяти — взять в фоне (пока — None).
    fn grid(&self, app: &AppHandle, track_id: &str) -> Option<Option<Arc<Pulses>>> {
        let mut g = self.grids.lock().unwrap_or_else(|e| e.into_inner());
        if let Some((_, p)) = g.items.iter().find(|(id, _)| id == track_id) {
            return Some(p.clone());
        }
        if g.fetching.insert(track_id.to_string()) {
            let app = app.clone();
            let id = track_id.to_string();
            tauri::async_runtime::spawn(async move {
                let pulses = fetch(&app, &id).await;
                let beat = app.state::<Beat>();
                let mut g = beat.grids.lock().unwrap_or_else(|e| e.into_inner());
                g.fetching.remove(&id);
                // Сбой сети — не «анализа нет»: спросим снова в следующий раз
                if let Ok(p) = pulses {
                    g.items.push_back((id, p.map(Arc::new)));
                    while g.items.len() > GRIDS_MAX {
                        g.items.pop_front();
                    }
                }
                drop(g);
                beat.notify();
            });
        }
        None
    }
}

/// Анализ трека → сетка. `Ok(None)` — анализа нет (404) или сетка не годится.
async fn fetch(app: &AppHandle, track_id: &str) -> Result<Option<Pulses>, String> {
    if let Some(a) = crate::offline::local_analysis(app, track_id)
        .and_then(|b| serde_json::from_slice::<Analysis>(&b).ok())
    {
        return Ok(Pulses::from_analysis(&a));
    }
    if crate::offline::offline_now(app) {
        return Err("нет сети".into());
    }
    let server = app.state::<Settings>().server_url();
    let (http, token) = app.state::<Player>().client_for(&server);
    let mut req = http.get(format!("{server}/api/tracks/{track_id}/analysis"));
    if let Some(token) = token {
        req = req.bearer_auth(token);
    }
    let res = req.send().await.map_err(|e| e.to_string())?;
    match res.status().as_u16() {
        200 => {
            let a = res.json::<Analysis>().await.map_err(|e| e.to_string())?;
            Ok(Pulses::from_analysis(&a))
        }
        404 => Ok(None),
        s => Err(format!("сервер ответил {s}")),
    }
}

fn state(app: &AppHandle) -> BeatState {
    let enabled = app
        .try_state::<Beat>()
        .is_some_and(|b| b.enabled.load(Ordering::Relaxed));
    BeatState {
        supported: swift::supported(),
        enabled,
    }
}

#[tauri::command]
pub fn beat_state(app: AppHandle) -> BeatState {
    state(&app)
}

/// Включить или выключить. Без Taptic Engine — ничего (ответ скажет `supported: false`).
#[tauri::command]
pub fn beat_set(app: AppHandle, enabled: bool) -> BeatState {
    if let Some(beat) = app.try_state::<Beat>() {
        let enabled = enabled && swift::supported();
        beat.enabled.store(enabled, Ordering::Relaxed);
        if enabled && !beat.started.swap(true, Ordering::Relaxed) {
            let app = app.clone();
            let _ = std::thread::Builder::new()
                .name("dw-beat".into())
                .spawn(move || run(&app));
        }
        beat.notify();
    }
    state(&app)
}

/// Есть ли у трека сетка долей: без неё выключатель недоступен.
#[tauri::command]
pub async fn beat_check(app: AppHandle, track_id: String) -> ShellResult<bool> {
    if let Some(beat) = app.try_state::<Beat>()
        && let Some(p) = beat.grid(&app, &track_id)
    {
        return Ok(p.is_some());
    }
    Ok(fetch(&app, &track_id).await.ok().flatten().is_some())
}

/// Поток ударов: спит, пока выключено, в фоне или на паузе.
fn run(app: &AppHandle) {
    let beat = app.state::<Beat>();
    let player = app.state::<Player>();
    let mut armed = false;
    let mut latency_ms = 0.0;
    let mut latency_at: Option<Instant> = None;
    // Что слышно и когда это было — чтобы заметить перемотку
    let mut last: Option<(String, f64, Instant)> = None;
    let mut fired: Option<(String, f64)> = None;
    loop {
        let active = beat.enabled.load(Ordering::Relaxed) && !player.in_background();
        let snap = player.engine().map(|e| e.snapshot());
        let playing = snap
            .as_ref()
            .is_some_and(|s| s.state == PlaybackState::Playing && !s.buffering);
        let (Some(snap), true, true) = (snap, active, playing) else {
            if armed {
                swift::stop();
                armed = false;
            }
            last = None;
            fired = None;
            beat.wait(Duration::from_millis(if active { 150 } else { 2000 }));
            continue;
        };
        let Some(track) = snap.stream_id.clone() else {
            beat.wait(Duration::from_millis(250));
            continue;
        };
        let pulses = match beat.grid(app, &track) {
            Some(Some(p)) => p,
            // Анализа нет — ждём следующего трека; берётся — ждём сетку
            Some(None) | None => {
                beat.wait(Duration::from_millis(250));
                continue;
            }
        };
        if !armed {
            swift::start();
            armed = true;
            latency_at = None;
        }
        if latency_at.is_none_or(|t| t.elapsed() > LATENCY_EVERY) {
            latency_ms = swift::output_latency_ms();
            latency_at = Some(Instant::now());
        }
        // AirPlay: позиция движка и так — то, что слышно
        let lag = if snap.output_buffered {
            0.0
        } else {
            latency_ms
        };
        let heard = snap.position_ms as f64 - lag + MOTOR_LEAD_MS;
        if let Some((t, at, when)) = &last {
            let expected = at + when.elapsed().as_secs_f64() * 1000.0;
            if t != &track || (heard - expected).abs() > JUMP_MS {
                swift::reset();
                fired = None;
            }
        }
        last = Some((track.clone(), heard, Instant::now()));
        let Some((at, strength)) = pulses.next(heard) else {
            beat.wait(Duration::from_millis(300));
            continue;
        };
        let wait = at - heard;
        if wait > LOOKAHEAD_MS {
            std::thread::sleep(Duration::from_secs_f64(
                ((wait - LOOKAHEAD_MS + 10.0).min(120.0)) / 1000.0,
            ));
            continue;
        }
        if fired.as_ref() != Some(&(track.clone(), at)) {
            let s = f64::from(strength);
            swift::tap(s, 0.35 + 0.4 * s, wait.max(0.0) / 1000.0);
            fired = Some((track, at));
        }
        std::thread::sleep(Duration::from_secs_f64((wait.max(0.0) + 10.0) / 1000.0));
    }
}

#[cfg(target_os = "ios")]
mod swift {
    use objc2::msg_send;
    use objc2::rc::autoreleasepool;
    use objc2::runtime::AnyClass;

    fn class() -> Option<&'static AnyClass> {
        AnyClass::get(c"DalyWaveBeat")
    }

    pub fn supported() -> bool {
        let Some(class) = class() else { return false };
        // SAFETY: `+[DalyWaveBeat supported]` — BOOL, любой поток
        autoreleasepool(|_| unsafe { msg_send![class, supported] })
    }

    pub fn output_latency_ms() -> f64 {
        let Some(class) = class() else { return 0.0 };
        // SAFETY: `+[DalyWaveBeat outputLatency]` — double в секундах
        let s: f64 = autoreleasepool(|_| unsafe { msg_send![class, outputLatency] });
        if s.is_finite() {
            (s * 1000.0).clamp(0.0, 1000.0)
        } else {
            0.0
        }
    }

    pub fn start() {
        if let Some(class) = class() {
            // SAFETY: без аргументов; Swift уходит в свою очередь
            autoreleasepool(|_| unsafe {
                let _: () = msg_send![class, start];
            });
        }
    }

    pub fn stop() {
        if let Some(class) = class() {
            // SAFETY: без аргументов
            autoreleasepool(|_| unsafe {
                let _: () = msg_send![class, stop];
            });
        }
    }

    pub fn reset() {
        if let Some(class) = class() {
            // SAFETY: без аргументов
            autoreleasepool(|_| unsafe {
                let _: () = msg_send![class, reset];
            });
        }
    }

    pub fn tap(intensity: f64, sharpness: f64, delay: f64) {
        if let Some(class) = class() {
            // SAFETY: `+[DalyWaveBeat tap:sharpness:delay:]` — три double
            autoreleasepool(|_| unsafe {
                let _: () = msg_send![class, tap: intensity, sharpness: sharpness, delay: delay];
            });
        }
    }
}

#[cfg(not(target_os = "ios"))]
mod swift {
    pub fn supported() -> bool {
        false
    }
    pub fn output_latency_ms() -> f64 {
        0.0
    }
    pub fn start() {}
    pub fn stop() {}
    pub fn reset() {}
    pub fn tap(_intensity: f64, _sharpness: f64, _delay: f64) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    fn analysis(beats: Vec<f64>, downbeats: Vec<f64>, curve: Vec<f64>) -> Analysis {
        serde_json::from_value(serde_json::json!({
            "beatsMs": beats,
            "downbeatsMs": downbeats,
            "energyCurve": curve,
        }))
        .unwrap()
    }

    #[test]
    fn downbeats_with_strength_from_energy() {
        let a = analysis(
            vec![],
            vec![500.0, 2500.0, 4500.0, 6500.0],
            vec![100.0, 200.0, 1000.0, 0.0, 500.0, 500.0, 250.0],
        );
        let p = Pulses::from_analysis(&a).unwrap();
        assert_eq!(p.at_ms, vec![500.0, 2500.0, 4500.0, 6500.0]);
        // 0-я секунда: 100/1000 → 0,46; 2-я: 1000/1000 → 1,0
        assert!((p.strength[0] - 0.46).abs() < 1e-6);
        assert!((p.strength[1] - 1.0).abs() < 1e-6);
        assert_eq!(p.next(0.0), Some((500.0, p.strength[0])));
        assert_eq!(p.next(500.0).map(|x| x.0), Some(2500.0), "строго после");
        assert_eq!(p.next(7000.0), None);
    }

    #[test]
    fn every_fourth_beat_without_downbeats() {
        let beats: Vec<f64> = (0..20).map(|i| 300.0 + 500.0 * f64::from(i)).collect();
        let p = Pulses::from_analysis(&analysis(beats, vec![], vec![])).unwrap();
        assert_eq!(p.at_ms[..3], [300.0, 2300.0, 4300.0]);
        // Без кривой энергии — средняя сила
        assert!((p.strength[0] - 0.7).abs() < 1e-6);
    }

    #[test]
    fn no_grid_no_pulses() {
        assert!(Pulses::from_analysis(&analysis(vec![], vec![], vec![])).is_none());
        assert!(Pulses::from_analysis(&analysis(vec![1.0, 2.0], vec![1.0], vec![])).is_none());
    }
}
