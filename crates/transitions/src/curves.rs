//! Автоматика перехода: что делать с каждой декой через `t` мс после начала перехода.
//!
//! [`Curves`] — несколько чисел (`Copy`), а [`Curves::at`] — чистая функция без выделения памяти:
//! микшер зовёт её в аудиоколбэке на каждые 32 кадра.

use std::f64::consts::FRAC_PI_2;

use crate::plan::Technique;

/// Линейка деки: громкость, три полосы эквалайзера, фильтры и посыл на эхо.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Strip {
    pub gain: f32,
    /// Усиления полос (линейные): до ~200 Гц, середина, выше ~2,5 кГц.
    pub low: f32,
    pub mid: f32,
    pub high: f32,
    /// Срез фильтра высоких частот, Гц; 0 — выключен.
    pub hp_hz: f32,
    /// Срез фильтра низких частот, Гц; 0 — выключен.
    pub lp_hz: f32,
    /// Посыл на эхо, 0…1 (до громкости деки).
    pub echo: f32,
}

impl Strip {
    pub const NEUTRAL: Strip = Strip {
        gain: 1.0,
        low: 1.0,
        mid: 1.0,
        high: 1.0,
        hp_hz: 0.0,
        lp_hz: 0.0,
        echo: 0.0,
    };
    pub const SILENT: Strip = Strip {
        gain: 0.0,
        ..Strip::NEUTRAL
    };
}

/// Обе деки в один момент перехода.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Mix {
    pub out: Strip,
    pub inc: Strip,
}

/// Параметры кривых одного перехода. Время — от начала перехода, мс.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Curves {
    pub technique: Technique,
    /// Длина автоматики: после неё уходящий молчит, входящий — без обработки.
    pub length_ms: f64,
    /// Входящий стартует через столько мс после начала перехода (эхо-выход: такт посыла).
    pub in_offset_ms: f64,
    /// Смена баса — на сильной доле.
    pub swap_ms: f64,
    /// Доля уходящего, мс.
    pub beat_ms: f64,
    /// Выразительность 0…1: длина посыла на эхо, крутизна фильтра.
    pub expressiveness: f64,
    pub echo_delay_ms: f64,
    pub echo_feedback: f64,
}

fn clamp01(x: f64) -> f64 {
    x.clamp(0.0, 1.0)
}

/// Равномощные кривые: cos² + sin² = 1.
fn eq_in(u: f64) -> f32 {
    (clamp01(u) * FRAC_PI_2).sin() as f32
}

fn eq_out(u: f64) -> f32 {
    (clamp01(u) * FRAC_PI_2).cos() as f32
}

/// Экспоненциальный ход частоты среза от `a` к `b` Гц при u ∈ [0, 1].
fn sweep(a: f64, b: f64, u: f64) -> f32 {
    (a * (b / a).powf(clamp01(u))) as f32
}

/// Смена баса за 1/8 доли: щелчка нет, а на слух — мгновенно, точно на долю.
fn swap(t: f64, at: f64, beat: f64) -> f32 {
    let w = (beat / 8.0).max(10.0);
    clamp01((t - at) / w) as f32
}

impl Curves {
    pub fn done(&self, t_ms: f64) -> bool {
        t_ms >= self.length_ms
    }

