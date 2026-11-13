//! Рекомендации этапа 2: анализ звука ([`analysis`]), индекс каталога вокруг вкуса ([`scout`]), грани вкуса
//! и миксы ([`taste`]), «Моя волна» ([`wave`]) с правилами отбора и расстановки ([`rank`]).
//!
//! Пока своей модели нет, поток собирают правила ([`MODEL_VERSION`]); показы и их исходы копятся
//! в `recommendation_impressions` — на них будет учиться и проверяться маленькая нейросеть вкуса.

pub mod analysis;
pub mod describe;
pub mod device_set;
pub mod explain;
pub mod map;
pub mod model;
pub mod prep;
pub mod profile;
pub mod rank;
pub mod repeat;
pub mod scout;
pub mod suggest;
pub mod taste;
pub mod trace;
pub mod training;
pub mod vectors;
pub mod wave;
pub mod why;

use std::{future::Future, time::Duration};

use chrono::{DateTime, NaiveDate, Timelike, Utc};
use dalywave_protocol::{catalog::AudioStatus, reco::RecoSettings};
use sqlx::{PgConnection, PgExecutor, PgPool};
use uuid::Uuid;

use crate::{
    catalog::CatalogError,
    jobs::{self, NewJob, lanes},
};

/// Ошибка, за которой может стоять взаимная блокировка Postgres.
pub trait Deadlock {
    fn is_deadlock(&self) -> bool;
}

impl Deadlock for sqlx::Error {
    fn is_deadlock(&self) -> bool {
        self.as_database_error()
            .and_then(|d| d.code())
            .is_some_and(|c| c == "40P01")
    }
}

impl Deadlock for CatalogError {
    fn is_deadlock(&self) -> bool {
        matches!(self, Self::Db(e) if e.is_deadlock())
    }
}

/// Параллельные разведки пишут в каталог одних и тех же артистов в разном порядке (у двух артистов общие
/// похожие) и изредка упираются во взаимную блокировку: такая запись повторяется сразу, а не через
/// полминуты очереди.
pub async fn retry_deadlocks<T, E, F, Fut>(mut run: F) -> Result<T, E>
where
    E: Deadlock,
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<T, E>>,
{
    let mut attempt: u32 = 0;
    loop {
        match run().await {
            Err(e) if attempt < 3 && e.is_deadlock() => {
                attempt += 1;
                tracing::debug!(attempt, "catalog write deadlocked, retrying");
                tokio::time::sleep(Duration::from_millis(50 * u64::from(attempt))).await;
            }
            other => return other,
        }
    }
}

/// Версия, которая собирает поток; пишется в показы.
pub const MODEL_VERSION: &str = "rules-1";

/// Настройки разведчика и рекомендаций (`reco_settings`).
#[derive(Debug, Clone)]
pub struct Settings {
    pub tracks_per_day: i32,
    pub max_index: i32,
    pub max_hops: i16,
    pub hop_decay: f32,
    pub utc_offset_min: i16,
    pub updated_at: DateTime<Utc>,
}

impl Settings {
    pub async fn load(db: impl PgExecutor<'_>) -> Result<Self, sqlx::Error> {
        sqlx::query_as!(
            Settings,
            "SELECT tracks_per_day, max_index, max_hops, hop_decay, utc_offset_min, updated_at
             FROM reco_settings WHERE id"
        )
        .fetch_one(db)
        .await
    }

    pub fn view(&self) -> RecoSettings {
        RecoSettings {
            tracks_per_day: self.tracks_per_day,
            max_index: self.max_index,
            max_hops: self.max_hops,
            hop_decay: self.hop_decay,
            utc_offset_min: self.utc_offset_min,
            updated_at: self.updated_at,
        }
    }

    /// Местное время слушателя.
    pub fn local_hour(&self, at: DateTime<Utc>) -> u32 {
        (at + chrono::Duration::minutes(i64::from(self.utc_offset_min))).hour()
    }
}

/// Время суток: у граней вкуса свои поправки на него.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DayPart {
    Night,
    Morning,
    Day,
    Evening,
}

impl DayPart {
    pub const ALL: [DayPart; 4] = [Self::Night, Self::Morning, Self::Day, Self::Evening];

