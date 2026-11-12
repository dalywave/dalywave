//! Своя сеть вкуса: по треку, контексту и граням вкуса предсказывает три вероятности — пропустит,
//! дослушает, лайкнет. Учится ночью в сервисе ml (numpy, ~150 тысяч параметров) на прослушиваниях
//! с отложенной по времени проверкой; включается сама, только если на отложенных данных точнее правил
//! (`rules-1`). До этого и при любом сбое поток собирают правила.
//!
//! Признаки строки — сырые: звучание (Discogs-EffNet, 1280), «смысл» (CLAP, 512) и [`SCALARS`]. Сжатие
//! векторов (PCA с выбеливанием по трекам обучающей части) и нормировка вшиты в ONNX, поэтому сервер
//! считает признаки одной функцией ([`scalars`]) и при обучении, и при подборе.

use std::{
    collections::{HashMap, HashSet},
    f32::consts::TAU,
    sync::{Arc, Mutex, OnceLock},
};

use base64::{Engine, engine::general_purpose::STANDARD};
use chrono::{DateTime, Datelike, Duration, NaiveDate, Timelike, Utc};
use dalywave_protocol::{
    jobs::kind,
    reco::{HeadMetrics, ModelTraining, ModelVersionView},
};
use ort::{environment::Environment, session::Session, value::Tensor};
use serde_json::{Value, json};
use sqlx::PgPool;
use uuid::Uuid;

use super::{
    Cand, MODEL_VERSION, Settings,
    rank::{self, Entry},
    taste::Facet,
    vectors::{SEMANTIC_DIM, SOUND_DIM, camelot, cosine},
};
use crate::{
    jobs::{
        self, JobRow, NewJob, lanes,
        worker::{Context, JobError, JobResult},
    },
    ml::MlError,
};

/// Головы сети в порядке выхода `probs`.
pub const HEADS: [&str; 3] = ["skip", "complete", "like"];

/// Числовые признаки строки в порядке входа сети (после векторов). Новый признак — только в конец:
/// иначе старая версия получит не те числа (сервер проверяет лишь ширину входа).
pub const SCALARS: [&str; 36] = [
    // Трек
    "bpm",
    "bpm_known",
    "key_sin",
    "key_cos",
    "key_minor",
    "key_known",
    "energy",
    "valence",
    "danceability",
    "voice",
    "approachability",
    "relaxed",
    "party",
    "has_semantic",
    "popularity",
    "fresh_release",
    // Контекст
    "hour_sin",
    "hour_cos",
    "weekday_sin",
    "weekday_cos",
    "weekend",
    "device_mobile",
    "device_desktop",
    "out_headphones",
    "out_wireless",
    "out_speaker",
    "session_pos",
    "skip_streak",
    "in_wave",
    // Вкус и знакомство
    "taste_sim",
    "facet_weight",
    "has_facets",
    "liked",
    "completed",
    "artist_familiar",
    "plays_30d",
];

/// Ширина входа сети.
pub const FEATURE_DIM: usize = SOUND_DIM + SEMANTIC_DIM + SCALARS.len();
/// Меньше размеченных прослушиваний — сеть не учится: правила лучше переобученной сети.
pub const MIN_ROWS: usize = 300;
/// Сколько строк, самое меньшее, уходит на проверку.
pub const MIN_HOLDOUT: usize = 60;
/// Самые поздние 20 % строк — отложенная проверка (запасной размер, когда истории по времени мало).
const HOLDOUT_SHARE: f64 = 0.2;
/// Отложенная проверка — последние недели истории: в неё попадает и импорт за эти недели, а не только
/// горстка свежих прослушиваний в DalyWave, на которых правила и сеть ведут себя иначе.
const HOLDOUT_WEEKS: i64 = 4;
/// Больше этой доли на проверку не уходит: иначе учить не на чем.
const MAX_HOLDOUT_SHARE: f64 = 0.4;
/// Последние прослушивания за год, не больше стольких.
const MAX_ROWS: i64 = 30_000;
/// Чтобы включиться, сеть должна обогнать правила по средней AUC хотя бы на столько (и не уступить в logloss).
const AUC_MARGIN: f64 = 0.005;
/// Вес оценки сети в общей оценке волны (относительно среднего по кандидатам).
const MODEL_WEIGHT: f32 = 0.35;
/// Копии ONNX в базе держатся у активной и стольких последних версий.
const KEEP_ARTIFACTS: i64 = 5;
/// Версии сети называются `nn-ГГГГММДД-ччммсс`.
pub const PREFIX: &str = "nn-";

// ───────────── Признаки ─────────────

/// Что известно о треке из анализа и каталога.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TrackFeat {
    pub bpm: Option<f32>,
    pub bpm_confidence: Option<f32>,
    pub key_camelot: Option<String>,
    pub key_confidence: Option<f32>,
    pub energy: Option<f32>,
    pub valence: Option<f32>,
    pub danceability: Option<f32>,
    pub voice: Option<f32>,
    pub approachability: Option<f32>,
    pub relaxed: Option<f32>,
    pub party: Option<f32>,
    pub rank: Option<i32>,
    pub release_date: Option<NaiveDate>,
}

impl TrackFeat {
    pub fn of(c: &Cand) -> Self {
        Self {
            bpm: c.bpm,
            bpm_confidence: c.bpm_confidence,
            key_camelot: c.key_camelot.clone(),
            key_confidence: c.key_confidence,
            energy: c.energy,
            valence: c.valence,
            danceability: c.danceability,
            voice: c.voice,
            approachability: c.approachability,
            relaxed: c.relaxed,
            party: c.party,
            rank: c.rank,
            release_date: c.release_date,
        }
    }
}

/// Когда и где играет: местное время слушателя, устройство, выход звука, ход сессии.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Moment {
    pub at: DateTime<Utc>,
    pub utc_offset_min: i16,
    /// `web`, `macos`, `windows`, `ios`, `ipados`…
    pub platform: Option<String>,
    /// `headphones`, `speaker`, `bluetooth`, `airplay`…
    pub output: Option<String>,
    /// Сколько треков сессии прозвучало до этого.
    pub session_pos: u32,
    /// Пропусков подряд перед этим треком.
    pub skip_streak: u32,
    /// Волна или радио, а не альбом и плейлист.
    pub in_wave: bool,
}

/// Вкус и знакомство с треком на тот момент.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Familiarity {
    /// Сходство с ближайшей гранью вкуса.
    pub taste_sim: Option<f32>,
    /// Вес этой грани.
    pub facet_weight: f32,
    pub liked: bool,
    /// Дослушиваний трека до этого.
    pub completed: i64,
    pub artist_familiar: bool,
    /// Прослушиваний трека за 30 дней до этого.
    pub plays_30d: i64,
}

