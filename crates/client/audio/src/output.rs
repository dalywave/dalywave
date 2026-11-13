//! Вывод: устройство через cpal, буферизованный вывод Apple (AirPlay 2, renderer.rs) или тестовый
//! захват в память.
//!
//! В колбэке cpal работает только микшер (без выделений и блокировок) и раскладка стерео на
//! каналы устройства. Загрузка колбэка (время обработки / длительность буфера) копится в атомиках.
//!
//! Ошибки потока cpal приходят контроллеру как [`OutputIssue`]: сменилось устройство (поток
//! переехал сам), устройство пропало, поток больше не годится и его надо открыть заново.
//!
//! Устройство открывается, запускается и останавливается в своём потоке ([`DeviceLink`]): когда
//! аудиосервер системы завис (coreaudiod, медиасервер iOS), его вызовы не возвращаются секундами,
//! и ждать их должен не контроллер. Контроллер только отдаёт команды и видит, что поток занят.

use std::panic::AssertUnwindSafe;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock, PoisonError};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use crossbeam_channel::{RecvTimeoutError, Sender};

use crate::AudioError;
use crate::mixer::{Mixer, MixerPorts};

/// Максимальный блок микшера за один проход; больший буфер устройства режется на части.
const MAX_BLOCK: usize = 4096;
/// Сколько ждать, пока поток устройства откроет вывод: дольше — аудиосервер не отвечает.
const OPEN_TIMEOUT: Duration = Duration::from_secs(4);
/// Сколько [`Output::halt`] ждёт остановки вывода.
const HALT_WAIT: Duration = Duration::from_millis(300);

#[derive(Default)]
pub(crate) struct LoadStats {
    avg: AtomicU32,
    max: AtomicU32,
    pub buffer_frames: AtomicU32,
}

impl LoadStats {
    fn record(&self, load: f32, frames: u32) {
        let avg = f32::from_bits(self.avg.load(Ordering::Relaxed));
        let avg = if avg == 0.0 {
            load
        } else {
            avg * 0.99 + load * 0.01
        };
        self.avg.store(avg.to_bits(), Ordering::Relaxed);
        if load > f32::from_bits(self.max.load(Ordering::Relaxed)) {
            self.max.store(load.to_bits(), Ordering::Relaxed);
        }
        self.buffer_frames.store(frames, Ordering::Relaxed);
    }

    pub fn get(&self) -> (f32, f32) {
        (
            f32::from_bits(self.avg.load(Ordering::Relaxed)),
            f32::from_bits(self.max.load(Ordering::Relaxed)),
        )
    }
}

pub(crate) struct DeviceInfo {
    pub sample_rate: u32,
    pub channels: u16,
}

/// Что открыл поток устройства: частота и каналы, порты микшера, выбранное устройство (если
/// открыто оно).
pub(crate) struct Opened {
    pub info: DeviceInfo,
    pub ports: MixerPorts,
    pub device: Option<String>,
}

/// Что система сообщила о потоке вывода.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum OutputIssue {
    /// Система перевела поток на другое устройство сама (наушники, AirPods) — звук идёт.
    Rerouted(String),
    /// Устройства больше нет: поток остановлен.
    Lost(String),
    /// Поток больше не годится (сменилась частота, сброс медиасервисов iOS) — открыть заново.
    Invalidated(String),
    /// Буферизованный вывод не работает (рендерер в состоянии ошибки) — открыть обычный.
    #[cfg_attr(not(target_vendor = "apple"), allow(dead_code))]
    BufferedFailed(String),
    /// Вывод не запустился (система отказала в запуске) — повторить позже.
    StartFailed(String),
    Other(String),
}

impl OutputIssue {
    fn from_cpal(e: &cpal::Error) -> Self {
        let msg = e.to_string();
        match e.kind() {
            cpal::ErrorKind::DeviceChanged => Self::Rerouted(msg),
            cpal::ErrorKind::DeviceNotAvailable => Self::Lost(msg),
            cpal::ErrorKind::StreamInvalidated => Self::Invalidated(msg),
            _ => Self::Other(msg),
        }
    }
}

