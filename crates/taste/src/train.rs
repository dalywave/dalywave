//! Обучение: PCA векторов и нормировка чисел по обучающей части, затем полносвязная сеть — Adam,
//! взвешенная логистическая потеря по трём головам, L2 и ранняя остановка по самым поздним строкам
//! обучающей части (как будет в жизни: учимся на прошлом, проверяемся на следующем).

use serde::{Deserialize, Serialize};

use crate::{
    HEADS,
    net::{Layer, Params, sigmoid},
    pca,
    rng::Rng,
};

/// Строки для обучения по времени. Векторы треков — отдельно: трек слушают много раз.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TrainSet {
    pub sound_dim: usize,
    pub semantic_dim: usize,
    pub scalar_dim: usize,
    /// Строки `sound_dim + semantic_dim`.
    pub vectors: Vec<f32>,
    /// Строка набора → строка `vectors`.
    pub row_vector: Vec<u32>,
    /// `rows × scalar_dim`.
    pub scalars: Vec<f32>,
    /// `rows × 3`: 0/1 по головам [`HEADS`].
    pub labels: Vec<f32>,
    /// Вес строки: старая история весит меньше свежей.
    pub weights: Vec<f32>,
    /// Сессия строки (номер без смысла) — для метрик внутри сессии.
    pub groups: Vec<u32>,
}

impl TrainSet {
    pub fn rows(&self) -> usize {
        self.row_vector.len()
    }

    pub fn vector_rows(&self) -> usize {
        let w = self.sound_dim + self.semantic_dim;
        self.vectors.len().checked_div(w).unwrap_or(0)
    }

    /// Сырые признаки строки `i` (ширина `sound + semantic + scalar`) — в `out`.
    pub fn features_into(&self, i: usize, out: &mut Vec<f32>) {
        let w = self.sound_dim + self.semantic_dim;
        let v = self.row_vector[i] as usize;
        out.extend_from_slice(&self.vectors[v * w..(v + 1) * w]);
        out.extend_from_slice(&self.scalars[i * self.scalar_dim..(i + 1) * self.scalar_dim]);
    }

    /// Строки `range` подряд сырыми признаками.
    pub fn features(&self, range: std::ops::Range<usize>) -> Vec<f32> {
        let mut out = Vec::with_capacity(
            range.len() * (self.sound_dim + self.semantic_dim + self.scalar_dim),
        );
        for i in range {
            self.features_into(i, &mut out);
        }
        out
    }

    /// Размеры согласованы, числа конечны, ссылки на векторы верные.
    pub fn validate(&self) -> Result<(), String> {
        let n = self.rows();
        let w = self.sound_dim + self.semantic_dim;
        if self.sound_dim == 0 || self.scalar_dim == 0 {
            return Err("пустой вход".into());
        }
        if w == 0 || !self.vectors.len().is_multiple_of(w) {
            return Err("векторы не делятся на строки".into());
        }
        if self.scalars.len() != n * self.scalar_dim
            || self.labels.len() != n * HEADS.len()
            || self.weights.len() != n
            || self.groups.len() != n
        {
            return Err("число строк признаков, меток, весов и сессий не совпадает".into());
        }
        let vr = self.vector_rows();
        if self.row_vector.iter().any(|v| *v as usize >= vr) {
            return Err("ссылка на несуществующий вектор".into());
        }
        if self
            .vectors
            .iter()
            .chain(&self.scalars)
            .any(|x| !x.is_finite())
        {
            return Err("в признаках есть NaN или бесконечность".into());
        }
        if self.labels.iter().any(|y| *y != 0.0 && *y != 1.0) {
            return Err("метки — только 0 и 1".into());
        }
        if self
            .weights
            .iter()
            .any(|w| !w.is_finite() || *w < 0.0 || *w > 100.0)
        {
            return Err("вес строки вне 0…100".into());
        }
        Ok(())
    }

    /// Первые `n` строк (векторы — все).
    pub fn head(&self, n: usize) -> Self {
        let n = n.min(self.rows());
        Self {
            sound_dim: self.sound_dim,
            semantic_dim: self.semantic_dim,
            scalar_dim: self.scalar_dim,
            vectors: self.vectors.clone(),
            row_vector: self.row_vector[..n].to_vec(),
            scalars: self.scalars[..n * self.scalar_dim].to_vec(),
            labels: self.labels[..n * 3].to_vec(),
            weights: self.weights[..n].to_vec(),
            groups: self.groups[..n].to_vec(),
        }
    }
}