    pub fn of_hour(hour: u32) -> Self {
        match hour {
            5..=11 => Self::Morning,
            12..=16 => Self::Day,
            17..=22 => Self::Evening,
            _ => Self::Night,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Night => "night",
            Self::Morning => "morning",
            Self::Day => "day",
            Self::Evening => "evening",
        }
    }
}

/// Сосед по вектору: трек записи (один на запись) и косинусное сходство.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Neighbour {
    pub track_id: Uuid,
    pub recording_id: Uuid,
    pub similarity: f32,
}

/// HNSW отдаёт не больше `hnsw.ef_search` строк (по умолчанию 40): на время транзакции — сколько нужно.
pub async fn widen_search(conn: &mut PgConnection, limit: i64) -> Result<(), sqlx::Error> {
    let ef = limit.clamp(40, 1000).to_string();
    sqlx::query_scalar!(
        r#"SELECT set_config('hnsw.ef_search', $1, true) AS "v!""#,
        ef
    )
    .fetch_one(conn)
    .await?;
    Ok(())
}

/// Ближайшие по звучанию. У записи с несколькими треками (сингл и он же в альбоме) берётся один:
/// понравившийся этому человеку, затем из альбома, затем популярный.
pub async fn nearest_sound(
    conn: &mut PgConnection,
    vector: &[f32],
    limit: i64,
    user_id: Uuid,
) -> Result<Vec<Neighbour>, sqlx::Error> {
    let rows = sqlx::query!(
        r#"WITH nn AS (
               SELECT a.recording_id, a.sound <=> $1::real[]::halfvec(1280) AS dist
               FROM track_analysis a
               WHERE a.sound IS NOT NULL
               ORDER BY a.sound <=> $1::real[]::halfvec(1280)
               LIMIT $2
           )
           SELECT DISTINCT ON (nn.recording_id) t.id AS "track_id!", nn.recording_id AS "recording_id!",
                  nn.dist AS "dist!"
           FROM nn JOIN tracks t ON t.recording_id = nn.recording_id
           ORDER BY nn.recording_id,
                    EXISTS (SELECT 1 FROM liked_tracks l WHERE l.user_id = $3 AND l.track_id = t.id) DESC,
                    (t.album_id IS NULL), t.rank DESC NULLS LAST, t.id"#,
        vector,
        limit,
        user_id,
    )
    .fetch_all(&mut *conn)
    .await?;
    Ok(sorted(rows.into_iter().map(|r| Neighbour {
        track_id: r.track_id,
        recording_id: r.recording_id,
        similarity: (1.0 - r.dist) as f32,
    })))
}