/// Проверки: захват, который ведёт себя как буферизованный вывод — слышно на `latency` кадров
/// позже, чем отдал микшер; сброс очереди — слышимое сразу догоняет микшер.
pub(crate) struct SimLag {
    latency: u64,
    heard: AtomicU64,
    flush: AtomicBool,
}

pub(crate) enum Output {
    /// Устройство (cpal) или проверочный вывод, который ведёт себя как оно. `running` — запуск
    /// запрошен: сам запуск идёт в потоке устройства и мог ещё не случиться.
    Device { link: DeviceLink, running: bool },
    /// AVSampleBufferAudioRenderer: звук уходит наперёд, слышно — по времени синхронизатора.
    #[cfg(target_vendor = "apple")]
    Buffered(crate::renderer::Renderer),
    Capture {
        stop: Arc<AtomicBool>,
        thread: Option<JoinHandle<()>>,
        sim: Option<Arc<SimLag>>,
    },
    /// Устройства нет (пропало и не вернулось): откроем заново по команде play.
    None,
}

/// Устройство вывода: выбранное по id или, если его нет (`None` или пропало), — по умолчанию.
/// Второе в ответе — id, если открыто выбранное: поток к нему привязан и за устройством по
/// умолчанию не следует.
pub(crate) fn query_device(
    preferred: Option<&str>,
) -> Result<(cpal::Device, cpal::SupportedStreamConfig, Option<String>), AudioError> {
    let host = cpal::default_host();
    if let Some(id) = preferred {
        let found = id
            .parse::<cpal::DeviceId>()
            .ok()
            .and_then(|parsed| host.device_by_id(&parsed));
        match found.map(|d| d.default_output_config().map(|c| (d, c))) {
            Some(Ok((device, config))) => return Ok((device, config, Some(id.to_string()))),
            Some(Err(e)) => log::warn!("output device {id} is not usable: {e}; using the default"),
            None => log::warn!("output device {id} is gone; using the default"),
        }
    }
    let device = host
        .default_output_device()
        .ok_or_else(|| AudioError::Output("нет устройства вывода".into()))?;
    let config = device
        .default_output_config()
        .map_err(|e| AudioError::Output(e.to_string()))?;
    Ok((device, config, None))
}

impl Output {
    /// Устройство через cpal. Открытие, запуск и остановка идут в своём потоке ([`DeviceLink`]):
    /// если аудиосервер системы завис, застревает этот поток, а не контроллер.
    #[allow(clippy::too_many_arguments)]
    pub fn open_device(
        preferred: Option<String>,
        buffer_frames: Option<u32>,
        start_threshold_ms: u32,
        volume: f32,
        load: Arc<LoadStats>,
        errors: Sender<OutputIssue>,
    ) -> Result<(Output, Opened), AudioError> {
        spawn_device(
            Box::new(move |errors| {
                let (device, supported, device_id) = query_device(preferred.as_deref())?;
                let rate = supported.sample_rate();
                let channels = supported.channels();
                let config = cpal::StreamConfig {
                    channels,
                    sample_rate: rate,
                    buffer_size: match buffer_frames {
                        Some(n) => cpal::BufferSize::Fixed(n),
                        None => cpal::BufferSize::Default,
                    },
                };
                let (mixer, ports) = Mixer::new(rate, start_threshold_ms, volume);
                let mut render =
                    render_callback(mixer, channels as usize, rate, load, errors.clone());
                let stream = device
                    .build_output_stream::<f32, _, _>(
                        config,
                        move |data: &mut [f32], _info: &cpal::OutputCallbackInfo| render(data),
                        move |err| {
                            let _ = errors.send(OutputIssue::from_cpal(&err));
                        },
                        None,
                    )
                    .map_err(|e| AudioError::Output(e.to_string()))?;
                // Поток создан остановленным: запустим по первой команде play.
                let _ = stream.pause();
                Ok((
                    Box::new(CpalStream(stream)) as Box<dyn DeviceStream>,
                    Opened {
                        info: DeviceInfo {
                            sample_rate: rate,
                            channels,
                        },
                        ports,
                        device: device_id,
                    },
                ))
            }),
            errors,
        )
    }

