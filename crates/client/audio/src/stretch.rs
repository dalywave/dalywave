//! Растяжение по времени без смены высоты (Signalsmith Stretch) по карте темпа деки.
//!
//! Стоит в потоке деки до ресемплера, на частоте файла. Карта говорит, какое место файла должно
//! звучать через `t` после старта деки; по ней на каждый блок выхода подаётся столько входа,
//! сколько нужно, — так доли входящего ложатся точно на доли уходящего, а после перехода скорость
//! плавно возвращается к единице.
//!
//! Задержка растяжения компенсирована: первые `inputLatency` кадров уходят в `seek`, первые
//! `outputLatency` кадров выхода выбрасываются — тогда кадр выхода 0 — это ровно начало карты
//! (замерено на щелчках, `tests::clicks_stay_on_the_map`). Когда скорость давно единица, звук
//! плавно (за ~40 мс) переходит на прямой путь, растяжение выключается и больше не тратит батарею.

use dalywave_transitions::TempoMap;
use ssstretch::Stretch;

const BLOCK: usize = 512;
/// Через сколько после возврата к своей скорости переходить на прямой путь, мс.
const BYPASS_AFTER_MS: f64 = 1000.0;
const BYPASS_XFADE: usize = 2048;

pub(crate) struct Stretcher {
    st: Option<Stretch>,
    rate: f64,
    map: TempoMap,
    in_latency: usize,
    out_latency: usize,
    /// Вход по каналам; `fifo[0]` — кадр файла `fifo_abs`.
    fifo: [Vec<f32>; 2],
    fifo_abs: u64,
    /// Сколько кадров файла уже отдано растяжению (абсолютный номер следующего).
    fed_abs: u64,
    seeded: bool,
    /// Кадров выхода растяжения, включая выброшенную задержку.
    produced: u64,
    out: [Vec<f32>; 2],
    /// Прямой путь: следующий кадр файла для выхода и сколько кадров перехода осталось.
    bypass_at: Option<u64>,
    xfade_left: usize,
}

impl Stretcher {
    /// Декодер уже стоит на начале карты (`map.start()`).
    pub fn new(source_rate: u32, map: TempoMap) -> Self {
        let mut st = Stretch::new();
        st.preset_default(2, source_rate as f32);
        let in_latency = st.input_latency().max(0) as usize;
        let out_latency = st.output_latency().max(0) as usize;
        let start = (map.start() * f64::from(source_rate) / 1000.0).round() as u64;
        Self {
            st: Some(st),
            rate: f64::from(source_rate),
            map,
            in_latency,
            out_latency,
            fifo: [Vec::with_capacity(1 << 16), Vec::with_capacity(1 << 16)],
            fifo_abs: start,
            fed_abs: start,
            seeded: false,
            produced: 0,
            out: [vec![0.0; BLOCK], vec![0.0; BLOCK]],
            bypass_at: None,
            xfade_left: 0,
        }
    }

    /// Абсолютный кадр файла, который должен звучать на кадре выхода `t`.
    fn src_frame(&self, t: u64) -> f64 {
        self.map.src_at(t as f64 * 1000.0 / self.rate) * self.rate / 1000.0
    }

    fn fifo_end(&self) -> u64 {
        self.fifo_abs + self.fifo[0].len() as u64
    }

    fn at(&self, abs: u64, ch: usize) -> f32 {
        abs.checked_sub(self.fifo_abs)
            .and_then(|i| self.fifo[ch].get(i as usize))
            .copied()
            .unwrap_or(0.0)
    }

    /// Кадров выхода отдано (без выброшенной задержки).
    fn out_pos(&self) -> u64 {
        self.produced.saturating_sub(self.out_latency as u64)
    }

    fn emit(&mut self, n: usize, out: &mut Vec<f32>) {
        for i in 0..n {
            let internal = self.produced + i as u64;
            if internal < self.out_latency as u64 {
                continue;
            }
            let t = internal - self.out_latency as u64;
            let (mut l, mut r) = (self.out[0][i], self.out[1][i]);
            if self.xfade_left > 0 {
                // Переход на прямой путь: файл с того места, что звучит сейчас
                let w = 1.0 - self.xfade_left as f32 / BYPASS_XFADE as f32;
                let abs = self.src_frame(t).round() as u64;
                l += (self.at(abs, 0) - l) * w;
                r += (self.at(abs, 1) - r) * w;
                self.xfade_left -= 1;
                if self.xfade_left == 0 {
                    self.bypass_at = Some(abs + 1);
                }
            }
            out.push(l);
            out.push(r);
        }
        self.produced += n as u64;
    }

