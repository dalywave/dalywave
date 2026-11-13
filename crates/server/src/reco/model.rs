//! Своя сеть вкуса: по треку, контексту и ходу сессии предсказывает три вероятности — пропустит,
//! дослушает, лайкнет. Учится ночью здесь же, в воркере (`dalywave-taste` — та же реализация, что в
//! приложении на компьютере), на всей истории прослушиваний: свежее весит больше старого. Проверка —
//! на самых поздних прослушиваниях, которых сеть не видела; включается сама, только если точнее правил
//! (`rules-1`) и текущей версии. До этого и при любом сбое поток собирают правила.
//!
//! Признаки строки — сырые: звучание (Discogs-EffNet, 1280), «смысл» (CLAP, 512) и [`SCALARS`]. Сжатие
//! векторов, нормировка и ограничение выбросов вшиты в веса и в ONNX, поэтому сервер считает признаки
//! одной функцией ([`scalars`]) и при обучении, и при подборе.
//!
//! Веса, присланные компьютером ([`super::training`]), проверяются здесь же тем же [`evaluate`] —
//! через onnxruntime, которым потом подбирается волна.

use std::{
    collections::{HashMap, HashSet},
    f32::consts::TAU,
    sync::{Arc, Mutex, OnceLock},
};

use chrono::{DateTime, Datelike, Duration, NaiveDate, Timelike, Utc};
use dalywave_protocol::{
    jobs::kind,
    reco::{HeadMetrics, ModelTraining, ModelVersionView},
};
use dalywave_taste::{
    Params, Spec, TrainReport, TrainSet,
    metrics::{self, Calibration, Comparison, Scores},
    onnx,
};
use ort::{environment::Environment, session::Session, value::Tensor};
use serde::Serialize;
use serde_json::{Value, json};
use sqlx::PgPool;
use uuid::Uuid;

use super::{
    Cand, MODEL_VERSION, Settings,
    rank::{self, Entry},
    taste::{self, Facet},
    vectors::{SEMANTIC_DIM, SOUND_DIM, blend, camelot, cosine},
};
use crate::{
    jobs::{
        self, JobRow, NewJob, lanes,
        worker::{Context, JobError, JobResult},
    },
    storage::Storage,
};

/// Головы сети в порядке выхода `probs`.
pub const HEADS: [&str; 3] = dalywave_taste::HEADS;

/// Числовые признаки строки в порядке входа сети (после векторов). Новый признак — только в конец;
/// версия с другой шириной входа не загружается, и поток собирают правила до следующего обучения.
pub const SCALARS: [&str; 43] = [
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
    // История трека и ход сессии
    "track_skips",
    "days_since_play",
    "heard_before",
    "hit_sim",
    "miss_sim",
    "has_hits",
    "has_misses",
];

/// Ширина входа сети.
pub const FEATURE_DIM: usize = SOUND_DIM + SEMANTIC_DIM + SCALARS.len();
/// Меньше размеченных прослушиваний — сеть не учится: правила лучше переобученной сети.
pub const MIN_ROWS: usize = 300;
/// Сколько строк, самое меньшее, уходит на проверку.
pub const MIN_HOLDOUT: usize = 60;
/// Отложенная проверка — последние недели истории, но не меньше этой доли строк: на четырёх неделях
/// импортированной истории сотня строк и два десятка пропусков, разница AUC тонет в шуме.
const HOLDOUT_WEEKS: i64 = 4;
const MIN_HOLDOUT_SHARE: f64 = 0.2;
/// Больше этой доли на проверку не уходит: иначе учить не на чем.
const MAX_HOLDOUT_SHARE: f64 = 0.4;
/// Последние прослушивания, не больше стольких.
const MAX_ROWS: i64 = 60_000;
/// Вес строки падает вдвое за столько дней: вкус меняется, свежее важнее.
const HALF_LIFE_DAYS: f32 = 365.0;
/// Ниже этого вес строки не падает: старая история всё ещё говорит о вкусе.
const MIN_WEIGHT: f32 = 0.1;
/// Чтобы включиться, сеть должна обогнать правила по средней AUC хотя бы на столько…
const AUC_MARGIN: f64 = 0.005;
/// …и быть лучше в стольких бутстреп-выборках по сессиям: не случайность отложенной части.
const P_BETTER: f64 = 0.9;
/// На сколько logloss новой версии может быть хуже текущей при лучшей AUC.
const LOGLOSS_SLACK: f64 = 0.002;
const BOOTSTRAP: usize = 200;
/// Вес оценки сети в общей оценке волны (относительно среднего по кандидатам).
const MODEL_WEIGHT: f32 = 0.35;
/// Копии ONNX в базе держатся у активных и стольких последних версий.
const KEEP_ARTIFACTS: i64 = 5;
/// Версии сети называются `nn-ГГГГММДД-ччммсс`.
pub const PREFIX: &str = "nn-";

/// Как учит сервер. Компьютеры учат так же, но со своим зерном.
pub fn spec() -> Spec {
    Spec::default()
}

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
    /// Ранних пропусков трека до этого.
    pub skips: i64,
    /// Сколько дней назад трек играл последний раз; не играл — `None`.
    pub days_since: Option<f32>,
    /// Сходство с «что сейчас заходит» и «что пропускается» сессии.
    pub hit_sim: Option<f32>,
    pub miss_sim: Option<f32>,
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
        log_scaled(f.skips),
        f.days_since
            .map_or(0.0, |d| (d.max(0.0).ln_1p() / 366f32.ln()).min(1.0)),
        b(f.days_since.is_some()),
        f.hit_sim.unwrap_or(0.0),
        f.miss_sim.unwrap_or(0.0),
        b(f.hit_sim.is_some()),
        b(f.miss_sim.is_some()),
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

/// Ранний пропуск: переключил в первой половине. У импорта Spotify «остановлено» — это `endplay`
/// (включил другое) и выход из приложения; в первой половине трека это почти всегда пропуск
/// (медиана — 5 % трека), поэтому для импорта оно тоже пропуск.
pub fn early_skip(end_reason: &str, played: f32, import: bool) -> bool {
    played < 0.5
        && (matches!(end_reason, "skipped" | "previous") || (import && end_reason == "stopped"))
}