    /// Проверки: вывод как у устройства (свой поток, реальное время), который умеет зависать.
    pub fn open_fake(
        fake: FakeOutput,
        rate: u32,
        start_threshold_ms: u32,
        volume: f32,
        load: Arc<LoadStats>,
        errors: Sender<OutputIssue>,
    ) -> Result<(Output, Opened), AudioError> {
        spawn_device(
            Box::new(move |errors| {
                if fake
                    .0
                    .failing_opens
                    .try_update(Ordering::AcqRel, Ordering::Acquire, |n| n.checked_sub(1))
                    .is_ok()
                {
                    return Err(AudioError::Output(
                        "нет устройства вывода (проверка)".into(),
                    ));
                }
                let (mixer, ports) = Mixer::new(rate, start_threshold_ms, volume);
                let render = render_callback(mixer, 2, rate, load, errors);
                Ok((
                    Box::new(FakeStream {
                        fake,
                        rate,
                        render: Arc::new(Mutex::new(Box::new(render))),
                        running: Arc::new(AtomicBool::new(false)),
                        thread: Mutex::new(None),
                    }) as Box<dyn DeviceStream>,
                    Opened {
                        info: DeviceInfo {
                            sample_rate: rate,
                            channels: 2,
                        },
                        ports,
                        device: None,
                    },
                ))
            }),
            errors,
        )
    }

    /// Буферизованный вывод Apple. Громкость — у рендерера: в микшере она была бы слышна позже.
    #[cfg(target_vendor = "apple")]
    pub fn open_buffered(
        mixer: Mixer,
        rate: u32,
        volume: f32,
        device_uid: Option<String>,
        load: Arc<LoadStats>,
        errors: Sender<OutputIssue>,
    ) -> Result<Output, AudioError> {
        crate::renderer::Renderer::open(mixer, rate, volume, device_uid, load, errors)
            .map(Output::Buffered)
    }

    /// `latency_frames` > 0 — захват ведёт себя как буферизованный вывод; `speed` > 0 — рендер не
    /// быстрее стольких скоростей реального времени (проверки, которым нужно время между событиями).
    pub fn open_capture(
        mut mixer: Mixer,
        capture: Arc<Mutex<Vec<f32>>>,
        latency_frames: u64,
        rate: u32,
        speed: f32,
    ) -> Output {
        let stop = Arc::new(AtomicBool::new(false));
        let sim = (latency_frames > 0).then(|| {
            Arc::new(SimLag {
                latency: latency_frames,
                heard: AtomicU64::new(0),
                flush: AtomicBool::new(false),
            })
        });
        let lag = sim.clone();
        let s = stop.clone();
        let thread = std::thread::Builder::new()
            .name("dw-capture".into())
            .spawn(move || {
                const BLOCK: usize = 1024;
                let mut block = vec![0f32; BLOCK * 2];
                // Часы микшера и нижняя граница слышимого после сброса
                let mut clock = 0u64;
                let mut floor = 0u64;
                let started = Instant::now();
                while !s.load(Ordering::Relaxed) {
                    if speed > 0.0
                        && clock as f64 / f64::from(rate)
                            > started.elapsed().as_secs_f64() * f64::from(speed)
                    {
                        std::thread::sleep(Duration::from_millis(1));
                        continue;
                    }
                    if let Some(lag) = &lag
                        && lag.flush.swap(false, Ordering::AcqRel)
                    {
                        floor = clock;
                    }
                    // В тестовом режиме недогрузок не бывает: ждём, пока у дек будут данные.
                    if !mixer.ready_for(BLOCK) {
                        std::thread::sleep(Duration::from_micros(500));
                        continue;
                    }
                    let was_active = !mixer.is_idle() && mixer.has_playing();
                    mixer.render(&mut block);
                    clock += BLOCK as u64;
                    let now_active = !mixer.is_idle() && mixer.has_playing();
                    if let Some(lag) = &lag {
                        // Ничего не звучит — очередь «доиграла»: слышно всё отданное
                        let heard = if was_active || now_active {
                            clock.saturating_sub(lag.latency).max(floor)
                        } else {
                            clock
                        };
                        lag.heard.store(heard, Ordering::Release);
                    }
                    if was_active || now_active {
                        if let Ok(mut c) = capture.lock() {
                            c.extend_from_slice(&block);
                        }
                    } else {
                        std::thread::sleep(Duration::from_millis(1));
                    }
                }
            })
            .ok();
        Output::Capture { stop, thread, sim }
    }

