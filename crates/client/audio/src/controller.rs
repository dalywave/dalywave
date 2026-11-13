//! Поток контроллера: очередь, жизненный цикл дек, подготовка следующего трека, события,
//! повторное открытие вывода.
//!
//! Аудиоколбэк он не трогает: общается с микшером через кольца команд и событий.
//!
//! Буферизованный вывод (AirPlay 2) отдаёт звук наперёд, на секунды: что отдал микшер, слышно
//! позже. Поэтому контроллер раз в тик помечает, что звучит на каком кадре часов микшера
//! ([`HeardAt`]), и сообщает позицию по слышимому кадру вывода; события о том, что слышно (смена
//! трека сама собой, уровень, конец очереди), ждут своего кадра. Команды человека (перемотка,
//! другой трек) сбрасывают поставленное наперёд и относятся к слышимому треку.
//! Тяжёлое (ссылка у сервера, открытие источника, разбор контейнера, перемотка) идёт в отдельных
//! потоках подготовки, чтобы команды обрабатывались сразу.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crossbeam_channel::{Receiver, Sender};
use dalywave_transitions::{Analysis, Plan, PlanInput, TempoMap};

use crate::deck::{self, DeckCmd, DeckHandle, PreparedTrack};
use crate::decode::TrackDecoder;
use crate::devices::OutputKind;
use crate::eq::EqTarget;
use crate::mixer::{MAX_DECKS, Mixer, MixerCmd, MixerEvent, MixerPorts, SleepCmd, StartRule};
use crate::output::{self, LoadStats, Output, OutputIssue};
use crate::route;
use crate::source::SourceCache;
use crate::{
    AudioError, ChangeReason, Command, DjSolo, EngineConfig, EqSettings, Event, OutputConfig,
    PlaybackState, SleepStatus, SleepTimer, Snapshot, SourceSpec, TrackSpec, Transition,
};

const POSITION_EVERY: Duration = Duration::from_millis(250);
/// Окно измерителя — 50 мс, контроллер просыпается раз в 20 мс: так выходит ~20 событий в секунду
const LEVEL_EVERY: Duration = Duration::from_millis(40);
/// Пауза дольше этого — останавливаем поток устройства (на iOS система сможет усыпить приложение).
const STOP_OUTPUT_AFTER: Duration = Duration::from_secs(3);
/// Устройство не просит звук столько времени, хотя должно играть, — системный вывод не отвечает
/// (coreaudiod или медиасервер iOS завис: запуск вывода не возвращается, колбэки не приходят).
/// Иначе плеер молча висел бы в «загрузке».
const OUTPUT_STALL: Duration = Duration::from_secs(3);
/// Повторы, пока вывод не отвечает: через столько после предыдущей попытки (дальше — по последнему).
const RETRY_BACKOFF: [Duration; 5] = [
    Duration::from_secs(1),
    Duration::from_secs(2),
    Duration::from_secs(5),
    Duration::from_secs(10),
    Duration::from_secs(30),
];
/// Брошенных потоков устройства (застряли в аудиосервере) не больше этого: пока их столько,
/// новые попытки не открываются — застряли бы так же.
const MAX_ABANDONED: u32 = 3;
/// «Диджей»: переход не раньше, чем через столько от текущей позиции — входящему нужно открыться,
/// перемотаться к сильной доле и набрать буфер.
const DJ_LEAD_MS: f64 = 12_000.0;
/// «Диджей»: входящий готовится за столько до начала перехода.
const DJ_PREP_MS: f64 = 20_000.0;
/// Анализ не пришёл, а до конца трека меньше этого — переход без него («Умный»).
const DJ_WAIT_ANALYSIS_MS: u64 = 40_000;

/// Копия трека для повтора одного трека: id элемента очереди + этот разделитель + номер.
/// Наружу (события, снимок) уходит id без хвоста — для интерфейса это тот же элемент.
const REPEAT_SEP: char = '\u{1f}';

/// id элемента очереди без хвоста копии повтора.
fn item_id(id: &str) -> &str {
    id.split(REPEAT_SEP).next().unwrap_or(id)
}

fn same_item(a: &str, b: &str) -> bool {
    item_id(a) == item_id(b)
}

/// Что звучало на кадре часов микшера: по этим отметкам буферизованный вывод сообщает, что
/// слышно сейчас.
#[derive(Clone, Debug)]
struct HeardAt {
    clock: u64,
    index: usize,
    /// id элемента очереди как есть (с хвостом копии повтора)
    id: String,
    position_ms: u64,
    duration_ms: Option<u64>,
    /// Позиция шла: играло, не перемотка
    moving: bool,
}

/// Отметок не больше этого (тик — 20 мс: минуты отставания с запасом).
const HEARD_MAX: usize = 20_000;

/// Идущий таймер сна.
struct SleepRun {
    timer: SleepTimer,
    /// «Через N минут»: когда пауза
    deadline: Option<Instant>,
    /// «В конце трека»: какой трек доигрываем
    track_id: Option<String>,
    track_index: Option<usize>,
    /// Затухание отдано микшеру
    fade_sent: bool,
}

/// Системный вывод не отвечает: сколько попыток было и когда следующая.
struct Outage {
    attempt: u32,
    next_try: Instant,
}

/// Анализ трека: ещё идёт или готов (None — его нет).
enum AnalysisState {
    Pending,
    Done(Option<Arc<Analysis>>),
}

/// Выбранный переход «Диджея» к следующему треку.
struct DjPlan {
    index: usize,
    id: String,
    plan: Plan,
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Role {
    /// Подключена со стартом «сейчас», ещё не зазвучала.
    Pending,
    Main,
    /// Ждёт кроссфейда.
    Next,
    /// Затухает или отсоединена, ждём освобождения слота.
    Outgoing,
}

struct DeckCtl {
    handle: DeckHandle,
    role: Role,
    first_index: usize,
    /// Первый трек деки (с известной ссылкой) — для событий, даже если очередь успела поменяться.
    first: TrackSpec,
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Purpose {
    Main,
    Next,
}

struct Prep {
    id: u64,
    index: usize,
    offset_ms: u64,
    purpose: Purpose,
}

/// Откуда начинать подготовку трека.
#[derive(Clone, Debug, Default)]
struct StartAt {
    offset_ms: u64,
    /// «Диджей»: точное начало (мс, дробное) и карта темпа входящего.
    tempo: Option<TempoMap>,
}

struct PrepResult {
    id: u64,
    res: Result<PreparedTrack, AudioError>,
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum NextState {
    None,
    Preparing(usize),
    Chained(usize),
    Waiting { slot: usize, index: usize },
}

impl NextState {
    fn index(self) -> Option<usize> {
        match self {
            Self::None => None,
            Self::Preparing(i) | Self::Chained(i) | Self::Waiting { index: i, .. } => Some(i),
        }
    }

