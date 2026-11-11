//! Дека: поток декодирования, который пишет стерео f32 с частотой устройства в свой кольцевой
//! буфер. В режиме «без пауз» к деке пристёгивается следующий трек (сегмент): декодер
//! переключается на него в момент конца предыдущего, ресемплер продолжает работу — стык точен.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

use crossbeam_channel::{Receiver, Sender};

use crate::decode::TrackDecoder;
use crate::resample::StreamResampler;
use crate::{AudioError, TrackSpec};

pub(crate) const UNKNOWN: u64 = u64::MAX;

#[derive(Clone, Debug)]
pub(crate) struct Segment {
    /// Индекс трека в очереди (на момент подготовки).
    pub index: usize,
    /// Что играет в сегменте: по `id` контроллер сверяет его с очередью.
    pub track: TrackSpec,
    /// Номер кадра деки, с которого начинается сегмент.
    pub deck_start: u64,
    /// Позиция в треке в начале сегмента (после перемотки), мс.
    pub offset_ms: u64,
    /// Полная длина трека, мс (если контейнер её сообщает).
    pub duration_ms: Option<u64>,
    pub info: String,
}

/// Состояние деки, которое читают микшер (атомики) и контроллер.
pub(crate) struct DeckShared {
    /// Все кадры записаны в буфер, дальше данных не будет.
    pub finished: AtomicBool,
    /// Сколько кадров всего выдаст дека (UNKNOWN, пока неизвестно). Точное значение — к концу.
    pub total_frames: AtomicU64,
    pub produced: AtomicU64,
    /// Контроллер обещал пристегнуть следующий трек — в конце текущего не сбрасывать ресемплер.
    pub expect_chain: AtomicBool,
    pub stop: AtomicBool,
    pub segments: Mutex<Vec<Segment>>,
    pub error: Mutex<Option<String>>,
}

impl DeckShared {
    fn new() -> Self {
        Self {
            finished: AtomicBool::new(false),
            total_frames: AtomicU64::new(UNKNOWN),
            produced: AtomicU64::new(0),
            expect_chain: AtomicBool::new(false),
            stop: AtomicBool::new(false),
            segments: Mutex::new(Vec::new()),
            error: Mutex::new(None),
        }
    }

    pub fn segments(&self) -> Vec<Segment> {
        self.segments.lock().map(|s| s.clone()).unwrap_or_default()
    }
}

/// Трек, открытый и (при необходимости) перемотанный заранее, в отдельном потоке.
pub(crate) struct PreparedTrack {
    pub index: usize,
    /// Трек с уже известной ссылкой и громкостью.
    pub spec: TrackSpec,
    pub decoder: TrackDecoder,
    pub gain: f32,
    pub offset_ms: u64,
}

pub(crate) enum DeckCmd {
    Chain(Box<PreparedTrack>),
    Unchain,
}

/// То, что уходит в микшер: потребитель кольца и общее состояние.
pub(crate) struct DeckLink {
    pub cons: rtrb::Consumer<f32>,
    pub shared: Arc<DeckShared>,
}

pub(crate) struct DeckHandle {
    pub shared: Arc<DeckShared>,
    pub tx: Sender<DeckCmd>,
    thread: Option<JoinHandle<()>>,
}

impl DeckHandle {
    pub fn stop(&mut self) {
        self.shared.stop.store(true, Ordering::Relaxed);
        if let Some(t) = self.thread.take() {
            // Не ждём: поток выйдет сам на ближайшей проверке флага.
            drop(t);
        }
    }
}

impl Drop for DeckHandle {
    fn drop(&mut self) {
        self.shared.stop.store(true, Ordering::Relaxed);
    }
}

