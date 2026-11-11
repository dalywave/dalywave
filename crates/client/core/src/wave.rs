//! Волна без сети: следующий трек из скачанного запаса.

use std::collections::HashSet;

use chrono::{DateTime, Duration, Utc};
use dalywave_protocol::wave::{WaveActivity, WaveCharacter, WaveLanguage, WaveMood, WaveSettings};
use uuid::Uuid;

/// Признак неизвестен — штраф как за небольшой выход за границу.
const UNKNOWN: f32 = 0.15;
/// Ступени ослабления границ — как на сервере.
const TOLERANCES: [f32; 4] = [0.0, 0.1, 0.2, 0.35];
/// Ранний пропуск: артист отдыхает столько треков.
const ARTIST_REST_STEPS: u32 = 10;
/// Ранний пропуск — до 30 секунд.
const EARLY_SKIP_MS: i64 = 30_000;
const SERVED_MEMORY: usize = 400;
const RECENT_ARTISTS: usize = 8;

/// Скачанный трек-кандидат.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Candidate {
    pub track_id: Uuid,
    pub title: String,
    pub artist_ids: Vec<Uuid>,
    pub artist_names: Vec<String>,
    pub energy: Option<f32>,
    pub valence: Option<f32>,
    pub danceability: Option<f32>,
    pub bpm: Option<f32>,
    pub key_camelot: Option<String>,
    pub voice: Option<f32>,
    pub language: Option<String>,
    pub approachability: Option<f32>,
    pub liked: bool,
    /// Из набора волны: `favorite` | `fresh`.
    pub set_kind: Option<String>,
    pub set_score: Option<f32>,
    pub plays_30d: i64,
    /// Ранние пропуски за 30 дней.
    pub skips_30d: i64,
    pub completed: i64,
    pub last_played: Option<DateTime<Utc>>,
    /// «Не рекомендовать» трек или артиста.
    pub blocked: bool,
}

impl Candidate {
    /// Знакомое: лайк или дослушан.
    pub fn familiar(&self) -> bool {
        self.liked || self.completed > 0
    }

    pub fn heard(&self) -> bool {
        self.liked || self.last_played.is_some()
    }

    fn song(&self) -> String {
        let artist = self.artist_ids.first().copied().unwrap_or(self.track_id);
        format!("{artist}:{}", self.title.trim().to_lowercase())
    }

    /// Точка в пространстве признаков для «заходит» и «пропускается»; без анализа — `None`.
    fn point(&self) -> Option<[f32; 4]> {
        let e = self.energy?;
        Some([
            e,
            self.valence.unwrap_or(0.5),
            self.danceability.unwrap_or(0.5),
            (self.bpm.unwrap_or(120.0) / 200.0).clamp(0.0, 1.5),
        ])
    }
}

/// Реакция на трек волны.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reaction {
    Like,
    Dislike,
    /// Пропуск на этой позиции.
    Skip {
        position_ms: i64,
    },
    Completed,
    NotNow,
}

/// Подпись «почему этот трек» без сети.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pick {
    pub track_id: Uuid,
    pub reason: String,
}

/// Состояние волны без сети: живёт в памяти приложения, пока длится сессия.
#[derive(Debug, Clone, Default)]
pub struct Session {
    pub settings: WaveSettings,
    pub step: u32,
    new_served: u32,
    served: Vec<Uuid>,
    songs: Vec<String>,
    recent_artists: Vec<Uuid>,
    last: Option<Last>,
    rested: Vec<(Uuid, u32)>,
    skip_streak: u32,
    /// Сумма и вес точек «заходит» и «пропускается».
    hits: ([f32; 4], f32),
    misses: ([f32; 4], f32),
    seed: u64,
}

#[derive(Debug, Clone)]
struct Last {
    artist_ids: Vec<Uuid>,
    bpm: Option<f32>,
    key: Option<String>,
}

/// Маленький детерминированный генератор (xorshift): доля случайности без зависимостей.
#[derive(Debug, Clone)]
struct Rng(u64);

