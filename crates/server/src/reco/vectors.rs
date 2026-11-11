//! Векторы и их арифметика: разбор float16 из ответа ml, нормировка, косинус, взвешенное среднее,
//! k-means по звучанию для граней вкуса, совместимость соседних треков по Камелоту и темпу.
//! Всё — чистые функции: их проверяют модульные тесты, а база хранит векторы в `halfvec`.

use base64::{Engine, engine::general_purpose::STANDARD};

/// Discogs-EffNet — звучание.
pub const SOUND_DIM: usize = 1280;
/// CLAP — общее пространство звука и текста.
pub const SEMANTIC_DIM: usize = 512;

/// float16 (IEEE 754, half) → float32 без потерь.
pub fn f16_to_f32(bits: u16) -> f32 {
    let sign = if bits & 0x8000 != 0 { -1.0 } else { 1.0 };
    let exp = (bits >> 10) & 0x1f;
    let frac = f32::from(bits & 0x3ff);
    match exp {
        // Субнормальные: frac × 2⁻²⁴
        0 => sign * frac * 2f32.powi(-24),
        0x1f if frac == 0.0 => sign * f32::INFINITY,
        0x1f => f32::NAN,
        _ => sign * (1.0 + frac / 1024.0) * 2f32.powi(i32::from(exp) - 15),
    }
}

/// Вектор из base64 от float16 little-endian. Ошибка — неверная длина или нечисловые значения.
pub fn decode_f16_base64(b64: &str, dim: usize) -> Result<Vec<f32>, String> {
    let bytes = STANDARD
        .decode(b64.trim())
        .map_err(|e| format!("вектор не в base64: {e}"))?;
    if bytes.len() != dim * 2 {
        return Err(format!(
            "в векторе {} чисел, а ждали {dim}",
            bytes.len() / 2
        ));
    }
    let v: Vec<f32> = bytes
        .as_chunks::<2>()
        .0
        .iter()
        .map(|b| f16_to_f32(u16::from_le_bytes(*b)))
        .collect();
    if v.iter().any(|x| !x.is_finite()) {
        return Err("в векторе есть бесконечность или NaN".into());
    }
    Ok(v)
}

pub fn dot(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

pub fn norm(v: &[f32]) -> f32 {
    dot(v, v).sqrt()
}

/// Длина 1; нулевой вектор остаётся нулевым.
pub fn normalize(v: &mut [f32]) {
    let n = norm(v);
    if n > 1e-12 {
        v.iter_mut().for_each(|x| *x /= n);
    }
}

pub fn normalized(mut v: Vec<f32>) -> Vec<f32> {
    normalize(&mut v);
    v
}

/// Косинусное сходство, −1..1; 0 — если один из векторов нулевой.
pub fn cosine(a: &[f32], b: &[f32]) -> f32 {
    let (na, nb) = (norm(a), norm(b));
    if na < 1e-12 || nb < 1e-12 {
        return 0.0;
    }
    dot(a, b) / (na * nb)
}

/// Взвешенное среднее нормированных векторов, нормированное. `None` — нечего усреднять.
pub fn weighted_mean<'a>(items: impl IntoIterator<Item = (&'a [f32], f32)>) -> Option<Vec<f32>> {
    let mut sum: Option<Vec<f32>> = None;
    for (v, w) in items {
        if w <= 0.0 || v.is_empty() {
            continue;
        }
        let n = norm(v);
        if n < 1e-12 {
            continue;
        }
        let acc = sum.get_or_insert_with(|| vec![0.0; v.len()]);
        if acc.len() != v.len() {
            continue;
        }
        for (a, x) in acc.iter_mut().zip(v) {
            *a += x / n * w;
        }
    }
    sum.filter(|s| norm(s) > 1e-12).map(normalized)
}

/// Сдвиг вектора сессии к новому треку: `old` весит `old_weight`, трек — `step`.
pub fn blend(old: Option<&[f32]>, old_weight: f32, v: &[f32], step: f32) -> Vec<f32> {
    let mut v = v.to_vec();
    normalize(&mut v);
    match old {
        Some(o) if o.len() == v.len() && old_weight > 0.0 => {
            let mut o = o.to_vec();
            normalize(&mut o);
            let total = old_weight + step;
            normalized(
                o.iter()
                    .zip(&v)
                    .map(|(a, b)| (a * old_weight + b * step) / total)
                    .collect(),
            )
        }
        _ => v,
    }
}