/// Ближайшие по звучанию среди подходящих под границы настроек ([`rank::Fit`]): подбор ищет подходящее
/// по всему индексу, а не только среди нескольких сотен ближайших ко вкусу. HNSW с фильтром отдаёт меньше
/// строк, чем просили, — итеративный обход продолжает поиск, пока не наберёт `limit`.
pub async fn nearest_fitting(
    conn: &mut PgConnection,
    vector: &[f32],
    limit: i64,
    user_id: Uuid,
    fit: &rank::Fit,
) -> Result<Vec<Neighbour>, sqlx::Error> {
    sqlx::query_scalar!(
        r#"SELECT set_config('hnsw.iterative_scan', 'relaxed_order', true) AS "v!""#
    )
    .fetch_one(&mut *conn)
    .await?;
    let (e_lo, e_hi) = fit.energy.unzip();
    let (v_lo, v_hi) = fit.valence.unzip();
    let (b_lo, b_hi) = fit.bpm.unzip();
    let rows = sqlx::query!(
        r#"WITH nn AS (
               SELECT a.recording_id, a.sound <=> $1::real[]::halfvec(1280) AS dist
               FROM track_analysis a
               WHERE a.sound IS NOT NULL
                 AND ($4::real IS NULL OR a.energy BETWEEN $4 AND $5)
                 AND ($6::real IS NULL OR a.valence BETWEEN $6 AND $7)
                 AND ($8::real IS NULL OR a.danceability >= $8)
                 AND ($9::real IS NULL OR a.bpm BETWEEN $9 AND $10)
                 AND ($11::real IS NULL OR a.voice <= $11)
                 AND ($12::text IS NULL OR (CASE WHEN a.language IS NOT NULL THEN a.language = 'ru'
                          ELSE EXISTS (SELECT 1 FROM tracks t2 WHERE t2.recording_id = a.recording_id
                                         AND (t2.title ~ '[А-Яа-яЁё]'
                                              OR EXISTS (SELECT 1 FROM track_artists ta JOIN artists ar ON ar.id = ta.artist_id
                                                         WHERE ta.track_id = t2.id AND ar.name ~ '[А-Яа-яЁё]')))
                          END) = ($12::text = 'ru'))
                 AND (NOT $13::bool OR NOT EXISTS (
                          SELECT 1 FROM tracks t2 WHERE t2.recording_id = a.recording_id
                            AND (EXISTS (SELECT 1 FROM play_events p WHERE p.user_id = $3 AND p.track_id = t2.id)
                                 OR EXISTS (SELECT 1 FROM liked_tracks l WHERE l.user_id = $3 AND l.track_id = t2.id))))
               ORDER BY a.sound <=> $1::real[]::halfvec(1280)
               LIMIT $2
           )
           SELECT DISTINCT ON (nn.recording_id) t.id AS "track_id!", nn.recording_id AS "recording_id!",
                  nn.dist AS "dist!"
           FROM nn JOIN tracks t ON t.recording_id = nn.recording_id
           ORDER BY nn.recording_id,
                    EXISTS (SELECT 1 FROM liked_tracks l WHERE l.user_id = $3 AND l.track_id = t.id) DESC,
                    (t.album_id IS NULL), t.rank DESC NULLS LAST, t.id"#,
        vector,
        limit,
        user_id,
        e_lo,
        e_hi,
        v_lo,
        v_hi,
        fit.danceability_min,
        b_lo,
        b_hi,
        fit.voice_max,
        fit.language,
        fit.unheard,
    )
    .fetch_all(&mut *conn)
    .await?;
    Ok(sorted(rows.into_iter().map(|r| Neighbour {
        track_id: r.track_id,
        recording_id: r.recording_id,
        similarity: (1.0 - r.dist) as f32,
    })))
}

/// Ближайшие по «смыслу» (CLAP): описание словами против звука.
pub async fn nearest_semantic(
    conn: &mut PgConnection,
    vector: &[f32],
    limit: i64,
    user_id: Uuid,
) -> Result<Vec<Neighbour>, sqlx::Error> {
    let rows = sqlx::query!(
        r#"WITH nn AS (
               SELECT a.recording_id, a.semantic <=> $1::real[]::halfvec(512) AS dist
               FROM track_analysis a
               WHERE a.semantic IS NOT NULL
               ORDER BY a.semantic <=> $1::real[]::halfvec(512)
               LIMIT $2
           )
           SELECT DISTINCT ON (nn.recording_id) t.id AS "track_id!", nn.recording_id AS "recording_id!",
                  nn.dist AS "dist!"
           FROM nn JOIN tracks t ON t.recording_id = nn.recording_id
           ORDER BY nn.recording_id,
                    EXISTS (SELECT 1 FROM liked_tracks l WHERE l.user_id = $3 AND l.track_id = t.id) DESC,
                    (t.album_id IS NULL), t.rank DESC NULLS LAST, t.id"#,
        vector,
        limit,
        user_id,
    )
    .fetch_all(&mut *conn)
    .await?;
    Ok(sorted(rows.into_iter().map(|r| Neighbour {
        track_id: r.track_id,
        recording_id: r.recording_id,
        similarity: (1.0 - r.dist) as f32,
    })))
}

fn sorted(items: impl Iterator<Item = Neighbour>) -> Vec<Neighbour> {
    let mut v: Vec<Neighbour> = items.collect();
    v.sort_by(|a, b| b.similarity.total_cmp(&a.similarity));
    v
}