impl Rng {
    fn next_f32(&mut self) -> f32 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        (x >> 40) as f32 / (1u64 << 24) as f32
    }
}

/// Границы признаков из занятия, настроения и языка — те же, что на сервере.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Constraints {
    pub energy: Option<(f32, f32)>,
    pub valence: Option<(f32, f32)>,
    pub danceability_min: Option<f32>,
    pub bpm: Option<(f32, f32)>,
    pub tempo: Option<f32>,
    pub voice_max: Option<f32>,
    pub language: WaveLanguage,
    pub unheard: bool,
}

impl Constraints {
    fn any(&self) -> bool {
        self.unheard
            || self.energy.is_some()
            || self.valence.is_some()
            || self.danceability_min.is_some()
            || self.bpm.is_some()
            || self.tempo.is_some()
            || self.voice_max.is_some()
            || self.language != WaveLanguage::Any
    }
}

fn narrow(range: &mut Option<(f32, f32)>, lo: f32, hi: f32) {
    let (a, b) = range.unwrap_or((0.0, 1.0));
    *range = Some((a.max(lo), b.min(hi)));
}

pub fn constraints(s: &WaveSettings) -> Constraints {
    let mut c = Constraints {
        language: s.language,
        unheard: s.character == WaveCharacter::New,
        ..Default::default()
    };
    match s.mood {
        WaveMood::Any => {}
        WaveMood::Energetic => narrow(&mut c.energy, 0.6, 1.0),
        WaveMood::Happy => narrow(&mut c.valence, 0.55, 1.0),
        WaveMood::Calm => narrow(&mut c.energy, 0.0, 0.5),
        WaveMood::Sad => narrow(&mut c.valence, 0.0, 0.5),
    }
    match s.activity {
        WaveActivity::Any | WaveActivity::Road => {}
        WaveActivity::Wake => {
            narrow(&mut c.energy, 0.25, 0.85);
            narrow(&mut c.valence, 0.35, 1.0);
        }
        WaveActivity::Work => {
            narrow(&mut c.energy, 0.0, 0.7);
            c.bpm = Some((70.0, 135.0));
        }
        WaveActivity::Sport => {
            narrow(&mut c.energy, 0.6, 1.0);
            match s.tempo {
                Some(t) => c.tempo = Some(f32::from(t.clamp(100, 220))),
                None => c.bpm = Some((115.0, 185.0)),
            }
        }
        WaveActivity::Rest => narrow(&mut c.energy, 0.0, 0.6),
        WaveActivity::Sleep => {
            narrow(&mut c.energy, 0.0, 0.5);
            c.bpm = Some((0.0, 115.0));
        }
        WaveActivity::Party => {
            narrow(&mut c.energy, 0.55, 1.0);
            c.danceability_min = Some(0.55);
        }
    }
    for r in [&mut c.energy, &mut c.valence].into_iter().flatten() {
        if r.0 > r.1 {
            let mid = (r.0 + r.1) / 2.0;
            *r = (mid - 0.05, mid + 0.05);
        }
    }
    if s.language == WaveLanguage::None {
        c.voice_max = Some(0.4);
    }
    c
}

fn has_cyrillic(s: &str) -> bool {
    s.chars()
        .any(|c| matches!(c, 'а'..='я' | 'А'..='Я' | 'ё' | 'Ё'))
}

fn russian(c: &Candidate) -> bool {
    match c.language.as_deref() {
        Some(lang) => lang == "ru",
        None => has_cyrillic(&c.title) || c.artist_names.iter().any(|n| has_cyrillic(n)),
    }
}

/// Разница темпов как доля с учётом «вдвое».
pub fn tempo_gap(a: f32, b: f32) -> f32 {
    if a <= 0.0 || b <= 0.0 {
        return 0.0;
    }
    [b, b * 2.0, b / 2.0]
        .iter()
        .map(|x| (x / a - 1.0).abs())
        .fold(f32::MAX, f32::min)
}

