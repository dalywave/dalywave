//! Обработка режима «Диджей» в аудиоколбэке: трёхполосный эквалайзер и фильтры на деку, эхо и
//! ограничитель на общий выход. Без выделения памяти: всё — числа и заранее выделенный буфер эха.
//!
//! Эквалайзер — кроссоверы Линквица — Райли 4-го порядка (по два биквада Баттерворта) на 200 Гц и
//! 2,5 кГц. Нижняя полоса проходит ещё и всепропускающий фильтр второго кроссовера: тогда сумма
//! полос при единичных усилениях — чистый фазовый сдвиг без провалов в АЧХ, а убранный бас
//! убирается целиком («kill», как на пульте).

use dalywave_transitions::Strip;

const LOW_HZ: f32 = 200.0;
const HIGH_HZ: f32 = 2500.0;
const BUTTERWORTH_Q: f32 = std::f32::consts::FRAC_1_SQRT_2;
/// Фильтры развёртки — с небольшим подъёмом у среза: развёртка слышна, но не свистит.
const SWEEP_Q: f32 = 0.9;

/// Биквад (транспонированная прямая форма II), состояние на два канала.
#[derive(Clone, Copy, Debug)]
struct Biquad {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
    z1: [f32; 2],
    z2: [f32; 2],
}

#[derive(Clone, Copy, Debug)]
enum Kind {
    Low,
    High,
    All,
}

impl Biquad {
    const fn identity() -> Self {
        Self {
            b0: 1.0,
            b1: 0.0,
            b2: 0.0,
            a1: 0.0,
            a2: 0.0,
            z1: [0.0; 2],
            z2: [0.0; 2],
        }
    }

    fn new(kind: Kind, rate: f32, hz: f32, q: f32) -> Self {
        let mut b = Self::identity();
        b.set(kind, rate, hz, q);
        b
    }

    /// Коэффициенты по RBJ Audio EQ Cookbook; состояние сохраняется — развёртка без щелчков.
    fn set(&mut self, kind: Kind, rate: f32, hz: f32, q: f32) {
        let hz = hz.clamp(10.0, rate * 0.45);
        let w = std::f32::consts::TAU * hz / rate;
        let (sin, cos) = w.sin_cos();
        let alpha = sin / (2.0 * q);
        let a0 = 1.0 + alpha;
        let (b0, b1, b2) = match kind {
            Kind::Low => ((1.0 - cos) / 2.0, 1.0 - cos, (1.0 - cos) / 2.0),
            Kind::High => ((1.0 + cos) / 2.0, -(1.0 + cos), (1.0 + cos) / 2.0),
            Kind::All => (1.0 - alpha, -2.0 * cos, 1.0 + alpha),
        };
        self.b0 = b0 / a0;
        self.b1 = b1 / a0;
        self.b2 = b2 / a0;
        self.a1 = -2.0 * cos / a0;
        self.a2 = (1.0 - alpha) / a0;
    }

    #[inline]
    fn run(&mut self, ch: usize, x: f32) -> f32 {
        let y = self.b0 * x + self.z1[ch];
        self.z1[ch] = self.b1 * x - self.a1 * y + self.z2[ch];
        self.z2[ch] = self.b2 * x - self.a2 * y;
        y
    }

    fn reset(&mut self) {
        self.z1 = [0.0; 2];
        self.z2 = [0.0; 2];
    }
}

/// Три полосы с компенсацией фазы.
#[derive(Clone, Copy, Debug)]
struct Eq3 {
    lp1: [Biquad; 2],
    hp1: [Biquad; 2],
    lp2: [Biquad; 2],
    hp2: [Biquad; 2],
    ap2: Biquad,
}

impl Eq3 {
    fn new(rate: f32) -> Self {
        let lr = |kind, hz| [Biquad::new(kind, rate, hz, BUTTERWORTH_Q); 2];
        Self {
            lp1: lr(Kind::Low, LOW_HZ),
            hp1: lr(Kind::High, LOW_HZ),
            lp2: lr(Kind::Low, HIGH_HZ),
            hp2: lr(Kind::High, HIGH_HZ),
            ap2: Biquad::new(Kind::All, rate, HIGH_HZ, BUTTERWORTH_Q),
        }
    }