/// Ближайшая грань: сходство и её вес. Граней нет — `None`.
pub fn nearest_facet(sound: &[f32], facets: &[(&[f32], f32)]) -> (Option<f32>, f32) {
    facets
        .iter()
        .map(|(c, w)| (cosine(sound, c), *w))
        .max_by(|a, b| a.0.total_cmp(&b.0))
        .map_or((None, 0.0), |(s, w)| (Some(s), w))
}

fn log_scaled(n: i64) -> f32 {
    ((n.max(0) as f32).ln_1p() / 20f32.ln()).min(1.0)
}

fn fresh(release: Option<NaiveDate>, at: DateTime<Utc>) -> bool {
    release.is_some_and(|d| (0..=60).contains(&(at.date_naive() - d).num_days()))
}

/// Числовые признаки строки — одни и те же при обучении и при подборе.
pub fn scalars(
    t: &TrackFeat,
    has_semantic: bool,
    m: &Moment,
    f: &Familiarity,
) -> [f32; SCALARS.len()] {
    let bpm_known = t.bpm.is_some() && t.bpm_confidence.unwrap_or(0.0) >= 0.3;
    let key = t
        .key_camelot
        .as_deref()
        .filter(|_| t.key_confidence.unwrap_or(0.0) >= 0.25)
        .and_then(camelot);
    let (key_sin, key_cos, key_minor) = key.map_or((0.0, 0.0, 0.0), |(n, letter)| {
        let a = TAU * f32::from(n) / 12.0;
        (a.sin(), a.cos(), if letter == 'A' { 1.0 } else { 0.0 })
    });
    let local = m.at + Duration::minutes(i64::from(m.utc_offset_min));
    let hour = (local.hour() as f32 + local.minute() as f32 / 60.0) / 24.0 * TAU;
    let wd = local.weekday().num_days_from_monday();
    let day = wd as f32 / 7.0 * TAU;
    let platform = m.platform.as_deref().unwrap_or("web");
    let output = m.output.as_deref().unwrap_or("");
    let pop = rank::popularity(&Cand {
        rank: t.rank,
        approachability: t.approachability,
        ..Default::default()
    });
    let b = |x: bool| if x { 1.0 } else { 0.0 };
    [
        if bpm_known {
            t.bpm.unwrap_or(0.0) / 200.0
        } else {
            0.0
        },
        b(bpm_known),
        key_sin,
        key_cos,
        key_minor,
        b(key.is_some()),
        t.energy.unwrap_or(0.6),
        t.valence.unwrap_or(0.5),
        t.danceability.unwrap_or(0.5),
        t.voice.unwrap_or(0.5),
        t.approachability.unwrap_or(0.5),
        t.relaxed.unwrap_or(0.5),
        t.party.unwrap_or(0.5),
        b(has_semantic),
        pop,
        b(fresh(t.release_date, m.at)),
        hour.sin(),
        hour.cos(),
        day.sin(),
        day.cos(),
        b(wd >= 5),
        b(matches!(platform, "ios" | "ipados" | "android")),
        b(matches!(platform, "macos" | "windows" | "linux")),
        b(output == "headphones"),
        b(matches!(output, "bluetooth" | "airplay")),
        b(matches!(output, "speaker" | "builtin")),
        ((1.0 + m.session_pos as f32).ln() / 50f32.ln()).min(1.0),
        m.skip_streak.min(5) as f32 / 5.0,
        b(m.in_wave),
        f.taste_sim.unwrap_or(0.0),
        f.facet_weight,
        b(f.taste_sim.is_some()),
        b(f.liked),
        log_scaled(f.completed),
        b(f.artist_familiar),
        log_scaled(f.plays_30d),
    ]
}

/// Оценка правил для той же строки — с чем сравнивается сеть: близость ко вкусу, лайк и свежий релиз,
/// как в [`rank::score`] без настроек волны и хода сессии.
pub fn rules_prior(t: &TrackFeat, f: &Familiarity, at: DateTime<Utc>) -> f32 {
    let mut s = 0.6 * f.taste_sim.unwrap_or(0.0);
    if f.liked {
        s += 0.08;
    }
    if fresh(t.release_date, at) {
        s += 0.05;
    }
    s
}

/// Метки прослушивания: пропуск, дослушивание, лайк. Сбой, обрыв, передача на другое устройство и
/// остановка посреди трека ничего не говорят о вкусе — без метки.
pub fn label(end_reason: &str, played_ms: i32, duration_ms: i32, liked: bool) -> Option<[f32; 3]> {
    if duration_ms <= 0 {
        return None;
    }
    let played = played_ms.max(0) as f32 / duration_ms as f32;
    let complete = end_reason == "completed" || played >= 0.8;
    let skip = !complete && matches!(end_reason, "skipped" | "previous") && played < 0.5;
    match end_reason {
        "error" | "interrupted" | "transferred" => return None,
        "stopped" if !complete => return None,
        _ => {}
    }
    let b = |x: bool| if x { 1.0 } else { 0.0 };
    Some([b(skip), b(complete), b(liked)])
}

/// Строка признаков: векторы и числа подряд.
fn row(sound: &[f32], semantic: Option<&[f32]>, scalars: &[f32]) -> Vec<f32> {
    let mut v = Vec::with_capacity(FEATURE_DIM);
    v.extend_from_slice(sound);
    match semantic {
        Some(s) => v.extend_from_slice(s),
        None => v.extend(std::iter::repeat_n(0.0, SEMANTIC_DIM)),
    }
    v.extend_from_slice(scalars);
    v
}

// ───────────── Набор для обучения ─────────────

/// Размеченные прослушивания по времени: векторы треков отдельно (трек слушают много раз).
#[derive(Debug, Default)]
pub struct Dataset {
    /// Строки `SOUND_DIM + SEMANTIC_DIM`.
    pub vectors: Vec<f32>,
    pub vector_rows: usize,
    pub row_vector: Vec<i32>,
    pub scalars: Vec<f32>,
    pub labels: Vec<f32>,
    pub baseline: Vec<f32>,
    pub rows: usize,
    pub holdout_from: usize,
}

impl Dataset {
    fn push_vector(&mut self, sound: &[f32], semantic: Option<&[f32]>) -> i32 {
        self.vectors.extend_from_slice(sound);
        match semantic {
            Some(s) => self.vectors.extend_from_slice(s),
            None => self.vectors.extend(std::iter::repeat_n(0.0, SEMANTIC_DIM)),
        }
        self.vector_rows += 1;
        (self.vector_rows - 1) as i32
    }

    /// Хватит ли строк для обучения и честной проверки.
    pub fn enough(&self) -> bool {
        self.rows >= MIN_ROWS && self.rows - self.holdout_from >= MIN_HOLDOUT
    }

