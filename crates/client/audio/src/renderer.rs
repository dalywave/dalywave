//! Буферизованный вывод Apple (iOS, macOS): AVSampleBufferAudioRenderer +
//! AVSampleBufferRenderSynchronizer — путь, который Apple советует своим движкам для AirPlay 2:
//! большой буфер на стороне колонки, несколько колонок сразу и верное время того, что слышно.
//!
//! Микшер тот же, что у cpal: без пауз, кроссфейд и «Диджей» сводятся до рендерера, сюда идёт уже
//! готовый стерео PCM (float32). Поток вывода сам берёт блоки у микшера, пока рендерер просит,
//! кладёт их в CMSampleBuffer со временем «кадр микшера / частота» и ведёт время синхронизатора:
//! слышимый кадр — это его время, задержку AirPlay (секунды) система учитывает сама.
//!
//! Чего нет у cpal: звук уходит наперёд, поэтому
//! - пауза — скорость синхронизатора 0 (мгновенно, поставленное остаётся в очереди и доиграет);
//! - перемотка и другой трек по команде — сброс очереди (`flush`), время — на новый кадр;
//! - громкость — у рендерера (сразу), а не в микшере (было бы слышно через секунды);
//! - тишину «пока дека готовится» не кладём: рендер ждёт данных ([`Mixer::can_feed`]), время
//!   синхронизатора стоит, пока не набралось начало ([`Feed`]).

use std::ffi::c_void;
use std::ptr::{NonNull, null, null_mut};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use block2::RcBlock;
use crossbeam_channel::Sender;
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, NSObjectProtocol, ProtocolObject};
use objc2_av_foundation::{
    AVQueuedSampleBufferRendering, AVQueuedSampleBufferRenderingStatus,
    AVSampleBufferAudioRenderer, AVSampleBufferRenderSynchronizer,
};
use objc2_core_audio_types::{
    AudioStreamBasicDescription, kAudioFormatFlagIsFloat, kAudioFormatFlagIsPacked,
    kAudioFormatLinearPCM,
};
use objc2_core_foundation::CFRetained;
use objc2_core_media::{
    CMAudioFormatDescriptionCreate, CMAudioSampleBufferCreateReadyWithPacketDescriptions,
    CMBlockBuffer, CMFormatDescription, CMItemCount, CMSampleBuffer, CMTime, CMTimeFlags,
    kCMBlockBufferAssureMemoryNowFlag,
};
use objc2_foundation::{NSNotification, NSNotificationCenter};

use crate::AudioError;
use crate::mixer::Mixer;
use crate::output::{LoadStats, OutputIssue};

/// Блок микшера на один CMSampleBuffer (~46 мс при 44,1 кГц).
const BLOCK: usize = 2048;
/// Сколько набрать после старта, перемотки или недогрузки, прежде чем пустить время.
const PRIME_MS: u64 = 300;
/// Микшер не может дать больше (конец очереди) столько времени — пустить время с тем, что есть.
const PRIME_GRACE: Duration = Duration::from_millis(150);
/// Время синхронизатора не идёт столько, хотя звук поставлен, — вывод считается неработающим
/// (AirPlay набирает буфер секунды две — с запасом).
const STALL: Duration = Duration::from_secs(8);
/// Страховка: дальше этого наперёд не рендерим, даже если рендерер просит ещё.
const MAX_AHEAD_MS: u64 = 30_000;
/// Частота вывода: родная для AirPlay; другую рендерер пересчитает сам.
pub(crate) const RATE: u32 = 44_100;

/// Что сделать со временем синхронизатора.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Clock {
    /// Скорость 0, время на месте (пауза: поставленное доиграет после).
    Stop,
    /// Скорость 0 и время — на этот кадр (сброс очереди, недогрузка).
    StopAt(u64),
    /// Скорость 1 с этого кадра (первый поставленный).
    StartAt(u64),
    /// Скорость 1 с текущего времени (снятие с паузы).
    Resume,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Run {
    /// Набираем начало: время стоит на `start_at` (и до первой команды «играть»).
    Priming,
    Running,
    /// Пауза после звука: время стоит, поставленное ждёт.
    Paused,
}