fn camelot(s: &str) -> Option<(u8, char)> {
    let s = s.trim();
    let letter = s.chars().last()?.to_ascii_uppercase();
    if letter != 'A' && letter != 'B' {
        return None;
    }
    let n: u8 = s[..s.len() - 1].parse().ok()?;
    (1..=12).contains(&n).then_some((n, letter))
}

/// Совместимость тональностей по кругу Камелота: 1 — та же, 0 — далёкие.
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

/// Насколько трек выходит за границы: 0 — подходит; язык — 10 (не ослабляется).
pub fn violation(c: &Candidate, k: &Constraints) -> f32 {
    let range = |x: Option<f32>, (lo, hi): (f32, f32)| match x {
        Some(x) => (lo - x).max(0.0) + (x - hi).max(0.0),
        None => UNKNOWN,
    };
    let mut v = 0.0;
    if let Some(r) = k.energy {
        v += range(c.energy, r);
    }
    if let Some(r) = k.valence {
        v += range(c.valence, r);
    }
    if let Some(m) = k.danceability_min {
        v += range(c.danceability, (m, 1.0));
    }
    if let Some(m) = k.voice_max {
        v += range(c.voice, (0.0, m));
    }
    if let Some(t) = k.tempo {
        v += match c.bpm {
            Some(bpm) => (tempo_gap(t, bpm) - 0.06).max(0.0),
            None => UNKNOWN,
        };
    } else if let Some((lo, hi)) = k.bpm {
        v += match c.bpm {
            Some(bpm) => ((lo - bpm).max(0.0) + (bpm - hi).max(0.0)) / 100.0,
            None => UNKNOWN,
        };
    }
    match k.language {
        WaveLanguage::Ru if !russian(c) => v += 10.0,
        WaveLanguage::Foreign if russian(c) => v += 10.0,
        _ => {}
    }
    if k.unheard && c.heard() {
        v += 1.0;
    }
    v
}

/// Тяга настроения и занятия внутри границ.
pub fn pull(s: &WaveSettings, c: &Candidate) -> f32 {
    let energy = c.energy.unwrap_or(0.6);
    let valence = c.valence.unwrap_or(0.5);
    let mood = match s.mood {
        WaveMood::Any => 0.0,
        WaveMood::Energetic => 1.2 * energy,
        WaveMood::Calm => -1.2 * energy,
        WaveMood::Happy => 1.2 * valence,
        WaveMood::Sad => -1.2 * valence,
    };
    let activity = match s.activity {
        WaveActivity::Sport | WaveActivity::Party => 0.5 * energy,
        WaveActivity::Work | WaveActivity::Rest => -0.5 * energy,
        _ => 0.0,
    };
    mood + activity
}

/// Ход энергии: «Засыпаю» снижает, «Просыпаюсь» поднимает.
pub fn target_energy(s: &WaveSettings, step: u32) -> Option<f32> {
    let n = step as f32;
    match s.activity {
        WaveActivity::Sleep => Some((0.45 - 0.02 * n).max(0.12)),
        WaveActivity::Wake => Some((0.3 + 0.03 * n).min(0.8)),
        _ => None,
    }
}

fn new_share(c: WaveCharacter) -> f32 {
    match c {
        WaveCharacter::Fav => 0.1,
        WaveCharacter::Mix => 0.45,
        WaveCharacter::New => 1.0,
        WaveCharacter::Pop => 0.5,
        WaveCharacter::Rare => 0.7,
    }
}

/// «Отдых» заслушанного: 6 часов после одного прослушивания за месяц, вдвое дольше за каждое следующее.
pub fn rest(plays_30d: i64, character: WaveCharacter) -> Duration {
    if plays_30d <= 0 {
        return Duration::zero();
    }
    let hours = 6.0 * 2f64.powi((plays_30d.min(8) - 1) as i32);
    let hours = if character == WaveCharacter::Fav {
        hours / 2.0
    } else {
        hours
    };
    Duration::minutes((hours.min(30.0 * 24.0) * 60.0) as i64)
}