    /// Тело `POST /v1/taste/train`.
    pub fn request(&self) -> Value {
        fn f32s(v: &[f32]) -> String {
            STANDARD.encode(v.iter().flat_map(|x| x.to_le_bytes()).collect::<Vec<u8>>())
        }
        // Векторы — в float16, как их хранит `halfvec`: набор вдвое меньше
        let f16s = |v: &[f32]| {
            STANDARD.encode(
                v.iter()
                    .flat_map(|x| f16_bits(*x).to_le_bytes())
                    .collect::<Vec<u8>>(),
            )
        };
        json!({
            "vectors": f16s(&self.vectors),
            "vectorRows": self.vector_rows,
            "soundDim": SOUND_DIM,
            "semanticDim": SEMANTIC_DIM,
            "rowVector": STANDARD.encode(self.row_vector.iter().flat_map(|x| x.to_le_bytes()).collect::<Vec<u8>>()),
            "scalars": f32s(&self.scalars),
            "scalarDim": SCALARS.len(),
            "rows": self.rows,
            "labels": f32s(&self.labels),
            "baseline": f32s(&self.baseline),
            "holdoutFrom": self.holdout_from,
            "featureNames": SCALARS.as_slice(),
        })
    }
}

/// float32 → биты float16 с округлением к ближайшему чётному (обратное `vectors::f16_to_f32`).
pub fn f16_bits(x: f32) -> u16 {
    let b = x.to_bits();
    let sign = ((b >> 16) & 0x8000) as u16;
    let exp = ((b >> 23) & 0xff) as i32;
    let man = b & 0x7f_ffff;
    if exp == 0xff {
        return sign | 0x7c00 | if man != 0 { 0x200 } else { 0 };
    }
    let e = exp - 127 + 15;
    if e >= 0x1f {
        return sign | 0x7c00;
    }
    if e <= 0 {
        // Субнормальные числа half
        if e < -10 {
            return sign;
        }
        let m = man | 0x80_0000;
        let shift = (14 - e) as u32;
        let half = 1u32 << (shift - 1);
        let mut r = m >> shift;
        let rem = m & ((1 << shift) - 1);
        if rem > half || (rem == half && r & 1 == 1) {
            r += 1;
        }
        return sign | r as u16;
    }
    let mut r = ((e as u32) << 10) | (man >> 13);
    let rem = man & 0x1fff;
    if rem > 0x1000 || (rem == 0x1000 && r & 1 == 1) {
        r += 1;
    }
    sign | r as u16
}

/// С какой строки начинается проверка: строки идут по времени, на проверку — последние
/// [`HOLDOUT_WEEKS`] недель истории, но не меньше [`MIN_HOLDOUT`] строк и не больше [`MAX_HOLDOUT_SHARE`].
pub fn holdout_start(times: &[DateTime<Utc>]) -> usize {
    let rows = times.len();
    let Some(last) = times.last() else {
        return 0;
    };
    let cut = *last - Duration::weeks(HOLDOUT_WEEKS);
    let by_time = rows - times.partition_point(|t| *t < cut);
    let floor = MIN_HOLDOUT.min(rows);
    let ceil = ((rows as f64 * MAX_HOLDOUT_SHARE).round() as usize).max(floor);
    rows - by_time.clamp(floor, ceil)
}

/// Сколько строк уходит на проверку.
pub fn holdout_size(rows: usize) -> usize {
    ((rows as f64 * HOLDOUT_SHARE).round() as usize)
        .max(MIN_HOLDOUT)
        .min(rows)
}

#[derive(Default)]
struct History {
    /// Прослушивания трека: когда и дослушан ли.
    plays: HashMap<Uuid, Vec<(DateTime<Utc>, bool)>>,
    artists: HashSet<Uuid>,
    likes: Vec<(DateTime<Utc>, Option<Uuid>)>,
    next_like: usize,
    last_at: Option<DateTime<Utc>>,
    last_session: Option<Uuid>,
    pos: u32,
    streak: u32,
}

struct Analysed {
    sound: Vec<f32>,
    semantic: Option<Vec<f32>>,
    feat: TrackFeat,
}

/// Подряд идущие прослушивания с паузами меньше получаса — одна сессия (если плеер её не прислал).
const SESSION_GAP: Duration = Duration::minutes(30);
/// Лайк относится к прослушиванию, если поставлен за минуту до начала и до 10 минут после конца.
const LIKE_BEFORE: Duration = Duration::minutes(1);
const LIKE_AFTER: Duration = Duration::minutes(10);