    fn with_index(self, i: usize) -> Self {
        match self {
            Self::None => Self::None,
            Self::Preparing(_) => Self::Preparing(i),
            Self::Chained(_) => Self::Chained(i),
            Self::Waiting { slot, .. } => Self::Waiting { slot, index: i },
        }
    }
}

/// Открытый вывод с микшером.
struct OutputParts {
    output: Output,
    rate: u32,
    channels: u16,
    ports: MixerPorts,
    load: Arc<LoadStats>,
    /// Выбранное устройство, если открыто оно; None — устройство по умолчанию или захват
    device: Option<String>,
}

/// Проблема вывода с номером потока: сообщения старого потока после открытия нового не нужны.
type Issue = (u32, OutputIssue);

struct Controller {
    cfg: EngineConfig,
    ev: Sender<Event>,
    snap: Arc<Mutex<Snapshot>>,
    capture: Arc<Mutex<Vec<f32>>>,
    issues_tx: Sender<Issue>,
    issues_rx: Receiver<Issue>,
    output: Output,
    output_gen: u32,
    output_lost: bool,
    output_reopens: u32,
    /// Куда выводить по выбору пользователя; None — устройство по умолчанию
    device_pref: Option<String>,
    /// Куда поток открыт на самом деле: выбранное могло пропасть
    output_device: Option<String>,
    /// Тестовый захват: частота, с которой его открыли заново.
    capture_rate: Option<u32>,
    device_rate: u32,
    device_channels: u16,
    load: Arc<LoadStats>,
    ports: MixerPorts,
    cache: SourceCache,
    queue: Vec<TrackSpec>,
    index: Option<usize>,
    state: PlaybackState,
    want_play: bool,
    volume: f32,
    transition: Transition,
    normalize: bool,
    levels: bool,
    level_seq: u64,
    decks: [Option<DeckCtl>; MAX_DECKS],
    preps: Vec<Prep>,
    prep_seq: u64,
    prep_tx: Sender<PrepResult>,
    prep_rx: Receiver<PrepResult>,
    next: NextState,
    failed: HashSet<usize>,
    t_cmd: Option<Instant>,
    last_pos_emit: Instant,
    last_level_emit: Instant,
    paused_since: Option<Instant>,
    /// Сторож вывода: последнее значение счётчика кадров и с какого момента оно не растёт.
    stall: Option<(u64, Instant)>,
    /// Системный вывод не отвечает, а звук нужен: ждём и повторяем
    outage: Option<Outage>,
    buffering: bool,
    seek_target_ms: Option<u64>,
    consecutive_errors: u32,
    shutdown: bool,
    analyses: HashMap<String, AnalysisState>,
    analysis_tx: Sender<(String, Option<Arc<Analysis>>)>,
    analysis_rx: Receiver<(String, Option<Arc<Analysis>>)>,
    dj_plan: Option<DjPlan>,
    eq: Option<EqSettings>,
    sleep: Option<SleepRun>,
    /// Микшер сообщил: затухание таймера сна дошло до тишины
    sleep_faded: bool,
    /// Повтор одного трека: следом за текущим стоит его копия
    repeat_one: bool,
    repeat_seq: u64,
    /// Открывать буферизованный вывод (AirPlay на маршруте)
    buffered_want: bool,
    /// Буферизованный вывод отказал: до ухода с AirPlay — обычный
    buffered_failed: bool,
    heard: VecDeque<HeardAt>,
    /// События о слышимом ждут своего кадра часов микшера
    deferred: RefCell<VecDeque<(u64, Event)>>,
}

pub(crate) fn run(
    cfg: EngineConfig,
    rx: Receiver<Command>,
    ev: Sender<Event>,
    snap: Arc<Mutex<Snapshot>>,
    capture: Arc<Mutex<Vec<f32>>>,
    ready: Sender<Result<(), AudioError>>,
) {
    let mut ctl = match Controller::new(cfg, ev, snap, capture) {
        Ok(c) => {
            c.update_snapshot();
            let _ = ready.send(Ok(()));
            c
        }
        Err(e) => {
            let _ = ready.send(Err(e));
            return;
        }
    };
    ctl.run(rx);
}

/// Маршрут звука сейчас: подставной (проверки) или системный; у захвата без подставного — пусто.
fn route_kinds(cfg: &EngineConfig, device: Option<&str>) -> Vec<OutputKind> {
    if let Some(p) = &cfg.route {
        return p.kinds();
    }
    if matches!(
        cfg.output,
        OutputConfig::Capture { .. } | OutputConfig::Fake { .. }
    ) {
        return Vec::new();
    }
    platform_route(device)
}

/// iOS: выходы маршрута аудиосессии (несколько HomePod — несколько выходов AirPlay).
#[cfg(target_os = "ios")]
fn platform_route(_device: Option<&str>) -> Vec<OutputKind> {
    crate::apple::current_route()
        .into_iter()
        .map(|p| p.kind)
        .collect()
}

/// macOS: тип выбранного устройства или устройства по умолчанию (колонка AirPlay — `AirPlay`).
#[cfg(target_os = "macos")]
fn platform_route(device: Option<&str>) -> Vec<OutputKind> {
    let Ok(list) = crate::devices::output_devices() else {
        return Vec::new();
    };
    list.iter()
        .find(|d| device.is_some_and(|id| d.id == id))
        .or_else(|| list.iter().find(|d| d.is_default))
        .map(|d| vec![d.kind])
        .unwrap_or_default()
}

#[cfg(not(any(target_os = "ios", target_os = "macos")))]
fn platform_route(_device: Option<&str>) -> Vec<OutputKind> {
    Vec::new()
}

/// Сообщения вывода живут в чужом потоке: помечаем их номером потока.
fn forward_issues(rx: Receiver<OutputIssue>, issues: &Sender<Issue>, generation: u32) {
    let issues = issues.clone();
    let _ = std::thread::Builder::new()
        .name("dw-audio-issues".into())
        .spawn(move || {
            for issue in rx.iter() {
                if issues.send((generation, issue)).is_err() {
                    return;
                }
            }
        });
}

/// Вывода нет (не открылся при запуске): микшер ни к чему не подключён, частота — обычная.
/// Настоящий откроется по команде play.
fn detached_parts(cfg: &EngineConfig) -> OutputParts {
    let rate = match cfg.output {
        OutputConfig::Fake { sample_rate, .. } | OutputConfig::Capture { sample_rate } => {
            sample_rate
        }
        OutputConfig::Device { .. } => 48_000,
    };
    let (_mixer, ports) = Mixer::new(rate, cfg.start_threshold_ms, cfg.volume);
    OutputParts {
        output: Output::None,
        rate,
        channels: 2,
        ports,
        load: Arc::new(LoadStats::default()),
        device: None,
    }
}

#[allow(clippy::too_many_arguments)]
fn open_output(
    cfg: &EngineConfig,
    device: Option<&str>,
    capture_rate: Option<u32>,
    volume: f32,
    capture: &Arc<Mutex<Vec<f32>>>,
    issues: &Sender<Issue>,
    generation: u32,
    buffered: bool,
) -> Result<OutputParts, AudioError> {
    let load = Arc::new(LoadStats::default());
    match cfg.output {
        OutputConfig::Device { buffer_frames, .. } => {
            #[cfg(target_vendor = "apple")]
            if buffered {
                // Громкость — у рендерера: в микшере её было бы слышно через секунды очереди
                let rate = crate::renderer::RATE;
                let (mixer, ports) = Mixer::new(rate, cfg.start_threshold_ms, 1.0);
                let (tx, rx) = crossbeam_channel::unbounded::<OutputIssue>();
                let uid = device
                    .and_then(|id| id.strip_prefix("coreaudio:"))
                    .map(str::to_string);
                let output = Output::open_buffered(mixer, rate, volume, uid, load.clone(), tx)?;
                forward_issues(rx, issues, generation);
                return Ok(OutputParts {
                    output,
                    rate,
                    channels: 2,
                    ports,
                    load,
                    device: device.map(str::to_string),
                });
            }
            #[cfg(not(target_vendor = "apple"))]
            let _ = buffered;
            let (tx, rx) = crossbeam_channel::unbounded::<OutputIssue>();
            let (output, opened) = Output::open_device(
                device.map(str::to_string),
                buffer_frames,
                cfg.start_threshold_ms,
                volume,
                load.clone(),
                tx,
            )?;
            forward_issues(rx, issues, generation);
            Ok(OutputParts {
                output,
                rate: opened.info.sample_rate,
                channels: opened.info.channels,
                ports: opened.ports,
                load,
                device: opened.device,
            })
        }
        OutputConfig::Fake {
            sample_rate,
            ref fake,
        } => {
            let (tx, rx) = crossbeam_channel::unbounded::<OutputIssue>();
            let (output, opened) = Output::open_fake(
                fake.clone(),
                sample_rate,
                cfg.start_threshold_ms,
                volume,
                load.clone(),
                tx,
            )?;
            forward_issues(rx, issues, generation);
            Ok(OutputParts {
                output,
                rate: opened.info.sample_rate,
                channels: opened.info.channels,
                ports: opened.ports,
                load,
                device: None,
            })
        }
        OutputConfig::Capture { sample_rate } => {
            let rate = capture_rate.unwrap_or(sample_rate);
            let (mixer, ports) = Mixer::new(rate, cfg.start_threshold_ms, volume);
            let latency = if buffered {
                u64::from(cfg.capture_latency_ms) * u64::from(rate) / 1000
            } else {
                0
            };
            Ok(OutputParts {
                output: Output::open_capture(
                    mixer,
                    capture.clone(),
                    latency,
                    rate,
                    cfg.capture_speed,
                ),
                rate,
                channels: 2,
                ports,
                load,
                device: None,
            })
        }
    }
}

impl Controller {
    fn new(
        cfg: EngineConfig,
        ev: Sender<Event>,
        snap: Arc<Mutex<Snapshot>>,
        capture: Arc<Mutex<Vec<f32>>>,
    ) -> Result<Self, AudioError> {
        let (issues_tx, issues_rx) = crossbeam_channel::unbounded();
        let device_pref = match &cfg.output {
            OutputConfig::Device { device, .. } => device.clone(),
            OutputConfig::Capture { .. } | OutputConfig::Fake { .. } => None,
        };
        let kinds = route_kinds(&cfg, device_pref.as_deref());
        let buffered_want = route::wants_buffered(cfg.buffered, &kinds).unwrap_or(false);
        let opened = match open_output(
            &cfg,
            device_pref.as_deref(),
            None,
            cfg.volume,
            &capture,
            &issues_tx,
            0,
            buffered_want,
        ) {
            // Буферизованный не открылся — обычный, звук важнее
            Err(e) if buffered_want => {
                log::warn!("buffered output failed to open: {e}; using realtime");
                open_output(
                    &cfg,
                    device_pref.as_deref(),
                    None,
                    cfg.volume,
                    &capture,
                    &issues_tx,
                    0,
                    false,
                )
            }
            r => r,
        };
        let parts = match opened {
            Ok(p) => p,
            // Аудиосервер не отвечает или устройства нет: движок всё равно нужен (очередь,
            // «Сейчас играет»), вывод откроется по команде play
            Err(e) if cfg.output.is_device() => {
                log::warn!("output did not open: {e}; will retry on play");
                detached_parts(&cfg)
            }
            Err(e) => return Err(e),
        };
        let output_lost = matches!(parts.output, Output::None);
        log::info!(
            "output: {} Hz, {} ch, {}, {} (route {})",
            parts.rate,
            parts.channels,
            parts.device.as_deref().unwrap_or("system default"),
            if parts.output.is_buffered() {
                "buffered"
            } else {
                "realtime"
            },
            route::describe(&kinds)
        );
        let (prep_tx, prep_rx) = crossbeam_channel::unbounded();
        let (analysis_tx, analysis_rx) = crossbeam_channel::unbounded();
        let mut ports = parts.ports;
        if matches!(cfg.transition, Transition::Dj { .. }) {
            let _ = ports.cmd_tx.push(MixerCmd::Dj(true));
        }
        if cfg.dj_solo != DjSolo::Both {
            let _ = ports.cmd_tx.push(MixerCmd::Solo(cfg.dj_solo));
        }
        if cfg.eq.is_some() {
            let _ = ports
                .cmd_tx
                .push(MixerCmd::Eq(EqTarget::new(cfg.eq.as_ref(), parts.rate)));
        }
        Ok(Self {
            eq: cfg.eq,
            sleep: None,
            sleep_faded: false,
            cache: SourceCache::with_resolver(cfg.user_agent.clone(), cfg.resolver.clone()),
            volume: cfg.volume,
            transition: cfg.transition,
            normalize: cfg.normalize,
            cfg,
            ev,
            snap,
            capture,
            issues_tx,
            issues_rx,
            output: parts.output,
            output_gen: 0,
            output_lost,
            output_reopens: 0,
            device_pref,
            output_device: parts.device,
            capture_rate: None,
            device_rate: parts.rate,
            device_channels: parts.channels,
            load: parts.load,
            ports,
            queue: Vec::new(),
            index: None,
            state: PlaybackState::Idle,
            want_play: false,
            levels: false,
            level_seq: 0,
            decks: Default::default(),
            preps: Vec::new(),
            prep_seq: 0,
            prep_tx,
            prep_rx,
            next: NextState::None,
            failed: HashSet::new(),
            t_cmd: None,
            last_pos_emit: Instant::now(),
            last_level_emit: Instant::now(),
            paused_since: None,
            stall: None,
            outage: None,
            buffering: false,
            seek_target_ms: None,
            consecutive_errors: 0,
            shutdown: false,
            analyses: HashMap::new(),
            analysis_tx,
            analysis_rx,
            dj_plan: None,
            repeat_one: false,
            repeat_seq: 0,
            buffered_want,
            buffered_failed: false,
            heard: VecDeque::new(),
            deferred: RefCell::new(VecDeque::new()),
        })
    }

    fn emit(&self, mut e: Event) {
        // Копия повтора снаружи — тот же элемент очереди
        match &mut e {
            Event::TrackChanged { track, .. } if track.id.contains(REPEAT_SEP) => {
                track.id = item_id(&track.id).to_string();
            }
            Event::Position { id, .. } | Event::Error { id: Some(id), .. }
                if id.contains(REPEAT_SEP) =>
            {
                *id = item_id(id).to_string();
            }
            _ => {}
        }
        if self.output.is_buffered() && Self::heard_later(&e) {
            let clock = self.clock();
            let pending = !self.deferred.borrow().is_empty();
            if pending || self.output.audible(clock) < clock {
                self.deferred.borrow_mut().push_back((clock, e));
                return;
            }
        }
        let _ = self.ev.send(e);
    }

    /// Событие о том, что слышно: у буферизованного вывода ждёт, пока это зазвучит.
    fn heard_later(e: &Event) -> bool {
        matches!(
            e,
            Event::TrackChanged {
                reason: ChangeReason::Auto,
                ..
            } | Event::TrackInfo { .. }
                | Event::Level(_)
                | Event::QueueEnded
                | Event::State(PlaybackState::Ended)
        )
    }

