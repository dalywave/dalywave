//! Куда идёт звук: список устройств вывода и выбор (macOS, Windows), системное окно выбора
//! AirPlay и Bluetooth (iPhone, iPad).
//!
//! На компьютерах звук выводит свой движок через cpal, поэтому и устройство выбираем сами:
//! движок открывает вывод на нём заново и продолжает трек с той же позиции; выбор запоминается
//! в настройках. На iOS движок играет через аудиосессию, и маршрут решает система — приложение
//! только открывает её окно (AVRoutePickerView) и показывает, куда звук идёт сейчас.
//!
//! Событие `audio:outputs` — когда список или маршрут поменялся. macOS сообщает о переменах
//! сама (слушатель CoreAudio); на Windows такого слушателя у cpal нет — список перечитывается
//! раз в 3 с, но только минуту после того, как интерфейс его спросил (шторка «Устройства»
//! открыта), чтобы не будить систему зря. Везде список перечитывается и после того, как движок
//! открыл вывод заново или поток сам переехал на другое устройство.

use std::sync::Mutex;
use std::time::{Duration, Instant};

use crossbeam_channel::Sender;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::{
    MAIN,
    error::{ShellError, ShellResult},
};
#[cfg(desktop)]
use crate::{player, settings::Settings};

pub const EVENT_OUTPUTS: &str = "audio:outputs";