/// Детерминированный генератор (splitmix64): k-means и доля случайности в волне воспроизводимы.
#[derive(Debug, Clone)]
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self(seed ^ 0x9e37_79b9_7f4a_7c15)
    }

    pub fn from_uuid(id: uuid::Uuid, salt: u64) -> Self {
        let (hi, lo) = id.as_u64_pair();
        Self::new(hi ^ lo.rotate_left(17) ^ salt.wrapping_mul(0x2545_f491_4f6c_dd1d))
    }

    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    /// 0..1
    pub fn next_f32(&mut self) -> f32 {
        (self.next_u64() >> 40) as f32 / (1u64 << 24) as f32
    }
}

/// Итог k-means: центры (нормированы) и номер центра у каждой точки.
#[derive(Debug, Clone, PartialEq)]
pub struct Clusters {
    pub centroids: Vec<Vec<f32>>,
    pub assignment: Vec<usize>,
}

/// Сферический взвешенный k-means: точки и центры нормированы, близость — косинус. Начало — k-means++
/// с весами, поэтому любимое и часто дослушанное тянет центры сильнее. Пустой кластер получает самую
/// далёкую от своего центра точку.
pub fn kmeans(
    points: &[Vec<f32>],
    weights: &[f32],
    k: usize,
    iters: usize,
    rng: &mut Rng,
) -> Clusters {
    let n = points.len();
    let k = k.clamp(1, n.max(1));
    if n == 0 {
        return Clusters {
            centroids: Vec::new(),
            assignment: Vec::new(),
        };
    }
    let pts: Vec<Vec<f32>> = points.iter().map(|p| normalized(p.clone())).collect();
    let w: Vec<f32> = (0..n)
        .map(|i| weights.get(i).copied().unwrap_or(1.0).max(1e-6))
        .collect();

    // k-means++: первый центр — с вероятностью по весу, следующие — по весу × квадрат расстояния
    let pick = |rng: &mut Rng, mass: &[f32]| -> usize {
        let total: f32 = mass.iter().sum();
        if total <= 0.0 {
            return 0;
        }
        let mut r = rng.next_f32() * total;
        for (i, m) in mass.iter().enumerate() {
            if r < *m {
                return i;
            }
            r -= m;
        }
        mass.len() - 1
    };
    let mut centroids: Vec<Vec<f32>> = vec![pts[pick(rng, &w)].clone()];
    while centroids.len() < k {
        let mass: Vec<f32> = pts
            .iter()
            .zip(&w)
            .map(|(p, wi)| {
                let best = centroids.iter().map(|c| dot(p, c)).fold(f32::MIN, f32::max);
                let d = (1.0 - best).max(0.0);
                wi * d * d
            })
            .collect();
        if mass.iter().sum::<f32>() <= 1e-9 {
            break;
        }
        centroids.push(pts[pick(rng, &mass)].clone());
    }

    let mut assignment = vec![0usize; n];
    for _ in 0..iters.max(1) {
        let mut changed = false;
        for (i, p) in pts.iter().enumerate() {
            let best = nearest(p, &centroids);
            if assignment[i] != best {
                assignment[i] = best;
                changed = true;
            }
        }
        let mut next: Vec<Vec<f32>> = Vec::with_capacity(centroids.len());
        for (c, old) in centroids.iter().enumerate() {
            let members = (0..n)
                .filter(|i| assignment[*i] == c)
                .map(|i| (pts[i].as_slice(), w[i]));
            next.push(weighted_mean(members).unwrap_or_else(|| old.clone()));
        }
        // Пустой кластер: забирает точку, хуже всех подходящую к своему центру
        let mut counts = vec![0usize; next.len()];
        for a in &assignment {
            counts[*a] += 1;
        }
        for c in 0..next.len() {
            if counts[c] > 0 {
                continue;
            }
            let worst = (0..n)
                .filter(|i| counts[assignment[*i]] > 1)
                .min_by(|a, b| {
                    dot(&pts[*a], &next[assignment[*a]])
                        .total_cmp(&dot(&pts[*b], &next[assignment[*b]]))
                });
            if let Some(i) = worst {
                counts[assignment[i]] -= 1;
                counts[c] = 1;
                assignment[i] = c;
                next[c] = pts[i].clone();
                changed = true;
            }
        }
        centroids = next;
        if !changed {
            break;
        }
    }
    Clusters {
        centroids,
        assignment,
    }
}

