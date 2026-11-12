//! Системная плашка «Сейчас играет» и пульт: медиаклавиши, наушники, Пункт управления, экран
//! блокировки iPhone, плашка Windows. Звук играет движок (player.rs), поэтому и плашку, и команды
//! ведёт Rust: macOS и iOS — MediaPlayer из движка (dalywave_audio::apple), Windows — SMTC
//! (souvlaki). Интерфейсу уходит событием `media:command` только то, что движок не может сам.

#[cfg(target_vendor = "apple")]
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Instant;

use dalywave_audio::{Event, PlaybackState};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};

use crate::MAIN;

pub const EVENT_COMMAND: &str = "media:command";
/// Расхождение позиции с тем, что досчитала система, после которого плашку надо поправить
const DRIFT_MS: u64 = 1500;

/// Что сейчас играет — для системной плашки и трея.
#[derive(Debug, Clone, PartialEq)]
pub struct NowPlaying {
    pub title: String,
    pub artist: String,
    pub album: Option<String>,
    /// http(s) — обложка для системной плашки
    pub artwork_url: Option<String>,
    pub duration_ms: Option<u64>,
    pub position_ms: u64,
    pub playing: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Source {
    /// Медиаклавиши, наушники, Пункт управления, экран блокировки, плашка Windows
    System,
    #[cfg_attr(mobile, allow(dead_code))]
    Tray,
}

/// Повтор: выключен, вся очередь по кругу, один трек — как `repeat` плеера интерфейса.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RepeatMode {
    #[default]
    Off,
    All,
    One,
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
    /// Плашка Windows (SMTC); на macOS и iOS такой кнопки нет
    #[cfg_attr(not(windows), allow(dead_code))]
    Stop,
    /// Перейти к позиции
    Seek {
        position_ms: u64,
    },
    /// Сдвинуть позицию: вперёд — положительное число (плашка Windows)
    #[cfg_attr(not(windows), allow(dead_code))]
    SeekBy {
        offset_ms: i64,
    },
    /// Пункт управления, CarPlay, Siri (macOS, iOS): очередь перемешивает интерфейс
    #[cfg_attr(not(target_vendor = "apple"), allow(dead_code))]
    SetShuffle {
        shuffle: bool,
    },
    #[cfg_attr(not(target_vendor = "apple"), allow(dead_code))]
    SetRepeat {
        mode: RepeatMode,
    },
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct CommandEvent {
    #[serde(flatten)]
    command: Command,
    source: Source,
}

/// Когда плашке последний раз сообщали позицию: дальше система досчитывает сама.
#[derive(Clone, Copy)]
struct Synced {
    at: Instant,
    position_ms: u64,
    playing: bool,
}

#[derive(Default)]
pub struct Media {
    current: Mutex<Option<NowPlaying>>,
    /// Перемешивание и повтор интерфейса — для кнопок системы
    modes: Mutex<(bool, RepeatMode)>,
    synced: Mutex<Option<Synced>>,
    #[cfg(target_vendor = "apple")]
    apple: Mutex<AppleState>,
    #[cfg(windows)]
    controls: Mutex<Option<souvlaki::MediaControls>>,
}

#[cfg(target_vendor = "apple")]
#[derive(Default)]
struct AppleState {
    artwork: Option<(String, Arc<dalywave_audio::apple::Artwork>)>,
    remote: Option<dalywave_audio::apple::RemoteCommands>,
    #[cfg(target_os = "ios")]
    observers: Vec<dalywave_audio::apple::SessionObservers>,
}

impl Media {
    pub fn current(&self) -> Option<NowPlaying> {
        self.current
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }
}

/// Отдать команду интерфейсу: движок её сам не выполнит (например, «играть» без очереди).
pub fn forward(app: &AppHandle, command: Command, source: Source) {
    log::debug!("media command {command:?} from {source:?} to the interface");
    let _ = app.emit_to(MAIN, EVENT_COMMAND, CommandEvent { command, source });
}

/// Windows: плашка SMTC привязана к окну — подключаем после его создания.
#[cfg(windows)]
pub fn init_window(app: &AppHandle, window: &tauri::WebviewWindow) {
    use souvlaki::{
        MediaControlEvent, MediaControls, MediaPosition, PlatformConfig, SeekDirection,
    };
    let config = PlatformConfig {
        display_name: "DalyWave",
        dbus_name: "rest.daly.wave",
        hwnd: window.hwnd().ok().map(|h| h.0),
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
        let ms = |d: std::time::Duration| i64::try_from(d.as_millis()).unwrap_or(i64::MAX);
        let command = match event {
            MediaControlEvent::Play => Command::Play,
            MediaControlEvent::Pause => Command::Pause,
            MediaControlEvent::Toggle => Command::Toggle,
            MediaControlEvent::Next => Command::Next,
            MediaControlEvent::Previous => Command::Previous,
            MediaControlEvent::Stop => Command::Stop,
            MediaControlEvent::Seek(SeekDirection::Forward) => {
                Command::SeekBy { offset_ms: 10_000 }
            }
            MediaControlEvent::Seek(SeekDirection::Backward) => {
                Command::SeekBy { offset_ms: -10_000 }
            }
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
                crate::window::show_main(&handle);
                return;
            }
            _ => return,
        };
        crate::player::remote(&handle, command, Source::System);
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

/// Движок готов: подключить пульт системы. Главный поток.
pub fn attach_engine(app: &AppHandle) {
    #[cfg(target_vendor = "apple")]
    {
        use dalywave_audio::apple::{RemoteCommand, register_remote_commands};
        let handle = app.clone();
        let remote = register_remote_commands(move |c| {
            let command = match c {
                RemoteCommand::Play => Command::Play,
                RemoteCommand::Pause => Command::Pause,
                RemoteCommand::Toggle => Command::Toggle,
                RemoteCommand::Next => Command::Next,
                RemoteCommand::Previous => Command::Previous,
                RemoteCommand::Seek(s) => Command::Seek {
                    position_ms: (s.max(0.0) * 1000.0) as u64,
                },
                RemoteCommand::SetShuffle(shuffle) => Command::SetShuffle { shuffle },
                RemoteCommand::SetRepeat(r) => Command::SetRepeat {
                    mode: match r {
                        dalywave_audio::apple::Repeat::Off => RepeatMode::Off,
                        dalywave_audio::apple::Repeat::One => RepeatMode::One,
                        dalywave_audio::apple::Repeat::All => RepeatMode::All,
                    },
                },
            };
            crate::player::remote(&handle, command, Source::System);
        });
        let media = app.state::<Media>();
        let mut apple = media.apple.lock().unwrap_or_else(|e| e.into_inner());
        apple.remote = Some(remote);
        let (shuffle, repeat) = *media.modes.lock().unwrap_or_else(|e| e.into_inner());
        apple_modes(shuffle, repeat);
        #[cfg(target_os = "ios")]
        apple.observers.extend(ios_observers(app));
    }
    #[cfg(not(target_vendor = "apple"))]
    let _ = app;
}

/// Перемешивание и повтор интерфейса — кнопкам системы (Пункт управления, CarPlay на macOS и iOS).
/// Плашка Windows (souvlaki) таких кнопок не умеет.
pub fn set_modes(app: &AppHandle, shuffle: bool, repeat: RepeatMode) {
    let media = app.state::<Media>();
    let mut modes = media.modes.lock().unwrap_or_else(|e| e.into_inner());
    if *modes == (shuffle, repeat) {
        return;
    }
    *modes = (shuffle, repeat);
    #[cfg(target_vendor = "apple")]
    {
        let _ = app.run_on_main_thread(move || apple_modes(shuffle, repeat));
    }
}

#[cfg(target_vendor = "apple")]
fn apple_modes(shuffle: bool, repeat: RepeatMode) {
    use dalywave_audio::apple::{Repeat, set_modes};
    set_modes(
        shuffle,
        match repeat {
            RepeatMode::Off => Repeat::Off,
            RepeatMode::All => Repeat::All,
            RepeatMode::One => Repeat::One,
        },
    );
}

/// iOS: звонок, Siri, будильник, наушники, сброс медиасервисов, уход в фон.
#[cfg(target_os = "ios")]
fn ios_observers(app: &AppHandle) -> Vec<dalywave_audio::apple::SessionObservers> {
    use std::sync::atomic::{AtomicBool, Ordering};

    use dalywave_audio::apple::{SessionEvent, observe_app_lifecycle, observe_session};

    use crate::player::Player;

    let handle = app.clone();
    let was_playing = Arc::new(AtomicBool::new(false));
    let session = observe_session(move |ev| {
        log::info!("audio session: {ev:?}");
        if ev == SessionEvent::RouteChanged {
            // AirPlay, наушники, Bluetooth — шторке «Устройства» новый маршрут, движку — выбор
            // вывода: AirPlay (и несколько HomePod) — буферизованный, остальное — обычный
            crate::audio::changed(&handle);
            if let Some(engine) = handle.state::<Player>().engine() {
                engine.route_changed();
            }
            return;
        }
        let Some(engine) = handle.state::<Player>().engine() else {
            return;
        };
        match ev {
            SessionEvent::InterruptionBegan => {
                was_playing.store(
                    engine.snapshot().state == PlaybackState::Playing,
                    Ordering::SeqCst,
                );
                engine.pause();
            }
            SessionEvent::InterruptionEnded { should_resume } => {
                if should_resume && was_playing.swap(false, Ordering::SeqCst) {
                    // Активация сессии — не на главном потоке
                    std::thread::spawn(move || {
                        if let Err(e) = dalywave_audio::apple::activate_audio_session() {
                            log::warn!("audio session after interruption: {e}");
                        }
                        engine.play();
                    });
                }
            }
            SessionEvent::HeadphonesUnplugged => engine.pause(),
            SessionEvent::RouteChanged => {}
            SessionEvent::MediaServicesReset => {
                std::thread::spawn(move || {
                    if let Err(e) = dalywave_audio::apple::activate_audio_session() {
                        log::warn!("audio session after media services reset: {e}");
                    }
                    engine.reopen_output("media services reset");
                });
            }
        }
    });
    let handle = app.clone();
    let lifecycle = observe_app_lifecycle(move |background| {
        log::info!(
            "app {}",
            if background {
                "in background"
            } else {
                "in foreground"
            }
        );
        handle.state::<Player>().set_background(&handle, background);
        crate::widgets::on_background(&handle, background);
    });
    vec![session, lifecycle]
}

/// Событие движка: обновить плашку, если что-то изменилось для системы.
pub fn on_player_event(app: &AppHandle, ev: &Event) {
    let media = app.state::<Media>();
    let mut track_changed = false;
    let mut force = false;
    {
        let mut current = media.current.lock().unwrap_or_else(|e| e.into_inner());
        match ev {
            Event::TrackChanged { track, .. } => {
                let playing = current.as_ref().is_some_and(|c| c.playing);
                *current = Some(NowPlaying {
                    title: track.title.clone(),
                    artist: track.artist.clone(),
                    album: (!track.album.is_empty()).then(|| track.album.clone()),
                    artwork_url: track.artwork_url.clone(),
                    duration_ms: track.duration_ms,
                    position_ms: 0,
                    playing,
                });
                track_changed = true;
            }
            Event::State(s) => {
                match s {
                    PlaybackState::Idle => *current = None,
                    // Загрузка следующего трека — не пауза: плашка не мигает
                    PlaybackState::Loading => return,
                    _ => {
                        if let Some(c) = current.as_mut() {
                            c.playing = *s == PlaybackState::Playing;
                        }
                    }
                }
                force = true;
            }
            Event::QueueEnded => {
                if let Some(c) = current.as_mut() {
                    c.playing = false;
                }
                force = true;
            }
            Event::Position {
                position_ms,
                duration_ms,
                ..
            } => {
                let Some(c) = current.as_mut() else { return };
                c.position_ms = *position_ms;
                if duration_ms.is_some() && c.duration_ms != *duration_ms {
                    c.duration_ms = *duration_ms;
                    force = true;
                }
            }
            _ => return,
        }
    }
    let now = media.current();
    if !force && !track_changed && !drifted(&media, now.as_ref()) {
        return;
    }
    // Виджетам позиция не нужна — только смена трека и «играет или пауза»
    if force || track_changed {
        crate::widgets::now_playing(app, now.as_ref());
    }
    *media.synced.lock().unwrap_or_else(|e| e.into_inner()) = now.as_ref().map(|n| Synced {
        at: Instant::now(),
        position_ms: n.position_ms,
        playing: n.playing,
    });
    publish(app, now, track_changed);
}

/// Система досчитывает позицию сама; поправлять плашку нужно после перемотки и раз в минуту.
fn drifted(media: &Media, now: Option<&NowPlaying>) -> bool {
    let Some(now) = now else { return false };
    let Some(s) = *media.synced.lock().unwrap_or_else(|e| e.into_inner()) else {
        return true;
    };
    let elapsed = s.at.elapsed();
    let expected = s.position_ms
        + if s.playing {
            elapsed.as_millis() as u64
        } else {
            0
        };
    expected.abs_diff(now.position_ms) > DRIFT_MS || elapsed.as_secs() >= 60
}

fn publish(app: &AppHandle, now: Option<NowPlaying>, track_changed: bool) {
    let handle = app.clone();
    let _ = app.run_on_main_thread(move || {
        publish_main(&handle, now.as_ref(), track_changed);
        #[cfg(desktop)]
        crate::tray::refresh(&handle);
    });
}

#[cfg(target_vendor = "apple")]
fn publish_main(app: &AppHandle, now: Option<&NowPlaying>, track_changed: bool) {
    use dalywave_audio::apple;
    let media = app.state::<Media>();
    let Some(now) = now else {
        apple::clear_now_playing();
        return;
    };
    let artwork = {
        let state = media.apple.lock().unwrap_or_else(|e| e.into_inner());
        state
            .artwork
            .as_ref()
            .filter(|(url, _)| now.artwork_url.as_ref() == Some(url))
            .map(|(_, a)| a.clone())
    };
    if track_changed
        && artwork.is_none()
        && let Some(url) = now.artwork_url.clone()
    {
        load_artwork(app, url);
    }
    apple::set_now_playing(
        &apple::NowPlaying {
            title: now.title.clone(),
            artist: now.artist.clone(),
            album: now.album.clone().unwrap_or_default(),
            duration_s: now.duration_ms.map(|d| d as f64 / 1000.0),
            elapsed_s: now.position_ms as f64 / 1000.0,
            rate: if now.playing { 1.0 } else { 0.0 },
        },
        artwork.as_deref(),
    );
}

/// Обложка для плашки: система берёт картинку из памяти, поэтому качаем её сами.
#[cfg(target_vendor = "apple")]
fn load_artwork(app: &AppHandle, url: String) {
    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        let bytes = async {
            let res = reqwest::get(&url).await?.error_for_status()?;
            res.bytes().await
        }
        .await;
        let bytes = match bytes {
            Ok(b) => b,
            Err(e) => {
                log::warn!("artwork for now playing: {e}");
                return;
            }
        };
        let main = handle.clone();
        let _ = handle.run_on_main_thread(move || {
            let Some(artwork) = dalywave_audio::apple::Artwork::from_bytes(&bytes) else {
                log::warn!("artwork for now playing: not an image");
                return;
            };
            let media = main.state::<Media>();
            media
                .apple
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .artwork = Some((url.clone(), Arc::new(artwork)));
            // Трек всё ещё тот же — показать обложку сразу
            let now = media.current();
            if now.as_ref().and_then(|n| n.artwork_url.as_ref()) == Some(&url) {
                publish_main(&main, now.as_ref(), false);
            }
        });
    });
}