    fn clock(&self) -> u64 {
        self.ports.shared.clock.load(Ordering::Relaxed)
    }

    /// Отложенные события, кадр которых уже слышен.
    fn release_heard(&self) {
        if self.deferred.borrow().is_empty() {
            return;
        }
        let ready: Vec<Event> = {
            let mut d = self.deferred.borrow_mut();
            let n = if self.output.is_buffered() {
                let audible = self.output.audible(self.clock());
                d.iter().take_while(|(c, _)| *c <= audible).count()
            } else {
                d.len()
            };
            d.drain(..n).map(|(_, e)| e).collect()
        };
        for e in ready {
            let _ = self.ev.send(e);
        }
    }

    /// Отметка «что звучит на этом кадре часов микшера» (только буферизованный вывод).
    fn record_heard(&mut self) {
        if !self.output.is_buffered() {
            self.heard.clear();
            return;
        }
        let clock = self.clock();
        let (Some(index), Some(id)) = (self.index, self.index.and_then(|i| self.id_at(i))) else {
            return;
        };
        let at = HeardAt {
            clock,
            index,
            id,
            position_ms: self.position_ms(),
            duration_ms: self.duration_ms(),
            moving: self.state == PlaybackState::Playing && self.seek_target_ms.is_none(),
        };
        match self.heard.back_mut() {
            Some(last) if last.clock == clock => *last = at,
            _ => self.heard.push_back(at),
        }
        let audible = self.output.audible(clock);
        while self.heard.len() > 1 && self.heard[1].clock <= audible {
            self.heard.pop_front();
        }
        while self.heard.len() > HEARD_MAX {
            self.heard.pop_front();
        }
    }

    /// Что слышно сейчас, если вывод отстаёт от микшера; None — слышно то же, что играет микшер.
    fn heard_now(&self) -> Option<HeardAt> {
        if self.seek_target_ms.is_some() || self.heard.is_empty() || !self.output.is_buffered() {
            return None;
        }
        let clock = self.clock();
        let audible = self.output.audible(clock);
        if audible >= clock {
            return None;
        }
        let at = self
            .heard
            .iter()
            .rev()
            .find(|h| h.clock <= audible)
            .or_else(|| self.heard.front())?;
        let mut h = at.clone();
        if h.moving && audible > h.clock {
            h.position_ms += (audible - h.clock) * 1000 / u64::from(self.device_rate.max(1));
            if let Some(d) = h.duration_ms {
                h.position_ms = h.position_ms.min(d);
            }
        }
        Some(h)
    }

    /// Номер, id, позиция и длина того, что слышно (для событий и снимка).
    fn reported(&self) -> (Option<usize>, Option<String>, u64, Option<u64>) {
        match self.heard_now() {
            Some(h) => (Some(h.index), Some(h.id), h.position_ms, h.duration_ms),
            None => (
                self.index,
                self.index.and_then(|i| self.id_at(i)),
                self.position_ms(),
                self.duration_ms(),
            ),
        }
    }

    /// Команда человека относится к тому, что он слышит: буферизованный вывод мог уже перейти к
    /// следующему треку, который ещё не звучит (о нём и не сообщали) — такой переход отменяется.
    fn sync_heard(&mut self) {
        if let Some(h) = self.heard_now()
            && self.index != Some(h.index)
            && self.queue.get(h.index).is_some_and(|t| t.id == h.id)
        {
            log::info!(
                "command for the heard track #{} (engine is already at #{:?})",
                h.index,
                self.index
            );
            self.index = Some(h.index);
        }
    }

    /// Сбросить поставленное наперёд: следующее, что прозвучит, — новое.
    fn flush_output(&mut self) {
        self.output.flush();
        self.heard.clear();
        self.deferred.borrow_mut().clear();
    }

    /// Маршрут звука сменился: нужен ли теперь буферизованный вывод (AirPlay) или обычный.
    fn route_changed(&mut self) {
        let kinds = route_kinds(&self.cfg, self.device_pref.as_deref());
        let wants = route::wants_buffered(self.cfg.buffered, &kinds);
        if wants == Some(false) {
            self.buffered_failed = false;
        }
        let Some(want) = route::switch_to(self.cfg.buffered, self.output.is_buffered(), &kinds)
        else {
            log::debug!("route {}: output stays", route::describe(&kinds));
            return;
        };
        if want && self.buffered_failed {
            return;
        }
        self.buffered_want = want;
        let reason = format!(
            "route {}: {} output",
            route::describe(&kinds),
            if want { "buffered" } else { "realtime" }
        );
        self.reopen_output(None, &reason);
    }

    fn set_state(&mut self, s: PlaybackState) {
        if self.state != s {
            self.state = s;
            self.emit(Event::State(s));
        }
    }

    fn set_buffering(&mut self, on: bool) {
        if self.buffering != on {
            self.buffering = on;
            self.emit(Event::Buffering(on));
        }
    }

    fn id_at(&self, index: usize) -> Option<String> {
        self.queue.get(index).map(|t| t.id.clone())
    }

    fn ms_to_frames(&self, ms: u64) -> u64 {
        ms * self.device_rate as u64 / 1000
    }

    fn mixer(&mut self, cmd: MixerCmd) {
        if self.ports.cmd_tx.push(cmd).is_err() {
            log::error!("mixer command queue full");
        }
    }

    fn run(&mut self, rx: Receiver<Command>) {
        while !self.shutdown {
            // Просыпаемся по команде, по готовности подготовленного трека или по таймеру.
            let prep_rx = self.prep_rx.clone();
            crossbeam_channel::select! {
                recv(rx) -> cmd => match cmd {
                    Ok(cmd) => {
                        self.handle(cmd);
                        while let Ok(cmd) = rx.try_recv() {
                            self.handle(cmd);
                        }
                    }
                    Err(_) => break,
                },
                recv(prep_rx) -> r => {
                    if let Ok(r) = r {
                        self.on_prep(r);
                    }
                },
                default(Duration::from_millis(20)) => {},
            }
            self.tick();
        }
        self.reset_all();
        self.output.halt();
    }

    fn tick(&mut self) {
        while self.ports.garbage_rx.pop().is_ok() {}
        self.poll_mixer_events();
        while let Ok(r) = self.prep_rx.try_recv() {
            self.on_prep(r);
        }
        self.poll_output_issues();
        while let Ok((key, a)) = self.analysis_rx.try_recv() {
            self.analyses.insert(key, AnalysisState::Done(a));
        }
        self.track_segments();
        self.ensure_repeat();
        self.maybe_plan();
        self.maybe_preload();
        self.sleep_tick();
        self.record_heard();
        self.release_heard();
        self.emit_position(false);
        self.emit_level();
        let has_next = self.index.and_then(|i| self.next_index(i)).is_some();
        let undecided = if matches!(self.transition, Transition::Dj { .. }) {
            // «Диджей» решает, когда главная дека зазвучала и пришёл анализ; с выбранным
            // переходом следующий готовится ближе к нему
            has_next
                && self.next == NextState::None
                && self.dj_plan.is_none()
                && (self.main_slot().is_some() || self.pending_slot().is_some())
        } else {
            has_next
                && self.next == NextState::None
                && (self.main_slot().is_some() || self.pending_slot().is_some())
        };
        let hold =
            !self.preps.is_empty() || undecided || matches!(self.next, NextState::Preparing(_));
        self.ports.shared.hold.store(hold, Ordering::Release);
        if let Some(t) = self.paused_since
            && t.elapsed() > STOP_OUTPUT_AFTER
            && self.output.is_running()
        {
            self.output.stop();
        }
        self.watch_output();
        self.update_snapshot();
    }

    /// Сторож вывода: должно звучать, а устройство не берёт ни кадра — системный вывод не отвечает.
    /// Тогда [`Event::OutputStalled`] и повторы с отступом ([`RETRY_BACKOFF`]); устройство снова
    /// берёт звук — [`Event::OutputRecovered`], играет дальше с той же позиции.
    fn watch_output(&mut self) {
        let clock = self.clock();
        if !self.cfg.output.is_device() || !self.want_play {
            // Пауза: ждать вывод незачем, откроем по команде play
            self.stall = None;
            self.end_outage();
            return;
        }
        // Кадры идут — вывод жив
        if let Some((c, _)) = self.stall
            && c != clock
        {
            self.stall = Some((clock, Instant::now()));
            if self.output.is_running() {
                self.end_outage();
            }
            return;
        }
        if let Some(o) = &self.outage {
            if Instant::now() >= o.next_try {
                self.retry_output();
            }
            return;
        }
        // Буферизованный вывод берёт звук рывками и ждёт данных сам — сторож только у устройства
        let watching = !self.output.is_buffered()
            && self.output.is_running()
            && matches!(self.state, PlaybackState::Loading | PlaybackState::Playing);
        if !watching {
            self.stall = None;
            return;
        }
        match self.stall {
            Some((_, since)) if since.elapsed() >= OUTPUT_STALL => {
                log::warn!(
                    "output: the device has not asked for audio for {} s{}",
                    OUTPUT_STALL.as_secs(),
                    match self.output.busy_for() {
                        Some(d) => format!(", its start hangs for {:.1} s", d.as_secs_f64()),
                        None => String::new(),
                    }
                );
                self.begin_outage();
            }
            Some(_) => {}
            None => self.stall = Some((clock, Instant::now())),
        }
    }

    /// Вывод не отвечает: сообщить и ждать первой попытки.
    fn begin_outage(&mut self) {
        if self.outage.is_some() {
            return;
        }
        let wait = RETRY_BACKOFF[0];
        self.outage = Some(Outage {
            attempt: 0,
            next_try: Instant::now() + wait,
        });
        self.set_buffering(false);
        self.emit(Event::OutputStalled {
            attempt: 0,
            retry_in_ms: wait.as_millis() as u64,
        });
    }

    fn end_outage(&mut self) {
        if self.outage.take().is_some() {
            log::info!("output: sound runs again");
            self.emit(Event::OutputRecovered);
        }
    }

    /// Очередная попытка: открыть вывод заново с той же позиции. Если поток устройства ещё ждёт
    /// аудиосервер (запуск не вернулся), новый не открываем — тот заиграет сам, когда сервер
    /// ответит; новый застрял бы так же.
    fn retry_output(&mut self) {
        let Some(o) = &mut self.outage else { return };
        o.attempt += 1;
        let attempt = o.attempt;
        let wait = RETRY_BACKOFF[(attempt as usize).min(RETRY_BACKOFF.len() - 1)];
        o.next_try = Instant::now() + wait;
        let waiting = self.output.busy_for();
        let abandoned = output::abandoned_threads();
        if let Some(d) = waiting {
            log::warn!(
                "output: attempt {attempt}: the start still waits for the system audio ({:.1} s)",
                d.as_secs_f64()
            );
        } else if abandoned >= MAX_ABANDONED {
            log::warn!("output: attempt {attempt}: {abandoned} device threads are stuck, waiting");
        } else {
            log::warn!("output: attempt {attempt}: reopening the output");
            self.reopen_output(None, "the output did not respond, retrying");
        }
        self.stall = Some((self.clock(), Instant::now()));
        // Открытие могло кончить ожидание (пауза) — тогда сообщать не о чем
        if self.outage.is_some() {
            self.emit(Event::OutputStalled {
                attempt,
                retry_in_ms: wait.as_millis() as u64,
            });
        }
    }

