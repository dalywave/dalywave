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

    /// Сетка с починкой. Модель местами меняет уровень пульса: в куплете отдаёт каждую долю, в
    /// припеве — через одну (шаги 420 и 840 мс вперемешку), теряет доли в брейках и ставит
    /// сильные доли то через такт, то через два. Средний шаг такой сетки ровный, а «ровность» —
    /// нет, и раньше такие треки целиком уходили в страховку.
    ///
    /// Починка: основной шаг — частый пульс, если он занимает заметную долю сетки (≥ 25 %);
    /// пропуски кратной длины заполняются равномерно, лишние доли (ближе полушага) убираются.
    /// Сильные доли — каждые `n` долей (n = 4 или 3, что лучше объясняет модель): пропуски
    /// заполняются, лишние (ближе n − 1 доли к предыдущей) убираются, сдвиг фазы на долю
    /// принимается. Уверенность растёт ровно настолько, насколько ровнее стала сетка. Готовые
    /// фразы с сервера сбрасываются — они считаны по сетке до починки.
    pub fn repaired(&self) -> Analysis {
        let mut b: Vec<f64> = self.beats_ms.clone();
        b.sort_by(f64::total_cmp);
        b.dedup_by(|x, y| (*x - *y).abs() < 1.0);
        if b.len() < 16 {
            return self.clone();
        }
        let steps: Vec<f64> = b.windows(2).map(|w| w[1] - w[0]).collect();
        let Some(m) = median(&mut steps.clone()) else {
            return self.clone();
        };
        let near = |x: f64, tol: f64| -> Vec<f64> {
            steps
                .iter()
                .copied()
                .filter(|s| (s - x).abs() <= tol * x)
                .collect()
        };
        let share = |x: f64| near(x, 0.08).len() as f64 / steps.len() as f64;
        // Частый пульс — если он не случайные лишние доли и не быстрее 210 BPM
        let base = if share(m / 2.0) >= 0.25 && 60_000.0 / (m / 2.0) <= 210.0 {
            m / 2.0
        } else {
            m
        };
        let Some(t) = median(&mut near(base, 0.08)) else {
            return self.clone();
        };
        let inliers_before = near(m, 0.07).len() as f64 / steps.len() as f64;

        // Лишняя доля (ближе 0,6 шага к соседней): из двух остаётся та, с которой соседние шаги
        // ближе к целому числу шагов
        let off = |g: f64| (g - (g / t).round().max(1.0) * t).abs();
        let mut clean: Vec<f64> = Vec::with_capacity(b.len());
        for (i, &x) in b.iter().enumerate() {
            if let Some(&last) = clean.last()
                && x - last < 0.6 * t
            {
                let prev = clean.len().checked_sub(2).map(|j| clean[j]);
                let next = b.get(i + 1).copied();
                let drop_last = match (prev, next) {
                    (Some(p), Some(n)) => off(x - p) + off(n - x) < off(last - p) + off(n - last),
                    (Some(p), None) => off(x - p) < off(last - p),
                    (None, Some(n)) => off(n - x) < off(n - last),
                    (None, None) => false,
                };
                if drop_last {
                    clean.pop();
                    clean.push(x);
                }
                continue;
            }
            clean.push(x);
        }

        // Пропуски кратной длины — равномерно
        let mut beats = Vec::with_capacity(clean.len() * 2);
        beats.push(clean[0]);
        for &x in &clean[1..] {
            let last = *beats.last().unwrap_or(&x);
            let gap = x - last;
            let k = (gap / t).round();
            if (2.0..=64.0).contains(&k) {
                for q in 1..k as usize {
                    beats.push(last + gap * q as f64 / k);
                }
            }
            beats.push(x);
        }

        // Сильные доли — номерами долей новой сетки
        let index_of = |ms: f64| -> Option<usize> {
            let i = beats.partition_point(|x| *x < ms);
            [i.checked_sub(1), Some(i)]
                .into_iter()
                .flatten()
                .filter(|j| *j < beats.len())
                .min_by(|a, b| (beats[*a] - ms).abs().total_cmp(&(beats[*b] - ms).abs()))
                .filter(|j| (beats[*j] - ms).abs() <= t / 3.0)
        };
        let mut idx: Vec<usize> = self
            .downbeats_ms
            .iter()
            .filter_map(|d| index_of(*d))
            .collect();
        idx.sort_unstable();
        idx.dedup();
        let gaps: Vec<usize> = idx.windows(2).map(|w| w[1] - w[0]).collect();
        let fits = |n: usize| gaps.iter().filter(|g| **g % n == 0).count();
        let per_bar = if !gaps.is_empty() && fits(3) > fits(4) + gaps.len() / 4 {
            3
        } else {
            4
        };
        let mut down: Vec<usize> = Vec::with_capacity(beats.len() / per_bar + 1);
        for i in idx {
            match down.last().copied() {
                None => down.push(i),
                Some(last) if i < last + per_bar - 1 => {}
                Some(mut last) => {
                    while i > last + per_bar + per_bar / 2 {
                        last += per_bar;
                        down.push(last);
                    }
                    down.push(i);
                }
            }
        }

        let new_steps: Vec<f64> = beats.windows(2).map(|w| w[1] - w[0]).collect();
        let inliers_after = new_steps
            .iter()
            .filter(|s| (*s - t).abs() <= 0.07 * t)
            .count() as f64
            / new_steps.len().max(1) as f64;
        let bpm_confidence = self.bpm_confidence.map(|c| {
            if inliers_before > 0.0 && inliers_after > inliers_before {
                (c * inliers_after / inliers_before).min(1.0)
            } else {
                c
            }
        });
        Analysis {
            bpm: Some(60_000.0 / t),
            bpm_confidence,
            downbeats_ms: down.iter().map(|i| beats[*i]).collect(),
            beats_ms: beats,
            phrases_ms: Vec::new(),
            ..self.clone()
        }
    }

    /// Насколько ровно идут доли на отрезке (как [`Grid::regularity`]); меньше 8 долей — 0.
    pub fn regularity_between(&self, from: f64, to: f64) -> f64 {
        let b: Vec<f64> = self
            .beats_ms
            .iter()
            .copied()
            .filter(|x| *x >= from && *x <= to)
            .collect();
        if b.len() < 8 {
            return 0.0;
        }
        let steps: Vec<f64> = b.windows(2).map(|w| w[1] - w[0]).collect();
        let Some(m) = median(&mut steps.clone()) else {
            return 0.0;
        };
        let dev = steps.iter().map(|s| (s - m).abs()).sum::<f64>() / steps.len() as f64;
        (1.0 - dev / m).clamp(0.0, 1.0)
    }

    /// Где музыка трека по-настоящему кончается: дальше — затухание или тишина. Последняя
    /// секунда, где сглаженная (3 с) энергия не ниже половины «тела» трека (70-й перцентиль).
    /// Нет кривой энергии — конец трека.
    pub fn music_end(&self) -> f64 {
        let dur = self.duration();
        let c = &self.energy_curve;
        if c.len() < 8 {
            return dur;
        }
        let mut sorted = c.clone();
        sorted.sort_by(f64::total_cmp);
        let body = sorted[(sorted.len() * 7 / 10).min(sorted.len() - 1)];
        let smooth = |i: usize| {
            let a = i.saturating_sub(1);
            let b = (i + 2).min(c.len());
            c[a..b].iter().sum::<f64>() / (b - a) as f64
        };
        let last = (0..c.len())
            .rev()
            .find(|i| smooth(*i) >= 0.5 * body)
            .unwrap_or(c.len() - 1);
        ((last + 1) as f64 * 1000.0).min(dur)
    }

    /// Начало концовки: оттуда уходящий можно уводить, не теряя «тела» трека. Концовка из
    /// анализа, если она есть и кончается не раньше музыки; иначе — конец музыки.
    pub fn outro_start(&self) -> f64 {
        let end = self.music_end();
        self.outro_start_ms
            .filter(|o| *o > 0.0 && *o < end)
            .unwrap_or(end)
    }

    /// Сетка долей без ступенек: модель отдаёт доли с шагом кадра (20 мс), а сведение требует
    /// точности в единицы миллисекунд. Сначала сетка чинится ([`Analysis::repaired`]), потом
    /// каждая доля заменяется прямой, проведённой методом наименьших квадратов через ±8
    /// соседних долей, — медленные изменения темпа сохраняются, дрожание кадров уходит. Сильные
    /// доли переносятся на ближайшие уточнённые доли.
    pub fn refined(&self) -> Analysis {
        const HALF: usize = 8;
        let fixed = self.repaired();
        let b = &fixed.beats_ms;
        if b.len() < 2 * HALF + 1 {
            return fixed;
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
        let downbeats = fixed
            .downbeats_ms
            .iter()
            .map(|d| {
                let j = fixed.beat_near(*d).unwrap_or(0);
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
            ..fixed
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
    fn repair_restores_the_fast_pulse() {
        // 142 BPM: модель то отдаёт каждую долю, то через одну (куплет — 840 мс, припев —
        // 420), сильные доли — то каждый такт, то через такт; плюс лишняя доля посреди такта
        let truth = synthetic(142.0, 4 * 64, 100.0);
        let mut a = truth.clone();
        let b = &truth.beats_ms;
        a.beats_ms = b
            .iter()
            .enumerate()
            .filter(|(i, _)| (i / 64) % 2 == 1 || i % 2 == 0)
            .map(|(_, t)| *t)
            .collect();
        a.beats_ms.push(b[100] + 130.0);
        a.beats_ms.sort_by(f64::total_cmp);
        a.downbeats_ms = truth
            .downbeats_ms
            .iter()
            .enumerate()
            .filter(|(i, _)| (i / 16) % 2 == 1 || i % 2 == 0)
            .map(|(_, t)| *t)
            .collect();
        a.bpm_confidence = Some(0.5);
        let raw = a.grid().unwrap();
        assert!(raw.regularity < 0.85, "{}", raw.regularity);

        let r = a.repaired();
        assert_eq!(r.beats_ms.len(), b.len());
        let worst = r
            .beats_ms
            .iter()
            .zip(b)
            .map(|(x, t)| (x - t).abs())
            .fold(0.0, f64::max);
        assert!(worst < 1e-6, "{worst}");
        assert_eq!(r.downbeats_ms, truth.downbeats_ms);
        let g = r.grid().unwrap();
        assert!((g.bpm - 142.0).abs() < 0.01 && g.regularity > 0.99);
        assert_eq!(g.beats_per_bar, 4);
        assert!(g.confidence > 0.7, "{}", g.confidence);
        // Ровную сетку починка не трогает
        assert_eq!(truth.repaired().beats_ms, truth.beats_ms);
    }

    #[test]
    fn spurious_beat_loses_to_the_real_one() {
        // Как у «ящиков»: 3360, 3620 (лишняя), 3840 — остаётся 3840
        let mut a = synthetic(125.0, 64, 0.0);
        a.beats_ms.push(3620.0);
        a.beats_ms.sort_by(f64::total_cmp);
        let r = a.repaired();
        assert!(r.beats_ms.contains(&3840.0) && !r.beats_ms.contains(&3620.0));
        assert_eq!(r.beats_ms.len(), 64);
    }

    #[test]
    fn music_ends_before_the_fade_out() {
        let mut a = synthetic(120.0, 4 * 50, 0.0); // 100 с
        for (s, e) in a.energy_curve.iter_mut().enumerate() {
            if s >= 80 {
                *e = 600.0 * (100 - s) as f64 / 21.0;
            }
        }
        let end = a.music_end();
        assert!((88_000.0..=92_000.0).contains(&end), "{end}");
        assert_eq!(a.outro_start(), end);
        a.outro_start_ms = Some(70_000.0);
        assert_eq!(a.outro_start(), 70_000.0);
        // Без кривой — конец трека
        a.energy_curve.clear();
        assert_eq!(a.music_end(), a.duration());
        // Местная ровность
        let s = synthetic(120.0, 200, 0.0);
        assert!(s.regularity_between(10_000.0, 60_000.0) > 0.99);
        assert_eq!(s.regularity_between(0.0, 2000.0), 0.0);
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