/// Набор из прослушиваний всех слушателей за год (последние [`MAX_ROWS`]), по времени. Признаки знакомства
/// и хода сессии — на момент прослушивания; грани вкуса — нынешние.
pub async fn dataset(db: &PgPool) -> Result<Dataset, sqlx::Error> {
    let settings = Settings::load(db).await?;
    let mut events = sqlx::query!(
        r#"SELECT p.user_id, p.track_id, t.recording_id AS "recording_id?", p.started_at, p.played_ms,
                  p.duration_ms, p.end_reason, p.context_type, p.wave_session_id, p.listening_session, p.output,
                  d.platform AS "platform?",
                  (SELECT ta.artist_id FROM track_artists ta WHERE ta.track_id = p.track_id
                   ORDER BY ta.position, ta.artist_id LIMIT 1) AS main_artist,
                  t.rank, al.release_date AS "release_date?"
           FROM play_events p
           JOIN users u ON u.id = p.user_id AND NOT u.disabled
           JOIN tracks t ON t.id = p.track_id
           LEFT JOIN albums al ON al.id = t.album_id
           LEFT JOIN devices d ON d.id = p.device_id
           WHERE p.started_at > now() - interval '365 days'
           ORDER BY p.started_at DESC, p.id DESC
           LIMIT $1"#,
        MAX_ROWS,
    )
    .fetch_all(db)
    .await?;
    events.reverse();

    let recordings: Vec<Uuid> = events
        .iter()
        .filter_map(|e| e.recording_id)
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();
    let analysed: HashMap<Uuid, Analysed> = sqlx::query!(
        r#"SELECT a.recording_id, a.sound::real[] AS "sound!", a.semantic::real[] AS semantic, a.bpm, a.bpm_confidence,
                  a.key_camelot, a.key_confidence, a.energy, a.valence, a.danceability, a.voice, a.approachability,
                  (a.moods ->> 'relaxed')::real AS relaxed, (a.moods ->> 'party')::real AS party
           FROM track_analysis a WHERE a.recording_id = ANY($1) AND a.sound IS NOT NULL"#,
        &recordings,
    )
    .fetch_all(db)
    .await?
    .into_iter()
    .map(|r| {
        (
            r.recording_id,
            Analysed {
                sound: r.sound,
                semantic: r.semantic,
                feat: TrackFeat {
                    bpm: r.bpm,
                    bpm_confidence: r.bpm_confidence,
                    key_camelot: r.key_camelot,
                    key_confidence: r.key_confidence,
                    energy: r.energy,
                    valence: r.valence,
                    danceability: r.danceability,
                    voice: r.voice,
                    approachability: r.approachability,
                    relaxed: r.relaxed,
                    party: r.party,
                    ..Default::default()
                },
            },
        )
    })
    .collect();

    // Лайки: в медиатеке (когда поставлен) и события «лайк» — метка и знакомство с артистом
    let liked_at: HashMap<(Uuid, Uuid), DateTime<Utc>> =
        sqlx::query!("SELECT user_id, track_id, created_at FROM liked_tracks")
            .fetch_all(db)
            .await?
            .into_iter()
            .map(|r| ((r.user_id, r.track_id), r.created_at))
            .collect();
    let mut like_events: HashMap<(Uuid, Uuid), Vec<DateTime<Utc>>> = HashMap::new();
    let mut history: HashMap<Uuid, History> = HashMap::new();
    for r in sqlx::query!(
        r#"SELECT f.user_id AS "user_id!", f.track_id AS "track_id!", f.at,
                  (SELECT ta.artist_id FROM track_artists ta WHERE ta.track_id = f.track_id
                   ORDER BY ta.position, ta.artist_id LIMIT 1) AS artist
           FROM feedback_events f WHERE f.kind IN ('like', 'more_like_this') AND f.track_id IS NOT NULL
           UNION ALL
           SELECT l.user_id, l.track_id, l.created_at,
                  (SELECT ta.artist_id FROM track_artists ta WHERE ta.track_id = l.track_id
                   ORDER BY ta.position, ta.artist_id LIMIT 1)
           FROM liked_tracks l"#
    )
    .fetch_all(db)
    .await?
    {
        let Some(at) = r.at else { continue };
        like_events.entry((r.user_id, r.track_id)).or_default().push(at);
        history.entry(r.user_id).or_default().likes.push((at, r.artist));
    }
    for h in history.values_mut() {
        h.likes.sort_by_key(|(at, _)| *at);
    }
    let impression_likes: HashSet<(Uuid, Uuid)> = sqlx::query!(
        r#"SELECT wave_session_id AS "session!", track_id FROM recommendation_impressions
           WHERE outcome IN ('like', 'more_like_this') AND wave_session_id IS NOT NULL"#
    )
    .fetch_all(db)
    .await?
    .into_iter()
    .map(|r| (r.session, r.track_id))
    .collect();
    let mut facets: HashMap<Uuid, Vec<Facet>> = HashMap::new();
    for user in events.iter().map(|e| e.user_id).collect::<HashSet<_>>() {
        facets.insert(user, super::taste::facets(db, user).await?);
    }

    let mut data = Dataset::default();
    // Время каждой строки — для отложенной проверки по неделям, а не по числу строк
    let mut row_at: Vec<DateTime<Utc>> = Vec::new();
    let mut vector_of: HashMap<Uuid, i32> = HashMap::new();
    for e in &events {
        let h = history.entry(e.user_id).or_default();
        let continued = match (e.listening_session, h.last_session) {
            (Some(s), Some(last)) => s == last,
            _ => h.last_at.is_some_and(|t| e.started_at - t < SESSION_GAP),
        };
        if !continued {
            h.pos = 0;
            h.streak = 0;
        }
        while let Some((at, artist)) = h.likes.get(h.next_like).copied()
            && at < e.started_at
        {
            if let Some(a) = artist {
                h.artists.insert(a);
            }
            h.next_like += 1;
        }
        let window_end =
            e.started_at + Duration::milliseconds(i64::from(e.duration_ms.max(0))) + LIKE_AFTER;
        let liked_now = like_events.get(&(e.user_id, e.track_id)).is_some_and(|ts| {
            ts.iter()
                .any(|t| *t >= e.started_at - LIKE_BEFORE && *t <= window_end)
        }) || e
            .wave_session_id
            .is_some_and(|s| impression_likes.contains(&(s, e.track_id)));
        let labels = label(&e.end_reason, e.played_ms, e.duration_ms, liked_now);
        let plays = h.plays.get(&e.track_id);
        let completed = plays.map_or(0, |p| p.iter().filter(|(_, c)| *c).count() as i64);
        let plays_30d = plays.map_or(0, |p| {
            p.iter()
                .filter(|(t, _)| *t > e.started_at - Duration::days(30))
                .count() as i64
        });
        if let (Some(y), Some(a)) = (labels, e.recording_id.and_then(|r| analysed.get(&r))) {
            let user_facets: Vec<(&[f32], f32)> = facets
                .get(&e.user_id)
                .map(|fs| {
                    fs.iter()
                        .map(|f| (f.centroid.as_slice(), f.weight))
                        .collect()
                })
                .unwrap_or_default();
            let (taste_sim, facet_weight) = nearest_facet(&a.sound, &user_facets);
            let fam = Familiarity {
                taste_sim,
                facet_weight,
                liked: liked_at
                    .get(&(e.user_id, e.track_id))
                    .is_some_and(|t| *t < e.started_at - LIKE_BEFORE),
                completed,
                artist_familiar: e.main_artist.is_some_and(|x| h.artists.contains(&x)),
                plays_30d,
            };
            let feat = TrackFeat {
                rank: e.rank,
                release_date: e.release_date,
                ..a.feat.clone()
            };
            let moment = Moment {
                at: e.started_at,
                utc_offset_min: settings.utc_offset_min,
                platform: e.platform.clone(),
                output: e.output.clone(),
                session_pos: h.pos,
                skip_streak: h.streak,
                in_wave: matches!(e.context_type.as_deref(), Some("wave" | "radio")),
            };
            let recording = e.recording_id.unwrap_or(e.track_id);
            let idx = match vector_of.get(&recording) {
                Some(i) => *i,
                None => {
                    let i = data.push_vector(&a.sound, a.semantic.as_deref());
                    vector_of.insert(recording, i);
                    i
                }
            };
            data.row_vector.push(idx);
            data.scalars
                .extend_from_slice(&scalars(&feat, a.semantic.is_some(), &moment, &fam));
            data.labels.extend_from_slice(&y);
            data.baseline.push(rules_prior(&feat, &fam, e.started_at));
            data.rows += 1;
            row_at.push(e.started_at);
        }
        // История — после строки: признаки видят только прошлое
        let complete = labels.is_some_and(|y| y[1] > 0.5);
        h.plays
            .entry(e.track_id)
            .or_default()
            .push((e.started_at, complete));
        if let Some(a) = e.main_artist {
            h.artists.insert(a);
        }
        if let Some(y) = labels {
            h.streak = if y[0] > 0.5 { h.streak + 1 } else { 0 };
        }
        h.pos += 1;
        h.last_at = Some(e.started_at);
        h.last_session = e.listening_session;
    }
    data.holdout_from = holdout_start(&row_at);
    Ok(data)
}