    // ---- команды ----

    fn handle(&mut self, cmd: Command) {
        match cmd {
            Command::LoadQueue {
                tracks,
                start,
                start_ms,
                autoplay,
            } => {
                self.sleep_rearm();
                self.load(tracks, start, start_ms, autoplay)
            }
            Command::Append(tracks) => {
                self.queue.extend(tracks);
                self.update_expect_chain();
            }
            Command::SetUpcoming(tracks) => self.set_upcoming(tracks),
            Command::Play => self.play(),
            Command::Pause => self.pause(),
            Command::Toggle => {
                if self.want_play {
                    self.pause()
                } else {
                    self.play()
                }
            }
            Command::Seek(ms) => {
                self.sleep_rearm();
                self.sync_heard();
                self.seek(ms)
            }
            Command::Next => {
                self.sleep_rearm();
                self.sync_heard();
                if let Some(i) = self.index
                    && let Some(j) = self.user_next_index(i)
                {
                    self.go_to(j, ChangeReason::User);
                }
            }
            Command::Previous => {
                self.sleep_rearm();
                self.sync_heard();
                if let Some(i) = self.index {
                    let heard_ms = self.reported().2;
                    match self.user_previous_index(i) {
                        Some(j) if heard_ms <= 3000 => self.go_to(j, ChangeReason::User),
                        _ => self.seek(0),
                    }
                }
            }
            Command::SetVolume(v) => {
                self.volume = v;
                if !self.output.set_volume(v) {
                    let ramp = (self.device_rate / 50).max(1);
                    self.mixer(MixerCmd::Volume { gain: v, ramp });
                }
            }
            Command::SetTransition(t) => {
                if t != self.transition {
                    let dj = matches!(t, Transition::Dj { .. });
                    if dj != matches!(self.transition, Transition::Dj { .. }) {
                        self.mixer(MixerCmd::Dj(dj));
                    }
                    self.transition = t;
                    self.reset_next();
                }
            }
            Command::SetNormalize(on) => self.normalize = on,
            Command::SetLevels(on) => self.levels = on,
            Command::ReopenOutput {
                sample_rate,
                reason,
            } => self.reopen_output(sample_rate, &reason),
            Command::SetOutputDevice(id) => self.set_output_device(id),
            Command::SetEq(eq) => {
                self.eq = eq;
                let t = EqTarget::new(eq.as_ref(), self.device_rate);
                self.mixer(MixerCmd::Eq(t));
            }
            Command::SetSleep(timer) => self.set_sleep(timer),
            Command::SetRepeatOne(on) => {
                self.repeat_one = on;
                self.ensure_repeat();
            }
            Command::RouteChanged => self.route_changed(),
            Command::Stop => {
                self.reset_all();
                self.want_play = false;
                self.pause_mixer();
                self.flush_output();
                self.set_state(PlaybackState::Idle);
            }
            Command::Shutdown => self.shutdown = true,
        }
    }

    fn load(&mut self, tracks: Vec<TrackSpec>, start: usize, start_ms: u64, autoplay: bool) {
        self.reset_all();
        self.queue = tracks;
        self.failed.clear();
        self.consecutive_errors = 0;
        if self.queue.is_empty() {
            self.index = None;
            self.set_state(PlaybackState::Idle);
            return;
        }
        let i = start.min(self.queue.len() - 1);
        self.index = Some(i);
        self.want_play = autoplay;
        self.t_cmd = Some(Instant::now());
        self.set_state(if autoplay {
            PlaybackState::Loading
        } else {
            PlaybackState::Paused
        });
        self.emit(Event::TrackChanged {
            index: i,
            track: self.queue[i].clone(),
            reason: ChangeReason::User,
        });
        self.seek_target_ms = (start_ms > 0).then_some(start_ms);
        self.start_prep(i, start_ms, Purpose::Main);
        if autoplay {
            self.resume_output();
        } else {
            // Без автозапуска новая дека не должна зазвучать сама
            self.pause_mixer();
        }
        self.flush_output();
        self.emit_position(true);
    }

    /// Всё после текущего трека — новое. Подготовленный следующий остаётся, если он тот же.
    fn set_upcoming(&mut self, mut tracks: Vec<TrackSpec>) {
        // Буферизованный вывод уже начал следующие треки, а слышно ещё прежний: интерфейс шлёт «что
        // после слышимого» — уже начатое движком из начала списка не повторяем
        if let (Some(i), Some(h)) = (self.index, self.heard_now())
            && h.index < i
        {
            let started: Vec<&str> = ((h.index + 1)..=i)
                .filter_map(|j| self.queue.get(j))
                .map(|t| item_id(&t.id))
                .collect();
            let skip = tracks
                .iter()
                .zip(&started)
                .take_while(|(t, s)| item_id(&t.id) == **s)
                .count();
            tracks.drain(..skip);
        }
        let Some(i) = self.index else {
            self.queue = tracks;
            self.failed.clear();
            return;
        };
        let prepared = self
            .next
            .index()
            .and_then(|j| self.queue.get(j))
            .map(|t| t.id.clone());
        // Копия повтора остаётся следом за текущим: подготовленный повтор не сбрасывается
        let keep = if self.repeat_one
            && self
                .queue
                .get(i + 1)
                .is_some_and(|t| same_item(&t.id, &self.queue[i].id))
        {
            i + 2
        } else {
            i + 1
        };
        self.queue.truncate(keep);
        self.queue.extend(tracks);
        self.failed.retain(|&j| j < keep);
        let next = self.next_index(i);
        let next_id = next.map(|j| self.queue[j].id.clone());
        match (prepared, next) {
            (Some(p), Some(j)) if Some(&p) == next_id.as_ref() => {
                self.next = self.next.with_index(j);
            }
            (Some(_), _) => self.reset_next(),
            (None, _) => {}
        }
        self.update_expect_chain();
    }

    fn play(&mut self) {
        if self.queue.is_empty() {
            return;
        }
        if self.want_play && self.state == PlaybackState::Playing {
            return;
        }
        self.want_play = true;
        self.t_cmd = Some(Instant::now());
        if self.output_lost {
            // Устройство пропадало: открываем вывод заново, текущий трек продолжится с места
            self.reopen_output(None, "play after the output device was lost");
            // Не открылся: ждём системный вывод (повторы) или, если ждать нечего, — пауза
            if self.output_lost && self.outage.is_none() {
                self.want_play = false;
            }
            return;
        }
        if self.state == PlaybackState::Ended {
            // Хвост прошлой очереди (и отложенный конец) больше не нужен
            self.flush_output();
        }
        if self.state == PlaybackState::Ended
            || self.main_slot().is_none() && self.pending_slot().is_none() && !self.has_main_prep()
        {
            let i = if self.state == PlaybackState::Ended {
                0
            } else {
                self.index.unwrap_or(0)
            };
            let offset = if self.state == PlaybackState::Ended {
                0
            } else {
                self.seek_target_ms.unwrap_or(0)
            };
            self.index = Some(i);
            self.emit(Event::TrackChanged {
                index: i,
                track: self.queue[i].clone(),
                reason: ChangeReason::User,
            });
            self.set_state(PlaybackState::Loading);
            self.start_prep(i, offset, Purpose::Main);
        } else if self.main_slot().is_some() {
            self.set_state(PlaybackState::Playing);
        }
        self.resume_output();
    }

    fn resume_output(&mut self) {
        self.paused_since = None;
        if self.output_lost && self.cfg.output.is_device() && self.outage.is_none() {
            // Вывода нет (не открылся при запуске, пропал): открыть сейчас — открытие продолжит
            // само, а не откроется — ждём системный вывод и повторяем
            self.reopen_output(None, "play without an output");
            return;
        }
        self.output.start();
        let fade = self.declick();
        self.mixer(MixerCmd::Resume { fade });
    }

    fn pause_mixer(&mut self) {
        let fade = self.declick() * 2;
        self.mixer(MixerCmd::Pause { fade });
        self.paused_since = Some(Instant::now());
        // Буферизованный: время синхронизатора стоит сразу, поставленное доиграет после паузы
        if self.output.is_buffered() {
            self.output.stop();
        }
    }

    fn pause(&mut self) {
        self.want_play = false;
        self.pause_mixer();
        if self.state != PlaybackState::Idle && self.state != PlaybackState::Ended {
            self.set_state(PlaybackState::Paused);
        }
        self.set_buffering(false);
        self.emit_position(true);
    }

    fn declick(&self) -> u32 {
        (self.device_rate / 200).max(1)
    }

    fn seek(&mut self, ms: u64) {
        let Some(i) = self.index else { return };
        let ms = match self.duration_ms() {
            Some(d) => ms.min(d.saturating_sub(100)),
            None => ms,
        };
        self.t_cmd = Some(Instant::now());
        self.detach_role(Role::Main);
        self.detach_role(Role::Pending);
        self.cancel_preps(Purpose::Main);
        // Пристёгнутый следующий трек жил в старой деке — подготовим его заново.
        if matches!(self.next, NextState::Chained(_)) {
            self.next = NextState::None;
        }
        // «Диджей» привязан к кадрам старой деки: переход выбирается заново от новой позиции
        if matches!(self.transition, Transition::Dj { .. }) {
            self.reset_next();
        }
        if self.state == PlaybackState::Ended {
            self.set_state(PlaybackState::Paused);
        }
        self.seek_target_ms = Some(ms);
        self.start_prep(i, ms, Purpose::Main);
        self.flush_output();
        self.emit_position(true);
    }

    fn go_to(&mut self, i: usize, reason: ChangeReason) {
        self.t_cmd = Some(Instant::now());
        if let NextState::Waiting { slot, index } = self.next
            && index == i
        {
            // Следующий уже в буфере (ждал кроссфейда) — стартуем сразу.
            self.mixer(MixerCmd::StartNow { slot, fade_in: 0 });
            self.index = Some(i);
            if reason == ChangeReason::User {
                self.flush_output();
            }
            self.emit(Event::TrackChanged {
                index: i,
                track: self.queue[i].clone(),
                reason,
            });
            return;
        }
        self.detach_role(Role::Main);
        self.detach_role(Role::Pending);
        self.cancel_preps(Purpose::Main);
        self.reset_next();
        self.index = Some(i);
        self.seek_target_ms = None;
        self.emit(Event::TrackChanged {
            index: i,
            track: self.queue[i].clone(),
            reason,
        });
        if self.state != PlaybackState::Paused {
            self.set_state(PlaybackState::Loading);
        }
        self.start_prep(i, 0, Purpose::Main);
        // По команде — сразу новое; сам (прошлый трек кончился) — после того, что ещё звучит
        if reason == ChangeReason::User {
            self.flush_output();
        }
    }

