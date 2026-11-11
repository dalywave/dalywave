//! Выбор перехода для пары треков.
//!
//! Перебираются границы фраз уходящего (вторая половина трека, после вокала — проверкой «голос на
//! голос»), начала фраз входящего (первая фраза, где уже есть звук), приёмы и длины. У каждого
//! варианта — оценка: длина под «выразительность», совместимость тональностей, скачок энергии,
//! сколько музыки пропадает с конца уходящего и с начала входящего, сила растяжения. Голос на
//! голос — вариант отброшен.
//!
//! Сведение по долям (смешивание со сменой баса, фильтр) растягивает входящий: каждая его доля
//! ложится на долю уходящего, после перехода скорость за 4 такта возвращается к своей. Эхо-выход и
//! резкая смена темп не трогают: уходящий обрывается на сильной доле, и с неё же звучит входящий.
//!
//! Страховка — «Умный» ([`Technique::Fade`]): равномощное наложение после вокала, без растяжения.
//! Он выбирается, если сетки долей нет или модель в ней не уверена, если доли неровные (живое
//! исполнение), если размер такта разный или темпы расходятся больше чем на 8 % (с учётом
//! «вдвое»), и если для перехода нигде нет места.

use std::fmt;

use crate::analysis::{Analysis, Grid};
use crate::curves::Curves;
use crate::key;
use crate::tempo::TempoMap;

/// Темпы расходятся больше — не сводим по долям.
pub const MAX_TEMPO_GAP: f64 = 0.08;
/// Уверенность модели в сетке долей ниже — не сводим по долям.
pub const MIN_CONFIDENCE: f64 = 0.5;
/// Доли идут неровнее — живое исполнение, сетка не держит.
pub const MIN_REGULARITY: f64 = 0.85;
/// Наложение «Умного», мс (как на сайте).
pub const SMART_FADE_MS: f64 = 4000.0;
/// За сколько тактов скорость входящего возвращается к своей.
pub const RAMP_BARS: u32 = 4;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Technique {
    /// Сведение по долям со сменой баса на сильной доле.
    Blend,
    /// Входящий открывается фильтром низких частот, уходящий уходит в фильтр высоких.
    Filter,
    /// Эхо-выход: уходящий отправляется в эхо и обрывается на сильной доле.
    Echo,
    /// Резкая смена на сильную долю.
    Cut,
    /// «Умный»: наложение без сведения по долям.
    Fade,
}

impl Technique {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Blend => "blend",
            Self::Filter => "filter",
            Self::Echo => "echo",
            Self::Cut => "cut",
            Self::Fade => "fade",
        }
    }
}

impl fmt::Display for Technique {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

pub struct PlanInput<'a> {
    pub out: &'a Analysis,
    pub inc: &'a Analysis,
    /// Точная длина треков (из декодера), если известна; иначе — из анализа.
    pub out_duration_ms: Option<f64>,
    pub in_duration_ms: Option<f64>,
    /// 0 — незаметно (длинное смешивание), 1 — эффектно (короткие приёмы, эхо, резкая смена).
    pub expressiveness: f64,
    /// Раньше этой позиции уходящего переход не начинать: уже прозвучало или входящий не успеет
    /// подготовиться.
    pub earliest_ms: f64,
}

/// Готовый переход. Времена `start_ms` — в уходящем треке, остальные — от начала перехода.
#[derive(Clone, Debug, PartialEq)]
pub struct Plan {
    pub technique: Technique,
    /// Позиция уходящего, с которой начинается автоматика.
    pub start_ms: f64,
    /// Входящий стартует через столько после начала перехода.
    pub in_offset_ms: f64,
    /// Позиция входящего в момент его старта (сильная доля, начало фразы).
    pub in_start_ms: f64,
    /// Длина автоматики; после неё уходящий молчит.
    pub length_ms: f64,
    pub swap_ms: f64,
    pub bars: u32,
    /// Доля уходящего, мс.
    pub beat_ms: f64,
    /// Карта темпа входящего от его старта (без растяжения — тождественная).
    pub tempo: TempoMap,
    /// Скорость входящего во время наложения: 1 — без растяжения.
    pub ratio: f64,
    pub expressiveness: f64,
    pub echo_delay_ms: f64,
    pub echo_feedback: f64,
    /// Почему так: для журнала.
    pub note: String,
}

