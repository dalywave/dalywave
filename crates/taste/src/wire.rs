//! Как набор и веса ездят по сети: JSON с массивами в base64 (векторы — float16, как в базе).
//!
//! В пакете только то, что нужно сети: векторы звучания, числа, метки, веса строк и номера сессий
//! без смысла. Ни треков, ни артистов, ни времени, ни слушателей. Отложенной выборки в пакете нет:
//! её проверяет сервер.

use base64::{Engine, engine::general_purpose::STANDARD};
use serde::{Deserialize, Serialize};

use crate::{
    f16,
    net::{Layer, Params},
    train::{Spec, TrainSet},
};

/// Версия формата пакета и весов.
pub const FORMAT: u32 = 1;

fn enc_f32(v: &[f32]) -> String {
    STANDARD.encode(v.iter().flat_map(|x| x.to_le_bytes()).collect::<Vec<u8>>())
}

fn enc_f16(v: &[f32]) -> String {
    STANDARD.encode(
        v.iter()
            .flat_map(|x| f16::to_bits(*x).to_le_bytes())
            .collect::<Vec<u8>>(),
    )
}

fn enc_u32(v: &[u32]) -> String {
    STANDARD.encode(v.iter().flat_map(|x| x.to_le_bytes()).collect::<Vec<u8>>())
}

fn bytes(s: &str, width: usize, count: usize, what: &str) -> Result<Vec<u8>, String> {
    let b = STANDARD
        .decode(s)
        .map_err(|e| format!("{what}: не base64 ({e})"))?;
    if b.len() != width * count {
        return Err(format!("{what}: {} байт вместо {}", b.len(), width * count));
    }
    Ok(b)
}

fn dec_f32(s: &str, count: usize, what: &str) -> Result<Vec<f32>, String> {
    Ok(bytes(s, 4, count, what)?
        .as_chunks::<4>()
        .0
        .iter()
        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect())
}

fn dec_f16(s: &str, count: usize, what: &str) -> Result<Vec<f32>, String> {
    Ok(bytes(s, 2, count, what)?
        .as_chunks::<2>()
        .0
        .iter()
        .map(|c| f16::from_bits(u16::from_le_bytes([c[0], c[1]])))
        .collect())
}

fn dec_u32(s: &str, count: usize, what: &str) -> Result<Vec<u32>, String> {
    Ok(bytes(s, 4, count, what)?
        .as_chunks::<4>()
        .0
        .iter()
        .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect())
}

/// Набор для обучения на компьютере.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Package {
    pub format: u32,
    pub spec: Spec,
    pub rows: usize,
    pub vector_rows: usize,
    pub sound_dim: usize,
    pub semantic_dim: usize,
    pub scalar_dim: usize,
    /// Названия чисел по порядку — для журнала и проверки, что сервер и компьютер говорят об одном.
    pub feature_names: Vec<String>,
    /// float16, `vector_rows × (sound + semantic)`.
    pub vectors: String,
    /// u32, `rows`.
    pub row_vector: String,
    /// f32, `rows × scalar_dim`.
    pub scalars: String,
    /// f32, `rows × 3`.
    pub labels: String,
    /// f32, `rows`.
    pub weights: String,
    /// u32, `rows`.
    pub groups: String,
}

impl Package {
    pub fn new(spec: Spec, data: &TrainSet, feature_names: &[&str]) -> Self {
        Self {
            format: FORMAT,
            spec,
            rows: data.rows(),
            vector_rows: data.vector_rows(),
            sound_dim: data.sound_dim,
            semantic_dim: data.semantic_dim,
            scalar_dim: data.scalar_dim,
            feature_names: feature_names.iter().map(|s| (*s).to_string()).collect(),
            vectors: enc_f16(&data.vectors),
            row_vector: enc_u32(&data.row_vector),
            scalars: enc_f32(&data.scalars),
            labels: enc_f32(&data.labels),
            weights: enc_f32(&data.weights),
            groups: enc_u32(&data.groups),
        }
    }

    /// Набор из пакета: размеры сверяются, числа проверяются.
    pub fn decode(&self) -> Result<(Spec, TrainSet), String> {
        if self.format != FORMAT {
            return Err(format!(
                "формат пакета {} — приложение знает {FORMAT}",
                self.format
            ));
        }
        if self.feature_names.len() != self.scalar_dim {
            return Err("названий признаков не столько, сколько чисел".into());
        }
        let width = self.sound_dim + self.semantic_dim;
        let set = TrainSet {
            sound_dim: self.sound_dim,
            semantic_dim: self.semantic_dim,
            scalar_dim: self.scalar_dim,
            vectors: dec_f16(&self.vectors, self.vector_rows * width, "векторы")?,
            row_vector: dec_u32(&self.row_vector, self.rows, "ссылки на векторы")?,
            scalars: dec_f32(&self.scalars, self.rows * self.scalar_dim, "числа")?,
            labels: dec_f32(&self.labels, self.rows * 3, "метки")?,
            weights: dec_f32(&self.weights, self.rows, "веса")?,
            groups: dec_u32(&self.groups, self.rows, "сессии")?,
        };
        set.validate()?;
        self.spec.validate()?;
        Ok((self.spec.clone(), set))
    }
}

/// Слой в весах для отправки.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WireLayer {
    pub inputs: usize,
    pub outputs: usize,
    pub w: String,
    pub b: String,
}