/// Описание обучения: что и как учить. Задаёт сервер; компьютер исполняет как есть.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Spec {
    /// Скрытые слои.
    pub hidden: Vec<usize>,
    /// Сколько компонент оставить от звучания и «смысла».
    pub sound_k: usize,
    pub semantic_k: usize,
    pub max_epochs: usize,
    /// Столько эпох без улучшения на проверочной части — стоп.
    pub patience: usize,
    pub batch: usize,
    pub lr: f32,
    pub weight_decay: f32,
    /// Доля самых поздних строк обучающей части для ранней остановки.
    pub val_share: f32,
    /// Предел входа первого слоя.
    pub clip: f32,
    /// Вес каждой головы в потере.
    pub head_weights: [f32; 3],
    pub seed: u64,
}

impl Default for Spec {
    fn default() -> Self {
        Self {
            hidden: vec![32],
            sound_k: 16,
            semantic_k: 8,
            max_epochs: 80,
            patience: 8,
            batch: 128,
            lr: 2e-3,
            weight_decay: 1e-2,
            val_share: 0.15,
            clip: 5.0,
            head_weights: [1.0, 1.0, 1.0],
            seed: 0,
        }
    }
}

impl Spec {
    /// Описание в разумных пределах: от сервера можно получить что угодно, но учить вечно не будем.
    pub fn validate(&self) -> Result<(), String> {
        if self.hidden.len() >= crate::net::MAX_LAYERS
            || self
                .hidden
                .iter()
                .any(|h| *h == 0 || *h > crate::net::MAX_WIDTH)
        {
            return Err("скрытые слои вне пределов".into());
        }
        if self.sound_k == 0
            || self.sound_k > crate::net::MAX_K
            || self.semantic_k > crate::net::MAX_K
        {
            return Err("сжатие векторов вне пределов".into());
        }
        if self.max_epochs == 0 || self.max_epochs > 500 || self.batch == 0 || self.batch > 8192 {
            return Err("эпохи или пакет вне пределов".into());
        }
        if !(self.lr > 0.0 && self.lr < 1.0) || !(0.0..1.0).contains(&self.weight_decay) {
            return Err("шаг или L2 вне пределов".into());
        }
        if !(0.0..0.5).contains(&self.val_share) || !(self.clip > 0.0 && self.clip <= 100.0) {
            return Err("доля проверки или предел входа вне пределов".into());
        }
        if self.head_weights.iter().any(|w| !(0.0..=10.0).contains(w)) {
            return Err("веса голов вне 0…10".into());
        }
        Ok(())
    }
}

/// Как шло обучение: эпохи, лучшая эпоха, кривая потерь.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrainReport {
    pub rows: usize,
    pub fit_rows: usize,
    pub val_rows: usize,
    pub epochs: usize,
    pub best_epoch: usize,
    /// Потеря на обучающей и проверочной части после каждой эпохи.
    pub fit_loss: Vec<f32>,
    pub val_loss: Vec<f32>,
    pub params: usize,
    pub took_ms: u64,
}

/// Средняя взвешенная логистическая потеря по головам.
pub fn loss(params: &Params, z: &[f32], labels: &[f32], weights: &[f32], head_w: &[f32; 3]) -> f32 {
    let n = weights.len();
    if n == 0 {
        return 0.0;
    }
    let logits = params.logits(z, n, None);
    let mut total = 0.0f64;
    let mut wsum = 0.0f64;
    for i in 0..n {
        for h in 0..3 {
            let x = logits[i * 3 + h];
            let y = labels[i * 3 + h];
            // softplus(x) − y·x, устойчиво
            let l = x.max(0.0) - x * y + (-x.abs()).exp().ln_1p();
            let w = f64::from(weights[i] * head_w[h]);
            total += w * f64::from(l);
            wsum += w;
        }
    }
    (total / wsum.max(1e-12)) as f32
}