fn similarity(a: &[f32; 4], b: &[f32; 4]) -> f32 {
    let d: f32 = a.iter().zip(b).map(|(x, y)| (x - y) * (x - y)).sum();
    (1.0 - d.sqrt() / 1.2).clamp(0.0, 1.0)
}

fn centroid((sum, w): &([f32; 4], f32)) -> Option<[f32; 4]> {
    (*w > 0.0).then(|| sum.map(|x| x / w))
}

impl Session {
    pub fn new(settings: WaveSettings, seed: u64) -> Self {
        Self {
            settings,
            seed: seed | 1,
            ..Default::default()
        }
    }

    /// Уже сыгранное до смены настроек: не повторять.
    pub fn exclude(&mut self, ids: impl IntoIterator<Item = Uuid>) {
        for id in ids {
            if !self.served.contains(&id) {
                self.served.push(id);
            }
        }
    }

    fn artist_rested(&self, c: &Candidate) -> bool {
        self.rested
            .iter()
            .any(|(a, until)| *until > self.step && c.artist_ids.contains(a))
    }

    fn push(&mut self, c: &Candidate) {
        self.served.push(c.track_id);
        if self.served.len() > SERVED_MEMORY {
            self.served.remove(0);
        }
        self.songs.push(c.song());
        if self.songs.len() > SERVED_MEMORY {
            self.songs.remove(0);
        }
        self.step += 1;
        if !c.familiar() {
            self.new_served += 1;
        }
        self.last = Some(Last {
            artist_ids: c.artist_ids.clone(),
            bpm: c.bpm,
            key: c.key_camelot.clone(),
        });
        if let Some(a) = c.artist_ids.first() {
            self.recent_artists.retain(|x| x != a);
            self.recent_artists.push(*a);
            if self.recent_artists.len() > RECENT_ARTISTS {
                self.recent_artists.remove(0);
            }
        }
    }

    /// Реакция на трек: «заходит» и «пропускается», серия пропусков, отдых артиста.
    pub fn react(&mut self, c: &Candidate, reaction: Reaction) {
        let (hit, miss, rest_artist, skip) = match reaction {
            Reaction::Like => (1.0, 0.0, false, Some(false)),
            Reaction::Completed => (0.5, 0.0, false, Some(false)),
            Reaction::Skip { position_ms } => {
                let early = position_ms < EARLY_SKIP_MS;
                (0.0, if early { 1.0 } else { 0.4 }, early, Some(true))
            }
            Reaction::Dislike => (0.0, 1.5, true, Some(true)),
            Reaction::NotNow => (0.0, 0.0, false, None),
        };
        match skip {
            Some(true) => self.skip_streak += 1,
            Some(false) => self.skip_streak = 0,
            None => {}
        }
        if rest_artist {
            for a in &c.artist_ids {
                self.rested.retain(|(x, _)| x != a);
                self.rested.push((*a, self.step + ARTIST_REST_STEPS));
            }
        }
        if let Some(p) = c.point() {
            for (acc, w) in [(&mut self.hits, hit), (&mut self.misses, miss)] {
                if w > 0.0 {
                    for (s, x) in acc.0.iter_mut().zip(p) {
                        *s += w * x;
                    }
                    acc.1 += w;
                }
            }
        }
        if !self.served.contains(&c.track_id) {
            self.served.push(c.track_id);
        }
    }

    fn admissible(&self, c: &Candidate, now: DateTime<Utc>, served: &HashSet<Uuid>) -> bool {
        let resting = c
            .last_played
            .is_some_and(|at| now - at < rest(c.plays_30d.max(1), self.settings.character));
        !c.blocked
            && !served.contains(&c.track_id)
            && !self.songs.contains(&c.song())
            && !resting
            && !self.artist_rested(c)
    }

