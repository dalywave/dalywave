//! «Собрать как сет»: порядок треков плейлиста, в котором они сводятся как DJ-сет. Соседи
//! совместимы по тональности (круг Камелота, [`key::compat`]) и близки по темпу — с учётом
//! половинного и двойного темпа и запаса растяжения, — а энергия идёт по выбранной кривой.
//!
//! Стоимость порядка — сумма стоимостей переходов между соседями и отклонений энергии от целевой
//! кривой на каждом месте ([`cost`]). Целевая кривая строится из энергий самого плейлиста: самые
//! спокойные треки получают места, где кривая ниже всего, самые энергичные — где выше, поэтому
//! кривая подходит и тихому плейлисту, и громкому.
//!
//! Поиск — эвристика задачи коммивояжёра на пути: несколько жадных стартов, затем улучшения 2-opt
//! (разворот отрезка) и or-opt (перенос отрезка до трёх треков), пока что-то улучшается. Случайности
//! нет: при одинаковом входе — одинаковый порядок, равные варианты решает исходный порядок. Если
//! исходный порядок не хуже найденного, улучшается он сам — повторная сборка ничего не меняет.
//!
//! Треки без анализа (ни темпа, ни тональности, ни энергии) не расставляются: они идут в конце в
//! прежнем порядке. Недостающие части анализа у остальных стоят нейтрально.

use crate::key;

/// Как идёт энергия по сету.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Curve {
    /// От спокойного к пику в конце.
    #[default]
    Rise,
    /// Подъём, пик около двух третей, спад.
    Wave,
    /// Без цели по энергии: как можно меньше скачков.
    Flat,
}

/// Что известно о треке для сборки. Энергии — 0…1.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SetTrack {
    pub bpm: Option<f64>,
    /// Тональность по Камелоту: «8A».
    pub key: Option<String>,
    pub key_confidence: Option<f64>,
    /// Энергия трека целиком.
    pub energy: Option<f64>,
    /// Энергия начала и конца ([`SetTrack::with_curve`]): насколько гладко стыкуются концовка и
    /// вступление соседа.
    pub head: Option<f64>,
    pub tail: Option<f64>,
    /// Основные исполнители — любые различимые номера.
    pub artists: Vec<u32>,
}

/// Сколько секунд начала и конца кривой энергии смотреть.
const EDGE_SECONDS: usize = 30;
/// Тише этого (из 1000) — тишина в начале или конце файла, не музыка.
const SILENCE: f64 = 30.0;

impl SetTrack {
    /// Есть ли что-то, по чему трек можно поставить.
    pub fn analyzed(&self) -> bool {
        self.bpm.is_some()
            || self.key.as_deref().and_then(key::camelot).is_some()
            || self.energy.is_some()
    }

    /// Начало и конец по кривой энергии полного трека (по секундам, 0…1000): средняя энергия
    /// первых и последних 30 секунд звука, без тишины по краям.
    pub fn with_curve(mut self, curve: &[f64]) -> Self {
        let from = curve.iter().position(|v| *v >= SILENCE);
        let to = curve.iter().rposition(|v| *v >= SILENCE);
        if let (Some(from), Some(to)) = (from, to) {
            let body = &curve[from..=to];
            let n = body.len().min(EDGE_SECONDS);
            if n >= 5 {
                let mean = |s: &[f64]| s.iter().sum::<f64>() / s.len() as f64 / 1000.0;
                self.head = Some(mean(&body[..n]).clamp(0.0, 1.0));
                self.tail = Some(mean(&body[body.len() - n..]).clamp(0.0, 1.0));
            }
        }
        self
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Options {
    pub curve: Curve,
    /// Индекс трека, который должен идти первым.
    pub first: Option<usize>,
    /// Не ставить подряд треки одного исполнителя.
    pub spread_artists: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            curve: Curve::Rise,
            first: None,
            spread_artists: true,
        }
    }
}

/// Как один трек переходит в следующий — для мягкой подсказки в интерфейсе.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fit {
    /// Темп близко, тональности совместимы.
    InKey,
    /// Темп близко, тональности не спорят.
    Smooth,
    /// Энергия заметно выше.
    Lift,
    /// Энергия заметно ниже.
    Calm,
    /// Смена темпа или тональности.
    Switch,
}