/// Сливает кластеры, чьи центры почти совпадают (косинус ≥ `threshold`): k-means с запасом делит
/// плотную группу надвое, а грани вкуса должны различаться.
pub fn merge_close(clusters: &mut Clusters, points: &[Vec<f32>], weights: &[f32], threshold: f32) {
    loop {
        let mut best: Option<(usize, usize, f32)> = None;
        for i in 0..clusters.centroids.len() {
            for j in (i + 1)..clusters.centroids.len() {
                let c = cosine(&clusters.centroids[i], &clusters.centroids[j]);
                if c >= threshold && best.is_none_or(|(_, _, b)| c > b) {
                    best = Some((i, j, c));
                }
            }
        }
        let Some((keep, gone, _)) = best else {
            return;
        };
        for a in clusters.assignment.iter_mut() {
            if *a == gone {
                *a = keep;
            } else if *a > gone {
                *a -= 1;
            }
        }
        clusters.centroids.remove(gone);
        let members = (0..points.len())
            .filter(|i| clusters.assignment[*i] == keep)
            .map(|i| (points[i].as_slice(), weights.get(i).copied().unwrap_or(1.0)));
        if let Some(c) = weighted_mean(members) {
            clusters.centroids[keep] = c;
        }
    }
}

/// Кластеры меньше `min_members` точек растворяются в ближайших: грань из одного трека — не грань.
/// Один кластер остаётся всегда.
pub fn drop_small(
    clusters: &mut Clusters,
    points: &[Vec<f32>],
    weights: &[f32],
    min_members: usize,
) {
    loop {
        if clusters.centroids.len() <= 1 {
            return;
        }
        let sizes: Vec<usize> = (0..clusters.centroids.len())
            .map(|c| clusters.assignment.iter().filter(|a| **a == c).count())
            .collect();
        let Some(small) = (0..sizes.len())
            .filter(|c| sizes[*c] < min_members)
            .min_by_key(|c| sizes[*c])
        else {
            return;
        };
        clusters.centroids.remove(small);
        for (i, a) in clusters.assignment.iter_mut().enumerate() {
            if *a == small {
                *a = nearest(&normalized(points[i].clone()), &clusters.centroids);
            } else if *a > small {
                *a -= 1;
            }
        }
        for c in 0..clusters.centroids.len() {
            let members = (0..points.len())
                .filter(|i| clusters.assignment[*i] == c)
                .map(|i| (points[i].as_slice(), weights.get(i).copied().unwrap_or(1.0)));
            if let Some(m) = weighted_mean(members) {
                clusters.centroids[c] = m;
            }
        }
    }
}

pub fn nearest(p: &[f32], centroids: &[Vec<f32>]) -> usize {
    centroids
        .iter()
        .enumerate()
        .map(|(i, c)| (i, dot(p, c)))
        .max_by(|a, b| a.1.total_cmp(&b.1))
        .map_or(0, |(i, _)| i)
}

/// Тональность по Камелоту: номер 1–12 и буква (A — минор, B — мажор).
pub fn camelot(key: &str) -> Option<(u8, char)> {
    let key = key.trim().to_ascii_uppercase();
    let letter = key.chars().last()?;
    if letter != 'A' && letter != 'B' {
        return None;
    }
    let num: u8 = key[..key.len() - 1].parse().ok()?;
    (1..=12).contains(&num).then_some((num, letter))
}

