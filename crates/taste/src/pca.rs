//! PCA с выбеливанием для векторов треков: случайная проекция с итерациями по степеням (Halko и др.),
//! затем точное разложение маленькой матрицы методом Якоби. Детерминированно при одном зерне.

use crate::rng::Rng;

/// Среднее и матрица сжатия `dim × k` (по строкам): выходы примерно единичной дисперсии.
pub struct Pca {
    pub mean: Vec<f32>,
    pub proj: Vec<f32>,
    pub k: usize,
}

/// `rows` строк ширины `dim`. Строк мало — компонент меньше.
pub fn fit(data: &[f32], rows: usize, dim: usize, k: usize, seed: u64) -> Pca {
    let k = k.min(dim).min(rows.saturating_sub(1)).max(1);
    let mut mean = vec![0.0f64; dim];
    for r in 0..rows {
        for (m, x) in mean.iter_mut().zip(&data[r * dim..(r + 1) * dim]) {
            *m += f64::from(*x);
        }
    }
    for m in &mut mean {
        *m /= rows.max(1) as f64;
    }
    let x: Vec<f64> = (0..rows * dim)
        .map(|i| f64::from(data[i]) - mean[i % dim])
        .collect();
    let l = (k + 10).min(dim).min(rows.max(1));
    let mut rng = Rng::new(seed);
    // Ω: dim × l → Y = X Ω: rows × l
    let omega: Vec<f64> = (0..dim * l).map(|_| rng.normal()).collect();
    let mut y = mul(&x, rows, dim, &omega, l);
    orthonormalize(&mut y, rows, l);
    for _ in 0..4 {
        let z = mul_t(&x, rows, dim, &y, l); // Xᵀ Y: dim × l
        let mut z = z;
        orthonormalize(&mut z, dim, l);
        y = mul(&x, rows, dim, &z, l);
        orthonormalize(&mut y, rows, l);
    }
    // B = Yᵀ X: l × dim; B Bᵀ = U Σ² Uᵀ; V = Bᵀ U / σ
    let b = mul_t(&y, rows, l, &x, dim); // l × dim
    let mut bbt = vec![0.0f64; l * l];
    for i in 0..l {
        for j in i..l {
            let s: f64 = (0..dim).map(|c| b[i * dim + c] * b[j * dim + c]).sum();
            bbt[i * l + j] = s;
            bbt[j * l + i] = s;
        }
    }
    let (vals, vecs) = jacobi(&mut bbt, l);
    let mut order: Vec<usize> = (0..l).collect();
    order.sort_by(|a, b| vals[*b].total_cmp(&vals[*a]));
    let mut proj = vec![0.0f32; dim * k];
    for (c, &e) in order.iter().take(k).enumerate() {
        let s2 = vals[e].max(0.0);
        let sigma = s2.sqrt();
        let var = s2 / (rows.saturating_sub(1).max(1)) as f64;
        let whiten = 1.0 / var.max(1e-6).sqrt();
        for d in 0..dim {
            let v: f64 = if sigma > 1e-12 {
                (0..l)
                    .map(|i| b[i * dim + d] * vecs[i * l + e])
                    .sum::<f64>()
                    / sigma
            } else {
                0.0
            };
            proj[d * k + c] = (v * whiten) as f32;
        }
    }
    Pca {
        mean: mean.iter().map(|m| *m as f32).collect(),
        proj,
        k,
    }
}

/// `a (n × m) · b (m × p)`.
fn mul(a: &[f64], n: usize, m: usize, b: &[f64], p: usize) -> Vec<f64> {
    let mut out = vec![0.0f64; n * p];
    for i in 0..n {
        let o = &mut out[i * p..(i + 1) * p];
        for k in 0..m {
            let x = a[i * m + k];
            if x == 0.0 {
                continue;
            }
            for (oj, bj) in o.iter_mut().zip(&b[k * p..(k + 1) * p]) {
                *oj += x * bj;
            }
        }
    }
    out
}

/// `aᵀ (m × n) · b (n × p)` при `a` — `n × m`.
fn mul_t(a: &[f64], n: usize, m: usize, b: &[f64], p: usize) -> Vec<f64> {
    let mut out = vec![0.0f64; m * p];
    for i in 0..n {
        let bi = &b[i * p..(i + 1) * p];
        for k in 0..m {
            let x = a[i * m + k];
            if x == 0.0 {
                continue;
            }
            for (oj, bj) in out[k * p..(k + 1) * p].iter_mut().zip(bi) {
                *oj += x * bj;
            }
        }
    }
    out
}