// ───────────── Решение: включать ли ─────────────

#[derive(Debug, Clone, PartialEq)]
pub struct Verdict {
    pub beats: bool,
    pub note: String,
}

fn metric(m: &Value, key: &str) -> Option<f64> {
    m.get(key).and_then(Value::as_f64)
}

/// Сеть включается, только если на отложенных данных точнее правил: средняя AUC выше хотя бы на
/// [`AUC_MARGIN`] и logloss не хуже.
pub fn judge(metrics: &Value) -> Verdict {
    let (Some(auc), Some(base_auc), Some(loss), Some(base_loss)) = (
        metric(metrics, "auc"),
        metric(metrics, "baselineAuc"),
        metric(metrics, "logloss"),
        metric(metrics, "baselineLogloss"),
    ) else {
        return Verdict {
            beats: false,
            note: "На отложенных данных не с чем сравнить: у голов один исход".into(),
        };
    };
    let beats = auc >= base_auc + AUC_MARGIN && loss <= base_loss;
    let note = if beats {
        format!(
            "Точнее правил: AUC {auc:.3} против {base_auc:.3}, logloss {loss:.3} против {base_loss:.3}"
        )
    } else {
        format!(
            "Правила точнее: AUC {auc:.3} против {base_auc:.3}, logloss {loss:.3} против {base_loss:.3}"
        )
    };
    Verdict { beats, note }
}

// ───────────── Загрузка и применение ─────────────

/// Загруженная версия сети.
pub struct Loaded {
    pub id: Uuid,
    pub version: String,
    session: Mutex<Session>,
}

impl Loaded {
    /// ONNX → сессия onnxruntime; проверка — пробный прогон строки нулей.
    pub fn build(id: Uuid, version: &str, onnx: &[u8]) -> Result<Self, String> {
        let env = environment()?;
        let session = Session::builder(env)
            .map_err(|e| e.to_string())?
            .with_intra_threads(1)
            .map_err(|e| e.to_string())?
            .commit_from_memory(onnx)
            .map_err(|e| e.to_string())?;
        let loaded = Self {
            id,
            version: version.to_string(),
            session: Mutex::new(session),
        };
        loaded.predict(vec![0.0; FEATURE_DIM], 1)?;
        Ok(loaded)
    }

    /// Вероятности пропуска, дослушивания и лайка для `n` строк ширины [`FEATURE_DIM`].
    pub fn predict(&self, features: Vec<f32>, n: usize) -> Result<Vec<[f32; 3]>, String> {
        if features.len() != n * FEATURE_DIM {
            return Err(format!("ширина признаков не {FEATURE_DIM}"));
        }
        let tensor = Tensor::from_array(([n, FEATURE_DIM], features)).map_err(|e| e.to_string())?;
        let mut session = self
            .session
            .lock()
            .map_err(|_| "сессия сети отравлена".to_string())?;
        let outputs = session
            .run(ort::inputs!["features" => tensor])
            .map_err(|e| e.to_string())?;
        let (_, probs) = outputs["probs"]
            .try_extract_tensor::<f32>()
            .map_err(|e| e.to_string())?;
        if probs.len() != n * HEADS.len() {
            return Err(format!(
                "сеть вернула {} чисел вместо {}",
                probs.len(),
                n * HEADS.len()
            ));
        }
        Ok(probs.as_chunks::<3>().0.to_vec())
    }
}

fn environment() -> Result<&'static Environment, String> {
    static ENV: OnceLock<Result<Environment, String>> = OnceLock::new();
    ENV.get_or_init(|| {
        ort::init()
            .with_name("dalywave-taste")
            .build()
            .map_err(|e| e.to_string())
    })
    .as_ref()
    .map_err(Clone::clone)
}

struct Cache {
    loaded: Option<Arc<Loaded>>,
    /// Версия, которую не удалось загрузить: не пробовать на каждом подборе.
    failed: Option<Uuid>,
}

static CACHE: Mutex<Cache> = Mutex::new(Cache {
    loaded: None,
    failed: None,
});

/// Активная сеть, если поток сейчас собирает она; правила или сбой загрузки — `None`.
pub async fn active(db: &PgPool) -> Result<Option<Arc<Loaded>>, sqlx::Error> {
    let Some(row) =
        sqlx::query!("SELECT id, version FROM model_versions WHERE kind = 'taste' AND active")
            .fetch_optional(db)
            .await?
    else {
        return Ok(None);
    };
    if !row.version.starts_with(PREFIX) {
        return Ok(None);
    }
    if let Ok(cache) = CACHE.lock() {
        if let Some(l) = cache.loaded.as_ref().filter(|l| l.id == row.id) {
            return Ok(Some(l.clone()));
        }
        if cache.failed == Some(row.id) {
            return Ok(None);
        }
    }
    let artifact = sqlx::query_scalar!("SELECT artifact FROM model_versions WHERE id = $1", row.id)
        .fetch_optional(db)
        .await?
        .flatten();
    let (id, version) = (row.id, row.version);
    let built = match artifact {
        Some(bytes) => tokio::task::spawn_blocking(move || Loaded::build(id, &version, &bytes))
            .await
            .unwrap_or_else(|e| Err(e.to_string())),
        None => Err("у версии нет ONNX".into()),
    };
    let mut cache = match CACHE.lock() {
        Ok(c) => c,
        Err(p) => p.into_inner(),
    };
    match built {
        Ok(l) => {
            let l = Arc::new(l);
            tracing::info!(version = %l.version, "taste model loaded");
            cache.loaded = Some(l.clone());
            cache.failed = None;
            Ok(Some(l))
        }
        Err(e) => {
            tracing::warn!(model = %id, error = %e, "taste model failed to load, ranking stays on rules");
            cache.failed = Some(id);
            Ok(None)
        }
    }
}

/// Версия, которая пишется в показы: сеть, если она оценила выдачу, иначе правила.
pub fn version_of(entries: &[&Cand]) -> String {
    if entries.iter().any(|c| c.model.is_some())
        && let Ok(cache) = CACHE.lock()
        && let Some(l) = &cache.loaded
    {
        return l.version.clone();
    }
    MODEL_VERSION.to_string()
}

/// Подбор волны прямо сейчас: время, устройство, ход сессии, грани вкуса.
pub struct Live<'a> {
    pub at: DateTime<Utc>,
    pub utc_offset_min: i16,
    pub platform: Option<&'a str>,
    pub step: u32,
    pub skip_streak: u32,
    pub facets: &'a [Facet],
}

