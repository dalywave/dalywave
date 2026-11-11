//! Потоковый ресемплер поверх rubato::Fft: компенсирует задержку фильтра в начале и при сбросе
//! выдаёт ровно ceil(вход × отношение) кадров — длина трека после ресемплинга точная.
//!
//! Внутри цепочки «без пауз» ресемплер не сбрасывается между треками одной частоты:
//! фильтр видит непрерывный сигнал, и на стыке нет «ступеньки» от обрыва в ноль.

use audioadapter_buffers::direct::InterleavedSlice;
use rubato::{Fft, FixedSync, Resampler};

use crate::AudioError;

const CHANNELS: usize = 2;
const CHUNK: usize = 1024;

pub(crate) struct StreamResampler {
    inner: Fft<f32>,
    pub from: u32,
    pub to: u32,
    inbuf: Vec<f32>,
    in_fill: usize,
    outbuf: Vec<f32>,
    to_skip: usize,
    in_total: u64,
    out_total: u64,
}

impl StreamResampler {
    pub fn new(from: u32, to: u32) -> Result<Self, AudioError> {
        let inner = Fft::<f32>::new(
            from as usize,
            to as usize,
            CHUNK,
            CHANNELS,
            FixedSync::Input,
        )
        .map_err(|e| AudioError::Decode(format!("resampler: {e}")))?;
        let in_max = inner.input_frames_max();
        let out_max = inner.output_frames_max();
        let to_skip = inner.output_delay();
        Ok(Self {
            inner,
            from,
            to,
            inbuf: vec![0.0; in_max * CHANNELS],
            in_fill: 0,
            outbuf: vec![0.0; out_max * CHANNELS],
            to_skip,
            in_total: 0,
            out_total: 0,
        })
    }

    /// Сколько выходных кадров соответствует `frames` входных (для оценки длины трека).
    pub fn scale(&self, frames: u64) -> u64 {
        (frames as u128 * self.to as u128).div_ceil(self.from as u128) as u64
    }

    fn run_chunk(&mut self, out: &mut Vec<f32>) -> Result<(), AudioError> {
        let need = self.inner.input_frames_next();
        let out_cap = self.outbuf.len() / CHANNELS;
        let input = InterleavedSlice::new(&self.inbuf[..], CHANNELS, need)
            .map_err(|e| AudioError::Decode(format!("resampler in: {e}")))?;
        let mut output = InterleavedSlice::new_mut(&mut self.outbuf[..], CHANNELS, out_cap)
            .map_err(|e| AudioError::Decode(format!("resampler out: {e}")))?;
        let (_, written) = self
            .inner
            .process_into_buffer(&input, &mut output, None)
            .map_err(|e| AudioError::Decode(format!("resample: {e}")))?;
        let skip = self.to_skip.min(written);
        self.to_skip -= skip;
        let produced = &self.outbuf[skip * CHANNELS..written * CHANNELS];
        out.extend_from_slice(produced);
        self.out_total += (written - skip) as u64;
        self.in_fill = 0;
        Ok(())
    }

    /// Принимает стерео-кадры, дописывает в `out` всё, что уже можно посчитать.
    pub fn process(&mut self, mut input: &[f32], out: &mut Vec<f32>) -> Result<(), AudioError> {
        self.in_total += (input.len() / CHANNELS) as u64;
        while !input.is_empty() {
            let need = self.inner.input_frames_next();
            let take = ((need - self.in_fill) * CHANNELS).min(input.len());
            let at = self.in_fill * CHANNELS;
            self.inbuf[at..at + take].copy_from_slice(&input[..take]);
            self.in_fill += take / CHANNELS;
            input = &input[take..];
            if self.in_fill == need {
                self.run_chunk(out)?;
            }
        }
        Ok(())
    }

    /// Конец потока: досчитывает хвост (вход дополняется нулями) и обрезает выход до точной длины.
    pub fn flush(&mut self, out: &mut Vec<f32>) -> Result<(), AudioError> {
        let target = self.scale(self.in_total);
        let start_len = out.len();
        let start_total = self.out_total;
        let mut guard = 0;
        while self.out_total < target && guard < 64 {
            let need = self.inner.input_frames_next();
            let at = self.in_fill * CHANNELS;
            self.inbuf[at..need * CHANNELS].fill(0.0);
            self.in_fill = need;
            self.run_chunk(out)?;
            guard += 1;
        }
        let want = (target - start_total) as usize;
        out.truncate(start_len + want * CHANNELS);
        self.out_total = target;
        Ok(())
    }

    pub fn input_frames(&self) -> u64 {
        self.in_total
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sine(rate: u32, freq: f64, frames: usize) -> Vec<f32> {
        let mut v = Vec::with_capacity(frames * 2);
        for i in 0..frames {
            let s = (2.0 * std::f64::consts::PI * freq * i as f64 / rate as f64).sin() as f32 * 0.5;
            v.push(s);
            v.push(-s);
        }
        v
    }

    #[test]
    fn exact_length_and_no_delay() {
        for (from, to) in [(44100u32, 48000u32), (96000, 48000), (48000, 44100)] {
            let frames = 44_101usize;
            let input = sine(from, 997.0, frames);
            let mut r = StreamResampler::new(from, to).unwrap();
            let mut out = Vec::new();
            for chunk in input.chunks(2 * 777) {
                r.process(chunk, &mut out).unwrap();
            }
            r.flush(&mut out).unwrap();
            let expect = (frames as u64 * to as u64).div_ceil(from as u64) as usize;
            assert_eq!(out.len() / 2, expect, "{from}->{to}");
            // Без задержки: в середине выход совпадает с синусом нужной частоты в той же фазе.
            let mut err = 0f64;
            for i in 2000..(expect - 2000) {
                let t = i as f64 / to as f64;
                let want = (2.0 * std::f64::consts::PI * 997.0 * t).sin() * 0.5;
                err = err.max((out[i * 2] as f64 - want).abs());
                assert!((out[i * 2] + out[i * 2 + 1]).abs() < 1e-4);
            }
            assert!(err < 2e-3, "{from}->{to}: max error {err}");
        }
    }
}
