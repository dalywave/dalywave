//! Карта темпа входящего трека: какое место файла звучит через `t` мс после старта деки.
//!
//! Во время перехода доли входящего ложатся на доли уходящего (узел на каждую долю — так
//! сетка следует и за живым, неровным темпом), затем скорость за несколько тактов линейно
//! возвращается к единице. После последнего узла — своя скорость: 1 мс звука на 1 мс файла.

/// Узлы (время от старта деки, позиция в файле), мс; обе координаты строго растут.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TempoMap {
    knots: Vec<(f64, f64)>,
}

impl TempoMap {
    /// Узлы должны расти по обеим координатам; неподходящие отбрасываются.
    pub fn new(knots: Vec<(f64, f64)>) -> Self {
        let mut out: Vec<(f64, f64)> = Vec::with_capacity(knots.len());
        for k in knots {
            if out.last().is_none_or(|l| k.0 > l.0 && k.1 > l.1) {
                out.push(k);
            }
        }
        Self { knots: out }
    }

    /// Без растяжения: файл с позиции `src_ms`.
    pub fn identity(src_ms: f64) -> Self {
        Self {
            knots: vec![(0.0, src_ms)],
        }
    }

    pub fn knots(&self) -> &[(f64, f64)] {
        &self.knots
    }

    /// Где в файле начинается звук деки.
    pub fn start(&self) -> f64 {
        self.knots.first().map_or(0.0, |k| k.1)
    }

    /// С какого времени деки скорость снова единица.
    pub fn settled_at(&self) -> f64 {
        self.knots.last().map_or(0.0, |k| k.0)
    }

    /// Растягивает ли карта звук хоть где-то.
    pub fn is_identity(&self) -> bool {
        self.knots.len() < 2
    }

    /// Позиция в файле через `t` мс от старта деки.
    pub fn src_at(&self, t: f64) -> f64 {
        let k = &self.knots;
        match k.len() {
            0 => t,
            1 => k[0].1 + (t - k[0].0),
            _ => {
                let i = k.partition_point(|p| p.0 <= t);
                if i == 0 {
                    // До первого узла — со скоростью первого отрезка
                    let r = (k[1].1 - k[0].1) / (k[1].0 - k[0].0);
                    k[0].1 + (t - k[0].0) * r
                } else if i >= k.len() {
                    let l = k[k.len() - 1];
                    l.1 + (t - l.0)
                } else {
                    let (a, b) = (k[i - 1], k[i]);
                    a.1 + (t - a.0) * (b.1 - a.1) / (b.0 - a.0)
                }
            }
        }
    }

    /// Обратное: через сколько мс от старта деки звучит место файла `src`.
    pub fn out_at(&self, src: f64) -> f64 {
        let k = &self.knots;
        match k.len() {
            0 => src,
            1 => k[0].0 + (src - k[0].1),
            _ => {
                let i = k.partition_point(|p| p.1 <= src);
                if i == 0 {
                    let r = (k[1].1 - k[0].1) / (k[1].0 - k[0].0);
                    k[0].0 + (src - k[0].1) / r
                } else if i >= k.len() {
                    let l = k[k.len() - 1];
                    l.0 + (src - l.1)
                } else {
                    let (a, b) = (k[i - 1], k[i]);
                    a.0 + (src - a.1) * (b.0 - a.0) / (b.1 - a.1)
                }
            }
        }
    }

    /// Скорость (мс файла на мс звука) в момент `t`.
    pub fn rate_at(&self, t: f64) -> f64 {
        let k = &self.knots;
        if k.len() < 2 {
            return 1.0;
        }
        let i = k.partition_point(|p| p.0 <= t).clamp(1, k.len());
        if i >= k.len() {
            return 1.0;
        }
        (k[i].1 - k[i - 1].1) / (k[i].0 - k[i - 1].0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_both_ways() {
        let m = TempoMap::new(vec![(0.0, 1000.0), (1000.0, 2100.0), (2000.0, 3150.0)]);
        assert_eq!(m.src_at(0.0), 1000.0);
        assert!((m.src_at(500.0) - 1550.0).abs() < 1e-9);
        assert!((m.rate_at(500.0) - 1.1).abs() < 1e-9);
        assert!((m.rate_at(1500.0) - 1.05).abs() < 1e-9);
        // После последнего узла — своя скорость
        assert!((m.src_at(3000.0) - 4150.0).abs() < 1e-9);
        assert_eq!(m.rate_at(5000.0), 1.0);
        for t in [0.0, 250.0, 999.0, 1800.0, 2600.0] {
            assert!((m.out_at(m.src_at(t)) - t).abs() < 1e-9, "t = {t}");
        }
        assert_eq!(m.settled_at(), 2000.0);
        let id = TempoMap::identity(500.0);
        assert!(id.is_identity());
        assert_eq!(id.src_at(100.0), 600.0);
        assert_eq!(id.out_at(600.0), 100.0);
        // Невозрастающие узлы отбрасываются
        assert_eq!(
            TempoMap::new(vec![(0.0, 0.0), (0.0, 1.0), (1.0, 0.0)])
                .knots()
                .len(),
            1
        );
    }
}
