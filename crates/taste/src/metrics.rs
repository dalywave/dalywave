//! Метрики на отложенной выборке: AUC и logloss по головам, качество порядка внутри сессии (так волна
//! и работает — выбирает, что поставить следующим) и бутстреп по сессиям: насколько уверенно одна
//! оценка лучше другой.

use serde::{Deserialize, Serialize};

use crate::{HEADS, net::sigmoid, rng::Rng};

/// AUC по рангам (Манн — Уитни), равные оценки — средним рангом. Один исход — `None`.
pub fn auc(y: &[f32], score: &[f32]) -> Option<f64> {
    auc_idx(y, score, &(0..y.len()).collect::<Vec<_>>())
}

fn auc_idx(y: &[f32], score: &[f32], idx: &[usize]) -> Option<f64> {
    let npos = idx.iter().filter(|i| y[**i] > 0.5).count();
    let nneg = idx.len() - npos;
    if npos == 0 || nneg == 0 {
        return None;
    }
    let mut order: Vec<usize> = idx.to_vec();
    order.sort_by(|a, b| score[*a].total_cmp(&score[*b]));
    let mut rank_sum = 0.0f64;
    let mut i = 0;
    while i < order.len() {
        let mut j = i;
        while j + 1 < order.len() && score[order[j + 1]] == score[order[i]] {
            j += 1;
        }
        let avg = (i + j) as f64 / 2.0 + 1.0;
        for k in &order[i..=j] {
            if y[*k] > 0.5 {
                rank_sum += avg;
            }
        }
        i = j + 1;
    }
    Some((rank_sum - (npos * (npos + 1)) as f64 / 2.0) / (npos as f64 * nneg as f64))
}

pub fn logloss(y: &[f32], p: &[f32]) -> f64 {
    if y.is_empty() {
        return 0.0;
    }
    y.iter()
        .zip(p)
        .map(|(y, p)| {
            let p = f64::from(*p).clamp(1e-6, 1.0 - 1e-6);
            let y = f64::from(*y);
            -(y * p.ln() + (1.0 - y) * (1.0 - p).ln())
        })
        .sum::<f64>()
        / y.len() as f64
}

/// Логистическая регрессия по одному признаку (Ньютон, L2): оценка правил → вероятность.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Calibration {
    pub a: f64,
    pub b: f64,
    pub mu: f64,
    pub sd: f64,
}

impl Calibration {
    pub fn fit(x: &[f32], y: &[f32]) -> Self {
        let n = x.len().max(1) as f64;
        let mu = x.iter().map(|v| f64::from(*v)).sum::<f64>() / n;
        let var = x.iter().map(|v| (f64::from(*v) - mu).powi(2)).sum::<f64>() / n;
        let sd = if var.sqrt() > 1e-12 { var.sqrt() } else { 1.0 };
        let rate = y.iter().map(|v| f64::from(*v)).sum::<f64>() / n;
        let (mut a, mut b) = (0.0f64, ((rate + 1e-3) / (1.0 - rate + 1e-3)).ln());
        for _ in 0..30 {
            let (mut ga, mut gb, mut haa, mut hab, mut hbb) = (1e-3 * a, 0.0, 1e-3, 0.0, 1e-9);
            for (xv, yv) in x.iter().zip(y) {
                let xs = (f64::from(*xv) - mu) / sd;
                let p = 1.0 / (1.0 + (-(a * xs + b)).exp());
                let d = p - f64::from(*yv);
                ga += d * xs;
                gb += d;
                let w = p * (1.0 - p);
                haa += w * xs * xs;
                hab += w * xs;
                hbb += w;
            }
            let det = haa * hbb - hab * hab;
            if det.abs() < 1e-12 {
                break;
            }
            a -= (hbb * ga - hab * gb) / det;
            b -= (haa * gb - hab * ga) / det;
        }
        Self { a, b, mu, sd }
    }

    pub fn apply(&self, x: f32) -> f32 {
        sigmoid((self.a * (f64::from(x) - self.mu) / self.sd + self.b) as f32)
    }
}

/// Голова на отложенной части.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Head {
    pub positives: usize,
    pub auc: Option<f64>,
    pub logloss: f64,
}

/// Оценки одной модели на отложенной части.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Scores {
    pub heads: Vec<Head>,
    /// Средние по головам, у которых оба исхода.
    pub auc: Option<f64>,
    pub logloss: Option<f64>,
    /// Порядок внутри сессии: доля верно упорядоченных пар «пропустил — дослушал» (средняя по сессиям).
    pub session_auc: Option<f64>,
    /// NDCG@10 внутри сессии: польза — дослушал (1) и лайк (+1).
    pub ndcg: Option<f64>,
    pub sessions: usize,
}