/// Время буферизованного вывода без AVFoundation: сколько поставлено, что слышно, когда пускать и
/// останавливать время синхронизатора. Кадры — кадры микшера (его часы и отметки времени буферов).
#[derive(Debug)]
pub(crate) struct Feed {
    prime: u64,
    enqueued: u64,
    heard: u64,
    start_at: u64,
    run: Run,
    /// С какого момента микшер не может дать ещё, пока набираем начало
    short_since: Option<Instant>,
}

impl Feed {
    pub fn new(rate: u32) -> Self {
        Self {
            prime: u64::from(rate) * PRIME_MS / 1000,
            enqueued: 0,
            heard: 0,
            start_at: 0,
            run: Run::Priming,
            short_since: None,
        }
    }

    pub fn enqueued(&self) -> u64 {
        self.enqueued
    }

    /// Кадр, который слышен сейчас.
    pub fn heard(&self) -> u64 {
        self.heard
    }

    /// Поставлено и ещё не прозвучало.
    pub fn ahead(&self) -> u64 {
        self.enqueued - self.heard
    }

    pub fn pushed(&mut self, frames: u64) {
        self.enqueued += frames;
    }

    /// Очередь рендерера сброшена: поставленное не прозвучит, время — на конец поставленного.
    pub fn flushed(&mut self) -> Clock {
        self.heard = self.enqueued;
        self.start_at = self.enqueued;
        self.short_since = None;
        self.run = Run::Priming;
        Clock::StopAt(self.enqueued)
    }

    /// Шаг потока вывода. `want` — должно звучать; `timebase` — время синхронизатора в кадрах
    /// (None — недействительно); `more` — рендер может дать ещё блок прямо сейчас.
    pub fn step(
        &mut self,
        want: bool,
        timebase: Option<u64>,
        more: bool,
        now: Instant,
    ) -> Option<Clock> {
        // Время синхронизатора меняется асинхронно: старое значение после сброса назад не тянет
        if self.run != Run::Priming
            && let Some(t) = timebase
        {
            self.heard = self.heard.max(t.min(self.enqueued));
        }
        match (self.run, want) {
            (Run::Priming, false) | (Run::Paused, false) => None,
            (Run::Running, false) => {
                self.run = Run::Paused;
                Some(Clock::Stop)
            }
            (Run::Paused, true) if self.enqueued > self.heard => {
                // Пауза с очередью: продолжить с того же места
                self.run = Run::Running;
                Some(Clock::Resume)
            }
            (Run::Paused, true) => {
                self.run = Run::Priming;
                self.start_at = self.enqueued;
                self.short_since = None;
                self.prime_done(more, now)
            }
            (Run::Priming, true) => self.prime_done(more, now),
            (Run::Running, true) if self.heard >= self.enqueued => {
                // Всё поставленное прозвучало, а нового нет: недогрузка — время ждёт данных
                self.run = Run::Priming;
                self.start_at = self.enqueued;
                self.short_since = None;
                Some(Clock::StopAt(self.enqueued))
            }
            (Run::Running, true) => None,
        }
    }

    fn prime_done(&mut self, more: bool, now: Instant) -> Option<Clock> {
        let have = self.enqueued - self.start_at;
        let short = if more || have == 0 {
            self.short_since = None;
            false
        } else {
            now.duration_since(*self.short_since.get_or_insert(now)) >= PRIME_GRACE
        };
        if have >= self.prime || short {
            self.run = Run::Running;
            self.heard = self.start_at;
            Some(Clock::StartAt(self.start_at))
        } else {
            None
        }
    }
}

/// Общее с контроллером: команды и что слышно.
pub(crate) struct RendererShared {
    stop: AtomicBool,
    play: AtomicBool,
    flush: AtomicBool,
    auto_flushed: AtomicBool,
    volume: AtomicU32,
    heard: AtomicU64,
}