/// Обучить сеть на `data` (все строки — обучающие; отложенная проверка у сервера своя).
pub fn train(data: &TrainSet, spec: &Spec) -> Result<(Params, TrainReport), String> {
    let started = std::time::Instant::now();
    data.validate()?;
    spec.validate()?;
    let n = data.rows();
    if n < 20 {
        return Err(format!("мало строк для обучения: {n}"));
    }
    let n_fit = ((n as f32 * (1.0 - spec.val_share)).round() as usize).clamp(1, n);
    let (sd, md, cd) = (data.sound_dim, data.semantic_dim, data.scalar_dim);
    let w = sd + md;

    // Сжатие — по трекам обучающей части (каждый трек один раз)
    let mut used = vec![false; data.vector_rows()];
    for v in &data.row_vector[..n_fit] {
        used[*v as usize] = true;
    }
    let tracks: Vec<usize> = (0..used.len()).filter(|i| used[*i]).collect();
    let mut sound = Vec::with_capacity(tracks.len() * sd);
    let mut semantic = Vec::with_capacity(tracks.len() * md);
    for t in &tracks {
        sound.extend_from_slice(&data.vectors[t * w..t * w + sd]);
        semantic.extend_from_slice(&data.vectors[t * w + sd..(t + 1) * w]);
    }
    let ps = pca::fit(&sound, tracks.len(), sd, spec.sound_k, spec.seed ^ 1);
    let (pt_mean, pt_proj, pt_k) = if md > 0 && spec.semantic_k > 0 {
        let p = pca::fit(&semantic, tracks.len(), md, spec.semantic_k, spec.seed ^ 2);
        (p.mean, p.proj, p.k)
    } else {
        (vec![0.0; md], Vec::new(), 0)
    };

    // Нормировка чисел — по обучающей части. Постоянное при обучении сеть не видит (σ⁻¹ = 0):
    // иначе признак, который при подборе другой (например, «в волне»), сдвинет вход на тысячи σ
    let mut mean = vec![0.0f64; cd];
    let mut sq = vec![0.0f64; cd];
    for i in 0..n_fit {
        for j in 0..cd {
            let x = f64::from(data.scalars[i * cd + j]);
            mean[j] += x;
            sq[j] += x * x;
        }
    }
    let mut scalar_mean = vec![0.0f32; cd];
    let mut scalar_scale = vec![0.0f32; cd];
    for j in 0..cd {
        let m = mean[j] / n_fit as f64;
        let var = (sq[j] / n_fit as f64 - m * m).max(0.0);
        scalar_mean[j] = m as f32;
        scalar_scale[j] = if var.sqrt() < 0.01 {
            0.0
        } else {
            (1.0 / var.sqrt()) as f32
        };
    }

    let mut rng = Rng::new(spec.seed);
    let reduced = ps.k + pt_k + cd;
    let mut sizes = vec![reduced];
    sizes.extend(&spec.hidden);
    sizes.push(HEADS.len());
    let layers = sizes
        .windows(2)
        .map(|s| {
            let scale = (2.0 / s[0] as f64).sqrt();
            Layer {
                inputs: s[0],
                outputs: s[1],
                w: (0..s[0] * s[1])
                    .map(|_| (rng.normal() * scale) as f32)
                    .collect(),
                b: vec![0.0; s[1]],
            }
        })
        .collect();
    let mut p = Params {
        sound_dim: sd,
        semantic_dim: md,
        scalar_dim: cd,
        sound_mean: ps.mean,
        sound_proj: ps.proj,
        sound_k: ps.k,
        semantic_mean: pt_mean,
        semantic_proj: pt_proj,
        semantic_k: pt_k,
        scalar_mean,
        scalar_scale,
        clip: spec.clip,
        layers,
    };
    // Голова лайка начинает с доли лайков, а не с 0,5: меньше эпох уходит на смещение
    if let Some(last) = p.layers.last_mut() {
        for h in 0..3 {
            let (mut pos, mut tot) = (0.0f64, 0.0f64);
            for i in 0..n_fit {
                pos += f64::from(data.weights[i] * data.labels[i * 3 + h]);
                tot += f64::from(data.weights[i]);
            }
            let rate = ((pos + 0.5) / (tot + 1.0)).clamp(1e-3, 1.0 - 1e-3);
            last.b[h] = (rate / (1.0 - rate)).ln() as f32;
        }
    }

    let z_all = p.reduce(&data.features(0..n), n);
    let z_fit = &z_all[..n_fit * reduced];
    let z_val = &z_all[n_fit * reduced..];
    let (y_fit, y_val) = data.labels.split_at(n_fit * 3);
    let (w_fit, w_val) = data.weights.split_at(n_fit);

    let mut adam = Adam::new(&p);
    let mut best = (f32::INFINITY, p.layers.clone(), 0usize);
    let mut stale = 0;
    let mut report = TrainReport {
        rows: n,
        fit_rows: n_fit,
        val_rows: n - n_fit,
        params: p.trainable(),
        ..Default::default()
    };
    let mut order: Vec<usize> = (0..n_fit).collect();
    let mut batch_z = Vec::new();
    let mut batch_y = Vec::new();
    let mut batch_w = Vec::new();
    for epoch in 1..=spec.max_epochs {
        report.epochs = epoch;
        rng.shuffle(&mut order);
        for chunk in order.chunks(spec.batch) {
            batch_z.clear();
            batch_y.clear();
            batch_w.clear();
            for &i in chunk {
                batch_z.extend_from_slice(&z_fit[i * reduced..(i + 1) * reduced]);
                batch_y.extend_from_slice(&y_fit[i * 3..i * 3 + 3]);
                batch_w.push(w_fit[i]);
            }
            let grads = gradients(
                &p,
                &batch_z,
                &batch_y,
                &batch_w,
                &spec.head_weights,
                spec.weight_decay,
            );
            adam.step(&mut p, &grads, spec.lr);
        }
        let fit = loss(&p, z_fit, y_fit, w_fit, &spec.head_weights);
        let val = if w_val.is_empty() {
            fit
        } else {
            loss(&p, z_val, y_val, w_val, &spec.head_weights)
        };
        report.fit_loss.push(fit);
        report.val_loss.push(val);
        if !val.is_finite() {
            break;
        }
        if val < best.0 - 1e-5 {
            best = (val, p.layers.clone(), epoch);
            stale = 0;
        } else {
            stale += 1;
            if stale >= spec.patience {
                break;
            }
        }
    }
    p.layers = best.1;
    report.best_epoch = best.2;
    report.took_ms = started.elapsed().as_millis() as u64;
    p.validate()?;
    Ok((p, report))
}

