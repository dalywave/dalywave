//! Выбор перехода для пары треков.
//!
//! Как у диджея: уходящий уводится на границе фразы в своей концовке, а не там, где он уже
//! затих. Концовка — начало концовки из анализа или, если её нет, последние такты перед концом
//! музыки ([`Analysis::music_end`]: дальше затухание и тишина). Сведение по долям (смешивание со
//! сменой баса, фильтр) подбирается так, чтобы входящий полностью принял трек к концу музыки
//! уходящего; эхо-выход и резкая смена — на границе фразы в начале концовки. Входящий входит с
//! начала своей фразы на вступлении (для резкой смены — ещё и с конца вступления, «в дроп»).
//!
//! Перебираются границы фраз уходящего, начала фраз входящего, приёмы и длины (4–32 такта). У
//! каждого варианта — оценка: длина под «выразительность», совместимость тональностей, скачок
//! энергии, насколько выход раньше или позже концовки, сколько вступления входящего пропадает,
//! сила растяжения. Голос на голос — вариант отброшен.
//!
//! Сведение по долям растягивает входящий: каждая его доля ложится на долю уходящего, после
//! перехода скорость плавно возвращается к своей (4–16 тактов, чем сильнее растяжение, тем
//! дольше). Растяжение — не больше 8 % (с учётом половинного и двойного темпа); дальше, при
//! разном размере такта или ненадёжной сетке входящего — эхо-выход или резкая смена на сильную
//! долю: им нужна только сетка уходящего.
//!
//! Страховка — «Умный» ([`Technique::Fade`]): равномощное наложение у конца музыки, без
//! растяжения. Только если у уходящего нет надёжной сетки долей (нет анализа полного файла,
//! модель не уверена, доли в концовке идут неровно) или для перехода нигде нет места. Причина —
//! в [`Plan::note`] (движок пишет её в журнал).

use std::fmt;

use crate::analysis::{Analysis, BARS_PER_PHRASE, Grid};
use crate::curves::Curves;
use crate::key;
use crate::tempo::TempoMap;

/// Темпы расходятся больше — не сводим по долям.
pub const MAX_TEMPO_GAP: f64 = 0.08;
/// Уверенность в сетке долей ниже — не сводим по долям.
pub const MIN_CONFIDENCE: f64 = 0.5;
/// Доли идут неровнее — живое исполнение, сетка не держит.
pub const MIN_REGULARITY: f64 = 0.85;
/// Наложение «Умного», мс (как на сайте).
pub const SMART_FADE_MS: f64 = 4000.0;
/// Где проверяется ровность долей: столько от конца музыки уходящего и от начала входящего.
const CHECK_WINDOW_MS: f64 = 90_000.0;

/// За сколько тактов скорость входящего возвращается к своей: по 2 такта на процент растяжения,
/// от 4 до 16 — заметного «уплывания» темпа нет.
pub fn ramp_bars(ratio: f64) -> u32 {
    ((ratio - 1.0).abs() * 200.0).round().clamp(4.0, 16.0) as u32
}

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

