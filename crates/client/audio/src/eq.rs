//! Эквалайзер на общем выходе: десять полос (31 Гц … 16 кГц, шаг — октава), колоколообразные
//! фильтры RBJ с Q = √2 (ширина — октава), предусиление вниз на самый высокий подъём АЧХ, чтобы
//! подъём не уводил звук в перегрузку (что прорвётся на переходных, срежет ограничитель микшера).
//!
//! В аудиоколбэке: усиления полос плавно идут к цели, коэффициенты пересчитываются раз в 16 кадров —
//! смена пресета и движение ползунков без щелчков. Выключенный и плоский эквалайзер не трогает
//! звук вовсе: склейка без пауз остаётся точной до бита. Состояние фильтров — в f64: на 31 Гц
//! полюса у самой единичной окружности, f32 там шумит.

use std::f64::consts::TAU;

/// Центры полос, Гц.
pub const EQ_BANDS_HZ: [f32; EQ_BANDS] = [
    31.0, 62.0, 125.0, 250.0, 500.0, 1000.0, 2000.0, 4000.0, 8000.0, 16000.0,
];
pub const EQ_BANDS: usize = 10;
/// Предел усиления полосы, дБ.
pub const EQ_MAX_DB: f32 = 12.0;
/// Ширина полосы — октава.
const Q: f64 = std::f64::consts::SQRT_2;
/// Кусок, на который коэффициенты считаются один раз.
const CHUNK: usize = 16;
/// За сколько самое большое изменение (24 дБ) доходит до цели, мс: шаг за кусок — доли децибела,
/// «молнии» от ступенек не слышно.
const GLIDE_MS: f32 = 150.0;

/// Готовые настройки: интерфейс показывает их по-русски («Больше баса», «Вокал»…). Таблица та же,
/// что в `apps/ui/app/utils/eq.ts` — сайт играет те же полосы через WebAudio.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EqPreset {
    Off,
    BassBoost,
    BassCut,
    Vocal,
    Acoustic,
    Electronic,
    /// «Громче тихое»: кривая тонкомпенсации — басы и верх, которые ухо теряет на малой громкости
    Loudness,
}

impl EqPreset {
    pub const ALL: [EqPreset; 7] = [
        Self::Off,
        Self::BassBoost,
        Self::BassCut,
        Self::Vocal,
        Self::Acoustic,
        Self::Electronic,
        Self::Loudness,
    ];

    pub fn id(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::BassBoost => "bass-boost",
            Self::BassCut => "bass-cut",
            Self::Vocal => "vocal",
            Self::Acoustic => "acoustic",
            Self::Electronic => "electronic",
            Self::Loudness => "loudness",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|p| p.id() == id)
    }

    /// Усиления полос, дБ: 31, 62, 125, 250, 500 Гц, 1, 2, 4, 8, 16 кГц.
    pub fn gains(self) -> [f32; EQ_BANDS] {
        match self {
            Self::Off => [0.0; EQ_BANDS],
            Self::BassBoost => [6.0, 5.5, 4.5, 2.5, 0.5, 0.0, 0.0, 0.0, 0.0, 0.0],
            Self::BassCut => [-6.0, -5.5, -4.5, -2.5, -0.5, 0.0, 0.0, 0.0, 0.0, 0.0],
            Self::Vocal => [-2.0, -2.0, -1.5, 0.0, 2.0, 3.5, 3.5, 2.0, 0.0, -1.0],
            Self::Acoustic => [3.0, 3.0, 2.5, 1.0, 1.0, 0.5, 1.5, 2.5, 2.5, 2.0],
            Self::Electronic => [4.5, 4.0, 1.5, 0.0, -1.5, 1.0, 0.5, 1.5, 4.0, 4.5],
            Self::Loudness => [5.0, 4.0, 2.0, 0.0, -1.0, -1.0, 0.0, 1.0, 3.0, 4.0],
        }
    }
}

/// Что играет эквалайзер: усиления полос, дБ (обрезаются до ±12).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EqSettings {
    pub gains_db: [f32; EQ_BANDS],
}

impl EqSettings {
    pub fn new(gains_db: [f32; EQ_BANDS]) -> Self {
        let mut g = gains_db;
        for x in &mut g {
            *x = if x.is_finite() {
                x.clamp(-EQ_MAX_DB, EQ_MAX_DB)
            } else {
                0.0
            };
        }
        Self { gains_db: g }
    }

