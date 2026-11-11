//! Измеритель уровня для волны интерфейса: RMS, пик и три полосы за окно ~50 мс.
//!
//! Работает в аудиоколбэке на каждом кадре, поэтому только сложения и умножения: полосы —
//! однополюсные фильтры (до 150 Гц, 150–2000 Гц, выше 2 кГц), никакого БПФ. Каналы меряются
//! порознь: сумма в моно обнуляет противофазный звук. Готовое окно публикуется атомиками,
//! контроллер забирает его по номеру.

use std::f32::consts::TAU;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};

use crate::Level;

const LOW_HZ: f32 = 150.0;
const HIGH_HZ: f32 = 2000.0;

/// Последнее окно: rms, peak, low, mid, high (биты f32) и номер окна.
#[derive(Default)]
pub(crate) struct LevelCell {
    values: [AtomicU32; 5],
    seq: AtomicU64,
}

impl LevelCell {
    fn publish(&self, l: Level) {
        let v = [l.rms, l.peak, l.bands[0], l.bands[1], l.bands[2]];
        for (cell, x) in self.values.iter().zip(v) {
            cell.store(x.to_bits(), Ordering::Relaxed);
        }
        self.seq.fetch_add(1, Ordering::Release);
    }

    /// Номер окна и уровень; номер растёт, только пока звучит хоть что-то.
    pub fn get(&self) -> (u64, Level) {
        let seq = self.seq.load(Ordering::Acquire);
        let f = |i: usize| f32::from_bits(self.values[i].load(Ordering::Relaxed));
        (
            seq,
            Level {
                rms: f(0),
                peak: f(1),
                bands: [f(2), f(3), f(4)],
            },
        )
    }
}

pub(crate) struct Meter {
    a_low: f32,
    a_high: f32,
    /// Фильтры по каналам: [левый, правый]
    lp_low: [f32; 2],
    lp_high: [f32; 2],
    sum: f32,
    peak: f32,
    sum_bands: [f32; 3],
    frames: u32,
    window: u32,
}

fn one_pole(cutoff: f32, rate: u32) -> f32 {
    1.0 - (-TAU * cutoff / rate as f32).exp()
}

impl Meter {
    pub fn new(rate: u32) -> Self {
        Self {
            a_low: one_pole(LOW_HZ, rate),
            a_high: one_pole(HIGH_HZ, rate),
            lp_low: [0.0; 2],
            lp_high: [0.0; 2],
            sum: 0.0,
            peak: 0.0,
            sum_bands: [0.0; 3],
            frames: 0,
            window: (rate / 20).max(1),
        }
    }

    #[inline]
    pub fn push(&mut self, l: f32, r: f32, cell: &LevelCell) {
        for (c, x) in [l, r].into_iter().enumerate() {
            self.lp_low[c] += self.a_low * (x - self.lp_low[c]);
            self.lp_high[c] += self.a_high * (x - self.lp_high[c]);
            let bands = [
                self.lp_low[c],
                self.lp_high[c] - self.lp_low[c],
                x - self.lp_high[c],
            ];
            self.sum += x * x;
            for (s, b) in self.sum_bands.iter_mut().zip(bands) {
                *s += b * b;
            }
        }
        self.peak = self.peak.max(l.abs()).max(r.abs());
        self.frames += 1;
        if self.frames >= self.window {
            // Среднее по обоим каналам
            let n = 2.0 * self.frames as f32;
            cell.publish(Level {
                rms: (self.sum / n).sqrt(),
                peak: self.peak,
                bands: self.sum_bands.map(|s| (s / n).sqrt()),
            });
            self.sum = 0.0;
            self.peak = 0.0;
            self.sum_bands = [0.0; 3];
            self.frames = 0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn measure(freq: f32, amp: f32) -> Level {
        let rate = 48_000;
        let mut m = Meter::new(rate);
        let cell = LevelCell::default();
        // Полсекунды на установление фильтров, затем последнее окно
        for i in 0..rate / 2 {
            let s = amp * (TAU * freq * i as f32 / rate as f32).sin();
            m.push(s, s, &cell);
        }
        cell.get().1
    }

    #[test]
    fn sine_levels_and_bands() {
        let bass = measure(60.0, 0.5);
        // RMS синуса — амплитуда / √2
        assert!((bass.rms - 0.5 / 2f32.sqrt()).abs() < 0.01, "{bass:?}");
        assert!((bass.peak - 0.5).abs() < 0.01);
        assert!(bass.bands[0] > bass.bands[1] && bass.bands[0] > bass.bands[2] * 4.0);
        let treble = measure(6000.0, 0.5);
        assert!(treble.bands[2] > treble.bands[0] * 4.0 && treble.bands[2] > treble.bands[1]);
        let mid = measure(600.0, 0.5);
        assert!(mid.bands[1] > mid.bands[0] && mid.bands[1] > mid.bands[2]);
    }

    #[test]
    fn silence_is_zero() {
        let l = measure(440.0, 0.0);
        assert_eq!(l, Level::default());
    }

    #[test]
    fn antiphase_stereo_is_not_silence() {
        let rate = 48_000;
        let mut m = Meter::new(rate);
        let cell = LevelCell::default();
        for i in 0..rate / 2 {
            let s = 0.5 * (TAU * 600.0 * i as f32 / rate as f32).sin();
            m.push(s, -s, &cell);
        }
        let l = cell.get().1;
        assert!((l.rms - 0.5 / 2f32.sqrt()).abs() < 0.01, "{l:?}");
        assert!(l.bands[1] > 0.1);
    }
}