    fn next_index(&self, i: usize) -> Option<usize> {
        ((i + 1)..self.queue.len()).find(|j| !self.failed.contains(j))
    }

    /// «Следующий» с пульта: копия повтора пропускается — это тот же трек.
    fn user_next_index(&self, i: usize) -> Option<usize> {
        let id = &self.queue.get(i)?.id;
        ((i + 1)..self.queue.len())
            .find(|j| !self.failed.contains(j) && !same_item(&self.queue[*j].id, id))
    }

    /// «Предыдущий» с пульта: сыгранные повторы того же трека пропускаются.
    fn user_previous_index(&self, i: usize) -> Option<usize> {
        let id = &self.queue.get(i)?.id;
        (0..i).rev().find(|&j| !same_item(&self.queue[j].id, id))
    }

    /// Повтор одного трека: следом за текущим всегда стоит его копия — движок готовит и
    /// склеивает её как обычный следующий трек (тот же файл, из кэша). Повтор сняли — копия уходит.
    fn ensure_repeat(&mut self) {
        let Some(i) = self.index else { return };
        let Some(cur) = self.queue.get(i) else { return };
        let next_is_copy = self
            .queue
            .get(i + 1)
            .is_some_and(|t| same_item(&t.id, &cur.id));
        if self.repeat_one == next_is_copy {
            return;
        }
        if self.repeat_one {
            let mut copy = cur.clone();
            self.repeat_seq += 1;
            copy.id = format!("{}{REPEAT_SEP}{}", item_id(&cur.id), self.repeat_seq);
            self.queue.insert(i + 1, copy);
            self.failed = self
                .failed
                .iter()
                .map(|&j| if j > i { j + 1 } else { j })
                .collect();
        } else {
            self.queue.remove(i + 1);
            self.failed = self
                .failed
                .iter()
                .filter(|&&j| j != i + 1)
                .map(|&j| if j > i + 1 { j - 1 } else { j })
                .collect();
        }
        // Подготовленный следующий был другим треком — готовим заново
        if self.next != NextState::None {
            self.reset_next();
        } else {
            self.dj_plan = None;
            self.update_expect_chain();
        }
    }

    /// Где в очереди трек `spec`, подготовленный под номером `index`. Очередь могли поменять,
    /// пока он готовился: тогда ищем его дальше текущего, а если его там нет — ставим следом.
    fn adopt(&mut self, index: usize, spec: &TrackSpec) -> usize {
        if self.queue.get(index).is_some_and(|t| t.id == spec.id) {
            return index;
        }
        let from = self.index.map_or(0, |i| i + 1).min(self.queue.len());
        if let Some(j) = (from..self.queue.len()).find(|&j| self.queue[j].id == spec.id) {
            return j;
        }
        self.queue.insert(from, spec.clone());
        self.failed = self
            .failed
            .iter()
            .map(|&j| if j >= from { j + 1 } else { j })
            .collect();
        from
    }

    // ---- вывод ----

    fn poll_output_issues(&mut self) {
        let mut reopen: Option<String> = None;
        let mut rerouted = false;
        while let Ok((generation, issue)) = self.issues_rx.try_recv() {
            if generation != self.output_gen {
                continue;
            }
            match issue {
                OutputIssue::Rerouted(msg) => {
                    // Поток сам переехал на новое устройство (macOS — устройство по умолчанию)
                    log::info!("output rerouted: {msg}");
                    self.emit(Event::Output {
                        sample_rate: self.device_rate,
                        channels: self.device_channels,
                        reason: format!("rerouted: {msg}"),
                    });
                    rerouted = true;
                }
                OutputIssue::BufferedFailed(msg) => {
                    // AirPlay через рендерер не идёт — обычный вывод, пока не уйдём с AirPlay
                    log::warn!("buffered output failed: {msg}; switching to realtime");
                    self.buffered_failed = true;
                    self.buffered_want = false;
                    reopen = Some(format!("buffered output failed: {msg}"));
                }
                OutputIssue::Lost(msg) => {
                    // Звук не должен внезапно пойти из другого устройства: пауза, как при
                    // отключении наушников. Вывод откроем заново по команде play.
                    log::warn!("output device lost: {msg}");
                    if self.want_play {
                        self.pause();
                    }
                    self.output_lost = true;
                    if self.output_device.is_some() {
                        // Пропало выбранное устройство (колонка AirPlay, ЦАП): дальше — системное.
                        // Открываем сразу, на паузе, чтобы список устройств показал, где звук.
                        self.device_pref = None;
                        reopen = Some(format!("selected output device lost: {msg}"));
                    }
                }
                OutputIssue::Invalidated(msg) => {
                    log::warn!("output stream invalidated: {msg}");
                    reopen = Some(msg);
                }
                OutputIssue::StartFailed(msg) => {
                    // Система не дала запустить вывод: звук нужен — ждём и пробуем снова
                    log::warn!("output did not start: {msg}");
                    if self.want_play && self.output.is_running() {
                        self.begin_outage();
                    }
                }
                OutputIssue::Other(msg) => log::debug!("output: {msg}"),
            }
        }
        if let Some(reason) = reopen {
            self.reopen_output(None, &reason);
        } else if rerouted {
            // Устройство по умолчанию сменилось само (macOS: выбрали колонку AirPlay в строке меню)
            self.route_changed();
        }
    }

    /// Выбор устройства вывода: открыть вывод заново, если звук идёт не туда.
    fn set_output_device(&mut self, id: Option<String>) {
        self.device_pref = id;
        if !matches!(self.cfg.output, OutputConfig::Device { .. }) {
            return;
        }
        if self.device_pref == self.output_device && !self.output_lost {
            return;
        }
        let reason = match &self.device_pref {
            Some(id) => format!("output device selected: {id}"),
            None => "output follows the system default".to_string(),
        };
        let kinds = route_kinds(&self.cfg, self.device_pref.as_deref());
        self.buffered_want = !self.buffered_failed
            && route::wants_buffered(self.cfg.buffered, &kinds).unwrap_or(false);
        self.reopen_output(None, &reason);
    }

    /// Открыть вывод заново (смена устройства, сброс медиасервисов) и продолжить текущий трек
    /// с той же позиции на новой частоте: старые деки писали в кольца со старой частотой,
    /// поэтому текущий трек готовится заново — как перемотка.
    fn reopen_output(&mut self, capture_rate: Option<u32>, reason: &str) {
        // Буферизованный вывод отдал звук наперёд: продолжаем с того, что слышно
        let position = match self.heard_now() {
            Some(h) if self.queue.get(h.index).is_some_and(|t| t.id == h.id) => {
                self.index = Some(h.index);
                h.position_ms
            }
            _ => self.position_ms(),
        };
        self.output.halt();
        self.heard.clear();
        self.deferred.borrow_mut().clear();
        for slot in 0..MAX_DECKS {
            if let Some(mut d) = self.decks[slot].take() {
                d.handle.stop();
            }
        }
        self.preps.clear();
        self.next = NextState::None;
        self.dj_plan = None;
        self.level_seq = 0;
        let capture_rate = capture_rate.or(self.capture_rate);
        let generation = self.output_gen.wrapping_add(1);
        let mut opened = open_output(
            &self.cfg,
            self.device_pref.as_deref(),
            capture_rate,
            self.volume,
            &self.capture,
            &self.issues_tx,
            generation,
            self.buffered_want,
        );
        if self.buffered_want
            && let Err(e) = &opened
        {
            // Буферизованный не открылся — обычный, звук важнее
            log::warn!("buffered output failed to open: {e}; using realtime");
            self.buffered_failed = true;
            self.buffered_want = false;
            opened = open_output(
                &self.cfg,
                self.device_pref.as_deref(),
                capture_rate,
                self.volume,
                &self.capture,
                &self.issues_tx,
                generation,
                false,
            );
        }
        match opened {
            Ok(parts) => {
                self.output_gen = generation;
                // Старый поток закрывается здесь: вместе с ним уходят микшер и кольца старых дек
                self.output = parts.output;
                self.ports = parts.ports;
                self.load = parts.load;
                self.device_rate = parts.rate;
                self.device_channels = parts.channels;
                self.output_device = parts.device;
                if matches!(self.cfg.output, OutputConfig::Capture { .. }) {
                    self.capture_rate = Some(parts.rate);
                }
                self.output_lost = false;
                self.output_reopens += 1;
                if matches!(self.transition, Transition::Dj { .. }) {
                    self.mixer(MixerCmd::Dj(true));
                }
                // Новый микшер: эквалайзер заново, затухание таймера сна — от новой позиции
                if self.eq.is_some() {
                    let t = EqTarget::new(self.eq.as_ref(), self.device_rate);
                    self.mixer(MixerCmd::Eq(t));
                }
                self.sleep_faded = false;
                if let Some(run) = &mut self.sleep {
                    run.fade_sent = false;
                }
                log::info!(
                    "output reopened ({reason}): {} Hz, {} ch, {}, {}, position {position} ms",
                    self.device_rate,
                    self.device_channels,
                    self.output_device.as_deref().unwrap_or("system default"),
                    if self.output.is_buffered() {
                        "buffered"
                    } else {
                        "realtime"
                    }
                );
                self.emit(Event::Output {
                    sample_rate: self.device_rate,
                    channels: self.device_channels,
                    reason: reason.into(),
                });
            }
            Err(e) => {
                log::warn!("could not reopen the output ({reason}): {e}");
                self.output = Output::None;
                self.output_device = None;
                self.output_lost = true;
                self.seek_target_ms = self.index.map(|_| position);
                if self.want_play && self.cfg.output.is_device() {
                    // Звук нужен: ждём системный вывод и пробуем снова, потом — с того же места
                    if self.index.is_some()
                        && matches!(self.state, PlaybackState::Idle | PlaybackState::Paused)
                    {
                        self.set_state(PlaybackState::Loading);
                    }
                    self.begin_outage();
                    return;
                }
                if self.want_play {
                    self.want_play = false;
                    self.set_state(PlaybackState::Paused);
                }
                self.set_buffering(false);
                self.emit(Event::Error {
                    index: self.index,
                    id: self.index.and_then(|i| self.id_at(i)),
                    message: e.to_string(),
                });
                return;
            }
        }
        let active = !matches!(self.state, PlaybackState::Idle | PlaybackState::Ended);
        if let Some(i) = self.index
            && active
        {
            self.seek_target_ms = Some(position);
            self.start_prep(i, position, Purpose::Main);
            if self.want_play {
                self.set_state(PlaybackState::Loading);
                self.resume_output();
            } else {
                self.pause_mixer();
            }
        }
    }

    // ---- деки ----