/// «Умный»: после вокала уходящего, у конца его музыки (не в хвосте затухания), на звуке
/// входящего. `note` — почему не по долям.
pub fn smart(input: &PlanInput<'_>, note: impl Into<String>) -> Plan {
    let out = input.out;
    let dur = input.out_duration_ms.unwrap_or_else(|| out.duration());
    let end = if out.energy_curve.is_empty() {
        dur
    } else {
        out.music_end().min(dur)
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
    let note = format!("страховка «Умный»: {}", note.into());
    Plan::crossfade(start, in_start, length, note)
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
    /// Сетка входящего, если она есть (для сведения по долям — ещё и надёжная).
    gi: Option<Grid>,
    /// Можно ли сводить по долям.
    sync: bool,
    m: f64,
    x: f64,
    key: f64,
    /// Конец музыки уходящего: дальше затухание.
    out_end: f64,
    out_dur: f64,
    /// Начало концовки уходящего.
    outro: f64,
    in_first: f64,
    in_phrases: Vec<f64>,
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
    let out_dur = input.out_duration_ms.unwrap_or_else(|| out.duration());
    let out_end = out.music_end().min(out_dur);

    // Уходящий: без его сетки нет ни границ фраз, ни сильных долей для выхода
    let Some(go) = out.grid() else {
        return smart(
            input,
            "у уходящего нет сетки долей (полный файл ещё не разобран)",
        );
    };
    if go.confidence < MIN_CONFIDENCE {
        return smart(
            input,
            format!("сетка долей уходящего неуверенная ({:.2})", go.confidence),
        );
    }
    let out_reg = out.regularity_between(out_end - CHECK_WINDOW_MS, out_end);
    if out_reg < MIN_REGULARITY {
        return smart(
            input,
            format!("доли уходящего в концовке идут неровно ({out_reg:.2})"),
        );
    }

    // Входящий: надёжная сетка нужна только для сведения по долям
    let in_first = inc.audible().0;
    let gi = inc.grid();
    let mut m = 1.0;
    let no_sync: Option<String> = match gi {
        None => Some("у входящего нет сетки долей".into()),
        Some(gi) => {
            let in_reg = inc.regularity_between(in_first, in_first + CHECK_WINDOW_MS);
            let (mm, ratio) = tempo_match(&go, &gi);
            m = mm;
            if gi.confidence < MIN_CONFIDENCE {
                Some(format!(
                    "сетка долей входящего неуверенная ({:.2})",
                    gi.confidence
                ))
            } else if in_reg < MIN_REGULARITY {
                Some(format!(
                    "доли входящего во вступлении идут неровно ({in_reg:.2})"
                ))
            } else if go.beats_per_bar != gi.beats_per_bar {
                Some(format!(
                    "разный размер такта ({} и {})",
                    go.beats_per_bar, gi.beats_per_bar
                ))
            } else if (ratio - 1.0).abs() > MAX_TEMPO_GAP {
                Some(format!(
                    "темпы {:.1} и {:.1} BPM расходятся на {:.0} %",
                    go.bpm,
                    gi.bpm,
                    (ratio - 1.0).abs() * 100.0
                ))
            } else {
                None
            }
        }
    };
    let ctx = Ctx {
        input,
        go,
        gi,
        sync: no_sync.is_none(),
        m,
        x,
        key: key::compat(out.key_camelot.as_deref(), inc.key_camelot.as_deref()),
        out_end,
        out_dur,
        outro: out.outro_start().min(out_end),
        in_first,
        in_phrases: if gi.is_some() {
            inc.phrases().starts
        } else {
            Vec::new()
        },
    };

    // Вход: первая сильная доля со звуком, первые две границы фраз и (для резкой смены и
    // эхо-выхода) начало фразы у конца вступления. Без сетки — первая доля со звуком.
    let in_points: Vec<f64> = match gi {
        Some(gi) => {
            let audible = |s: &f64| *s + gi.beat_ms >= in_first;
            let mut v: Vec<f64> = inc
                .downbeats_ms
                .iter()
                .copied()
                .find(audible)
                .into_iter()
                .collect();
            let mut add = |s: f64| {
                if v.iter().all(|x| (x - s).abs() > 1.0) {
                    v.push(s);
                }
            };
            for s in ctx.in_phrases.iter().copied().filter(audible).take(2) {
                add(s);
            }
            let bar = gi.bar_ms();
            if let Some(drop) = inc
                .intro_end_ms
                .filter(|ie| *ie > in_first + 4.0 * bar)
                .and_then(|ie| {
                    ctx.in_phrases
                        .iter()
                        .copied()
                        .filter(|s| (s - ie).abs() <= 2.0 * bar)
                        .min_by(|a, b| (a - ie).abs().total_cmp(&(b - ie).abs()))
                })
            {
                add(drop);
            }
            v
        }
        None => vec![
            inc.beats_ms
                .iter()
                .copied()
                .find(|b| *b >= in_first - 50.0)
                .unwrap_or(in_first)
                .max(0.0),
        ],
    };
    // Выход: границы фраз уходящего во второй части трека, не позже чем за 2 такта до конца
    // музыки, и середины фраз (4 такта): в медленном треке фраза длится полминуты, и до
    // ближайшей границы иначе пришлось бы уходить слишком рано
    let bar = go.bar_ms();
    let half = BARS_PER_PHRASE / 2;
    let out_points: Vec<(f64, bool)> = out
        .phrases()
        .starts
        .into_iter()
        .flat_map(|p| {
            let i = out.downbeats_ms.partition_point(|d| *d < p - 1.0);
            [
                Some((p, true)),
                out.downbeats_ms.get(i + half).map(|d| (*d, false)),
            ]
        })
        .flatten()
        .filter(|(p, _)| {
            *p >= out_dur * 0.35 && *p >= out_end - 180_000.0 && *p <= out_end - 2.0 * bar
        })
        .collect();

    let mut candidates: Vec<(f64, Plan)> = Vec::new();
    for &(p, phrase) in &out_points {
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
                candidates.extend(
                    ctx.candidate(p, s, technique, bars)
                        .map(|(score, plan)| (score - if phrase { 0.0 } else { 0.04 }, plan)),
                );
            }
        }
    }
    // Длина наложения — пожелание выразительности, но не длиннее, чем вообще помещается (голос
    // уходящего до конца и короткое вступление входящего оставляют место на 4–8 тактов)
    let pref: f64 = if x < 0.34 {
        32.0
    } else if x < 0.67 {
        16.0
    } else {
        8.0
    };
    let longest = candidates
        .iter()
        .filter(|(_, p)| p.is_beatmatched())
        .map(|(_, p)| f64::from(p.bars))
        .fold(0.0, f64::max);
    let pref = if longest > 0.0 {
        pref.min(longest)
    } else {
        pref
    };
    let best = candidates
        .into_iter()
        .map(|(score, plan)| {
            let len = if plan.is_beatmatched() {
                0.12 * (f64::from(plan.bars) / pref).log2().abs()
            } else {
                0.0
            };
            (score - len, plan)
        })
        .fold(None::<(f64, Plan)>, |best, c| match best {
            Some(b) if b.0 >= c.0 => Some(b),
            _ => Some(c),
        });
    match best {
        Some((_, mut plan)) => {
            if let Some(why) = no_sync {
                plan.note.push_str(&format!("; по долям не сводим: {why}"));
            }
            plan
        }
        None => smart(input, "в концовке нет места для перехода по долям"),
    }
}