/// Перемены приходят пачкой (устройство пропало — сменилось и устройство по умолчанию)
const DEBOUNCE: Duration = Duration::from_millis(300);
#[cfg(all(desktop, not(target_os = "macos")))]
const POLL_EVERY: Duration = Duration::from_secs(3);
#[cfg(all(desktop, not(target_os = "macos")))]
const POLL_WHILE_ASKED: Duration = Duration::from_secs(60);
/// Сколько `audio_set_output` ждёт, пока движок откроет вывод на новом устройстве (AirPlay — до
/// секунды-двух)
#[cfg(desktop)]
const SWITCH_WAIT: Duration = Duration::from_secs(3);

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioOutput {
    /// `cpal::DeviceId` строкой (macOS, Windows) или тип порта (iOS)
    id: String,
    name: String,
    /// `builtin`, `headphones`, `bluetooth`, `airplay`, `usb`, `hdmi`, `other`
    kind: &'static str,
    /// Сейчас звук идёт сюда
    current: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioOutputs {
    /// `list` — свой список и `audio_set_output`; `picker` — окно системы, `audio_show_picker`
    mode: &'static str,
    outputs: Vec<AudioOutput>,
    /// Поток следует за устройством по умолчанию в системе
    follows_system: bool,
}

pub struct Audio {
    /// Что интерфейс знает о списке: последний ответ или событие
    last: Mutex<Option<AudioOutputs>>,
    #[cfg_attr(any(target_os = "macos", mobile), allow(dead_code))]
    asked_at: Mutex<Option<Instant>>,
    refresh: Sender<()>,
    #[cfg(target_os = "macos")]
    _watch: Option<dalywave_audio::devices::DeviceWatch>,
}

/// Состояние и фоновый поток, который перечитывает список и шлёт `audio:outputs`.
pub fn init(app: &AppHandle) {
    let (tx, rx) = crossbeam_channel::unbounded::<()>();
    #[cfg(target_os = "macos")]
    let watch = {
        let tx = tx.clone();
        match dalywave_audio::devices::watch_output_devices(move || {
            let _ = tx.send(());
        }) {
            Ok(w) => Some(w),
            Err(e) => {
                log::warn!("audio outputs: no CoreAudio listener: {e}");
                None
            }
        }
    };
    app.manage(Audio {
        last: Mutex::new(None),
        asked_at: Mutex::new(None),
        refresh: tx,
        #[cfg(target_os = "macos")]
        _watch: watch,
    });
    let app = app.clone();
    let spawned = std::thread::Builder::new()
        .name("dw-audio-outputs".into())
        .spawn(move || watch_loop(&app, &rx));
    if let Err(e) = spawned {
        log::warn!("audio outputs: no watcher thread: {e}");
    }
}

fn watch_loop(app: &AppHandle, rx: &crossbeam_channel::Receiver<()>) {
    loop {
        #[cfg(any(target_os = "macos", mobile))]
        if rx.recv().is_err() {
            return;
        }
        #[cfg(all(desktop, not(target_os = "macos")))]
        match rx.recv_timeout(POLL_EVERY) {
            Ok(()) => {}
            Err(crossbeam_channel::RecvTimeoutError::Timeout) => {
                let asked = *app
                    .state::<Audio>()
                    .asked_at
                    .lock()
                    .unwrap_or_else(|e| e.into_inner());
                if asked.is_none_or(|t| t.elapsed() > POLL_WHILE_ASKED) {
                    continue;
                }
            }
            Err(crossbeam_channel::RecvTimeoutError::Disconnected) => return,
        }
        std::thread::sleep(DEBOUNCE);
        while rx.try_recv().is_ok() {}
        // macOS: устройство по умолчанию могло стать колонкой AirPlay (или перестать) — движок сам
        // решит, нужен ли буферизованный вывод
        #[cfg(target_os = "macos")]
        if let Some(engine) = app.state::<player::Player>().engine() {
            engine.route_changed();
        }
        match current(app) {
            Ok(outputs) => publish(app, outputs),
            Err(e) => log::debug!("audio outputs: {e}"),
        }
    }
}

/// Что-то поменялось (маршрут iOS, движок открыл вывод заново) — перечитать список.
pub fn changed(app: &AppHandle) {
    if let Some(audio) = app.try_state::<Audio>() {
        let _ = audio.refresh.send(());
    }
}

/// Запомнить и отправить интерфейсу, если список отличается от того, что он уже знает.
fn publish(app: &AppHandle, outputs: AudioOutputs) {
    let audio = app.state::<Audio>();
    let mut last = audio.last.lock().unwrap_or_else(|e| e.into_inner());
    if last.as_ref() == Some(&outputs) {
        return;
    }
    *last = Some(outputs.clone());
    drop(last);
    log::info!(
        "audio outputs: {}",
        outputs
            .outputs
            .iter()
            .map(|o| format!(
                "{}{} ({})",
                if o.current { "*" } else { "" },
                o.name,
                o.kind
            ))
            .collect::<Vec<_>>()
            .join(", ")
    );
    let _ = app.emit_to(MAIN, EVENT_OUTPUTS, outputs);
}

fn remember(app: &AppHandle, outputs: &AudioOutputs) {
    let audio = app.state::<Audio>();
    *audio.last.lock().unwrap_or_else(|e| e.into_inner()) = Some(outputs.clone());
    *audio.asked_at.lock().unwrap_or_else(|e| e.into_inner()) = Some(Instant::now());
}

/// Компьютеры: все устройства вывода; текущее — то, к которому привязан поток движка, или
/// устройство по умолчанию, если поток следует за ним.
#[cfg(desktop)]
fn current(app: &AppHandle) -> ShellResult<AudioOutputs> {
    let devices =
        dalywave_audio::devices::output_devices().map_err(|e| ShellError::Audio(e.to_string()))?;
    let pinned = app
        .state::<player::Player>()
        .engine()
        .and_then(|e| e.snapshot().output_device);
    let outputs = devices
        .into_iter()
        .map(|d| AudioOutput {
            current: match &pinned {
                Some(id) => *id == d.id,
                None => d.is_default,
            },
            kind: d.kind.as_str(),
            name: d.name,
            id: d.id,
        })
        .collect();
    Ok(AudioOutputs {
        mode: "list",
        outputs,
        follows_system: pinned.is_none(),
    })
}

/// iPhone и iPad: текущий маршрут аудиосессии; выбирает система.
#[cfg(mobile)]
fn current(_app: &AppHandle) -> ShellResult<AudioOutputs> {
    #[cfg(target_os = "ios")]
    let outputs = dalywave_audio::apple::current_route()
        .into_iter()
        .map(|p| AudioOutput {
            id: p.port_type,
            name: p.name,
            kind: p.kind.as_str(),
            current: true,
        })
        .collect();
    #[cfg(not(target_os = "ios"))]
    let outputs = Vec::new();
    Ok(AudioOutputs {
        mode: "picker",
        outputs,
        follows_system: true,
    })
}

#[tauri::command]
pub async fn audio_outputs(app: AppHandle) -> ShellResult<AudioOutputs> {
    let outputs = current(&app)?;
    remember(&app, &outputs);
    Ok(outputs)
}

/// Вывести звук на устройство; `null` — на системное по умолчанию. Выбор запоминается.
#[cfg(desktop)]
#[tauri::command]
pub async fn audio_set_output(app: AppHandle, id: Option<String>) -> ShellResult<AudioOutputs> {
    let engine = player::engine(&app).await?;
    if let Some(id) = &id {
        let devices = dalywave_audio::devices::output_devices()
            .map_err(|e| ShellError::Audio(e.to_string()))?;
        if !devices.iter().any(|d| d.id == *id) {
            return Err(ShellError::Invalid(format!("устройства вывода «{id}» нет")));
        }
    }
    app.state::<Settings>().set_audio_output(id.clone())?;
    let before = engine.snapshot();
    engine.set_output_device(id.clone());
    if before.output_device != id {
        // Движок открывает вывод заново в своём потоке — ответить уже с новым устройством
        let deadline = Instant::now() + SWITCH_WAIT;
        while engine.snapshot().output_reopens == before.output_reopens && Instant::now() < deadline
        {
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    }
    let outputs = current(&app)?;
    remember(&app, &outputs);
    log::info!(
        "audio output: {}",
        id.as_deref().unwrap_or("system default")
    );
    Ok(outputs)
}

#[cfg(mobile)]
#[tauri::command]
pub async fn audio_set_output(_id: Option<String>) -> ShellResult<AudioOutputs> {
    Err(ShellError::Unsupported(
        "на iPhone и iPad устройство выбирает система: audio_show_picker".into(),
    ))
}

/// Окно системы «Куда выводить звук» (AirPlay, Bluetooth, динамик).
#[cfg(target_os = "ios")]
#[tauri::command]
pub async fn audio_show_picker(app: AppHandle) -> ShellResult<()> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    let handle = app.clone();
    app.run_on_main_thread(move || {
        let _ = tx.send(crate::ios::show_route_picker(&handle));
    })?;
    rx.await
        .map_err(ShellError::internal)?
        .map_err(ShellError::Audio)
}

#[cfg(not(target_os = "ios"))]
#[tauri::command]
pub async fn audio_show_picker() -> ShellResult<()> {
    Err(ShellError::Unsupported(
        "окно выбора системы — только на iPhone и iPad: audio_set_output".into(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn outputs_json() {
        let v = serde_json::to_value(AudioOutputs {
            mode: "list",
            outputs: vec![AudioOutput {
                id: "coreaudio:BuiltInSpeakerDevice".into(),
                name: "Динамики MacBook Pro".into(),
                kind: "builtin",
                current: true,
            }],
            follows_system: true,
        })
        .unwrap();
        assert_eq!(
            v,
            serde_json::json!({
                "mode": "list",
                "outputs": [{
                    "id": "coreaudio:BuiltInSpeakerDevice",
                    "name": "Динамики MacBook Pro",
                    "kind": "builtin",
                    "current": true
                }],
                "followsSystem": true
            })
        );
    }
}
