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
//! режим «Диджей» используют разные деки, которые сводит микшер.
//!
//! «Диджей»: движок заранее берёт анализ текущего и следующего трека ([`AnalysisSource`]),
//! выбирает переход (`dalywave_transitions::plan`), готовит входящий с нужной сильной доли,
//! растягивает его по карте темпа (Signalsmith Stretch) и стартует ровно на кадре сильной доли
//! уходящего; автоматику (громкости, эквалайзер, фильтры, эхо) ведёт микшер.
//!
//! Эквалайзер (10 полос, [`EqSettings`]) и таймер сна ([`SleepTimer`], затухание и пауза) — на
//! общем выходе микшера.
//!
//! Вывод — cpal, а на Apple, когда звук идёт в AirPlay, — буферизованный вывод
//! (AVSampleBufferAudioRenderer, `renderer.rs`): AirPlay 2 с буфером на стороне колонки и
//! несколькими колонками сразу. Он берёт звук на секунды вперёд, поэтому позиция, смена трека и
//! уровень в событиях — по тому, что слышно, а не по тому, что отдал микшер ([`route`]).
//!
//! Ссылки на файлы — подписанные и живут несколько часов. Если ссылка перестала работать посреди
//! трека или её ещё нет ([`SourceSpec::Deferred`]), движок просит новую у [`Resolver`] вызывающего
//! и продолжает с той же позиции. Кэш — в памяти (текущий, следующий и предыдущий треки);
//! кэш на диске появится вместе с загрузками для офлайна.

mod controller;
mod deck;
mod decode;
pub mod devices;
mod eq;
mod error;
mod fx;
mod meter;
mod mixer;
mod mp4;
mod output;
#[cfg(target_vendor = "apple")]
mod renderer;
mod resample;
pub mod route;
pub mod source;
mod stretch;
pub mod testkit;

#[cfg(target_vendor = "apple")]
pub mod apple;

use std::fmt;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

use crossbeam_channel::{Receiver, Sender};

pub use dalywave_transitions as transitions;
pub use dalywave_transitions::Analysis;
pub use eq::{EQ_BANDS, EQ_BANDS_HZ, EQ_MAX_DB, EqPreset, EqSettings};
pub use error::AudioError;
pub use mixer::equal_power;
#[doc(hidden)]
pub use output::FakeOutput;
pub use route::{BufferedOutput, RouteProbe};

/// Куда выводится звук.
#[derive(Clone, Debug)]
pub enum OutputConfig {
    /// Устройство вывода через cpal (CoreAudio, WASAPI). По умолчанию — системное: на macOS
    /// поток сам переходит на новое устройство по умолчанию (наушники, AirPods).
    Device {
        /// Желаемый размер буфера в кадрах; None — по умолчанию системы.
        buffer_frames: Option<u32>,
        /// Выбранное устройство ([`devices::OutputDevice::id`]); None — системное по умолчанию.
        /// Нет такого — играем на системном. Сменить на ходу — [`Engine::set_output_device`].
        device: Option<String>,
    },
    /// Тестовый режим: выход рендерится в память так быстро, как готовы данные,
    /// без недогрузок. Используется для проверки стыков.
    Capture { sample_rate: u32 },
    /// Проверки: вывод как у устройства — свой поток, реальное время, сторож вывода; умеет
    /// «зависать», как при зависшем аудиосервере ([`FakeOutput`]). Звук никуда не идёт.
    #[doc(hidden)]
    Fake { sample_rate: u32, fake: FakeOutput },
}

impl OutputConfig {
    /// Вывод через поток устройства: настоящее или проверочное.
    fn is_device(&self) -> bool {
        matches!(self, Self::Device { .. } | Self::Fake { .. })
    }
}

/// Режим перехода между треками.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Transition {
    Gapless,
    Crossfade {
        ms: u32,
    },
    /// Сведение, как в сете. `expressiveness` — 0…100: от незаметного до эффектного.
    Dj {
        expressiveness: u8,
    },
}

/// Что вернул [`Resolver`]: свежая ссылка на файл и данные для выравнивания громкости.
#[derive(Clone, Debug, Default)]
pub struct Resolved {
    pub url: String,
    pub loudness_lufs: Option<f32>,
    pub peak_db: Option<f32>,
    pub duration_ms: Option<u64>,
}

type AnalysisFn = dyn Fn(&TrackSpec) -> Result<Option<Analysis>, String> + Send + Sync;

