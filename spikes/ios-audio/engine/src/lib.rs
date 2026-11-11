//! dw-audio — аудиодвижок DalyWave (спайк).
//!
//! Поток данных:
//!
//! ```text
//! источник (HTTP с Range / файл) → symphonia (+libopus) → TrackDecoder (точная обрезка)
//!   → DeckWorker: громкость трека, ресемплинг rubato до частоты устройства
//!   → кольцевой буфер rtrb (SPSC, без блокировок)
//!   → Mixer в аудиоколбэке cpal: N дек, огибающие кроссфейда, общая громкость
//! ```
//!
//! «Без пауз» сделан цепочкой сегментов внутри одной деки: следующий трек декодируется в тот же
//! буфер сразу за предыдущим, ресемплер не сбрасывается — стык точен до сэмпла. Кроссфейд и
//! будущий режим «Диджей» используют разные деки, которые сводит микшер.

mod controller;
mod deck;
mod decode;
mod error;
mod mixer;
mod mp4;
mod output;
mod resample;
pub mod source;
pub mod testkit;

#[cfg(target_vendor = "apple")]
pub mod apple;

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

use crossbeam_channel::{Receiver, Sender};

pub use error::AudioError;
pub use mixer::equal_power;

/// Куда выводится звук.
#[derive(Clone, Debug)]
pub enum OutputConfig {
    /// Системное устройство вывода через cpal (CoreAudio, WASAPI).
    Device {
        /// Желаемый размер буфера в кадрах; None — по умолчанию системы.
        buffer_frames: Option<u32>,
    },
    /// Тестовый режим: выход рендерится в память так быстро, как готовы данные,
    /// без недогрузок. Используется для проверки стыков.
    Capture { sample_rate: u32 },
}

/// Режим перехода между треками.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Transition {
    Gapless,
    Crossfade { ms: u32 },
}

#[derive(Clone, Debug)]
pub struct EngineConfig {
    pub output: OutputConfig,
    /// Глубина кольцевого буфера деки, мс.
    pub ring_ms: u32,
    /// Сколько должно накопиться в буфере, прежде чем дека зазвучит, мс.
    pub start_threshold_ms: u32,
    /// За сколько секунд до конца трека готовить следующий.
    pub preload_secs: u32,
    /// Целевая громкость нормализации, LUFS.
    pub target_lufs: f32,
    pub normalize: bool,
    pub transition: Transition,
    pub volume: f32,
    pub user_agent: String,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            output: OutputConfig::Device {
                buffer_frames: None,
            },
            ring_ms: 3000,
            start_threshold_ms: 100,
            preload_secs: 30,
            target_lufs: -14.0,
            normalize: true,
            transition: Transition::Gapless,
            volume: 1.0,
            user_agent: format!("DalyWave-audio/{}", env!("CARGO_PKG_VERSION")),
        }
    }
}

/// Откуда брать байты трека.
#[derive(Clone, Debug)]
pub enum SourceSpec {
    Url(String),
    File(PathBuf),
}

/// Трек в очереди. Метаданные приходят от вызывающего (с сервера), движок их не угадывает.
#[derive(Clone, Debug)]
pub struct TrackSpec {
    pub id: String,
    pub source: SourceSpec,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub artwork_url: Option<String>,
    pub duration_ms: Option<u64>,
    /// Интегральная громкость трека (EBU R128), LUFS.
    pub loudness_lufs: Option<f32>,
    /// Пик трека, dBFS: усиление при нормализации не поднимет пик выше −1 dBFS.
    pub peak_db: Option<f32>,
}

