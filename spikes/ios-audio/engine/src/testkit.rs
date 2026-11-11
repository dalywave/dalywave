//! Инструменты проверки: декодирование целиком, рендер очереди в память (тестовый режим
//! вывода), анализ стыков. Используются тестами движка и самопроверкой в приложении
//! (тот же код гоняется на iOS).

use std::time::{Duration, Instant};

use crate::decode::TrackDecoder;
use crate::resample::StreamResampler;
use crate::source::SourceCache;
use crate::{
    AudioError, Engine, EngineConfig, Event, OutputConfig, SourceSpec, TrackSpec, Transition,
};

pub struct Decoded {
    pub rate: u32,
    pub codec: String,
    pub container: String,
    pub total_frames_hint: Option<u64>,
    /// Стерео с чередованием каналов.
    pub samples: Vec<f32>,
}

/// Декодирует источник целиком тем же путём, что и дека (с точной обрезкой).
pub fn decode_all(spec: &SourceSpec) -> Result<Decoded, AudioError> {
    let cache = SourceCache::new("dw-testkit".into());
    let (source, ext) = cache.open(spec)?;
    let mut d = TrackDecoder::open(source, ext.as_deref())?;
    let mut samples = Vec::new();
    while d.read(&mut samples)?.is_some() {}
    Ok(Decoded {
        rate: d.source_rate,
        codec: d.codec.clone(),
        container: d.container.to_string(),
        total_frames_hint: d.total_frames,
        samples,
    })
}

/// Декодирует с перемоткой к кадру `frame` (проверка точности seek).
pub fn decode_from(spec: &SourceSpec, frame: u64) -> Result<Vec<f32>, AudioError> {
    let cache = SourceCache::new("dw-testkit".into());
    let (source, ext) = cache.open(spec)?;
    let mut d = TrackDecoder::open(source, ext.as_deref())?;
    d.seek_frame(frame)?;
    let mut samples = Vec::new();
    while d.read(&mut samples)?.is_some() {}
    Ok(samples)
}

/// Непрерывный ресемплинг целого сигнала (эталон для проверки стыков после ресемплинга).
pub fn resample_all(input: &[f32], from: u32, to: u32) -> Result<Vec<f32>, AudioError> {
    let mut r = StreamResampler::new(from, to)?;
    let mut out = Vec::new();
    for chunk in input.chunks(4096) {
        r.process(chunk, &mut out)?;
    }
    r.flush(&mut out)?;
    Ok(out)
}

/// Ресемплинг по кускам со сбросом в ноль на каждой границе — как если бы каждый трек
/// ресемплировался отдельно. Нужен, чтобы показать, какой щелчок убирает непрерывная цепочка.
pub fn resample_per_track(
    input: &[f32],
    bounds: &[usize],
    from: u32,
    to: u32,
) -> Result<Vec<f32>, AudioError> {
    let mut out = Vec::new();
    for w in bounds.windows(2) {
        let part = &input[w[0] * 2..w[1] * 2];
        out.extend(resample_all(part, from, to)?);
    }
    Ok(out)
}

pub struct CaptureResult {
    pub samples: Vec<f32>,
    pub events: Vec<Event>,
    pub elapsed: Duration,
}

/// Проигрывает очередь в тестовом режиме вывода и возвращает выход движка.
pub fn render_queue(
    tracks: Vec<TrackSpec>,
    sample_rate: u32,
    transition: Transition,
    timeout: Duration,
) -> Result<CaptureResult, AudioError> {
    let cfg = EngineConfig {
        output: OutputConfig::Capture { sample_rate },
        transition,
        normalize: false,
        // Короткие тестовые треки: готовим следующий сразу.
        preload_secs: 3600,
        ..EngineConfig::default()
    };
    let (engine, events) = Engine::new(cfg)?;
    let t0 = Instant::now();
    engine.load_queue(tracks, 0, true);
    let mut log = Vec::new();
    loop {
        let left = timeout
            .checked_sub(t0.elapsed())
            .ok_or_else(|| AudioError::Output("timeout".into()))?;
        match events.recv_timeout(left) {
            Ok(e) => {
                let end = matches!(e, Event::QueueEnded);
                log.push(e);
                if end {
                    break;
                }
            }
            Err(_) => return Err(AudioError::Output("timeout waiting for queue end".into())),
        }
    }
    // Даём захвату дописать последний блок.
    std::thread::sleep(Duration::from_millis(50));
    let samples = engine.take_capture();
    Ok(CaptureResult {
        samples,
        events: log,
        elapsed: t0.elapsed(),
    })
}