/// Ортонормировать столбцы `a` (`n × p`) — модифицированный Грам — Шмидт.
fn orthonormalize(a: &mut [f64], n: usize, p: usize) {
    for j in 0..p {
        for q in 0..j {
            let dot: f64 = (0..n).map(|i| a[i * p + j] * a[i * p + q]).sum();
            for i in 0..n {
                a[i * p + j] -= dot * a[i * p + q];
            }
        }
        let norm: f64 = (0..n).map(|i| a[i * p + j].powi(2)).sum::<f64>().sqrt();
        if norm > 1e-12 {
            for i in 0..n {
                a[i * p + j] /= norm;
            }
        } else {
            for i in 0..n {
                a[i * p + j] = 0.0;
            }
        }
    }
}

/// Собственные числа и векторы (по столбцам) симметричной `n × n` — циклический Якоби.
fn jacobi(a: &mut [f64], n: usize) -> (Vec<f64>, Vec<f64>) {
    let mut v = vec![0.0f64; n * n];
    for i in 0..n {
        v[i * n + i] = 1.0;
    }
    for _ in 0..100 {
        let off: f64 = (0..n)
            .flat_map(|i| (0..n).filter(move |j| *j != i).map(move |j| (i, j)))
            .map(|(i, j)| a[i * n + j].powi(2))
            .sum();
        if off < 1e-22 {
            break;
        }
        for p in 0..n {
            for q in p + 1..n {
                let apq = a[p * n + q];
                if apq.abs() < 1e-300 {
                    continue;
                }
                let theta = (a[q * n + q] - a[p * n + p]) / (2.0 * apq);
                let t = theta.signum() / (theta.abs() + (theta * theta + 1.0).sqrt());
                let t = if theta == 0.0 { 1.0 } else { t };
                let c = 1.0 / (t * t + 1.0).sqrt();
                let s = t * c;
                for k in 0..n {
                    let akp = a[k * n + p];
                    let akq = a[k * n + q];
                    a[k * n + p] = c * akp - s * akq;
                    a[k * n + q] = s * akp + c * akq;
                }
                for k in 0..n {
                    let apk = a[p * n + k];
                    let aqk = a[q * n + k];
                    a[p * n + k] = c * apk - s * aqk;
                    a[q * n + k] = s * apk + c * aqk;
                }
                for k in 0..n {
                    let vkp = v[k * n + p];
                    let vkq = v[k * n + q];
                    v[k * n + p] = c * vkp - s * vkq;
                    v[k * n + q] = s * vkp + c * vkq;
                }
            }
        }
    }
    ((0..n).map(|i| a[i * n + i]).collect(), v)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_main_directions_and_whitens() {
        // Три направления с дисперсиями 9, 4, 1 в пространстве 20, плюс малый шум
        let mut rng = Rng::new(7);
        let dim = 20;
        let rows = 400;
        let mut data = vec![0.0f32; rows * dim];
        for r in 0..rows {
            let (a, b, c) = (3.0 * rng.normal(), 2.0 * rng.normal(), rng.normal());
            for d in 0..dim {
                let noise = 0.01 * rng.normal();
                let v = match d {
                    0 => a,
                    5 => b,
                    9 => c,
                    _ => 0.0,
                };
                data[r * dim + d] = (v + noise + 1.0) as f32;
            }
        }
        let p = fit(&data, rows, dim, 3, 1);
        assert_eq!(p.k, 3);
        assert!((p.mean[0] - 1.0).abs() < 0.5);
        // Первая компонента — ось 0, вторая — ось 5
        assert!(p.proj[0].abs() > 0.2 && p.proj[1].abs() < 0.05);
        assert!(p.proj[5 * 3 + 1].abs() > 0.3);
        // Выбеливание: дисперсия выходов около 1
        let mut var = [0.0f64; 3];
        for r in 0..rows {
            for (c, acc) in var.iter_mut().enumerate() {
                let z: f32 = (0..dim)
                    .map(|d| (data[r * dim + d] - p.mean[d]) * p.proj[d * 3 + c])
                    .sum();
                *acc += f64::from(z * z) / rows as f64;
            }
        }
        for v in var {
            assert!((v - 1.0).abs() < 0.1, "{var:?}");
        }
    }
}