impl Fit {
    /// Сводится без усилия: такие переходы считаются «плавными».
    pub fn is_smooth(self) -> bool {
        matches!(self, Self::InKey | Self::Smooth)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Arrangement {
    /// Новый порядок — индексы входного списка: сначала расставленные, затем без анализа.
    pub order: Vec<usize>,
    /// Сколько первых в `order` расставлены по анализу (вместе с заданным первым).
    pub placed: usize,
    /// Стоимость исходного и нового порядка ([`cost`]): чем меньше, тем лучше сводится.
    pub cost_before: f64,
    pub cost_after: f64,
}

// ---------- Стоимость перехода ----------

/// Растяжение до этой доли незаметно на слух.
const STRETCH_EASY: f64 = 0.02;
/// До этой доли сводится растяжением без труда.
const STRETCH_OK: f64 = 0.06;
const W_KEY: f64 = 1.0;
const W_TEMPO: f64 = 1.0;
/// Скачок энергии между соседями: при кривой его направляет цель, ровно — главное условие.
const W_JUMP: f64 = 0.8;
const W_JUMP_FLAT: f64 = 2.0;
/// Шов: концовка уходящего против вступления следующего.
const W_SEAM: f64 = 0.4;
const W_ARTIST: f64 = 1.0;
/// Отклонение энергии от кривой на своём месте — в долях размаха энергий плейлиста, но размах не
/// меньше [`MIN_SPREAD`]: почти одинаковые по энергии треки кривая не растаскивает.
const W_PLACE: f64 = 1.5;
const MIN_SPREAD: f64 = 0.25;
/// Неизвестно — как средний переход.
const UNKNOWN_KEY: f64 = 0.5;
const UNKNOWN_TEMPO: f64 = 0.45;
const UNKNOWN_JUMP: f64 = 0.15;
/// Пик «Волны» — на двух третях; к концу энергия опускается на столько от пика (в долях размаха).
const WAVE_PEAK: f64 = 2.0 / 3.0;
const WAVE_DROP: f64 = 0.6;

/// Разница темпов как доля (0.05 — 5 %), с учётом половинного и двойного темпа; второе — пришлось
/// ли сводить через половинный или двойной.
pub fn tempo_gap(a: f64, b: f64) -> (f64, bool) {
    let mut best = (f64::INFINITY, false);
    for (m, folded) in [(1.0, false), (2.0, true), (0.5, true)] {
        let d = (b * m / a).ln().abs();
        if d < best.0 - 1e-12 {
            best = (d, folded);
        }
    }
    (best.0.exp() - 1.0, best.1)
}

fn tempo_cost(a: Option<f64>, b: Option<f64>) -> f64 {
    let (Some(a), Some(b)) = (a.filter(|v| *v > 0.0), b.filter(|v| *v > 0.0)) else {
        return UNKNOWN_TEMPO;
    };
    let (gap, folded) = tempo_gap(a, b);
    let p = gap * 100.0;
    let base = if gap <= STRETCH_EASY {
        p * 0.02
    } else if gap <= STRETCH_OK {
        0.04 + (p - 2.0) * 0.09
    } else {
        (0.40 + (p - 6.0) * 0.06).min(1.0)
    };
    base + if folded { 0.1 } else { 0.0 }
}

fn key_cost(a: &SetTrack, b: &SetTrack) -> f64 {
    let raw = 1.0 - key::compat(a.key.as_deref(), b.key.as_deref());
    // Неуверенная тональность — ближе к нейтральной
    let conf = a
        .key_confidence
        .unwrap_or(1.0)
        .min(b.key_confidence.unwrap_or(1.0));
    let trust = (conf / 0.4).clamp(0.0, 1.0);
    UNKNOWN_KEY + (raw - UNKNOWN_KEY) * trust
}

fn shares_artist(a: &SetTrack, b: &SetTrack) -> bool {
    a.artists.iter().any(|x| b.artists.contains(x))
}

/// Стоимость перехода `a` → `b` без учёта места в сете.
fn step_cost(a: &SetTrack, b: &SetTrack, opts: &Options) -> f64 {
    let w_jump = if opts.curve == Curve::Flat {
        W_JUMP_FLAT
    } else {
        W_JUMP
    };
    let mut c = W_KEY * key_cost(a, b) + W_TEMPO * tempo_cost(a.bpm, b.bpm);
    c += w_jump
        * match (a.energy, b.energy) {
            (Some(x), Some(y)) => (y - x).abs(),
            _ => UNKNOWN_JUMP,
        };
    if let (Some(t), Some(h)) = (a.tail, b.head) {
        c += W_SEAM * (t - h).abs();
    }
    if opts.spread_artists && shares_artist(a, b) {
        c += W_ARTIST;
    }
    c
}

/// Подсказка для перехода `a` → `b`. Без анализа у одного из двух — нет подсказки.
pub fn fit(a: &SetTrack, b: &SetTrack) -> Option<Fit> {
    if !a.analyzed() || !b.analyzed() {
        return None;
    }
    let tempo_close = match (a.bpm, b.bpm) {
        (Some(x), Some(y)) if x > 0.0 && y > 0.0 => Some(tempo_gap(x, y).0 <= STRETCH_OK),
        _ => None,
    };
    let keys = match (
        a.key.as_deref().and_then(key::camelot),
        b.key.as_deref().and_then(key::camelot),
    ) {
        (Some(_), Some(_)) => Some(key::compat(a.key.as_deref(), b.key.as_deref())),
        _ => None,
    };
    let jump = match (a.energy, b.energy) {
        (Some(x), Some(y)) => y - x,
        _ => 0.0,
    };
    Some(match (tempo_close, keys) {
        (Some(true), Some(k)) if k >= 0.8 => Fit::InKey,
        (Some(true), k) if k.is_none_or(|k| k >= 0.4) => Fit::Smooth,
        (None, Some(k)) if k >= 0.8 => Fit::Smooth,
        _ if jump >= 0.15 => Fit::Lift,
        _ if jump <= -0.15 => Fit::Calm,
        _ => Fit::Switch,
    })
}

// ---------- Целевая кривая ----------

fn shape(curve: Curve, t: f64) -> f64 {
    match curve {
        Curve::Rise => t,
        Curve::Wave if t <= WAVE_PEAK => t / WAVE_PEAK,
        Curve::Wave => 1.0 - WAVE_DROP * (t - WAVE_PEAK) / (1.0 - WAVE_PEAK),
        Curve::Flat => 0.5,
    }
}

fn quantile(sorted: &[f64], q: f64) -> f64 {
    let x = q.clamp(0.0, 1.0) * (sorted.len() - 1) as f64;
    let i = x.floor() as usize;
    let j = (i + 1).min(sorted.len() - 1);
    sorted[i] + (sorted[j] - sorted[i]) * (x - i as f64)
}

/// Целевая энергия для каждого из `m` мест: энергии плейлиста, разложенные по форме кривой
/// (места с меньшим значением формы получают меньшие энергии). «Ровно» — без цели.
pub fn targets(energies: &[f64], m: usize, curve: Curve) -> Option<Vec<f64>> {
    if curve == Curve::Flat || energies.is_empty() || m == 0 {
        return None;
    }
    let mut sorted = energies.to_vec();
    sorted.sort_by(f64::total_cmp);
    let t = |i: usize| {
        if m == 1 {
            0.0
        } else {
            i as f64 / (m - 1) as f64
        }
    };
    let mut slots: Vec<usize> = (0..m).collect();
    slots.sort_by(|&a, &b| {
        shape(curve, t(a))
            .total_cmp(&shape(curve, t(b)))
            .then(a.cmp(&b))
    });
    let mut out = vec![0.0; m];
    for (rank, &slot) in slots.iter().enumerate() {
        let q = if m == 1 {
            0.0
        } else {
            rank as f64 / (m - 1) as f64
        };
        out[slot] = quantile(&sorted, q);
    }
    Some(out)
}

// ---------- Задача ----------

/// Матрицы стоимостей для расставляемых треков (локальные номера 0..m).
struct Costs {
    m: usize,
    step: Vec<f64>,
    /// `place[x * m + i]` — трек `x` на месте `i`; пусто — кривая «Ровно».
    place: Vec<f64>,
}

impl Costs {
    fn new(tracks: &[&SetTrack], opts: &Options) -> Self {
        let m = tracks.len();
        let mut step = vec![0.0; m * m];
        for (a, ta) in tracks.iter().enumerate() {
            for (b, tb) in tracks.iter().enumerate() {
                if a != b {
                    step[a * m + b] = step_cost(ta, tb, opts);
                }
            }
        }
        let energies: Vec<f64> = tracks.iter().filter_map(|t| t.energy).collect();
        let place = match targets(&energies, m, opts.curve) {
            Some(target) => {
                // Отклонение — в долях размаха энергий плейлиста (без крайних 10 % с каждой
                // стороны): у плейлиста, где треки близки по энергии, кривая всё равно заметна
                let mut sorted = energies.clone();
                sorted.sort_by(f64::total_cmp);
                let spread = quantile(&sorted, 0.9) - quantile(&sorted, 0.1);
                let w = W_PLACE / spread.max(MIN_SPREAD);
                let mut place = vec![0.0; m * m];
                for (x, t) in tracks.iter().enumerate() {
                    if let Some(e) = t.energy {
                        for (i, goal) in target.iter().enumerate() {
                            place[x * m + i] = w * (e - goal).abs();
                        }
                    }
                }
                place
            }
            None => Vec::new(),
        };
        Self { m, step, place }
    }

    fn step(&self, a: usize, b: usize) -> f64 {
        self.step[a * self.m + b]
    }

    fn place(&self, x: usize, i: usize) -> f64 {
        if self.place.is_empty() {
            0.0
        } else {
            self.place[x * self.m + i]
        }
    }

    fn positional(&self) -> bool {
        !self.place.is_empty()
    }

    fn total(&self, seq: &[usize]) -> f64 {
        let steps: f64 = seq.windows(2).map(|w| self.step(w[0], w[1])).sum();
        let places: f64 = seq.iter().enumerate().map(|(i, &x)| self.place(x, i)).sum();
        steps + places
    }
}

const EPS: f64 = 1e-9;
/// Проходов улучшения не больше этого — на случай медленной сходимости.
const MAX_PASSES: usize = 60;
/// Длиннее не разворачиваем: на больших плейлистах это дорого, а выигрыш редкий.
const MAX_REVERSE: usize = 120;
/// Жадных стартов, когда первый трек не задан.
const STARTS: usize = 8;

/// Жадный порядок от `start`: каждый раз — самый дешёвый следующий трек на следующем месте.
fn greedy(c: &Costs, start: usize) -> Vec<usize> {
    let mut used = vec![false; c.m];
    let mut seq = Vec::with_capacity(c.m);
    used[start] = true;
    seq.push(start);
    for pos in 1..c.m {
        let last = seq[pos - 1];
        let mut best = (f64::INFINITY, usize::MAX);
        for (x, _) in used.iter().enumerate().filter(|(_, u)| !**u) {
            let v = c.step(last, x) + c.place(x, pos);
            if v < best.0 - EPS {
                best = (v, x);
            }
        }
        used[best.1] = true;
        seq.push(best.1);
    }
    seq
}

/// 2-opt: разворот отрезка `s[i..=j]`. Внутренние переходы считаются накопительно по `j`.
fn two_opt(c: &Costs, s: &mut [usize], lo: usize) -> bool {
    let m = s.len();
    let mut any = false;
    for i in lo..m.saturating_sub(1) {
        let mut internal = 0.0;
        let mut j = i + 1;
        while j < m && j - i <= MAX_REVERSE {
            internal += c.step(s[j], s[j - 1]) - c.step(s[j - 1], s[j]);
            let mut d = internal;
            if i > 0 {
                d += c.step(s[i - 1], s[j]) - c.step(s[i - 1], s[i]);
            }
            if j + 1 < m {
                d += c.step(s[i], s[j + 1]) - c.step(s[j], s[j + 1]);
            }
            if c.positional() {
                for k in i..=j {
                    d += c.place(s[i + j - k], k) - c.place(s[k], k);
                }
            }
            if d < -EPS {
                #[cfg(debug_assertions)]
                let was = c.total(s);
                s[i..=j].reverse();
                #[cfg(debug_assertions)]
                debug_assert!(
                    (c.total(s) - was - d).abs() < 1e-6,
                    "2-opt: разница посчитана неверно"
                );
                // У развёрнутого отрезка разница «наоборот минус как есть» меняет знак
                internal = -internal;
                any = true;
            }
            j += 1;
        }
    }
    any
}

/// Накопленные изменения стоимости мест, если трек с места `x` сдвинуть на `shift`.
fn shift_sums(c: &Costs, s: &[usize], shift: isize, out: &mut Vec<f64>) {
    out.clear();
    out.push(0.0);
    let m = s.len() as isize;
    let mut acc = 0.0;
    for (x, &t) in s.iter().enumerate() {
        let to = x as isize + shift;
        if (0..m).contains(&to) {
            acc += c.place(t, to as usize) - c.place(t, x);
        }
        out.push(acc);
    }
}

/// Or-opt: перенос отрезка из 1–3 треков на лучшее место (вперёд или назад).
fn or_opt(c: &Costs, s: &mut [usize], lo: usize) -> bool {
    let m = s.len();
    let mut any = false;
    let (mut back, mut fwd) = (Vec::new(), Vec::new());
    for len in 1..=3usize {
        if lo + len >= m {
            break;
        }
        let mut stale = true;
        let mut a = lo;
        while a + len <= m {
            if c.positional() && stale {
                shift_sums(c, s, -(len as isize), &mut back);
                shift_sums(c, s, len as isize, &mut fwd);
                stale = false;
            }
            let (first, last) = (s[a], s[a + len - 1]);
            let seg = |to: usize| -> f64 {
                (0..len)
                    .map(|q| c.place(s[a + q], to + q) - c.place(s[a + q], a + q))
                    .sum()
            };
            let mut best = (-EPS, usize::MAX);
            // Вперёд: отрезок встаёт сразу после s[b]
            for b in a + len..m {
                let mut d = c.step(s[b], first) - c.step(last, s[a + len]);
                if a > 0 {
                    d += c.step(s[a - 1], s[a + len]) - c.step(s[a - 1], first);
                }
                if b + 1 < m {
                    d += c.step(last, s[b + 1]) - c.step(s[b], s[b + 1]);
                }
                if c.positional() {
                    d += back[b + 1] - back[a + len] + seg(b + 1 - len);
                }
                if d < best.0 {
                    best = (d, b);
                }
            }
            // Назад: отрезок встаёт перед s[b]
            for b in lo..a {
                let mut d = c.step(last, s[b]) - c.step(s[a - 1], first);
                if b > 0 {
                    d += c.step(s[b - 1], first) - c.step(s[b - 1], s[b]);
                }
                if a + len < m {
                    d += c.step(s[a - 1], s[a + len]) - c.step(last, s[a + len]);
                }
                if c.positional() {
                    d += fwd[a] - fwd[b] + seg(b);
                }
                if d < best.0 {
                    best = (d, b);
                }
            }
            if best.1 != usize::MAX {
                let b = best.1;
                #[cfg(debug_assertions)]
                let was = c.total(s);
                if b > a {
                    s[a..=b].rotate_left(len);
                } else {
                    s[b..a + len].rotate_right(len);
                }
                #[cfg(debug_assertions)]
                debug_assert!(
                    (c.total(s) - was - best.0).abs() < 1e-6,
                    "or-opt: разница посчитана неверно"
                );
                stale = true;
                any = true;
            }
            a += 1;
        }
    }
    any
}

fn improve(c: &Costs, seq: &mut [usize], lo: usize) {
    for _ in 0..MAX_PASSES {
        let a = two_opt(c, seq, lo);
        let b = or_opt(c, seq, lo);
        if !a && !b {
            break;
        }
    }
}

/// Расставляемые (с анализом) по порядку `order`; заданный первый без анализа в задачу не входит.
fn analyzed_part(tracks: &[SetTrack], order: &[usize]) -> Vec<usize> {
    order
        .iter()
        .copied()
        .filter(|&i| tracks[i].analyzed())
        .collect()
}

/// Стоимость порядка: переходы между соседями и места на кривой — по трекам с анализом, в том
/// порядке, в каком они идут. Треки без анализа не считаются.
pub fn cost(tracks: &[SetTrack], order: &[usize], opts: &Options) -> f64 {
    let ids = analyzed_part(tracks, order);
    let refs: Vec<&SetTrack> = ids.iter().map(|&i| &tracks[i]).collect();
    let c = Costs::new(&refs, opts);
    c.total(&(0..ids.len()).collect::<Vec<_>>())
}

/// Новый порядок для `tracks` (в исходном порядке плейлиста).
pub fn arrange(tracks: &[SetTrack], opts: &Options) -> Arrangement {
    let n = tracks.len();
    let original: Vec<usize> = (0..n).collect();
    let first = opts.first.filter(|&f| f < n);
    // Расставляемые: с анализом, в исходном порядке. Заданный первый без анализа — просто первым
    let ids: Vec<usize> = analyzed_part(tracks, &original);
    let lead_unanalyzed = first.filter(|&f| !tracks[f].analyzed());
    let rest: Vec<usize> = original
        .iter()
        .copied()
        .filter(|&i| !tracks[i].analyzed() && Some(i) != lead_unanalyzed)
        .collect();
    let cost_before = cost(tracks, &original, opts);

    let refs: Vec<&SetTrack> = ids.iter().map(|&i| &tracks[i]).collect();
    let c = Costs::new(&refs, opts);
    let fixed = first.and_then(|f| ids.iter().position(|&i| i == f));
    let lo = usize::from(fixed.is_some());

    let mut seq: Vec<usize> = if c.m == 0 {
        Vec::new()
    } else {
        let starts: Vec<usize> = match fixed {
            Some(f) => vec![f],
            None => start_candidates(&c, &refs),
        };
        let mut best: Option<(f64, Vec<usize>)> = None;
        for s in starts {
            let g = greedy(&c, s);
            let v = c.total(&g);
            if best.as_ref().is_none_or(|(bv, _)| v < bv - EPS) {
                best = Some((v, g));
            }
        }
        let (_, mut seq) = best.expect("хотя бы один старт");
        improve(&c, &mut seq, lo);
        // Исходный порядок не хуже — улучшаем его: повторная сборка не перетасовывает зря
        let mut kept: Vec<usize> = (0..c.m).collect();
        if let Some(f) = fixed {
            kept.remove(f);
            kept.insert(0, f);
        }
        if c.total(&kept) <= c.total(&seq) + EPS {
            improve(&c, &mut kept, lo);
            seq = kept;
        }
        seq
    };

    let mut order = Vec::with_capacity(n);
    order.extend(lead_unanalyzed);
    order.extend(seq.drain(..).map(|x| ids[x]));
    let placed = order.len();
    order.extend(rest);
    let cost_after = cost(tracks, &order, opts);
    Arrangement {
        order,
        placed,
        cost_before,
        cost_after,
    }
}

/// С чего пробовать начинать: при кривой — треки, лучше всего подходящие на первое место; ровно —
/// самые спокойные и самые энергичные (концы пути с малыми скачками обычно крайние).
fn start_candidates(c: &Costs, tracks: &[&SetTrack]) -> Vec<usize> {
    let mut idx: Vec<usize> = (0..c.m).collect();
    if c.positional() {
        idx.sort_by(|&a, &b| c.place(a, 0).total_cmp(&c.place(b, 0)).then(a.cmp(&b)));
        idx.truncate(STARTS);
        return idx;
    }
    let energy = |i: usize| tracks[i].energy.unwrap_or(0.5);
    idx.sort_by(|&a, &b| energy(a).total_cmp(&energy(b)).then(a.cmp(&b)));
    let mut out: Vec<usize> = idx.iter().copied().take(STARTS / 2).collect();
    for &i in idx.iter().rev().take(STARTS / 2) {
        if !out.contains(&i) {
            out.push(i);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEYS: [&str; 24] = [
        "1A", "2A", "3A", "4A", "5A", "6A", "7A", "8A", "9A", "10A", "11A", "12A", "1B", "2B",
        "3B", "4B", "5B", "6B", "7B", "8B", "9B", "10B", "11B", "12B",
    ];

    /// Детерминированный «шум» без внешних зависимостей.
    struct Lcg(u64);
    impl Lcg {
        fn next(&mut self) -> f64 {
            self.0 = self
                .0
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            (self.0 >> 11) as f64 / (1u64 << 53) as f64
        }
    }

    fn track(bpm: f64, key: &str, energy: f64, artist: u32) -> SetTrack {
        SetTrack {
            bpm: Some(bpm),
            key: Some(key.into()),
            key_confidence: Some(0.8),
            energy: Some(energy),
            head: None,
            tail: None,
            artists: vec![artist],
        }
    }

    fn random_playlist(n: usize, seed: u64) -> Vec<SetTrack> {
        let mut r = Lcg(seed);
        (0..n)
            .map(|i| {
                let bpm = 90.0 + (r.next() * 50.0).round();
                let key = KEYS[(r.next() * 24.0) as usize % 24];
                let energy = ((r.next() * 0.9 + 0.05) * 1000.0).round() / 1000.0;
                track(bpm, key, energy, (i % 7) as u32)
            })
            .collect()
    }

    fn is_permutation(order: &[usize], n: usize) -> bool {
        let mut seen = vec![false; n];
        order.len() == n
            && order
                .iter()
                .all(|&i| i < n && !std::mem::replace(&mut seen[i], true))
    }

    #[test]
    fn tempo_folds_half_and_double() {
        let (g, folded) = tempo_gap(128.0, 128.0);
        assert!(g.abs() < 1e-12 && !folded);
        let (g, folded) = tempo_gap(70.0, 140.0);
        assert!(g.abs() < 1e-12 && folded);
        let (g, _) = tempo_gap(120.0, 126.0);
        assert!((g - 0.05).abs() < 1e-9);
        // Растяжение в пределах запаса почти бесплатно, далёкий темп дорог
        assert!(tempo_cost(Some(124.0), Some(125.0)) < 0.05);
        assert!(tempo_cost(Some(100.0), Some(140.0)) >= 1.0);
        assert!(tempo_cost(Some(70.0), Some(140.0)) > tempo_cost(Some(140.0), Some(140.0)));
        assert_eq!(tempo_cost(None, Some(120.0)), UNKNOWN_TEMPO);
    }

    #[test]
    fn hints_follow_key_and_tempo() {
        let a = track(124.0, "8A", 0.5, 1);
        assert_eq!(fit(&a, &track(126.0, "9A", 0.5, 2)), Some(Fit::InKey));
        assert_eq!(fit(&a, &track(63.0, "8B", 0.5, 2)), Some(Fit::InKey));
        assert_eq!(fit(&a, &track(125.0, "10A", 0.5, 2)), Some(Fit::Smooth));
        assert_eq!(fit(&a, &track(150.0, "3B", 0.8, 2)), Some(Fit::Lift));
        assert_eq!(fit(&a, &track(150.0, "3B", 0.2, 2)), Some(Fit::Calm));
        assert_eq!(fit(&a, &track(150.0, "3B", 0.5, 2)), Some(Fit::Switch));
        assert_eq!(fit(&a, &SetTrack::default()), None);
    }

    #[test]
    fn edges_of_the_energy_curve() {
        let mut curve = vec![0.0; 3];
        curve.extend(std::iter::repeat_n(200.0, 40));
        curve.extend(std::iter::repeat_n(900.0, 40));
        curve.extend([0.0, 0.0]);
        let t = SetTrack::default().with_curve(&curve);
        assert!((t.head.unwrap() - 0.2).abs() < 1e-9);
        assert!((t.tail.unwrap() - 0.9).abs() < 1e-9);
        // Слишком короткое — без краёв
        assert_eq!(SetTrack::default().with_curve(&[500.0; 3]).head, None);
    }

    #[test]
    fn targets_follow_the_shape() {
        let e = [0.1, 0.9, 0.5, 0.3, 0.7];
        let rise = targets(&e, 5, Curve::Rise).unwrap();
        assert_eq!(rise, vec![0.1, 0.3, 0.5, 0.7, 0.9]);
        let wave = targets(&e, 7, Curve::Wave).unwrap();
        let peak = wave
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.total_cmp(b.1))
            .unwrap()
            .0;
        assert_eq!(peak, 4, "{wave:?}");
        assert!(wave[0] < wave[6] && wave[6] < wave[4]);
        assert_eq!(targets(&e, 5, Curve::Flat), None);
    }

    #[test]
    fn local_search_keeps_its_bookkeeping() {
        // Быстрые разницы 2-opt и or-opt совпадают с полным пересчётом: итог не хуже жадного
        for curve in [Curve::Rise, Curve::Wave, Curve::Flat] {
            let tracks = random_playlist(40, 7);
            let refs: Vec<&SetTrack> = tracks.iter().collect();
            let opts = Options {
                curve,
                ..Options::default()
            };
            let c = Costs::new(&refs, &opts);
            let mut seq: Vec<usize> = (0..c.m).collect();
            let mut prev = c.total(&seq);
            for _ in 0..10 {
                let mut probe = seq.clone();
                let moved = two_opt(&c, &mut probe, 0) | or_opt(&c, &mut probe, 0);
                let now = c.total(&probe);
                assert!(now <= prev + 1e-9, "{curve:?}: {now} > {prev}");
                assert!(is_permutation(&probe, c.m));
                seq = probe;
                prev = now;
                if !moved {
                    break;
                }
            }
        }
    }

    #[test]
    fn set_is_better_than_the_playlist() {
        for (curve, seed) in [(Curve::Rise, 1), (Curve::Wave, 2), (Curve::Flat, 3)] {
            let tracks = random_playlist(60, seed);
            let opts = Options {
                curve,
                ..Options::default()
            };
            let a = arrange(&tracks, &opts);
            assert!(is_permutation(&a.order, tracks.len()));
            assert_eq!(a.placed, tracks.len());
            assert!(
                a.cost_after < a.cost_before * 0.7,
                "{curve:?}: {} → {}",
                a.cost_before,
                a.cost_after
            );
            assert!((cost(&tracks, &a.order, &opts) - a.cost_after).abs() < 1e-9);
            // Плавных переходов стало больше
            let smooth = |order: &[usize]| {
                order
                    .windows(2)
                    .filter(|w| fit(&tracks[w[0]], &tracks[w[1]]).is_some_and(Fit::is_smooth))
                    .count()
            };
            let before = smooth(&(0..tracks.len()).collect::<Vec<_>>());
            assert!(smooth(&a.order) > before, "{curve:?}");
        }
    }

    /// Средняя энергия треть за третью.
    fn thirds(tracks: &[SetTrack], order: &[usize]) -> [f64; 3] {
        let n = order.len();
        let mut out = [0.0; 3];
        for (k, part) in out.iter_mut().enumerate() {
            let slice = &order[k * n / 3..(k + 1) * n / 3];
            *part = slice
                .iter()
                .map(|&i| tracks[i].energy.unwrap())
                .sum::<f64>()
                / slice.len() as f64;
        }
        out
    }

    #[test]
    fn rise_climbs_and_wave_peaks_late() {
        let tracks = random_playlist(45, 11);
        let rise = arrange(&tracks, &Options::default());
        let [a, b, c] = thirds(&tracks, &rise.order);
        assert!(a < b && b < c, "{a} {b} {c}");
        // Последний трек — среди самых энергичных
        let mut energies: Vec<f64> = tracks.iter().map(|t| t.energy.unwrap()).collect();
        energies.sort_by(f64::total_cmp);
        let last = tracks[*rise.order.last().unwrap()].energy.unwrap();
        assert!(last >= energies[energies.len() * 3 / 4], "{last}");

        let wave = arrange(
            &tracks,
            &Options {
                curve: Curve::Wave,
                ..Options::default()
            },
        );
        let [a, b, c] = thirds(&tracks, &wave.order);
        assert!(a < b && c < b, "{a} {b} {c}");
    }

    #[test]
    fn flat_keeps_jumps_small() {
        let tracks = random_playlist(50, 5);
        let opts = Options {
            curve: Curve::Flat,
            ..Options::default()
        };
        let a = arrange(&tracks, &opts);
        let jumps = |order: &[usize]| {
            order
                .windows(2)
                .map(|w| (tracks[w[1]].energy.unwrap() - tracks[w[0]].energy.unwrap()).abs())
                .sum::<f64>()
        };
        assert!(jumps(&a.order) < jumps(&(0..tracks.len()).collect::<Vec<_>>()) * 0.5);
    }

    #[test]
    fn deterministic_and_stable() {
        let tracks = random_playlist(80, 9);
        for curve in [Curve::Rise, Curve::Wave, Curve::Flat] {
            let opts = Options {
                curve,
                ..Options::default()
            };
            let a = arrange(&tracks, &opts);
            assert_eq!(a, arrange(&tracks, &opts));
            // Собранный ещё раз — не перетасовывается
            let again: Vec<SetTrack> = a.order.iter().map(|&i| tracks[i].clone()).collect();
            let b = arrange(&again, &opts);
            assert!(b.cost_after <= a.cost_after + 1e-9);
            assert_eq!(b.order, (0..tracks.len()).collect::<Vec<_>>(), "{curve:?}");
        }
    }

    #[test]
    fn first_track_and_unanalyzed() {
        let mut tracks = random_playlist(20, 4);
        tracks[3] = SetTrack {
            artists: vec![1],
            ..SetTrack::default()
        };
        tracks[12] = SetTrack::default();
        // Частичный анализ — расставляется
        tracks[7].bpm = None;
        tracks[7].key = None;
        let opts = Options {
            first: Some(15),
            ..Options::default()
        };
        let a = arrange(&tracks, &opts);
        assert!(is_permutation(&a.order, 20));
        assert_eq!(a.order[0], 15);
        assert_eq!(a.placed, 18);
        assert_eq!(&a.order[18..], &[3, 12]);
        assert!(a.order[..18].contains(&7));

        // Первым — трек без анализа: он первым, остальные за ним
        let a = arrange(
            &tracks,
            &Options {
                first: Some(12),
                ..Options::default()
            },
        );
        assert_eq!(a.order[0], 12);
        assert_eq!(a.placed, 19);
        assert_eq!(a.order[19], 3);

        // Совсем без анализа — порядок прежний
        let blank = vec![SetTrack::default(); 5];
        let a = arrange(&blank, &Options::default());
        assert_eq!(a.order, vec![0, 1, 2, 3, 4]);
        assert_eq!(a.placed, 0);
        assert_eq!(arrange(&[], &Options::default()).order, Vec::<usize>::new());
    }

    #[test]
    fn same_artist_is_spread() {
        // Две группы по тональности; внутри каждой — по два трека одного исполнителя
        let tracks = vec![
            track(124.0, "8A", 0.5, 1),
            track(124.0, "8A", 0.5, 1),
            track(124.0, "8A", 0.5, 2),
            track(124.0, "8A", 0.5, 2),
            track(124.0, "8A", 0.5, 3),
        ];
        let opts = Options {
            curve: Curve::Flat,
            ..Options::default()
        };
        let a = arrange(&tracks, &opts);
        assert!(
            a.order
                .windows(2)
                .all(|w| !shares_artist(&tracks[w[0]], &tracks[w[1]])),
            "{:?}",
            a.order
        );
    }

    #[test]
    fn big_playlist_is_fast() {
        let tracks = random_playlist(300, 21);
        let start = std::time::Instant::now();
        let a = arrange(&tracks, &Options::default());
        let took = start.elapsed();
        assert!(a.cost_after < a.cost_before);
        // Без оптимизаций тесты медленнее в разы: время проверяется только в сборке с ними
        if !cfg!(debug_assertions) {
            assert!(took.as_millis() < 700, "{took:?}");
        }
    }
}
