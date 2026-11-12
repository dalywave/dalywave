//! Вывод: устройство через cpal, буферизованный вывод Apple (AirPlay 2, renderer.rs) или тестовый
//! захват в память.
//!
//! В колбэке cpal работает только микшер (без выделений и блокировок) и раскладка стерео на
//! каналы устройства. Загрузка колбэка (время обработки / длительность буфера) копится в атомиках.
//!
//! Ошибки потока cpal приходят контроллеру как [`OutputIssue`]: сменилось устройство (поток
//! переехал сам), устройство пропало, поток больше не годится и его надо открыть заново.

use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use crossbeam_channel::Sender;

use crate::AudioError;
use crate::mixer::Mixer;

/// Максимальный блок микшера за один проход; больший буфер устройства режется на части.
const MAX_BLOCK: usize = 4096;

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
    Device {
        stream: cpal::Stream,
        running: bool,
    },
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
    pub fn open_device(
        device: cpal::Device,
        supported: cpal::SupportedStreamConfig,
        buffer_frames: Option<u32>,
        mut mixer: Mixer,
        load: Arc<LoadStats>,
        errors: Sender<OutputIssue>,
    ) -> Result<(Output, DeviceInfo), AudioError> {
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
        let ch = channels as usize;
        let mut scratch = vec![0f32; MAX_BLOCK * 2];
        let stream = device
            .build_output_stream::<f32, _, _>(
                config,
                move |data: &mut [f32], _info: &cpal::OutputCallbackInfo| {
                    let t0 = Instant::now();
                    let frames = data.len() / ch;
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
                                for (o, p) in out.chunks_exact_mut(ch).zip(block.as_chunks::<2>().0)
                                {
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
                    let budget = frames as f32 / rate as f32;
                    if budget > 0.0 {
                        load.record(t0.elapsed().as_secs_f32() / budget, frames as u32);
                    }
                },
                move |err| {
                    let _ = errors.send(OutputIssue::from_cpal(&err));
                },
                None,
            )
            .map_err(|e| AudioError::Output(e.to_string()))?;
        // Поток создан остановленным: запустим по первой команде play.
        let _ = stream.pause();
        Ok((
            Output::Device {
                stream,
                running: false,
            },
            DeviceInfo {
                sample_rate: rate,
                channels,
            },
        ))
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
        if let Output::Device { stream, running } = self {
            // Всегда зовём play: на iOS систему может остановить поток сама (прерывание звонком),
            // и наш флаг об этом не знает; cpal ведёт своё состояние и повторный play безвреден.
            match stream.play() {
                Ok(()) => *running = true,
                Err(e) => log::warn!("stream play: {e}"),
            }
        }
    }

    pub fn stop(&mut self) {
        #[cfg(target_vendor = "apple")]
        if let Output::Buffered(r) = self {
            r.set_playing(false);
            return;
        }
        if let Output::Device { stream, running } = self
            && *running
        {
            let _ = stream.pause();
            *running = false;
        }
    }

    /// Остановить совсем, перед тем как открыть вывод заново: после возврата микшер больше
    /// не вызывается, и счётчики позиции не растут.
    pub fn halt(&mut self) {
        match self {
            Output::Device { stream, running } => {
                let _ = stream.pause();
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