    fn score(&self, c: &Candidate, rng: &mut Rng) -> f32 {
        let familiar = c.familiar();
        // Запас сервер уже отобрал по вкусу: его оценка — вместо сходства с гранями
        let mut s = 0.6
            * c.set_score
                .unwrap_or(if c.liked { 0.6 } else { 0.4 })
                .clamp(0.0, 1.0);
        match self.settings.character {
            WaveCharacter::Fav if familiar => s += 0.3,
            WaveCharacter::New if familiar => s -= 0.3,
            WaveCharacter::New => s += 0.1,
            WaveCharacter::Pop => s += 0.35 * c.approachability.unwrap_or(0.5) - 0.1,
            WaveCharacter::Rare => s += 0.3 * (1.0 - c.approachability.unwrap_or(0.5)) - 0.1,
            _ => {}
        }
        if c.liked {
            s += 0.08;
        }
        // Часто пропускаемое за месяц — вниз
        s -= 0.1 * (c.skips_30d.min(3) as f32);
        s += pull(&self.settings, c);
        if let Some(p) = c.point() {
            if let Some(h) = centroid(&self.hits) {
                s += 0.3 * (similarity(&p, &h) - 0.5);
            }
            if let Some(m) = centroid(&self.misses) {
                let streak = if self.skip_streak >= 2 { 2.0 } else { 1.0 };
                s -= 0.35 * streak * (similarity(&p, &m) - 0.5).max(0.0) * 2.0;
            }
        }
        let jitter = if self.skip_streak >= 2 { 0.2 } else { 0.1 };
        s + jitter * rng.next_f32()
    }

    /// Следующие `count` треков из кандидатов и подпись к каждому.
    pub fn next(&mut self, cands: &[Candidate], count: usize, now: DateTime<Utc>) -> Vec<Pick> {
        let served: HashSet<Uuid> = self.served.iter().copied().collect();
        let bounds = constraints(&self.settings);
        let mut rng =
            Rng(self.seed ^ (u64::from(self.step) + 1).wrapping_mul(0x9E37_79B9_7F4A_7C15));
        let mut admitted: Vec<&Candidate> = cands
            .iter()
            .filter(|c| self.admissible(c, now, &served))
            .collect();
        // Всё заслушано (маленький запас): отдых не держит волну — лишь бы не повтор в сессии
        if admitted.len() < count {
            admitted = cands
                .iter()
                .filter(|c| !c.blocked && !served.contains(&c.track_id) && !self.artist_rested(c))
                .collect();
        }
        let scored: Vec<(&Candidate, f32, f32)> = admitted
            .into_iter()
            .map(|c| {
                let v = violation(c, &bounds);
                (c, self.score(c, &mut rng) - 1.5 * v.min(1.0), v)
            })
            .collect();
        let pool = within_bounds(scored, count, &bounds);
        self.sequence(pool, count)
    }

    fn sequence(&mut self, mut pool: Vec<(&Candidate, f32, f32)>, count: usize) -> Vec<Pick> {
        let share = new_share(self.settings.character);
        let mut out = Vec::with_capacity(count);
        while out.len() < count && !pool.is_empty() {
            let target = target_energy(&self.settings, self.step);
            let want_new = (self.new_served as f32) < share * (self.step + 1) as f32;
            let prev: Vec<Uuid> = self
                .last
                .as_ref()
                .map_or_else(Vec::new, |l| l.artist_ids.clone());
            let mut best: Option<(usize, f32)> = None;
            for (i, (c, score, _)) in pool.iter().enumerate() {
                if c.artist_ids.iter().any(|a| prev.contains(a)) || self.songs.contains(&c.song()) {
                    continue;
                }
                let mut v = *score;
                if c.familiar() != want_new {
                    v += 0.25;
                }
                if c.artist_ids
                    .iter()
                    .any(|a| self.recent_artists.iter().rev().take(4).any(|r| r == a))
                {
                    v -= 0.3;
                }
                if let Some(t) = target {
                    v -= match c.energy {
                        Some(e) => 1.5 * (e - t).abs(),
                        None => 0.3,
                    };
                }
                if let Some(last) = &self.last {
                    if last.key.is_some() && c.key_camelot.is_some() {
                        v += 0.12
                            * (camelot_compat(last.key.as_deref(), c.key_camelot.as_deref()) - 0.5);
                    }
                    if let (Some(a), Some(b)) = (last.bpm, c.bpm) {
                        v -= 0.6 * (tempo_gap(a, b) - 0.06).max(0.0);
                    }
                }
                if best.is_none_or(|(_, bv)| v > bv) {
                    best = Some((i, v));
                }
            }
            let Some((i, _)) = best else { break };
            let (c, _, _) = pool.swap_remove(i);
            self.push(c);
            out.push(Pick {
                track_id: c.track_id,
                reason: reason(c),
            });
        }
        out
    }
}

