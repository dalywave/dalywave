//! Системная плашка «Сейчас играет» (macOS — MPNowPlayingInfoCenter, Windows — SMTC) и
//! медиаклавиши. Звук пока играет в WebView: состояние присылает интерфейс, а команды
//! клавиш, наушников, Пункта управления и трея уходят ему событием `media:command`.

use std::{sync::Mutex, time::Duration};

use serde::{Deserialize, Serialize};
use souvlaki::{
    MediaControlEvent, MediaControls, MediaMetadata, MediaPlayback, MediaPosition, PlatformConfig,
    SeekDirection,
};
use tauri::{AppHandle, Emitter, Manager, WebviewWindow};

use crate::{MAIN, error::ShellResult};

pub const EVENT_COMMAND: &str = "media:command";
/// Шаг перемотки, если система не сказала, на сколько
const SEEK_STEP_MS: i64 = 10_000;

/// Что сейчас играет — присылает интерфейс при смене трека, паузе, запуске и перемотке.
/// Позицию между событиями система досчитывает сама.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NowPlaying {
    pub title: String,
    pub artist: String,
    #[serde(default)]
    pub album: Option<String>,
    /// http(s) или file:// — обложка для системной плашки
    #[serde(default)]
    pub artwork_url: Option<String>,
    #[serde(default)]
    pub duration_ms: Option<u64>,
    #[serde(default)]
    pub position_ms: Option<u64>,
    pub playing: bool,
}

