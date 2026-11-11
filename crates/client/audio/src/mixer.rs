//! Микшер N дек. Работает в аудиоколбэке: без выделения памяти, без блокировок и системных
//! вызовов. Команды приходят через SPSC-кольцо, события и освободившиеся деки уходят обратно
//! через такие же кольца (память освобождает контроллер, не аудиопоток).
//!
//! Старт деки задаётся правилом: «сейчас» или «за N кадров до конца главной деки» (кроссфейд).
//! Блок колбэка режется так, чтобы точка старта попала точно в нужный кадр.
//! Для режима «Диджей» сюда добавятся правила по позиции другой деки (доли), эквалайзер и
//! фильтры на деку; растяжение по времени — в потоке деки, до кольца.
//!
//! Уровень для волны интерфейса меряется здесь же, после паузы и до общей громкости.

use std::f32::consts::FRAC_PI_2;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::time::Instant;

use crate::deck::{DeckLink, UNKNOWN};
use crate::meter::{LevelCell, Meter};

pub(crate) const MAX_DECKS: usize = 4;
pub(crate) const NO_SLOT: usize = usize::MAX;

/// Равномощная пара усилений для кроссфейда в точке t ∈ [0, 1]: (уходящий, входящий).
/// cos² + sin² = 1 — суммарная мощность некоррелированных сигналов постоянна.
pub fn equal_power(t: f32) -> (f32, f32) {
    let t = t.clamp(0.0, 1.0);
    ((t * FRAC_PI_2).cos(), (t * FRAC_PI_2).sin())
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum StartRule {
    Now,
    /// Стартовать, когда у главной деки останется `frames` кадров, с равномощным кроссфейдом.
    BeforeMainEnd {
        frames: u64,
    },
}

pub(crate) enum MixerCmd {
    Attach {
        slot: usize,
        link: DeckLink,
        rule: StartRule,
        fade_in: u32,
    },
    StartNow {
        slot: usize,
        fade_in: u32,
    },
    Detach {
        slot: usize,
        fade_out: u32,
    },
    Pause {
        fade: u32,
    },
    Resume {
        fade: u32,
    },
    Volume {
        gain: f32,
        ramp: u32,
    },
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum MixerEvent {
    Started {
        slot: usize,
        crossfade: bool,
        at: Instant,
    },
    /// Дека доиграла до конца (данные кончились, дека отработала).
    Ended {
        slot: usize,
    },
    /// Слот освобождён (после затухания при отсоединении или кроссфейде).
    Removed {
        slot: usize,
    },
    Starved {
        slot: usize,
    },
    Resumed {
        slot: usize,
    },
}

/// Что микшер публикует для контроллера.
pub(crate) struct MixerShared {
    pub frames_read: [AtomicU64; MAX_DECKS],
    pub main: AtomicUsize,
    pub clock: AtomicU64,
    pub underruns: AtomicU64,
    pub lost_events: AtomicU64,
    /// Тестовый режим: контроллер ещё готовит следующий трек — захват ждёт (в реальном времени
    /// подготовка идёт за 30 с до конца, а захват рендерит в сотни раз быстрее).
    pub hold: AtomicBool,
    pub level: LevelCell,
}

impl MixerShared {
    pub fn new() -> Self {
        Self {
            frames_read: Default::default(),
            main: AtomicUsize::new(NO_SLOT),
            clock: AtomicU64::new(0),
            underruns: AtomicU64::new(0),
            lost_events: AtomicU64::new(0),
            hold: AtomicBool::new(false),
            level: LevelCell::default(),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Shape {
    Linear,
    EqualPowerIn,
    EqualPowerOut,
}

#[derive(Clone, Copy, Debug)]
struct Env {
    gain: f32,
    from: f32,
    to: f32,
    pos: u32,
    len: u32,
    shape: Shape,
    active: bool,
}

impl Env {
    const fn fixed(gain: f32) -> Self {
        Self {
            gain,
            from: gain,
            to: gain,
            pos: 0,
            len: 0,
            shape: Shape::Linear,
            active: false,
        }
    }

    fn linear(&mut self, to: f32, len: u32) {
        if len == 0 {
            *self = Self::fixed(to);
            return;
        }
        *self = Self {
            gain: self.gain,
            from: self.gain,
            to,
            pos: 0,
            len,
            shape: Shape::Linear,
            active: true,
        };
    }

    fn equal_power(&mut self, fade_in: bool, len: u32) {
        let len = len.max(1);
        let (shape, gain, to) = if fade_in {
            (Shape::EqualPowerIn, 0.0, 1.0)
        } else {
            (Shape::EqualPowerOut, self.gain, 0.0)
        };
        *self = Self {
            gain,
            from: gain,
            to,
            pos: 0,
            len,
            shape,
            active: true,
        };
    }

    #[inline]
    fn next(&mut self) -> f32 {
        if !self.active {
            return self.gain;
        }
        self.pos += 1;
        let t = self.pos as f32 / self.len as f32;
        self.gain = match self.shape {
            Shape::Linear => self.from + (self.to - self.from) * t,
            Shape::EqualPowerIn => equal_power(t).1,
            Shape::EqualPowerOut => self.from * equal_power(t).0,
        };
        if self.pos >= self.len {
            self.active = false;
            self.gain = self.to;
        }
        self.gain
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum SlotState {
    Empty,
    Waiting(StartRule),
    Playing,
}

impl PartialEq for StartRule {
    fn eq(&self, other: &Self) -> bool {
        matches!(
            (self, other),
            (StartRule::Now, StartRule::Now)
                | (
                    StartRule::BeforeMainEnd { .. },
                    StartRule::BeforeMainEnd { .. }
                )
        )
    }
}

struct Slot {
    link: Option<DeckLink>,
    state: SlotState,
    env: Env,
    fade_in: u32,
    frames_read: u64,
    starved: bool,
    remove_after_fade: bool,
}

impl Slot {
    const fn empty() -> Self {
        Self {
            link: None,
            state: SlotState::Empty,
            env: Env::fixed(1.0),
            fade_in: 0,
            frames_read: 0,
            starved: false,
            remove_after_fade: false,
        }
    }

    fn avail(&self) -> usize {
        self.link.as_ref().map(|l| l.cons.slots() / 2).unwrap_or(0)
    }

    fn finished(&self) -> bool {
        self.link
            .as_ref()
            .map(|l| l.shared.finished.load(Ordering::Acquire))
            .unwrap_or(true)
    }

    /// Сколько кадров осталось до конца деки, если известно.
    fn remaining(&self) -> u64 {
        let Some(l) = &self.link else { return 0 };
        // Порядок важен: сначала флаг конца, потом заполненность (дека пишет данные до флага).
        if l.shared.finished.load(Ordering::Acquire) {
            return (l.cons.slots() / 2) as u64;
        }
        let total = l.shared.total_frames.load(Ordering::Relaxed);
        if total == UNKNOWN {
            UNKNOWN
        } else {
            total.saturating_sub(self.frames_read)
        }
    }
}

pub(crate) struct Mixer {
    slots: [Slot; MAX_DECKS],
    main: usize,
    main_ended: bool,
    cmd_rx: rtrb::Consumer<MixerCmd>,
    evt_tx: rtrb::Producer<MixerEvent>,
    garbage_tx: rtrb::Producer<DeckLink>,
    shared: Arc<MixerShared>,
    start_threshold: usize,
    resume_threshold: usize,
    paused: bool,
    pause_env: Env,
    volume: Env,
    declick: u32,
    meter: Meter,
}

pub(crate) struct MixerPorts {
    pub cmd_tx: rtrb::Producer<MixerCmd>,
    pub evt_rx: rtrb::Consumer<MixerEvent>,
    pub garbage_rx: rtrb::Consumer<DeckLink>,
    pub shared: Arc<MixerShared>,
}

impl Mixer {
    pub fn new(sample_rate: u32, start_threshold_ms: u32, volume: f32) -> (Self, MixerPorts) {
        let (cmd_tx, cmd_rx) = rtrb::RingBuffer::new(64);
        let (evt_tx, evt_rx) = rtrb::RingBuffer::new(256);
        let (garbage_tx, garbage_rx) = rtrb::RingBuffer::new(32);
        let shared = Arc::new(MixerShared::new());
        let start_threshold = (sample_rate as u64 * start_threshold_ms as u64 / 1000) as usize;
        let mixer = Self {
            slots: [Slot::empty(), Slot::empty(), Slot::empty(), Slot::empty()],
            main: NO_SLOT,
            main_ended: false,
            cmd_rx,
            evt_tx,
            garbage_tx,
            shared: shared.clone(),
            start_threshold: start_threshold.max(1),
            // После недогрузки ждём запаса побольше, чтобы не «заикаться».
            resume_threshold: (start_threshold * 2).max(1),
            // Стартуем на паузе: контроллер снимет её по команде play.
            paused: true,
            pause_env: Env::fixed(0.0),
            volume: Env::fixed(volume),
            declick: (sample_rate / 200).max(1),
            meter: Meter::new(sample_rate),
        };
        (
            mixer,
            MixerPorts {
                cmd_tx,
                evt_rx,
                garbage_rx,
                shared,
            },
        )
    }

    fn event(&mut self, e: MixerEvent) {
        if self.evt_tx.push(e).is_err() {
            self.shared.lost_events.fetch_add(1, Ordering::Relaxed);
        }
    }

    fn remove(&mut self, i: usize) {
        if let Some(link) = self.slots[i].link.take()
            && let Err(rtrb::PushError::Full(link)) = self.garbage_tx.push(link)
        {
            // Освобождать память в аудиопотоке нельзя; кольцо мусора не должно переполняться.
            std::mem::forget(link);
        }
        self.slots[i] = Slot::empty();
        self.shared.frames_read[i].store(0, Ordering::Relaxed);
        if self.main == i {
            self.main = NO_SLOT;
        }
        self.event(MixerEvent::Removed { slot: i });
    }

    fn drain_commands(&mut self) {
        for _ in 0..32 {
            let Ok(cmd) = self.cmd_rx.pop() else { break };
            match cmd {
                MixerCmd::Attach {
                    slot,
                    link,
                    rule,
                    fade_in,
                } => {
                    if slot >= MAX_DECKS {
                        continue;
                    }
                    if self.slots[slot].state != SlotState::Empty {
                        self.remove(slot);
                    }
                    let s = &mut self.slots[slot];
                    s.link = Some(link);
                    s.state = SlotState::Waiting(rule);
                    s.fade_in = fade_in;
                    s.frames_read = 0;
                    s.starved = false;
                    s.remove_after_fade = false;
                    s.env = Env::fixed(1.0);
                }
                MixerCmd::StartNow { slot, fade_in } => {
                    if slot < MAX_DECKS && matches!(self.slots[slot].state, SlotState::Waiting(_)) {
                        self.slots[slot].fade_in = fade_in;
                        self.start(slot, None);
                    }
                }
                MixerCmd::Detach { slot, fade_out } => {
                    if slot >= MAX_DECKS {
                        continue;
                    }
                    if self.main == slot {
                        self.main = NO_SLOT;
                        self.main_ended = false;
                    }
                    match self.slots[slot].state {
                        SlotState::Empty => {}
                        SlotState::Waiting(_) => self.remove(slot),
                        SlotState::Playing => {
                            if fade_out == 0 || self.paused {
                                self.remove(slot);
                            } else {
                                let s = &mut self.slots[slot];
                                s.env.linear(0.0, fade_out);
                                s.remove_after_fade = true;
                            }
                        }
                    }
                }
                MixerCmd::Pause { fade } => {
                    self.paused = true;
                    self.pause_env.linear(0.0, fade);
                }
                MixerCmd::Resume { fade } => {
                    self.paused = false;
                    // Старт с тишины (ни одна дека ещё не звучит) — без подъёма: начало трека
                    // должно остаться точным. Подъём нужен только при снятии с паузы посреди трека.
                    if self.slots.iter().any(|s| s.state == SlotState::Playing) {
                        self.pause_env.linear(1.0, fade);
                    } else {
                        self.pause_env = Env::fixed(1.0);
                    }
                }
                MixerCmd::Volume { gain, ramp } => self.volume.linear(gain, ramp),
            }
        }
    }

    fn ready(&self, i: usize) -> bool {
        let s = &self.slots[i];
        s.avail() >= self.start_threshold || s.finished()
    }

    fn start(&mut self, i: usize, crossfade: Option<u32>) {
        let prev = self.main;
        let fade_in = self.slots[i].fade_in;
        {
            let s = &mut self.slots[i];
            s.state = SlotState::Playing;
            s.starved = false;
            match crossfade {
                Some(len) => s.env.equal_power(true, len),
                None => {
                    s.env = Env::fixed(if fade_in > 0 { 0.0 } else { 1.0 });
                    s.env.linear(1.0, fade_in);
                }
            }
        }
        if prev != NO_SLOT && prev != i && self.slots[prev].state == SlotState::Playing {
            let declick = self.declick;
            let p = &mut self.slots[prev];
            match crossfade {
                Some(len) => p.env.equal_power(false, len),
                None => p.env.linear(0.0, declick),
            }
            p.remove_after_fade = true;
        }
        self.main = i;
        self.main_ended = false;
        self.event(MixerEvent::Started {
            slot: i,
            crossfade: crossfade.is_some(),
            at: Instant::now(),
        });
    }

    fn check_starts(&mut self) {
        for i in 0..MAX_DECKS {
            let SlotState::Waiting(rule) = self.slots[i].state else {
                continue;
            };
            match rule {
                StartRule::Now => {
                    if self.ready(i) {
                        self.start(i, None);
                    }
                }
                StartRule::BeforeMainEnd { frames } => {
                    if self.main != NO_SLOT && self.main != i {
                        let rem = self.slots[self.main].remaining();
                        if rem != UNKNOWN && rem <= frames && self.ready(i) {
                            // Осталось меньше заданного (перемотка к концу) — кроссфейд короче.
                            let len = rem.max(1).min(u32::MAX as u64) as u32;
                            if rem == 0 {
                                self.start(i, None);
                            } else {
                                self.start(i, Some(len));
                            }
                        }
                    } else if self.main == NO_SLOT && self.main_ended && self.ready(i) {
                        self.start(i, None);
                    }
                }
            }
        }
    }

    /// Сколько кадров можно отрендерить, не проскочив момент старта ждущей деки.
    fn limit(&self, mut n: usize) -> usize {
        if self.main == NO_SLOT {
            return n;
        }
        let m = &self.slots[self.main];
        let rem = m.remaining();
        if rem == UNKNOWN {
            return n;
        }
        for s in &self.slots {
            if let SlotState::Waiting(StartRule::BeforeMainEnd { frames }) = s.state
                && rem > frames
            {
                n = n.min((rem - frames) as usize);
            }
        }
        // Конец главной деки — тоже граница: следующая может стартовать ровно за ним.
        if m.finished() && rem > 0 {
            n = n.min(rem as usize);
        }
        n.max(1)
    }

    fn render_slot(&mut self, i: usize, out: &mut [f32]) {
        let n = out.len() / 2;
        let resume_threshold = self.resume_threshold;
        let s = &mut self.slots[i];
        let Some(link) = s.link.as_mut() else { return };
        let avail = link.cons.slots() / 2;
        let finished = link.shared.finished.load(Ordering::Acquire);
        if s.starved {
            if avail >= resume_threshold || finished {
                s.starved = false;
                self.event(MixerEvent::Resumed { slot: i });
            } else {
                return;
            }
        }
        let s = &mut self.slots[i];
        let Some(link) = s.link.as_mut() else { return };
        let take = n.min(avail);
        if take > 0
            && let Ok(chunk) = link.cons.read_chunk(take * 2)
        {
            let (a, b) = chunk.as_slices();
            let mut f = 0usize;
            for part in [a, b] {
                for pair in part.as_chunks::<2>().0 {
                    let g = s.env.next();
                    out[f * 2] += pair[0] * g;
                    out[f * 2 + 1] += pair[1] * g;
                    f += 1;
                }
            }
            chunk.commit_all();
            s.frames_read += take as u64;
        }
        if s.remove_after_fade && !s.env.active && s.env.gain == 0.0 {
            self.remove(i);
            return;
        }
        if take < n {
            let s = &self.slots[i];
            let drained = s.link.as_ref().map(|l| l.cons.slots() == 0).unwrap_or(true);
            if finished && drained {
                let was_main = self.main == i;
                self.event(MixerEvent::Ended { slot: i });
                self.remove(i);
                if was_main {
                    self.main_ended = true;
                }
            } else {
                self.slots[i].starved = true;
                self.shared.underruns.fetch_add(1, Ordering::Relaxed);
                self.event(MixerEvent::Starved { slot: i });
            }
        }
    }

    /// Рендер блока: `out` — стерео с чередованием каналов.
    pub fn render(&mut self, out: &mut [f32]) {
        self.drain_commands();
        out.fill(0.0);
        let frames = out.len() / 2;
        let silent = self.paused && !self.pause_env.active && self.pause_env.gain == 0.0;
        if !silent {
            let mut pos = 0;
            while pos < frames {
                self.check_starts();
                let n = self.limit(frames - pos);
                for i in 0..MAX_DECKS {
                    if self.slots[i].state == SlotState::Playing {
                        self.render_slot(i, &mut out[pos * 2..(pos + n) * 2]);
                    }
                }
                pos += n;
            }
            let level = &self.shared.level;
            for pair in out.as_chunks_mut::<2>().0 {
                let p = self.pause_env.next();
                let (l, r) = (pair[0] * p, pair[1] * p);
                self.meter.push(l, r, level);
                let v = self.volume.next();
                pair[0] = (l * v).clamp(-1.0, 1.0);
                pair[1] = (r * v).clamp(-1.0, 1.0);
            }
        }
        for i in 0..MAX_DECKS {
            self.shared.frames_read[i].store(self.slots[i].frames_read, Ordering::Relaxed);
        }
        self.shared.main.store(self.main, Ordering::Relaxed);
        self.shared
            .clock
            .fetch_add(frames as u64, Ordering::Relaxed);
    }

    /// Тестовый режим: все подключённые деки имеют данные на `frames` кадров (или кончились).
    pub fn ready_for(&self, frames: usize) -> bool {
        !self.shared.hold.load(Ordering::Acquire)
            && self
                .slots
                .iter()
                .all(|s| s.state == SlotState::Empty || s.avail() >= frames || s.finished())
    }

    pub fn is_idle(&self) -> bool {
        self.slots.iter().all(|s| s.state == SlotState::Empty)
    }

    pub fn has_playing(&self) -> bool {
        !self.paused && self.slots.iter().any(|s| s.state == SlotState::Playing)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn equal_power_curve_keeps_power() {
        let (o0, i0) = equal_power(0.0);
        assert!((o0 - 1.0).abs() < 1e-6 && i0.abs() < 1e-6);
        let (o1, i1) = equal_power(1.0);
        assert!(o1.abs() < 1e-6 && (i1 - 1.0).abs() < 1e-6);
        let mut prev = (1.0f32, 0.0f32);
        for k in 0..=1000 {
            let (o, i) = equal_power(k as f32 / 1000.0);
            assert!((o * o + i * i - 1.0).abs() < 1e-5, "power at {k}");
            assert!(o <= prev.0 + 1e-6 && i >= prev.1 - 1e-6, "monotonic at {k}");
            prev = (o, i);
        }
        // В середине оба на −3 дБ.
        let (o, i) = equal_power(0.5);
        assert!((20.0 * o.log10() + 3.0103).abs() < 0.01 && (o - i).abs() < 1e-6);
    }

    #[test]
    fn envelope_reaches_target_exactly() {
        let mut e = Env::fixed(1.0);
        e.equal_power(false, 480);
        let mut last = 1.0;
        for _ in 0..480 {
            last = e.next();
        }
        assert_eq!(last, 0.0);
        assert!(!e.active);
    }
}