    pub fn at(&self, t: f64) -> Mix {
        let len = self.length_ms.max(1.0);
        let u = t / len;
        let mut out = Strip::NEUTRAL;
        let mut inc = Strip::NEUTRAL;
        if t >= len {
            return Mix {
                out: Strip::SILENT,
                inc,
            };
        }
        match self.technique {
            Technique::Fade => {
                out.gain = eq_out(u);
                inc.gain = eq_in(u);
            }
            Technique::Blend => {
                // Входящий выходит в первой половине без баса, на сильной доле в середине басы
                // меняются местами, во второй половине уходит уходящий.
                inc.gain = eq_in(u / 0.5);
                out.gain = eq_out((u - 0.5) / 0.5);
                inc.low = swap(t, self.swap_ms, self.beat_ms);
                out.low = 1.0 - inc.low;
                // Верх входящего открывается чуть позже — тарелки не спорят
                inc.high = 0.5 + 0.5 * eq_in(u / 0.6);
            }
            Technique::Filter => {
                inc.gain = eq_in(u / 0.25);
                inc.lp_hz = if u < 0.5 {
                    sweep(250.0, 18_000.0, u / 0.5)
                } else {
                    0.0
                };
                inc.low = swap(t, self.swap_ms, self.beat_ms);
                out.low = 1.0 - inc.low;
                out.hp_hz = if u > 0.25 {
                    sweep(
                        25.0,
                        600.0 + 1400.0 * self.expressiveness,
                        (u - 0.25) / 0.75,
                    )
                } else {
                    0.0
                };
                out.gain = eq_out((u - 0.75) / 0.25);
            }
            Technique::Echo => {
                let cut = self.in_offset_ms;
                // Посыл на эхо — последняя доля перед срезом (смелее — весь такт)
                let send_len = if self.expressiveness >= 0.67 {
                    self.beat_ms * 4.0
                } else {
                    self.beat_ms
                };
                out.echo = clamp01((t - (cut - send_len)) / send_len) as f32;
                // К срезу уходящий слегка «поднимается»: срезаем низ
                out.hp_hz = if t > cut - self.beat_ms * 4.0 {
                    sweep(
                        25.0,
                        250.0,
                        (t - (cut - self.beat_ms * 4.0)) / (self.beat_ms * 4.0),
                    )
                } else {
                    0.0
                };
                out.gain = 1.0 - clamp01((t - cut) / 8.0) as f32;
                if t >= cut {
                    out.echo = 0.0;
                }
                inc.gain = clamp01((t - cut) / 3.0) as f32;
            }
            Technique::Cut => {
                out.gain = 1.0 - clamp01(t / 8.0) as f32;
                inc.gain = clamp01(t / 3.0) as f32;
            }
        }
        Mix { out, inc }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn curves(technique: Technique) -> Curves {
        Curves {
            technique,
            length_ms: 32_000.0,
            in_offset_ms: 0.0,
            swap_ms: 16_000.0,
            beat_ms: 500.0,
            expressiveness: 0.5,
            echo_delay_ms: 375.0,
            echo_feedback: 0.5,
        }
    }

    #[test]
    fn blend_swaps_bass_on_the_beat() {
        let c = curves(Technique::Blend);
        let before = c.at(15_990.0);
        assert_eq!((before.out.low, before.inc.low), (1.0, 0.0));
        let after = c.at(16_000.0 + 62.5);
        assert_eq!((after.out.low, after.inc.low), (0.0, 1.0));
        // Начало: только уходящий; конец: только входящий
        let s = c.at(0.0);
        assert_eq!((s.out.gain, s.inc.gain), (1.0, 0.0));
        let e = c.at(31_999.0);
        assert!(e.out.gain < 0.01 && (e.inc.gain - 1.0).abs() < 1e-6);
        assert_eq!(c.at(32_000.0).out, Strip::SILENT);
        assert!(c.done(32_000.0));
    }

    #[test]
    fn fade_keeps_power() {
        let c = curves(Technique::Fade);
        for k in 0..=100 {
            let m = c.at(k as f64 * 320.0 - 1.0);
            let p = m.out.gain.powi(2) + m.inc.gain.powi(2);
            assert!((p - 1.0).abs() < 1e-5);
        }
    }

    #[test]
    fn echo_out_cuts_on_the_one() {
        let mut c = curves(Technique::Echo);
        c.in_offset_ms = 2000.0;
        c.length_ms = 2010.0;
        assert_eq!(c.at(1000.0).out.echo, 0.0);
        assert!(c.at(1750.0).out.echo > 0.4);
        let at_cut = c.at(2008.0);
        assert_eq!(at_cut.out.gain, 0.0);
        assert_eq!(at_cut.inc.gain, 1.0);
    }

    #[test]
    fn filter_sweeps_up_and_down() {
        let c = curves(Technique::Filter);
        let a = c.at(1000.0);
        let b = c.at(14_000.0);
        assert!(a.inc.lp_hz > 0.0 && a.inc.lp_hz < b.inc.lp_hz);
        assert_eq!(a.out.hp_hz, 0.0);
        assert!(c.at(20_000.0).out.hp_hz < c.at(30_000.0).out.hp_hz);
        assert_eq!(c.at(20_000.0).inc.lp_hz, 0.0);
    }
}