impl Ctx<'_> {
    fn candidate(&self, p: f64, s: f64, technique: Technique, bars: u32) -> Option<(f64, Plan)> {
        let (out, inc) = (self.input.out, self.input.inc);
        let go = &self.go;
        let bpb = go.beats_per_bar as usize;
        let jp = out.beat_near(p)?;
        let x = self.x;
        let bar = go.bar_ms();

        let (start, in_offset, length, tempo, ratio, swap_ms, s) = match technique {
            Technique::Blend | Technique::Filter => {
                if !self.sync {
                    return None;
                }
                let gi = self.gi.as_ref()?;
                let is = inc.beat_near(s)?;
                let s = inc.beats_ms[is];
                let k_total = bars as usize * bpb;
                let start = out.beats_ms[jp];
                let length = out.beat_at((jp + k_total) as f64, go) - start;
                // Наложение не уходит в затухание уходящего и тем более за конец файла
                if start + length > (self.out_end + 2.0 * bar).min(self.out_dur) {
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
                let n = (ramp_bars(r_end) * gi.beats_per_bar) as usize;
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
                (start, 0.0, length, TempoMap::new(knots), ratio, swap, s)
            }
            Technique::Echo => {
                if jp < bpb {
                    return None;
                }
                let start = out.beats_ms[jp - bpb];
                let cut = out.beats_ms[jp] - start;
                (start, cut, cut + 10.0, TempoMap::identity(s), 1.0, cut, s)
            }
            Technique::Cut => (
                out.beats_ms[jp],
                0.0,
                10.0,
                TempoMap::identity(s),
                1.0,
                0.0,
                s,
            ),
            Technique::Fade => return None,
        };
        if start < self.input.earliest_ms {
            return None;
        }
        let beatmatched = matches!(technique, Technique::Blend | Technique::Filter);
        let cut_at = start + in_offset;
        if !beatmatched && cut_at > self.out_end - 2.0 * bar {
            return None;
        }
        // Оборвать посреди вокала грубо, но рэп часто поёт до последней секунды: тогда эхо-выход
        // на границе фразы — лучшее, что есть, поэтому штраф, а не запрет
        let cut_in_vocals = !beatmatched && out.vocal_at(cut_at + 1.0);
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

        // Оценка (без длины наложения — её учитывает `plan`, когда известны все варианты)
        let in_bar = self.gi.map_or(bar, |g| g.bar_ms());
        let e_in = inc.energy_mean(s, s + 8.0 * in_bar);
        let e_out = out.energy_mean(cut_at - 8.0 * bar, cut_at);
        let de = match (e_in, e_out) {
            (Some(a), Some(b)) => a - b,
            _ => 0.0,
        };
        let energetic = e_in.is_some_and(|e| e >= 0.6) && e_out.is_some_and(|e| e >= 0.6);
        let k = self.key;
        let stretch = (ratio - 1.0).abs();
        // Несовместимые тональности режут слух тем сильнее, чем дольше звучат вместе
        let clash = (1.0 - k) * (f64::from(bars) / 16.0).min(1.0);
        // Сведение по долям — основа «Диджея»; эхо-выход и резкая смена — с выразительностью,
        // при несовместимой тональности и при скачке энергии
        let mut score = match technique {
            Technique::Blend => {
                0.45 + 0.3 * (1.0 - x) - 0.3 * clash - 0.15 * de.abs() - 2.0 * stretch
            }
            Technique::Filter => {
                0.3 + 0.25 * x
                    - 0.1 * clash
                    - 0.1 * de.abs()
                    - 2.0 * stretch
                    - if x < 0.25 { 0.2 } else { 0.0 }
            }
            Technique::Echo => {
                0.05 + 0.35 * x + 0.2 * (-de - 0.1).max(0.0) + 0.15 * (1.0 - k)
                    // Эхо-выход заметен — для «незаметного» он последнее средство
                    - if x < 0.34 { 0.25 } else { 0.0 }
            }
            Technique::Cut => {
                -0.05
                    + 0.4 * x
                    + 0.4 * (de - 0.15).max(0.0)
                    + 0.1 * (1.0 - k)
                    + if energetic { 0.15 } else { 0.0 }
                    - if x < 0.5 { 0.3 } else { 0.0 }
            }
            Technique::Fade => 0.0,
        };
        if cut_in_vocals {
            score -= match technique {
                Technique::Echo => 0.15,
                _ => 0.3,
            };
        }
        // Где выходить. Сведение по долям: входящий принимает трек к концу музыки уходящего,
        // начало — не раньше концовки (или конца музыки минус длина наложения). Эхо и резкая
        // смена: на начале концовки, но не ближе 4 тактов к концу музыки.
        // Штраф — за секунды: столько «тела» трека пропадает или столько звучит затухание.
        let (early, late) = if beatmatched {
            let target = self.outro.min(self.out_end - length);
            (
                (target - start).max(0.0),
                0.03 * (start + length - self.out_end).max(0.0),
            )
        } else {
            let target = self.outro.min(self.out_end - 4.0 * bar);
            (
                (target - cut_at).max(0.0),
                0.005 * (cut_at - target).max(0.0),
            )
        };
        score -= (0.008 * early + late) / 1000.0;
        // Вступление входящего: пропущенное — жаль, вход с границы фразы — фразы совпадают
        let skipped_in = (s - self.in_first).max(0.0) / 1000.0;
        score -= 0.004 * skipped_in;
        if self.in_phrases.iter().any(|q| (q - s).abs() < 1.0) {
            score += 0.03;
        }

        let bar_no = out.downbeats_ms.partition_point(|d| *d < start - 1.0) + 1;
        let note = format!(
            "{technique} {bars} т. с {:.1} с уходящего (такт {bar_no}, концовка {:.1} с, музыка до \
             {:.1} с) на {:.1} с входящего, темп ×{ratio:.3}, тональности {:.1}, энергия {de:+.2}",
            start / 1000.0,
            self.outro / 1000.0,
            self.out_end / 1000.0,
            s.max(0.0) / 1000.0,
            k
        );
        Some((score, Plan { note, ..plan }))
    }

    /// Голос на голос: в какой-то момент оба трека ясно слышны (не тише −6 дБ) и в обоих поют.
    /// Миллисекунды самой смены (у эхо-выхода и резкой смены уходящий гаснет за 8 мс, а входящий
    /// открывается за 3) — не наложение: шаг проверки мог попасть прямо в них.
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
            if t >= plan.in_offset_ms + 20.0
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
        // Темпы далеко — не страховка, а эхо-выход или резкая смена на сильной доле: им сетка
        // входящего не нужна
        let out = synthetic(124.0, 4 * 96, 0.0);
        let far = synthetic(100.0, 4 * 96, 0.0);
        let p = plan(&input(&out, &far, 0.5));
        assert!(
            matches!(p.technique, Technique::Echo | Technique::Cut),
            "{}",
            p.note
        );
        assert!(p.note.contains("расходятся"), "{}", p.note);
        assert!(p.tempo.is_identity());
        let g = out.grid().unwrap();
        assert!(out.is_downbeat(p.in_at_ms(), &g), "{}", p.note);

        // Страховка — только когда у уходящего нет надёжной сетки
        let mut shaky = synthetic(124.0, 4 * 96, 0.0);
        shaky.bpm_confidence = Some(0.3);
        let p = plan(&input(&shaky, &out, 0.5));
        assert_eq!(p.technique, Technique::Fade);
        assert!(p.note.starts_with("страховка «Умный»"), "{}", p.note);

        let empty = Analysis::default();
        let p = plan(&input(&empty, &out, 0.5));
        assert_eq!(p.technique, Technique::Fade);
        assert_eq!(
            p.length_ms, 500.0,
            "длины нет — короткое наложение в начале"
        );
    }

    #[test]
    fn exits_on_a_phrase_before_the_fade_out() {
        // 124 BPM, 192 с; последние 24 с — затухание, концовка (по анализу) — со 160 с
        let mut out = synthetic(124.0, 4 * 99, 0.0);
        let n = out.energy_curve.len();
        for (s, e) in out.energy_curve.iter_mut().enumerate() {
            if s + 24 >= n {
                *e = 600.0 * (n - s) as f64 / 25.0;
            }
        }
        out.outro_start_ms = Some(160_000.0);
        let end = out.music_end();
        assert!(end < out.duration() - 8000.0, "{end}");
        let inc = synthetic(124.0, 4 * 96, 0.0);
        let bar = 4.0 * 60_000.0 / 124.0;
        for x in [0.1, 0.5, 0.9] {
            let p = plan(&input(&out, &inc, x));
            assert_ne!(p.technique, Technique::Fade, "{}", p.note);
            // Уходящий отдаёт трек до затухания, а не в нём
            let heard = if p.is_beatmatched() {
                p.start_ms + p.length_ms
            } else {
                p.in_at_ms()
            };
            assert!(heard <= end + 2.0 * bar, "{x}: {}", p.note);
            // и не раньше чем за 32 такта до конца музыки
            assert!(heard >= end - 32.0 * bar, "{x}: {}", p.note);
            // Начало — на сильной доле, на границе фразы или середине фразы
            let edge = if p.is_beatmatched() {
                p.start_ms
            } else {
                p.in_at_ms()
            };
            let phrases = out.refined().phrases();
            assert!(
                phrases
                    .starts
                    .iter()
                    .any(|q| (q - edge).abs() < 1.0 || (q + 4.0 * bar - edge).abs() < 1.0),
                "{x}: {}",
                p.note
            );
        }
    }

    #[test]
    fn ramp_grows_with_stretch() {
        assert_eq!(ramp_bars(1.0), 4);
        assert_eq!(ramp_bars(1.01), 4);
        assert_eq!(ramp_bars(0.96), 8);
        assert_eq!(ramp_bars(1.08), 16);
        assert_eq!(ramp_bars(1.2), 16);
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
