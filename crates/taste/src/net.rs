//! Веса сети и её вывод. Тот же граф — в ONNX ([`crate::onnx::export`]).
//!
//! Вход строки: `[звучание sound_dim | «смысл» semantic_dim | числа scalar_dim]`.
//! `z = clip([(звучание − μ)·P_s, («смысл» − μ)·P_t, (числа − μ)·σ⁻¹], ±clip)`, дальше слои: ReLU на
//! скрытых, сигмоида на выходе (три головы).

use serde::{Deserialize, Serialize};

use crate::HEADS;

/// Полносвязный слой: `w` — `inputs × outputs` по строкам.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Layer {
    pub inputs: usize,
    pub outputs: usize,
    pub w: Vec<f32>,
    pub b: Vec<f32>,
}

/// Всё, что нужно для вывода: сжатие векторов, нормировка чисел, слои.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Params {
    pub sound_dim: usize,
    pub semantic_dim: usize,
    pub scalar_dim: usize,
    pub sound_mean: Vec<f32>,
    /// `sound_dim × sound_k` по строкам.
    pub sound_proj: Vec<f32>,
    pub sound_k: usize,
    pub semantic_mean: Vec<f32>,
    /// `semantic_dim × semantic_k` по строкам.
    pub semantic_proj: Vec<f32>,
    pub semantic_k: usize,
    pub scalar_mean: Vec<f32>,
    /// 1/σ; у постоянных при обучении признаков — 0: сеть их не видит.
    pub scalar_scale: Vec<f32>,
    /// Предел по модулю для входа первого слоя: выброс не уводит сеть в насыщение.
    pub clip: f32,
    pub layers: Vec<Layer>,
}

/// Пределы для присланных весов: сервер не доверяет компьютеру.
pub const MAX_LAYERS: usize = 5;
pub const MAX_WIDTH: usize = 1024;
pub const MAX_K: usize = 256;
pub const MAX_PARAMS: usize = 2_000_000;
/// Больше по модулю веса не бывают у нормально обученной сети.
const MAX_ABS: f32 = 1e4;

impl Params {
    /// Ширина входа сети.
    pub fn input_dim(&self) -> usize {
        self.sound_dim + self.semantic_dim + self.scalar_dim
    }

    /// Ширина входа первого слоя.
    pub fn reduced_dim(&self) -> usize {
        self.sound_k + self.semantic_k + self.scalar_dim
    }

    /// Обучаемых параметров (слои).
    pub fn trainable(&self) -> usize {
        self.layers.iter().map(|l| l.w.len() + l.b.len()).sum()
    }

    /// Согласованы ли размеры, конечны ли числа, не слишком ли сеть велика.
    pub fn validate(&self) -> Result<(), String> {
        let check = |name: &str, v: &[f32], n: usize| -> Result<(), String> {
            if v.len() != n {
                return Err(format!("{name}: {} чисел вместо {n}", v.len()));
            }
            if v.iter().any(|x| !x.is_finite() || x.abs() > MAX_ABS) {
                return Err(format!("{name}: числа вне допустимого"));
            }
            Ok(())
        };
        if self.sound_k == 0 || self.sound_k > MAX_K || self.semantic_k > MAX_K {
            return Err("сжатие векторов вне пределов".into());
        }
        if self.sound_dim == 0 || self.scalar_dim == 0 {
            return Err("пустой вход".into());
        }
        check("sound_mean", &self.sound_mean, self.sound_dim)?;
        check(
            "sound_proj",
            &self.sound_proj,
            self.sound_dim * self.sound_k,
        )?;
        check("semantic_mean", &self.semantic_mean, self.semantic_dim)?;
        check(
            "semantic_proj",
            &self.semantic_proj,
            self.semantic_dim * self.semantic_k,
        )?;
        check("scalar_mean", &self.scalar_mean, self.scalar_dim)?;
        check("scalar_scale", &self.scalar_scale, self.scalar_dim)?;
        if !(self.clip.is_finite() && self.clip > 0.0 && self.clip <= 100.0) {
            return Err("предел входа вне (0, 100]".into());
        }
        if self.layers.is_empty() || self.layers.len() > MAX_LAYERS {
            return Err(format!(
                "слоёв {} — нужно от 1 до {MAX_LAYERS}",
                self.layers.len()
            ));
        }
        let mut width = self.reduced_dim();
        for (i, l) in self.layers.iter().enumerate() {
            if l.inputs != width || l.outputs == 0 || l.outputs > MAX_WIDTH {
                return Err(format!("слой {i}: размеры не сходятся"));
            }
            check(&format!("слой {i}: w"), &l.w, l.inputs * l.outputs)?;
            check(&format!("слой {i}: b"), &l.b, l.outputs)?;
            width = l.outputs;
        }
        if width != HEADS.len() {
            return Err(format!("выходов {width} вместо {}", HEADS.len()));
        }
        if self.trainable() > MAX_PARAMS {
            return Err("сеть больше допустимого".into());
        }
        Ok(())
    }