/// Отбор с ослаблением границ: самая строгая ступень, где набирается вдвое больше нужного.
fn within_bounds<'a>(
    scored: Vec<(&'a Candidate, f32, f32)>,
    count: usize,
    k: &Constraints,
) -> Vec<(&'a Candidate, f32, f32)> {
    if !k.any() {
        return scored;
    }
    for tol in TOLERANCES {
        let fit = scored.iter().filter(|s| s.2 <= tol + 1e-6).count();
        if fit >= count * 2 || (tol == TOLERANCES[TOLERANCES.len() - 1] && fit >= count) {
            return scored.into_iter().filter(|s| s.2 <= tol + 1e-6).collect();
        }
    }
    let mut rest: Vec<_> = scored.into_iter().filter(|s| s.2 < 10.0).collect();
    rest.sort_by(|a, b| a.2.total_cmp(&b.2));
    rest.truncate(count * 3);
    rest
}

fn reason(c: &Candidate) -> String {
    if c.liked && c.completed >= 5 {
        return "Вы часто слушаете".into();
    }
    if c.liked {
        return "Из любимого".into();
    }
    if c.completed >= 3 {
        return "Вы часто слушаете".into();
    }
    if c.set_kind.as_deref() == Some("fresh") && !c.heard() {
        return "Новое для тебя".into();
    }
    "Из скачанного".into()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cand(n: u128, artist: u128, energy: f32, valence: f32) -> Candidate {
        Candidate {
            track_id: Uuid::from_u128(n),
            title: format!("Track {n}"),
            artist_ids: vec![Uuid::from_u128(1000 + artist)],
            artist_names: vec![format!("Artist {artist}")],
            energy: Some(energy),
            valence: Some(valence),
            danceability: Some(0.5),
            bpm: Some(120.0),
            set_score: Some(0.5),
            ..Default::default()
        }
    }

    fn library() -> Vec<Candidate> {
        (0..40)
            .map(|i| {
                let e = (i as f32) / 40.0;
                cand(i as u128, (i % 10) as u128, e, 1.0 - e)
            })
            .collect()
    }

    #[test]
    fn mood_bounds_are_respected() {
        let lib = library();
        let settings = WaveSettings {
            mood: WaveMood::Energetic,
            ..Default::default()
        };
        let mut s = Session::new(settings, 7);
        let picks = s.next(&lib, 6, Utc::now());
        assert_eq!(picks.len(), 6);
        for p in &picks {
            let c = lib.iter().find(|c| c.track_id == p.track_id).unwrap();
            assert!(c.energy.unwrap() >= 0.6, "«Бодрое» — только энергичное");
        }
        let mut calm = Session::new(
            WaveSettings {
                mood: WaveMood::Calm,
                ..Default::default()
            },
            7,
        );
        for p in calm.next(&lib, 6, Utc::now()) {
            let c = lib.iter().find(|c| c.track_id == p.track_id).unwrap();
            assert!(c.energy.unwrap() <= 0.5);
        }
    }

    #[test]
    fn never_same_artist_twice_in_a_row_and_no_repeats() {
        let lib = library();
        let mut s = Session::new(WaveSettings::default(), 3);
        let mut all = Vec::new();
        for _ in 0..5 {
            all.extend(s.next(&lib, 4, Utc::now()));
        }
        let ids: HashSet<Uuid> = all.iter().map(|p| p.track_id).collect();
        assert_eq!(ids.len(), all.len(), "без повторов в сессии");
        for w in all.windows(2) {
            let a = &lib
                .iter()
                .find(|c| c.track_id == w[0].track_id)
                .unwrap()
                .artist_ids;
            let b = &lib
                .iter()
                .find(|c| c.track_id == w[1].track_id)
                .unwrap()
                .artist_ids;
            assert_ne!(a, b, "тот же артист подряд");
        }
    }

    #[test]
    fn early_skip_rests_the_artist_and_steers_away() {
        let lib = library();
        let mut s = Session::new(WaveSettings::default(), 11);
        let first = s.next(&lib, 1, Utc::now());
        let skipped = lib
            .iter()
            .find(|c| c.track_id == first[0].track_id)
            .unwrap()
            .clone();
        s.react(&skipped, Reaction::Skip { position_ms: 5_000 });
        let next = s.next(&lib, 8, Utc::now());
        for p in &next {
            let c = lib.iter().find(|c| c.track_id == p.track_id).unwrap();
            assert!(
                !c.artist_ids.contains(&skipped.artist_ids[0]),
                "артист раннего пропуска отдыхает"
            );
        }
        // Два ранних пропуска громкого — волна уходит к тихому
        let loud = cand(500, 50, 0.95, 0.5);
        let loud2 = cand(501, 51, 0.9, 0.5);
        let mut s = Session::new(WaveSettings::default(), 5);
        s.react(&loud, Reaction::Skip { position_ms: 3_000 });
        s.react(&loud2, Reaction::Skip { position_ms: 3_000 });
        let picks = s.next(&lib, 5, Utc::now());
        let mean: f32 = picks
            .iter()
            .map(|p| {
                lib.iter()
                    .find(|c| c.track_id == p.track_id)
                    .unwrap()
                    .energy
                    .unwrap()
            })
            .sum::<f32>()
            / picks.len() as f32;
        assert!(
            mean < 0.5,
            "после пропусков громкого средняя энергия {mean}"
        );
    }

    #[test]
    fn played_recently_rests_unless_nothing_else() {
        let mut lib = library();
        let now = Utc::now();
        for c in lib.iter_mut().take(38) {
            c.plays_30d = 1;
            c.last_played = Some(now - Duration::hours(1));
        }
        let mut s = Session::new(WaveSettings::default(), 9);
        let picks = s.next(&lib, 2, now);
        let fresh: HashSet<Uuid> = lib[38..].iter().map(|c| c.track_id).collect();
        assert!(picks.iter().all(|p| fresh.contains(&p.track_id)));
        // Запас кончился — волна не обрывается
        let more = s.next(&lib, 4, now);
        assert_eq!(more.len(), 4);
    }

    #[test]
    fn language_is_never_relaxed() {
        let mut lib = library();
        lib[3].title = "Звезда".into();
        lib[17].artist_names = vec!["Кино".into()];
        let mut s = Session::new(
            WaveSettings {
                language: WaveLanguage::Ru,
                ..Default::default()
            },
            1,
        );
        let picks = s.next(&lib, 5, Utc::now());
        assert_eq!(picks.len(), 2);
    }

    #[test]
    fn camelot_and_tempo() {
        assert_eq!(camelot_compat(Some("8A"), Some("8A")), 1.0);
        assert_eq!(camelot_compat(Some("8A"), Some("9A")), 0.8);
        assert_eq!(camelot_compat(Some("12B"), Some("1B")), 0.8);
        assert_eq!(camelot_compat(Some("8A"), Some("2B")), 0.0);
        assert!(tempo_gap(80.0, 160.0) < 1e-6);
    }
}