struct Grads {
    w: Vec<Vec<f32>>,
    b: Vec<Vec<f32>>,
}

fn gradients(p: &Params, z: &[f32], y: &[f32], w: &[f32], head_w: &[f32; 3], decay: f32) -> Grads {
    let n = w.len();
    let mut acts = Vec::with_capacity(p.layers.len());
    let logits = p.logits(z, n, Some(&mut acts));
    let wsum: f32 = w
        .iter()
        .map(|x| x * head_w.iter().sum::<f32>())
        .sum::<f32>()
        .max(1e-12);
    // dL/dлогит
    let mut g: Vec<f32> = (0..n * 3)
        .map(|k| {
            let (i, h) = (k / 3, k % 3);
            (sigmoid(logits[k]) - y[k]) * w[i] * head_w[h] / wsum
        })
        .collect();
    let layers = p.layers.len();
    let mut gw = vec![Vec::new(); layers];
    let mut gb = vec![Vec::new(); layers];
    for li in (0..layers).rev() {
        let l = &p.layers[li];
        let a = &acts[li];
        let mut dw = vec![0.0f32; l.inputs * l.outputs];
        let mut db = vec![0.0f32; l.outputs];
        for i in 0..n {
            let gi = &g[i * l.outputs..(i + 1) * l.outputs];
            for (d, x) in db.iter_mut().zip(gi) {
                *d += x;
            }
            let ai = &a[i * l.inputs..(i + 1) * l.inputs];
            for (k, &x) in ai.iter().enumerate() {
                if x == 0.0 {
                    continue;
                }
                for (d, gj) in dw[k * l.outputs..(k + 1) * l.outputs].iter_mut().zip(gi) {
                    *d += x * gj;
                }
            }
        }
        for (d, wv) in dw.iter_mut().zip(&l.w) {
            *d += decay * wv;
        }
        if li > 0 {
            // Назад через слой и ReLU предыдущего (его выход — вход этого слоя)
            let mut prev = vec![0.0f32; n * l.inputs];
            for i in 0..n {
                let gi = &g[i * l.outputs..(i + 1) * l.outputs];
                let ai = &a[i * l.inputs..(i + 1) * l.inputs];
                for k in 0..l.inputs {
                    if ai[k] <= 0.0 {
                        continue;
                    }
                    let wr = &l.w[k * l.outputs..(k + 1) * l.outputs];
                    prev[i * l.inputs + k] = wr.iter().zip(gi).map(|(a, b)| a * b).sum();
                }
            }
            g = prev;
        }
        gw[li] = dw;
        gb[li] = db;
    }
    Grads { w: gw, b: gb }
}

struct Adam {
    t: i32,
    m: Grads,
    v: Grads,
}

impl Adam {
    fn new(p: &Params) -> Self {
        let zeros = || Grads {
            w: p.layers.iter().map(|l| vec![0.0; l.w.len()]).collect(),
            b: p.layers.iter().map(|l| vec![0.0; l.b.len()]).collect(),
        };
        Self {
            t: 0,
            m: zeros(),
            v: zeros(),
        }
    }