    fn main_slot(&self) -> Option<usize> {
        self.decks
            .iter()
            .position(|d| matches!(d, Some(d) if d.role == Role::Main))
    }

    fn pending_slot(&self) -> Option<usize> {
        self.decks
            .iter()
            .position(|d| matches!(d, Some(d) if d.role == Role::Pending))
    }

    fn free_slot(&self) -> Option<usize> {
        self.decks.iter().position(|d| d.is_none())
    }

    fn has_main_prep(&self) -> bool {
        self.preps.iter().any(|p| p.purpose == Purpose::Main)
    }

    fn detach_role(&mut self, role: Role) {
        let fade = self.declick();
        for slot in 0..MAX_DECKS {
            if matches!(&self.decks[slot], Some(d) if d.role == role) {
                self.mixer(MixerCmd::Detach {
                    slot,
                    fade_out: fade,
                });
                if let Some(d) = &mut self.decks[slot] {
                    d.role = Role::Outgoing;
                }
            }
        }
    }

    fn reset_next(&mut self) {
        match self.next {
            NextState::Waiting { .. } => self.detach_role(Role::Next),
            NextState::Chained(_) => {
                if let Some(m) = self.main_slot().or(self.pending_slot())
                    && let Some(d) = &self.decks[m]
                {
                    let _ = d.handle.tx.send(DeckCmd::Unchain);
                }
            }
            _ => {}
        }
        self.cancel_preps(Purpose::Next);
        self.next = NextState::None;
        self.dj_plan = None;
        self.update_expect_chain();
    }

    fn reset_all(&mut self) {
        for slot in 0..MAX_DECKS {
            if self.decks[slot].is_some() {
                self.mixer(MixerCmd::Detach {
                    slot,
                    fade_out: (self.device_rate / 200).max(1),
                });
                if let Some(d) = &mut self.decks[slot] {
                    d.role = Role::Outgoing;
                }
            }
        }
        self.preps.clear();
        self.next = NextState::None;
        self.dj_plan = None;
        self.seek_target_ms = None;
        self.set_buffering(false);
    }

    fn cancel_preps(&mut self, purpose: Purpose) {
        self.preps.retain(|p| p.purpose != purpose);
    }

    /// Ждать ли деке следующий трек в конце текущего (режим «без пауз»).
    fn update_expect_chain(&self) {
        let Some(m) = self.main_slot().or(self.pending_slot()) else {
            return;
        };
        let Some(d) = &self.decks[m] else { return };
        let last_index = d
            .handle
            .shared
            .segments()
            .last()
            .map(|s| s.index)
            .unwrap_or(d.first_index);
        let expect = self.transition == Transition::Gapless
            && self.next_index(last_index).is_some()
            && self.next != NextState::None;
        d.handle
            .shared
            .expect_chain
            .store(expect, Ordering::Relaxed);
    }

    fn start_prep(&mut self, index: usize, offset_ms: u64, purpose: Purpose) {
        self.start_prep_at(
            index,
            StartAt {
                offset_ms,
                tempo: None,
            },
            purpose,
        );
    }

    fn start_prep_at(&mut self, index: usize, at: StartAt, purpose: Purpose) {
        let StartAt { offset_ms, tempo } = at;
        let spec = self.queue[index].clone();
        let normalize = self.normalize;
        let target = self.cfg.target_lufs;
        let resolver = self.cfg.resolver.clone();
        let cache = self.cache.clone();
        self.prep_seq += 1;
        let id = self.prep_seq;
        let tx = self.prep_tx.clone();
        let _ = std::thread::Builder::new()
            .name("dw-prep".into())
            .spawn(move || {
                let res = (|| {
                    let mut spec = spec;
                    if spec.source == SourceSpec::Deferred {
                        let resolver = resolver
                            .ok_or_else(|| AudioError::Network("нет ссылки на файл".into()))?;
                        let r = resolver.resolve(&spec).map_err(AudioError::Network)?;
                        spec.source = SourceSpec::Url(r.url);
                        spec.loudness_lufs = r.loudness_lufs.or(spec.loudness_lufs);
                        spec.peak_db = r.peak_db.or(spec.peak_db);
                        spec.duration_ms = r.duration_ms.or(spec.duration_ms);
                    }
                    let gain = if normalize {
                        spec.gain_for(target)
                    } else {
                        1.0
                    };
                    let (source, ext) = cache.open_track(&spec)?;
                    let mut decoder = TrackDecoder::open(source, ext.as_deref())?;
                    // «Диджей» входит с сильной доли — с точностью до кадра, а не до миллисекунды
                    let frame = match &tempo {
                        Some(m) => {
                            (m.start() * f64::from(decoder.source_rate) / 1000.0).round() as u64
                        }
                        None => offset_ms * decoder.source_rate as u64 / 1000,
                    };
                    if frame > 0 {
                        decoder.seek_frame(frame)?;
                    }
                    Ok(PreparedTrack {
                        index,
                        spec,
                        decoder,
                        gain,
                        offset_ms,
                        tempo,
                    })
                })();
                let _ = tx.send(PrepResult { id, res });
            });
        if purpose == Purpose::Next {
            self.next = NextState::Preparing(index);
        }
        self.preps.push(Prep {
            id,
            index,
            offset_ms,
            purpose,
        });
        // Держим в памяти только нужное: текущий, следующий и предыдущий треки.
        let mut keep: Vec<String> = Vec::new();
        for j in [
            Some(index),
            self.index,
            self.index.and_then(|i| i.checked_sub(1)),
            self.index.and_then(|i| self.next_index(i)),
        ]
        .into_iter()
        .flatten()
        {
            if let Some(SourceSpec::Url(u)) = self.queue.get(j).map(|t| &t.source) {
                keep.push(u.clone());
            }
        }
        let keep_refs: Vec<&str> = keep.iter().map(String::as_str).collect();
        self.cache.retain(&keep_refs);
        self.cache.prefetch(&self.queue[index]);
    }

    fn on_prep(&mut self, r: PrepResult) {
        // Отменённые подготовки (перемотка, смена трека) просто отбрасываем.
        let Some(pos) = self.preps.iter().position(|p| p.id == r.id) else {
            return;
        };
        let p = self.preps.remove(pos);
        let (index, offset_ms, purpose) = (p.index, p.offset_ms, p.purpose);
        match r.res {
            Ok(p) => {
                self.consecutive_errors = 0;
                // Ссылку и громкость дал Resolver — запомним: перемотка возьмёт тот же кэш
                if let Some(q) = self.queue.get_mut(p.index)
                    && q.id == p.spec.id
                    && q.source != p.spec.source
                {
                    *q = p.spec.clone();
                }
                match purpose {
                    Purpose::Main => self.attach_main(p),
                    Purpose::Next => self.attach_next(p),
                }
            }
            Err(e) => {
                log::warn!("prepare #{index} failed: {e}");
                self.emit(Event::Error {
                    index: Some(index),
                    id: self.id_at(index),
                    message: e.to_string(),
                });
                self.failed.insert(index);
                self.consecutive_errors += 1;
                match purpose {
                    Purpose::Main => {
                        let next = self.next_index(index);
                        if let (Some(j), true) =
                            (next, self.consecutive_errors < 3 && offset_ms == 0)
                        {
                            self.go_to(j, ChangeReason::Auto);
                        } else {
                            self.want_play = false;
                            self.set_buffering(false);
                            self.set_state(PlaybackState::Idle);
                        }
                    }
                    Purpose::Next => {
                        self.next = NextState::None;
                        self.update_expect_chain();
                    }
                }
            }
        }
    }

    fn attach_main(&mut self, p: PreparedTrack) {
        let Some(slot) = self.free_slot() else {
            log::error!("no free deck slot");
            return;
        };
        let index = p.index;
        let first = p.spec.clone();
        let fade_in = if p.offset_ms > 0 { self.declick() } else { 0 };
        let info = p.decoder.describe();
        let ring = self.ms_to_frames(self.cfg.ring_ms as u64) as usize;
        match deck::spawn(p, self.device_rate, ring) {
            Ok((handle, link)) => {
                self.mixer(MixerCmd::Attach {
                    slot,
                    link,
                    rule: StartRule::Now,
                    fade_in,
                });
                self.decks[slot] = Some(DeckCtl {
                    handle,
                    role: Role::Pending,
                    first_index: index,
                    first,
                });
                self.emit(Event::TrackInfo { index, info });
                if self.want_play {
                    self.set_buffering(true);
                }
                self.update_expect_chain();
            }
            Err(e) => self.emit(Event::Error {
                index: Some(index),
                id: Some(first.id),
                message: e.to_string(),
            }),
        }
    }

    fn attach_next(&mut self, p: PreparedTrack) {
        let index = p.index;
        let NextState::Preparing(expected) = self.next else {
            return;
        };
        if expected != index {
            return;
        }
        if self.queue.get(index).is_none_or(|t| t.id != p.spec.id) {
            // Очередь поменялась, пока трек готовился: следующий подготовим заново
            self.next = NextState::None;
            return;
        }
        let main = self.main_slot().or(self.pending_slot());
        match (self.transition, main) {
            (Transition::Gapless, Some(m)) => {
                let alive = self.decks[m]
                    .as_ref()
                    .map(|d| !d.handle.shared.finished.load(Ordering::Acquire))
                    .unwrap_or(false);
                if alive {
                    if let Some(d) = &self.decks[m] {
                        let _ = d.handle.tx.send(DeckCmd::Chain(Box::new(p)));
                    }
                    self.next = NextState::Chained(index);
                    self.update_expect_chain();
                } else {
                    // Дека уже доиграла (сбой подготовки по времени) — играем следующий отдельной декой.
                    self.next = NextState::None;
                }
            }
            (Transition::Crossfade { ms }, Some(_)) => {
                let Some(slot) = self.free_slot() else { return };
                let ring = self.ms_to_frames(self.cfg.ring_ms as u64) as usize;
                let info = p.decoder.describe();
                let first = p.spec.clone();
                match deck::spawn(p, self.device_rate, ring) {
                    Ok((handle, link)) => {
                        let frames = self.ms_to_frames(ms as u64);
                        self.mixer(MixerCmd::Attach {
                            slot,
                            link,
                            rule: StartRule::BeforeMainEnd { frames },
                            fade_in: 0,
                        });
                        self.decks[slot] = Some(DeckCtl {
                            handle,
                            role: Role::Next,
                            first_index: index,
                            first,
                        });
                        self.next = NextState::Waiting { slot, index };
                        self.emit(Event::TrackInfo { index, info });
                    }
                    Err(e) => self.emit(Event::Error {
                        index: Some(index),
                        id: Some(first.id),
                        message: e.to_string(),
                    }),
                }
            }
            (Transition::Dj { .. }, Some(m)) => self.attach_dj(p, m),
            _ => self.next = NextState::None,
        }
    }