    #[inline]
    fn run(&mut self, ch: usize, x: f32, g: [f32; 3]) -> f32 {
        let pair = |b: &mut [Biquad; 2], x: f32| {
            let y = b[0].run(ch, x);
            b[1].run(ch, y)
        };
        let low = pair(&mut self.lp1, x);
        let rest = pair(&mut self.hp1, x);
        let mid = pair(&mut self.lp2, rest);
        let high = pair(&mut self.hp2, rest);
        // LR4 НЧ + ВЧ второго кроссовера = всепропускающий 2-го порядка: нижняя полоса проходит
        // его же, чтобы фаза полос совпала
        let low = self.ap2.run(ch, low);
        g[0] * low + g[1] * mid + g[2] * high
    }

    fn reset(&mut self) {
        for b in self
            .lp1
            .iter_mut()
            .chain(self.hp1.iter_mut())
            .chain(self.lp2.iter_mut())
            .chain(self.hp2.iter_mut())
        {
            b.reset();
        }
        self.ap2.reset();
    }
}

/// Фильтр развёртки: включается и выключается плавным переходом «сухой ↔ обработанный».
#[derive(Clone, Copy, Debug)]
struct Sweep {
    bq: Biquad,
    kind: Kind,
    hz: f32,
    wet: f32,
    target: f32,
}

impl Sweep {
    fn new(kind: Kind) -> Self {
        Self {
            bq: Biquad::identity(),
            kind,
            hz: 0.0,
            wet: 0.0,
            target: 0.0,
        }
    }

    fn set(&mut self, rate: f32, hz: f32) {
        if hz > 0.0 {
            if self.wet == 0.0 && self.target == 0.0 {
                self.bq.reset();
            }
            if (hz - self.hz).abs() > 0.01 {
                self.bq.set(self.kind, rate, hz, SWEEP_Q);
                self.hz = hz;
            }
            self.target = 1.0;
        } else {
            self.target = 0.0;
        }
    }

    #[inline]
    fn active(&self) -> bool {
        self.wet > 0.0 || self.target > 0.0
    }

    #[inline]
    fn step(&mut self, step: f32) {
        if self.wet < self.target {
            self.wet = (self.wet + step).min(self.target);
        } else if self.wet > self.target {
            self.wet = (self.wet - step).max(self.target);
        }
    }

    #[inline]
    fn run(&mut self, ch: usize, x: f32) -> f32 {
        let y = self.bq.run(ch, x);
        x + self.wet * (y - x)
    }
}

/// Обработка одной деки по линейке [`Strip`]. Пока линейка нейтральна и обработка выключена,
/// звук проходит без изменений (и без фазового сдвига эквалайзера).
#[derive(Clone, Copy, Debug)]
pub(crate) struct DeckFx {
    rate: f32,
    eq: Eq3,
    hp: Sweep,
    lp: Sweep,
    /// Доля звука через эквалайзер: плавно 0 ↔ 1 при включении и выключении.
    eq_wet: f32,
    eq_target: f32,
    /// Усиления на начало и конец текущего куска — между ними линейно, без «ступенек».
    from: Strip,
    to: Strip,
    step: f32,
}

impl DeckFx {
    pub fn new(rate: u32) -> Self {
        let rate = rate as f32;
        Self {
            rate,
            eq: Eq3::new(rate),
            hp: Sweep::new(Kind::High),
            lp: Sweep::new(Kind::Low),
            eq_wet: 0.0,
            eq_target: 0.0,
            from: Strip::NEUTRAL,
            to: Strip::NEUTRAL,
            // Включение и выключение — за ~5 мс
            step: 1.0 / (rate * 0.005).max(1.0),
        }
    }

    /// Обработка нужна: эквалайзер или фильтр ещё включены.
    pub fn active(&self) -> bool {
        self.eq_wet > 0.0 || self.eq_target > 0.0 || self.hp.active() || self.lp.active()
    }

    /// Новая линейка на следующий кусок. `instant` — без перехода сухой → обработанный (дека
    /// только стартует из тишины).
    pub fn set(&mut self, strip: Strip, enabled: bool, instant: bool) {
        self.from = self.to;
        self.to = strip;
        self.eq_target = if enabled { 1.0 } else { 0.0 };
        if instant {
            if enabled && self.eq_wet == 0.0 {
                self.eq.reset();
            }
            self.eq_wet = self.eq_target;
            self.from = strip;
        }
        self.hp
            .set(self.rate, if enabled { strip.hp_hz } else { 0.0 });
        self.lp
            .set(self.rate, if enabled { strip.lp_hz } else { 0.0 });
    }