    fn step(&mut self, p: &mut Params, g: &Grads, lr: f32) {
        const B1: f32 = 0.9;
        const B2: f32 = 0.999;
        const EPS: f32 = 1e-8;
        self.t += 1;
        let c1 = 1.0 - B1.powi(self.t);
        let c2 = 1.0 - B2.powi(self.t);
        for (li, l) in p.layers.iter_mut().enumerate() {
            for (params, grads, m, v) in [
                (&mut l.w, &g.w[li], &mut self.m.w[li], &mut self.v.w[li]),
                (&mut l.b, &g.b[li], &mut self.m.b[li], &mut self.v.b[li]),
            ] {
                for j in 0..params.len() {
                    m[j] = B1 * m[j] + (1.0 - B1) * grads[j];
                    v[j] = B2 * v[j] + (1.0 - B2) * grads[j] * grads[j];
                    params[j] -= lr * (m[j] / c1) / ((v[j] / c2).sqrt() + EPS);
                }
            }
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// Синтетика: звучание на оси 0 — любимое (дослушивают), на оси 1 — пропускают; число «знаком» — тоже.
    pub(crate) fn synthetic(rows: usize, seed: u64) -> TrainSet {
        let mut rng = Rng::new(seed);
        let (sd, md, cd) = (24, 8, 4);
        let tracks = 60;
        let mut vectors = Vec::new();
        let mut loved = Vec::new();
        for t in 0..tracks {
            let good = t % 2 == 0;
            loved.push(good);
            for d in 0..sd + md {
                let base = if (d == 0 && good) || (d == 1 && !good) {
                    1.0
                } else {
                    0.0
                };
                vectors.push((base + 0.3 * rng.normal()) as f32);
            }
        }
        let mut set = TrainSet {
            sound_dim: sd,
            semantic_dim: md,
            scalar_dim: cd,
            vectors,
            ..Default::default()
        };
        for i in 0..rows {
            let t = rng.below(tracks);
            let p_skip = if loved[t] { 0.15 } else { 0.75 };
            let skip = rng.uniform() < p_skip;
            let like = !skip && loved[t] && rng.uniform() < 0.3;
            set.row_vector.push(t as u32);
            set.scalars.extend([
                rng.uniform() as f32,
                0.5,
                if loved[t] { 1.0 } else { 0.0 },
                rng.normal() as f32,
            ]);
            set.labels
                .extend([skip as u8 as f32, (!skip) as u8 as f32, like as u8 as f32]);
            set.weights.push(1.0);
            set.groups.push((i / 10) as u32);
        }
        set
    }

    #[test]
    fn learns_and_is_deterministic() {
        let data = synthetic(1200, 3);
        let spec = Spec {
            seed: 5,
            max_epochs: 40,
            ..Default::default()
        };
        let (p, report) = train(&data, &spec).unwrap();
        assert!(report.best_epoch >= 1);
        assert!(report.val_loss[report.best_epoch - 1] < report.val_loss[0] + 1e-6);
        // Постоянный признак сеть не видит
        assert_eq!(p.scalar_scale[1], 0.0);
        let probs = p.forward(&data.features(0..data.rows()), data.rows());
        let skip_loved: f32 = (0..data.rows())
            .filter(|i| data.scalars[i * 4 + 2] == 1.0)
            .map(|i| probs[i][0])
            .sum::<f32>();
        let n_loved = (0..data.rows())
            .filter(|i| data.scalars[i * 4 + 2] == 1.0)
            .count() as f32;
        let skip_other: f32 = (0..data.rows())
            .filter(|i| data.scalars[i * 4 + 2] == 0.0)
            .map(|i| probs[i][0])
            .sum::<f32>();
        let n_other = data.rows() as f32 - n_loved;
        assert!(
            skip_loved / n_loved < 0.35 && skip_other / n_other > 0.55,
            "{} {}",
            skip_loved / n_loved,
            skip_other / n_other
        );
        // То же зерно — те же веса
        let (again, _) = train(&data, &spec).unwrap();
        assert_eq!(p, again);
    }

    #[test]
    fn rejects_bad_input() {
        let mut data = synthetic(100, 1);
        data.weights[3] = f32::NAN;
        assert!(train(&data, &Spec::default()).is_err());
        let data = synthetic(100, 1);
        let spec = Spec {
            hidden: vec![0],
            ..Default::default()
        };
        assert!(train(&data, &spec).is_err());
        assert!(train(&synthetic(10, 1), &Spec::default()).is_err());
    }
}
