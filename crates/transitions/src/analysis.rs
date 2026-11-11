//! Анализ трека для переходов: то, что сервер отдаёт в `GET /api/tracks/{id}/analysis` (и что
//! присылает сервис анализа — те же имена полей в camelCase). Числа — `f64`: сервис присылает
//! дробные миллисекунды, сервер — целые; незнакомые поля пропускаются.
//!
//! Фразы считаются по сильным долям: музыка строится блоками по 8 тактов, и граница блока — там,
//! где меняется энергия, начинается или кончается вокал, кончается вступление или начинается
//! концовка. Из восьми возможных сдвигов сетки фраз выбирается тот, на чьих границах таких
//! перемен больше всего.

use serde::{Deserialize, Deserializer, Serialize};

/// Тактов во фразе.
pub const BARS_PER_PHRASE: usize = 8;

fn nullable<'de, D, T>(d: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: Default + Deserialize<'de>,
{
    Ok(Option::<T>::deserialize(d)?.unwrap_or_default())
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Analysis {
    pub duration_ms: Option<f64>,
    pub bpm: Option<f64>,
    /// Уверенность модели в сетке долей, 0…1.
    pub bpm_confidence: Option<f64>,
    #[serde(deserialize_with = "nullable")]
    pub beats_ms: Vec<f64>,
    #[serde(deserialize_with = "nullable")]
    pub downbeats_ms: Vec<f64>,
    /// Начала фраз по 8 тактов; пусто — считаются по сильным долям ([`Analysis::phrases`]).
    #[serde(deserialize_with = "nullable")]
    pub phrases_ms: Vec<f64>,
    pub key_camelot: Option<String>,
    pub key_confidence: Option<f64>,
    pub energy: Option<f64>,
    /// Энергия по секундам, 0…1000.
    #[serde(deserialize_with = "nullable")]
    pub energy_curve: Vec<f64>,
    /// Участки с вокалом: [начало, конец].
    #[serde(deserialize_with = "nullable")]
    pub vocal_segments: Vec<[f64; 2]>,
    pub intro_end_ms: Option<f64>,
    pub outro_start_ms: Option<f64>,
}

/// Сетка долей: средний шаг и насколько она ровная.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Grid {
    /// Медианный шаг долей, мс.
    pub beat_ms: f64,
    pub bpm: f64,
    /// 1 — доли идут ровно; 0 — разброс шага с сам шаг.
    pub regularity: f64,
    /// Долей в такте (медиана между сильными долями).
    pub beats_per_bar: u32,
    pub confidence: f64,
}

impl Grid {
    pub fn bar_ms(&self) -> f64 {
        self.beat_ms * f64::from(self.beats_per_bar)
    }
}

/// Фразы: начала блоков по 8 тактов и среди них — начала блоков по 16.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Phrases {
    pub starts: Vec<f64>,
    pub major: Vec<f64>,
}

/// Предложенные точки сведения: с какой доли входить в трек и где из него выходить.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Cues {
    pub mix_in_ms: f64,
    pub mix_out_ms: f64,
}

fn median(v: &mut [f64]) -> Option<f64> {
    if v.is_empty() {
        return None;
    }
    v.sort_by(f64::total_cmp);
    Some(v[v.len() / 2])
}

impl Analysis {
    /// Длина трека: из анализа или по последней доле.
    pub fn duration(&self) -> f64 {
        self.duration_ms
            .or_else(|| self.beats_ms.last().map(|b| b + 500.0))
            .unwrap_or(0.0)
    }