    pub fn preset(p: EqPreset) -> Self {
        Self::new(p.gains())
    }

    pub fn is_flat(&self) -> bool {
        self.gains_db.iter().all(|g| *g == 0.0)
    }

    /// АЧХ всей цепочки на частоте `hz` при частоте дискретизации `rate`, дБ (без предусиления).
    pub fn response_db(&self, hz: f32, rate: u32) -> f32 {
        let rate = f64::from(rate);
        let w = TAU * f64::from(hz) / rate;
        let mut db = 0.0;
        for (b, g) in self.gains_db.iter().enumerate() {
            let c = Coefs::peaking(f64::from(EQ_BANDS_HZ[b]), f64::from(*g), rate);
            db += c.magnitude_db(w);
        }
        db as f32
    }

    /// Предусиление, дБ (≤ 0): минус самый высокий подъём АЧХ в слышимом диапазоне — синус любой
    /// частоты с полной шкалой на входе остаётся в шкале и на выходе.
    pub fn preamp_db(&self, rate: u32) -> f32 {
        if self.gains_db.iter().all(|g| *g <= 0.0) {
            return 0.0;
        }
        let top = (rate as f32 * 0.5).min(20_000.0);
        let steps = 480;
        let mut peak = 0.0f32;
        for i in 0..=steps {
            let hz = 20.0 * (top / 20.0).powf(i as f32 / steps as f32);
            peak = peak.max(self.response_db(hz, rate));
        }
        for hz in EQ_BANDS_HZ {
            if hz < top {
                peak = peak.max(self.response_db(hz, rate));
            }
        }
        -peak.max(0.0)
    }
}

/// Коэффициенты биквада, нормированные на a0.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Coefs {
    b0: f64,
    b1: f64,
    b2: f64,
    a1: f64,
    a2: f64,
}

impl Coefs {
    const IDENTITY: Self = Self {
        b0: 1.0,
        b1: 0.0,
        b2: 0.0,
        a1: 0.0,
        a2: 0.0,
    };

    /// Колокол по RBJ Audio EQ Cookbook.
    fn peaking(hz: f64, gain_db: f64, rate: f64) -> Self {
        let hz = hz.clamp(10.0, rate * 0.45);
        let (sin, cos) = (TAU * hz / rate).sin_cos();
        Self::from_parts(sin / (2.0 * Q), cos, 10f64.powf(gain_db / 40.0))
    }

    #[inline]
    fn from_parts(alpha: f64, cos: f64, a: f64) -> Self {
        let a0 = 1.0 + alpha / a;
        Self {
            b0: (1.0 + alpha * a) / a0,
            b1: -2.0 * cos / a0,
            b2: (1.0 - alpha * a) / a0,
            a1: -2.0 * cos / a0,
            a2: (1.0 - alpha / a) / a0,
        }
    }

    fn magnitude_db(&self, w: f64) -> f64 {
        // |B(e^jw)| / |A(e^jw)|, A = 1 + a1 z⁻¹ + a2 z⁻²
        let (c1, s1) = (w.cos(), w.sin());
        let (c2, s2) = ((2.0 * w).cos(), (2.0 * w).sin());
        let br = self.b0 + self.b1 * c1 + self.b2 * c2;
        let bi = -(self.b1 * s1 + self.b2 * s2);
        let ar = 1.0 + self.a1 * c1 + self.a2 * c2;
        let ai = -(self.a1 * s1 + self.a2 * s2);
        10.0 * ((br * br + bi * bi) / (ar * ar + ai * ai)).log10()
    }
}

/// Полоса: постоянные части коэффициентов, текущее усиление и состояние на два канала.
#[derive(Clone, Copy, Debug)]
struct Band {
    alpha: f64,
    cos: f64,
    gain_db: f32,
    c: Coefs,
    z1: [f64; 2],
    z2: [f64; 2],
}

impl Band {
    fn new(hz: f32, rate: u32) -> Self {
        let rate = f64::from(rate);
        let hz = f64::from(hz).clamp(10.0, rate * 0.45);
        let (sin, cos) = (TAU * hz / rate).sin_cos();
        Self {
            alpha: sin / (2.0 * Q),
            cos,
            gain_db: 0.0,
            c: Coefs::IDENTITY,
            z1: [0.0; 2],
            z2: [0.0; 2],
        }
    }