/// Оценка сети каждому кандидату с анализом (`Cand::model`) — относительно среднего по кандидатам:
/// сеть двигает порядок, а не общий уровень. Сети нет или она дала сбой — ничего не меняется.
pub async fn annotate(db: &PgPool, entries: &mut [Entry], live: &Live<'_>) -> Option<String> {
    let loaded = match active(db).await {
        Ok(Some(l)) => l,
        Ok(None) => return None,
        Err(e) => {
            tracing::warn!(error = %e, "taste model lookup failed");
            return None;
        }
    };
    let recordings: Vec<Uuid> = entries
        .iter()
        .filter_map(|e| e.cand.recording_id)
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();
    if recordings.is_empty() {
        return None;
    }
    let vectors: HashMap<Uuid, (Vec<f32>, Option<Vec<f32>>)> = match sqlx::query!(
        r#"SELECT recording_id, sound::real[] AS "sound!", semantic::real[] AS semantic
           FROM track_analysis WHERE recording_id = ANY($1) AND sound IS NOT NULL"#,
        &recordings,
    )
    .fetch_all(db)
    .await
    {
        Ok(rows) => rows
            .into_iter()
            .map(|r| (r.recording_id, (r.sound, r.semantic)))
            .collect(),
        Err(e) => {
            tracing::warn!(error = %e, "taste model: vectors unavailable");
            return None;
        }
    };
    let facets: Vec<(&[f32], f32)> = live
        .facets
        .iter()
        .map(|f| (f.centroid.as_slice(), f.weight))
        .collect();
    let moment = Moment {
        at: live.at,
        utc_offset_min: live.utc_offset_min,
        platform: live.platform.map(str::to_string),
        output: None,
        session_pos: live.step,
        skip_streak: live.skip_streak,
        in_wave: true,
    };
    let mut idx = Vec::new();
    let mut features = Vec::new();
    for (i, e) in entries.iter().enumerate() {
        let c = &e.cand;
        let Some((sound, semantic)) = c.recording_id.and_then(|r| vectors.get(&r)) else {
            continue;
        };
        let (taste_sim, facet_weight) = nearest_facet(sound, &facets);
        let fam = Familiarity {
            taste_sim,
            facet_weight,
            liked: c.liked,
            completed: c.completed,
            artist_familiar: c.artist_familiar,
            plays_30d: c.plays_30d,
        };
        let s = scalars(&TrackFeat::of(c), semantic.is_some(), &moment, &fam);
        features.extend(row(sound, semantic.as_deref(), &s));
        idx.push(i);
    }
    if idx.is_empty() {
        return None;
    }
    let n = idx.len();
    let model = loaded.clone();
    let probs = match tokio::task::spawn_blocking(move || model.predict(features, n)).await {
        Ok(Ok(p)) => p,
        Ok(Err(e)) => {
            tracing::warn!(error = %e, "taste model failed, ranking stays on rules");
            return None;
        }
        Err(e) => {
            tracing::warn!(error = %e, "taste model task failed");
            return None;
        }
    };
    let raw: Vec<f32> = probs
        .iter()
        .map(|[skip, complete, like]| 0.5 * complete + like - 0.8 * skip)
        .collect();
    let mean = raw.iter().sum::<f32>() / n as f32;
    for (i, r) in idx.into_iter().zip(raw) {
        entries[i].cand.model = Some(MODEL_WEIGHT * (r - mean));
    }
    Some(loaded.version.clone())
}

// ───────────── Обучение: задача и расписание ─────────────

/// Ставит обучение (одно на сервер).
pub async fn request(db: &PgPool) -> Result<bool, sqlx::Error> {
    let mut conn = db.acquire().await?;
    Ok(jobs::enqueue(
        &mut conn,
        NewJob::new(kind::TASTE_MODEL, lanes::MAINTENANCE, json!({})).dedupe("taste_model"),
    )
    .await?
    .created)
}

/// Раз в сутки ночью (4–7 по времени слушателя; не было ночью — через двое суток), если прослушиваний
/// хватает хотя бы на попытку.
pub async fn schedule(db: &PgPool, settings: &Settings) -> Result<bool, sqlx::Error> {
    let hour = settings.local_hour(Utc::now());
    let hours: i32 = if (4..7).contains(&hour) { 20 } else { 48 };
    let due = sqlx::query_scalar!(
        r#"SELECT NOT EXISTS (SELECT 1 FROM jobs WHERE kind = 'taste_model'
                                AND (status IN ('queued', 'running')
                                     OR (status IN ('done', 'failed') AND finished_at > now() - make_interval(hours => $1))))
                  AND (SELECT count(*) FROM play_events WHERE started_at > now() - interval '365 days') >= $2
                  AS "due!""#,
        hours,
        MIN_ROWS as i64,
    )
    .fetch_one(db)
    .await?;
    if !due {
        return Ok(false);
    }
    request(db).await
}

fn too_little(rows: usize) -> Value {
    json!({
        "outcome": "too_little_data",
        "rows": rows,
        "minRows": MIN_ROWS,
        "message": format!("Мало данных: {rows} размеченных прослушиваний из {MIN_ROWS} — волну собирают правила"),
    })
}