/// Оценка строки для порядка в волне — так же сеть двигает кандидатов при подборе.
pub fn wave_score(p: &[f32; 3]) -> f32 {
    0.5 * p[1] + p[2] - 0.8 * p[0]
}

/// Метрики по вероятностям `probs` (строки × 3), меткам и сессиям строк.
pub fn scores(probs: &[[f32; 3]], labels: &[f32], groups: &[u32], rank: &[f32]) -> Scores {
    let idx: Vec<usize> = (0..probs.len()).collect();
    scores_idx(probs, labels, groups, rank, &idx)
}

fn scores_idx(
    probs: &[[f32; 3]],
    labels: &[f32],
    groups: &[u32],
    rank: &[f32],
    idx: &[usize],
) -> Scores {
    let mut heads = Vec::with_capacity(3);
    let (mut aucs, mut losses) = (Vec::new(), Vec::new());
    for h in 0..HEADS.len() {
        let y: Vec<f32> = idx.iter().map(|i| labels[i * 3 + h]).collect();
        let p: Vec<f32> = idx.iter().map(|i| probs[*i][h]).collect();
        let a = auc(&y, &p);
        let l = logloss(&y, &p);
        if let Some(a) = a {
            aucs.push(a);
            losses.push(l);
        }
        heads.push(Head {
            positives: y.iter().filter(|v| **v > 0.5).count(),
            auc: a,
            logloss: l,
        });
    }
    let mean = |xs: &[f64]| (!xs.is_empty()).then(|| xs.iter().sum::<f64>() / xs.len() as f64);
    let (session_auc, ndcg, sessions) = session_metrics(labels, groups, rank, idx);
    Scores {
        heads,
        auc: mean(&aucs),
        logloss: mean(&losses),
        session_auc,
        ndcg,
        sessions,
    }
}

/// Сессии по порядку строк: AUC пар «пропуск — не пропуск» по оценке `rank` (выше — лучше) и NDCG@10.
fn session_metrics(
    labels: &[f32],
    groups: &[u32],
    rank: &[f32],
    idx: &[usize],
) -> (Option<f64>, Option<f64>, usize) {
    let mut by: std::collections::BTreeMap<u32, Vec<usize>> = Default::default();
    for &i in idx {
        by.entry(groups[i]).or_default().push(i);
    }
    let (mut pairs_ok, mut pairs) = (0.0f64, 0.0f64);
    let (mut ndcg_sum, mut ndcg_n) = (0.0f64, 0usize);
    for rows in by.values() {
        if rows.len() < 2 {
            continue;
        }
        // Пары: не пропущенный должен стоять выше пропущенного
        for &a in rows {
            if labels[a * 3] > 0.5 {
                continue;
            }
            for &b in rows {
                if labels[b * 3] < 0.5 {
                    continue;
                }
                pairs += 1.0;
                if rank[a] > rank[b] {
                    pairs_ok += 1.0;
                } else if rank[a] == rank[b] {
                    pairs_ok += 0.5;
                }
            }
        }
        let gain = |i: usize| f64::from(labels[i * 3 + 1] + labels[i * 3 + 2]);
        let mut by_rank = rows.clone();
        by_rank.sort_by(|a, b| rank[*b].total_cmp(&rank[*a]).then(a.cmp(b)));
        let mut ideal: Vec<f64> = rows.iter().map(|i| gain(*i)).collect();
        ideal.sort_by(|a, b| b.total_cmp(a));
        let dcg = |g: &mut dyn Iterator<Item = f64>| {
            g.take(10)
                .enumerate()
                .map(|(k, x)| x / (k as f64 + 2.0).log2())
                .sum::<f64>()
        };
        let best = dcg(&mut ideal.iter().copied());
        // Все исходы одинаковы — порядок ничего не значит
        if best > 0.0 && ideal.first() != ideal.last() {
            ndcg_sum += dcg(&mut by_rank.iter().map(|i| gain(*i))) / best;
            ndcg_n += 1;
        }
    }
    (
        (pairs > 0.0).then(|| pairs_ok / pairs),
        (ndcg_n > 0).then(|| ndcg_sum / ndcg_n as f64),
        ndcg_n,
    )
}

/// Сравнение двух оценок на одной отложенной части: разница средних AUC и доля бутстреп-выборок
/// по сессиям, где новая лучше.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Comparison {
    pub auc_gain: Option<f64>,
    pub logloss_gain: Option<f64>,
    /// Доля бутстреп-выборок, где средняя AUC новой выше.
    pub p_better: Option<f64>,
}