pub(crate) struct Renderer {
    shared: Arc<RendererShared>,
    thread: Option<JoinHandle<()>>,
}

impl Renderer {
    /// Открыть вывод. `device_uid` — macOS: CoreAudio UID устройства (колонка AirPlay в списке
    /// устройств); None — системный вывод (iOS — маршрут аудиосессии).
    pub fn open(
        mixer: Mixer,
        rate: u32,
        volume: f32,
        device_uid: Option<String>,
        load: Arc<LoadStats>,
        errors: Sender<OutputIssue>,
    ) -> Result<Renderer, AudioError> {
        let shared = Arc::new(RendererShared {
            stop: AtomicBool::new(false),
            play: AtomicBool::new(false),
            flush: AtomicBool::new(false),
            auto_flushed: AtomicBool::new(false),
            volume: AtomicU32::new(volume.to_bits()),
            heard: AtomicU64::new(0),
        });
        let (ready_tx, ready_rx) = crossbeam_channel::bounded(1);
        let s = shared.clone();
        let thread = std::thread::Builder::new()
            .name("dw-renderer".into())
            .spawn(move || run(mixer, rate, s, device_uid, load, errors, ready_tx))
            .map_err(|e| AudioError::Output(e.to_string()))?;
        match ready_rx.recv() {
            Ok(Ok(())) => Ok(Renderer {
                shared,
                thread: Some(thread),
            }),
            Ok(Err(e)) => {
                let _ = thread.join();
                Err(e)
            }
            Err(_) => Err(AudioError::Output("renderer thread died".into())),
        }
    }

    pub fn set_playing(&self, on: bool) {
        self.shared.play.store(on, Ordering::Release);
    }

    pub fn is_playing(&self) -> bool {
        self.shared.play.load(Ordering::Acquire)
    }

    /// Сбросить поставленное (перемотка, другой трек по команде).
    pub fn flush(&self) {
        self.shared.flush.store(true, Ordering::Release);
    }

    pub fn set_volume(&self, v: f32) {
        self.shared.volume.store(v.to_bits(), Ordering::Relaxed);
    }

    /// Кадр микшера, который слышен сейчас.
    pub fn heard(&self) -> u64 {
        self.shared.heard.load(Ordering::Acquire)
    }