/// Вектор звучания записи трека, если она проанализирована.
pub async fn track_sound(
    db: impl PgExecutor<'_>,
    track_id: Uuid,
) -> Result<Option<Vec<f32>>, sqlx::Error> {
    let v = sqlx::query_scalar!(
        r#"SELECT a.sound::real[] AS "sound!" FROM tracks t
           JOIN track_analysis a ON a.recording_id = t.recording_id
           WHERE t.id = $1 AND a.sound IS NOT NULL"#,
        track_id
    )
    .fetch_optional(db)
    .await?;
    Ok(v)
}

/// Всё, что нужно отбору и расстановке о треке-кандидате, — одним запросом для сотен треков.
#[derive(Debug, Clone, Default)]
pub struct Cand {
    pub track_id: Uuid,
    pub recording_id: Option<Uuid>,
    pub deezer_id: Option<i64>,
    pub title: String,
    pub artist_ids: Vec<Uuid>,
    pub artist_names: Vec<String>,
    pub audio_status: Option<AudioStatus>,
    /// Популярность у Deezer.
    pub rank: Option<i32>,
    pub release_date: Option<NaiveDate>,
    pub analyzed: bool,
    pub energy: Option<f32>,
    pub valence: Option<f32>,
    pub danceability: Option<f32>,
    pub bpm: Option<f32>,
    pub bpm_confidence: Option<f32>,
    pub key_camelot: Option<String>,
    pub key_confidence: Option<f32>,
    pub voice: Option<f32>,
    pub language: Option<String>,
    pub approachability: Option<f32>,
    pub relaxed: Option<f32>,
    pub party: Option<f32>,
    pub liked: bool,
    /// Прослушиваний за 30 дней — для «отдыха» заслушанного.
    pub plays_30d: i64,
    /// Дослушиваний за всё время.
    pub completed: i64,
    /// Ранних пропусков за всё время (как метка пропуска у сети вкуса).
    pub skips: i64,
    pub last_played: Option<DateTime<Utc>>,
    /// «Не рекомендовать» или «не нравится» — трек или артист.
    pub excluded: bool,
    /// Человек уже слушал или лайкал этого артиста.
    pub artist_familiar: bool,
    /// Сходство с «что сейчас заходит» и «что пропускается» сессии.
    pub hit_sim: Option<f32>,
    pub miss_sim: Option<f32>,
    /// Сходство с ближайшей гранью вкуса слушателя: нет граней или анализа — `None`.
    pub taste_sim: Option<f32>,
    /// Альбом трека и его жанры Deezer — по ним узнаются саундтреки.
    pub album_title: Option<String>,
    pub album_genres: Vec<String>,
    /// Сумма вероятностей стилей Discogs «Stage & Screen» (саундтреки, музыка к фильмам и играм).
    pub screen: f32,
    /// Насколько трек под «меньше такого» слушателя, 0..1 (`taste_damp`).
    pub damped: f32,
    /// Оценка сети вкуса относительно остальных кандидатов ([`model::annotate`]); сеть выключена — `None`.
    pub model: Option<f32>,
    /// Версия сети, которая дала оценку.
    pub model_version: Option<String>,
    /// Вероятности сети: пропуск, дослушивание, лайк — для следа подбора.
    pub model_probs: Option<[f32; 3]>,
    /// След подбора, из которого выбран трек ([`trace`]): пишется вместе с показом.
    pub trace: Option<std::sync::Arc<trace::Draft>>,
}

impl Cand {
    /// Трек знаком: лайкнут или дослушан.
    pub fn familiar(&self) -> bool {
        self.liked || self.completed > 0
    }

    /// Трек уже слышал: лайк или хоть одно прослушивание.
    pub fn heard(&self) -> bool {
        self.liked || self.last_played.is_some()
    }

    pub fn main_artist(&self) -> Option<(Uuid, &str)> {
        Some((
            *self.artist_ids.first()?,
            self.artist_names.first()?.as_str(),
        ))
    }

    pub fn ready(&self) -> bool {
        self.audio_status == Some(AudioStatus::Ready)
    }
}

/// Включён ли у слушателя фильтр «Без нецензурного».
pub async fn hide_explicit(db: &PgPool, user_id: Uuid) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar!(r#"SELECT hide_explicit($1) AS "hide!""#, user_id)
        .fetch_one(db)
        .await
}