    /// Отдать вход (стерео, чередование) и забрать готовый растянутый выход.
    pub fn process(&mut self, input: &[f32], out: &mut Vec<f32>) {
        for pair in input.as_chunks::<2>().0 {
            self.fifo[0].push(pair[0]);
            self.fifo[1].push(pair[1]);
        }
        if let Some(next) = self.bypass_at {
            self.drain_direct(next, out);
            return;
        }
        let Some(mut st) = self.st.take() else {
            return;
        };
        if !self.seeded {
            if self.fifo[0].len() < self.in_latency {
                self.st = Some(st);
                return;
            }
            let n = self.in_latency;
            let rate = self.map.rate_at(0.0);
            // SAFETY: в каждом канале fifo не меньше n кадров
            unsafe {
                st.seek(
                    &[self.fifo[0].as_ptr(), self.fifo[1].as_ptr()],
                    n as i32,
                    rate,
                );
            }
            self.fed_abs = self.fifo_abs + n as u64;
            self.seeded = true;
        }
        loop {
            if self.xfade_left == 0 && self.bypass_at.is_none() && self.ready_to_bypass() {
                self.xfade_left = BYPASS_XFADE;
            }
            // Сколько файла должно уйти в растяжение к концу блока: место по карте плюс
            // входная задержка
            let target = (self.src_frame(self.produced + BLOCK as u64) + self.in_latency as f64)
                .round()
                .max(self.fed_abs as f64) as u64;
            let need = (target - self.fed_abs) as usize;
            if self.fed_abs + need as u64 > self.fifo_end() {
                break;
            }
            let off = (self.fed_abs - self.fifo_abs) as usize;
            // SAFETY: off + need ≤ длины fifo, выход — BLOCK кадров в каждом канале
            unsafe {
                st.process(
                    &[self.fifo[0][off..].as_ptr(), self.fifo[1][off..].as_ptr()],
                    need as i32,
                    &mut [self.out[0].as_mut_ptr(), self.out[1].as_mut_ptr()],
                    BLOCK as i32,
                );
            }
            self.fed_abs += need as u64;
            self.emit(BLOCK, out);
            if let Some(next) = self.bypass_at {
                // Растяжение больше не нужно
                self.drain_direct(next, out);
                return;
            }
        }
        self.trim();
        self.st = Some(st);
    }

    /// Скорость давно единица — можно уходить на прямой путь.
    fn ready_to_bypass(&self) -> bool {
        let t_ms = self.out_pos() as f64 * 1000.0 / self.rate;
        t_ms > self.map.settled_at() + BYPASS_AFTER_MS
    }

    fn drain_direct(&mut self, next: u64, out: &mut Vec<f32>) {
        let from = next.max(self.fifo_abs);
        for abs in from..self.fifo_end() {
            out.push(self.at(abs, 0));
            out.push(self.at(abs, 1));
        }
        self.bypass_at = Some(self.fifo_end().max(next));
        self.fifo_abs = self.fifo_end();
        self.fifo[0].clear();
        self.fifo[1].clear();
    }

    /// Держим в памяти только вход, ещё нужный растяжению или прямому пути.
    fn trim(&mut self) {
        let keep_from = self
            .fed_abs
            .saturating_sub((self.in_latency + self.out_latency) as u64 * 2 + 8192);
        let drop = keep_from.saturating_sub(self.fifo_abs) as usize;
        if drop > 1 << 15 {
            self.fifo[0].drain(..drop);
            self.fifo[1].drain(..drop);
            self.fifo_abs += drop as u64;
        }
    }