impl NowPlaying {
    fn same_track(&self, other: &Self) -> bool {
        self.title == other.title
            && self.artist == other.artist
            && self.album == other.album
            && self.artwork_url == other.artwork_url
            && self.duration_ms == other.duration_ms
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Source {
    /// Медиаклавиши, наушники, Пункт управления macOS, плашка Windows
    System,
    Tray,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(
    tag = "action",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum Command {
    Play,
    Pause,
    Toggle,
    Next,
    Previous,
    Stop,
    /// Перейти к позиции
    Seek {
        position_ms: u64,
    },
    /// Сдвинуть позицию: вперёд — положительное число
    SeekBy {
        offset_ms: i64,
    },
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct CommandEvent {
    #[serde(flatten)]
    command: Command,
    source: Source,
}

#[derive(Default)]
pub struct Media {
    controls: Mutex<Option<MediaControls>>,
    current: Mutex<Option<NowPlaying>>,
}

impl Media {
    pub fn current(&self) -> Option<NowPlaying> {
        self.current
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }
}

/// Подключить системные медиаэлементы. На Windows им нужно окно, поэтому — после его создания.
pub fn init(app: &AppHandle, window: &WebviewWindow) {
    #[cfg(windows)]
    let hwnd = window.hwnd().ok().map(|h| h.0);
    #[cfg(not(windows))]
    let hwnd = {
        let _ = window;
        None
    };
    let config = PlatformConfig {
        display_name: "DalyWave",
        dbus_name: "rest.daly.wave",
        hwnd,
    };
    let mut controls = match MediaControls::new(config) {
        Ok(controls) => controls,
        Err(e) => {
            log::warn!("system media controls are unavailable: {e:?}");
            return;
        }
    };
    let handle = app.clone();
    let attached = controls.attach(move |event| {
        if let Some(command) = from_system(event, &handle) {
            send(&handle, command, Source::System);
        }
    });
    if let Err(e) = attached {
        log::warn!("could not attach media key handlers: {e:?}");
        return;
    }
    *app.state::<Media>()
        .controls
        .lock()
        .unwrap_or_else(|e| e.into_inner()) = Some(controls);
}

fn from_system(event: MediaControlEvent, app: &AppHandle) -> Option<Command> {
    let ms = |d: Duration| i64::try_from(d.as_millis()).unwrap_or(i64::MAX);
    Some(match event {
        MediaControlEvent::Play => Command::Play,
        MediaControlEvent::Pause => Command::Pause,
        MediaControlEvent::Toggle => Command::Toggle,
        MediaControlEvent::Next => Command::Next,
        MediaControlEvent::Previous => Command::Previous,
        MediaControlEvent::Stop => Command::Stop,
        MediaControlEvent::Seek(SeekDirection::Forward) => Command::SeekBy {
            offset_ms: SEEK_STEP_MS,
        },
        MediaControlEvent::Seek(SeekDirection::Backward) => Command::SeekBy {
            offset_ms: -SEEK_STEP_MS,
        },
        MediaControlEvent::SeekBy(SeekDirection::Forward, d) => {
            Command::SeekBy { offset_ms: ms(d) }
        }
        MediaControlEvent::SeekBy(SeekDirection::Backward, d) => {
            Command::SeekBy { offset_ms: -ms(d) }
        }
        MediaControlEvent::SetPosition(MediaPosition(d)) => Command::Seek {
            position_ms: u64::try_from(d.as_millis()).unwrap_or(u64::MAX),
        },
        MediaControlEvent::Raise => {
            crate::window::show_main(app);
            return None;
        }
        _ => return None,
    })
}

pub fn send(app: &AppHandle, command: Command, source: Source) {
    log::debug!("media command {command:?} from {source:?}");
    let _ = app.emit_to(MAIN, EVENT_COMMAND, CommandEvent { command, source });
}

/// `null` — ничего не играет: плашка пропадает.
#[tauri::command]
pub fn now_playing_update(app: AppHandle, state: Option<NowPlaying>) -> ShellResult<()> {
    let media = app.state::<Media>();
    let previous = {
        let mut current = media.current.lock().unwrap_or_else(|e| e.into_inner());
        std::mem::replace(&mut *current, state.clone())
    };
    {
        let mut controls = media.controls.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(controls) = controls.as_mut() {
            publish(controls, previous.as_ref(), state.as_ref());
        }
    }
    crate::tray::refresh(&app);
    Ok(())
}

fn publish(
    controls: &mut MediaControls,
    previous: Option<&NowPlaying>,
    state: Option<&NowPlaying>,
) {
    let Some(state) = state else {
        if let Err(e) = controls.set_playback(MediaPlayback::Stopped) {
            log::warn!("could not clear now playing: {e:?}");
        }
        let _ = controls.set_metadata(MediaMetadata::default());
        return;
    };
    // Обложку система загружает заново на каждое обновление метаданных — только при смене трека
    if previous.is_none_or(|p| !p.same_track(state)) {
        let metadata = MediaMetadata {
            title: Some(&state.title),
            artist: Some(&state.artist),
            album: state.album.as_deref(),
            cover_url: state.artwork_url.as_deref(),
            duration: state.duration_ms.map(Duration::from_millis),
        };
        if let Err(e) = controls.set_metadata(metadata) {
            log::warn!("could not publish now playing metadata: {e:?}");
        }
    }
    let progress = state
        .position_ms
        .map(|ms| MediaPosition(Duration::from_millis(ms)));
    let playback = if state.playing {
        MediaPlayback::Playing { progress }
    } else {
        MediaPlayback::Paused { progress }
    };
    if let Err(e) = controls.set_playback(playback) {
        log::warn!("could not publish playback state: {e:?}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_json() {
        let event = CommandEvent {
            command: Command::SeekBy { offset_ms: -10_000 },
            source: Source::System,
        };
        assert_eq!(
            serde_json::to_value(event).unwrap(),
            serde_json::json!({ "action": "seekBy", "offsetMs": -10000, "source": "system" })
        );
        let event = CommandEvent {
            command: Command::Toggle,
            source: Source::Tray,
        };
        assert_eq!(
            serde_json::to_value(event).unwrap(),
            serde_json::json!({ "action": "toggle", "source": "tray" })
        );
    }

    #[test]
    fn now_playing_json() {
        let state: NowPlaying = serde_json::from_value(serde_json::json!({
            "title": "Float", "artist": "Call Me Karizma", "durationMs": 169000, "playing": true
        }))
        .unwrap();
        assert_eq!(state.duration_ms, Some(169_000));
        assert!(state.album.is_none());
    }
}