/// С фильтром «Без нецензурного» — чистые версии explicit-треков из списка, что есть в каталоге:
/// подбор ставит их вместо explicit (а explicit без чистой версии отсеивает [`candidates`]).
pub async fn clean_swaps(
    db: &PgPool,
    user_id: Uuid,
    ids: &[Uuid],
) -> Result<std::collections::HashMap<Uuid, Uuid>, sqlx::Error> {
    if ids.is_empty() || !hide_explicit(db, user_id).await? {
        return Ok(Default::default());
    }
    Ok(sqlx::query!(
        r#"SELECT id, clean_version(id) AS "clean!" FROM tracks
           WHERE id = ANY($1) AND explicit AND clean_version(id) IS NOT NULL"#,
        ids
    )
    .fetch_all(db)
    .await?
    .into_iter()
    .map(|r| (r.id, r.clean))
    .collect())
}

/// Список id с чистыми версиями вместо explicit, без повторов (чистая могла быть в списке и сама).
pub fn apply_swaps(ids: &[Uuid], swaps: &std::collections::HashMap<Uuid, Uuid>) -> Vec<Uuid> {
    let mut seen = std::collections::HashSet::new();
    ids.iter()
        .map(|id| *swaps.get(id).unwrap_or(id))
        .filter(|id| seen.insert(*id))
        .collect()
}