    pub fn start(&mut self) {
        #[cfg(target_vendor = "apple")]
        if let Output::Buffered(r) = self {
            r.set_playing(true);
            return;
        }
        if let Output::Device { link, running } = self {
            // Всегда зовём play: на iOS систему может остановить поток сама (прерывание звонком),
            // и наш флаг об этом не знает; cpal ведёт своё состояние и повторный play безвреден.
            // Запуск идёт в потоке устройства: зависший аудиосервер не держит контроллер.
            link.send(DeviceCmd::Play);
            *running = true;
        }
    }

    pub fn stop(&mut self) {
        #[cfg(target_vendor = "apple")]
        if let Output::Buffered(r) = self {
            r.set_playing(false);
            return;
        }
        if let Output::Device { link, running } = self
            && *running
        {
            link.send(DeviceCmd::Pause);
            *running = false;
        }
    }

    /// Остановить совсем, перед тем как открыть вывод заново: после возврата микшер больше
    /// не вызывается, и счётчики позиции не растут.
    pub fn halt(&mut self) {
        match self {
            Output::Device { link, running } => {
                link.halt();
                *running = false;
            }
            #[cfg(target_vendor = "apple")]
            Output::Buffered(r) => r.halt(),
            Output::Capture { stop, thread, .. } => {
                stop.store(true, Ordering::Relaxed);
                if let Some(t) = thread.take() {
                    let _ = t.join();
                }
            }
            Output::None => {}
        }
    }

    pub fn is_running(&self) -> bool {
        match self {
            Output::Device { running, .. } => *running,
            #[cfg(target_vendor = "apple")]
            Output::Buffered(r) => r.is_playing(),
            Output::Capture { .. } => true,
            Output::None => false,
        }
    }

    /// Поток устройства посреди системного вызова (открытие, запуск, остановка) — столько времени.
    pub fn busy_for(&self) -> Option<Duration> {
        match self {
            Output::Device { link, .. } => link.worker.busy_for(),
            _ => None,
        }
    }

    /// Звук уходит наперёд: слышно позже, чем отдал микшер.
    pub fn is_buffered(&self) -> bool {
        match self {
            #[cfg(target_vendor = "apple")]
            Output::Buffered(_) => true,
            Output::Capture { sim, .. } => sim.is_some(),
            _ => false,
        }
    }

    /// Кадр часов микшера, который слышен сейчас (`clock` — часы микшера). У cpal — сами часы:
    /// его буфер — миллисекунды.
    pub fn audible(&self, clock: u64) -> u64 {
        match self {
            #[cfg(target_vendor = "apple")]
            Output::Buffered(r) => r.heard().min(clock),
            Output::Capture { sim: Some(s), .. } => s.heard.load(Ordering::Acquire).min(clock),
            _ => clock,
        }
    }

    /// Сбросить поставленное наперёд (перемотка, другой трек по команде).
    pub fn flush(&self) {
        match self {
            #[cfg(target_vendor = "apple")]
            Output::Buffered(r) => r.flush(),
            Output::Capture { sim: Some(s), .. } => s.flush.store(true, Ordering::Release),
            _ => {}
        }
    }

    /// Громкость на выходе, если вывод её ведёт сам (буферизованный — сразу, а не через секунды
    /// очереди). `false` — громкость ведёт микшер.
    pub fn set_volume(&self, volume: f32) -> bool {
        match self {
            #[cfg(target_vendor = "apple")]
            Output::Buffered(r) => {
                r.set_volume(volume);
                true
            }
            _ => {
                let _ = volume;
                false
            }
        }
    }
}