    /// Обработать кусок стерео-кадров на месте; `send` получает посыл на эхо.
    pub fn process(&mut self, buf: &mut [f32], send: &mut [f32]) {
        let n = buf.len() / 2;
        if n == 0 {
            return;
        }
        let (a, b) = (self.from, self.to);
        for f in 0..n {
            let u = (f + 1) as f32 / n as f32;
            let lerp = |x: f32, y: f32| x + (y - x) * u;
            let g = [lerp(a.low, b.low), lerp(a.mid, b.mid), lerp(a.high, b.high)];
            let gain = lerp(a.gain, b.gain);
            let echo = lerp(a.echo, b.echo);
            if self.eq_wet < self.eq_target {
                self.eq_wet = (self.eq_wet + self.step).min(self.eq_target);
            } else if self.eq_wet > self.eq_target {
                self.eq_wet = (self.eq_wet - self.step).max(self.eq_target);
            }
            self.hp.step(self.step);
            self.lp.step(self.step);
            for ch in 0..2 {
                let x = buf[f * 2 + ch];
                let mut y = x;
                if self.eq_wet > 0.0 {
                    let e = self.eq.run(ch, x, g);
                    y = x + self.eq_wet * (e - x);
                }
                if self.hp.active() {
                    y = self.hp.run(ch, y);
                }
                if self.lp.active() {
                    y = self.lp.run(ch, y);
                }
                send[f * 2 + ch] += y * echo;
                buf[f * 2 + ch] = y * gain;
            }
        }
        if self.eq_wet == 0.0 && self.eq_target == 0.0 {
            self.eq.reset();
        }
    }
}

/// Эхо на общем выходе: задержка с обратной связью, низ и верх повторов срезаны, чтобы хвост не
/// гудел и не звенел. Буфер — на 4 секунды, выделяется вместе с микшером.
pub(crate) struct EchoBus {
    buf: Vec<f32>,
    pos: usize,
    delay: usize,
    feedback: f32,
    lp: [f32; 2],
    hp: Biquad,
    /// Сколько кадров ещё звучит хвост (после последнего посыла).
    ring: usize,
}

impl EchoBus {
    pub fn new(rate: u32) -> Self {
        let frames = rate as usize * 4;
        Self {
            buf: vec![0.0; frames * 2],
            pos: 0,
            delay: rate as usize / 2,
            feedback: 0.5,
            lp: [0.0; 2],
            hp: Biquad::new(Kind::High, rate as f32, 180.0, BUTTERWORTH_Q),
            ring: 0,
        }
    }

    pub fn configure(&mut self, delay_frames: usize, feedback: f32) {
        self.delay = delay_frames.clamp(1, self.buf.len() / 2 - 1);
        self.feedback = feedback.clamp(0.0, 0.85);
    }

    pub fn active(&self) -> bool {
        self.ring > 0
    }

    /// Добавляет в `out` выход эха; `send` — что пришло на вход за этот кусок.
    pub fn process(&mut self, send: &[f32], out: &mut [f32]) {
        let frames = self.buf.len() / 2;
        let has_input = send.iter().any(|x| *x != 0.0);
        if has_input {
            // После последнего посыла хвост живёт, пока повтор не затихнет до −60 дБ
            let repeats = if self.feedback > 0.0 {
                (-3.0 / self.feedback.log10()).ceil() as usize + 1
            } else {
                1
            };
            self.ring = self.delay * repeats.min(64);
        } else if self.ring == 0 {
            return;
        }
        for f in 0..send.len() / 2 {
            let read = (self.pos + frames - self.delay) % frames;
            for ch in 0..2 {
                let delayed = self.buf[read * 2 + ch];
                // Затухание верха в каждом повторе — однополюсный НЧ
                self.lp[ch] += 0.35 * (delayed - self.lp[ch]);
                let wet = self.hp.run(ch, self.lp[ch]);
                self.buf[self.pos * 2 + ch] = send[f * 2 + ch] + wet * self.feedback;
                out[f * 2 + ch] += wet;
            }
            self.pos = (self.pos + 1) % frames;
            self.ring = self.ring.saturating_sub(1);
        }
        if self.ring == 0 {
            self.buf.fill(0.0);
            self.lp = [0.0; 2];
            self.hp.reset();
        }
    }
}

/// Ограничитель пиков общего выхода: две деки на полной громкости в середине смешивания не
/// уходят в перегруз. Атака ~0,3 мс, отпускание ~150 мс, потолок −1 dBFS.
pub(crate) struct Limiter {
    env: f32,
    attack: f32,
    release: f32,
    ceiling: f32,
}

