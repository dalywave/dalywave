//! dalywave-audio — аудиодвижок приложений DalyWave (macOS, Windows, iOS, iPadOS).
//!
//! Поток данных:
//!
//! ```text
//! источник (HTTP с Range / файл) → symphonia (+libopus) → TrackDecoder (точная обрезка)
//!   → DeckWorker: громкость трека, ресемплинг rubato до частоты устройства
//!   → кольцевой буфер rtrb (SPSC, без блокировок)
//!   → Mixer в аудиоколбэке cpal: N дек, огибающие кроссфейда, общая громкость, измеритель уровня
//! ```
//!
//! «Без пауз» сделан цепочкой сегментов внутри одной деки: следующий трек декодируется в тот же
//! буфер сразу за предыдущим, ресемплер не сбрасывается — стык точен до сэмпла. Кроссфейд и
//! будущий режим «Диджей» используют разные деки, которые сводит микшер.
//!
//! Ссылки на файлы — подписанные и живут несколько часов. Если ссылка перестала работать посреди
//! трека или её ещё нет ([`SourceSpec::Deferred`]), движок просит новую у [`Resolver`] вызывающего
//! и продолжает с той же позиции. Кэш — в памяти (текущий, следующий и предыдущий треки);
//! кэш на диске появится вместе с загрузками для офлайна.

mod controller;
mod deck;
mod decode;
mod error;
mod meter;
mod mixer;
mod mp4;
mod output;
mod resample;
pub mod source;
pub mod testkit;

#[cfg(target_vendor = "apple")]
pub mod apple;

use std::fmt;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

use crossbeam_channel::{Receiver, Sender};

pub use error::AudioError;
pub use mixer::equal_power;

/// Куда выводится звук.
#[derive(Clone, Debug)]
pub enum OutputConfig {
    /// Системное устройство вывода по умолчанию через cpal (CoreAudio, WASAPI). На macOS поток
    /// сам переходит на новое устройство по умолчанию (наушники, AirPods).
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

/// Что вернул [`Resolver`]: свежая ссылка на файл и данные для выравнивания громкости.
#[derive(Clone, Debug, Default)]
pub struct Resolved {
    pub url: String,
    pub loudness_lufs: Option<f32>,
    pub peak_db: Option<f32>,
    pub duration_ms: Option<u64>,
}

type ResolveFn = dyn Fn(&TrackSpec) -> Result<Resolved, String> + Send + Sync;

/// Даёт ссылку на файл трека по [`TrackSpec::stream_id`]: когда ссылки ещё нет
/// ([`SourceSpec::Deferred`]) и когда старая перестала действовать (истёк срок подписи —
/// сервер хранилища отвечает 401/403/410). Вызывается в фоновых потоках движка, может ходить
/// в сеть и блокировать.
#[derive(Clone)]
pub struct Resolver(Arc<ResolveFn>);

impl Resolver {
    pub fn new(f: impl Fn(&TrackSpec) -> Result<Resolved, String> + Send + Sync + 'static) -> Self {
        Self(Arc::new(f))
    }

    pub fn resolve(&self, track: &TrackSpec) -> Result<Resolved, String> {
        (self.0)(track)
    }
}

impl fmt::Debug for Resolver {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Resolver")
    }
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
    pub resolver: Option<Resolver>,
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
            resolver: None,
        }
    }
}

/// Откуда брать байты трека.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SourceSpec {
    Url(String),
    File(PathBuf),
    /// Ссылку даст [`Resolver`], когда трек понадобится (очередь волны: ссылки живут
    /// несколько часов, просить их для всей очереди заранее незачем).
    Deferred,
}

/// Трек в очереди. Метаданные приходят от вызывающего (с сервера), движок их не угадывает.
#[derive(Clone, Debug)]
pub struct TrackSpec {
    /// Ключ элемента очереди: возвращается в событиях как есть.
    pub id: String,
    pub source: SourceSpec,
    /// По нему [`Resolver`] находит трек на сервере; без него ссылку обновить нельзя.
    pub stream_id: Option<String>,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub artwork_url: Option<String>,
    pub duration_ms: Option<u64>,
    /// Интегральная громкость трека (EBU R128), LUFS.
    pub loudness_lufs: Option<f32>,
    /// Истинный пик трека, dBFS: усиление при нормализации не поднимет пик выше −1 dBFS.
    pub peak_db: Option<f32>,
}