/// Насколько гладко сводятся две тональности: 1 — та же, 0.8 — соседняя по кругу или параллельная,
/// 0.4 — через одну, 0 — далёкие. Неизвестная тональность — нейтрально, 0.5.
pub fn camelot_compat(a: Option<&str>, b: Option<&str>) -> f32 {
    let (Some((na, la)), Some((nb, lb))) = (a.and_then(camelot), b.and_then(camelot)) else {
        return 0.5;
    };
    let diff = (i16::from(na) - i16::from(nb)).rem_euclid(12);
    let step = diff.min(12 - diff);
    match (step, la == lb) {
        (0, true) => 1.0,
        (0, false) | (1, true) => 0.8,
        (2, true) | (1, false) => 0.4,
        _ => 0.0,
    }
}

/// Разница темпов как доля с учётом «вдвое» (80 и 160 BPM сводятся): 0 — тот же темп.
pub fn tempo_gap(a: f32, b: f32) -> f32 {
    if a <= 0.0 || b <= 0.0 {
        return 0.0;
    }
    [b, b * 2.0, b / 2.0]
        .iter()
        .map(|x| (x / a - 1.0).abs())
        .fold(f32::MAX, f32::min)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn to_f16_bits(x: f32) -> u16 {
        // Только для точно представимых чисел из тестов
        match x {
            0.0 => 0,
            1.0 => 0x3c00,
            -2.0 => 0xc000,
            0.5 => 0x3800,
            65504.0 => 0x7bff,
            _ => panic!("{x}"),
        }
    }

    #[test]
    fn half_floats_decode_exactly() {
        for x in [0.0f32, 1.0, -2.0, 0.5, 65504.0] {
            assert_eq!(f16_to_f32(to_f16_bits(x)), x);
        }
        // Наименьшее субнормальное
        assert_eq!(f16_to_f32(0x0001), 2f32.powi(-24));
        assert!(f16_to_f32(0x7c00).is_infinite());
        assert!(f16_to_f32(0x7e00).is_nan());
        let bytes: Vec<u8> = [1.0f32, -2.0, 0.5]
            .iter()
            .flat_map(|x| to_f16_bits(*x).to_le_bytes())
            .collect();
        let b64 = STANDARD.encode(bytes);
        assert_eq!(decode_f16_base64(&b64, 3).unwrap(), vec![1.0, -2.0, 0.5]);
        assert!(decode_f16_base64(&b64, 4).is_err());
        assert!(decode_f16_base64("не base64", 3).is_err());
        let inf = STANDARD.encode(0x7c00u16.to_le_bytes());
        assert!(decode_f16_base64(&inf, 1).is_err());
    }

    #[test]
    fn means_and_blends() {
        let a = vec![1.0, 0.0];
        let b = vec![0.0, 2.0];
        let m = weighted_mean([(a.as_slice(), 1.0), (b.as_slice(), 1.0)]).unwrap();
        assert!((m[0] - m[1]).abs() < 1e-6 && (norm(&m) - 1.0).abs() < 1e-6);
        let tilted = weighted_mean([(a.as_slice(), 3.0), (b.as_slice(), 1.0)]).unwrap();
        assert!(tilted[0] > tilted[1]);
        assert!(weighted_mean([(a.as_slice(), 0.0)]).is_none());
        assert!((cosine(&a, &b)).abs() < 1e-6);
        assert_eq!(cosine(&a, &[0.0, 0.0]), 0.0);
        let first = blend(None, 0.0, &b, 1.0);
        assert_eq!(first, vec![0.0, 1.0]);
        let moved = blend(Some(&a), 1.0, &b, 1.0);
        assert!((moved[0] - moved[1]).abs() < 1e-6);
    }

    #[test]
    fn kmeans_finds_groups_and_is_reproducible() {
        let dim = 8;
        let around = |axis: usize, jitter: f32| {
            let mut v = vec![0.01; dim];
            v[axis] = 1.0;
            v[(axis + 1) % dim] = jitter;
            v
        };
        let mut points = Vec::new();
        for i in 0..6 {
            points.push(around(0, 0.05 * i as f32));
            points.push(around(4, 0.05 * i as f32));
        }
        let weights = vec![1.0; points.len()];
        let a = kmeans(&points, &weights, 2, 20, &mut Rng::new(7));
        let b = kmeans(&points, &weights, 2, 20, &mut Rng::new(7));
        assert_eq!(a, b);
        assert_eq!(a.centroids.len(), 2);
        // Чётные точки — вокруг оси 0, нечётные — вокруг оси 4
        let even = a.assignment[0];
        for (i, c) in a.assignment.iter().enumerate() {
            assert_eq!(*c == even, i % 2 == 0, "{:?}", a.assignment);
        }
        for c in &a.centroids {
            assert!((norm(c) - 1.0).abs() < 1e-4);
        }
        // Кластеров не больше, чем точек
        let one = kmeans(&points[..1], &weights[..1], 3, 10, &mut Rng::new(1));
        assert_eq!(one.centroids.len(), 1);
        assert!(
            kmeans(&[], &[], 2, 5, &mut Rng::new(1))
                .centroids
                .is_empty()
        );
        // Четыре кластера на две группы: почти совпадающие сливаются
        let mut four = kmeans(&points, &weights, 4, 20, &mut Rng::new(7));
        assert_eq!(four.centroids.len(), 4);
        merge_close(&mut four, &points, &weights, 0.93);
        assert_eq!(four.centroids.len(), 2);
        assert!(four.assignment.iter().all(|a| *a < 2));
        let even = four.assignment[0];
        for (i, c) in four.assignment.iter().enumerate() {
            assert_eq!(*c == even, i % 2 == 0);
        }
        // Одиночка растворяется в ближайшем кластере, последний кластер остаётся
        let mut lone = Clusters {
            centroids: vec![around(0, 0.0), around(4, 0.0), around(5, 0.0)],
            assignment: vec![0, 0, 0, 1, 1, 2],
        };
        let pts = vec![
            around(0, 0.0),
            around(0, 0.1),
            around(0, 0.2),
            around(4, 0.0),
            around(4, 0.1),
            around(4, 0.3),
        ];
        drop_small(&mut lone, &pts, &[1.0; 6], 3);
        assert_eq!(lone.centroids.len(), 2, "одиночка ушла к ближайшей двойке");
        assert_eq!(lone.assignment[5], lone.assignment[3]);
        assert_ne!(lone.assignment[0], lone.assignment[3]);
        let mut two = Clusters {
            centroids: vec![around(0, 0.0), around(4, 0.0)],
            assignment: vec![0, 0, 0, 1, 1, 1],
        };
        drop_small(&mut two, &pts, &[1.0; 6], 3);
        assert_eq!(two.centroids.len(), 2);
        let mut one = Clusters {
            centroids: vec![around(0, 0.0)],
            assignment: vec![0],
        };
        drop_small(&mut one, &pts[..1], &[1.0], 3);
        assert_eq!(one.centroids.len(), 1);
    }

    #[test]
    fn camelot_neighbours_and_tempo() {
        assert_eq!(camelot("8A"), Some((8, 'A')));
        assert_eq!(camelot("12b"), Some((12, 'B')));
        assert_eq!(camelot("13A"), None);
        assert_eq!(camelot("C major"), None);
        assert_eq!(camelot_compat(Some("8A"), Some("8A")), 1.0);
        assert_eq!(camelot_compat(Some("8A"), Some("9A")), 0.8);
        assert_eq!(
            camelot_compat(Some("12A"), Some("1A")),
            0.8,
            "круг замыкается"
        );
        assert_eq!(camelot_compat(Some("8A"), Some("8B")), 0.8);
        assert_eq!(camelot_compat(Some("8A"), Some("10A")), 0.4);
        assert_eq!(camelot_compat(Some("8A"), Some("2B")), 0.0);
        assert_eq!(camelot_compat(None, Some("2B")), 0.5);
        assert!(tempo_gap(120.0, 120.0) < 1e-6);
        assert!(tempo_gap(80.0, 160.0) < 1e-6, "вдвое — сводится");
        assert!((tempo_gap(100.0, 110.0) - 0.1).abs() < 1e-6);
        let mut r = Rng::new(3);
        let x = r.next_f32();
        assert!((0.0..1.0).contains(&x));
        assert_ne!(Rng::new(3).next_u64(), Rng::new(4).next_u64());
    }
}