    pub fn halt(&mut self) {
        self.shared.stop.store(true, Ordering::Release);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

impl Drop for Renderer {
    fn drop(&mut self) {
        self.halt();
    }
}

fn cm_time(frames: u64, rate: u32) -> CMTime {
    // SAFETY: CMTimeMake — чистая функция
    unsafe { CMTime::new(frames as i64, rate as i32) }
}

fn frames_of(t: CMTime, rate: u32) -> Option<u64> {
    if !t.flags.contains(CMTimeFlags::Valid) || t.timescale <= 0 {
        return None;
    }
    let f = i128::from(t.value) * i128::from(rate) / i128::from(t.timescale);
    Some(f.max(0) as u64)
}

/// Формат буферов: float32, стерео с чередованием каналов.
fn pcm_format(rate: u32) -> Result<CFRetained<CMFormatDescription>, AudioError> {
    let asbd = AudioStreamBasicDescription {
        mSampleRate: f64::from(rate),
        mFormatID: kAudioFormatLinearPCM,
        mFormatFlags: kAudioFormatFlagIsFloat | kAudioFormatFlagIsPacked,
        mBytesPerPacket: 8,
        mFramesPerPacket: 1,
        mBytesPerFrame: 8,
        mChannelsPerFrame: 2,
        mBitsPerChannel: 32,
        mReserved: 0,
    };
    let mut out: *const CMFormatDescription = null();
    // SAFETY: ASBD и выходной указатель живут на время вызова; раскладки и cookie нет
    let status = unsafe {
        CMAudioFormatDescriptionCreate(
            None,
            NonNull::from(&asbd),
            0,
            null(),
            0,
            null(),
            None,
            NonNull::from(&mut out),
        )
    };
    let out = NonNull::new(out.cast_mut()).filter(|_| status == 0);
    // SAFETY: Create-функция отдаёт объект с +1
    out.map(|p| unsafe { CFRetained::from_raw(p) })
        .ok_or_else(|| AudioError::Output(format!("CMAudioFormatDescriptionCreate: {status}")))
}

/// Блок PCM → CMSampleBuffer с отметкой времени `pts` (кадры).
fn sample_buffer(
    format: &CMFormatDescription,
    pcm: &[f32],
    pts: u64,
    rate: u32,
) -> Option<CFRetained<CMSampleBuffer>> {
    let bytes = std::mem::size_of_val(pcm);
    let mut block: *mut CMBlockBuffer = null_mut();
    // SAFETY: CoreMedia выделяет память сама (memory_block = NULL), выходной указатель — наш
    let status = unsafe {
        CMBlockBuffer::create_with_memory_block(
            None,
            null_mut(),
            bytes,
            None,
            null(),
            0,
            bytes,
            kCMBlockBufferAssureMemoryNowFlag,
            NonNull::from(&mut block),
        )
    };
    if status != 0 {
        return None;
    }
    // SAFETY: Create-функция отдаёт объект с +1
    let block = unsafe { CFRetained::from_raw(NonNull::new(block)?) };
    // SAFETY: копируем `bytes` байт из живого среза в память блока того же размера
    let status = unsafe {
        CMBlockBuffer::replace_data_bytes(
            NonNull::new(pcm.as_ptr().cast_mut().cast::<c_void>())?,
            &block,
            0,
            bytes,
        )
    };
    if status != 0 {
        return None;
    }
    let mut sample: *mut CMSampleBuffer = null_mut();
    // SAFETY: PCM без описаний пакетов; объекты живы на время вызова
    let status = unsafe {
        CMAudioSampleBufferCreateReadyWithPacketDescriptions(
            None,
            &block,
            format,
            (pcm.len() / 2) as CMItemCount,
            cm_time(pts, rate),
            null(),
            NonNull::from(&mut sample),
        )
    };
    if status != 0 {
        return None;
    }
    // SAFETY: Create-функция отдаёт объект с +1
    Some(unsafe { CFRetained::from_raw(NonNull::new(sample)?) })
}

/// Подписка на «рендерер сбросил очередь сам» (выбрали другую колонку, сменился формат выхода).
struct Observers(Vec<Retained<ProtocolObject<dyn NSObjectProtocol>>>);

impl Drop for Observers {
    fn drop(&mut self) {
        let nc = NSNotificationCenter::defaultCenter();
        for o in &self.0 {
            // SAFETY: наблюдатели этого центра уведомлений
            unsafe { nc.removeObserver(&*(Retained::as_ptr(o) as *const AnyObject)) };
        }
    }
}

fn observe(renderer: &AVSampleBufferAudioRenderer, shared: &Arc<RendererShared>) -> Observers {
    use objc2_av_foundation::{
        AVSampleBufferAudioRendererOutputConfigurationDidChangeNotification,
        AVSampleBufferAudioRendererWasFlushedAutomaticallyNotification,
    };
    let nc = NSNotificationCenter::defaultCenter();
    let mut out = Vec::new();
    // SAFETY: имена уведомлений — константы AVFoundation; блок только ставит флаг
    unsafe {
        for name in [
            AVSampleBufferAudioRendererWasFlushedAutomaticallyNotification,
            AVSampleBufferAudioRendererOutputConfigurationDidChangeNotification,
        ] {
            let s = shared.clone();
            let block = RcBlock::new(move |_n: NonNull<NSNotification>| {
                s.auto_flushed.store(true, Ordering::Release);
            });
            let object = &*(renderer as *const AVSampleBufferAudioRenderer as *const AnyObject);
            out.push(nc.addObserverForName_object_queue_usingBlock(
                Some(name),
                Some(object),
                None,
                &block,
            ));
        }
    }
    Observers(out)
}

fn run(
    mut mixer: Mixer,
    rate: u32,
    shared: Arc<RendererShared>,
    device_uid: Option<String>,
    load: Arc<LoadStats>,
    errors: Sender<OutputIssue>,
    ready: Sender<Result<(), AudioError>>,
) {
    let format = match pcm_format(rate) {
        Ok(f) => f,
        Err(e) => {
            let _ = ready.send(Err(e));
            return;
        }
    };
    // SAFETY: объекты AVFoundation создаются и живут только в этом потоке
    let (renderer, sync) = unsafe {
        let renderer = AVSampleBufferAudioRenderer::new();
        #[cfg(target_os = "macos")]
        if let Some(uid) = &device_uid {
            renderer.setAudioOutputDeviceUniqueID(Some(&objc2_foundation::NSString::from_str(uid)));
        }
        let sync = AVSampleBufferRenderSynchronizer::new();
        sync.addRenderer(ProtocolObject::from_ref(&*renderer));
        (renderer, sync)
    };
    #[cfg(not(target_os = "macos"))]
    let _ = device_uid;
    let _observers = observe(&renderer, &shared);
    load.buffer_frames.store(BLOCK as u32, Ordering::Relaxed);
    let _ = ready.send(Ok(()));

    let mut feed = Feed::new(rate);
    let mut block = vec![0f32; BLOCK * 2];
    let mut volume = f32::NAN;
    let mut failed = false;
    // Сторож: должно звучать, звук поставлен, а время синхронизатора стоит (завис системный звук)
    let mut still: Option<(u64, Instant)> = None;
    let max_ahead = u64::from(rate) * MAX_AHEAD_MS / 1000;
    let apply = |c: Clock| unsafe {
        // SAFETY: синхронизатор этого потока
        match c {
            Clock::Stop => sync.setRate(0.0),
            Clock::StopAt(f) => sync.setRate_time(0.0, cm_time(f, rate)),
            Clock::StartAt(f) => sync.setRate_time(1.0, cm_time(f, rate)),
            Clock::Resume => sync.setRate(1.0),
        }
    };
    while !shared.stop.load(Ordering::Acquire) {
        let v = f32::from_bits(shared.volume.load(Ordering::Relaxed));
        if v != volume {
            volume = v;
            unsafe { renderer.setVolume(v) };
        }
        if shared.flush.swap(false, Ordering::AcqRel) {
            unsafe { renderer.flush() };
            apply(feed.flushed());
        }
        if shared.auto_flushed.swap(false, Ordering::AcqRel) {
            // Поставленное пропало (или звучит не в том формате): контроллер откроет вывод заново
            // со слышимого места — звук не теряется, будто короткая пауза
            unsafe { renderer.flush() };
            apply(feed.flushed());
            let _ = errors.send(OutputIssue::Invalidated(
                "renderer flushed by the system (route or output format changed)".into(),
            ));
        }
        if !failed && unsafe { renderer.status() } == AVQueuedSampleBufferRenderingStatus::Failed {
            failed = true;
            let why = unsafe { renderer.error() }
                .map(|e| e.localizedDescription().to_string())
                .unwrap_or_else(|| "unknown error".into());
            let _ = errors.send(OutputIssue::BufferedFailed(why));
        }
        let want = shared.play.load(Ordering::Acquire) && !failed;
        let room =
            want && unsafe { renderer.isReadyForMoreMediaData() } && feed.ahead() < max_ahead;
        let more = room && mixer.can_feed(BLOCK);
        let mut fed = false;
        if more {
            mixer.render(&mut block);
            if let Some(sb) = sample_buffer(&format, &block, feed.enqueued(), rate) {
                unsafe { renderer.enqueueSampleBuffer(&sb) };
            }
            // Кадры считаются и при сбое буфера: часы микшера и время синхронизатора — одни
            feed.pushed(BLOCK as u64);
            fed = true;
        } else {
            mixer.poll();
        }
        let timebase = frames_of(unsafe { sync.currentTime() }, rate);
        if let Some(c) = feed.step(want, timebase, more, Instant::now()) {
            apply(c);
        }
        shared.heard.store(feed.heard(), Ordering::Release);
        if want && feed.ahead() > 0 {
            match still {
                Some((h, since)) if h == feed.heard() => {
                    if !failed && since.elapsed() > STALL {
                        failed = true;
                        let _ = errors.send(OutputIssue::BufferedFailed(
                            "the renderer clock does not move".into(),
                        ));
                    }
                }
                _ => still = Some((feed.heard(), Instant::now())),
            }
        } else {
            still = None;
        }
        if !fed {
            std::thread::sleep(Duration::from_millis(5));
        }
    }
    unsafe {
        sync.setRate(0.0);
        renderer.flush();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rate() -> u32 {
        44_100
    }

    #[test]
    fn priming_starts_the_clock_at_the_first_frame() {
        let t0 = Instant::now();
        let mut f = Feed::new(rate());
        assert_eq!(f.step(true, Some(0), true, t0), None);
        f.pushed(2048);
        assert_eq!(f.step(true, Some(0), true, t0), None);
        while f.enqueued() < 13_230 {
            f.pushed(2048);
        }
        assert_eq!(f.step(true, Some(0), true, t0), Some(Clock::StartAt(0)));
        // Время идёт — слышимое за ним, но не дальше поставленного
        assert_eq!(f.step(true, Some(4410), true, t0), None);
        assert_eq!(f.heard(), 4410);
        assert_eq!(f.ahead(), f.enqueued() - 4410);
    }

    #[test]
    fn pause_keeps_the_queue_and_resume_continues() {
        let t0 = Instant::now();
        let mut f = Feed::new(rate());
        f.pushed(20_000);
        assert_eq!(f.step(true, Some(0), true, t0), Some(Clock::StartAt(0)));
        assert_eq!(f.step(false, Some(5000), false, t0), Some(Clock::Stop));
        assert_eq!(f.heard(), 5000);
        assert_eq!(f.step(false, Some(5000), false, t0), None);
        // Поставленное осталось: снятие с паузы — тем же временем
        assert_eq!(f.step(true, Some(5000), false, t0), Some(Clock::Resume));
    }

    #[test]
    fn flush_moves_the_clock_to_the_new_audio() {
        let t0 = Instant::now();
        let mut f = Feed::new(rate());
        f.pushed(40_000);
        f.step(true, Some(0), true, t0);
        f.step(true, Some(10_000), true, t0);
        assert_eq!(f.flushed(), Clock::StopAt(40_000));
        assert_eq!(f.heard(), 40_000);
        // Старое время синхронизатора (обновляется асинхронно) слышимое назад не тянет
        assert_eq!(f.step(true, Some(10_500), true, t0), None);
        assert_eq!(f.heard(), 40_000);
        f.pushed(14_000);
        assert_eq!(
            f.step(true, Some(10_500), true, t0),
            Some(Clock::StartAt(40_000))
        );
    }

    #[test]
    fn underrun_stops_the_clock_until_data_returns() {
        let t0 = Instant::now();
        let mut f = Feed::new(rate());
        f.pushed(14_000);
        assert_eq!(f.step(true, Some(0), true, t0), Some(Clock::StartAt(0)));
        assert_eq!(
            f.step(true, Some(14_500), false, t0),
            Some(Clock::StopAt(14_000))
        );
        assert_eq!(f.heard(), 14_000);
        // Микшер ещё пуст: ждём
        assert_eq!(f.step(true, Some(14_000), false, t0), None);
        f.pushed(2048);
        // Данных мало, но больше не будет (конец очереди): через PRIME_GRACE — пускаем
        assert_eq!(f.step(true, Some(14_000), false, t0), None);
        let later = t0 + PRIME_GRACE + Duration::from_millis(1);
        assert_eq!(
            f.step(true, Some(14_000), false, later),
            Some(Clock::StartAt(14_000))
        );
    }
}