/// Веса обученной сети — то, что компьютер отправляет серверу.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WireParams {
    pub format: u32,
    pub sound_dim: usize,
    pub semantic_dim: usize,
    pub scalar_dim: usize,
    pub sound_k: usize,
    pub semantic_k: usize,
    pub sound_mean: String,
    pub sound_proj: String,
    pub semantic_mean: String,
    pub semantic_proj: String,
    pub scalar_mean: String,
    pub scalar_scale: String,
    pub clip: f32,
    pub layers: Vec<WireLayer>,
}

impl WireParams {
    pub fn new(p: &Params) -> Self {
        Self {
            format: FORMAT,
            sound_dim: p.sound_dim,
            semantic_dim: p.semantic_dim,
            scalar_dim: p.scalar_dim,
            sound_k: p.sound_k,
            semantic_k: p.semantic_k,
            sound_mean: enc_f32(&p.sound_mean),
            sound_proj: enc_f32(&p.sound_proj),
            semantic_mean: enc_f32(&p.semantic_mean),
            semantic_proj: enc_f32(&p.semantic_proj),
            scalar_mean: enc_f32(&p.scalar_mean),
            scalar_scale: enc_f32(&p.scalar_scale),
            clip: p.clip,
            layers: p
                .layers
                .iter()
                .map(|l| WireLayer {
                    inputs: l.inputs,
                    outputs: l.outputs,
                    w: enc_f32(&l.w),
                    b: enc_f32(&l.b),
                })
                .collect(),
        }
    }

    /// Веса из присланного: размеры, конечность, пределы — [`Params::validate`].
    pub fn decode(&self) -> Result<Params, String> {
        if self.format != FORMAT {
            return Err(format!(
                "формат весов {} — сервер знает {FORMAT}",
                self.format
            ));
        }
        let lim = crate::net::MAX_K;
        if self.sound_k > lim || self.semantic_k > lim || self.layers.len() > crate::net::MAX_LAYERS
        {
            return Err("веса больше допустимого".into());
        }
        if self
            .layers
            .iter()
            .any(|l| l.inputs > 4096 || l.outputs > crate::net::MAX_WIDTH)
        {
            return Err("слой больше допустимого".into());
        }
        if self.sound_dim > 8192 || self.semantic_dim > 8192 || self.scalar_dim > 1024 {
            return Err("вход больше допустимого".into());
        }
        let p = Params {
            sound_dim: self.sound_dim,
            semantic_dim: self.semantic_dim,
            scalar_dim: self.scalar_dim,
            sound_mean: dec_f32(&self.sound_mean, self.sound_dim, "sound_mean")?,
            sound_proj: dec_f32(
                &self.sound_proj,
                self.sound_dim * self.sound_k,
                "sound_proj",
            )?,
            sound_k: self.sound_k,
            semantic_mean: dec_f32(&self.semantic_mean, self.semantic_dim, "semantic_mean")?,
            semantic_proj: dec_f32(
                &self.semantic_proj,
                self.semantic_dim * self.semantic_k,
                "semantic_proj",
            )?,
            semantic_k: self.semantic_k,
            scalar_mean: dec_f32(&self.scalar_mean, self.scalar_dim, "scalar_mean")?,
            scalar_scale: dec_f32(&self.scalar_scale, self.scalar_dim, "scalar_scale")?,
            clip: self.clip,
            layers: self
                .layers
                .iter()
                .enumerate()
                .map(|(i, l)| {
                    Ok(Layer {
                        inputs: l.inputs,
                        outputs: l.outputs,
                        w: dec_f32(&l.w, l.inputs * l.outputs, &format!("слой {i}: w"))?,
                        b: dec_f32(&l.b, l.outputs, &format!("слой {i}: b"))?,
                    })
                })
                .collect::<Result<_, String>>()?,
        };
        p.validate()?;
        Ok(p)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn package_and_params_round_trip() {
        let data = crate::train::tests::synthetic(200, 4);
        let names = ["a", "b", "c", "d"];
        let pkg = Package::new(Spec::default(), &data, &names);
        let json = serde_json::to_string(&pkg).unwrap();
        let back: Package = serde_json::from_str(&json).unwrap();
        let (spec, set) = back.decode().unwrap();
        assert_eq!(spec, Spec::default());
        assert_eq!(set.labels, data.labels);
        assert_eq!(set.row_vector, data.row_vector);
        // Векторы — float16: близко, но не точно
        assert!(
            set.vectors
                .iter()
                .zip(&data.vectors)
                .all(|(a, b)| (a - b).abs() < 2e-3 * b.abs().max(1.0))
        );

        let (p, _) = crate::train(
            &set,
            &Spec {
                max_epochs: 2,
                ..Default::default()
            },
        )
        .unwrap();
        let wire = WireParams::new(&p);
        let p2 = serde_json::from_str::<WireParams>(&serde_json::to_string(&wire).unwrap())
            .unwrap()
            .decode()
            .unwrap();
        assert_eq!(p, p2, "веса едут без потерь");

        // Порча размеров и чисел не проходит
        let mut bad = wire.clone();
        bad.layers[0].b = enc_f32(&[1.0]);
        assert!(bad.decode().is_err());
        let mut bad = wire.clone();
        bad.clip = f32::NAN;
        assert!(bad.decode().is_err());
        let mut bad = wire;
        bad.layers.pop();
        assert!(bad.decode().is_err(), "выходов не три");
        let mut pkg = pkg;
        pkg.rows += 1;
        assert!(pkg.decode().is_err());
    }
}