/// Анализ трека для «Диджея» (сетка долей, фразы, вокал, энергия) по [`TrackSpec::stream_id`]:
/// приложение берёт его у сервера. `Ok(None)` — анализа нет (переход будет «Умным»).
/// Вызывается в фоновых потоках движка, может ходить в сеть.
#[derive(Clone)]
pub struct AnalysisSource(Arc<AnalysisFn>);

impl AnalysisSource {
    pub fn new(
        f: impl Fn(&TrackSpec) -> Result<Option<Analysis>, String> + Send + Sync + 'static,
    ) -> Self {
        Self(Arc::new(f))
    }

    pub fn fetch(&self, track: &TrackSpec) -> Result<Option<Analysis>, String> {
        (self.0)(track)
    }
}

impl fmt::Debug for AnalysisSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("AnalysisSource")
    }
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
    /// Анализ треков для «Диджея»; без него — [`TrackSpec::analysis`] или «Умный» переход.
    pub analysis: Option<AnalysisSource>,
    /// Проверки «Диджея»: во время перехода слышна только одна дека и без автоматики — так рендер
    /// даёт дорожки уходящего и входящего по отдельности (сверка долей по атакам).
    #[doc(hidden)]
    pub dj_solo: DjSolo,
    /// Эквалайзер с запуска (приложение помнит его между запусками); None — выключен.
    pub eq: Option<EqSettings>,
    /// Таймер сна: за сколько до конца звук затухает, мс.
    pub sleep_fade_ms: u32,
    /// Apple: когда выводить буферизованно (AirPlay 2). `Auto` — по маршруту: iOS — маршрут
    /// аудиосессии, macOS — тип выбранного устройства. Смена маршрута — [`Engine::route_changed`].
    pub buffered: BufferedOutput,
    /// Маршрут звука вместо системного (проверки — подставной). Для захвата без него маршрута нет.
    pub route: Option<RouteProbe>,
    /// Проверки: захват на буферизованном пути ведёт себя как он — слышно на столько позже, мс.
    #[doc(hidden)]
    pub capture_latency_ms: u32,
    /// Проверки: захват не быстрее стольких скоростей реального времени; 0 — как успевают деки.
    #[doc(hidden)]
    pub capture_speed: f32,
}

/// Какая дека слышна во время перехода (только для проверок).
#[doc(hidden)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DjSolo {
    #[default]
    Both,
    Outgoing,
    Incoming,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            output: OutputConfig::Device {
                buffer_frames: None,
                device: None,
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
            analysis: None,
            dj_solo: DjSolo::Both,
            eq: None,
            sleep_fade_ms: 15_000,
            buffered: BufferedOutput::Auto,
            route: None,
            capture_latency_ms: 2000,
            capture_speed: 0.0,
        }
    }
}

/// Таймер сна: когда остановить музыку. Последние секунды ([`EngineConfig::sleep_fade_ms`]) звук
/// плавно затухает, затем пауза.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SleepTimer {
    /// Через `ms` от команды (по часам, пауза его не останавливает).
    After { ms: u64 },
    /// В конце текущего трека; следующий встаёт в начало и ждёт на паузе.
    EndOfTrack,
}

/// Что с таймером сна.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SleepStatus {
    /// None — таймера нет.
    pub timer: Option<SleepTimer>,
    /// Сколько осталось до паузы, мс (для конца трека — по позиции, если длина известна).
    pub remaining_ms: Option<u64>,
    /// Звук уже затухает.
    pub fading: bool,
    /// Таймер сработал: музыка на паузе.
    pub finished: bool,
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
    /// Анализ для «Диджея», если он уже есть; иначе движок спросит [`EngineConfig::analysis`].
    pub analysis: Option<Arc<Analysis>>,
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
            analysis: None,
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
    /// «Диджей» выбрал переход к следующему треку (для журнала и проверок).
    TransitionPlanned {
        index: usize,
        next_index: usize,
        /// `blend`, `filter`, `echo`, `cut`, `fade` («Умный»)
        technique: &'static str,
        /// Позиция уходящего, где начинается переход, и входящего — где он вступает, мс.
        start_ms: f64,
        in_start_ms: f64,
        /// Длина автоматики и через сколько от её начала вступает входящий, мс.
        length_ms: f64,
        in_offset_ms: f64,
        /// Карта темпа входящего от его старта.
        tempo: dalywave_transitions::TempoMap,
        note: String,
    },
    /// Устройство вывода открыто заново (сменилось или пропало и вернулось).
    Output {
        sample_rate: u32,
        channels: u16,
        reason: String,
    },
    /// Таймер сна: поставили, сняли, началось затухание или сработал.
    Sleep(SleepStatus),
    /// Звук должен идти, а системный вывод не отвечает (аудиосервер завис, вывод не запускается
    /// или не берёт звук). Движок повторяет попытки с отступом (1, 2, 5, 10, 30 с) и продолжит с
    /// той же позиции. Приходит при каждой попытке: `attempt` — номер, `retry_in_ms` — до
    /// следующей.
    OutputStalled {
        attempt: u32,
        retry_in_ms: u64,
    },
    /// Ждать вывод больше не нужно: звук снова идёт или человек поставил паузу.
    OutputRecovered,
}