    fn set_gain(&mut self, db: f32) {
        self.gain_db = db;
        self.c = if db == 0.0 {
            Coefs::IDENTITY
        } else {
            Coefs::from_parts(self.alpha, self.cos, 10f64.powf(f64::from(db) / 40.0))
        };
    }

    #[inline]
    fn run(&mut self, ch: usize, x: f64) -> f64 {
        let c = &self.c;
        let y = c.b0 * x + self.z1[ch];
        self.z1[ch] = c.b1 * x - c.a1 * y + self.z2[ch];
        self.z2[ch] = c.b2 * x - c.a2 * y;
        y
    }

    fn reset(&mut self) {
        self.z1 = [0.0; 2];
        self.z2 = [0.0; 2];
    }
}

/// Эквалайзер в аудиоколбэке. Без выделения памяти.
pub(crate) struct Equalizer {
    bands: [Band; EQ_BANDS],
    target: [f32; EQ_BANDS],
    preamp_db: f32,
    target_preamp_db: f32,
    /// Шаг усиления за кусок, дБ
    step: f32,
    active: bool,
}

/// Команда микшеру: цели усилений и предусиления (посчитано контроллером).
#[derive(Clone, Copy, Debug)]
pub(crate) struct EqTarget {
    pub gains_db: [f32; EQ_BANDS],
    pub preamp_db: f32,
}

impl EqTarget {
    pub fn new(settings: Option<&EqSettings>, rate: u32) -> Self {
        match settings {
            Some(s) if !s.is_flat() => Self {
                gains_db: s.gains_db,
                preamp_db: s.preamp_db(rate),
            },
            _ => Self {
                gains_db: [0.0; EQ_BANDS],
                preamp_db: 0.0,
            },
        }
    }
}

impl Equalizer {
    pub fn new(rate: u32) -> Self {
        let chunks = (GLIDE_MS / 1000.0 * rate as f32 / CHUNK as f32).max(1.0);
        Self {
            bands: EQ_BANDS_HZ.map(|hz| Band::new(hz, rate)),
            target: [0.0; EQ_BANDS],
            preamp_db: 0.0,
            target_preamp_db: 0.0,
            // Самый большой скачок (24 дБ) проходится за время скольжения
            step: 2.0 * EQ_MAX_DB / chunks,
            active: false,
        }
    }

    pub fn set(&mut self, t: EqTarget) {
        self.target = t.gains_db;
        self.target_preamp_db = t.preamp_db;
        if !self.at_rest() {
            self.active = true;
        }
    }

    /// Звук идёт мимо: всё плоское и пришло к цели.
    fn at_rest(&self) -> bool {
        self.preamp_db == 0.0
            && self.target_preamp_db == 0.0
            && self
                .bands
                .iter()
                .zip(&self.target)
                .all(|(b, t)| b.gain_db == 0.0 && *t == 0.0)
    }

    pub fn active(&self) -> bool {
        self.active
    }

    fn glide(&mut self) {
        let step = self.step;
        let approach = |cur: f32, to: f32| {
            if (to - cur).abs() <= step {
                to
            } else {
                cur + step.copysign(to - cur)
            }
        };
        for (b, t) in self.bands.iter_mut().zip(self.target) {
            if b.gain_db != t {
                b.set_gain(approach(b.gain_db, t));
            }
        }
        self.preamp_db = approach(self.preamp_db, self.target_preamp_db);
    }