    /// Сетка долей, если она есть (хотя бы 16 долей).
    pub fn grid(&self) -> Option<Grid> {
        if self.beats_ms.len() < 16 {
            return None;
        }
        let mut steps: Vec<f64> = self.beats_ms.windows(2).map(|w| w[1] - w[0]).collect();
        let beat_ms = median(&mut steps.clone())?;
        if beat_ms <= 0.0 {
            return None;
        }
        // Разброс — по средней абсолютной разнице с медианой: одиночные пропуски долей его не раздувают
        let dev: f64 =
            steps.iter_mut().map(|s| (*s - beat_ms).abs()).sum::<f64>() / steps.len() as f64;
        let mut per_bar: Vec<f64> = self
            .downbeats_ms
            .windows(2)
            .map(|w| ((w[1] - w[0]) / beat_ms).round())
            .filter(|n| *n >= 2.0 && *n <= 7.0)
            .collect();
        let beats_per_bar = median(&mut per_bar).unwrap_or(4.0) as u32;
        Some(Grid {
            beat_ms,
            bpm: 60_000.0 / beat_ms,
            regularity: (1.0 - dev / beat_ms).clamp(0.0, 1.0),
            beats_per_bar,
            confidence: self.bpm_confidence.unwrap_or(0.0),
        })
    }

    /// Сетка долей без ступенек: модель отдаёт доли с шагом кадра (20 мс), а сведение требует
    /// точности в единицы миллисекунд. Каждая доля заменяется прямой, проведённой методом
    /// наименьших квадратов через ±8 соседних долей, — медленные изменения темпа сохраняются,
    /// дрожание кадров уходит. Сильные доли переносятся на ближайшие уточнённые доли.
    pub fn refined(&self) -> Analysis {
        const HALF: usize = 8;
        let b = &self.beats_ms;
        if b.len() < 2 * HALF + 1 {
            return self.clone();
        }
        let refined: Vec<f64> = (0..b.len())
            .map(|i| {
                let lo = i.saturating_sub(HALF).min(b.len() - 2 * HALF - 1);
                let hi = lo + 2 * HALF + 1;
                let n = (hi - lo) as f64;
                let (mut sx, mut sy, mut sxx, mut sxy) = (0.0, 0.0, 0.0, 0.0);
                for (k, t) in b[lo..hi].iter().enumerate() {
                    let x = k as f64;
                    sx += x;
                    sy += t;
                    sxx += x * x;
                    sxy += x * t;
                }
                let slope = (n * sxy - sx * sy) / (n * sxx - sx * sx);
                let icpt = (sy - slope * sx) / n;
                let fit = icpt + slope * (i - lo) as f64;
                // Пропуск или лишняя доля ломает прямую — там остаётся то, что дала модель
                if (fit - b[i]).abs() > 40.0 { b[i] } else { fit }
            })
            .collect();
        let downbeats = self
            .downbeats_ms
            .iter()
            .map(|d| {
                let j = self.beat_near(*d).unwrap_or(0);
                if (b[j] - d).abs() <= 40.0 {
                    refined[j]
                } else {
                    *d
                }
            })
            .collect();
        Analysis {
            beats_ms: refined,
            downbeats_ms: downbeats,
            ..self.clone()
        }
    }

    /// Номер доли, ближайшей к `ms`.
    pub fn beat_near(&self, ms: f64) -> Option<usize> {
        let b = &self.beats_ms;
        if b.is_empty() {
            return None;
        }
        let i = b.partition_point(|x| *x < ms);
        match i {
            0 => Some(0),
            i if i >= b.len() => Some(b.len() - 1),
            i => Some(if ms - b[i - 1] <= b[i] - ms { i - 1 } else { i }),
        }
    }

    /// Время доли с дробным номером; за пределами сетки — продолжение медианным шагом.
    pub fn beat_at(&self, index: f64, grid: &Grid) -> f64 {
        let b = &self.beats_ms;
        let last = b.len() as f64 - 1.0;
        if index < 0.0 {
            return b[0] + index * grid.beat_ms;
        }
        if index >= last {
            return b[b.len() - 1] + (index - last) * grid.beat_ms;
        }
        let i = index.floor() as usize;
        let f = index - i as f64;
        b[i] + (b[i + 1] - b[i]) * f
    }