impl Plan {
    /// Наложение без сведения по долям: «Умный» и движок без анализа.
    pub fn crossfade(start_ms: f64, in_start_ms: f64, length_ms: f64, note: String) -> Plan {
        Plan {
            technique: Technique::Fade,
            start_ms,
            in_offset_ms: 0.0,
            in_start_ms,
            length_ms: length_ms.max(1.0),
            swap_ms: length_ms / 2.0,
            bars: 0,
            beat_ms: 500.0,
            tempo: TempoMap::identity(in_start_ms),
            ratio: 1.0,
            expressiveness: 0.0,
            echo_delay_ms: 375.0,
            echo_feedback: 0.0,
            note,
        }
    }

    pub fn curves(&self) -> Curves {
        Curves {
            technique: self.technique,
            length_ms: self.length_ms,
            in_offset_ms: self.in_offset_ms,
            swap_ms: self.swap_ms,
            beat_ms: self.beat_ms,
            expressiveness: self.expressiveness,
            echo_delay_ms: self.echo_delay_ms,
            echo_feedback: self.echo_feedback,
        }
    }

    /// Позиция уходящего, когда стартует входящий.
    pub fn in_at_ms(&self) -> f64 {
        self.start_ms + self.in_offset_ms
    }

    pub fn is_beatmatched(&self) -> bool {
        matches!(self.technique, Technique::Blend | Technique::Filter)
    }
}

/// «Умный»: после вокала уходящего, без тишины и длинного хвоста, на звуке входящего.
pub fn smart(input: &PlanInput<'_>, note: impl Into<String>) -> Plan {
    let out = input.out;
    let dur = input.out_duration_ms.unwrap_or_else(|| out.duration());
    let (_, audible_end) = out.audible();
    let end = if audible_end > 0.0 {
        audible_end.min(dur)
    } else {
        dur
    };
    let mut start = end - SMART_FADE_MS;
    let vocals = out.vocal_end();
    if vocals > start && vocals < end - 1500.0 {
        start = vocals;
    }
    start = start.max(input.earliest_ms);
    let length = (end - start).clamp(500.0, SMART_FADE_MS);
    let (first, _) = input.inc.audible();
    let in_start = input
        .inc
        .beats_ms
        .iter()
        .copied()
        .find(|b| *b >= first - 50.0)
        .unwrap_or(first)
        .max(0.0);
    Plan::crossfade(start, in_start, length, note.into())
}

/// Сколько входящих долей на одну долю уходящего (1, 2 или ½) и скорость растяжения.
fn tempo_match(go: &Grid, gi: &Grid) -> (f64, f64) {
    [1.0, 2.0, 0.5]
        .into_iter()
        .map(|m| (m, gi.beat_ms * m / go.beat_ms))
        .min_by(|a, b| (a.1 - 1.0).abs().total_cmp(&(b.1 - 1.0).abs()))
        .unwrap_or((1.0, 1.0))
}

struct Ctx<'a> {
    input: &'a PlanInput<'a>,
    go: Grid,
    gi: Grid,
    m: f64,
    x: f64,
    key: f64,
    out_end: f64,
    in_first: f64,
}