impl TrackSpec {
    pub fn url(id: impl Into<String>, url: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            source: SourceSpec::Url(url.into()),
            stream_id: None,
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

    /// Усиление нормализации к `target` LUFS: как на сайте — от −12 до +6 дБ и не выше −1 dBFS
    /// по истинному пику.
    pub fn gain_for(&self, target_lufs: f32) -> f32 {
        let Some(lufs) = self.loudness_lufs else {
            return 1.0;
        };
        let mut db = (target_lufs - lufs).clamp(-12.0, 6.0);
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
    /// Сам перешёл к следующему треку (без пауз, кроссфейдом или пропустив сбойный).
    Auto,
    /// Команда: загрузка очереди, next, previous.
    User,
}

/// Уровень звука за последние ~50 мс (до общей громкости: волна интерфейса движется и при
/// тихой громкости). Полосы — RMS после простых фильтров: до 150 Гц, 150–2000 Гц, выше 2 кГц.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Level {
    pub rms: f32,
    pub peak: f32,
    pub bands: [f32; 3],
}

#[derive(Clone, Debug)]
pub enum Event {
    State(PlaybackState),
    /// Каждые 250 мс во время воспроизведения и сразу после перемотки и паузы.
    Position {
        index: usize,
        id: String,
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
        id: Option<String>,
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
    /// ~20 раз в секунду во время воспроизведения, если включено [`Engine::set_levels`].
    Level(Level),
    /// Устройство вывода открыто заново (сменилось или пропало и вернулось).
    Output {
        sample_rate: u32,
        channels: u16,
        reason: String,
    },
}

/// Снимок состояния для синхронных запросов (интерфейс, «Сейчас играет»).
#[derive(Clone, Debug)]
pub struct Snapshot {
    pub state: PlaybackState,
    pub index: Option<usize>,
    pub id: Option<String>,
    pub position_ms: u64,
    pub duration_ms: Option<u64>,
    pub volume: f32,
    pub transition: Transition,
    pub queue_len: usize,
    pub buffering: bool,
    pub device_rate: u32,
    pub device_channels: u16,
    pub underruns: u64,
    /// Средняя и максимальная загрузка аудиоколбэка: время обработки / длительность буфера.
    pub callback_load_avg: f32,
    pub callback_load_max: f32,
    pub buffer_frames: u32,
    /// Сколько кадров отдал микшер с запуска (растёт, только пока устройство запрашивает звук).
    pub output_frames: u64,
    /// Сколько раз вывод открывался заново.
    pub output_reopens: u32,
}

pub(crate) enum Command {
    LoadQueue {
        tracks: Vec<TrackSpec>,
        start: usize,
        start_ms: u64,
        autoplay: bool,
    },
    Append(Vec<TrackSpec>),
    SetUpcoming(Vec<TrackSpec>),
    Play,
    Pause,
    Toggle,
    Seek(u64),
    Next,
    Previous,
    SetVolume(f32),
    SetTransition(Transition),
    SetNormalize(bool),
    SetLevels(bool),
    ReopenOutput {
        sample_rate: Option<u32>,
        reason: String,
    },
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
    /// Создаёт движок и открывает устройство вывода (на macOS первый раз — до 200 мс, поэтому
    /// не на главном потоке). На iOS аудиосессию нужно активировать до вызова
    /// (`apple::activate_audio_session`), иначе частота устройства может быть не той.
    pub fn new(config: EngineConfig) -> Result<(Engine, Receiver<Event>), AudioError> {
        let (tx, rx) = crossbeam_channel::unbounded();
        let (ev_tx, ev_rx) = crossbeam_channel::unbounded();
        let snapshot = Arc::new(Mutex::new(Snapshot {
            state: PlaybackState::Idle,
            index: None,
            id: None,
            position_ms: 0,
            duration_ms: None,
            volume: config.volume,
            transition: config.transition,
            queue_len: 0,
            buffering: false,
            device_rate: 0,
            device_channels: 0,
            underruns: 0,
            callback_load_avg: 0.0,
            callback_load_max: 0.0,
            buffer_frames: 0,
            output_frames: 0,
            output_reopens: 0,
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
        self.load_queue_at(tracks, start, 0, autoplay);
    }

    /// Новая очередь, первый трек — с позиции `start_ms`.
    pub fn load_queue_at(
        &self,
        tracks: Vec<TrackSpec>,
        start: usize,
        start_ms: u64,
        autoplay: bool,
    ) {
        self.send(Command::LoadQueue {
            tracks,
            start,
            start_ms,
            autoplay,
        });
    }

    pub fn append(&self, tracks: Vec<TrackSpec>) {
        self.send(Command::Append(tracks));
    }

    /// Заменить всё, что после текущего трека. Уже подготовленный следующий трек остаётся,
    /// если он стоит первым и в новом списке (по `id`).
    pub fn set_upcoming(&self, tracks: Vec<TrackSpec>) {
        self.send(Command::SetUpcoming(tracks));
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
    /// Выравнивание громкости по LUFS; действует со следующего подготовленного трека.
    pub fn set_normalize(&self, on: bool) {
        self.send(Command::SetNormalize(on));
    }
    /// События [`Event::Level`] (для волны интерфейса): включать, только пока их кто-то рисует.
    pub fn set_levels(&self, on: bool) {
        self.send(Command::SetLevels(on));
    }
    /// Открыть устройство вывода заново и продолжить с той же позиции. Движок делает это сам,
    /// когда система сообщает, что поток больше не годится; вызывать вручную — например, после
    /// сброса медиасервисов iOS.
    pub fn reopen_output(&self, reason: &str) {
        self.send(Command::ReopenOutput {
            sample_rate: None,
            reason: reason.into(),
        });
    }
    /// Тестовый режим: открыть захват заново с другой частотой — как смена устройства.
    pub fn reopen_capture(&self, sample_rate: u32) {
        self.send(Command::ReopenOutput {
            sample_rate: Some(sample_rate),
            reason: "capture".into(),
        });
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
