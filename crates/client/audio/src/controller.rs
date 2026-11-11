//! Поток контроллера: очередь, жизненный цикл дек, подготовка следующего трека, события,
//! повторное открытие вывода.
//!
//! Аудиоколбэк он не трогает: общается с микшером через кольца команд и событий.
//! Тяжёлое (ссылка у сервера, открытие источника, разбор контейнера, перемотка) идёт в отдельных
//! потоках подготовки, чтобы команды обрабатывались сразу.

use std::collections::HashSet;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crossbeam_channel::{Receiver, Sender};

use crate::deck::{self, DeckCmd, DeckHandle, PreparedTrack};
use crate::decode::TrackDecoder;
use crate::mixer::{MAX_DECKS, Mixer, MixerCmd, MixerEvent, MixerPorts, StartRule};
use crate::output::{self, LoadStats, Output, OutputIssue};
use crate::source::SourceCache;
use crate::{
    AudioError, ChangeReason, Command, EngineConfig, Event, OutputConfig, PlaybackState, Snapshot,
    SourceSpec, TrackSpec, Transition,
};

const POSITION_EVERY: Duration = Duration::from_millis(250);
/// Окно измерителя — 50 мс, контроллер просыпается раз в 20 мс: так выходит ~20 событий в секунду
const LEVEL_EVERY: Duration = Duration::from_millis(40);
/// Пауза дольше этого — останавливаем поток устройства (на iOS система сможет усыпить приложение).
const STOP_OUTPUT_AFTER: Duration = Duration::from_secs(3);

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
    buffering: bool,
    seek_target_ms: Option<u64>,
    consecutive_errors: u32,
    shutdown: bool,
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