#[cfg(windows)]
fn publish_main(app: &AppHandle, now: Option<&NowPlaying>, track_changed: bool) {
    use souvlaki::{MediaMetadata, MediaPlayback, MediaPosition};
    use std::time::Duration;
    let media = app.state::<Media>();
    let mut controls = media.controls.lock().unwrap_or_else(|e| e.into_inner());
    let Some(controls) = controls.as_mut() else {
        return;
    };
    let Some(now) = now else {
        let _ = controls.set_playback(MediaPlayback::Stopped);
        let _ = controls.set_metadata(MediaMetadata::default());
        return;
    };
    // Обложку система загружает заново на каждое обновление метаданных — только при смене трека
    if track_changed {
        let metadata = MediaMetadata {
            title: Some(&now.title),
            artist: Some(&now.artist),
            album: now.album.as_deref(),
            cover_url: now.artwork_url.as_deref(),
            duration: now.duration_ms.map(Duration::from_millis),
        };
        if let Err(e) = controls.set_metadata(metadata) {
            log::warn!("could not publish now playing metadata: {e:?}");
        }
    }
    let progress = Some(MediaPosition(Duration::from_millis(now.position_ms)));
    let playback = if now.playing {
        MediaPlayback::Playing { progress }
    } else {
        MediaPlayback::Paused { progress }
    };
    if let Err(e) = controls.set_playback(playback) {
        log::warn!("could not publish playback state: {e:?}");
    }
}

