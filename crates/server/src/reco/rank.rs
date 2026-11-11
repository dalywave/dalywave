//! Правила волны (`rules-1`): границы занятия и настроения, ход энергии, «отдых» заслушанного, оценка
//! кандидатов, расстановка (разнообразие, без двух треков артиста подряд, стыки по темпу и Камелоту),
//! подписи «почему этот трек» и реакция сессии на лайки и пропуски. Чистые функции над [`Cand`]:
//! их проверяют модульные тесты, а [`super::wave`] только собирает данные и сохраняет итог.

use std::collections::{HashMap, HashSet};

use chrono::{DateTime, Duration, Utc};
use dalywave_protocol::{
    catalog::AudioStatus,
    wave::{
        Reason, ReasonKind, WaveActivity, WaveCharacter, WaveFeedbackKind, WaveLanguage, WaveMood,
        WaveSettings,
    },
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{
    Cand,
    vectors::{Rng, camelot_compat, tempo_gap},
};

/// Признак, которого нет (трек ещё не проанализирован), считается небольшим нарушением границ.
const UNKNOWN: f32 = 0.15;
/// Ступени ослабления границ, если строго подходящих мало.
const TOLERANCES: [f32; 4] = [0.0, 0.1, 0.2, 0.35];
/// Сколько последних артистов помнит сессия, чтобы не повторяться.
pub const RECENT_ARTISTS: usize = 8;
/// Сколько выданного помнит сессия: столько треков не повторится.
pub const SERVED_MEMORY: usize = 400;
/// Пропущенный рано или нелюбимый артист отдыхает столько треков.
const ARTIST_REST_STEPS: u32 = 10;

/// Откуда кандидат: от этого зависит подпись.
#[derive(Debug, Clone, PartialEq)]
pub enum Origin {
    /// Сосед грани вкуса по звучанию; `artists` — главные артисты грани для «Похоже на …».
    Facet { artists: Vec<(Uuid, String)> },
    /// Сосед «что сейчас заходит» сессии.
    Session { artist: Option<(Uuid, String)> },
    /// Сосед затравки радио.
    Seed { label: String, artist: Option<Uuid> },
    /// По описанию словами.
    Query { text: String },
    /// Лайкнутое.
    Favorite,
    /// Любимое, которое давно не играло.
    Forgotten,
    /// Свежий релиз артиста из подписок.
    NewRelease,
}

impl Origin {
    /// Чья подпись важнее, если трек пришёл из нескольких мест.
    fn rank(&self) -> u8 {
        match self {
            Self::Seed { .. } | Self::Query { .. } => 6,
            Self::Forgotten => 5,
            Self::NewRelease => 4,
            Self::Session { .. } => 3,
            Self::Facet { .. } => 2,
            Self::Favorite => 1,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Entry {
    pub cand: Cand,
    pub origin: Origin,
    /// Сходство с тем, рядом с чем трек нашёлся (косинус); у любимого и новинок — условное.
    pub sim: f32,
}

impl Origin {
    /// Сходство с затравкой радио, а не со вкусом.
    fn seeded(&self) -> bool {
        matches!(self, Self::Seed { .. } | Self::Query { .. })
    }
}

/// Один трек — одна запись в пуле: из нескольких источников остаётся самое объяснимое. Сходство
/// с затравкой и сходство со вкусом — разные шкалы: у затравки остаётся своё, у остальных — лучшее.
pub fn merge(entries: Vec<Entry>) -> Vec<Entry> {
    let mut index: HashMap<Uuid, usize> = HashMap::with_capacity(entries.len());
    let mut out: Vec<Entry> = Vec::with_capacity(entries.len());
    for e in entries {
        let key = e.cand.recording_id.unwrap_or(e.cand.track_id);
        let Some(&i) = index.get(&key) else {
            index.insert(key, out.len());
            out.push(e);
            continue;
        };
        let x = &mut out[i];
        let better = e.origin.rank() > x.origin.rank()
            || (e.origin.rank() == x.origin.rank() && e.sim > x.sim);
        if e.origin.seeded() || x.origin.seeded() {
            if better {
                *x = e;
            }
        } else {
            let sim = x.sim.max(e.sim);
            if better {
                *x = e;
            }
            x.sim = sim;
        }
    }
    out
}

/// Границы признаков из занятия, настроения и языка.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Constraints {
    pub energy: Option<(f32, f32)>,
    pub valence: Option<(f32, f32)>,
    pub danceability_min: Option<f32>,
    pub bpm: Option<(f32, f32)>,
    /// Темп бега: трек в темп шага или вдвое медленнее.
    pub tempo: Option<f32>,
    pub voice_max: Option<f32>,
    pub language: WaveLanguage,
}

impl Constraints {
    pub fn any(&self) -> bool {
        self.energy.is_some()
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
    // Противоречие («Засыпаю» и «Бодрое»): узкая полоса посередине, остальное решит ослабление
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

/// Русский ли трек. Язык вокала сервис анализа пока не определяет, поэтому — кириллица в названии
/// или в имени артиста; определённый язык, когда появится, главнее.
pub fn russian(c: &Cand) -> bool {
    match c.language.as_deref() {
        Some(lang) => lang == "ru",
        None => has_cyrillic(&c.title) || c.artist_names.iter().any(|n| has_cyrillic(n)),
    }
}

/// Насколько трек выходит за границы: 0 — подходит. Язык не ослабляется.
pub fn violation(c: &Cand, k: &Constraints) -> f32 {
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
        // 10 BPM мимо границы — как 0,1 по энергии
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
    v
}

/// Ход энергии по занятию: «Засыпаю» снижает, «Просыпаюсь» поднимает. `None` — ровный поток.
pub fn target_energy(s: &WaveSettings, step: u32) -> Option<f32> {
    let n = step as f32;
    match s.activity {
        WaveActivity::Sleep => Some((0.45 - 0.02 * n).max(0.12)),
        WaveActivity::Wake => Some((0.3 + 0.03 * n).min(0.8)),
        _ => None,
    }
}

/// Доля незнакомого по характеру.
pub fn new_share(c: WaveCharacter) -> f32 {
    match c {
        WaveCharacter::Fav => 0.1,
        WaveCharacter::Mix => 0.45,
        WaveCharacter::New => 0.85,
        WaveCharacter::Pop => 0.5,
        WaveCharacter::Rare => 0.7,
    }
}

/// Популярность 0..1: ранг Deezer (до ~1 000 000, логарифм выравнивает хвост) и «массовость» звучания.
pub fn popularity(c: &Cand) -> f32 {
    let r = c.rank.unwrap_or(0).max(0) as f32;
    let by_rank = ((r + 1.0).ln() / 1_000_000f32.ln()).clamp(0.0, 1.0);
    match c.approachability {
        Some(a) => 0.75 * by_rank + 0.25 * a.clamp(0.0, 1.0),
        None => by_rank,
    }
}

/// Сколько трек «отдыхает» после последнего прослушивания: чем чаще его играли за месяц, тем дольше —
/// 6 часов, 12, сутки, двое… до 30 дней. В «Любимом» отдых вдвое короче.
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

pub fn resting(c: &Cand, character: WaveCharacter, now: DateTime<Utc>) -> bool {
    match c.last_played {
        Some(at) => now - at < rest(c.plays_30d.max(1), character),
        None => false,
    }
}

/// Трек можно сыграть: файл на сервере или его можно скачать.
pub fn acquirable(c: &Cand) -> bool {
    c.ready()
        || (c.deezer_id.is_some()
            && !matches!(
                c.audio_status,
                Some(AudioStatus::Unavailable | AudioStatus::Disputed | AudioStatus::Failed)
            ))
}

/// Состояние сессии волны (`wave_sessions.state`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SessionState {
    /// Выданное: id записей (или трека без записи), последние [`SERVED_MEMORY`].
    pub served: Vec<Uuid>,
    /// Сколько выдано всего.
    pub step: u32,
    /// Сколько из них — незнакомое.
    pub new_served: u32,
    pub last: Option<LastTrack>,
    pub recent_artists: Vec<Uuid>,
    /// Вес векторов «заходит» и «пропускается»: сколько реакций в них сложено.
    pub hits_weight: f32,
    pub misses_weight: f32,
    pub skip_streak: u32,
    pub rested_artists: Vec<RestedArtist>,
    /// Артист последнего «зашедшего» трека — для «Похоже на …».
    pub hit_artist: Option<NamedRef>,
    /// Выданные песни (артист и название без версии): «Slowed» и «Sped Up» той же песни — не новый трек.
    pub songs: Vec<String>,
    /// Подпись сессии: «Моя волна», «Радио: Lowborn».
    pub title: String,
    /// Затравка радио: как её назвать в подписи и её артист.
    pub seed_label: Option<String>,
    pub seed_artist: Option<Uuid>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LastTrack {
    pub track_id: Uuid,
    pub artist_ids: Vec<Uuid>,
    pub bpm: Option<f32>,
    pub key_camelot: Option<String>,
    pub energy: Option<f32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RestedArtist {
    pub artist_id: Uuid,
    pub until_step: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NamedRef {
    pub id: Uuid,
    pub name: String,
}

/// Песня: главный артист и название без версии (её Deezer держит отдельно).
pub fn song(c: &Cand) -> String {
    let artist = c.artist_ids.first().copied().unwrap_or(c.track_id);
    format!("{artist}:{}", c.title.trim().to_lowercase())
}

impl SessionState {
    pub fn served_set(&self) -> HashSet<Uuid> {
        self.served.iter().copied().collect()
    }

    pub fn served_song(&self, c: &Cand) -> bool {
        self.songs.contains(&song(c))
    }

    pub fn artist_rested(&self, c: &Cand) -> bool {
        self.rested_artists
            .iter()
            .any(|r| r.until_step > self.step && c.artist_ids.contains(&r.artist_id))
    }

    /// Выданный трек: запомнить, сдвинуть шаг, последний трек и недавних артистов.
    pub fn push(&mut self, c: &Cand) {
        self.served.push(c.recording_id.unwrap_or(c.track_id));
        if self.served.len() > SERVED_MEMORY {
            let extra = self.served.len() - SERVED_MEMORY;
            self.served.drain(..extra);
        }
        self.songs.push(song(c));
        if self.songs.len() > SERVED_MEMORY {
            let extra = self.songs.len() - SERVED_MEMORY;
            self.songs.drain(..extra);
        }
        self.step += 1;
        if !c.familiar() {
            self.new_served += 1;
        }
        self.last = Some(LastTrack {
            track_id: c.track_id,
            artist_ids: c.artist_ids.clone(),
            bpm: c.bpm.filter(|_| c.bpm_confidence.unwrap_or(0.0) >= 0.3),
            key_camelot: c
                .key_camelot
                .clone()
                .filter(|_| c.key_confidence.unwrap_or(0.0) >= 0.25),
            energy: c.energy,
        });
        if let Some(a) = c.artist_ids.first() {
            self.recent_artists.retain(|x| x != a);
            self.recent_artists.push(*a);
            if self.recent_artists.len() > RECENT_ARTISTS {
                self.recent_artists.remove(0);
            }
        }
        self.rested_artists.retain(|r| r.until_step > self.step);
    }
}

/// Что реакция делает с сессией.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Effect {
    /// Сколько веса добавить в «заходит».
    pub hit: f32,
    /// Сколько — в «пропускается».
    pub miss: f32,
    /// Артисту — отдохнуть до конца ближайших треков.
    pub rest_artist: bool,
    /// Пропуск подряд (`Some(true)`), конец серии (`Some(false)`) или без изменений.
    pub skip: Option<bool>,
}

pub fn effect(kind: WaveFeedbackKind, position_ms: Option<i32>) -> Effect {
    // Ранний пропуск (первые 30 секунд) говорит о треке больше позднего
    let early = position_ms.is_none_or(|p| p < 30_000);
    match kind {
        WaveFeedbackKind::Like => Effect {
            hit: 1.0,
            miss: 0.0,
            rest_artist: false,
            skip: Some(false),
        },
        WaveFeedbackKind::MoreLikeThis => Effect {
            hit: 1.5,
            miss: 0.0,
            rest_artist: false,
            skip: Some(false),
        },
        WaveFeedbackKind::Completed => Effect {
            hit: 0.5,
            miss: 0.0,
            rest_artist: false,
            skip: Some(false),
        },
        WaveFeedbackKind::Skip => Effect {
            hit: 0.0,
            miss: if early { 1.0 } else { 0.4 },
            rest_artist: early,
            skip: Some(true),
        },
        WaveFeedbackKind::Dislike => Effect {
            hit: 0.0,
            miss: 1.5,
            rest_artist: true,
            skip: Some(true),
        },
        // «Не сейчас» — без штрафа: трек просто не вернётся в этой сессии
        WaveFeedbackKind::NotNow => Effect {
            hit: 0.0,
            miss: 0.0,
            rest_artist: false,
            skip: None,
        },
    }
}

impl SessionState {
    pub fn apply(&mut self, effect: &Effect, track: &Cand) {
        match effect.skip {
            Some(true) => self.skip_streak += 1,
            Some(false) => self.skip_streak = 0,
            None => {}
        }
        if effect.rest_artist {
            for a in &track.artist_ids {
                self.rested_artists.retain(|r| r.artist_id != *a);
                self.rested_artists.push(RestedArtist {
                    artist_id: *a,
                    until_step: self.step + ARTIST_REST_STEPS,
                });
            }
        }
        if effect.hit > 0.0
            && let Some((id, name)) = track.main_artist()
        {
            self.hit_artist = Some(NamedRef {
                id,
                name: name.to_string(),
            });
        }
        let key = track.recording_id.unwrap_or(track.track_id);
        if !self.served.contains(&key) {
            self.served.push(key);
        }
    }
}

/// Всё, что нужно оценке и расстановке, кроме самих кандидатов.
#[derive(Debug, Clone)]
pub struct RankContext<'a> {
    pub settings: &'a WaveSettings,
    pub state: &'a SessionState,
    pub can_acquire: bool,
    pub now: DateTime<Utc>,
    /// Сходство, начиная с которого незнакомый трек подписывается «Похоже на …», а не «Новое для тебя».
    pub similar_threshold: f32,
}

impl RankContext<'_> {
    /// Радио: сессия с затравкой.
    pub fn seeded(&self) -> bool {
        self.settings.seed.is_some()
    }
}

#[derive(Debug, Clone)]
pub struct Scored {
    pub entry: Entry,
    pub score: f32,
    pub violation: f32,
}

/// Кандидаты, которых нельзя предлагать вовсе: «не рекомендовать», «не нравится», уже выданное,
/// отдыхающее, недоступное для скачивания, отдыхающий в сессии артист.
pub fn admissible(e: &Entry, ctx: &RankContext, exclude: &HashSet<Uuid>) -> bool {
    let c = &e.cand;
    let key = c.recording_id.unwrap_or(c.track_id);
    !c.excluded
        && !exclude.contains(&c.track_id)
        && !exclude.contains(&key)
        && !ctx.state.served_song(c)
        && !resting(c, ctx.settings.character, ctx.now)
        && acquirable(c)
        && !ctx.state.artist_rested(c)
}

/// Тяга настроения и занятия: границы отсекают явно чужое, а внутри них «Спокойное» тянет к тихому,
/// «Бодрое» — к громкому, «Весёлое» и «Грустное» — по позитивности. Шкала признаков у сервиса анализа
/// сжата (у громкой медиатеки энергия почти вся выше 0,5), поэтому без тяги ослабленные границы
/// пропускали бы всё подряд. Неизвестный признак — середина шкалы.
pub fn pull(s: &WaveSettings, c: &Cand) -> f32 {
    const MOOD: f32 = 1.2;
    const ACTIVITY: f32 = 0.5;
    let energy = c.energy.unwrap_or(0.6);
    let valence = c.valence.unwrap_or(0.5);
    let mood = match s.mood {
        WaveMood::Any => 0.0,
        WaveMood::Energetic => MOOD * energy,
        WaveMood::Calm => -MOOD * energy,
        WaveMood::Happy => MOOD * valence,
        WaveMood::Sad => -MOOD * valence,
    };
    let activity = match s.activity {
        WaveActivity::Sport | WaveActivity::Party => ACTIVITY * energy,
        WaveActivity::Work | WaveActivity::Rest => -ACTIVITY * energy,
        // Ход энергии «Засыпаю» и «Просыпаюсь» задаёт расстановка
        WaveActivity::Any | WaveActivity::Road | WaveActivity::Sleep | WaveActivity::Wake => 0.0,
    };
    mood + activity
}

/// Оценка без учёта соседей: сходство, характер, свежесть, популярность, реакции сессии и чуть случайности,
/// чтобы поток не замыкался.
pub fn score(e: &Entry, ctx: &RankContext, rng: &mut Rng) -> f32 {
    let c = &e.cand;
    let familiar = c.familiar();
    let pop = popularity(c);
    let mut s = 0.6 * e.sim;
    // Радио держится затравки: соседи вкуса — только подмешиваются
    if ctx.seeded() && !e.origin.seeded() {
        s -= 0.5;
    }
    match ctx.settings.character {
        WaveCharacter::Fav if familiar => s += 0.3,
        WaveCharacter::Fav => {}
        WaveCharacter::Mix => {}
        WaveCharacter::New if familiar => s -= 0.3,
        WaveCharacter::New => s += 0.1,
        WaveCharacter::Pop => s += 0.35 * pop - 0.1,
        WaveCharacter::Rare => s += 0.3 * (1.0 - pop) - 0.1,
    }
    if c.liked {
        s += 0.08;
    }
    s += pull(ctx.settings, c);
    if let Some(h) = c.hit_sim {
        s += 0.3 * (h - 0.5);
    }
    let streak = if ctx.state.skip_streak >= 2 { 2.0 } else { 1.0 };
    if let Some(m) = c.miss_sim {
        s -= 0.35 * streak * (m - 0.5).max(0.0);
    }
    if let Some(d) = c.release_date {
        let age = (ctx.now.date_naive() - d).num_days();
        if (0..=60).contains(&age) {
            s += 0.05;
        }
    }
    if c.ready() {
        s += 0.03;
    } else if !ctx.can_acquire {
        // Качать некому: волна играет то, что уже на сервере
        s -= 1.0;
    }
    let jitter = if ctx.state.skip_streak >= 2 { 0.2 } else { 0.1 };
    s + jitter * rng.next_f32()
}

/// Отбор с ослаблением границ: самая строгая ступень, на которой набирается вдвое больше нужного,
/// иначе — лучшие по нарушению.
pub fn within_bounds(scored: Vec<Scored>, count: usize, constraints: &Constraints) -> Vec<Scored> {
    if !constraints.any() {
        return scored;
    }
    for tol in TOLERANCES {
        let fit = scored.iter().filter(|s| s.violation <= tol + 1e-6).count();
        if fit >= count * 2 || (tol == TOLERANCES[TOLERANCES.len() - 1] && fit >= count) {
            return scored
                .into_iter()
                .filter(|s| s.violation <= tol + 1e-6)
                .collect();
        }
    }
    // Даже ослабленным границам подходит мало: лучшие по нарушению, но язык — никогда
    let mut rest: Vec<Scored> = scored.into_iter().filter(|s| s.violation < 10.0).collect();
    rest.sort_by(|a, b| a.violation.total_cmp(&b.violation));
    rest.truncate(count * 3);
    rest
}

/// Расстановка: жадно, слот за слотом. Никогда — тот же артист подряд; реже — недавние артисты;
/// доля незнакомого — по характеру; ход энергии — по занятию; соседние треки — близкие по темпу
/// и совместимые по Камелоту.
pub fn sequence(mut pool: Vec<Scored>, count: usize, ctx: &RankContext) -> Vec<Scored> {
    let share = new_share(ctx.settings.character);
    let mut out: Vec<Scored> = Vec::with_capacity(count);
    let mut state = ctx.state.clone();
    let mut batch_artists: Vec<Uuid> = Vec::new();
    while out.len() < count && !pool.is_empty() {
        let target = target_energy(ctx.settings, state.step);
        let want_new = (state.new_served as f32) < share * (state.step + 1) as f32;
        let prev_artists: &[Uuid] = state.last.as_ref().map_or(&[], |l| &l.artist_ids);
        let mut best: Option<(usize, f32)> = None;
        for (i, s) in pool.iter().enumerate() {
            let c = &s.entry.cand;
            if c.artist_ids.iter().any(|a| prev_artists.contains(a)) || state.served_song(c) {
                continue;
            }
            let mut v = s.score;
            if c.familiar() != want_new {
                v += 0.25;
            }
            if c.artist_ids
                .iter()
                .any(|a| state.recent_artists.iter().rev().take(4).any(|r| r == a))
            {
                v -= 0.3;
            }
            let repeats = c
                .artist_ids
                .iter()
                .map(|a| batch_artists.iter().filter(|b| *b == a).count())
                .max()
                .unwrap_or(0);
            v -= 0.25 * repeats as f32;
            // Ход энергии — суть «Засыпаю» и «Просыпаюсь»: он сильнее знакомости и разнообразия
            if let Some(t) = target {
                v -= match c.energy {
                    Some(e) => 1.5 * (e - t).abs(),
                    None => 0.3,
                };
            }
            if let Some(last) = &state.last {
                let key = c
                    .key_camelot
                    .as_deref()
                    .filter(|_| c.key_confidence.unwrap_or(0.0) >= 0.25);
                if let (Some(a), Some(b)) = (last.key_camelot.as_deref(), key) {
                    v += 0.12 * (camelot_compat(Some(a), Some(b)) - 0.5);
                }
                let bpm = c.bpm.filter(|_| c.bpm_confidence.unwrap_or(0.0) >= 0.3);
                if let (Some(a), Some(b)) = (last.bpm, bpm) {
                    v -= 0.6 * (tempo_gap(a, b) - 0.06).max(0.0);
                }
            }
            if best.is_none_or(|(_, bv)| v > bv) {
                best = Some((i, v));
            }
        }
        // Остались только треки того же артиста: лучше короче выдача, чем повтор подряд
        let Some((i, _)) = best else {
            break;
        };
        let pick = pool.swap_remove(i);
        batch_artists.extend(pick.entry.cand.artist_ids.iter().copied());
        state.push(&pick.entry.cand);
        out.push(pick);
    }
    out
}

/// Подпись «почему этот трек».
pub fn reason(e: &Entry, ctx: &RankContext) -> Reason {
    let c = &e.cand;
    let r = |kind, text: String| Reason { kind, text };
    let own = |id: &Uuid| c.artist_ids.contains(id);
    match &e.origin {
        Origin::Forgotten => return r(ReasonKind::Forgotten, "Давно не слушали".into()),
        Origin::NewRelease => {
            let text = match c.main_artist() {
                Some((_, name)) => format!("Новинка: {name}"),
                None => "Новинка из подписок".into(),
            };
            return r(ReasonKind::NewRelease, text);
        }
        Origin::Query { text } => {
            return r(ReasonKind::Query, format!("По запросу «{text}»"));
        }
        Origin::Seed { label, artist } => {
            let text = match (artist, c.main_artist()) {
                (Some(a), Some((_, name))) if own(a) => format!("Ещё {name}"),
                _ => format!("Похоже на {label}"),
            };
            return r(ReasonKind::Seed, text);
        }
        _ => {}
    }
    if c.liked && c.completed >= 5 {
        return r(ReasonKind::Often, "Вы часто слушаете".into());
    }
    if c.liked {
        return r(ReasonKind::Favorite, "Из любимого".into());
    }
    if c.completed >= 3 {
        return r(ReasonKind::Often, "Вы часто слушаете".into());
    }
    let familiar = c.familiar();
    let pop = popularity(c);
    if !familiar && ctx.settings.character == WaveCharacter::Pop && pop >= 0.75 {
        return r(ReasonKind::Popular, "Популярное у слушателей".into());
    }
    if !familiar && ctx.settings.character == WaveCharacter::Rare && pop <= 0.4 {
        return r(ReasonKind::Rare, "Редкая находка".into());
    }
    let reference = match &e.origin {
        Origin::Facet { artists } => artists.iter().find(|(id, _)| !own(id)).cloned(),
        Origin::Session { artist } => artist.clone().filter(|(id, _)| !own(id)),
        _ => None,
    };
    match reference {
        Some((_, name)) if e.sim >= ctx.similar_threshold || c.artist_familiar => {
            let kind = if matches!(e.origin, Origin::Session { .. }) {
                ReasonKind::Session
            } else {
                ReasonKind::Similar
            };
            r(kind, format!("Похоже на {name}"))
        }
        _ if !familiar => r(ReasonKind::NewToYou, "Новое для тебя".into()),
        _ => r(ReasonKind::Favorite, "Из любимого".into()),
    }
}

/// Медиана сходства соседей граней и сессии: выше неё незнакомое подписывается «Похоже на …».
pub fn similar_threshold(entries: &[Entry]) -> f32 {
    let mut sims: Vec<f32> = entries
        .iter()
        .filter(|e| matches!(e.origin, Origin::Facet { .. } | Origin::Session { .. }))
        .map(|e| e.sim)
        .collect();
    if sims.is_empty() {
        return 1.0;
    }
    sims.sort_by(|a, b| a.total_cmp(b));
    sims[sims.len() / 2]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cand(n: u128, artist: u128) -> Cand {
        Cand {
            track_id: Uuid::from_u128(n),
            recording_id: Some(Uuid::from_u128(1000 + n)),
            deezer_id: Some(n as i64),
            title: format!("track {n}"),
            artist_ids: vec![Uuid::from_u128(artist)],
            artist_names: vec![format!("artist {artist}")],
            audio_status: Some(AudioStatus::None),
            analyzed: true,
            energy: Some(0.5),
            valence: Some(0.5),
            ..Default::default()
        }
    }

    fn entry(c: Cand, sim: f32) -> Entry {
        Entry {
            cand: c,
            origin: Origin::Facet {
                artists: vec![(Uuid::from_u128(9), "Lowborn".into())],
            },
            sim,
        }
    }

    fn ctx<'a>(settings: &'a WaveSettings, state: &'a SessionState) -> RankContext<'a> {
        RankContext {
            settings,
            state,
            can_acquire: true,
            now: Utc::now(),
            similar_threshold: 0.8,
        }
    }

    #[test]
    fn moods_and_activities_set_bounds() {
        let s = |activity, mood| WaveSettings {
            activity,
            mood,
            ..Default::default()
        };
        let calm = constraints(&s(WaveActivity::Any, WaveMood::Calm));
        assert_eq!(calm.energy, Some((0.0, 0.5)));
        let mut c = cand(1, 1);
        c.energy = Some(0.8);
        assert!((violation(&c, &calm) - 0.3).abs() < 1e-6);
        c.energy = Some(0.4);
        assert_eq!(violation(&c, &calm), 0.0);
        c.energy = None;
        assert_eq!(violation(&c, &calm), UNKNOWN);

        // «Тренируюсь» с темпом бега 170: трек в 85 BPM — вдвое медленнее шага, подходит
        let run = constraints(&WaveSettings {
            activity: WaveActivity::Sport,
            tempo: Some(170),
            ..Default::default()
        });
        assert_eq!(run.tempo, Some(170.0));
        let mut c = cand(2, 2);
        c.energy = Some(0.8);
        c.bpm = Some(85.0);
        assert_eq!(violation(&c, &run), 0.0);
        c.bpm = Some(128.0);
        assert!(violation(&c, &run) > 0.1);
        // Без темпа — диапазон
        let sport = constraints(&s(WaveActivity::Sport, WaveMood::Any));
        assert_eq!(sport.bpm, Some((115.0, 185.0)));
        assert!(violation(&c, &sport) < 1e-6);

        // Противоречие сводится к узкой полосе
        let odd = constraints(&s(WaveActivity::Sleep, WaveMood::Energetic));
        let (lo, hi) = odd.energy.unwrap();
        assert!(lo < hi && (hi - lo - 0.1).abs() < 1e-6);
        let party = constraints(&s(WaveActivity::Party, WaveMood::Happy));
        assert_eq!(party.danceability_min, Some(0.55));
        assert_eq!(party.valence, Some((0.55, 1.0)));
    }

    #[test]
    fn language_by_voice_and_script() {
        let words = constraints(&WaveSettings {
            language: WaveLanguage::None,
            ..Default::default()
        });
        let mut c = cand(1, 1);
        c.voice = Some(0.9);
        assert!(violation(&c, &words) > 0.4);
        c.voice = Some(0.1);
        assert_eq!(violation(&c, &words), 0.0);

        let ru = constraints(&WaveSettings {
            language: WaveLanguage::Ru,
            ..Default::default()
        });
        let foreign = constraints(&WaveSettings {
            language: WaveLanguage::Foreign,
            ..Default::default()
        });
        let mut c = cand(3, 3);
        c.title = "Лонг Айленд".into();
        assert!(russian(&c));
        assert_eq!(violation(&c, &ru), 0.0);
        assert!(violation(&c, &foreign) >= 10.0);
        c.title = "Devil".into();
        c.artist_names = vec!["Lowborn".into()];
        assert!(!russian(&c));
        c.language = Some("ru".into());
        assert!(russian(&c), "определённый язык главнее письма");
    }

    #[test]
    fn overplayed_tracks_rest_longer() {
        assert_eq!(rest(0, WaveCharacter::Mix), Duration::zero());
        assert_eq!(rest(1, WaveCharacter::Mix), Duration::hours(6));
        assert_eq!(rest(3, WaveCharacter::Mix), Duration::hours(24));
        assert_eq!(rest(3, WaveCharacter::Fav), Duration::hours(12));
        assert_eq!(rest(50, WaveCharacter::Mix), Duration::days(30));
        let now = Utc::now();
        let mut c = cand(1, 1);
        c.last_played = Some(now - Duration::hours(3));
        c.plays_30d = 1;
        assert!(resting(&c, WaveCharacter::Mix, now));
        c.last_played = Some(now - Duration::hours(7));
        assert!(!resting(&c, WaveCharacter::Mix, now));
        c.plays_30d = 4;
        assert!(
            resting(&c, WaveCharacter::Mix, now),
            "заслушанный — 48 часов"
        );
        c.audio_status = Some(AudioStatus::Unavailable);
        assert!(!acquirable(&c));
        c.audio_status = Some(AudioStatus::Ready);
        assert!(acquirable(&c));
    }

    #[test]
    fn moods_pull_inside_the_bounds() {
        let calm = WaveSettings {
            mood: WaveMood::Calm,
            ..Default::default()
        };
        let loud = WaveSettings {
            mood: WaveMood::Energetic,
            activity: WaveActivity::Sport,
            ..Default::default()
        };
        let mut quiet = cand(1, 1);
        quiet.energy = Some(0.5);
        let mut noisy = cand(2, 2);
        noisy.energy = Some(0.8);
        assert!(pull(&calm, &quiet) > pull(&calm, &noisy));
        assert!(pull(&loud, &noisy) > pull(&loud, &quiet));
        assert!((pull(&loud, &noisy) - pull(&loud, &quiet) - 1.7 * 0.3).abs() < 1e-5);
        let sad = WaveSettings {
            mood: WaveMood::Sad,
            ..Default::default()
        };
        quiet.valence = Some(0.2);
        assert!(pull(&sad, &quiet) > pull(&sad, &noisy));
        assert_eq!(pull(&WaveSettings::default(), &noisy), 0.0);
    }

    #[test]
    fn energy_goes_down_to_sleep_and_up_to_wake() {
        let sleep = WaveSettings {
            activity: WaveActivity::Sleep,
            ..Default::default()
        };
        let a = target_energy(&sleep, 0).unwrap();
        let b = target_energy(&sleep, 10).unwrap();
        assert!(b < a);
        assert!(target_energy(&sleep, 1000).unwrap() >= 0.12);
        let wake = WaveSettings {
            activity: WaveActivity::Wake,
            ..Default::default()
        };
        assert!(target_energy(&wake, 10).unwrap() > target_energy(&wake, 0).unwrap());
        assert!(target_energy(&WaveSettings::default(), 3).is_none());
    }

    #[test]
    fn sequence_never_repeats_an_artist_in_a_row_and_follows_energy() {
        let settings = WaveSettings {
            activity: WaveActivity::Sleep,
            ..Default::default()
        };
        let state = SessionState::default();
        let ctx = ctx(&settings, &state);
        // Пять треков одного артиста с лучшими оценками и пять разных
        let mut pool = Vec::new();
        for i in 0..5 {
            let mut c = cand(i, 1);
            c.energy = Some(0.4 - 0.05 * i as f32);
            pool.push(Scored {
                entry: entry(c, 0.9),
                score: 1.0,
                violation: 0.0,
            });
        }
        for i in 5..10 {
            let mut c = cand(i, 10 + i);
            c.energy = Some(0.9 - 0.08 * (i - 5) as f32);
            pool.push(Scored {
                entry: entry(c, 0.7),
                score: 0.6,
                violation: 0.0,
            });
        }
        let out = sequence(pool, 8, &ctx);
        assert_eq!(out.len(), 8);
        for w in out.windows(2) {
            let a = &w[0].entry.cand.artist_ids;
            let b = &w[1].entry.cand.artist_ids;
            assert!(!a.iter().any(|x| b.contains(x)), "артист подряд");
        }
        // Другая версия уже выданной песни — не новый трек
        let mut slowed = cand(30, 31);
        slowed.title = "Track 30".into();
        let mut original = cand(31, 31);
        original.title = "track 30 ".into();
        let mut state = SessionState::default();
        state.push(&original);
        assert!(state.served_song(&slowed));
        let two = vec![
            Scored {
                entry: entry(slowed, 0.9),
                score: 1.0,
                violation: 0.0,
            },
            Scored {
                entry: entry(cand(32, 32), 0.5),
                score: 0.1,
                violation: 0.0,
            },
        ];
        let ctx2 = RankContext {
            state: &state,
            ..ctx.clone()
        };
        let picked = sequence(two, 2, &ctx2);
        assert_eq!(picked.len(), 1);
        assert_eq!(picked[0].entry.cand.track_id, Uuid::from_u128(32));
        // Пул из одного артиста: дальше первого трека не идём
        let mono: Vec<Scored> = (20..25)
            .map(|i| Scored {
                entry: entry(cand(i, 1), 0.9),
                score: 1.0,
                violation: 0.0,
            })
            .collect();
        assert_eq!(sequence(mono, 5, &ctx).len(), 1);
    }

    #[test]
    fn bounds_relax_only_when_needed() {
        let k = constraints(&WaveSettings {
            mood: WaveMood::Calm,
            ..Default::default()
        });
        let mk = |n: u128, v: f32| Scored {
            entry: entry(cand(n, n), 0.8),
            score: 0.5,
            violation: v,
        };
        let pool: Vec<Scored> = (0..10)
            .map(|n| mk(n, 0.0))
            .chain((10..20).map(|n| mk(n, 0.15)))
            .collect();
        assert_eq!(
            within_bounds(pool.clone(), 5, &k).len(),
            10,
            "строгих хватает"
        );
        assert_eq!(within_bounds(pool, 8, &k).len(), 20, "ослаблено до 0,2");
        let lang: Vec<Scored> = (0..3).map(|n| mk(n, 10.0)).collect();
        assert!(
            within_bounds(lang, 3, &k).is_empty(),
            "чужой язык не проходит никогда"
        );
    }

    #[test]
    fn reasons_explain_the_pick() {
        let settings = WaveSettings::default();
        let state = SessionState::default();
        let ctx = ctx(&settings, &state);
        let mut liked = cand(1, 1);
        liked.liked = true;
        liked.completed = 1;
        assert_eq!(reason(&entry(liked.clone(), 0.9), &ctx).text, "Из любимого");
        liked.completed = 7;
        assert_eq!(reason(&entry(liked, 0.9), &ctx).kind, ReasonKind::Often);
        // Незнакомое и близкое — «Похоже на …», далёкое — «Новое для тебя»
        let close = reason(&entry(cand(2, 2), 0.9), &ctx);
        assert_eq!(close.text, "Похоже на Lowborn");
        assert_eq!(close.kind, ReasonKind::Similar);
        let far = reason(&entry(cand(3, 3), 0.5), &ctx);
        assert_eq!(far.text, "Новое для тебя");
        // Трек самого Lowborn не «похож на Lowborn»
        let own = reason(&entry(cand(4, 9), 0.95), &ctx);
        assert_eq!(own.kind, ReasonKind::NewToYou);
        let forgotten = Entry {
            origin: Origin::Forgotten,
            ..entry(cand(5, 5), 0.5)
        };
        assert_eq!(reason(&forgotten, &ctx).text, "Давно не слушали");
        let radio = Entry {
            origin: Origin::Seed {
                label: "Lowborn".into(),
                artist: Some(Uuid::from_u128(9)),
            },
            ..entry(cand(6, 9), 0.9)
        };
        assert_eq!(reason(&radio, &ctx).text, "Ещё artist 9");
    }

    #[test]
    fn feedback_moves_the_session() {
        let mut state = SessionState::default();
        let c = cand(1, 1);
        state.push(&c);
        assert_eq!(state.step, 1);
        assert_eq!(state.new_served, 1);
        let skip = effect(WaveFeedbackKind::Skip, Some(5_000));
        assert_eq!(skip.miss, 1.0);
        state.apply(&skip, &c);
        assert_eq!(state.skip_streak, 1);
        assert!(
            state.artist_rested(&cand(2, 1)),
            "рано пропущенный артист отдыхает"
        );
        let late = effect(WaveFeedbackKind::Skip, Some(120_000));
        assert!(late.miss < 1.0 && !late.rest_artist);
        state.apply(&effect(WaveFeedbackKind::Like, None), &cand(3, 3));
        assert_eq!(state.skip_streak, 0);
        assert_eq!(state.hit_artist.as_ref().unwrap().name, "artist 3");
        let not_now = effect(WaveFeedbackKind::NotNow, None);
        assert_eq!((not_now.hit, not_now.miss), (0.0, 0.0));
        // Память выданного ограничена
        for i in 0..(SERVED_MEMORY as u128 + 10) {
            state.push(&cand(100 + i, 100 + i));
        }
        assert_eq!(state.served.len(), SERVED_MEMORY);
        assert!(state.recent_artists.len() <= RECENT_ARTISTS);
    }

    #[test]
    fn merge_keeps_one_entry_per_recording() {
        let a = entry(cand(1, 1), 0.7);
        let b = Entry {
            origin: Origin::Forgotten,
            ..entry(cand(1, 1), 0.4)
        };
        let merged = merge(vec![a, b, entry(cand(2, 2), 0.5)]);
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0].origin, Origin::Forgotten);
        assert_eq!(merged[0].sim, 0.7);
        // Затравка и вкус — разные шкалы: у радио остаётся сходство с затравкой
        let query = Entry {
            origin: Origin::Query {
                text: "дождь".into(),
            },
            ..entry(cand(3, 3), 0.1)
        };
        let merged = merge(vec![entry(cand(3, 3), 0.95), query]);
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].sim, 0.1);
        assert!(matches!(merged[0].origin, Origin::Query { .. }));
    }
}