fn open_output(
    cfg: &EngineConfig,
    capture_rate: Option<u32>,
    volume: f32,
    capture: &Arc<Mutex<Vec<f32>>>,
    issues: &Sender<Issue>,
    generation: u32,
) -> Result<OutputParts, AudioError> {
    let load = Arc::new(LoadStats::default());
    match cfg.output {
        OutputConfig::Device { buffer_frames } => {
            let (device, supported) = output::query_device()?;
            let rate = supported.sample_rate();
            let (mixer, ports) = Mixer::new(rate, cfg.start_threshold_ms, volume);
            let (tx, rx) = crossbeam_channel::unbounded::<OutputIssue>();
            let (output, info) =
                Output::open_device(device, supported, buffer_frames, mixer, load.clone(), tx)?;
            // Колбэк ошибок cpal живёт в чужом потоке: помечаем его сообщения номером потока
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
            Ok(OutputParts {
                output,
                rate: info.sample_rate,
                channels: info.channels,
                ports,
                load,
            })
        }
        OutputConfig::Capture { sample_rate } => {
            let rate = capture_rate.unwrap_or(sample_rate);
            let (mixer, ports) = Mixer::new(rate, cfg.start_threshold_ms, volume);
            Ok(OutputParts {
                output: Output::open_capture(mixer, capture.clone()),
                rate,
                channels: 2,
                ports,
                load,
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
        let parts = open_output(&cfg, None, cfg.volume, &capture, &issues_tx, 0)?;
        log::info!("output: {} Hz, {} ch", parts.rate, parts.channels);
        let (prep_tx, prep_rx) = crossbeam_channel::unbounded();
        Ok(Self {
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
            output_lost: false,
            output_reopens: 0,
            capture_rate: None,
            device_rate: parts.rate,
            device_channels: parts.channels,
            load: parts.load,
            ports: parts.ports,
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
            buffering: false,
            seek_target_ms: None,
            consecutive_errors: 0,
            shutdown: false,
        })
    }

    fn emit(&self, e: Event) {
        let _ = self.ev.send(e);
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
        self.track_segments();
        self.maybe_preload();
        self.emit_position(false);
        self.emit_level();
        let undecided = self.index.and_then(|i| self.next_index(i)).is_some()
            && self.next == NextState::None
            && (self.main_slot().is_some() || self.pending_slot().is_some());
        let hold =
            !self.preps.is_empty() || undecided || matches!(self.next, NextState::Preparing(_));
        self.ports.shared.hold.store(hold, Ordering::Release);
        if let Some(t) = self.paused_since
            && t.elapsed() > STOP_OUTPUT_AFTER
            && self.output.is_running()
        {
            self.output.stop();
        }
        self.update_snapshot();
    }

    // ---- команды ----

    fn handle(&mut self, cmd: Command) {
        match cmd {
            Command::LoadQueue {
                tracks,
                start,
                start_ms,
                autoplay,
            } => self.load(tracks, start, start_ms, autoplay),
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
            Command::Seek(ms) => self.seek(ms),
            Command::Next => {
                if let Some(i) = self.index
                    && let Some(j) = self.next_index(i)
                {
                    self.go_to(j, ChangeReason::User);
                }
            }
            Command::Previous => {
                if let Some(i) = self.index {
                    if self.position_ms() > 3000 || i == 0 {
                        self.seek(0);
                    } else {
                        self.go_to(i - 1, ChangeReason::User);
                    }
                }
            }
            Command::SetVolume(v) => {
                self.volume = v;
                let ramp = (self.device_rate / 50).max(1);
                self.mixer(MixerCmd::Volume { gain: v, ramp });
            }
            Command::SetTransition(t) => {
                if t != self.transition {
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
            Command::Stop => {
                self.reset_all();
                self.want_play = false;
                self.pause_mixer();
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
        self.emit_position(true);
    }

    /// Всё после текущего трека — новое. Подготовленный следующий остаётся, если он тот же.
    fn set_upcoming(&mut self, tracks: Vec<TrackSpec>) {
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
        self.queue.truncate(i + 1);
        self.queue.extend(tracks);
        self.failed.retain(|&j| j <= i);
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
            if self.output_lost {
                self.want_play = false;
            }
            return;
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
        self.output.start();
        let fade = self.declick();
        self.mixer(MixerCmd::Resume { fade });
    }

    fn pause_mixer(&mut self) {
        let fade = self.declick() * 2;
        self.mixer(MixerCmd::Pause { fade });
        self.paused_since = Some(Instant::now());
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
        if self.state == PlaybackState::Ended {
            self.set_state(PlaybackState::Paused);
        }
        self.seek_target_ms = Some(ms);
        self.start_prep(i, ms, Purpose::Main);
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
    }

    fn next_index(&self, i: usize) -> Option<usize> {
        ((i + 1)..self.queue.len()).find(|j| !self.failed.contains(j))
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
                }
                OutputIssue::Lost(msg) => {
                    // Звук не должен внезапно пойти из другого устройства: пауза, как при
                    // отключении наушников. Вывод откроем заново по команде play.
                    log::warn!("output device lost: {msg}");
                    if self.want_play {
                        self.pause();
                    }
                    self.output_lost = true;
                }
                OutputIssue::Invalidated(msg) => {
                    log::warn!("output stream invalidated: {msg}");
                    reopen = Some(msg);
                }
                OutputIssue::Other(msg) => log::debug!("output: {msg}"),
            }
        }
        if let Some(reason) = reopen {
            self.reopen_output(None, &reason);
        }
    }

    /// Открыть вывод заново (смена устройства, сброс медиасервисов) и продолжить текущий трек
    /// с той же позиции на новой частоте: старые деки писали в кольца со старой частотой,
    /// поэтому текущий трек готовится заново — как перемотка.
    fn reopen_output(&mut self, capture_rate: Option<u32>, reason: &str) {
        self.output.halt();
        let position = self.position_ms();
        for slot in 0..MAX_DECKS {
            if let Some(mut d) = self.decks[slot].take() {
                d.handle.stop();
            }
        }
        self.preps.clear();
        self.next = NextState::None;
        self.level_seq = 0;
        let capture_rate = capture_rate.or(self.capture_rate);
        let generation = self.output_gen.wrapping_add(1);
        match open_output(
            &self.cfg,
            capture_rate,
            self.volume,
            &self.capture,
            &self.issues_tx,
            generation,
        ) {
            Ok(parts) => {
                self.output_gen = generation;
                // Старый поток закрывается здесь: вместе с ним уходят микшер и кольца старых дек
                self.output = parts.output;
                self.ports = parts.ports;
                self.load = parts.load;
                self.device_rate = parts.rate;
                self.device_channels = parts.channels;
                if matches!(self.cfg.output, OutputConfig::Capture { .. }) {
                    self.capture_rate = Some(parts.rate);
                }
                self.output_lost = false;
                self.output_reopens += 1;
                log::info!(
                    "output reopened ({reason}): {} Hz, {} ch, position {position} ms",
                    parts.rate,
                    parts.channels
                );
                self.emit(Event::Output {
                    sample_rate: parts.rate,
                    channels: parts.channels,
                    reason: reason.into(),
                });
            }
            Err(e) => {
                log::warn!("could not reopen the output ({reason}): {e}");
                self.output = Output::None;
                self.output_lost = true;
                self.seek_target_ms = self.index.map(|_| position);
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
                    if offset_ms > 0 {
                        let frame = offset_ms * decoder.source_rate as u64 / 1000;
                        decoder.seek_frame(frame)?;
                    }
                    Ok(PreparedTrack {
                        index,
                        spec,
                        decoder,
                        gain,
                        offset_ms,
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
            _ => self.next = NextState::None,
        }
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
        let remaining = self
            .duration_ms()
            .map(|d| d.saturating_sub(self.position_ms()));
        let window = self.cfg.preload_secs as u64 * 1000
            + match self.transition {
                Transition::Crossfade { ms } => ms as u64,
                Transition::Gapless => 0,
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
                seg.offset_ms + (read - seg.deck_start) * 1000 / self.device_rate as u64
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
        let Some(index) = self.index else { return };
        let Some(id) = self.id_at(index) else { return };
        self.last_pos_emit = Instant::now();
        self.emit(Event::Position {
            index,
            id,
            position_ms: self.position_ms(),
            duration_ms: self.duration_ms(),
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

    fn update_snapshot(&self) {
        let (avg, max) = self.load.get();
        let snap = Snapshot {
            state: self.state,
            index: self.index,
            id: self.index.and_then(|i| self.id_at(i)),
            position_ms: self.position_ms(),
            duration_ms: self.duration_ms(),
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
        };
        if let Ok(mut s) = self.snap.lock() {
            *s = snap;
        }
    }
}