    /// Обработать стерео с чередованием каналов на месте.
    pub fn process(&mut self, buf: &mut [f32]) {
        if !self.active {
            return;
        }
        for chunk in buf.chunks_mut(CHUNK * 2) {
            let pre_from = 10f64.powf(f64::from(self.preamp_db) / 20.0);
            self.glide();
            let pre_to = 10f64.powf(f64::from(self.preamp_db) / 20.0);
            // Предусиление — по сэмплу, без ступенек
            let frames = (chunk.len() / 2).max(1) as f64;
            for (i, pair) in chunk.as_chunks_mut::<2>().0.iter_mut().enumerate() {
                let pre = pre_from + (pre_to - pre_from) * (i + 1) as f64 / frames;
                for (ch, s) in pair.iter_mut().enumerate() {
                    let mut x = f64::from(*s) * pre;
                    for b in &mut self.bands {
                        x = b.run(ch, x);
                    }
                    *s = x as f32;
                }
            }
        }
        if self.at_rest() {
            // Всё плоско: дальше — мимо, с чистыми фильтрами для следующего включения
            self.active = false;
            for b in &mut self.bands {
                b.reset();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: u32 = 48_000;

    fn sine(hz: f32, frames: usize, amp: f32) -> Vec<f32> {
        (0..frames)
            .flat_map(|i| {
                let v = (std::f32::consts::TAU * hz * i as f32 / RATE as f32).sin() * amp;
                [v, v]
            })
            .collect()
    }

    fn rms(buf: &[f32]) -> f32 {
        (buf.iter().map(|x| x * x).sum::<f32>() / buf.len() as f32).sqrt()
    }

    fn eq_for(s: &EqSettings) -> Equalizer {
        let mut eq = Equalizer::new(RATE);
        eq.set(EqTarget::new(Some(s), RATE));
        // Пройти скольжение до цели на тишине
        let mut warm = vec![0.0f32; RATE as usize / 2 * 2];
        eq.process(&mut warm);
        eq
    }

    /// Логарифмический свип через эквалайзер: усиление в окнах вокруг заданных частот сверяется
    /// с расчётной АЧХ (с предусилением).
    fn sweep_response(s: &EqSettings, probes: &[f32]) -> Vec<(f32, f32, f32)> {
        let (f0, f1, secs) = (20.0f64, 20_000.0f64, 12.0f64);
        let n = (secs * f64::from(RATE)) as usize;
        let k = (f1 / f0).ln();
        let input: Vec<f32> = (0..n)
            .flat_map(|i| {
                let t = i as f64 / f64::from(RATE);
                let phase = TAU * f0 * secs / k * ((k * t / secs).exp() - 1.0);
                let v = (phase.sin() * 0.25) as f32;
                [v, v]
            })
            .collect();
        let mut out = input.clone();
        let mut eq = eq_for(s);
        eq.process(&mut out);
        let pre = s.preamp_db(RATE);
        probes
            .iter()
            .map(|&hz| {
                // Окно ±1/12 октавы вокруг частоты: свип медленный, частота почти постоянна
                let at = |f: f64| ((f / f0).ln() / k * n as f64) as usize;
                let (a, b) = (
                    at(f64::from(hz) * 2f64.powf(-1.0 / 12.0)),
                    at(f64::from(hz) * 2f64.powf(1.0 / 12.0)),
                );
                let got = 20.0 * (rms(&out[a * 2..b * 2]) / rms(&input[a * 2..b * 2])).log10();
                let want = s.response_db(hz, RATE) + pre;
                (hz, got, want)
            })
            .collect()
    }

    #[test]
    fn presets_follow_their_designed_response() {
        let probes = [
            40.0, 62.0, 100.0, 125.0, 250.0, 500.0, 1000.0, 2000.0, 4000.0, 8000.0, 12000.0,
        ];
        for p in EqPreset::ALL {
            let s = EqSettings::preset(p);
            let rows = sweep_response(&s, &probes);
            let line: Vec<String> = rows
                .iter()
                .map(|(hz, got, _)| format!("{hz:.0}:{got:+.1}"))
                .collect();
            println!(
                "{:11} preamp {:+.1} dB | {}",
                p.id(),
                s.preamp_db(RATE),
                line.join(" ")
            );
            for (hz, got, want) in rows {
                assert!(
                    (got - want).abs() < 0.6,
                    "{}: {hz} Hz got {got:+.2} dB, want {want:+.2} dB",
                    p.id()
                );
            }
        }
    }

    #[test]
    fn preset_shapes_mean_what_they_say() {
        let at = |p: EqPreset, hz: f32| {
            let s = EqSettings::preset(p);
            s.response_db(hz, RATE) + s.preamp_db(RATE)
        };
        // Больше баса: низ громче середины на 5+ дБ, меньше баса — наоборот
        assert!(at(EqPreset::BassBoost, 60.0) - at(EqPreset::BassBoost, 1000.0) > 5.0);
        assert!(at(EqPreset::BassCut, 1000.0) - at(EqPreset::BassCut, 60.0) > 5.0);
        // Вокал: середина над краями
        assert!(at(EqPreset::Vocal, 1500.0) - at(EqPreset::Vocal, 60.0) > 4.0);
        // Громче тихое: края над серединой
        assert!(at(EqPreset::Loudness, 40.0) - at(EqPreset::Loudness, 700.0) > 4.0);
        assert!(at(EqPreset::Loudness, 14000.0) - at(EqPreset::Loudness, 700.0) > 3.0);
    }

    #[test]
    fn preamp_keeps_full_scale_sines_out_of_clipping() {
        for p in EqPreset::ALL {
            let s = EqSettings::preset(p);
            for hz in [
                31.0, 45.0, 62.0, 90.0, 125.0, 1000.0, 8000.0, 11000.0, 16000.0,
            ] {
                let mut eq = eq_for(&s);
                let mut buf = sine(hz, RATE as usize, 1.0);
                eq.process(&mut buf);
                let peak = buf[buf.len() / 2..]
                    .iter()
                    .fold(0.0f32, |m, x| m.max(x.abs()));
                assert!(peak <= 1.0 + 1e-3, "{}: {hz} Hz peaks at {peak}", p.id());
            }
        }
        // Свой: всё вверх на 12 дБ — предусиление −12 и выше
        let all_up = EqSettings::new([12.0; EQ_BANDS]);
        assert!(all_up.preamp_db(RATE) <= -12.0);
        let mut eq = eq_for(&all_up);
        let mut buf = sine(3000.0, RATE as usize, 1.0);
        eq.process(&mut buf);
        assert!(buf.iter().all(|x| x.abs() <= 1.001));
    }

    #[test]
    fn flat_or_off_is_bit_exact_passthrough() {
        let input = sine(440.0, 4800, 0.7);
        let mut eq = Equalizer::new(RATE);
        let mut buf = input.clone();
        eq.process(&mut buf);
        assert_eq!(buf, input);
        // Включили и выключили: после скольжения к нулю — снова мимо, бит в бит
        eq.set(EqTarget::new(
            Some(&EqSettings::preset(EqPreset::Vocal)),
            RATE,
        ));
        let mut warm = sine(440.0, RATE as usize / 4, 0.7);
        eq.process(&mut warm);
        assert!(eq.active());
        eq.set(EqTarget::new(None, RATE));
        let mut tail = sine(440.0, RATE as usize / 4, 0.7);
        eq.process(&mut tail);
        assert!(!eq.active());
        let mut buf = input.clone();
        eq.process(&mut buf);
        assert_eq!(buf, input);
    }

    #[test]
    fn preset_change_glides_without_clicks() {
        // Тон 1 кГц; посреди — с «Меньше баса» на «Вокал» (+3,5 дБ на 1 кГц). Соседние сэмплы не
        // прыгают сильнее, чем позволяет сам синус с новой громкостью
        let s1 = EqSettings::preset(EqPreset::BassCut);
        let s2 = EqSettings::preset(EqPreset::Vocal);
        let mut eq = eq_for(&s1);
        let mut a = sine(1000.0, RATE as usize / 2, 0.5);
        eq.process(&mut a);
        eq.set(EqTarget::new(Some(&s2), RATE));
        let mut b: Vec<f32> = sine(1000.0, RATE as usize, 0.5)[(RATE as usize / 2) * 2..].to_vec();
        b.extend(sine(1000.0, RATE as usize / 2, 0.5));
        eq.process(&mut b);
        let max_step = 0.5 * 2.0 * std::f32::consts::PI * 1000.0 / RATE as f32 * 1.6;
        let left: Vec<f32> = a.iter().chain(&b).step_by(2).copied().collect();
        for w in left.windows(2) {
            assert!(
                (w[1] - w[0]).abs() < max_step,
                "jump {}",
                (w[1] - w[0]).abs()
            );
        }
    }

    #[test]
    fn preset_ids_round_trip() {
        for p in EqPreset::ALL {
            assert_eq!(EqPreset::from_id(p.id()), Some(p));
        }
        assert_eq!(EqPreset::from_id("custom"), None);
    }
}
