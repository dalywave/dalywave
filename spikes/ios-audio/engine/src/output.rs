//! Вывод: устройство через cpal или тестовый захват в память.
//!
//! В колбэке cpal работает только микшер (без выделений и блокировок) и раскладка стерео на
//! каналы устройства. Загрузка колбэка (время обработки / длительность буфера) копится в атомиках.

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
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

pub(crate) enum Output {
    Device {
        stream: cpal::Stream,
        running: bool,
    },
    Capture {
        stop: Arc<AtomicBool>,
        thread: Option<JoinHandle<()>>,
    },
}

pub(crate) fn query_device() -> Result<(cpal::Device, cpal::SupportedStreamConfig), AudioError> {
    let host = cpal::default_host();
    let device = host
        .default_output_device()
        .ok_or_else(|| AudioError::Output("нет устройства вывода".into()))?;
    let config = device
        .default_output_config()
        .map_err(|e| AudioError::Output(e.to_string()))?;
    Ok((device, config))
}

impl Output {
    pub fn open_device(
        device: cpal::Device,
        supported: cpal::SupportedStreamConfig,
        buffer_frames: Option<u32>,
        mut mixer: Mixer,
        load: Arc<LoadStats>,
        errors: Sender<String>,
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
                    let _ = errors.send(err.to_string());
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

    pub fn open_capture(mut mixer: Mixer, capture: Arc<Mutex<Vec<f32>>>) -> Output {
        let stop = Arc::new(AtomicBool::new(false));
        let s = stop.clone();
        let thread = std::thread::Builder::new()
            .name("dw-capture".into())
            .spawn(move || {
                const BLOCK: usize = 1024;
                let mut block = vec![0f32; BLOCK * 2];
                while !s.load(Ordering::Relaxed) {
                    // В тестовом режиме недогрузок не бывает: ждём, пока у дек будут данные.
                    if !mixer.ready_for(BLOCK) {
                        std::thread::sleep(Duration::from_micros(500));
                        continue;
                    }
                    let was_active = !mixer.is_idle() && mixer.has_playing();
                    mixer.render(&mut block);
                    let now_active = !mixer.is_idle() && mixer.has_playing();
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
        Output::Capture { stop, thread }
    }

    pub fn start(&mut self) {
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
        if let Output::Device { stream, running } = self
            && *running
        {
            let _ = stream.pause();
            *running = false;
        }
    }

    pub fn is_running(&self) -> bool {
        match self {
            Output::Device { running, .. } => *running,
            Output::Capture { .. } => true,
        }
    }
}

impl Drop for Output {
    fn drop(&mut self) {
        if let Output::Capture { stop, thread } = self {
            stop.store(true, Ordering::Relaxed);
            if let Some(t) = thread.take() {
                let _ = t.join();
            }
        }
    }
}