/// Снимок состояния для синхронных запросов (интерфейс, «Сейчас играет»).
#[derive(Clone, Debug)]
pub struct Snapshot {
    pub state: PlaybackState,
    pub index: Option<usize>,
    pub id: Option<String>,
    /// [`TrackSpec::stream_id`] того, что слышно (id трека на сервере): «Нравится» голосом
    pub stream_id: Option<String>,
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
    /// Устройство, к которому привязан поток ([`devices::OutputDevice::id`]); None — поток
    /// следует за устройством по умолчанию (или тестовый захват).
    pub output_device: Option<String>,
    pub eq: Option<EqSettings>,
    /// Таймер сна, если он стоит.
    pub sleep: Option<SleepStatus>,
    /// Повтор одного трека ([`Engine::set_repeat_one`]).
    pub repeat_one: bool,
    /// Есть куда перейти командой «следующий» / «предыдущий» по очереди движка (повтор не в счёт).
    pub has_next: bool,
    pub has_previous: bool,
    /// Вывод буферизованный (AirPlay 2): позиция и смена трека — по тому, что слышно.
    pub output_buffered: bool,
    /// Сколько звука отдано наперёд и ещё не слышно, мс (у cpal — 0).
    pub output_ahead_ms: u32,
    /// Системный вывод не отвечает, движок ждёт его ([`Event::OutputStalled`]).
    pub output_stalled: bool,
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
    SetOutputDevice(Option<String>),
    SetEq(Option<EqSettings>),
    SetSleep(Option<SleepTimer>),
    SetRepeatOne(bool),
    RouteChanged,
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
    ///
    /// Устройство не открылось (аудиосервер не отвечает, устройства нет) — движок всё равно
    /// создаётся: вывод откроется по команде play, а пока его нет — [`Event::OutputStalled`].
    pub fn new(config: EngineConfig) -> Result<(Engine, Receiver<Event>), AudioError> {
        let (tx, rx) = crossbeam_channel::unbounded();
        let (ev_tx, ev_rx) = crossbeam_channel::unbounded();
        let snapshot = Arc::new(Mutex::new(Snapshot {
            state: PlaybackState::Idle,
            index: None,
            id: None,
            stream_id: None,
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
            output_device: None,
            eq: config.eq,
            sleep: None,
            repeat_one: false,
            has_next: false,
            has_previous: false,
            output_buffered: false,
            output_ahead_ms: 0,
            output_stalled: false,
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
    /// Вывести звук на устройство ([`devices::output_devices`]); None — на системное по
    /// умолчанию, поток следует за ним. Вывод открывается заново, трек продолжается с той же
    /// позиции, пауза остаётся паузой. Пропадёт выбранное устройство — пауза и снова системное.
    pub fn set_output_device(&self, id: Option<String>) {
        self.send(Command::SetOutputDevice(id));
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
    /// Эквалайзер на общем выходе; None или плоский — звук идёт мимо, бит в бит. Смена плавная.
    pub fn set_eq(&self, eq: Option<EqSettings>) {
        self.send(Command::SetEq(eq));
    }
    /// Таймер сна; None — снять (затухание, если началось, отыгрывается обратно). Ход —
    /// событиями [`Event::Sleep`].
    pub fn set_sleep(&self, timer: Option<SleepTimer>) {
        self.send(Command::SetSleep(timer));
    }

    /// Повтор одного трека: в конце текущего движок сам начинает его заново — тот же файл
    /// готовится заранее и склеивается без паузы (или переходом, как обычный следующий).
    /// «Следующий» и «предыдущий» с пульта повтор пропускают.
    pub fn set_repeat_one(&self, on: bool) {
        self.send(Command::SetRepeatOne(on));
    }

    /// Маршрут звука сменился (iOS — уведомление аудиосессии, macOS — устройство по умолчанию):
    /// движок сам решает, нужен ли буферизованный вывод (AirPlay), и при смене открывает вывод
    /// заново с той же слышимой позиции.
    pub fn route_changed(&self) {
        self.send(Command::RouteChanged);
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