impl TrackSpec {
    pub fn url(id: impl Into<String>, url: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            source: SourceSpec::Url(url.into()),
            title: String::new(),
            artist: String::new(),
            album: String::new(),
            artwork_url: None,
            duration_ms: None,
            loudness_lufs: None,
            peak_db: None,
        }
    }

    pub fn file(id: impl Into<String>, path: impl Into<PathBuf>) -> Self {
        let mut t = Self::url(id, "");
        t.source = SourceSpec::File(path.into());
        t
    }

    /// Усиление нормализации к `target` LUFS с ограничением по пику и сверху +12 дБ.
    pub fn gain_for(&self, target_lufs: f32) -> f32 {
        let Some(lufs) = self.loudness_lufs else {
            return 1.0;
        };
        let mut db = (target_lufs - lufs).min(12.0);
        if let Some(peak) = self.peak_db {
            db = db.min(-1.0 - peak);
        }
        10f32.powf(db / 20.0)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlaybackState {
    Idle,
    Loading,
    Playing,
    Paused,
    /// Очередь доиграла до конца.
    Ended,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChangeReason {
    /// Сам перешёл к следующему треку (без пауз или кроссфейдом).
    Auto,
    /// Команда: загрузка очереди, next, previous.
    User,
}

#[derive(Clone, Debug)]
pub enum Event {
    State(PlaybackState),
    /// Каждые 250 мс во время воспроизведения и сразу после перемотки и паузы.
    Position {
        index: usize,
        position_ms: u64,
        duration_ms: Option<u64>,
    },
    TrackChanged {
        index: usize,
        track: TrackSpec,
        reason: ChangeReason,
    },
    Buffering(bool),
    Error {
        index: Option<usize>,
        message: String,
    },
    QueueEnded,
    /// Время от команды до первого сэмпла в колбэке вывода.
    FirstAudio {
        index: usize,
        latency_ms: f64,
    },
    /// Диагностика деки: кодек, контейнер, частоты.
    TrackInfo {
        index: usize,
        info: String,
    },
}

/// Снимок состояния для синхронных запросов (интерфейс, «сейчас играет»).
#[derive(Clone, Debug)]
pub struct Snapshot {
    pub state: PlaybackState,
    pub index: Option<usize>,
    pub position_ms: u64,
    pub duration_ms: Option<u64>,
    pub volume: f32,
    pub transition: Transition,
    pub queue_len: usize,
    pub device_rate: u32,
    pub device_channels: u16,
    pub underruns: u64,
    /// Средняя и максимальная загрузка аудиоколбэка: время обработки / длительность буфера.
    pub callback_load_avg: f32,
    pub callback_load_max: f32,
    pub buffer_frames: u32,
    /// Сколько кадров отдал микшер с запуска (растёт, только пока устройство запрашивает звук).
    pub output_frames: u64,
}

pub(crate) enum Command {
    LoadQueue {
        tracks: Vec<TrackSpec>,
        start: usize,
        autoplay: bool,
    },
    Append(Vec<TrackSpec>),
    Play,
    Pause,
    Toggle,
    Seek(u64),
    Next,
    Previous,
    SetVolume(f32),
    SetTransition(Transition),
    Stop,
    Shutdown,
}

struct Inner {
    tx: Sender<Command>,
    snapshot: Arc<Mutex<Snapshot>>,
    capture: Arc<Mutex<Vec<f32>>>,
    thread: Mutex<Option<JoinHandle<()>>>,
}

impl Drop for Inner {
    fn drop(&mut self) {
        let _ = self.tx.send(Command::Shutdown);
        if let Some(t) = self.thread.lock().ok().and_then(|mut g| g.take()) {
            let _ = t.join();
        }
    }
}

/// Ручка движка: дешёвая в клонировании, потокобезопасная. Все команды асинхронны —
/// результат приходит событиями.
#[derive(Clone)]
pub struct Engine {
    inner: Arc<Inner>,
}

impl Engine {
    /// Создаёт движок и открывает устройство вывода. На iOS аудиосессию нужно активировать
    /// до вызова (см. `apple::activate_audio_session`), иначе частота устройства может быть
    /// не той.
    pub fn new(config: EngineConfig) -> Result<(Engine, Receiver<Event>), AudioError> {
        let (tx, rx) = crossbeam_channel::unbounded();
        let (ev_tx, ev_rx) = crossbeam_channel::unbounded();
        let snapshot = Arc::new(Mutex::new(Snapshot {
            state: PlaybackState::Idle,
            index: None,
            position_ms: 0,
            duration_ms: None,
            volume: config.volume,
            transition: config.transition,
            queue_len: 0,
            device_rate: 0,
            device_channels: 0,
            underruns: 0,
            callback_load_avg: 0.0,
            callback_load_max: 0.0,
            buffer_frames: 0,
            output_frames: 0,
        }));
        let capture = Arc::new(Mutex::new(Vec::new()));
        // Устройство открывается в потоке контроллера (cpal::Stream не везде Send),
        // результат открытия возвращается сюда.
        let (ready_tx, ready_rx) = crossbeam_channel::bounded(1);
        let snap = snapshot.clone();
        let cap = capture.clone();
        let thread = std::thread::Builder::new()
            .name("dw-audio-ctl".into())
            .spawn(move || controller::run(config, rx, ev_tx, snap, cap, ready_tx))
            .map_err(|e| AudioError::Output(e.to_string()))?;
        match ready_rx.recv() {
            Ok(Ok(())) => {}
            Ok(Err(e)) => {
                let _ = thread.join();
                return Err(e);
            }
            Err(_) => return Err(AudioError::Output("controller thread died".into())),
        }
        let inner = Arc::new(Inner {
            tx,
            snapshot,
            capture,
            thread: Mutex::new(Some(thread)),
        });
        Ok((Engine { inner }, ev_rx))
    }

    fn send(&self, cmd: Command) {
        let _ = self.inner.tx.send(cmd);
    }

    pub fn load_queue(&self, tracks: Vec<TrackSpec>, start: usize, autoplay: bool) {
        self.send(Command::LoadQueue {
            tracks,
            start,
            autoplay,
        });
    }
    pub fn append(&self, tracks: Vec<TrackSpec>) {
        self.send(Command::Append(tracks));
    }
    pub fn play(&self) {
        self.send(Command::Play);
    }
    pub fn pause(&self) {
        self.send(Command::Pause);
    }
    pub fn toggle(&self) {
        self.send(Command::Toggle);
    }
    pub fn seek(&self, position_ms: u64) {
        self.send(Command::Seek(position_ms));
    }
    pub fn next(&self) {
        self.send(Command::Next);
    }
    pub fn previous(&self) {
        self.send(Command::Previous);
    }
    pub fn set_volume(&self, volume: f32) {
        self.send(Command::SetVolume(volume.clamp(0.0, 1.0)));
    }
    pub fn set_transition(&self, transition: Transition) {
        self.send(Command::SetTransition(transition));
    }
    pub fn stop(&self) {
        self.send(Command::Stop);
    }

    pub fn snapshot(&self) -> Snapshot {
        self.inner
            .snapshot
            .lock()
            .map(|s| s.clone())
            .unwrap_or_else(|p| p.into_inner().clone())
    }

    /// Тестовый режим: забирает накопленный выход (стерео, чередование каналов).
    pub fn take_capture(&self) -> Vec<f32> {
        self.inner
            .capture
            .lock()
            .map(|mut c| std::mem::take(&mut *c))
            .unwrap_or_default()
    }
}