/// Задача `taste_model`: набор → обучение в ml → проверка, что ONNX загружается → версия в
/// `model_versions` (ONNX — в хранилище и копией в базе) → включение, если точнее правил; иначе — правила.
pub async fn run(ctx: Arc<Context>, _job: JobRow) -> JobResult {
    let data = dataset(&ctx.db).await?;
    if !data.enough() {
        return Ok(too_little(data.rows).into());
    }
    let trained = match ctx.ml.train_taste(&data.request()).await {
        Ok(t) => t,
        Err(MlError::Api { code, .. }) if code == "too_little_data" => {
            return Ok(too_little(data.rows).into());
        }
        Err(e) if e.is_temporary() => return Err(JobError::retry(e)),
        Err(e) => return Err(JobError::permanent(e)),
    };
    if trained.input_dim != FEATURE_DIM {
        return Err(JobError::permanent(format!(
            "ширина входа сети {} вместо {FEATURE_DIM}",
            trained.input_dim
        )));
    }
    let onnx = STANDARD
        .decode(&trained.onnx)
        .map_err(|e| JobError::permanent(format!("ONNX не в base64: {e}")))?;
    let probe = onnx.clone();
    tokio::task::spawn_blocking(move || Loaded::build(Uuid::nil(), "probe", &probe).map(|_| ()))
        .await
        .map_err(|e| JobError::retry(e.to_string()))?
        .map_err(|e| JobError::permanent(format!("сервер не загружает ONNX: {e}")))?;

    let verdict = judge(&trained.metrics);
    let stamp = format!("{PREFIX}{}", Utc::now().format("%Y%m%d-%H%M%S"));
    // Два обучения в одну секунду (ручное «Обучить» сразу за ночным) — суффикс
    let same = sqlx::query_scalar!(
        r#"SELECT count(*) AS "n!" FROM model_versions WHERE kind = 'taste' AND version LIKE $1 || '%'"#,
        stamp,
    )
    .fetch_one(&ctx.db)
    .await?;
    let version = if same == 0 {
        stamp
    } else {
        format!("{stamp}-{}", same + 1)
    };
    let key = format!("models/taste/{version}.onnx");
    let artifact_key = match ctx
        .storage
        .put(&key, onnx.clone(), "application/octet-stream", None)
        .await
    {
        Ok(()) => Some(key),
        Err(e) => {
            tracing::warn!(error = %e, "taste model: storage upload failed, kept in the database only");
            None
        }
    };
    let mut metrics = trained.metrics.clone();
    if let Some(m) = metrics.as_object_mut() {
        m.insert("rows".into(), json!(data.rows));
        m.insert("params".into(), json!(trained.params));
        m.insert("epochs".into(), json!(trained.epochs));
        m.insert("reduced".into(), trained.reduced.clone());
        m.insert("features".into(), json!(SCALARS.as_slice()));
        m.insert("beatsRules".into(), json!(verdict.beats));
        m.insert("note".into(), json!(verdict.note));
    }

    let mut tx = ctx.db.begin().await?;
    let id = sqlx::query_scalar!(
        "INSERT INTO model_versions (kind, version, metrics, artifact_key, artifact, active)
         VALUES ('taste', $1, $2, $3, $4, false) RETURNING id",
        version,
        metrics,
        artifact_key,
        onnx,
    )
    .fetch_one(&mut *tx)
    .await?;
    sqlx::query!("UPDATE model_versions SET active = false WHERE kind = 'taste' AND active")
        .execute(&mut *tx)
        .await?;
    if verdict.beats {
        sqlx::query!("UPDATE model_versions SET active = true WHERE id = $1", id)
            .execute(&mut *tx)
            .await?;
    } else {
        // Свежая проверка говорит, что правила точнее: прежняя сеть тоже выключается
        sqlx::query!(
            "INSERT INTO model_versions (kind, version, active) VALUES ('taste', $1, true)
             ON CONFLICT (kind, version) DO UPDATE SET active = true",
            MODEL_VERSION,
        )
        .execute(&mut *tx)
        .await?;
    }
    sqlx::query!(
        "UPDATE model_versions SET artifact = NULL
         WHERE kind = 'taste' AND NOT active AND artifact IS NOT NULL
           AND id NOT IN (SELECT id FROM model_versions WHERE kind = 'taste' AND artifact IS NOT NULL
                          ORDER BY created_at DESC LIMIT $1)",
        KEEP_ARTIFACTS,
    )
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    tracing::info!(%version, active = verdict.beats, note = %verdict.note, "taste model trained");
    Ok(json!({
        "outcome": "trained",
        "version": version,
        "active": verdict.beats,
        "rows": data.rows,
        "message": verdict.note,
    })
    .into())
}

// ───────────── Админка ─────────────

fn view(
    id: Uuid,
    version: String,
    active: bool,
    created_at: DateTime<Utc>,
    m: &Value,
) -> ModelVersionView {
    let learned = version.starts_with(PREFIX);
    let heads = HEADS
        .iter()
        .filter_map(|h| {
            let x = m.get("heads")?.get(*h)?;
            Some(HeadMetrics {
                head: (*h).to_string(),
                positives: x.get("positives").and_then(Value::as_i64).unwrap_or(0),
                auc: metric(x, "auc"),
                logloss: metric(x, "logloss"),
                baseline_auc: metric(x, "baselineAuc"),
                baseline_logloss: metric(x, "baselineLogloss"),
            })
        })
        .collect();
    ModelVersionView {
        id,
        learned,
        active,
        created_at,
        auc: metric(m, "auc"),
        logloss: metric(m, "logloss"),
        baseline_auc: metric(m, "baselineAuc"),
        baseline_logloss: metric(m, "baselineLogloss"),
        heads,
        train_rows: m.get("train").and_then(Value::as_i64),
        holdout_rows: m.get("holdout").and_then(Value::as_i64),
        params: m.get("params").and_then(Value::as_i64),
        beats_rules: m.get("beatsRules").and_then(Value::as_bool),
        note: m.get("note").and_then(Value::as_str).map(str::to_string),
        version,
    }
}

/// Версии модели вкуса, новые первыми (последние 20).
pub async fn versions(db: &PgPool) -> Result<Vec<ModelVersionView>, sqlx::Error> {
    Ok(sqlx::query!(
        "SELECT id, version, active, created_at, metrics FROM model_versions
         WHERE kind = 'taste' ORDER BY created_at DESC, id DESC LIMIT 20"
    )
    .fetch_all(db)
    .await?
    .into_iter()
    .map(|r| view(r.id, r.version, r.active, r.created_at, &r.metrics))
    .collect())
}