#[cfg(not(any(target_vendor = "apple", windows)))]
fn publish_main(_app: &AppHandle, _now: Option<&NowPlaying>, _track_changed: bool) {}

/// Для журнала проверки: что записано в «Сейчас играет» (macOS и iOS).
pub fn log_now_playing(app: &AppHandle) {
    #[cfg(target_vendor = "apple")]
    {
        let _ = app.run_on_main_thread(|| {
            log::info!(
                "now playing: {}",
                dalywave_audio::apple::now_playing_description()
            );
        });
    }
    #[cfg(not(target_vendor = "apple"))]
    let _ = app;
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
        let event = CommandEvent {
            command: Command::SetRepeat {
                mode: RepeatMode::One,
            },
            source: Source::System,
        };
        assert_eq!(
            serde_json::to_value(event).unwrap(),
            serde_json::json!({ "action": "setRepeat", "mode": "one", "source": "system" })
        );
        let event = CommandEvent {
            command: Command::SetShuffle { shuffle: true },
            source: Source::System,
        };
        assert_eq!(
            serde_json::to_value(event).unwrap(),
            serde_json::json!({ "action": "setShuffle", "shuffle": true, "source": "system" })
        );
        assert_eq!(
            serde_json::from_str::<RepeatMode>(r#""all""#).unwrap(),
            RepeatMode::All
        );
    }
}