    /// Кадр главной деки `m`, на котором звучит место уходящего трека `track_ms`.
    fn main_frame_at(&self, m: usize, track_ms: f64) -> Option<u64> {
        let d = self.decks[m].as_ref()?;
        let segs = d.handle.shared.segments();
        let id = self
            .index
            .and_then(|i| self.queue.get(i))
            .map(|t| t.id.clone());
        let seg = segs
            .iter()
            .rev()
            .find(|s| Some(&s.track.id) == id.as_ref())
            .or(segs.last())?;
        let ms = seg.elapsed_ms(track_ms).max(0.0);
        Some(seg.deck_start + (ms * f64::from(self.device_rate) / 1000.0).round() as u64)
    }

    /// «Диджей»: входящая дека ждёт сильной доли уходящего, автоматика — начала перехода.
    fn attach_dj(&mut self, p: PreparedTrack, m: usize) {
        let index = p.index;
        let plan = match &self.dj_plan {
            Some(dj) if dj.index == index && dj.id == p.spec.id => dj.plan.clone(),
            _ => {
                self.next = NextState::None;
                return;
            }
        };
        let (Some(start_frame), Some(in_frame)) = (
            self.main_frame_at(m, plan.start_ms),
            self.main_frame_at(m, plan.in_at_ms()),
        ) else {
            self.next = NextState::None;
            return;
        };
        let read = self.ports.shared.frames_read[m].load(Ordering::Relaxed);
        if start_frame < read {
            log::warn!(
                "dj: transition start is {} ms late",
                (read - start_frame) * 1000 / u64::from(self.device_rate)
            );
        }
        let Some(slot) = self.free_slot() else { return };
        let ring = self.ms_to_frames(self.cfg.ring_ms as u64) as usize;
        let info = p.decoder.describe();
        let first = p.spec.clone();
        match deck::spawn(p, self.device_rate, ring) {
            Ok((handle, link)) => {
                self.mixer(MixerCmd::Attach {
                    slot,
                    link,
                    rule: StartRule::AtMainFrame { frame: in_frame },
                    fade_in: 0,
                });
                self.mixer(MixerCmd::Plan {
                    slot,
                    start_frame,
                    curves: plan.curves(),
                });
                self.decks[slot] = Some(DeckCtl {
                    handle,
                    role: Role::Next,
                    first_index: index,
                    first,
                });
                self.next = NextState::Waiting { slot, index };
                self.emit(Event::TrackInfo { index, info });
            }
            Err(e) => self.emit(Event::Error {
                index: Some(index),
                id: Some(first.id),
                message: e.to_string(),
            }),
        }
    }

    // ---- «Диджей»: анализ и выбор перехода ----

    fn analysis_key(t: &TrackSpec) -> String {
        t.stream_id.clone().unwrap_or_else(|| t.id.clone())
    }

    /// Анализ трека: уже есть, идёт или запрашивается сейчас (в фоне).
    fn analysis(&mut self, index: usize) -> Option<Option<Arc<Analysis>>> {
        let t = self.queue.get(index)?.clone();
        if let Some(a) = &t.analysis {
            return Some(Some(a.clone()));
        }
        let key = Self::analysis_key(&t);
        match self.analyses.get(&key) {
            Some(AnalysisState::Done(a)) => return Some(a.clone()),
            Some(AnalysisState::Pending) => return None,
            None => {}
        }
        let Some(source) = self.cfg.analysis.clone() else {
            return Some(None);
        };
        self.analyses.insert(key.clone(), AnalysisState::Pending);
        // Держим немного: волна длинная, а нужен лишь текущий и следующий
        if self.analyses.len() > 16 {
            let keep: HashSet<String> = [self.index, self.index.and_then(|i| self.next_index(i))]
                .into_iter()
                .flatten()
                .filter_map(|i| self.queue.get(i).map(Self::analysis_key))
                .chain([key.clone()])
                .collect();
            self.analyses.retain(|k, _| keep.contains(k));
        }
        let tx = self.analysis_tx.clone();
        let _ = std::thread::Builder::new()
            .name("dw-analysis".into())
            .spawn(move || {
                let a = match source.fetch(&t) {
                    Ok(a) => a.map(Arc::new),
                    Err(e) => {
                        log::info!("analysis of {} is not available: {e}", t.id);
                        None
                    }
                };
                let _ = tx.send((key, a));
            });
        None
    }

    /// Выбрать переход к следующему треку, как только известны оба анализа.
    fn maybe_plan(&mut self) {
        let Transition::Dj { expressiveness } = self.transition else {
            return;
        };
        if self.dj_plan.is_some() || self.next != NextState::None {
            return;
        }
        let Some(i) = self.index else { return };
        let Some(j) = self.next_index(i) else { return };
        // Главная дека ещё играет прежний трек (переход в разгаре)
        if self
            .current_segment()
            .is_none_or(|(_, seg, _)| seg.track.id != self.queue[i].id)
        {
            return;
        }
        let out = self.analysis(i);
        let inc = self.analysis(j);
        let pos = self.position_ms();
        let dur = self.duration_ms();
        let remaining = dur.map(|d| d.saturating_sub(pos));
        if (out.is_none() || inc.is_none()) && remaining.is_none_or(|r| r > DJ_WAIT_ANALYSIS_MS) {
            return;
        }
        let none = Analysis::default();
        let out = out.flatten();
        let inc = inc.flatten();
        // Входящий прошлого перехода мог ещё возвращаться к своему темпу: новый переход — после
        let settled = self
            .current_segment()
            .and_then(|(_, seg, _)| seg.tempo.map(|t| t.src_at(t.settled_at())))
            .unwrap_or(0.0);
        let input = PlanInput {
            out: out.as_deref().unwrap_or(&none),
            inc: inc.as_deref().unwrap_or(&none),
            out_duration_ms: dur.map(|d| d as f64),
            in_duration_ms: self.queue[j].duration_ms.map(|d| d as f64),
            expressiveness: f64::from(expressiveness.min(100)) / 100.0,
            earliest_ms: (pos as f64 + DJ_LEAD_MS).max(settled),
        };
        let plan = dalywave_transitions::plan(&input);
        log::info!("dj #{i} → #{j}: {}", plan.note);
        self.emit(Event::TransitionPlanned {
            index: i,
            next_index: j,
            technique: plan.technique.as_str(),
            start_ms: plan.start_ms,
            in_start_ms: plan.in_start_ms,
            length_ms: plan.length_ms,
            in_offset_ms: plan.in_offset_ms,
            tempo: plan.tempo.clone(),
            note: plan.note.clone(),
        });
        self.dj_plan = Some(DjPlan {
            index: j,
            id: self.queue[j].id.clone(),
            plan,
        });
    }

    fn poll_mixer_events(&mut self) {
        while let Ok(e) = self.ports.evt_rx.pop() {
            match e {
                MixerEvent::Started {
                    slot,
                    crossfade,
                    at,
                } => {
                    let Some((role, first_index, first)) = self.decks[slot]
                        .as_ref()
                        .map(|d| (d.role, d.first_index, d.first.clone()))
                    else {
                        continue;
                    };
                    // Прежняя главная дека уходит (кроссфейд или замена).
                    for (i, d) in self.decks.iter_mut().enumerate() {
                        if let Some(d) = d
                            && i != slot
                            && d.role == Role::Main
                        {
                            d.role = Role::Outgoing;
                        }
                    }
                    if let Some(d) = &mut self.decks[slot] {
                        d.role = Role::Main;
                    }
                    let mut index = first_index;
                    if role == Role::Next {
                        self.next = NextState::None;
                        self.dj_plan = None;
                        index = self.adopt(first_index, &first);
                        if self.index != Some(index) {
                            self.index = Some(index);
                            self.emit(Event::TrackChanged {
                                index,
                                track: first,
                                reason: ChangeReason::Auto,
                            });
                        }
                        log::info!("crossfade start into #{index} (crossfade={crossfade})");
                    }
                    self.seek_target_ms = None;
                    if let Some(t) = self.t_cmd.take() {
                        let latency_ms = at.saturating_duration_since(t).as_secs_f64() * 1000.0;
                        self.emit(Event::FirstAudio { index, latency_ms });
                    }
                    self.set_buffering(false);
                    if self.want_play {
                        self.set_state(PlaybackState::Playing);
                    }
                    self.update_expect_chain();
                    self.emit_position(true);
                }
                MixerEvent::Ended { slot } => {
                    let was_main = matches!(&self.decks[slot], Some(d) if d.role == Role::Main);
                    if let Some(d) = &mut self.decks[slot] {
                        d.role = Role::Outgoing;
                    }
                    if was_main {
                        let waiting = matches!(self.next, NextState::Waiting { .. });
                        if !waiting {
                            let last = self.index.unwrap_or(0);
                            match self.next_index(last) {
                                Some(j) if self.state == PlaybackState::Playing => {
                                    self.go_to(j, ChangeReason::Auto)
                                }
                                _ => {
                                    self.want_play = false;
                                    self.set_buffering(false);
                                    self.set_state(PlaybackState::Ended);
                                    self.emit(Event::QueueEnded);
                                }
                            }
                        }
                    }
                }
                MixerEvent::Removed { slot } => {
                    if let Some(mut d) = self.decks[slot].take() {
                        if let Some(err) = d.handle.shared.error.lock().ok().and_then(|g| g.clone())
                            && !err.contains("остановлено")
                        {
                            self.emit(Event::Error {
                                index: Some(d.first_index),
                                id: Some(d.first.id.clone()),
                                message: err,
                            });
                        }
                        d.handle.stop();
                    }
                }
                MixerEvent::Starved { slot } => {
                    if matches!(&self.decks[slot], Some(d) if d.role == Role::Main) {
                        self.set_buffering(true);
                    }
                }
                MixerEvent::Resumed { slot } => {
                    if matches!(&self.decks[slot], Some(d) if d.role == Role::Main) {
                        self.set_buffering(false);
                    }
                }
                MixerEvent::SleepFaded => {
                    if self.sleep.is_some() {
                        self.sleep_faded = true;
                    }
                }
            }
        }
    }

    /// «Без пауз»: смена трека внутри деки, когда воспроизведение перешло границу сегмента.
    fn track_segments(&mut self) {
        let Some(m) = self.main_slot() else { return };
        let Some(d) = &self.decks[m] else { return };
        let read = self.ports.shared.frames_read[m].load(Ordering::Relaxed);
        let segs = d.handle.shared.segments();
        let Some(seg) = segs.iter().rev().find(|s| s.deck_start <= read).cloned() else {
            return;
        };
        let current = self.index.and_then(|i| self.queue.get(i)).map(|t| &t.id);
        if seg.deck_start > 0 && current != Some(&seg.track.id) {
            let i = self.adopt(seg.index, &seg.track);
            self.index = Some(i);
            if matches!(self.next, NextState::Chained(_)) {
                self.next = NextState::None;
            }
            self.emit(Event::TrackChanged {
                index: i,
                track: seg.track.clone(),
                reason: ChangeReason::Auto,
            });
            self.emit(Event::TrackInfo {
                index: i,
                info: seg.info.clone(),
            });
            self.update_expect_chain();
        }
    }