    /// Совпадает ли доля с сильной (в пределах четверти доли).
    pub fn is_downbeat(&self, ms: f64, grid: &Grid) -> bool {
        let d = &self.downbeats_ms;
        let i = d.partition_point(|x| *x < ms);
        let near = |j: usize| {
            d.get(j)
                .is_some_and(|x| (x - ms).abs() <= grid.beat_ms / 4.0)
        };
        near(i) || (i > 0 && near(i - 1))
    }

    /// Есть ли вокал в момент `ms`.
    pub fn vocal_at(&self, ms: f64) -> bool {
        self.vocal_segments.iter().any(|[a, b]| ms >= *a && ms < *b)
    }

    /// Конец вокала: после этого момента голоса в треке нет.
    pub fn vocal_end(&self) -> f64 {
        self.vocal_segments.iter().map(|s| s[1]).fold(0.0, f64::max)
    }

    /// Первый момент с голосом не раньше `ms`.
    pub fn vocal_start_after(&self, ms: f64) -> Option<f64> {
        self.vocal_segments
            .iter()
            .filter(|[_, b]| *b > ms)
            .map(|[a, _]| a.max(ms))
            .min_by(f64::total_cmp)
    }

    /// Энергия в момент `ms`, 0…1 (кривая по секундам). Нет кривой — None.
    pub fn energy_at(&self, ms: f64) -> Option<f64> {
        let c = &self.energy_curve;
        if c.is_empty() {
            return None;
        }
        let i = ((ms / 1000.0).floor().max(0.0) as usize).min(c.len() - 1);
        Some((c[i] / 1000.0).clamp(0.0, 1.0))
    }

    /// Средняя энергия на отрезке.
    pub fn energy_mean(&self, from: f64, to: f64) -> Option<f64> {
        let c = &self.energy_curve;
        if c.is_empty() {
            return None;
        }
        let a = ((from / 1000.0).floor().max(0.0) as usize).min(c.len() - 1);
        let b = ((to / 1000.0).ceil().max(0.0) as usize).clamp(a + 1, c.len());
        Some(c[a..b].iter().sum::<f64>() / (b - a) as f64 / 1000.0)
    }

    /// Где звук начинается и кончается по-настоящему: тишина и затухание по краям не в счёт
    /// (энергия ниже 8 % медианной).
    pub fn audible(&self) -> (f64, f64) {
        let dur = self.duration();
        let c = &self.energy_curve;
        let Some(med) = median(&mut c.clone()) else {
            return (0.0, dur);
        };
        let floor = med * 0.08;
        let first = c.iter().position(|e| *e > floor).unwrap_or(0);
        let last = c.iter().rposition(|e| *e > floor).unwrap_or(c.len() - 1);
        (first as f64 * 1000.0, ((last + 1) as f64 * 1000.0).min(dur))
    }

    /// Насколько момент похож на границу фразы: перемена энергии, край вступления, концовки или
    /// вокала рядом.
    fn novelty(&self, ms: f64, bar_ms: f64) -> f64 {
        let mut score = 0.0;
        if let (Some(before), Some(after)) = (
            self.energy_mean(ms - 4000.0, ms),
            self.energy_mean(ms, ms + 4000.0),
        ) {
            score += (after - before).abs();
        }
        let near = |x: f64| (x - ms).abs() <= bar_ms / 2.0;
        if self.intro_end_ms.is_some_and(near) {
            score += 0.5;
        }
        if self.outro_start_ms.is_some_and(near) {
            score += 0.5;
        }
        if self
            .vocal_segments
            .iter()
            .any(|[a, b]| near(*a) || near(*b))
        {
            score += 0.3;
        }
        score
    }