/// Метки прослушивания: пропуск, дослушивание, лайк. Сбой, обрыв, передача на другое устройство и
/// остановка посреди трека в DalyWave ничего не говорят о вкусе — без метки.
pub fn label(
    end_reason: &str,
    played_ms: i32,
    duration_ms: i32,
    liked: bool,
    import: bool,
) -> Option<[f32; 3]> {
    if duration_ms <= 0 {
        return None;
    }
    let played = played_ms.max(0) as f32 / duration_ms as f32;
    let complete = end_reason == "completed" || played >= 0.8;
    let skip = !complete && early_skip(end_reason, played, import);
    match end_reason {
        "error" | "interrupted" | "transferred" => return None,
        "stopped" if !complete && !skip => return None,
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

/// Из чего собирается набор. [`Recipe::default`] — как учит сервер; [`Recipe::legacy`] — как было до
/// разбора «почему сеть плохо учится» (для сравнения «до/после» на одной базе).
#[derive(Debug, Clone, PartialEq)]
pub struct Recipe {
    /// Только последние столько дней истории; `None` — вся.
    pub since_days: Option<i64>,
    /// Импорт: «остановлено» в первой половине трека — пропуск ([`early_skip`]).
    pub import_stops_skip: bool,
    /// Лайк — трек в медиатеке (у импортированных лайков даты нет), а не только лайк рядом с
    /// прослушиванием.
    pub library_likes: bool,
    /// Вес строки падает вдвое за столько дней; `None` — все строки равны.
    pub half_life_days: Option<f32>,
    /// Грани вкуса для признаков — собранные по истории до отложенной части, а не нынешние: нынешние
    /// знают исходы отложенных прослушиваний.
    pub cut_facets: bool,
    /// Отложенная часть — не меньше этой доли строк.
    pub min_holdout_share: f64,
}

impl Default for Recipe {
    fn default() -> Self {
        Self {
            since_days: None,
            import_stops_skip: true,
            library_likes: true,
            half_life_days: Some(HALF_LIFE_DAYS),
            cut_facets: true,
            min_holdout_share: MIN_HOLDOUT_SHARE,
        }
    }
}

impl Recipe {
    pub fn legacy() -> Self {
        Self {
            since_days: Some(365),
            import_stops_skip: false,
            library_likes: false,
            half_life_days: None,
            cut_facets: false,
            min_holdout_share: 0.0,
        }
    }
}

/// Размеченные прослушивания по времени и всё, что нужно для проверки.
#[derive(Debug, Default, Clone)]
pub struct Dataset {
    /// Все строки по времени: признаки, метки, веса, сессии.
    pub set: TrainSet,
    /// Оценка правил строки.
    pub baseline: Vec<f32>,
    pub times: Vec<DateTime<Utc>>,
    pub users: Vec<Uuid>,
    /// С этой строки — отложенная проверка.
    pub holdout_from: usize,
}

impl Dataset {
    pub fn rows(&self) -> usize {
        self.set.rows()
    }

    /// Хватит ли строк для обучения и честной проверки.
    pub fn enough(&self) -> bool {
        self.rows() >= MIN_ROWS && self.rows() - self.holdout_from >= MIN_HOLDOUT
    }

    /// Когда начинается отложенная часть (время первой её строки).
    pub fn holdout_cut(&self) -> Option<DateTime<Utc>> {
        self.times.get(self.holdout_from).copied()
    }

    /// Обучающая часть — то, что получает компьютер и на чём учится сервер.
    pub fn train_part(&self) -> TrainSet {
        self.set.head(self.holdout_from)
    }
}

/// С какой строки начинается проверка: строки идут по времени, на проверку — последние
/// [`HOLDOUT_WEEKS`] недель истории, но не меньше `min_share` строк и [`MIN_HOLDOUT`] и не больше
/// [`MAX_HOLDOUT_SHARE`].
pub fn holdout_start(times: &[DateTime<Utc>], min_share: f64) -> usize {
    let rows = times.len();
    let Some(last) = times.last() else {
        return 0;
    };
    let cut = *last - Duration::weeks(HOLDOUT_WEEKS);
    let by_time = rows - times.partition_point(|t| *t < cut);
    let floor = MIN_HOLDOUT
        .max((rows as f64 * min_share).round() as usize)
        .min(rows);
    let ceil = ((rows as f64 * MAX_HOLDOUT_SHARE).round() as usize).max(floor);
    let mut start = rows - by_time.clamp(floor, ceil);
    // Строки одного момента не делятся между частями
    while start > 0 && start < rows && times[start - 1] == times[start] {
        start -= 1;
    }
    start
}

#[derive(Default)]
struct History {
    /// Прослушивания трека: когда, дослушан ли, ранний ли пропуск.
    plays: HashMap<Uuid, Vec<(DateTime<Utc>, bool, bool)>>,
    artists: HashSet<Uuid>,
    likes: Vec<(DateTime<Utc>, Option<Uuid>)>,
    next_like: usize,
    last_at: Option<DateTime<Utc>>,
    last_session: Option<Uuid>,
    pos: u32,
    streak: u32,
    group: u32,
    /// Векторы сессии: «заходит» и «пропускается» с накопленным весом — как у волны.
    hits: Option<(Vec<f32>, f32)>,
    misses: Option<(Vec<f32>, f32)>,
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

struct Event {
    user_id: Uuid,
    track_id: Uuid,
    recording_id: Option<Uuid>,
    started_at: DateTime<Utc>,
    played_ms: i32,
    duration_ms: i32,
    end_reason: String,
    context_type: Option<String>,
    wave_session_id: Option<Uuid>,
    listening_session: Option<Uuid>,
    output: Option<String>,
    platform: Option<String>,
    main_artist: Option<Uuid>,
    rank: Option<i32>,
    release_date: Option<NaiveDate>,
    import: bool,
}

/// Набор из прослушиваний всех слушателей или одного (`user`; последние [`MAX_ROWS`]), по времени.
/// Признаки знакомства и хода сессии — на момент прослушивания; грани вкуса — по истории до отложенной
/// части ([`Recipe::cut_facets`]) или нынешние. Отложенная часть — с момента `cut` (задание компьютеру,
/// выданное раньше) или по [`holdout_start`].
pub async fn dataset(
    db: &PgPool,
    recipe: &Recipe,
    user: Option<Uuid>,
    cut: Option<DateTime<Utc>>,
) -> Result<Dataset, sqlx::Error> {
    let settings = Settings::load(db).await?;
    let since_days = recipe.since_days.unwrap_or(365 * 50) as i32;
    let mut events: Vec<Event> = sqlx::query!(
        r#"SELECT p.user_id, p.track_id, t.recording_id AS "recording_id?", p.started_at, p.played_ms,
                  p.duration_ms, p.end_reason, p.context_type, p.wave_session_id, p.listening_session, p.output,
                  COALESCE(d.platform, p.platform) AS "platform?", p.source,
                  (SELECT ta.artist_id FROM track_artists ta WHERE ta.track_id = p.track_id
                   ORDER BY ta.position, ta.artist_id LIMIT 1) AS main_artist,
                  t.rank, al.release_date AS "release_date?"
           FROM play_events p
           JOIN users u ON u.id = p.user_id AND NOT u.disabled
           JOIN tracks t ON t.id = p.track_id
           LEFT JOIN albums al ON al.id = t.album_id
           LEFT JOIN devices d ON d.id = p.device_id
           WHERE p.started_at > now() - make_interval(days => $2)
             AND p.started_at <= now() + interval '1 day'
             AND ($3::uuid IS NULL OR p.user_id = $3)
           ORDER BY p.started_at DESC, p.id DESC
           LIMIT $1"#,
        MAX_ROWS,
        since_days,
        user,
    )
    .fetch_all(db)
    .await?
    .into_iter()
    .map(|r| Event {
        user_id: r.user_id,
        track_id: r.track_id,
        recording_id: r.recording_id,
        started_at: r.started_at,
        played_ms: r.played_ms,
        duration_ms: r.duration_ms,
        end_reason: r.end_reason,
        context_type: r.context_type,
        wave_session_id: r.wave_session_id,
        listening_session: r.listening_session,
        output: r.output,
        platform: r.platform,
        main_artist: r.main_artist,
        rank: r.rank,
        release_date: r.release_date,
        import: r.source == "import",
    })
    .collect();
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

    // Где кончится обучающая часть: по меткам строк, которые войдут в набор (анализ есть, метка есть)
    let labelled_times: Vec<DateTime<Utc>> = events
        .iter()
        .filter(|e| {
            e.recording_id.is_some_and(|r| analysed.contains_key(&r))
                && label(
                    &e.end_reason,
                    e.played_ms,
                    e.duration_ms,
                    false,
                    e.import && recipe.import_stops_skip,
                )
                .is_some()
        })
        .map(|e| e.started_at)
        .collect();
    let cut = cut.or_else(|| {
        labelled_times
            .get(holdout_start(&labelled_times, recipe.min_holdout_share))
            .copied()
    });

    let users: HashSet<Uuid> = events.iter().map(|e| e.user_id).collect();
    let mut facets: HashMap<Uuid, Vec<(Vec<f32>, f32)>> = HashMap::new();
    for user in users {
        let fs = match (recipe.cut_facets, cut) {
            (true, Some(cut)) => facets_at(&events, &analysed, &liked_at, user, cut, recipe),
            _ => taste::facets(db, user)
                .await?
                .into_iter()
                .map(|f| (f.centroid, f.weight))
                .collect(),
        };
        facets.insert(user, fs);
    }

    let mut data = Dataset {
        set: TrainSet {
            sound_dim: SOUND_DIM,
            semantic_dim: SEMANTIC_DIM,
            scalar_dim: SCALARS.len(),
            ..Default::default()
        },
        ..Default::default()
    };
    let mut vector_of: HashMap<Uuid, u32> = HashMap::new();
    let mut next_group = 0u32;
    for e in &events {
        let h = history.entry(e.user_id).or_default();
        let continued = match (e.listening_session, h.last_session) {
            (Some(s), Some(last)) => s == last,
            _ => h.last_at.is_some_and(|t| e.started_at - t < SESSION_GAP),
        };
        if !continued || h.last_at.is_none() {
            h.pos = 0;
            h.streak = 0;
            h.hits = None;
            h.misses = None;
            h.group = next_group;
            next_group += 1;
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
        let liked_before = liked_at
            .get(&(e.user_id, e.track_id))
            .is_some_and(|t| *t < e.started_at - LIKE_BEFORE);
        let liked_now = like_events.get(&(e.user_id, e.track_id)).is_some_and(|ts| {
            ts.iter()
                .any(|t| *t >= e.started_at - LIKE_BEFORE && *t <= window_end)
        }) || e
            .wave_session_id
            .is_some_and(|s| impression_likes.contains(&(s, e.track_id)))
            || (recipe.library_likes && liked_at.contains_key(&(e.user_id, e.track_id)));
        let import_skip = e.import && recipe.import_stops_skip;
        let labels = label(
            &e.end_reason,
            e.played_ms,
            e.duration_ms,
            liked_now,
            import_skip,
        );
        let plays = h.plays.get(&e.track_id);
        let completed = plays.map_or(0, |p| p.iter().filter(|(_, c, _)| *c).count() as i64);
        let skips = plays.map_or(0, |p| p.iter().filter(|(_, _, s)| *s).count() as i64);
        let plays_30d = plays.map_or(0, |p| {
            p.iter()
                .filter(|(t, _, _)| *t > e.started_at - Duration::days(30))
                .count() as i64
        });
        let days_since = plays
            .and_then(|p| p.last())
            .map(|(t, _, _)| (e.started_at - *t).num_minutes() as f32 / 1440.0);
        let analysed_track = e.recording_id.and_then(|r| analysed.get(&r));
        if let (Some(y), Some(a)) = (labels, analysed_track) {
            let user_facets: Vec<(&[f32], f32)> = facets
                .get(&e.user_id)
                .map(|fs| fs.iter().map(|(c, w)| (c.as_slice(), *w)).collect())
                .unwrap_or_default();
            let (taste_sim, facet_weight) = nearest_facet(&a.sound, &user_facets);
            let fam = Familiarity {
                taste_sim,
                facet_weight,
                liked: liked_before,
                completed,
                artist_familiar: e.main_artist.is_some_and(|x| h.artists.contains(&x)),
                plays_30d,
                skips,
                days_since,
                hit_sim: h.hits.as_ref().map(|(v, _)| cosine(&a.sound, v)),
                miss_sim: h.misses.as_ref().map(|(v, _)| cosine(&a.sound, v)),
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
            let idx = *vector_of.entry(recording).or_insert_with(|| {
                data.set.vectors.extend_from_slice(&a.sound);
                match &a.semantic {
                    Some(s) => data.set.vectors.extend_from_slice(s),
                    None => data
                        .set
                        .vectors
                        .extend(std::iter::repeat_n(0.0, SEMANTIC_DIM)),
                }
                (data.set.vector_rows() - 1) as u32
            });
            data.set.row_vector.push(idx);
            data.set.scalars.extend_from_slice(&scalars(
                &feat,
                a.semantic.is_some(),
                &moment,
                &fam,
            ));
            data.set.labels.extend_from_slice(&y);
            data.set.groups.push(h.group);
            data.baseline.push(rules_prior(&feat, &fam, e.started_at));
            data.times.push(e.started_at);
            data.users.push(e.user_id);
        }
        // История — после строки: признаки видят только прошлое
        let complete = labels.is_some_and(|y| y[1] > 0.5);
        let skipped = labels.is_some_and(|y| y[0] > 0.5);
        h.plays
            .entry(e.track_id)
            .or_default()
            .push((e.started_at, complete, skipped));
        if let Some(a) = e.main_artist {
            h.artists.insert(a);
        }
        if let Some(y) = labels {
            h.streak = if y[0] > 0.5 { h.streak + 1 } else { 0 };
        }
        // Векторы сессии — как у волны (`rank::effect`): дослушал — 0,5 в «заходит», лайк — 1,
        // ранний пропуск — 1 в «пропускается», поздний — 0,4
        if let (Some(y), Some(a)) = (labels, analysed_track) {
            let hit = 0.5 * y[1] + y[2];
            let early = e.played_ms < 30_000;
            let miss = if y[0] > 0.5 {
                if early { 1.0 } else { 0.4 }
            } else {
                0.0
            };
            if hit > 0.0 {
                let (old, w) = h.hits.take().map_or((None, 0.0), |(v, w)| (Some(v), w));
                h.hits = Some((blend(old.as_deref(), w, &a.sound, hit), w + hit));
            }
            if miss > 0.0 {
                let (old, w) = h.misses.take().map_or((None, 0.0), |(v, w)| (Some(v), w));
                h.misses = Some((blend(old.as_deref(), w, &a.sound, miss), w + miss));
            }
        }
        h.pos += 1;
        h.last_at = Some(e.started_at);
        h.last_session = e.listening_session;
    }
    let last = data.times.last().copied();
    data.set.weights = data
        .times
        .iter()
        .map(|t| match (recipe.half_life_days, last) {
            (Some(half), Some(last)) => {
                let age = (last - *t).num_minutes() as f32 / 1440.0;
                0.5f32.powf(age / half).max(MIN_WEIGHT)
            }
            _ => 1.0,
        })
        .collect();
    data.holdout_from = match cut {
        Some(cut) => data.times.partition_point(|t| *t < cut),
        None => holdout_start(&data.times, recipe.min_holdout_share),
    };
    Ok(data)
}

/// Грани слушателя, какими они были бы к моменту `cut`: те же веса треков, что у ночного пересчёта
/// (`taste::listened_weight`), но только по прослушиваниям и лайкам до `cut`.
fn facets_at(
    events: &[Event],
    analysed: &HashMap<Uuid, Analysed>,
    liked_at: &HashMap<(Uuid, Uuid), DateTime<Utc>>,
    user: Uuid,
    cut: DateTime<Utc>,
    recipe: &Recipe,
) -> Vec<(Vec<f32>, f32)> {
    let mut by_recording: HashMap<Uuid, (f32, f32, Option<f32>)> = HashMap::new();
    for e in events
        .iter()
        .filter(|e| e.user_id == user && e.started_at < cut)
    {
        let Some(r) = e.recording_id.filter(|r| analysed.contains_key(r)) else {
            continue;
        };
        if e.started_at < cut - Duration::days(365) {
            continue;
        }
        let age = (cut - e.started_at).num_minutes() as f32 / 1440.0;
        let Some(y) = label(
            &e.end_reason,
            e.played_ms,
            e.duration_ms,
            false,
            e.import && recipe.import_stops_skip,
        ) else {
            continue;
        };
        let entry = by_recording.entry(r).or_default();
        if y[1] > 0.5 {
            entry.0 += (-age / 60.0).exp();
        }
        if y[0] > 0.5 && e.played_ms < 30_000 {
            entry.1 += (-age / 30.0).exp();
        }
        if let Some(t) = liked_at.get(&(user, e.track_id)).filter(|t| **t < cut) {
            entry.2 = Some((cut - *t).num_minutes() as f32 / 1440.0);
        }
    }
    let mut items: Vec<(Uuid, f32)> = by_recording
        .into_iter()
        .map(|(r, (full, skips, liked))| (r, taste::listened_weight(liked, full, skips)))
        .filter(|(_, w)| *w > 0.15)
        .collect();
    items.sort_by_key(|(r, _)| *r);
    let points: Vec<Vec<f32>> = items
        .iter()
        .map(|(r, _)| analysed[r].sound.clone())
        .collect();
    let weights: Vec<f32> = items.iter().map(|(_, w)| *w).collect();
    taste::centroids(&points, &weights, user)
}

// ───────────── Проверка и решение ─────────────

/// Чем кончилась проверка версии на отложенной части.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Evaluation {
    pub train: usize,
    pub holdout: usize,
    pub net: Scores,
    pub rules: Scores,
    pub vs_rules: Comparison,
    /// Текущая включённая версия на той же отложенной части.
    pub current: Option<(String, Scores)>,
    pub vs_current: Option<Comparison>,
    /// Текущая версия сама точнее правил на этой части.
    pub current_beats_rules: Option<bool>,
}

/// Вероятности сети для строк `range` набора: onnxruntime, пачками.
fn predict_rows(
    model: &Loaded,
    set: &TrainSet,
    range: std::ops::Range<usize>,
) -> Result<Vec<[f32; 3]>, String> {
    let mut out = Vec::with_capacity(range.len());
    let mut start = range.start;
    while start < range.end {
        let end = (start + 512).min(range.end);
        out.extend(model.predict(set.features(start..end), end - start)?);
        start = end;
    }
    Ok(out)
}

/// Правила как вероятности: оценка калибруется логистической регрессией на обучающей части.
fn rules_probs(data: &Dataset) -> Vec<[f32; 3]> {
    let h = data.holdout_from;
    let cal: Vec<Calibration> = (0..3)
        .map(|k| {
            let y: Vec<f32> = (0..h).map(|i| data.set.labels[i * 3 + k]).collect();
            Calibration::fit(&data.baseline[..h], &y)
        })
        .collect();
    data.baseline[h..]
        .iter()
        .map(|x| [cal[0].apply(*x), cal[1].apply(*x), cal[2].apply(*x)])
        .collect()
}

fn ranked(probs: &[[f32; 3]]) -> Vec<f32> {
    probs.iter().map(metrics::wave_score).collect()
}

/// Проверка сети `net` на отложенной части набора — против правил и против включённой `current`.
/// Синхронно: вызывать из `spawn_blocking`.
pub fn evaluate(
    data: &Dataset,
    net: &Loaded,
    current: Option<&Loaded>,
) -> Result<Evaluation, String> {
    let h = data.holdout_from;
    let n = data.rows();
    let labels = &data.set.labels[h * 3..];
    let groups = &data.set.groups[h..];
    let probs = predict_rows(net, &data.set, h..n)?;
    let rules = rules_probs(data);
    let rules_rank: Vec<f32> = data.baseline[h..].to_vec();
    let mut eval = Evaluation {
        train: h,
        holdout: n - h,
        net: metrics::scores(&probs, labels, groups, &ranked(&probs)),
        rules: metrics::scores(&rules, labels, groups, &rules_rank),
        vs_rules: metrics::compare(&probs, &rules, labels, groups, BOOTSTRAP, 7),
        ..Default::default()
    };
    if let Some(cur) = current {
        match predict_rows(cur, &data.set, h..n) {
            Ok(cp) => {
                let vs_rules = metrics::compare(&cp, &rules, labels, groups, BOOTSTRAP, 7);
                let scores = metrics::scores(&cp, labels, groups, &ranked(&cp));
                eval.current_beats_rules = Some(beats(&vs_rules, &scores, &eval.rules));
                eval.vs_current =
                    Some(metrics::compare(&probs, &cp, labels, groups, BOOTSTRAP, 11));
                eval.current = Some((cur.version.clone(), scores));
            }
            Err(e) => {
                tracing::warn!(error = %e, "taste model: current version failed on the holdout")
            }
        }
    }
    Ok(eval)
}

fn beats(vs_rules: &Comparison, net: &Scores, rules: &Scores) -> bool {
    match (
        vs_rules.auc_gain,
        vs_rules.p_better,
        net.logloss,
        rules.logloss,
    ) {
        (Some(gain), Some(p), Some(loss), Some(base)) => {
            gain >= AUC_MARGIN && p >= P_BETTER && loss <= base
        }
        _ => false,
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Verdict {
    pub beats_rules: bool,
    /// Лучше текущей включённой; текущей нет — `None`.
    pub beats_current: Option<bool>,
    /// Включить эту версию.
    pub activate: bool,
    /// Не включаем, но текущая остаётся (она сама точнее правил).
    pub keep_current: bool,
    pub note: String,
}

fn pct(x: Option<f64>) -> String {
    x.map_or("—".into(), |p| format!("{:.0} %", p * 100.0))
}

fn num(x: Option<f64>) -> String {
    x.map_or("—".into(), |v| format!("{v:.3}"))
}

/// Включать ли: точнее правил (AUC выше на [`AUC_MARGIN`], уверенность бутстрепа не ниже [`P_BETTER`],
/// logloss не хуже) и не хуже включённой сети на той же отложенной части.
pub fn judge(e: &Evaluation) -> Verdict {
    let beats_rules = beats(&e.vs_rules, &e.net, &e.rules);
    let beats_current = e.vs_current.as_ref().map(|c| {
        c.auc_gain.is_some_and(|g| g >= 0.0) && c.logloss_gain.is_some_and(|g| g >= -LOGLOSS_SLACK)
    });
    let activate = beats_rules && beats_current != Some(false);
    let keep_current = !activate && e.current_beats_rules == Some(true);
    let vs = format!(
        "AUC {} против {} у правил, уверенность {}, logloss {} против {}",
        num(e.net.auc),
        num(e.rules.auc),
        pct(e.vs_rules.p_better),
        num(e.net.logloss),
        num(e.rules.logloss),
    );
    let current = e.current.as_ref().map(|(v, s)| (v.as_str(), s.auc));
    let note = match (activate, beats_rules, current) {
        (true, _, Some((v, auc))) => {
            format!("Включена: точнее правил ({vs}) и {v} (AUC {})", num(auc))
        }
        (true, _, None) => format!("Включена: точнее правил ({vs})"),
        (false, true, Some((v, auc))) => format!(
            "Не включена: {v} не хуже (AUC {} против {})",
            num(auc),
            num(e.net.auc)
        ),
        (false, _, _) if e.net.auc.is_none() => {
            "Не включена: на отложенных данных не с чем сравнить — у голов один исход".into()
        }
        (false, _, _) => format!("Не включена: правила не хуже ({vs})"),
    };
    Verdict {
        beats_rules,
        beats_current,
        activate,
        keep_current,
        note,
    }
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

/// Загруженные версии (общая и свои у слушателей) и те, что загрузить не удалось.
struct Cache {
    loaded: HashMap<Uuid, Arc<Loaded>>,
    failed: HashSet<Uuid>,
}

static CACHE: OnceLock<Mutex<Cache>> = OnceLock::new();

fn cache() -> std::sync::MutexGuard<'static, Cache> {
    let m = CACHE.get_or_init(|| {
        Mutex::new(Cache {
            loaded: HashMap::new(),
            failed: HashSet::new(),
        })
    });
    m.lock().unwrap_or_else(|p| p.into_inner())
}

/// Сеть, которая собирает поток слушателю: своя, если включена, иначе общая. Правила или сбой
/// загрузки — `None`. `user = None` — только общая.
pub async fn active(db: &PgPool, user: Option<Uuid>) -> Result<Option<Arc<Loaded>>, sqlx::Error> {
    let rows = sqlx::query!(
        r#"SELECT id, version, user_id FROM model_versions
           WHERE kind = 'taste' AND active AND (user_id IS NULL OR user_id = $1)
           ORDER BY user_id IS NULL"#,
        user,
    )
    .fetch_all(db)
    .await?;
    for row in rows {
        if !row.version.starts_with(PREFIX) {
            continue;
        }
        if let Some(l) = load(db, row.id, &row.version).await? {
            return Ok(Some(l));
        }
    }
    Ok(None)
}

async fn load(db: &PgPool, id: Uuid, version: &str) -> Result<Option<Arc<Loaded>>, sqlx::Error> {
    {
        let c = cache();
        if let Some(l) = c.loaded.get(&id) {
            return Ok(Some(l.clone()));
        }
        if c.failed.contains(&id) {
            return Ok(None);
        }
    }
    let artifact = sqlx::query_scalar!("SELECT artifact FROM model_versions WHERE id = $1", id)
        .fetch_optional(db)
        .await?
        .flatten();
    let name = version.to_string();
    let built = match artifact {
        Some(bytes) => tokio::task::spawn_blocking(move || Loaded::build(id, &name, &bytes))
            .await
            .unwrap_or_else(|e| Err(e.to_string())),
        None => Err("у версии нет ONNX".into()),
    };
    let mut c = cache();
    match built {
        Ok(l) => {
            let l = Arc::new(l);
            tracing::info!(version = %l.version, "taste model loaded");
            // Выключенные версии из памяти уходят: держим немного
            if c.loaded.len() >= 16 {
                c.loaded.clear();
            }
            c.loaded.insert(id, l.clone());
            Ok(Some(l))
        }
        Err(e) => {
            tracing::warn!(model = %id, error = %e, "taste model failed to load, ranking stays on rules");
            c.failed.insert(id);
            Ok(None)
        }
    }
}

/// Версия, которая пишется в показы: сеть, если она оценила выдачу, иначе правила.
pub fn version_of(entries: &[&Cand]) -> String {
    entries
        .iter()
        .find_map(|c| c.model_version.clone())
        .unwrap_or_else(|| MODEL_VERSION.to_string())
}

/// Подбор волны прямо сейчас: время, устройство, ход сессии, грани вкуса.
pub struct Live<'a> {
    pub user: Uuid,
    pub at: DateTime<Utc>,
    pub utc_offset_min: i16,
    pub platform: Option<&'a str>,
    pub step: u32,
    pub skip_streak: u32,
    pub facets: &'a [Facet],
}

/// Признаки кандидатов для сети: строки и номера кандидатов, у которых есть анализ.
async fn live_features(
    db: &PgPool,
    cands: &[&Cand],
    live: &Live<'_>,
) -> Result<(Vec<f32>, Vec<usize>), sqlx::Error> {
    let recordings: Vec<Uuid> = cands
        .iter()
        .filter_map(|c| c.recording_id)
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();
    let vectors: HashMap<Uuid, (Vec<f32>, Option<Vec<f32>>)> = sqlx::query!(
        r#"SELECT recording_id, sound::real[] AS "sound!", semantic::real[] AS semantic
           FROM track_analysis WHERE recording_id = ANY($1) AND sound IS NOT NULL"#,
        &recordings,
    )
    .fetch_all(db)
    .await?
    .into_iter()
    .map(|r| (r.recording_id, (r.sound, r.semantic)))
    .collect();
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
    for (i, c) in cands.iter().enumerate() {
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
            skips: c.skips,
            days_since: c
                .last_played
                .map(|t| (live.at - t).num_minutes() as f32 / 1440.0),
            hit_sim: c.hit_sim,
            miss_sim: c.miss_sim,
        };
        let s = scalars(&TrackFeat::of(c), semantic.is_some(), &moment, &fam);
        features.extend(row(sound, semantic.as_deref(), &s));
        idx.push(i);
    }
    Ok((features, idx))
}

/// Вероятности сети для кандидатов (по номерам); сети нет или сбой — пусто.
pub async fn probabilities(
    db: &PgPool,
    cands: &[&Cand],
    live: &Live<'_>,
) -> Option<(String, HashMap<usize, [f32; 3]>)> {
    let loaded = match active(db, Some(live.user)).await {
        Ok(Some(l)) => l,
        Ok(None) => return None,
        Err(e) => {
            tracing::warn!(error = %e, "taste model lookup failed");
            return None;
        }
    };
    let (features, idx) = match live_features(db, cands, live).await {
        Ok(x) => x,
        Err(e) => {
            tracing::warn!(error = %e, "taste model: vectors unavailable");
            return None;
        }
    };
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
    Some((loaded.version.clone(), idx.into_iter().zip(probs).collect()))
}

/// Оценка сети каждому кандидату с анализом (`Cand::model`) — относительно среднего по кандидатам:
/// сеть двигает порядок, а не общий уровень. Сети нет или она дала сбой — ничего не меняется.
pub async fn annotate(db: &PgPool, entries: &mut [Entry], live: &Live<'_>) -> Option<String> {
    let cands: Vec<&Cand> = entries.iter().map(|e| &e.cand).collect();
    let (version, probs) = probabilities(db, &cands, live).await?;
    let n = probs.len();
    let raw: Vec<(usize, f32)> = probs
        .into_iter()
        .map(|(i, p)| {
            // Сами вероятности — в след подбора («Как подобран»)
            entries[i].cand.model_probs = Some(p);
            (i, metrics::wave_score(&p))
        })
        .collect();
    let mean = raw.iter().map(|(_, r)| r).sum::<f32>() / n as f32;
    for (i, r) in raw {
        entries[i].cand.model = Some(MODEL_WEIGHT * (r - mean));
        entries[i].cand.model_version = Some(version.clone());
    }
    Some(version)
}

// ───────────── Версии: запись и включение ─────────────

/// Новая обученная версия: где обучена и для кого.
pub struct NewVersion<'a> {
    pub params: &'a Params,
    pub report: &'a TrainReport,
    pub spec: &'a Spec,
    pub eval: &'a Evaluation,
    pub verdict: &'a Verdict,
    /// Своя сеть слушателя; `None` — общая.
    pub user_id: Option<Uuid>,
    /// Обучена на компьютере: устройство и его имя.
    pub device: Option<(Uuid, String)>,
    pub onnx: Vec<u8>,
}

/// Записанная версия.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Stored {
    pub id: Uuid,
    pub version: String,
    pub active: bool,
    pub note: String,
}

fn metrics_json(v: &NewVersion<'_>) -> Value {
    let e = v.eval;
    let head = |k: usize| {
        json!({
            "positives": e.net.heads.get(k).map_or(0, |h| h.positives),
            "auc": e.net.heads.get(k).and_then(|h| h.auc),
            "logloss": e.net.heads.get(k).map(|h| h.logloss),
            "baselineAuc": e.rules.heads.get(k).and_then(|h| h.auc),
            "baselineLogloss": e.rules.heads.get(k).map(|h| h.logloss),
        })
    };
    json!({
        "train": e.train,
        "holdout": e.holdout,
        "rows": e.train + e.holdout,
        "heads": { "skip": head(0), "complete": head(1), "like": head(2) },
        "auc": e.net.auc,
        "logloss": e.net.logloss,
        "baselineAuc": e.rules.auc,
        "baselineLogloss": e.rules.logloss,
        "sessionAuc": e.net.session_auc,
        "baselineSessionAuc": e.rules.session_auc,
        "ndcg": e.net.ndcg,
        "baselineNdcg": e.rules.ndcg,
        "sessions": e.net.sessions,
        "pBetter": e.vs_rules.p_better,
        "current": e.current.as_ref().map(|(version, s)| json!({
            "version": version, "auc": s.auc, "logloss": s.logloss, "sessionAuc": s.session_auc, "ndcg": s.ndcg,
            "pBetter": e.vs_current.as_ref().and_then(|c| c.p_better),
        })),
        "beatsRules": v.verdict.beats_rules,
        "beatsCurrent": v.verdict.beats_current,
        "note": v.verdict.note,
        "params": v.params.trainable(),
        "epochs": v.report.best_epoch,
        "tookMs": v.report.took_ms,
        "curve": { "fit": v.report.fit_loss, "val": v.report.val_loss },
        "reduced": { "sound": v.params.sound_k, "semantic": v.params.semantic_k },
        "spec": v.spec,
        "features": SCALARS.as_slice(),
    })
}

/// Записать версию (ONNX — в хранилище и копией в базе) и решить, что собирает поток: эта версия,
/// прежняя или правила.
pub async fn store(
    db: &PgPool,
    storage: &Storage,
    v: NewVersion<'_>,
) -> Result<Stored, sqlx::Error> {
    let stamp = format!("{PREFIX}{}", Utc::now().format("%Y%m%d-%H%M%S"));
    // Два обучения в одну секунду (ручное «Обучить» сразу за ночным) — суффикс
    let same = sqlx::query_scalar!(
        r#"SELECT count(*) AS "n!" FROM model_versions WHERE kind = 'taste' AND version LIKE $1 || '%'"#,
        stamp,
    )
    .fetch_one(db)
    .await?;
    let version = if same == 0 {
        stamp
    } else {
        format!("{stamp}-{}", same + 1)
    };
    let key = format!("models/taste/{version}.onnx");
    let artifact_key = match storage
        .put(&key, v.onnx.clone(), "application/octet-stream", None)
        .await
    {
        Ok(()) => Some(key),
        Err(e) => {
            tracing::warn!(error = %e, "taste model: storage upload failed, kept in the database only");
            None
        }
    };
    let metrics = metrics_json(&v);
    let (device_id, device_name) = v.device.clone().unzip();
    let origin = if v.device.is_some() {
        "device"
    } else {
        "server"
    };

    let mut tx = db.begin().await?;
    let id = sqlx::query_scalar!(
        "INSERT INTO model_versions (kind, version, metrics, artifact_key, artifact, active, user_id, origin,
                                     device_id, device_name)
         VALUES ('taste', $1, $2, $3, $4, false, $5, $6, $7, $8) RETURNING id",
        version,
        metrics,
        artifact_key,
        v.onnx,
        v.user_id,
        origin,
        device_id,
        device_name,
    )
    .fetch_one(&mut *tx)
    .await?;
    if !v.verdict.keep_current {
        sqlx::query!(
            "UPDATE model_versions SET active = false
             WHERE kind = 'taste' AND active AND user_id IS NOT DISTINCT FROM $1",
            v.user_id,
        )
        .execute(&mut *tx)
        .await?;
        if v.verdict.activate {
            sqlx::query!("UPDATE model_versions SET active = true WHERE id = $1", id)
                .execute(&mut *tx)
                .await?;
        } else if v.user_id.is_none() {
            // Свежая проверка говорит, что правила точнее: прежняя общая сеть тоже выключается.
            // Своя сеть слушателя просто выключается — дальше его поток собирает общая
            sqlx::query!(
                "INSERT INTO model_versions (kind, version, active) VALUES ('taste', $1, true)
                 ON CONFLICT (kind, version) DO UPDATE SET active = true",
                MODEL_VERSION,
            )
            .execute(&mut *tx)
            .await?;
        }
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
    tracing::info!(%version, origin, active = v.verdict.activate, note = %v.verdict.note, "taste model stored");
    Ok(Stored {
        id,
        version,
        active: v.verdict.activate,
        note: v.verdict.note.clone(),
    })
}

/// Веса → ONNX → загрузка → проверка на отложенной части набора против правил и включённой версии.
/// Одна дорога и для своего обучения, и для весов с компьютера.
pub async fn check(
    db: &PgPool,
    data: Arc<Dataset>,
    params: &Params,
    user: Option<Uuid>,
) -> Result<(Vec<u8>, Evaluation, Verdict), String> {
    if params.input_dim() != FEATURE_DIM || params.scalar_dim != SCALARS.len() {
        return Err(format!(
            "ширина входа {} вместо {FEATURE_DIM}",
            params.input_dim()
        ));
    }
    let onnx = onnx::export(params);
    let current = match user {
        Some(u) => personal_active(db, u).await,
        None => active(db, None).await,
    }
    .map_err(|e| e.to_string())?;
    let bytes = onnx.clone();
    let eval = tokio::task::spawn_blocking(move || {
        let net = Loaded::build(Uuid::nil(), "candidate", &bytes)
            .map_err(|e| format!("сервер не загружает ONNX: {e}"))?;
        evaluate(&data, &net, current.as_deref())
    })
    .await
    .map_err(|e| e.to_string())??;
    let verdict = judge(&eval);
    Ok((onnx, eval, verdict))
}

/// Своя включённая сеть слушателя (без общей).
async fn personal_active(db: &PgPool, user: Uuid) -> Result<Option<Arc<Loaded>>, sqlx::Error> {
    let Some(row) = sqlx::query!(
        "SELECT id, version FROM model_versions WHERE kind = 'taste' AND active AND user_id = $1",
        user,
    )
    .fetch_optional(db)
    .await?
    else {
        return Ok(None);
    };
    load(db, row.id, &row.version).await
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
                  AND (SELECT count(*) FROM play_events) >= $2
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

/// Задача `taste_model`: набор → обучение здесь же → проверка через onnxruntime на отложенной части →
/// версия в `model_versions` → включение, если точнее правил и текущей; иначе — прежняя или правила.
pub async fn run(ctx: Arc<Context>, _job: JobRow) -> JobResult {
    let data = Arc::new(dataset(&ctx.db, &Recipe::default(), None, None).await?);
    if !data.enough() {
        return Ok(too_little(data.rows()).into());
    }
    let spec = spec();
    let train_set = data.train_part();
    let s = spec.clone();
    let (params, report) =
        tokio::task::spawn_blocking(move || dalywave_taste::train(&train_set, &s))
            .await
            .map_err(|e| JobError::retry(e.to_string()))?
            .map_err(JobError::permanent)?;
    let (onnx, eval, verdict) = check(&ctx.db, data.clone(), &params, None)
        .await
        .map_err(JobError::permanent)?;
    let stored = store(
        &ctx.db,
        &ctx.storage,
        NewVersion {
            params: &params,
            report: &report,
            spec: &spec,
            eval: &eval,
            verdict: &verdict,
            user_id: None,
            device: None,
            onnx,
        },
    )
    .await?;
    Ok(json!({
        "outcome": "trained",
        "version": stored.version,
        "active": stored.active,
        "rows": data.rows(),
        "message": stored.note,
    })
    .into())
}

// ───────────── Админка ─────────────

struct VersionRow {
    id: Uuid,
    version: String,
    active: bool,
    created_at: DateTime<Utc>,
    metrics: Value,
    origin: String,
    device_name: Option<String>,
    user_name: Option<String>,
}

fn metric(m: &Value, key: &str) -> Option<f64> {
    m.get(key).and_then(Value::as_f64)
}

fn view(r: VersionRow) -> ModelVersionView {
    let m = &r.metrics;
    let learned = r.version.starts_with(PREFIX);
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
    let current = m.get("current").filter(|c| c.is_object());
    ModelVersionView {
        id: r.id,
        learned,
        active: r.active,
        created_at: r.created_at,
        auc: metric(m, "auc"),
        logloss: metric(m, "logloss"),
        baseline_auc: metric(m, "baselineAuc"),
        baseline_logloss: metric(m, "baselineLogloss"),
        session_auc: metric(m, "sessionAuc"),
        baseline_session_auc: metric(m, "baselineSessionAuc"),
        ndcg: metric(m, "ndcg"),
        baseline_ndcg: metric(m, "baselineNdcg"),
        p_better: metric(m, "pBetter"),
        current_version: current
            .and_then(|c| c.get("version"))
            .and_then(Value::as_str)
            .map(str::to_string),
        current_auc: current.and_then(|c| metric(c, "auc")),
        beats_current: m.get("beatsCurrent").and_then(Value::as_bool),
        heads,
        train_rows: m.get("train").and_then(Value::as_i64),
        holdout_rows: m.get("holdout").and_then(Value::as_i64),
        params: m.get("params").and_then(Value::as_i64),
        epochs: m.get("epochs").and_then(Value::as_i64),
        beats_rules: m.get("beatsRules").and_then(Value::as_bool),
        note: m.get("note").and_then(Value::as_str).map(str::to_string),
        origin: if learned { r.origin } else { "rules".into() },
        device_name: r.device_name,
        user_name: r.user_name,
        version: r.version,
    }
}

/// Версии модели вкуса, новые первыми (последние 30): общие и свои у слушателей.
pub async fn versions(db: &PgPool) -> Result<Vec<ModelVersionView>, sqlx::Error> {
    Ok(sqlx::query!(
        r#"SELECT m.id, m.version, m.active, m.created_at, m.metrics, m.origin, m.device_name,
                  u.name AS "user_name?"
           FROM model_versions m LEFT JOIN users u ON u.id = m.user_id
           WHERE m.kind = 'taste' ORDER BY m.created_at DESC, m.id DESC LIMIT 30"#
    )
    .fetch_all(db)
    .await?
    .into_iter()
    .map(|r| {
        view(VersionRow {
            id: r.id,
            version: r.version,
            active: r.active,
            created_at: r.created_at,
            metrics: r.metrics,
            origin: r.origin,
            device_name: r.device_name,
            user_name: r.user_name,
        })
    })
    .collect())
}

/// Последнее обучение, сколько данных есть сейчас и последние задания компьютерам.
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
           WHERE p.end_reason NOT IN ('error', 'interrupted', 'transferred')"#
    )
    .fetch_one(db)
    .await?;
    let tasks = super::training::recent(db).await?;
    let mut out = ModelTraining {
        rows,
        min_rows: MIN_ROWS as i64,
        device_tasks: tasks,
        ..Default::default()
    };
    let Some(last) = last else {
        return Ok(out);
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
    out.running = matches!(last.status.as_str(), "queued" | "running");
    out.last_run_at = last.at;
    out.outcome = outcome;
    out.message = message;
    Ok(out)
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
            skips: 2,
            days_since: Some(10.0),
            hit_sim: Some(0.7),
            miss_sim: None,
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
        assert!(s[idx("track_skips")] > 0.0);
        assert!(s[idx("days_since_play")] > 0.0 && s[idx("days_since_play")] < 1.0);
        assert_eq!(s[idx("heard_before")], 1.0);
        assert_eq!(s[idx("hit_sim")], 0.7);
        assert_eq!((s[idx("has_hits")], s[idx("has_misses")]), (1.0, 0.0));
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
        assert_eq!(empty[idx("heard_before")], 0.0);
        assert_eq!(
            empty[idx("device_mobile")] + empty[idx("device_desktop")],
            0.0,
            "сайт"
        );
    }

    #[test]
    fn labels_say_nothing_about_errors_and_stops() {
        assert_eq!(
            label("completed", 180_000, 180_000, true, false),
            Some([0.0, 1.0, 1.0])
        );
        assert_eq!(
            label("skipped", 5_000, 180_000, false, false),
            Some([1.0, 0.0, 0.0])
        );
        // Поздний пропуск — не пропуск и не дослушивание
        assert_eq!(
            label("skipped", 120_000, 180_000, false, false),
            Some([0.0, 0.0, 0.0])
        );
        // Дослушал почти до конца и переключил — дослушал
        assert_eq!(
            label("skipped", 170_000, 180_000, false, false),
            Some([0.0, 1.0, 0.0])
        );
        assert_eq!(label("error", 1_000, 180_000, false, false), None);
        assert_eq!(label("transferred", 60_000, 180_000, false, false), None);
        assert_eq!(label("stopped", 60_000, 180_000, false, false), None);
        assert_eq!(label("completed", 1, 0, false, false), None);
        // Импорт Spotify: «остановлено» в начале трека (endplay) — пропуск; в середине — без метки
        assert_eq!(
            label("stopped", 9_000, 180_000, false, true),
            Some([1.0, 0.0, 0.0])
        );
        assert_eq!(label("stopped", 120_000, 180_000, false, true), None);
        assert_eq!(
            label("stopped", 179_000, 180_000, true, true),
            Some([0.0, 1.0, 1.0])
        );
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

    fn eval(auc: f64, base: f64, p: f64, loss: f64, base_loss: f64) -> Evaluation {
        let s = |a: f64, l: f64| Scores {
            auc: Some(a),
            logloss: Some(l),
            ..Default::default()
        };
        Evaluation {
            net: s(auc, loss),
            rules: s(base, base_loss),
            vs_rules: Comparison {
                auc_gain: Some(auc - base),
                logloss_gain: Some(base_loss - loss),
                p_better: Some(p),
            },
            ..Default::default()
        }
    }

    #[test]
    fn verdict_needs_better_auc_confidence_and_no_worse_logloss() {
        assert!(judge(&eval(0.72, 0.65, 0.99, 0.50, 0.55)).activate);
        assert!(
            !judge(&eval(0.652, 0.65, 0.99, 0.50, 0.55)).activate,
            "в пределах шума"
        );
        assert!(
            !judge(&eval(0.72, 0.65, 0.7, 0.50, 0.55)).activate,
            "неуверенно"
        );
        assert!(
            !judge(&eval(0.72, 0.65, 0.99, 0.60, 0.55)).activate,
            "logloss хуже"
        );
        let none = judge(&Evaluation::default());
        assert!(!none.activate && none.note.contains("не с чем сравнить"));

        // Лучше правил, но хуже включённой — остаётся включённая
        let mut e = eval(0.72, 0.65, 0.99, 0.50, 0.55);
        e.current = Some((
            "nn-old".into(),
            Scores {
                auc: Some(0.74),
                ..Default::default()
            },
        ));
        e.vs_current = Some(Comparison {
            auc_gain: Some(-0.02),
            logloss_gain: Some(0.0),
            p_better: Some(0.1),
        });
        e.current_beats_rules = Some(true);
        let v = judge(&e);
        assert!(v.beats_rules && !v.activate && v.keep_current);
        assert!(v.note.contains("nn-old"), "{}", v.note);
    }

    #[test]
    fn holdout_is_recent_weeks_but_not_too_small() {
        let day = |d: i64| DateTime::<Utc>::from_timestamp(0, 0).unwrap() + Duration::days(d);
        // Год истории по строке в день: последние четыре недели — 29 строк, меньше 20 % — уходит 73
        let year: Vec<_> = (0..365).map(day).collect();
        assert_eq!(365 - holdout_start(&year, 0.2), 73);
        assert_eq!(
            365 - holdout_start(&year, 0.0),
            60,
            "без доли — не меньше 60"
        );
        // Вся история за неделю: последние недели — это всё, но больше 40 % на проверку не уходит
        let week: Vec<_> = (0..300)
            .map(|i| day(i / 50) + Duration::minutes(i))
            .collect();
        assert_eq!(300 - holdout_start(&week, 0.2), 120);
        // Плотный хвост свежих строк: всё из последних четырёх недель
        let mut tail: Vec<_> = (0..300).map(day).collect();
        tail.extend((0..80).map(|i| day(300) + Duration::minutes(i)));
        assert_eq!(380 - holdout_start(&tail, 0.2), 80 + 27);
        assert_eq!(holdout_start(&[], 0.2), 0);
        // Строки одного момента — в одной части
        let same = vec![day(0); 100];
        assert_eq!(holdout_start(&same, 0.2), 0);
    }
}