pub fn plan(input: &PlanInput<'_>) -> Plan {
    let (out, inc) = (input.out.refined(), input.inc.refined());
    let input = &PlanInput {
        out: &out,
        inc: &inc,
        ..*input
    };
    let (out, inc) = (input.out, input.inc);
    let x = input.expressiveness.clamp(0.0, 1.0);
    let (Some(go), Some(gi)) = (out.grid(), inc.grid()) else {
        return smart(input, "нет сетки долей");
    };
    if go.confidence < MIN_CONFIDENCE || gi.confidence < MIN_CONFIDENCE {
        return smart(
            input,
            format!(
                "сетка долей неуверенная ({:.2} → {:.2})",
                go.confidence, gi.confidence
            ),
        );
    }
    if go.regularity < MIN_REGULARITY || gi.regularity < MIN_REGULARITY {
        return smart(input, "доли идут неровно");
    }
    if go.beats_per_bar != gi.beats_per_bar {
        return smart(input, "разный размер такта");
    }
    let (m, ratio) = tempo_match(&go, &gi);
    if (ratio - 1.0).abs() > MAX_TEMPO_GAP {
        return smart(
            input,
            format!(
                "темпы {:.1} и {:.1} BPM расходятся на {:.0} %",
                go.bpm,
                gi.bpm,
                (ratio - 1.0).abs() * 100.0
            ),
        );
    }
    let out_dur = input.out_duration_ms.unwrap_or_else(|| out.duration());
    let (_, out_audible) = out.audible();
    let ctx = Ctx {
        input,
        go,
        gi,
        m,
        x,
        key: key::compat(out.key_camelot.as_deref(), inc.key_camelot.as_deref()),
        out_end: out_audible.min(out_dur),
        in_first: inc.audible().0,
    };

    // Вход: первая сильная доля со звуком (начало вступления — самое длинное место без голоса)
    // и первые две границы фраз
    let in_points: Vec<f64> = {
        let audible = |s: &f64| *s + gi.beat_ms >= ctx.in_first;
        let mut v: Vec<f64> = inc
            .downbeats_ms
            .iter()
            .copied()
            .find(audible)
            .into_iter()
            .collect();
        for s in inc.phrases().starts.into_iter().filter(audible).take(2) {
            if v.iter().all(|x| (x - s).abs() > 1.0) {
                v.push(s);
            }
        }
        v
    };
    let out_points: Vec<f64> = out
        .phrases()
        .starts
        .into_iter()
        .filter(|p| *p >= out_dur * 0.4 && *p >= out_dur - 150_000.0 && *p <= ctx.out_end)
        .collect();

    let mut best: Option<(f64, Plan)> = None;
    for &p in &out_points {
        for &s in &in_points {
            for (technique, bars) in [
                (Technique::Blend, 32),
                (Technique::Blend, 16),
                (Technique::Blend, 8),
                (Technique::Blend, 4),
                (Technique::Filter, 32),
                (Technique::Filter, 16),
                (Technique::Filter, 8),
                (Technique::Filter, 4),
                (Technique::Echo, 1),
                (Technique::Cut, 0),
            ] {
                if let Some((score, plan)) = ctx.candidate(p, s, technique, bars)
                    && best.as_ref().is_none_or(|(b, _)| score > *b)
                {
                    best = Some((score, plan));
                }
            }
        }
    }
    match best {
        Some((_, plan)) => plan,
        None => smart(input, "после вокала нет места для перехода по долям"),
    }
}