/// Последнее обучение и сколько данных есть сейчас.
pub async fn training(db: &PgPool) -> Result<ModelTraining, sqlx::Error> {
    let last = sqlx::query!(
        "SELECT status, result, last_error, COALESCE(finished_at, updated_at) AS at FROM jobs
         WHERE kind = 'taste_model' ORDER BY created_at DESC, id DESC LIMIT 1"
    )
    .fetch_optional(db)
    .await?;
    let rows = sqlx::query_scalar!(
        r#"SELECT count(*) AS "n!" FROM play_events p JOIN tracks t ON t.id = p.track_id
           JOIN track_analysis a ON a.recording_id = t.recording_id AND a.sound IS NOT NULL
           WHERE p.started_at > now() - interval '365 days'
             AND p.end_reason NOT IN ('error', 'interrupted', 'transferred')"#
    )
    .fetch_one(db)
    .await?;
    let Some(last) = last else {
        return Ok(ModelTraining {
            rows,
            min_rows: MIN_ROWS as i64,
            ..Default::default()
        });
    };
    let result = last.result.unwrap_or(Value::Null);
    let (outcome, message) = match last.status.as_str() {
        "failed" => (Some("failed".to_string()), last.last_error),
        "done" => (
            result
                .get("outcome")
                .and_then(Value::as_str)
                .map(str::to_string),
            result
                .get("message")
                .and_then(Value::as_str)
                .map(str::to_string),
        ),
        _ => (None, None),
    };
    Ok(ModelTraining {
        running: matches!(last.status.as_str(), "queued" | "running"),
        last_run_at: last.at,
        outcome,
        message,
        rows,
        min_rows: MIN_ROWS as i64,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn moment(hour: u32) -> Moment {
        Moment {
            at: DateTime::parse_from_rfc3339(&format!("2026-11-14T{hour:02}:00:00Z"))
                .unwrap()
                .with_timezone(&Utc),
            utc_offset_min: 180,
            platform: Some("ios".into()),
            output: Some("bluetooth".into()),
            session_pos: 3,
            skip_streak: 2,
            in_wave: true,
        }
    }

    fn idx(name: &str) -> usize {
        SCALARS.iter().position(|s| *s == name).unwrap()
    }

    #[test]
    fn scalars_encode_track_context_and_taste() {
        let t = TrackFeat {
            bpm: Some(120.0),
            bpm_confidence: Some(0.9),
            key_camelot: Some("8A".into()),
            key_confidence: Some(0.8),
            energy: Some(0.7),
            rank: Some(500_000),
            ..Default::default()
        };
        let f = Familiarity {
            taste_sim: Some(0.8),
            facet_weight: 0.4,
            liked: true,
            completed: 3,
            artist_familiar: true,
            plays_30d: 0,
        };
        // 18:00 UTC — 21:00 в Москве, суббота
        let s = scalars(&t, true, &moment(18), &f);
        assert_eq!(s.len(), SCALARS.len());
        assert!((s[idx("bpm")] - 0.6).abs() < 1e-6);
        assert_eq!(s[idx("key_known")], 1.0);
        assert_eq!(s[idx("key_minor")], 1.0);
        assert_eq!(s[idx("weekend")], 1.0);
        assert_eq!(s[idx("device_mobile")], 1.0);
        assert_eq!(s[idx("out_wireless")], 1.0);
        assert_eq!(s[idx("skip_streak")], 0.4);
        let hour = 21.0 / 24.0 * TAU;
        assert!((s[idx("hour_sin")] - hour.sin()).abs() < 1e-5);
        assert_eq!(s[idx("taste_sim")], 0.8);
        assert_eq!(s[idx("has_facets")], 1.0);
        assert!(s[idx("completed")] > 0.0 && s[idx("plays_30d")] == 0.0);
        assert!(s.iter().all(|x| x.is_finite()));

        // Неизвестное — середина шкалы и флаги «неизвестно»
        let empty = scalars(
            &TrackFeat::default(),
            false,
            &Moment::default(),
            &Familiarity::default(),
        );
        assert_eq!(empty[idx("bpm_known")], 0.0);
        assert_eq!(empty[idx("key_known")], 0.0);
        assert_eq!(empty[idx("energy")], 0.6);
        assert_eq!(empty[idx("has_facets")], 0.0);
        assert_eq!(
            empty[idx("device_mobile")] + empty[idx("device_desktop")],
            0.0,
            "сайт"
        );
    }

    #[test]
    fn labels_say_nothing_about_errors_and_stops() {
        assert_eq!(
            label("completed", 180_000, 180_000, true),
            Some([0.0, 1.0, 1.0])
        );
        assert_eq!(
            label("skipped", 5_000, 180_000, false),
            Some([1.0, 0.0, 0.0])
        );
        // Поздний пропуск — не пропуск и не дослушивание
        assert_eq!(
            label("skipped", 120_000, 180_000, false),
            Some([0.0, 0.0, 0.0])
        );
        // Дослушал почти до конца и переключил — дослушал
        assert_eq!(
            label("skipped", 170_000, 180_000, false),
            Some([0.0, 1.0, 0.0])
        );
        assert_eq!(label("error", 1_000, 180_000, false), None);
        assert_eq!(label("transferred", 60_000, 180_000, false), None);
        assert_eq!(label("stopped", 60_000, 180_000, false), None);
        assert_eq!(label("completed", 1, 0, false), None);
    }

    #[test]
    fn nearest_facet_and_rules_prior() {
        let a = vec![1.0, 0.0];
        let b = vec![0.0, 1.0];
        let facets: Vec<(&[f32], f32)> = vec![(&a, 0.7), (&b, 0.3)];
        let (sim, w) = nearest_facet(&[0.1, 1.0], &facets);
        assert!(sim.unwrap() > 0.9);
        assert_eq!(w, 0.3);
        assert_eq!(nearest_facet(&[1.0, 0.0], &[]), (None, 0.0));
        let f = Familiarity {
            taste_sim: Some(0.5),
            liked: true,
            ..Default::default()
        };
        assert!((rules_prior(&TrackFeat::default(), &f, Utc::now()) - 0.38).abs() < 1e-6);
    }

    #[test]
    fn half_precision_round_trips() {
        use crate::reco::vectors::f16_to_f32;
        for x in [
            0.0f32, 1.0, -1.0, 0.5, 0.1, -0.333, 1e-5, 6.1e-5, 65504.0, 0.0312,
        ] {
            let back = f16_to_f32(f16_bits(x));
            assert!((back - x).abs() <= x.abs() * 1e-3 + 1e-7, "{x} → {back}");
        }
        assert_eq!(f16_bits(1.0), 0x3c00);
        assert_eq!(f16_bits(-2.0), 0xc000);
        assert_eq!(f16_bits(1e9), 0x7c00);
    }

    #[test]
    fn verdict_needs_better_auc_and_no_worse_logloss() {
        let m = |auc: f64, base: f64, loss: f64, base_loss: f64| json!({ "auc": auc, "baselineAuc": base, "logloss": loss, "baselineLogloss": base_loss });
        assert!(judge(&m(0.72, 0.65, 0.50, 0.55)).beats);
        assert!(!judge(&m(0.652, 0.65, 0.50, 0.55)).beats, "в пределах шума");
        assert!(!judge(&m(0.72, 0.65, 0.60, 0.55)).beats, "logloss хуже");
        let none = judge(&json!({ "auc": null, "baselineAuc": null }));
        assert!(!none.beats);
        assert_eq!(holdout_size(300), 60);
        let day = |d: i64| DateTime::<Utc>::from_timestamp(0, 0).unwrap() + Duration::days(d);
        // Год истории по строке в день: на проверку — последние четыре недели
        let year: Vec<_> = (0..365).map(day).collect();
        assert_eq!(365 - holdout_start(&year), 60.max(29));
        // Вся история за неделю: последние недели — это всё, но больше 40 % на проверку не уходит
        let week: Vec<_> = (0..300).map(|i| day(i / 50)).collect();
        assert_eq!(300 - holdout_start(&week), 120);
        // Плотный хвост свежих строк: всё из последних четырёх недель
        let mut tail: Vec<_> = (0..300).map(day).collect();
        tail.extend((0..80).map(|_| day(300)));
        assert_eq!(380 - holdout_start(&tail), 80 + 28);
        assert_eq!(holdout_start(&[]), 0);
        assert_eq!(holdout_size(1000), 200);
    }
}