impl Drop for Output {
    fn drop(&mut self) {
        if let Output::Capture { .. } = self {
            self.halt();
        }
    }
}

// ---- поток устройства ----

/// Потоков устройства, брошенных посреди системного вызова (аудиосервер так и не ответил). Пока
/// таких много, новых попыток открыть вывод не делаем: они застряли бы так же.
static ABANDONED: AtomicU32 = AtomicU32::new(0);

pub(crate) fn abandoned_threads() -> u32 {
    ABANDONED.load(Ordering::Relaxed)
}

#[derive(Default)]
struct WorkerState {
    busy_since: Option<Instant>,
    abandoned: bool,
}

/// Состояние потока устройства: идёт ли системный вызов и с какого момента.
#[derive(Default)]
struct Worker(Mutex<WorkerState>);

impl Worker {
    fn state(&self) -> MutexGuard<'_, WorkerState> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Системный вызов в потоке устройства: пока он идёт, поток «занят».
    fn call<T>(&self, f: impl FnOnce() -> T) -> T {
        self.state().busy_since = Some(Instant::now());
        let r = f();
        let mut s = self.state();
        s.busy_since = None;
        if std::mem::take(&mut s.abandoned) {
            ABANDONED.fetch_sub(1, Ordering::Relaxed);
        }
        r
    }

    fn busy_for(&self) -> Option<Duration> {
        self.state().busy_since.map(|t| t.elapsed())
    }

    /// Контроллер больше не ждёт этот поток: застрял посреди вызова — считаем брошенным, пока
    /// вызов не вернётся.
    fn abandon(&self) {
        let mut s = self.state();
        if s.busy_since.is_some() && !s.abandoned {
            s.abandoned = true;
            ABANDONED.fetch_add(1, Ordering::Relaxed);
        }
    }
}

enum DeviceCmd {
    Play,
    Pause,
    /// Остановить и ответить: после ответа колбэк вывода больше не вызывается.
    Halt(Sender<()>),
}

/// Ручка потока устройства: команды уходят без ожидания (кроме [`DeviceLink::halt`]). Ручку
/// уронили — поток закрывает вывод и выходит, даже если сейчас застрял в системе: когда вызов
/// вернётся.
pub(crate) struct DeviceLink {
    cmds: Sender<DeviceCmd>,
    worker: Arc<Worker>,
}

impl DeviceLink {
    fn send(&self, cmd: DeviceCmd) {
        let _ = self.cmds.send(cmd);
    }

    fn halt(&self) {
        let (tx, rx) = crossbeam_channel::bounded(1);
        self.send(DeviceCmd::Halt(tx));
        if rx.recv_timeout(HALT_WAIT).is_err() {
            log::warn!("output: the device thread is stuck in the system audio, leaving it");
            self.worker.abandon();
        }
    }
}

/// Поток вывода внутри потока устройства: cpal или проверочный.
trait DeviceStream {
    fn play(&self) -> Result<(), String>;
    fn pause(&self) -> Result<(), String>;
}

struct CpalStream(cpal::Stream);

impl DeviceStream for CpalStream {
    fn play(&self) -> Result<(), String> {
        if let Some(hang) = injected_hang() {
            log::warn!("output: DALYWAVE_AUDIO_FAULT — the start hangs for {hang:?}");
            std::thread::sleep(hang);
        }
        self.0.play().map_err(|e| e.to_string())
    }

    fn pause(&self) -> Result<(), String> {
        self.0.pause().map_err(|e| e.to_string())
    }
}