pub(crate) fn spawn(
    prepared: PreparedTrack,
    device_rate: u32,
    ring_frames: usize,
) -> Result<(DeckHandle, DeckLink), AudioError> {
    let (prod, cons) = rtrb::RingBuffer::new(ring_frames * 2);
    let shared = Arc::new(DeckShared::new());
    let (tx, rx) = crossbeam_channel::unbounded();
    let s = shared.clone();
    let thread = std::thread::Builder::new()
        .name("dw-deck".into())
        .spawn(move || {
            let mut w = Worker::new(prepared, device_rate, prod, s.clone(), rx, ring_frames);
            if let Err(e) = w.run() {
                log::warn!("deck error: {e}");
                if let Ok(mut g) = s.error.lock() {
                    *g = Some(e.to_string());
                }
            }
            s.total_frames
                .store(s.produced.load(Ordering::Relaxed), Ordering::Relaxed);
            s.finished.store(true, Ordering::Release);
        })
        .map_err(|e| AudioError::Decode(e.to_string()))?;
    Ok((
        DeckHandle {
            shared: shared.clone(),
            tx,
            thread: Some(thread),
        },
        DeckLink { cons, shared },
    ))
}

struct Worker {
    cur: PreparedTrack,
    next: Option<Box<PreparedTrack>>,
    device_rate: u32,
    prod: rtrb::Producer<f32>,
    shared: Arc<DeckShared>,
    rx: Receiver<DeckCmd>,
    ring_frames: usize,
    resampler: Option<StreamResampler>,
    /// Номер кадра деки, с которого пишет текущий экземпляр ресемплера.
    resampler_base: u64,
    /// Кадров отдано в pending (записано или ждёт места в кольце).
    emitted: u64,
    pending: Vec<f32>,
    pending_pos: usize,
    src: Vec<f32>,
}

fn write_ring(prod: &mut rtrb::Producer<f32>, data: &[f32]) -> usize {
    let n = prod.slots().min(data.len()) & !1;
    if n == 0 {
        return 0;
    }
    let Ok(mut chunk) = prod.write_chunk(n) else {
        return 0;
    };
    let (a, b) = chunk.as_mut_slices();
    let al = a.len();
    a.copy_from_slice(&data[..al]);
    b.copy_from_slice(&data[al..n]);
    chunk.commit_all();
    n
}

impl Worker {
    fn new(
        cur: PreparedTrack,
        device_rate: u32,
        prod: rtrb::Producer<f32>,
        shared: Arc<DeckShared>,
        rx: Receiver<DeckCmd>,
        ring_frames: usize,
    ) -> Self {
        Self {
            cur,
            next: None,
            device_rate,
            prod,
            shared,
            rx,
            ring_frames,
            resampler: None,
            resampler_base: 0,
            emitted: 0,
            pending: Vec::with_capacity(32 * 1024),
            pending_pos: 0,
            src: Vec::with_capacity(32 * 1024),
        }
    }

    fn make_resampler(&mut self, rate: u32) -> Result<(), AudioError> {
        self.resampler = if rate != self.device_rate {
            Some(StreamResampler::new(rate, self.device_rate)?)
        } else {
            None
        };
        self.resampler_base = self.emitted;
        Ok(())
    }

    fn device_frames(&self, source_frames: u64, rate: u32) -> u64 {
        (source_frames as u128 * self.device_rate as u128).div_ceil(rate as u128) as u64
    }

    fn begin_segment(&mut self, deck_start: u64) {
        let d = &self.cur.decoder;
        let seg = Segment {
            index: self.cur.index,
            track: self.cur.spec.clone(),
            deck_start,
            offset_ms: self.cur.offset_ms,
            duration_ms: d.total_frames.map(|t| t * 1000 / d.source_rate as u64),
            info: d.describe(),
        };
        if let Ok(mut s) = self.shared.segments.lock() {
            s.push(seg);
        }
        // Оценка полной длины деки — для момента старта кроссфейда.
        let total = d
            .total_frames
            .map(|t| t.saturating_sub(d.position))
            .map(|left| deck_start + self.device_frames(left, d.source_rate));
        self.shared
            .total_frames
            .store(total.unwrap_or(UNKNOWN), Ordering::Relaxed);
    }