    fn maybe_preload(&mut self) {
        if self.next != NextState::None {
            return;
        }
        let Some(i) = self.index else { return };
        let Some(j) = self.next_index(i) else { return };
        if self.main_slot().is_none() && self.pending_slot().is_none() {
            return;
        }
        if matches!(self.transition, Transition::Dj { .. }) {
            let Some(dj) = &self.dj_plan else { return };
            if dj.index != j || self.queue[j].id != dj.id {
                self.dj_plan = None;
                return;
            }
            if (self.position_ms() as f64) < dj.plan.start_ms - DJ_PREP_MS {
                return;
            }
            let at = StartAt {
                offset_ms: dj.plan.in_start_ms.max(0.0).round() as u64,
                tempo: Some(dj.plan.tempo.clone()),
            };
            self.start_prep_at(j, at, Purpose::Next);
            return;
        }
        let remaining = self
            .duration_ms()
            .map(|d| d.saturating_sub(self.position_ms()));
        let window = self.cfg.preload_secs as u64 * 1000
            + match self.transition {
                Transition::Crossfade { ms } => ms as u64,
                Transition::Gapless | Transition::Dj { .. } => 0,
            };
        if remaining.is_none_or(|r| r <= window) {
            self.start_prep(j, 0, Purpose::Next);
            self.update_expect_chain();
        }
    }

    // ---- позиция и уровень ----

    fn current_segment(&self) -> Option<(usize, deck::Segment, u64)> {
        let m = self.main_slot()?;
        let d = self.decks[m].as_ref()?;
        let read = self.ports.shared.frames_read[m].load(Ordering::Relaxed);
        let segs = d.handle.shared.segments();
        let seg = segs.iter().rev().find(|s| s.deck_start <= read)?.clone();
        Some((m, seg, read))
    }

    fn position_ms(&self) -> u64 {
        if let Some(t) = self.seek_target_ms {
            return t;
        }
        match self.current_segment() {
            Some((_, seg, read)) => {
                let elapsed = (read - seg.deck_start) as f64 * 1000.0 / f64::from(self.device_rate);
                seg.track_ms(elapsed).max(0.0) as u64
            }
            None => 0,
        }
    }

    fn duration_ms(&self) -> Option<u64> {
        let i = self.index?;
        let id = &self.queue.get(i)?.id;
        if let Some((_, seg, _)) = self.current_segment()
            && &seg.track.id == id
            && let Some(d) = seg.duration_ms
        {
            return Some(d);
        }
        self.queue.get(i).and_then(|t| t.duration_ms)
    }

    fn emit_position(&mut self, force: bool) {
        let playing = self.state == PlaybackState::Playing;
        if !force && (!playing || self.last_pos_emit.elapsed() < POSITION_EVERY) {
            return;
        }
        let (Some(index), Some(id), position_ms, duration_ms) = self.reported() else {
            return;
        };
        self.last_pos_emit = Instant::now();
        self.emit(Event::Position {
            index,
            id,
            position_ms,
            duration_ms,
        });
    }

    fn emit_level(&mut self) {
        if !self.levels
            || self.state != PlaybackState::Playing
            || self.last_level_emit.elapsed() < LEVEL_EVERY
        {
            return;
        }
        let (seq, level) = self.ports.shared.level.get();
        if seq == self.level_seq {
            return;
        }
        self.level_seq = seq;
        self.last_level_emit = Instant::now();
        self.emit(Event::Level(level));
    }

    // ---- таймер сна ----

    fn sleep_fade_ms(&self) -> u64 {
        u64::from(self.cfg.sleep_fade_ms)
    }

    fn sleep_status(&self) -> Option<SleepStatus> {
        let run = self.sleep.as_ref()?;
        let remaining_ms = match run.timer {
            SleepTimer::After { .. } => run
                .deadline
                .map(|d| d.saturating_duration_since(Instant::now()).as_millis() as u64),
            SleepTimer::EndOfTrack => self
                .duration_ms()
                .map(|d| d.saturating_sub(self.position_ms())),
        };
        let fading = run.fade_sent
            && (run.timer != SleepTimer::EndOfTrack
                || remaining_ms.is_some_and(|r| r <= self.sleep_fade_ms()));
        Some(SleepStatus {
            timer: Some(run.timer),
            remaining_ms,
            fading,
            finished: false,
        })
    }

    fn emit_sleep(&self) {
        let status = self.sleep_status().unwrap_or(SleepStatus {
            timer: None,
            remaining_ms: None,
            fading: false,
            finished: false,
        });
        self.emit(Event::Sleep(status));
    }

    fn set_sleep(&mut self, timer: Option<SleepTimer>) {
        // Затухание уже шло — громкость возвращается
        if self.sleep.as_ref().is_some_and(|r| r.fade_sent) {
            let ramp = self.declick() * 40;
            self.mixer(MixerCmd::Sleep(SleepCmd::Cancel { ramp }));
        }
        self.sleep_faded = false;
        self.sleep = timer.map(|timer| SleepRun {
            timer,
            deadline: match timer {
                SleepTimer::After { ms } => Some(Instant::now() + Duration::from_millis(ms)),
                SleepTimer::EndOfTrack => None,
            },
            track_id: self.index.and_then(|i| self.id_at(i)),
            track_index: self.index,
            fade_sent: false,
        });
        log::info!("sleep timer: {timer:?}");
        self.emit_sleep();
        // Короткий таймер — затухание сразу, не дожидаясь следующего тика
        self.sleep_tick();
    }

    /// Команда человека (перемотка, другой трек, новая очередь): затухание к концу трека,
    /// привязанное к кадрам старой деки, больше не к месту — таймер ставится заново.
    fn sleep_rearm(&mut self) {
        let Some(run) = &mut self.sleep else { return };
        if run.timer == SleepTimer::EndOfTrack && run.fade_sent && !self.sleep_faded {
            run.fade_sent = false;
            let ramp = self.declick() * 4;
            self.mixer(MixerCmd::Sleep(SleepCmd::Cancel { ramp }));
            self.emit_sleep();
        }
    }

    fn sleep_tick(&mut self) {
        if self.sleep.is_none() {
            return;
        }
        if self.sleep_faded {
            self.sleep_faded = false;
            self.finish_sleep();
            return;
        }
        let fade = self.sleep_fade_ms();
        let playing = self.want_play && self.state == PlaybackState::Playing;
        let index = self.index;
        let current_id = index.and_then(|i| self.id_at(i));
        let main = self.main_slot();
        let duration = self.duration_ms();
        let position = self.position_ms();
        let Some(run) = &mut self.sleep else { return };
        match run.timer {
            SleepTimer::After { .. } => {
                let left = run.deadline.map_or(0, |d| {
                    d.saturating_duration_since(Instant::now()).as_millis() as u64
                });
                if !run.fade_sent {
                    if left <= fade && playing {
                        run.fade_sent = true;
                        let len = self.ms_to_frames(left.max(1)).min(u64::from(u32::MAX)) as u32;
                        self.mixer(MixerCmd::Sleep(SleepCmd::Fade { at: None, len }));
                        self.emit_sleep();
                    } else if left == 0 {
                        // На паузе таймер истёк сам: музыка и так стоит
                        self.finish_sleep();
                    }
                } else if left == 0 && !playing {
                    // Поставили паузу посреди затухания: оно уже не дойдёт до тишины
                    self.mixer(MixerCmd::Sleep(SleepCmd::Cancel { ramp: 0 }));
                    self.finish_sleep();
                }
            }
            SleepTimer::EndOfTrack => {
                if run.fade_sent {
                    return;
                }
                // Человек сам перешёл к другому треку — таймер про его конец
                if current_id != run.track_id {
                    run.track_id = current_id;
                    run.track_index = index;
                }
                let (Some(m), Some(d), true) = (main, duration, playing) else {
                    return;
                };
                // Затухание уходит микшеру сразу, привязанным к кадру деки: оно начнётся точно за
                // `fade` до конца трека. Перемотка и другой трек ставят его заново (sleep_rearm)
                let remaining = d.saturating_sub(position);
                run.fade_sent = true;
                run.track_index = index;
                let at = if remaining > fade {
                    self.main_frame_at(m, d.saturating_sub(fade) as f64)
                        .map(|frame| (m, frame))
                } else {
                    None
                };
                let len_ms = if at.is_some() { fade } else { remaining.max(1) };
                let len = self.ms_to_frames(len_ms).min(u64::from(u32::MAX)) as u32;
                self.mixer(MixerCmd::Sleep(SleepCmd::Fade { at, len }));
                self.emit_sleep();
            }
        }
    }

    /// Затухание дошло до тишины (или таймер истёк на паузе): пауза. «В конце трека» — следующий
    /// трек (если уже начался) встаёт в начало.
    fn finish_sleep(&mut self) {
        let Some(run) = self.sleep.take() else { return };
        log::info!("sleep timer: done");
        self.pause();
        if run.timer == SleepTimer::EndOfTrack
            && run.track_index.is_some()
            && self.index != run.track_index
            && self.position_ms() > 0
        {
            self.seek(0);
        }
        self.emit(Event::Sleep(SleepStatus {
            timer: None,
            remaining_ms: None,
            fading: false,
            finished: true,
        }));
    }

    fn update_snapshot(&self) {
        let (avg, max) = self.load.get();
        let (index, id, position_ms, duration_ms) = self.reported();
        let clock = self.clock();
        let ahead = clock - self.output.audible(clock);
        let snap = Snapshot {
            state: self.state,
            index,
            stream_id: index
                .and_then(|i| self.queue.get(i))
                .and_then(|t| t.stream_id.clone()),
            id: id.map(|id| item_id(&id).to_string()),
            position_ms,
            duration_ms,
            volume: self.volume,
            transition: self.transition,
            queue_len: self.queue.len(),
            buffering: self.buffering,
            device_rate: self.device_rate,
            device_channels: self.device_channels,
            underruns: self.ports.shared.underruns.load(Ordering::Relaxed),
            callback_load_avg: avg,
            callback_load_max: max,
            buffer_frames: self.load.buffer_frames.load(Ordering::Relaxed),
            output_frames: self.ports.shared.clock.load(Ordering::Relaxed),
            output_reopens: self.output_reopens,
            output_device: self.output_device.clone(),
            eq: self.eq,
            sleep: self.sleep_status(),
            repeat_one: self.repeat_one,
            has_next: self.index.and_then(|i| self.user_next_index(i)).is_some(),
            has_previous: self
                .index
                .and_then(|i| self.user_previous_index(i))
                .is_some(),
            output_buffered: self.output.is_buffered(),
            output_ahead_ms: (ahead * 1000 / u64::from(self.device_rate.max(1))) as u32,
            output_stalled: self.outage.is_some(),
        };
        if let Ok(mut s) = self.snap.lock() {
            *s = snap;
        }
    }
}