/// Новая (`a`) против старой (`b`) на тех же строках. Сессии — единица выборки: строки сессии связаны.
pub fn compare(
    a: &[[f32; 3]],
    b: &[[f32; 3]],
    labels: &[f32],
    groups: &[u32],
    resamples: usize,
    seed: u64,
) -> Comparison {
    let ra: Vec<f32> = a.iter().map(wave_score).collect();
    let rb: Vec<f32> = b.iter().map(wave_score).collect();
    let all: Vec<usize> = (0..a.len()).collect();
    let sa = scores_idx(a, labels, groups, &ra, &all);
    let sb = scores_idx(b, labels, groups, &rb, &all);
    let (auc_gain, logloss_gain) = match (paired_auc(&sa, &sb), sa.logloss, sb.logloss) {
        (Some((x, y)), Some(la), Some(lb)) => (Some(x - y), Some(lb - la)),
        _ => (None, None),
    };
    let mut groups_list: Vec<u32> = groups.to_vec();
    groups_list.sort_unstable();
    groups_list.dedup();
    let mut rows_of: std::collections::HashMap<u32, Vec<usize>> = Default::default();
    for (i, g) in groups.iter().enumerate() {
        rows_of.entry(*g).or_default().push(i);
    }
    let mut rng = Rng::new(seed);
    let (mut wins, mut total) = (0usize, 0usize);
    if auc_gain.is_some() && !groups_list.is_empty() {
        for _ in 0..resamples {
            let mut idx = Vec::with_capacity(a.len());
            for _ in 0..groups_list.len() {
                let g = groups_list[rng.below(groups_list.len())];
                idx.extend_from_slice(&rows_of[&g]);
            }
            let x = scores_idx(a, labels, groups, &ra, &idx);
            let y = scores_idx(b, labels, groups, &rb, &idx);
            if let Some((x, y)) = paired_auc(&x, &y) {
                total += 1;
                if x > y {
                    wins += 1;
                }
            }
        }
    }
    Comparison {
        auc_gain,
        logloss_gain,
        p_better: (total > 0).then(|| wins as f64 / total as f64),
    }
}

/// Средние AUC по головам, где AUC есть у обеих.
fn paired_auc(a: &Scores, b: &Scores) -> Option<(f64, f64)> {
    let pairs: Vec<(f64, f64)> = a
        .heads
        .iter()
        .zip(&b.heads)
        .filter_map(|(x, y)| Some((x.auc?, y.auc?)))
        .collect();
    if pairs.is_empty() {
        return None;
    }
    let n = pairs.len() as f64;
    Some((
        pairs.iter().map(|p| p.0).sum::<f64>() / n,
        pairs.iter().map(|p| p.1).sum::<f64>() / n,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auc_and_logloss() {
        assert_eq!(auc(&[0.0, 0.0, 1.0, 1.0], &[0.1, 0.2, 0.8, 0.9]), Some(1.0));
        assert_eq!(auc(&[0.0, 1.0], &[0.5, 0.5]), Some(0.5));
        assert_eq!(auc(&[1.0, 1.0], &[0.1, 0.2]), None);
        assert!((auc(&[0.0, 1.0, 0.0, 1.0], &[0.1, 0.4, 0.5, 0.8]).unwrap() - 0.75).abs() < 1e-9);
        assert!(logloss(&[1.0], &[0.9]) < logloss(&[1.0], &[0.5]));
    }

    #[test]
    fn calibration_follows_sign() {
        let x = [0.1f32, 0.2, 0.3, 0.7, 0.8, 0.9];
        let y = [1.0f32, 1.0, 1.0, 0.0, 0.0, 0.0];
        let c = Calibration::fit(&x, &y);
        assert!(c.apply(0.1) > 0.5 && c.apply(0.9) < 0.5);
    }

    #[test]
    fn session_order_and_comparison() {
        // Две сессии: в каждой один пропуск и одно дослушивание
        let labels = [1.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0];
        let groups = [1, 1, 2, 2];
        let good = [
            [0.9, 0.1, 0.0],
            [0.1, 0.9, 0.8],
            [0.8, 0.2, 0.0],
            [0.2, 0.8, 0.1],
        ];
        let bad = [
            [0.1, 0.9, 0.0],
            [0.9, 0.1, 0.0],
            [0.2, 0.8, 0.0],
            [0.8, 0.2, 0.0],
        ];
        let rg: Vec<f32> = good.iter().map(wave_score).collect();
        let s = scores(&good, &labels, &groups, &rg);
        assert_eq!(s.session_auc, Some(1.0));
        assert_eq!(s.ndcg, Some(1.0));
        assert_eq!(s.sessions, 2);
        let c = compare(&good, &bad, &labels, &groups, 50, 1);
        assert!(c.auc_gain.unwrap() > 0.5);
        assert_eq!(c.p_better, Some(1.0));
    }
}