impl Ctx<'_> {
    fn candidate(&self, p: f64, s: f64, technique: Technique, bars: u32) -> Option<(f64, Plan)> {
        let (out, inc) = (self.input.out, self.input.inc);
        let (go, gi) = (&self.go, &self.gi);
        let bpb = go.beats_per_bar as usize;
        let jp = out.beat_near(p)?;
        let is = inc.beat_near(s)?;
        let s = inc.beats_ms[is];
        let x = self.x;

        let (start, in_offset, length, tempo, ratio, swap_ms) = match technique {
            Technique::Blend | Technique::Filter => {
                let k_total = bars as usize * bpb;
                let start = out.beats_ms[jp];
                let length = out.beat_at((jp + k_total) as f64, go) - start;
                if start + length > self.out_end + go.beat_ms {
                    return None;
                }
                let mut knots: Vec<(f64, f64)> = (0..=k_total)
                    .map(|k| {
                        (
                            out.beat_at((jp + k) as f64, go) - start,
                            inc.beat_at(is as f64 + k as f64 * self.m, gi),
                        )
                    })
                    .collect();
                let ratio = (knots[k_total].1 - knots[0].1) / length;
                let (t1, s1) = knots[k_total];
                let (t0, s0) = knots[k_total - 1];
                let r_end = (s1 - s0) / (t1 - t0);
                // Возврат к своей скорости — по долям входящего, скорость меняется линейно
                let n = (RAMP_BARS * gi.beats_per_bar) as usize;
                let mut idx = is as f64 + k_total as f64 * self.m;
                let (mut t, mut prev) = (t1, r_end);
                for q in 1..=n {
                    let rate = r_end + (1.0 - r_end) * q as f64 / n as f64;
                    let a = inc.beat_at(idx, gi);
                    let b = inc.beat_at(idx + 1.0, gi);
                    t += (b - a) / ((prev + rate) / 2.0);
                    prev = rate;
                    idx += 1.0;
                    knots.push((t, b));
                }
                let swap_beat = (bars as usize / 2) * bpb;
                let swap = out.beat_at((jp + swap_beat) as f64, go) - start;
                (start, 0.0, length, TempoMap::new(knots), ratio, swap)
            }
            Technique::Echo => {
                if jp < bpb {
                    return None;
                }
                let start = out.beats_ms[jp - bpb];
                let cut = out.beats_ms[jp] - start;
                (start, cut, cut + 10.0, TempoMap::identity(s), 1.0, cut)
            }
            Technique::Cut => (out.beats_ms[jp], 0.0, 10.0, TempoMap::identity(s), 1.0, 0.0),
            Technique::Fade => return None,
        };
        if start < self.input.earliest_ms {
            return None;
        }
        let cut_at = start + in_offset;
        // Оборвать посреди вокала грубо, но рэп часто поёт до последней секунды: тогда эхо-выход
        // на границе фразы — лучшее, что есть, поэтому штраф, а не запрет
        let cut_in_vocals =
            matches!(technique, Technique::Echo | Technique::Cut) && out.vocal_at(cut_at + 1.0);
        if matches!(technique, Technique::Echo | Technique::Cut) && cut_at > self.out_end {
            return None;
        }
        let plan = Plan {
            technique,
            start_ms: start,
            in_offset_ms: in_offset,
            in_start_ms: s,
            length_ms: length,
            swap_ms,
            bars,
            beat_ms: go.beat_ms,
            tempo,
            ratio,
            expressiveness: x,
            echo_delay_ms: go.beat_ms * 0.75,
            echo_feedback: 0.45 + 0.15 * x,
            note: String::new(),
        };
        if self.voices_clash(&plan) {
            return None;
        }

        // Оценка
        let pref = if x < 0.34 {
            32.0
        } else if x < 0.67 {
            16.0
        } else {
            8.0
        };
        let bar = go.bar_ms();
        let de = match (
            inc.energy_mean(s, s + 8.0 * bar),
            out.energy_mean(cut_at - 8.0 * bar, cut_at),
        ) {
            (Some(a), Some(b)) => a - b,
            _ => 0.0,
        };
        let k = self.key;
        let stretch = (ratio - 1.0).abs();
        let mut score = match technique {
            Technique::Blend => 0.35 * k + 0.25 * (1.0 - x) - 0.3 * de.abs() - 2.0 * stretch,
            Technique::Filter => {
                0.1 + 0.15 * k + 0.3 * x
                    - 0.15 * de.abs()
                    - 2.0 * stretch
                    - if x < 0.25 { 0.2 } else { 0.0 }
            }
            Technique::Echo => {
                -0.05 + 0.35 * x + 0.3 * (-de - 0.1).max(0.0) + 0.2 * (1.0 - k)
                    // Эхо-выход заметен — для «незаметного» он последнее средство
                    - if x < 0.34 { 0.25 } else { 0.0 }
            }
            Technique::Cut => {
                -0.15 + 0.35 * x + 0.5 * (de - 0.15).max(0.0) + 0.1 * (1.0 - k)
                    - if x < 0.5 { 0.3 } else { 0.0 }
            }
            Technique::Fade => 0.0,
        };
        if plan.is_beatmatched() {
            // Длина — пожелание: короче, если длинному не хватает места после вокала
            score -= 0.12 * (f64::from(bars) / pref).log2().abs();
        }
        if cut_in_vocals {
            score -= match technique {
                Technique::Echo => 0.15,
                _ => 0.3,
            };
        }
        let heard_until = if plan.is_beatmatched() {
            start + length
        } else {
            cut_at
        };
        let skipped_out = (self.out_end - heard_until).max(0.0) / 1000.0;
        let skipped_in = (s - self.in_first).max(0.0) / 1000.0;
        score -= 0.004 * (skipped_out - 6.0).max(0.0) + 0.004 * skipped_in;

        let note = format!(
            "{technique} {bars} т. с {:.1} с уходящего на {:.1} с входящего, темп ×{ratio:.3}, \
             тональности {:.1}, энергия {de:+.2}",
            start / 1000.0,
            s / 1000.0,
            k
        );
        Some((score, Plan { note, ..plan }))
    }

    /// Голос на голос: в какой-то момент оба трека ясно слышны (не тише −6 дБ) и в обоих поют.
    fn voices_clash(&self, plan: &Plan) -> bool {
        let (out, inc) = (self.input.out, self.input.inc);
        if out.vocal_segments.is_empty() || inc.vocal_segments.is_empty() {
            return false;
        }
        let curves = plan.curves();
        let step = plan.beat_ms / 2.0;
        let mut t = 0.0;
        while t < plan.length_ms {
            let mix = curves.at(t);
            if t >= plan.in_offset_ms
                && mix.out.gain > 0.5
                && mix.inc.gain > 0.5
                && out.vocal_at(plan.start_ms + t)
                && inc.vocal_at(plan.tempo.src_at(t - plan.in_offset_ms))
            {
                return true;
            }
            t += step;
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::tests::synthetic;

    fn input<'a>(out: &'a Analysis, inc: &'a Analysis, x: f64) -> PlanInput<'a> {
        PlanInput {
            out,
            inc,
            out_duration_ms: None,
            in_duration_ms: None,
            expressiveness: x,
            earliest_ms: 0.0,
        }
    }

    #[test]
    fn blends_close_tempos_on_phrases() {
        let out = synthetic(124.0, 4 * 96, 0.0);
        let inc = synthetic(120.0, 4 * 96, 250.0);
        let p = plan(&input(&out, &inc, 0.1));
        assert_eq!(p.technique, Technique::Blend, "{}", p.note);
        assert_eq!(p.bars, 32);
        // Старт — на границе фразы уходящего, вход — с первой сильной доли входящего
        let bar = 4.0 * 60_000.0 / 124.0;
        let bars_in = (p.start_ms / bar).round();
        assert!((bars_in * bar - p.start_ms).abs() < 1e-6 && (bars_in as u32).is_multiple_of(8));
        assert_eq!(p.in_start_ms, 250.0);
        // Входящий ускорен до 124: каждая его доля — на доле уходящего
        assert!((p.ratio - 124.0 / 120.0).abs() < 1e-9);
        for k in 0..=128 {
            let t = k as f64 * 60_000.0 / 124.0;
            let src = p.tempo.src_at(t);
            assert!((src - (250.0 + k as f64 * 500.0)).abs() < 1e-6, "beat {k}");
        }
        // Смена баса — на сильной доле в середине
        assert!((p.swap_ms - 16.0 * bar).abs() < 1e-6);
        // После перехода скорость за 4 такта возвращается к единице
        assert!((p.tempo.rate_at(p.length_ms + 1.0) - p.ratio).abs() < 0.01);
        assert_eq!(p.tempo.rate_at(p.tempo.settled_at() + 1.0), 1.0);
        assert!(p.tempo.settled_at() > p.length_ms + 15.0 * 500.0);
    }

    #[test]
    fn expressiveness_shortens_and_spices() {
        let out = synthetic(124.0, 4 * 96, 0.0);
        let inc = synthetic(124.0, 4 * 96, 0.0);
        let subtle = plan(&input(&out, &inc, 0.0));
        let bold = plan(&input(&out, &inc, 1.0));
        assert!(subtle.bars > bold.bars || bold.technique != subtle.technique);
        assert_ne!(bold.technique, Technique::Fade);
    }

    #[test]
    fn half_time_is_matched() {
        let out = synthetic(140.0, 4 * 96, 0.0);
        let inc = synthetic(71.0, 4 * 64, 0.0);
        let p = plan(&input(&out, &inc, 0.2));
        assert!(p.is_beatmatched(), "{}", p.note);
        // Одна доля входящего на две доли уходящего
        let beat_out = 60_000.0 / 140.0;
        let src = p.tempo.src_at(2.0 * beat_out);
        assert!((src - 60_000.0 / 71.0).abs() < 1e-6);
    }

    #[test]
    fn falls_back_to_smart() {
        let out = synthetic(124.0, 4 * 96, 0.0);
        let far = synthetic(100.0, 4 * 96, 0.0);
        let p = plan(&input(&out, &far, 0.5));
        assert_eq!(p.technique, Technique::Fade);
        assert!(p.note.contains("расходятся"), "{}", p.note);
        assert!(p.tempo.is_identity());

        let mut shaky = synthetic(124.0, 4 * 96, 0.0);
        shaky.bpm_confidence = Some(0.3);
        assert_eq!(plan(&input(&shaky, &out, 0.5)).technique, Technique::Fade);

        let empty = Analysis::default();
        let p = plan(&input(&empty, &out, 0.5));
        assert_eq!(p.technique, Technique::Fade);
        assert_eq!(
            p.length_ms, 500.0,
            "длины нет — короткое наложение в начале"
        );
    }

    #[test]
    fn smart_waits_for_vocals_to_end() {
        let mut out = synthetic(124.0, 4 * 96, 0.0);
        let dur = out.duration();
        out.vocal_segments = vec![[10_000.0, dur - 3000.0]];
        let p = smart(&input(&out, &out, 0.5), "тест");
        assert_eq!(p.start_ms, dur - 3000.0);
        assert!((p.length_ms - 3000.0).abs() < 1e-6);
    }

    #[test]
    fn never_voice_over_voice() {
        let mut out = synthetic(124.0, 4 * 96, 0.0);
        let mut inc = synthetic(124.0, 4 * 96, 0.0);
        let dur = out.duration();
        // Уходящий поёт почти до конца, входящий — с первых секунд
        out.vocal_segments = vec![[5000.0, dur - 4.0 * 1935.0]];
        inc.vocal_segments = vec![[2000.0, 100_000.0]];
        let p = plan(&input(&out, &inc, 0.5));
        let curves = p.curves();
        let mut t = 0.0;
        while t < p.length_ms {
            let m = curves.at(t);
            let both = m.out.gain > 0.5
                && m.inc.gain > 0.5
                && t >= p.in_offset_ms
                && out.vocal_at(p.start_ms + t)
                && inc.vocal_at(p.tempo.src_at(t - p.in_offset_ms));
            assert!(!both, "{} at {t}", p.note);
            t += 100.0;
        }
    }

    #[test]
    fn rap_to_the_last_second_still_mixes() {
        // Вокал почти от начала до самого конца у обоих (так размечает сервис анализа рэп),
        // у входящего — 13 с вступления без голоса
        let mut out = synthetic(120.0, 4 * 80, 0.0);
        let mut inc = synthetic(125.0, 4 * 100, 0.0);
        out.vocal_segments = vec![[8000.0, out.duration()]];
        inc.vocal_segments = vec![[13_400.0, inc.duration()]];
        for x in [0.2, 0.5, 0.9] {
            let p = plan(&input(&out, &inc, x));
            assert_ne!(p.technique, Technique::Fade, "{x}: {}", p.note);
        }
    }

    #[test]
    fn respects_earliest_start() {
        let out = synthetic(124.0, 4 * 96, 0.0);
        let inc = synthetic(124.0, 4 * 96, 0.0);
        let mut i = input(&out, &inc, 0.3);
        i.earliest_ms = out.duration() - 20_000.0;
        let p = plan(&i);
        assert!(p.start_ms >= i.earliest_ms, "{}", p.note);
    }
}