    /// Ждём места в кольце: спим тем дольше, чем больше запас (бережём батарею).
    fn wait_for_space(&self) {
        let buffered = (self.ring_frames as u64).saturating_sub(self.prod.slots() as u64 / 2);
        let buffered_ms = buffered * 1000 / self.device_rate as u64;
        let ms = (buffered_ms.saturating_sub(500) / 2).clamp(5, 250);
        std::thread::sleep(Duration::from_millis(ms));
    }

    fn flush_pending(&mut self) -> bool {
        while self.pending_pos < self.pending.len() {
            if self.shared.stop.load(Ordering::Relaxed) {
                return false;
            }
            let n = write_ring(&mut self.prod, &self.pending[self.pending_pos..]);
            self.pending_pos += n;
            self.shared
                .produced
                .fetch_add((n / 2) as u64, Ordering::Relaxed);
            if self.pending_pos < self.pending.len() {
                self.drain_commands();
                self.wait_for_space();
            }
        }
        self.pending.clear();
        self.pending_pos = 0;
        true
    }

    fn drain_commands(&mut self) {
        while let Ok(cmd) = self.rx.try_recv() {
            match cmd {
                DeckCmd::Chain(p) => self.next = Some(p),
                DeckCmd::Unchain => self.next = None,
            }
        }
    }

    fn push_source(&mut self) -> Result<(), AudioError> {
        let gain = self.cur.gain;
        if gain != 1.0 {
            for s in &mut self.src {
                *s *= gain;
            }
        }
        let before = self.pending.len();
        match &mut self.resampler {
            Some(r) => r.process(&self.src, &mut self.pending)?,
            None => self.pending.extend_from_slice(&self.src),
        }
        self.emitted += ((self.pending.len() - before) / 2) as u64;
        self.src.clear();
        Ok(())
    }

    fn flush_resampler(&mut self) -> Result<(), AudioError> {
        if let Some(r) = &mut self.resampler {
            let before = self.pending.len();
            r.flush(&mut self.pending)?;
            self.emitted += ((self.pending.len() - before) / 2) as u64;
        }
        Ok(())
    }

    fn run(&mut self) -> Result<(), AudioError> {
        self.make_resampler(self.cur.decoder.source_rate)?;
        self.begin_segment(0);
        loop {
            if self.shared.stop.load(Ordering::Relaxed) {
                return Ok(());
            }
            self.drain_commands();
            if !self.flush_pending() {
                return Ok(());
            }
            let res = self.cur.decoder.read(&mut self.src);
            match res {
                Ok(Some(_)) => self.push_source()?,
                Ok(None) => {
                    if self.next.is_none() && self.shared.expect_chain.load(Ordering::Relaxed) {
                        // Следующий трек ещё готовится: ждём, не сбрасывая ресемплер.
                        std::thread::sleep(Duration::from_millis(2));
                        continue;
                    }
                    let Some(next) = self.next.take() else {
                        self.flush_resampler()?;
                        self.flush_pending();
                        return Ok(());
                    };
                    let same_rate = next.decoder.source_rate == self.cur.decoder.source_rate;
                    let boundary = match (&self.resampler, same_rate) {
                        // Тот же ресемплер: стык — в точной позиции непрерывного потока.
                        (Some(r), true) => self.resampler_base + r.scale(r.input_frames()),
                        (None, true) => self.emitted,
                        _ => {
                            self.flush_resampler()?;
                            self.emitted
                        }
                    };
                    self.cur = *next;
                    if !same_rate {
                        self.make_resampler(self.cur.decoder.source_rate)?;
                    }
                    self.begin_segment(boundary);
                }
                Err(e) => {
                    if self.shared.stop.load(Ordering::Relaxed) {
                        return Ok(());
                    }
                    // Обрыв посреди трека: доигрываем то, что есть, и сообщаем.
                    self.flush_resampler()?;
                    self.flush_pending();
                    return Err(e);
                }
            }
        }
    }
}