/// Проверка без зависшего аудиосервера (симулятор, устройство): `DALYWAVE_AUDIO_FAULT=hang:<с>` —
/// первый запуск вывода не возвращается столько секунд, как при зависшем аудиосервере.
fn injected_hang() -> Option<Duration> {
    static HANG: OnceLock<Option<Duration>> = OnceLock::new();
    static USED: AtomicBool = AtomicBool::new(false);
    let hang = (*HANG.get_or_init(|| {
        let v = std::env::var("DALYWAVE_AUDIO_FAULT").ok()?;
        let secs = v.strip_prefix("hang:")?.parse::<f64>().ok()?;
        Some(Duration::from_secs_f64(secs.max(0.0)))
    }))?;
    (!USED.swap(true, Ordering::Relaxed)).then_some(hang)
}

type OpenFn = Box<
    dyn FnOnce(Sender<OutputIssue>) -> Result<(Box<dyn DeviceStream>, Opened), AudioError> + Send,
>;

/// Запустить поток устройства и дождаться открытия, но не дольше [`OPEN_TIMEOUT`]: аудиосервер
/// не отвечает — ошибка, а застрявший поток закроет вывод сам, когда вызов вернётся.
fn spawn_device(open: OpenFn, errors: Sender<OutputIssue>) -> Result<(Output, Opened), AudioError> {
    let worker = Arc::new(Worker::default());
    let (ready_tx, ready_rx) = crossbeam_channel::bounded(1);
    let (cmd_tx, cmd_rx) = crossbeam_channel::unbounded::<DeviceCmd>();
    let w = worker.clone();
    std::thread::Builder::new()
        .name("dw-audio-device".into())
        .spawn(move || {
            let stream = match w.call(|| open(errors.clone())) {
                Ok((stream, opened)) => {
                    if ready_tx.send(Ok(opened)).is_err() {
                        w.call(|| drop(stream));
                        return;
                    }
                    stream
                }
                Err(e) => {
                    let _ = ready_tx.send(Err(e));
                    return;
                }
            };
            for cmd in cmd_rx.iter() {
                match cmd {
                    DeviceCmd::Play => {
                        if let Err(e) = w.call(|| stream.play()) {
                            log::warn!("stream play: {e}");
                            let _ = errors.send(OutputIssue::StartFailed(e));
                        }
                    }
                    DeviceCmd::Pause => {
                        if let Err(e) = w.call(|| stream.pause()) {
                            log::debug!("stream pause: {e}");
                        }
                    }
                    DeviceCmd::Halt(ack) => {
                        let _ = w.call(|| stream.pause());
                        let _ = ack.send(());
                    }
                }
            }
            w.call(|| drop(stream));
        })
        .map_err(|e| AudioError::Output(e.to_string()))?;
    match ready_rx.recv_timeout(OPEN_TIMEOUT) {
        Ok(Ok(opened)) => Ok((
            Output::Device {
                link: DeviceLink {
                    cmds: cmd_tx,
                    worker,
                },
                running: false,
            },
            opened,
        )),
        Ok(Err(e)) => Err(e),
        Err(RecvTimeoutError::Timeout) => {
            worker.abandon();
            Err(AudioError::Output(
                "системный звук не отвечает: устройство не открылось".into(),
            ))
        }
        Err(RecvTimeoutError::Disconnected) => {
            Err(AudioError::Output("поток устройства завершился".into()))
        }
    }
}

/// Колбэк вывода: микшер и раскладка стерео на каналы устройства, загрузка — в атомики.
/// Паника внутри не уходит в систему (через границу FFI это abort): дальше тишина, а контроллер
/// открывает вывод заново. В сборке с `panic = "abort"` это не спасает — там паника и так abort.
fn render_callback(
    mut mixer: Mixer,
    ch: usize,
    rate: u32,
    load: Arc<LoadStats>,
    errors: Sender<OutputIssue>,
) -> impl FnMut(&mut [f32]) + Send + 'static {
    let mut scratch = vec![0f32; MAX_BLOCK * 2];
    let mut broken = false;
    move |data: &mut [f32]| {
        if broken {
            data.fill(0.0);
            return;
        }
        let t0 = Instant::now();
        let frames = data.len() / ch.max(1);
        let rendered = std::panic::catch_unwind(AssertUnwindSafe(|| {
            render_frames(&mut mixer, &mut scratch, data, ch, frames);
        }));
        if rendered.is_err() {
            broken = true;
            data.fill(0.0);
            let _ = errors.send(OutputIssue::Invalidated("audio callback panicked".into()));
            return;
        }
        let budget = frames as f32 / rate as f32;
        if budget > 0.0 {
            load.record(t0.elapsed().as_secs_f32() / budget, frames as u32);
        }
    }
}