    /// Вход кончился: доиграть остаток и хвост задержки.
    pub fn finish(&mut self, out: &mut Vec<f32>) {
        if let Some(next) = self.bypass_at {
            self.drain_direct(next, out);
            return;
        }
        let Some(mut st) = self.st.take() else {
            return;
        };
        if !self.seeded {
            // Короче задержки: растягивать нечего
            for i in 0..self.fifo[0].len() {
                out.push(self.fifo[0][i]);
                out.push(self.fifo[1][i]);
            }
            return;
        }
        let left = self.fifo_end().saturating_sub(self.fed_abs) as usize;
        let rate = self.map.rate_at(self.out_pos() as f64 * 1000.0 / self.rate);
        let total = (left as f64 / rate).round() as usize;
        let (mut done, mut fed) = (0usize, 0usize);
        while done < total {
            let n = (total - done).min(BLOCK);
            let n_in = (((done + n) as f64 * rate).round() as usize).min(left) - fed;
            let o = (self.fed_abs - self.fifo_abs) as usize;
            // SAFETY: o + n_in ≤ длины fifo, выход — n ≤ BLOCK кадров
            unsafe {
                st.process(
                    &[self.fifo[0][o..].as_ptr(), self.fifo[1][o..].as_ptr()],
                    n_in as i32,
                    &mut [self.out[0].as_mut_ptr(), self.out[1].as_mut_ptr()],
                    n as i32,
                );
            }
            self.fed_abs += n_in as u64;
            fed += n_in;
            self.emit(n, out);
            done += n;
        }
        let mut tail = self.out_latency;
        while tail > 0 {
            let n = tail.min(BLOCK);
            // SAFETY: выход — n ≤ BLOCK кадров в каждом канале
            unsafe {
                st.flush(
                    &mut [self.out[0].as_mut_ptr(), self.out[1].as_mut_ptr()],
                    n as i32,
                );
            }
            self.emit(n, out);
            tail -= n;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: u32 = 48_000;

    fn clicks(frames: usize, period: usize, first: usize) -> Vec<f32> {
        let mut v = vec![0.0f32; frames * 2];
        let mut i = first;
        while i < frames {
            for k in 0..24 {
                if i + k < frames {
                    let s = if k % 2 == 0 { 0.9 } else { -0.9 };
                    v[(i + k) * 2] = s;
                    v[(i + k) * 2 + 1] = s;
                }
            }
            i += period;
        }
        v
    }

    fn onsets(v: &[f32]) -> Vec<usize> {
        let mut o: Vec<usize> = Vec::new();
        for (i, x) in v.iter().step_by(2).enumerate() {
            if x.abs() > 0.3 && o.last().is_none_or(|l| i - l > 4000) {
                o.push(i);
            }
        }
        o
    }

    fn run(map: TempoMap, input: &[f32]) -> Vec<f32> {
        let start = (map.start() * RATE as f64 / 1000.0).round() as usize;
        let mut s = Stretcher::new(RATE, map);
        let mut out = Vec::new();
        for chunk in input[start * 2..].chunks(4096 * 2) {
            s.process(chunk, &mut out);
        }
        s.finish(&mut out);
        out
    }

    #[test]
    fn clicks_stay_on_the_map() {
        // Щелчки каждые 0,5 с с 0,1 с; дека стартует с 0,1 с, первые 4 доли ускорены на 5 %,
        // затем за 4 доли скорость возвращается к единице
        let input = clicks(RATE as usize * 12, 24_000, 4800);
        let mut knots = vec![];
        let mut t = 0.0;
        for k in 0..=4 {
            knots.push((t, 100.0 + k as f64 * 500.0));
            if k < 4 {
                t += 500.0 / 1.05;
            }
        }
        let mut rate = 1.05;
        for k in 5..=8 {
            let next = 1.05 + (1.0 - 1.05) * (k - 4) as f64 / 4.0;
            t += 500.0 / ((rate + next) / 2.0);
            rate = next;
            knots.push((t, 100.0 + k as f64 * 500.0));
        }
        let map = TempoMap::new(knots);
        let out = run(map.clone(), &input);
        let on = onsets(&out);
        assert!(on.len() >= 18, "{} щелчков", on.len());
        for (k, o) in on.iter().enumerate().take(18) {
            let expect = map.out_at(100.0 + k as f64 * 500.0) * RATE as f64 / 1000.0;
            let err_ms = (*o as f64 - expect).abs() * 1000.0 / RATE as f64;
            assert!(
                err_ms < 3.0,
                "щелчок {k}: {o} вместо {expect:.0} ({err_ms:.2} мс)"
            );
        }
        // Длина: всё, что было после старта, по карте
        let expect_len = map.out_at(12_000.0) * RATE as f64 / 1000.0;
        assert!(((out.len() / 2) as f64 - expect_len).abs() < 2.0 * BLOCK as f64);
    }

    #[test]
    fn identity_passes_through_after_bypass() {
        let input = clicks(RATE as usize * 6, 24_000, 0);
        let out = run(TempoMap::identity(0.0), &input);
        assert!(((out.len() / 2) as i64 - (RATE as i64 * 6)).abs() < 8);
        // После перехода на прямой путь — точная копия входа
        let from = RATE as usize * 3;
        for i in from..RATE as usize * 6 - 10 {
            assert_eq!(out[i * 2], input[i * 2], "кадр {i}");
        }
    }
}