    /// Фразы по 8 тактов. Есть готовые (`phrasesMs` с сервера) — они; иначе — по сильным долям.
    pub fn phrases(&self) -> Phrases {
        let d = &self.downbeats_ms;
        if !self.phrases_ms.is_empty() {
            return Phrases {
                starts: self.phrases_ms.clone(),
                major: self.phrases_ms.iter().step_by(2).copied().collect(),
            };
        }
        if d.is_empty() {
            return Phrases::default();
        }
        let bar_ms = self.grid().map(|g| g.bar_ms()).unwrap_or_else(|| {
            let mut s: Vec<f64> = d.windows(2).map(|w| w[1] - w[0]).collect();
            median(&mut s).unwrap_or(2000.0)
        });
        let score = |k: usize, step: usize| -> f64 {
            let picks: Vec<f64> = d.iter().skip(k).step_by(step).copied().collect();
            if picks.is_empty() {
                return f64::MIN;
            }
            picks.iter().map(|t| self.novelty(*t, bar_ms)).sum::<f64>() / picks.len() as f64
        };
        let shifts = BARS_PER_PHRASE.min(d.len());
        let mut best = 0;
        for k in 1..shifts {
            // Ничья — у меньшего сдвига: фраза чаще всего начинается с первой сильной доли
            if score(k, BARS_PER_PHRASE) > score(best, BARS_PER_PHRASE) + 1e-9 {
                best = k;
            }
        }
        let starts: Vec<f64> = d
            .iter()
            .skip(best)
            .step_by(BARS_PER_PHRASE)
            .copied()
            .collect();
        let major_shift = if starts.len() > 1
            && score(best + BARS_PER_PHRASE, 2 * BARS_PER_PHRASE)
                > score(best, 2 * BARS_PER_PHRASE) + 1e-9
        {
            1
        } else {
            0
        };
        let major = starts
            .iter()
            .skip(major_shift)
            .step_by(2)
            .copied()
            .collect();
        Phrases { starts, major }
    }

    /// Предложенные точки сведения. Вход — первая фраза, где уже есть звук; выход — первая
    /// граница фразы после вокала, за которой остаётся 16 тактов музыки (или хотя бы 8).
    pub fn cues(&self) -> Option<Cues> {
        let grid = self.grid()?;
        let phrases = self.phrases();
        let (first, last) = self.audible();
        let mix_in_ms = phrases
            .starts
            .iter()
            .copied()
            .find(|p| *p + grid.beat_ms >= first)
            .or_else(|| self.downbeats_ms.first().copied())?;
        let bar = grid.bar_ms();
        let after_vocals = self.vocal_end() - grid.beat_ms;
        let from = after_vocals.max(self.outro_start_ms.unwrap_or(0.0) - 8.0 * bar);
        let fits = |bars: f64| {
            phrases
                .starts
                .iter()
                .copied()
                .find(|p| *p >= from && *p > mix_in_ms && last - *p >= bars * bar)
        };
        let mix_out_ms = fits(16.0)
            .or_else(|| fits(8.0))
            .or_else(|| {
                phrases
                    .starts
                    .iter()
                    .copied()
                    .rfind(|p| *p > mix_in_ms && last - *p >= 8.0 * bar)
            })
            .unwrap_or_else(|| (last - 8.0 * bar).max(mix_in_ms));
        Some(Cues {
            mix_in_ms,
            mix_out_ms,
        })
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// Ровная сетка: `bpm`, `beats` долей с `start`, сильная — каждая четвёртая.
    pub fn synthetic(bpm: f64, beats: usize, start: f64) -> Analysis {
        let step = 60_000.0 / bpm;
        let beats_ms: Vec<f64> = (0..beats).map(|i| start + i as f64 * step).collect();
        let downbeats_ms = beats_ms.iter().step_by(4).copied().collect();
        let dur = start + beats as f64 * step;
        Analysis {
            duration_ms: Some(dur),
            bpm: Some(bpm),
            bpm_confidence: Some(0.95),
            beats_ms,
            downbeats_ms,
            energy_curve: vec![600.0; (dur / 1000.0).ceil() as usize],
            ..Default::default()
        }
    }

    #[test]
    fn parses_ml_and_server_shapes() {
        let ml: Analysis = serde_json::from_str(
            r#"{"bpm": 124.99, "bpmConfidence": 0.94, "beatsMs": [10.5, 490.2], "downbeatsMs": null,
                "vocalSegments": [[14408.0, 199693.0]], "energyCurve": [278.6], "sound": "AAAA",
                "keyCamelot": "6A"}"#,
        )
        .unwrap();
        assert_eq!(ml.beats_ms, vec![10.5, 490.2]);
        assert!(ml.downbeats_ms.is_empty());
        assert_eq!(ml.vocal_segments, vec![[14408.0, 199693.0]]);
        let server: Analysis = serde_json::from_str(
            r#"{"beatsMs": [10, 490], "phrasesMs": [10], "durationMs": 1000}"#,
        )
        .unwrap();
        assert_eq!(server.phrases_ms, vec![10.0]);
        assert_eq!(server.duration(), 1000.0);
    }