    /// Вход первого слоя для `n` строк сырых признаков.
    pub fn reduce(&self, features: &[f32], n: usize) -> Vec<f32> {
        let d = self.input_dim();
        let r = self.reduced_dim();
        let mut z = vec![0.0f32; n * r];
        for i in 0..n {
            let row = &features[i * d..(i + 1) * d];
            let out = &mut z[i * r..(i + 1) * r];
            project(
                &row[..self.sound_dim],
                &self.sound_mean,
                &self.sound_proj,
                self.sound_k,
                &mut out[..self.sound_k],
            );
            project(
                &row[self.sound_dim..self.sound_dim + self.semantic_dim],
                &self.semantic_mean,
                &self.semantic_proj,
                self.semantic_k,
                &mut out[self.sound_k..self.sound_k + self.semantic_k],
            );
            let sc = &row[self.sound_dim + self.semantic_dim..];
            for (j, o) in out[self.sound_k + self.semantic_k..].iter_mut().enumerate() {
                *o = (sc[j] - self.scalar_mean[j]) * self.scalar_scale[j];
            }
            for x in out.iter_mut() {
                *x = x.clamp(-self.clip, self.clip);
            }
        }
        z
    }

    /// Логиты по слоям для готового входа первого слоя; `acts` — входы слоёв (для обучения).
    pub fn logits(&self, z: &[f32], n: usize, mut acts: Option<&mut Vec<Vec<f32>>>) -> Vec<f32> {
        let mut h = z.to_vec();
        let last = self.layers.len() - 1;
        for (i, l) in self.layers.iter().enumerate() {
            let mut out = dense(&h, n, l);
            if i < last {
                for x in &mut out {
                    *x = x.max(0.0);
                }
            }
            if let Some(a) = acts.as_deref_mut() {
                a.push(h);
            }
            h = out;
        }
        h
    }

    /// Вероятности `[skip, complete, like]` для `n` строк ширины [`Params::input_dim`].
    pub fn forward(&self, features: &[f32], n: usize) -> Vec<[f32; 3]> {
        assert_eq!(features.len(), n * self.input_dim(), "ширина признаков");
        let z = self.reduce(features, n);
        self.logits(&z, n, None)
            .as_chunks::<3>()
            .0
            .iter()
            .map(|c| [sigmoid(c[0]), sigmoid(c[1]), sigmoid(c[2])])
            .collect()
    }
}

fn project(x: &[f32], mean: &[f32], proj: &[f32], k: usize, out: &mut [f32]) {
    out.fill(0.0);
    for (j, (&xi, &m)) in x.iter().zip(mean).enumerate() {
        let c = xi - m;
        if c == 0.0 {
            continue;
        }
        let row = &proj[j * k..(j + 1) * k];
        for (o, p) in out.iter_mut().zip(row) {
            *o += c * p;
        }
    }
}

/// `h · W + b` для `n` строк.
pub(crate) fn dense(h: &[f32], n: usize, l: &Layer) -> Vec<f32> {
    let mut out = Vec::with_capacity(n * l.outputs);
    for i in 0..n {
        out.extend_from_slice(&l.b);
        let row = &h[i * l.inputs..(i + 1) * l.inputs];
        let o = &mut out[i * l.outputs..];
        for (k, &x) in row.iter().enumerate() {
            if x == 0.0 {
                continue;
            }
            let w = &l.w[k * l.outputs..(k + 1) * l.outputs];
            for (oj, wj) in o.iter_mut().zip(w) {
                *oj += x * wj;
            }
        }
    }
    out
}

pub fn sigmoid(x: f32) -> f32 {
    1.0 / (1.0 + (-x.clamp(-40.0, 40.0)).exp())
}