fn render_frames(
    mixer: &mut Mixer,
    scratch: &mut [f32],
    data: &mut [f32],
    ch: usize,
    frames: usize,
) {
    let mut done = 0;
    while done < frames {
        let n = (frames - done).min(MAX_BLOCK);
        let block = &mut scratch[..n * 2];
        mixer.render(block);
        let out = &mut data[done * ch..(done + n) * ch];
        match ch {
            2 => out.copy_from_slice(block),
            1 => {
                for (o, p) in out.iter_mut().zip(block.as_chunks::<2>().0) {
                    *o = 0.5 * (p[0] + p[1]);
                }
            }
            _ => {
                for (o, p) in out.chunks_exact_mut(ch).zip(block.as_chunks::<2>().0) {
                    o[0] = p[0];
                    o[1] = p[1];
                    for x in &mut o[2..] {
                        *x = 0.0;
                    }
                }
            }
        }
        done += n;
    }
}

// ---- проверочный вывод ----

/// Проверки: вывод «как устройство» — свой поток, звук берёт в реальном времени и умеет вести
/// себя как при зависшем аудиосервере: запуск не возвращается, колбэки не приходят.
#[doc(hidden)]
#[derive(Clone, Debug, Default)]
pub struct FakeOutput(Arc<FakeState>);

#[derive(Debug, Default)]
struct FakeState {
    hung: AtomicBool,
    failing_opens: AtomicU32,
}

impl FakeOutput {
    /// Аудиосервер завис (`true`) или ожил (`false`).
    pub fn set_hung(&self, hung: bool) {
        self.0.hung.store(hung, Ordering::Release);
    }

    /// Следующие `n` открытий вывода кончатся ошибкой.
    pub fn fail_opens(&self, n: u32) {
        self.0.failing_opens.store(n, Ordering::Release);
    }

    fn hung(&self) -> bool {
        self.0.hung.load(Ordering::Acquire)
    }
}

type RenderFn = Box<dyn FnMut(&mut [f32]) + Send>;

struct FakeStream {
    fake: FakeOutput,
    rate: u32,
    render: Arc<Mutex<RenderFn>>,
    running: Arc<AtomicBool>,
    thread: Mutex<Option<JoinHandle<()>>>,
}

impl DeviceStream for FakeStream {
    fn play(&self) -> Result<(), String> {
        // Запуск «висит», пока аудиосервер не оживёт
        while self.fake.hung() {
            std::thread::sleep(Duration::from_millis(5));
        }
        if self.running.swap(true, Ordering::AcqRel) {
            return Ok(());
        }
        let (fake, render, running) =
            (self.fake.clone(), self.render.clone(), self.running.clone());
        // По блоку в 10 мс раз в 10 мс — как устройство
        let block = (self.rate / 100).max(1) as usize;
        let thread = std::thread::Builder::new()
            .name("dw-fake-output".into())
            .spawn(move || {
                let mut buf = vec![0f32; block * 2];
                while running.load(Ordering::Acquire) {
                    if !fake.hung()
                        && let Ok(mut r) = render.lock()
                    {
                        r(&mut buf);
                    }
                    std::thread::sleep(Duration::from_millis(10));
                }
            })
            .map_err(|e| e.to_string())?;
        *self.thread.lock().unwrap_or_else(PoisonError::into_inner) = Some(thread);
        Ok(())
    }

    fn pause(&self) -> Result<(), String> {
        self.running.store(false, Ordering::Release);
        let thread = self
            .thread
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take();
        if let Some(t) = thread {
            let _ = t.join();
        }
        Ok(())
    }
}

impl Drop for FakeStream {
    fn drop(&mut self) {
        let _ = self.pause();
    }
}