    #[test]
    fn refined_grid_removes_frame_jitter() {
        let mut a = synthetic(124.0, 64, 100.0);
        let truth = a.beats_ms.clone();
        // Как у модели: доли округлены до кадра 20 мс
        for b in a.beats_ms.iter_mut().chain(a.downbeats_ms.iter_mut()) {
            *b = (*b / 20.0).round() * 20.0;
        }
        let r = a.refined();
        let worst = r
            .beats_ms
            .iter()
            .zip(&truth)
            .map(|(x, t)| (x - t).abs())
            .fold(0.0, f64::max);
        assert!(worst < 2.0, "{worst}");
        assert!((r.downbeats_ms[1] - truth[4]).abs() < 2.0);
    }

    #[test]
    fn grid_of_a_steady_track() {
        let a = synthetic(120.0, 200, 300.0);
        let g = a.grid().unwrap();
        assert!((g.bpm - 120.0).abs() < 1e-6);
        assert_eq!(g.beats_per_bar, 4);
        assert!(g.regularity > 0.99);
        assert_eq!(a.beat_near(810.0), Some(1));
        assert!((a.beat_at(1.5, &g) - 1050.0).abs() < 1e-9);
        assert!(a.is_downbeat(300.0 + 4.0 * 500.0, &g));
        assert!(!a.is_downbeat(300.0 + 5.0 * 500.0, &g));
    }

    #[test]
    fn phrases_follow_energy_changes() {
        // 120 BPM, такт 2 с. Сетка фраз сдвинута на 3 такта: энергия меняется на 6-й, 22-й,
        // 38-й … секунде — там и должны начинаться фразы.
        let mut a = synthetic(120.0, 4 * 64, 0.0);
        for (s, e) in a.energy_curve.iter_mut().enumerate() {
            let block = (s as i64 - 6).div_euclid(16);
            *e = if block % 2 == 0 { 800.0 } else { 300.0 };
        }
        let p = a.phrases();
        assert_eq!(p.starts[0], 6000.0);
        assert!((p.starts[1] - p.starts[0] - 16_000.0).abs() < 1e-9);
        // Без кривой энергии — с первой сильной доли
        let mut flat = synthetic(120.0, 64, 0.0);
        flat.energy_curve.clear();
        assert_eq!(flat.phrases().starts[0], 0.0);
    }

    #[test]
    fn cues_skip_vocals_and_silence() {
        let mut a = synthetic(120.0, 4 * 96, 0.0); // 192 с
        a.energy_curve[0] = 0.0;
        a.energy_curve[1] = 0.0;
        a.vocal_segments = vec![[20_000.0, 140_000.0]];
        let c = a.cues().unwrap();
        // Первые 2 с тишины: звук начинается с перемены энергии — там и фраза, и вход
        assert_eq!(c.mix_in_ms, 2000.0);
        // После вокала (140 с) первая граница фразы — 146 с, за ней 46 с ≥ 16 тактов
        assert_eq!(c.mix_out_ms, 146_000.0);
    }
}