impl Limiter {
    pub fn new(rate: u32) -> Self {
        let coef = |ms: f32| (-1.0 / (rate as f32 * ms / 1000.0)).exp();
        Self {
            env: 0.0,
            attack: coef(0.3),
            release: coef(150.0),
            ceiling: 10f32.powf(-1.0 / 20.0),
        }
    }

    #[inline]
    pub fn run(&mut self, l: f32, r: f32) -> (f32, f32) {
        let peak = l.abs().max(r.abs());
        let c = if peak > self.env {
            self.attack
        } else {
            self.release
        };
        self.env = peak + c * (self.env - peak);
        if self.env <= self.ceiling && peak <= self.ceiling {
            return (l, r);
        }
        // Огибающая догоняет пик за ~0,3 мс; то, что прорвалось раньше, срезает потолок
        let g = self.ceiling / self.env.max(self.ceiling);
        (
            (l * g).clamp(-self.ceiling, self.ceiling),
            (r * g).clamp(-self.ceiling, self.ceiling),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: u32 = 48_000;

    fn sine(hz: f32, frames: usize) -> Vec<f32> {
        (0..frames)
            .flat_map(|i| {
                let v = (std::f32::consts::TAU * hz * i as f32 / RATE as f32).sin() * 0.5;
                [v, v]
            })
            .collect()
    }

    fn rms_tail(buf: &[f32]) -> f32 {
        let tail = &buf[buf.len() / 2..];
        (tail.iter().map(|x| x * x).sum::<f32>() / tail.len() as f32).sqrt()
    }

    fn through(strip: Strip, hz: f32) -> f32 {
        let mut fx = DeckFx::new(RATE);
        fx.set(strip, true, true);
        let mut buf = sine(hz, RATE as usize / 2);
        let mut send = vec![0.0; buf.len()];
        for (chunk, s) in buf.chunks_mut(64).zip(send.chunks_mut(64)) {
            fx.set(strip, true, false);
            fx.process(chunk, s);
        }
        rms_tail(&buf) / rms_tail(&sine(hz, RATE as usize / 2))
    }

    #[test]
    fn eq_is_flat_at_unity_and_kills_bands() {
        for hz in [50.0, 200.0, 1000.0, 2500.0, 8000.0] {
            let g = through(Strip::NEUTRAL, hz);
            assert!((g - 1.0).abs() < 0.02, "{hz} Hz: {g}");
        }
        let no_bass = Strip {
            low: 0.0,
            ..Strip::NEUTRAL
        };
        assert!(through(no_bass, 50.0) < 0.05, "бас убран");
        assert!(
            (through(no_bass, 1000.0) - 1.0).abs() < 0.05,
            "середина цела"
        );
        let no_high = Strip {
            high: 0.0,
            ..Strip::NEUTRAL
        };
        assert!(through(no_high, 10_000.0) < 0.05);
    }

    #[test]
    fn sweep_filters_cut() {
        let hp = Strip {
            hp_hz: 1000.0,
            ..Strip::NEUTRAL
        };
        assert!(through(hp, 100.0) < 0.05);
        let lp = Strip {
            lp_hz: 300.0,
            ..Strip::NEUTRAL
        };
        assert!(through(lp, 5000.0) < 0.02);
    }

    #[test]
    fn echo_repeats_and_dies() {
        let mut bus = EchoBus::new(RATE);
        bus.configure(4800, 0.5);
        let mut send = vec![0.0; 9600 * 2];
        send[0] = 1.0;
        send[1] = 1.0;
        let mut out = vec![0.0; send.len()];
        bus.process(&send, &mut out);
        let first = out.iter().step_by(2).position(|x| x.abs() > 0.05).unwrap();
        assert!(
            (4800..4810).contains(&first),
            "повтор через задержку: {first}"
        );
        assert!(bus.active());
        let silent = vec![0.0; RATE as usize * 2 * 4];
        let mut out = vec![0.0; silent.len()];
        bus.process(&silent, &mut out);
        assert!(!bus.active(), "хвост затих");
    }

    #[test]
    fn limiter_keeps_ceiling() {
        let mut lim = Limiter::new(RATE);
        let ceiling = 10f32.powf(-1.0 / 20.0);
        for i in 0..RATE {
            let x = 1.6 * (i as f32 * 0.05).sin();
            let (l, r) = lim.run(x, -x);
            assert!(l.abs() <= ceiling + 1e-6 && r.abs() <= ceiling + 1e-6);
        }
        // Тихое проходит как есть
        let mut lim = Limiter::new(RATE);
        assert_eq!(lim.run(0.3, -0.2), (0.3, -0.2));
    }
}