/// Кандидаты по id в том же порядке; неизвестные пропускаются. `hits` и `misses` — векторы сессии:
/// сходство с ними считает база, а не приложение — векторы треков не гоняются по сети.
pub async fn candidates(
    db: &PgPool,
    ids: &[Uuid],
    user_id: Uuid,
    hits: Option<&[f32]>,
    misses: Option<&[f32]>,
) -> Result<Vec<Cand>, sqlx::Error> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    let hide_explicit = hide_explicit(db, user_id).await?;
    let rows = sqlx::query!(
        r#"WITH fam AS (
               SELECT DISTINCT ta.artist_id FROM track_artists ta
               WHERE ta.track_id IN (SELECT track_id FROM liked_tracks WHERE user_id = $2
                                     UNION SELECT track_id FROM play_events WHERE user_id = $2)
           )
           SELECT t.id, t.recording_id, t.deezer_id, t.title, t.rank, al.release_date AS "release_date?",
                  r.audio_status AS "audio_status?",
                  ARRAY(SELECT ta.artist_id FROM track_artists ta WHERE ta.track_id = t.id
                        ORDER BY ta.position, ta.artist_id) AS "artist_ids!",
                  ARRAY(SELECT ar.name FROM track_artists ta JOIN artists ar ON ar.id = ta.artist_id
                        WHERE ta.track_id = t.id ORDER BY ta.position, ta.artist_id) AS "artist_names!",
                  a.recording_id IS NOT NULL AS "analyzed!",
                  a.energy AS "energy?", a.valence AS "valence?", a.danceability AS "danceability?",
                  a.bpm AS "bpm?", a.bpm_confidence AS "bpm_confidence?", a.key_camelot AS "key_camelot?",
                  a.key_confidence AS "key_confidence?", a.voice AS "voice?", a.language AS "language?",
                  a.approachability AS "approachability?",
                  (a.moods ->> 'relaxed')::real AS relaxed, (a.moods ->> 'party')::real AS party,
                  EXISTS (SELECT 1 FROM liked_tracks l WHERE l.user_id = $2 AND l.track_id = t.id) AS "liked!",
                  (SELECT count(*) FROM play_events p WHERE p.user_id = $2 AND p.track_id = t.id
                     AND p.started_at > now() - interval '30 days') AS "plays_30d!",
                  (SELECT count(*) FROM play_events p WHERE p.user_id = $2 AND p.track_id = t.id
                     AND (p.end_reason = 'completed' OR p.played_ms >= 0.8 * p.duration_ms)) AS "completed!",
                  (SELECT count(*) FROM play_events p WHERE p.user_id = $2 AND p.track_id = t.id
                     AND p.played_ms < 0.5 * p.duration_ms AND p.end_reason <> 'completed'
                     AND (p.end_reason IN ('skipped', 'previous') OR (p.source = 'import' AND p.end_reason = 'stopped')))
                     AS "skips!",
                  (SELECT max(p.started_at) FROM play_events p WHERE p.user_id = $2 AND p.track_id = t.id) AS last_played,
                  (EXISTS (SELECT 1 FROM blocked_items b WHERE b.user_id = $2 AND b.kind = 'track' AND b.ref_id = t.id)
                   OR EXISTS (SELECT 1 FROM blocked_items b JOIN track_artists ta ON ta.artist_id = b.ref_id
                              WHERE b.user_id = $2 AND b.kind = 'artist' AND ta.track_id = t.id)
                   OR COALESCE((SELECT f.kind = 'dislike' FROM feedback_events f
                                WHERE f.user_id = $2 AND f.track_id = t.id AND f.kind IN ('dislike', 'undislike')
                                ORDER BY f.at DESC LIMIT 1), false)
                   OR EXISTS (SELECT 1 FROM track_artists ta
                              WHERE ta.track_id = t.id
                                AND COALESCE((SELECT f.kind = 'dislike' FROM feedback_events f
                                              WHERE f.user_id = $2 AND f.artist_id = ta.artist_id AND f.track_id IS NULL
                                                AND f.kind IN ('dislike', 'undislike')
                                              ORDER BY f.at DESC LIMIT 1), false))
                   -- «Без нецензурного»: explicit без чистой версии не играет
                   OR (t.explicit AND $5)
                  ) AS "excluded!",
                  EXISTS (SELECT 1 FROM track_artists ta WHERE ta.track_id = t.id AND ta.role = 'main'
                            AND ta.artist_id IN (SELECT artist_id FROM fam)) AS "artist_familiar!",
                  CASE WHEN $3::real[] IS NULL OR a.sound IS NULL THEN NULL
                       ELSE (1 - (a.sound <=> $3::real[]::halfvec(1280)))::real END AS hit_sim,
                  CASE WHEN $4::real[] IS NULL OR a.sound IS NULL THEN NULL
                       ELSE (1 - (a.sound <=> $4::real[]::halfvec(1280)))::real END AS miss_sim,
                  (SELECT max(1 - (a.sound <=> f.centroid)) FROM taste_facets f WHERE f.user_id = $2)::real
                      AS taste_sim,
                  al.title AS "album_title?", COALESCE(al.genres, '{}') AS "album_genres!",
                  COALESCE((SELECT sum((s ->> 'p')::real) FROM jsonb_array_elements(
                                CASE WHEN jsonb_typeof(a.styles) = 'array' THEN a.styles ELSE '[]' END) s
                            WHERE s ->> 'name' LIKE 'Stage & Screen%'), 0)::real AS "screen!",
                  CASE WHEN a.sound IS NULL THEN 0 ELSE taste_damp($2, a.sound, 0.72, 0.86) END::real AS "damped!"
           FROM unnest($1::uuid[]) WITH ORDINALITY AS ids(id, ord)
           JOIN tracks t ON t.id = ids.id
           LEFT JOIN recordings r ON r.id = t.recording_id
           LEFT JOIN track_analysis a ON a.recording_id = t.recording_id
           LEFT JOIN albums al ON al.id = t.album_id
           ORDER BY ids.ord"#,
        ids,
        user_id,
        hits,
        misses,
        hide_explicit,
    )
    .fetch_all(db)
    .await?;
    Ok(rows
        .into_iter()
        .map(|r| Cand {
            track_id: r.id,
            recording_id: r.recording_id,
            deezer_id: r.deezer_id,
            title: r.title,
            artist_ids: r.artist_ids,
            artist_names: r.artist_names,
            audio_status: r.audio_status.and_then(|s| s.parse().ok()),
            rank: r.rank,
            release_date: r.release_date,
            analyzed: r.analyzed,
            energy: r.energy,
            valence: r.valence,
            danceability: r.danceability,
            bpm: r.bpm,
            bpm_confidence: r.bpm_confidence,
            key_camelot: r.key_camelot,
            key_confidence: r.key_confidence,
            voice: r.voice,
            language: r.language,
            approachability: r.approachability,
            relaxed: r.relaxed,
            party: r.party,
            liked: r.liked,
            plays_30d: r.plays_30d,
            completed: r.completed,
            skips: r.skips,
            last_played: r.last_played,
            excluded: r.excluded,
            artist_familiar: r.artist_familiar,
            hit_sim: r.hit_sim,
            miss_sim: r.miss_sim,
            taste_sim: r.taste_sim,
            album_title: r.album_title,
            album_genres: r.album_genres,
            screen: r.screen,
            damped: r.damped,
            model: None,
            model_version: None,
            model_probs: None,
            trace: None,
        })
        .collect())
}