#[derive(Debug, Clone)]
pub struct JoinReport {
    /// Номер кадра стыка в эталоне.
    pub at: usize,
    /// Сдвиг выхода после стыка относительно до стыка, в сэмплах (0 — точно; >0 — вставлен
    /// зазор; <0 — потеряны сэмплы).
    pub shift: f64,
    /// RMS ошибки (выход − эталон) в окне ±128 кадров у стыка, dBFS.
    pub err_near_db: f64,
    /// Максимум |ошибки| у стыка, dBFS.
    pub peak_near_db: f64,
}

#[derive(Debug, Clone)]
pub struct GaplessReport {
    pub expected_frames: usize,
    pub got_frames: usize,
    /// Сдвиг всего выхода относительно эталона, кадров (должен быть 0).
    pub offset: f64,
    pub joins: Vec<JoinReport>,
    /// RMS ошибки по всему сигналу вне стыков, dBFS (шум кодека).
    pub err_body_db: f64,
    /// Самая длинная серия цифровой тишины внутри сигнала, кадров.
    pub max_zero_run: usize,
    pub channels_ok: bool,
}

impl GaplessReport {
    pub fn worst_shift(&self) -> f64 {
        self.joins.iter().map(|j| j.shift.abs()).fold(0.0, f64::max)
    }

    /// На сколько дБ ошибка у стыка выше, чем в теле сигнала (щелчок — большое число).
    pub fn worst_click_db(&self) -> f64 {
        self.joins
            .iter()
            .map(|j| j.err_near_db - self.err_body_db)
            .fold(f64::MIN, f64::max)
    }

    pub fn summary(&self) -> String {
        let shifts: Vec<String> = self
            .joins
            .iter()
            .map(|j| format!("{:+.2}", j.shift))
            .collect();
        let clicks: Vec<String> = self
            .joins
            .iter()
            .map(|j| format!("{:.1}", j.err_near_db))
            .collect();
        format!(
            "frames {}/{} offset {:+.2} shifts [{}] err@join dB [{}] body {:.1} dB zero-run {} channels {}",
            self.got_frames,
            self.expected_frames,
            self.offset,
            shifts.join(", "),
            clicks.join(", "),
            self.err_body_db,
            self.max_zero_run,
            if self.channels_ok { "ok" } else { "SWAPPED" }
        )
    }
}

fn left(x: &[f32]) -> Vec<f64> {
    x.as_chunks::<2>().0.iter().map(|p| p[0] as f64).collect()
}

// Анализ ходит по двум буферам со сдвигом индекса — с явными индексами понятнее, чем с итераторами.
#[allow(clippy::needless_range_loop)]
/// Сдвиг выхода относительно эталона на окне [a, b): минимум энергии разности по целым сдвигам
/// и уточнение параболой. В отличие от пика корреляции не путается на низких частотах.
fn best_shift(out: &[f64], reference: &[f64], a: usize, b: usize, center: i64, radius: i64) -> f64 {
    let resid = |d: i64| -> f64 {
        let mut s = 0.0;
        for n in a..b {
            let k = n as i64 + d;
            let o = if k >= 0 && (k as usize) < out.len() {
                out[k as usize]
            } else {
                0.0
            };
            let e = o - reference[n];
            s += e * e;
        }
        s
    };
    let mut best = (center, f64::MAX);
    for d in (center - radius)..=(center + radius) {
        let r = resid(d);
        if r < best.1 {
            best = (d, r);
        }
    }
    let (d, r0) = best;
    let rm = resid(d - 1);
    let rp = resid(d + 1);
    let denom = rm - 2.0 * r0 + rp;
    let frac = if denom.abs() > 1e-18 {
        0.5 * (rm - rp) / denom
    } else {
        0.0
    };
    d as f64 + frac.clamp(-0.5, 0.5)
}

fn db(x: f64) -> f64 {
    20.0 * x.max(1e-12).log10()
}

#[allow(clippy::needless_range_loop)]
/// Сравнивает выход движка с эталоном (оба стерео, одна частота). `bounds` — номера кадров
/// начала каждого трека в эталоне плюс конец.
pub fn analyze_joins(rendered: &[f32], reference: &[f32], bounds: &[usize]) -> GaplessReport {
    let out = left(rendered);
    let refl = left(reference);
    let total = *bounds.last().unwrap_or(&refl.len());
    let got_frames = {
        let mut last = 0;
        for (i, v) in out.iter().enumerate() {
            if v.abs() > 1e-4 {
                last = i + 1;
            }
        }
        last
    };
    // Общий сдвиг по началу сигнала. Радиус поиска мал: на 96 кГц период начального свипа
    // (110 Гц) — 873 сэмпла, при большом радиусе корреляция цепляется за соседний период.
    let win = 8192.min(total / 4);
    let offset = best_shift(&out, &refl, 2048, 2048 + win, 0, 64);
    let d = offset.round() as i64;
    let at = |n: usize| -> f64 {
        let k = n as i64 + d;
        if k >= 0 && (k as usize) < out.len() {
            out[k as usize]
        } else {
            0.0
        }
    };
    let mut joins = Vec::new();
    for &b in &bounds[1..bounds.len().saturating_sub(1)] {
        // Окна отступают от стыка на 2048 кадров: lossy-кодеки (MDCT до 2048) размазывают
        // щелчки-метки на стыке, и окно рядом с ними смещает оценку.
        let (w, gap) = (4096, 2048);
        let before = best_shift(&out, &refl, b - gap - w, b - gap, d, 48);
        let after = best_shift(&out, &refl, b + gap, b + gap + w, d, 48);
        let mut se = 0.0;
        let mut pk = 0f64;
        for n in (b - 128)..(b + 128) {
            let e = at(n) - refl[n];
            se += e * e;
            pk = pk.max(e.abs());
        }
        joins.push(JoinReport {
            at: b,
            shift: after - before,
            err_near_db: db((se / 256.0).sqrt()),
            peak_near_db: db(pk),
        });
    }
    let mut se = 0.0;
    let mut count = 0usize;
    for n in 1024..total.saturating_sub(1024) {
        if bounds.iter().any(|&b| n + 256 > b && n < b + 256) {
            continue;
        }
        let e = at(n) - refl[n];
        se += e * e;
        count += 1;
    }
    let err_body_db = db((se / count.max(1) as f64).sqrt());
    let mut max_zero_run = 0;
    let mut run = 0;
    let end = (total as i64 + d).clamp(0, out.len() as i64) as usize;
    for v in &out[..end] {
        if v.abs() < 1e-7 {
            run += 1;
            max_zero_run = max_zero_run.max(run);
        } else {
            run = 0;
        }
    }
    // Правый канал эталона — минус левый: проверяем, что каналы не перепутаны.
    let mut dot = 0.0;
    for n in (4096..total.saturating_sub(4096)).step_by(7) {
        let k = (n as i64 + d) as usize;
        if k * 2 + 1 < rendered.len() {
            dot += rendered[k * 2 + 1] as f64 * reference[n * 2 + 1] as f64;
        }
    }
    GaplessReport {
        expected_frames: total,
        got_frames,
        offset,
        joins,
        err_body_db,
        max_zero_run,
        channels_ok: dot > 0.0,
    }
}

/// Сравнивает выход движка с конкатенацией отдельно декодированных треков: при точной склейке
/// они совпадают бит в бит (декодирование детерминировано). Возвращает число отличающихся
/// сэмплов, максимальную разницу и разницу длины в кадрах.
pub fn compare_with_concat(
    rendered: &[f32],
    parts: &[SourceSpec],
) -> Result<(usize, f32, i64), AudioError> {
    let mut concat = Vec::new();
    for p in parts {
        concat.extend(decode_all(p)?.samples);
    }
    let n = concat.len().min(rendered.len());
    let mut diff = 0;
    let mut max = 0f32;
    for i in 0..n {
        let d = (rendered[i] - concat[i]).abs();
        if d > 0.0 {
            diff += 1;
            max = max.max(d);
        }
    }
    // Хвост выхода после конца — только тишина (последний блок захвата).
    let tail_nonzero = rendered[n..].iter().filter(|x| x.abs() > 0.0).count();
    let len_diff = if tail_nonzero > 0 {
        (rendered.len() as i64 - concat.len() as i64) / 2
    } else {
        (n as i64 - concat.len() as i64) / 2
    };
    Ok((diff, max, len_diff))
}

/// Читает WAV (эталон) в стерео f32.
pub fn read_wav(path: &std::path::Path) -> Result<(u32, Vec<f32>), AudioError> {
    let d = decode_all(&SourceSpec::File(path.to_path_buf()))?;
    Ok((d.rate, d.samples))
}