/// Плановые задачи рекомендаций — раз в час из обслуживания воркера: проход разведчика, заготовки волны,
/// полный анализ неразобранного скачанного, ночной пересчёт вкуса и обучение сети вкуса, чистка старых
/// сессий и показов.
pub async fn schedule(db: &PgPool) -> Result<ScheduleReport, sqlx::Error> {
    let settings = Settings::load(db).await?;
    let mut conn = db.acquire().await?;
    let scout = jobs::enqueue(
        &mut conn,
        NewJob::new(
            dalywave_protocol::jobs::kind::SCOUT,
            lanes::SCOUT,
            serde_json::json!({}),
        )
        .dedupe("scout")
        .priority(scout::PLAN_PRIORITY),
    )
    .await?
    .created;
    let pools = jobs::enqueue(
        &mut conn,
        NewJob::new(
            dalywave_protocol::jobs::kind::WAVE_POOL,
            lanes::MAINTENANCE,
            serde_json::json!({}),
        )
        .dedupe("wave_pool"),
    )
    .await?
    .created;
    // Плейлисты по описанию с обновлением раз в неделю: срок каждого проверяет сама задача
    jobs::enqueue(
        &mut conn,
        NewJob::new(
            dalywave_protocol::jobs::kind::DESCRIBE_REFRESH,
            lanes::MAINTENANCE,
            serde_json::json!({}),
        )
        .dedupe("describe_refresh"),
    )
    .await?;
    drop(conn);
    let analysis = analysis::schedule_backfill(db, 500).await?;
    let taste = taste::schedule(db, &settings).await?;
    let model = model::schedule(db, &settings).await?;
    let map = map::schedule(db, &settings).await?;
    let traces = trace::cleanup(db).await?;
    let sessions =
        sqlx::query!("DELETE FROM wave_sessions WHERE last_active_at < now() - interval '30 days'")
            .execute(db)
            .await?
            .rows_affected();
    let impressions = sqlx::query!(
        "DELETE FROM recommendation_impressions WHERE shown_at < now() - interval '180 days'"
    )
    .execute(db)
    .await?
    .rows_affected();
    Ok(ScheduleReport {
        scout,
        pools,
        analysis,
        taste,
        model,
        sessions_deleted: sessions,
        impressions_deleted: impressions,
        map,
        traces_deleted: traces,
    })
}

#[derive(Debug, Default)]
pub struct ScheduleReport {
    pub scout: bool,
    /// Поставлен пересчёт заготовок волны.
    pub pools: bool,
    pub analysis: usize,
    pub taste: usize,
    /// Поставлено ночное обучение сети вкуса.
    pub model: bool,
    pub sessions_deleted: u64,
    pub impressions_deleted: u64,
    /// Поставлен пересчёт карты звучания («Как подобран»).
    pub map: bool,
    /// Удалено следов подбора старше 30 дней.
    pub traces_deleted: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn day_parts() {
        assert_eq!(DayPart::of_hour(3), DayPart::Night);
        assert_eq!(DayPart::of_hour(7), DayPart::Morning);
        assert_eq!(DayPart::of_hour(13), DayPart::Day);
        assert_eq!(DayPart::of_hour(20), DayPart::Evening);
        assert_eq!(DayPart::of_hour(23), DayPart::Night);
        let s = Settings {
            tracks_per_day: 1,
            max_index: 1,
            max_hops: 1,
            hop_decay: 0.5,
            utc_offset_min: 180,
            updated_at: Utc::now(),
        };
        let at = DateTime::parse_from_rfc3339("2026-11-11T22:30:00Z")
            .unwrap()
            .with_timezone(&Utc);
        assert_eq!(s.local_hour(at), 1, "Москва: 01:30");
    }
}
