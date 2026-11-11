//! Вкус слушателя: грани — k-means по звучанию любимого и дослушанного (свежее и часто дослушанное весит
//! больше, пропуски тянут вниз, «не нравится» исключает), поправки граней по времени суток и устройству,
//! подписи из стилей Discogs и артистов. Каждую ночь — пересчёт и «Миксы дня» (плейлист вида `mix` на
//! грань), раз в неделю — «Открытия недели»; лайк сдвигает ближайшую грань сразу.

use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use chrono::{DateTime, Utc};
use dalywave_protocol::{
    jobs::{TastePayload, kind},
    realtime::{LibraryChange, ServerEvent},
    sync::SyncEntity,
};
use serde::Serialize;
use serde_json::{Value, json};
use sqlx::PgPool;
use uuid::Uuid;

use super::{
    Cand, DayPart, Settings, candidates, nearest_sound,
    rank::{acquirable, resting},
    track_sound,
    vectors::{Rng, blend, cosine, drop_small, kmeans, merge_close, nearest},
    widen_search,
};
use crate::{
    jobs::{
        self, NewJob, lanes,
        worker::{Context, JobResult},
    },
    realtime::{self, Audience},
};

/// Треков в миксе дня и в «Открытиях недели».
pub const MIX_SIZE: usize = 30;
const MAX_FACETS: usize = 6;
/// Меньше — граней нет: не из чего собирать.
const MIN_TRACKS: usize = 3;
/// Грани ближе этого по звучанию (косинус центров) — одна грань.
const MERGE_COSINE: f32 = 0.93;
const EXAMPLES: usize = 8;
/// В миксе не больше стольких треков одного артиста; в открытиях — двух.
const MIX_PER_ARTIST: usize = 3;
const DISCOVERY_PER_ARTIST: usize = 2;
pub const DAILY_MIX: &str = "daily_mix";
pub const DISCOVER_WEEKLY: &str = "discover_weekly";

/// Трек, который слушатель любит или дослушивает, с весом.
#[derive(Debug, Clone)]
struct Listened {
    track_id: Uuid,
    weight: f32,
    sound: Vec<f32>,
    styles: Option<Value>,
    artists: Vec<Uuid>,
}

/// Вес трека во вкусе: лайк — 1 (за полгода свежесть падает до половины), дослушивания — по 0,35
/// с затуханием за два месяца (не больше шести), ранние пропуски — минус 0,25 с затуханием за месяц.
pub fn listened_weight(liked_age_days: Option<f32>, full: f32, skips: f32) -> f32 {
    let liked = liked_age_days.map_or(0.0, |age| 0.5 + 0.5 * (-age.max(0.0) / 180.0).exp());
    liked + 0.35 * full.min(6.0) - 0.25 * skips
}

async fn listened(db: &PgPool, user_id: Uuid) -> Result<Vec<Listened>, sqlx::Error> {
    let rows = sqlx::query!(
        r#"WITH plays AS (
               SELECT p.track_id,
                      sum(CASE WHEN p.end_reason = 'completed' OR p.played_ms >= 0.8 * p.duration_ms
                               THEN exp(-extract(epoch FROM now() - p.started_at) / 86400.0 / 60.0) ELSE 0 END)::real AS full,
                      sum(CASE WHEN p.end_reason = 'skipped' AND p.played_ms < 30000
                               THEN exp(-extract(epoch FROM now() - p.started_at) / 86400.0 / 30.0) ELSE 0 END)::real AS skips
               FROM play_events p
               WHERE p.user_id = $1 AND p.started_at > now() - interval '365 days'
               GROUP BY p.track_id
           ), liked AS (
               SELECT track_id, (extract(epoch FROM now() - created_at) / 86400.0)::real AS age
               FROM liked_tracks WHERE user_id = $1
           )
           SELECT t.id AS "track_id!", t.recording_id AS "recording_id!", l.age AS "liked_age?",
                  COALESCE(p.full, 0)::real AS "full!", COALESCE(p.skips, 0)::real AS "skips!",
                  a.sound::real[] AS "sound!", a.styles,
                  ARRAY(SELECT ta.artist_id FROM track_artists ta WHERE ta.track_id = t.id AND ta.role = 'main'
                        ORDER BY ta.position, ta.artist_id) AS "artists!"
           FROM tracks t
           JOIN track_analysis a ON a.recording_id = t.recording_id AND a.sound IS NOT NULL
           LEFT JOIN liked l ON l.track_id = t.id
           LEFT JOIN plays p ON p.track_id = t.id
           WHERE (l.track_id IS NOT NULL OR p.track_id IS NOT NULL)
             AND NOT COALESCE((SELECT f.kind = 'dislike' FROM feedback_events f
                               WHERE f.user_id = $1 AND f.track_id = t.id AND f.kind IN ('dislike', 'undislike')
                               ORDER BY f.at DESC LIMIT 1), false)
             AND NOT EXISTS (SELECT 1 FROM blocked_items b WHERE b.user_id = $1 AND b.kind = 'track' AND b.ref_id = t.id)"#,
        user_id,
    )
    .fetch_all(db)
    .await?;
    // Сингл и альбомная версия — одна запись: вес складывается
    let mut by_recording: HashMap<Uuid, Listened> = HashMap::new();
    for r in rows {
        let w = listened_weight(r.liked_age, r.full, r.skips);
        by_recording
            .entry(r.recording_id)
            .and_modify(|l| l.weight += w)
            .or_insert(Listened {
                track_id: r.track_id,
                weight: w,
                sound: r.sound,
                styles: r.styles,
                artists: r.artists,
            });
    }
    let mut out: Vec<Listened> = by_recording
        .into_values()
        .filter(|l| l.weight > 0.15)
        .collect();
    out.sort_by_key(|l| l.track_id);
    Ok(out)
}

/// Сколько граней: корень из половины числа треков, от 1 до 6, и не меньше трёх треков на грань.
pub fn facet_count(tracks: usize) -> usize {
    let k = ((tracks as f32 / 2.0).sqrt().round() as usize).clamp(1, MAX_FACETS);
    k.min((tracks / MIN_TRACKS).max(1))
}

/// Стиль Discogs без жанра: «Hip Hop---Trap» → «Trap».
fn style_name(raw: &str) -> &str {
    raw.rsplit("---").next().unwrap_or(raw).trim()
}

/// Подпись грани: главный стиль и два главных артиста — «Trap · Lowborn, Mzlff».
pub fn facet_label(style: Option<&str>, artists: &[String]) -> String {
    let names = artists
        .iter()
        .take(2)
        .cloned()
        .collect::<Vec<_>>()
        .join(", ");
    match (style, names.is_empty()) {
        (Some(s), false) => format!("{s} · {names}"),
        (Some(s), true) => s.to_string(),
        (None, false) => names,
        (None, true) => "Ваш вкус".into(),
    }
}

/// Грань в базе.
#[derive(Debug, Clone)]
pub struct Facet {
    pub id: Uuid,
    pub position: i16,
    pub label: String,
    pub centroid: Vec<f32>,
    pub weight: f32,
    pub tracks: i32,
    pub context: Value,
    pub examples: Vec<Uuid>,
    /// Главные артисты: id и имя.
    pub artists: Vec<(Uuid, String)>,
}

impl Facet {
    /// Поправка грани на время суток и устройство: больше 1 — её слушают тогда чаще.
    pub fn lift(&self, part: DayPart, platform: Option<&str>) -> f32 {
        let of = |path: &[&str]| {
            path.iter()
                .try_fold(&self.context, |v, k| v.get(*k))
                .and_then(Value::as_f64)
                .map_or(1.0, |x| x as f32)
        };
        let mut lift = of(&["timeOfDay", part.as_str()]);
        if let Some(p) = platform {
            lift *= of(&["platform", p]);
        }
        lift.clamp(0.5, 2.0)
    }
}

pub async fn facets(db: &PgPool, user_id: Uuid) -> Result<Vec<Facet>, sqlx::Error> {
    let rows = sqlx::query!(
        r#"SELECT f.id, f.position, f.label, f.centroid::real[] AS "centroid!", f.weight, f.tracks, f.context,
                  f.examples,
                  ARRAY(SELECT a.name FROM unnest(f.artists) WITH ORDINALITY AS x(id, ord)
                        JOIN artists a ON a.id = x.id ORDER BY x.ord) AS "names!",
                  ARRAY(SELECT a.id FROM unnest(f.artists) WITH ORDINALITY AS x(id, ord)
                        JOIN artists a ON a.id = x.id ORDER BY x.ord) AS "named!"
           FROM taste_facets f WHERE f.user_id = $1 ORDER BY f.position"#,
        user_id,
    )
    .fetch_all(db)
    .await?;
    Ok(rows
        .into_iter()
        .map(|r| Facet {
            id: r.id,
            position: r.position,
            label: r.label,
            centroid: r.centroid,
            weight: r.weight,
            tracks: r.tracks,
            context: r.context,
            examples: r.examples,
            artists: r.named.into_iter().zip(r.names).collect(),
        })
        .collect())
}

/// Поправки по времени суток и устройству: доля прослушиваний грани в этой части суток против доли
/// всех прослушиваний, со сглаживанием — у редких граней поправки близки к 1.
fn context(
    members: &HashSet<Uuid>,
    plays: &[(Uuid, DateTime<Utc>, Option<String>)],
    settings: &Settings,
) -> Value {
    let mut all: HashMap<String, f32> = HashMap::new();
    let mut mine: HashMap<String, f32> = HashMap::new();
    let mut all_platform: HashMap<String, f32> = HashMap::new();
    let mut mine_platform: HashMap<String, f32> = HashMap::new();
    for (track, at, platform) in plays {
        let part = DayPart::of_hour(settings.local_hour(*at))
            .as_str()
            .to_string();
        *all.entry(part.clone()).or_default() += 1.0;
        let platform = platform.clone().unwrap_or_else(|| "web".into());
        *all_platform.entry(platform.clone()).or_default() += 1.0;
        if members.contains(track) {
            *mine.entry(part).or_default() += 1.0;
            *mine_platform.entry(platform).or_default() += 1.0;
        }
    }
    let lifts = |all: &HashMap<String, f32>, mine: &HashMap<String, f32>, keys: Vec<String>| {
        let n = keys.len().max(1) as f32;
        let total: f32 = all.values().sum();
        let own: f32 = mine.values().sum();
        keys.into_iter()
            .map(|k| {
                let p_mine = (mine.get(&k).copied().unwrap_or(0.0) + 1.0) / (own + n);
                let p_all = (all.get(&k).copied().unwrap_or(0.0) + 1.0) / (total + n);
                let lift = (p_mine / p_all).clamp(0.5, 2.0);
                (k, json!((lift * 100.0).round() / 100.0))
            })
            .collect::<serde_json::Map<String, Value>>()
    };
    let parts: Vec<String> = DayPart::ALL
        .iter()
        .map(|p| p.as_str().to_string())
        .collect();
    let mut platforms: Vec<String> = all_platform.keys().cloned().collect();
    platforms.sort();
    json!({
        "timeOfDay": lifts(&all, &mine, parts),
        "platform": lifts(&all_platform, &mine_platform, platforms),
    })
}

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TasteReport {
    pub tracks: usize,
    pub facets: usize,
    pub mixes: usize,
    pub discovery: Option<usize>,
}

/// Полный пересчёт вкуса: грани, затем миксы дня и, если пора, «Открытия недели».
pub async fn rebuild(
    db: &PgPool,
    user_id: Uuid,
    settings: &Settings,
    refresh_discovery: bool,
) -> Result<TasteReport, sqlx::Error> {
    let items = listened(db, user_id).await?;
    let mut report = TasteReport {
        tracks: items.len(),
        ..Default::default()
    };
    if items.len() < MIN_TRACKS {
        // Не из чего собирать: старые грани остаются, пока не появятся данные
        return Ok(report);
    }
    let points: Vec<Vec<f32>> = items.iter().map(|l| l.sound.clone()).collect();
    let weights: Vec<f32> = items.iter().map(|l| l.weight).collect();
    let k = facet_count(items.len());
    let mut rng = Rng::from_uuid(user_id, 1);
    let mut clusters = kmeans(&points, &weights, k, 30, &mut rng);
    merge_close(&mut clusters, &points, &weights, MERGE_COSINE);
    drop_small(&mut clusters, &points, &weights, MIN_TRACKS);

    let plays: Vec<(Uuid, DateTime<Utc>, Option<String>)> = sqlx::query!(
        r#"SELECT p.track_id, p.started_at, d.platform AS "platform?"
           FROM play_events p LEFT JOIN devices d ON d.id = p.device_id
           WHERE p.user_id = $1 AND p.started_at > now() - interval '180 days'
             AND (p.end_reason = 'completed' OR p.played_ms >= 0.8 * p.duration_ms)"#,
        user_id,
    )
    .fetch_all(db)
    .await?
    .into_iter()
    .map(|r| (r.track_id, r.started_at, r.platform))
    .collect();

    let total: f32 = weights.iter().sum::<f32>().max(1e-6);
    struct Draft {
        centroid: Vec<f32>,
        weight: f32,
        tracks: i32,
        examples: Vec<Uuid>,
        artists: Vec<Uuid>,
        style: Option<String>,
        context: Value,
    }
    let mut drafts: Vec<Draft> = Vec::new();
    for (c, centroid) in clusters.centroids.iter().enumerate() {
        let members: Vec<usize> = (0..items.len())
            .filter(|i| clusters.assignment[*i] == c)
            .collect();
        if members.is_empty() {
            continue;
        }
        let weight: f32 = members.iter().map(|i| items[*i].weight).sum();
        let mut ranked = members.clone();
        ranked.sort_by(|a, b| {
            let score = |i: &usize| items[*i].weight * cosine(&items[*i].sound, centroid);
            score(b).total_cmp(&score(a))
        });
        let mut artists: HashMap<Uuid, f32> = HashMap::new();
        let mut styles: HashMap<String, f32> = HashMap::new();
        for i in &members {
            let l = &items[*i];
            if let Some(a) = l.artists.first() {
                *artists.entry(*a).or_default() += l.weight;
            }
            for s in l
                .styles
                .as_ref()
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                if let (Some(name), Some(p)) = (
                    s.get("name").and_then(Value::as_str),
                    s.get("p").and_then(Value::as_f64),
                ) {
                    *styles.entry(style_name(name).to_string()).or_default() += l.weight * p as f32;
                }
            }
        }
        let mut artists: Vec<(Uuid, f32)> = artists.into_iter().collect();
        artists.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
        let style = styles
            .into_iter()
            .max_by(|a, b| a.1.total_cmp(&b.1).then(b.0.cmp(&a.0)))
            .map(|(s, _)| s);
        let member_ids: HashSet<Uuid> = members.iter().map(|i| items[*i].track_id).collect();
        drafts.push(Draft {
            centroid: centroid.clone(),
            weight: weight / total,
            tracks: members.len() as i32,
            examples: ranked
                .iter()
                .take(EXAMPLES)
                .map(|i| items[*i].track_id)
                .collect(),
            artists: artists.into_iter().take(5).map(|(a, _)| a).collect(),
            style,
            context: context(&member_ids, &plays, settings),
        });
    }
    drafts.sort_by(|a, b| b.weight.total_cmp(&a.weight));

    let artist_ids: Vec<Uuid> = drafts.iter().flat_map(|d| d.artists.clone()).collect();
    let names: HashMap<Uuid, String> = sqlx::query!(
        "SELECT id, name FROM artists WHERE id = ANY($1)",
        &artist_ids
    )
    .fetch_all(db)
    .await?
    .into_iter()
    .map(|r| (r.id, r.name))
    .collect();

    let mut tx = db.begin().await?;
    sqlx::query!("DELETE FROM taste_facets WHERE user_id = $1", user_id)
        .execute(&mut *tx)
        .await?;
    for (position, d) in drafts.iter().enumerate() {
        let artist_names: Vec<String> = d
            .artists
            .iter()
            .filter_map(|a| names.get(a).cloned())
            .collect();
        let label = facet_label(d.style.as_deref(), &artist_names);
        sqlx::query!(
            "INSERT INTO taste_facets (user_id, position, label, centroid, weight, tracks, context, examples, artists)
             VALUES ($1, $2, $3, $4::real[]::halfvec(1280), $5, $6, $7, $8, $9)",
            user_id,
            position as i16,
            label,
            &d.centroid,
            d.weight,
            d.tracks,
            d.context,
            &d.examples,
            &d.artists,
        )
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    report.facets = drafts.len();

    let saved = facets(db, user_id).await?;
    report.mixes = refresh_daily_mixes(db, user_id, &saved, settings).await?;
    let due = refresh_discovery
        || sqlx::query_scalar!(
            r#"SELECT NOT EXISTS (SELECT 1 FROM playlists
                                  WHERE user_id = $1 AND kind = 'mix' AND rules ->> 'generator' = 'discover_weekly'
                                    AND updated_at > now() - interval '7 days') AS "due!""#,
            user_id,
        )
        .fetch_one(db)
        .await?;
    if due {
        report.discovery = Some(refresh_discovery_mix(db, user_id, &saved, settings).await?);
    }
    Ok(report)
}

/// Лайк сдвигает ближайшую грань к треку сразу, не дожидаясь ночного пересчёта. Граней ещё нет — пора
/// их собрать.
pub async fn on_like(db: &PgPool, user_id: Uuid, track_id: Uuid) -> Result<(), sqlx::Error> {
    let Some(v) = track_sound(db, track_id).await? else {
        return Ok(());
    };
    let current = facets(db, user_id).await?;
    if current.is_empty() {
        let mut conn = db.acquire().await?;
        request(&mut conn, user_id, false).await?;
        return Ok(());
    }
    let centroids: Vec<Vec<f32>> = current.iter().map(|f| f.centroid.clone()).collect();
    let f = &current[nearest(&v, &centroids)];
    let centroid = blend(Some(&f.centroid), f.tracks.max(1) as f32, &v, 1.0);
    let mut examples = vec![track_id];
    examples.extend(f.examples.iter().copied().filter(|e| *e != track_id));
    examples.truncate(EXAMPLES);
    sqlx::query!(
        "UPDATE taste_facets SET centroid = $2::real[]::halfvec(1280), tracks = tracks + 1, examples = $3,
                updated_at = now()
         WHERE id = $1",
        f.id,
        &centroid,
        &examples,
    )
    .execute(db)
    .await?;
    Ok(())
}

/// Ставит пересчёт вкуса слушателя.
pub async fn request(
    conn: &mut sqlx::PgConnection,
    user_id: Uuid,
    refresh_discovery: bool,
) -> Result<bool, sqlx::Error> {
    Ok(jobs::enqueue(
        conn,
        NewJob::new(
            kind::TASTE,
            lanes::MAINTENANCE,
            TastePayload {
                user_id,
                refresh_discovery,
            },
        )
        .dedupe(format!("taste:{user_id}")),
    )
    .await?
    .created)
}

/// Ночной пересчёт: ночью (3–6 по времени слушателя) — тем, кого не пересчитывали 20 часов, днём — тем,
/// кого не пересчитывали полтора дня (сервер спал ночью). Граней ещё нет — каждый час: как только
/// разобрано достаточно любимого, они появятся.
pub async fn schedule(db: &PgPool, settings: &Settings) -> Result<usize, sqlx::Error> {
    let hour = settings.local_hour(Utc::now());
    let hours: i32 = if (3..6).contains(&hour) { 20 } else { 36 };
    let users = sqlx::query_scalar!(
        "SELECT u.id FROM users u
         WHERE NOT u.disabled
           AND (EXISTS (SELECT 1 FROM liked_tracks l WHERE l.user_id = u.id)
                OR EXISTS (SELECT 1 FROM play_events p WHERE p.user_id = u.id))
           AND NOT EXISTS (SELECT 1 FROM jobs j WHERE j.kind = 'taste' AND j.payload ->> 'userId' = u.id::text
                             AND (j.status IN ('queued', 'running')
                                  OR (j.status = 'done' AND j.finished_at > now() - make_interval(hours =>
                                      CASE WHEN EXISTS (SELECT 1 FROM taste_facets f WHERE f.user_id = u.id)
                                           THEN $1 ELSE 1 END))))",
        hours,
    )
    .fetch_all(db)
    .await?;
    let mut created = 0;
    let mut conn = db.acquire().await?;
    for user in users {
        created += usize::from(request(&mut conn, user, false).await?);
    }
    Ok(created)
}

/// Задача `taste`.
pub async fn run(ctx: Arc<Context>, job: jobs::JobRow) -> JobResult {
    let p: TastePayload = job.payload()?;
    let settings = Settings::load(&ctx.db).await?;
    let report = rebuild(&ctx.db, p.user_id, &settings, p.refresh_discovery).await?;
    Ok(serde_json::to_value(report)?.into())
}

// ───────────── Миксы ─────────────

/// Подходит ли трек в микс: не «не рекомендовать» и не «не нравится», можно сыграть, не заслушан.
fn mixable(c: &Cand, now: DateTime<Utc>) -> bool {
    !c.excluded && acquirable(c) && !resting(c, dalywave_protocol::wave::WaveCharacter::Mix, now)
}

/// Не больше `per_artist` треков артиста, без повторов записей.
fn take_diverse<'a>(
    pool: impl Iterator<Item = &'a Cand>,
    per_artist: usize,
    limit: usize,
    used_recordings: &mut HashSet<Uuid>,
    artist_count: &mut HashMap<Uuid, usize>,
) -> Vec<Uuid> {
    let mut out = Vec::new();
    for c in pool {
        if out.len() >= limit {
            break;
        }
        let key = c.recording_id.unwrap_or(c.track_id);
        let artist = c.artist_ids.first().copied().unwrap_or(c.track_id);
        if used_recordings.contains(&key)
            || artist_count.get(&artist).copied().unwrap_or(0) >= per_artist
        {
            continue;
        }
        used_recordings.insert(key);
        *artist_count.entry(artist).or_default() += 1;
        out.push(c.track_id);
    }
    out
}

/// Соседи центра грани с данными для отбора, ближние первыми.
async fn around(
    db: &PgPool,
    user_id: Uuid,
    centroid: &[f32],
    limit: i64,
) -> Result<Vec<(Cand, f32)>, sqlx::Error> {
    let mut tx = db.begin().await?;
    widen_search(&mut tx, limit).await?;
    let near = nearest_sound(&mut tx, centroid, limit, user_id).await?;
    tx.commit().await?;
    let ids: Vec<Uuid> = near.iter().map(|n| n.track_id).collect();
    let sims: HashMap<Uuid, f32> = near.iter().map(|n| (n.track_id, n.similarity)).collect();
    Ok(candidates(db, &ids, user_id, None, None)
        .await?
        .into_iter()
        .map(|c| {
            let sim = sims.get(&c.track_id).copied().unwrap_or(0.0);
            (c, sim)
        })
        .collect())
}

/// Микс грани: знакомое и новое через одно, по близости к центру грани.
async fn daily_mix(db: &PgPool, user_id: Uuid, facet: &Facet) -> Result<Vec<Uuid>, sqlx::Error> {
    let now = Utc::now();
    let pool: Vec<(Cand, f32)> = around(db, user_id, &facet.centroid, 300)
        .await?
        .into_iter()
        .filter(|(c, _)| mixable(c, now))
        .collect();
    let familiar: Vec<&Cand> = pool
        .iter()
        .filter(|(c, _)| c.familiar())
        .map(|(c, _)| c)
        .collect();
    let fresh: Vec<&Cand> = pool
        .iter()
        .filter(|(c, _)| !c.familiar())
        .map(|(c, _)| c)
        .collect();
    let mut used = HashSet::new();
    let mut counts = HashMap::new();
    let half = MIX_SIZE / 2;
    let known = take_diverse(
        familiar.iter().copied(),
        MIX_PER_ARTIST,
        half,
        &mut used,
        &mut counts,
    );
    let new = take_diverse(
        fresh.iter().copied(),
        MIX_PER_ARTIST,
        MIX_SIZE - known.len(),
        &mut used,
        &mut counts,
    );
    // Не хватило нового — добираем знакомым
    let more = take_diverse(
        familiar.iter().copied(),
        MIX_PER_ARTIST,
        MIX_SIZE.saturating_sub(known.len() + new.len()),
        &mut used,
        &mut counts,
    );
    let mut out = Vec::with_capacity(MIX_SIZE);
    let (mut a, mut b) = (known.into_iter().chain(more), new.into_iter());
    loop {
        match (a.next(), b.next()) {
            (None, None) => break,
            (x, y) => out.extend(x.into_iter().chain(y)),
        }
    }
    Ok(out)
}

/// Записывает сгенерированный плейлист вида `mix`: тот же генератор и номер — тот же плейлист,
/// новые треки. Возвращает id плейлиста.
async fn save_mix(
    db: &PgPool,
    user_id: Uuid,
    generator: &str,
    slot: i32,
    title: &str,
    description: &str,
    tracks: &[Uuid],
) -> Result<Uuid, sqlx::Error> {
    let rules = json!({ "generator": generator, "slot": slot, "builtAt": Utc::now() });
    let mut tx = db.begin().await?;
    let existing = sqlx::query_scalar!(
        "SELECT id FROM playlists
         WHERE user_id = $1 AND kind = 'mix' AND rules ->> 'generator' = $2 AND (rules ->> 'slot')::int = $3
         ORDER BY created_at LIMIT 1 FOR UPDATE",
        user_id,
        generator,
        slot,
    )
    .fetch_optional(&mut *tx)
    .await?;
    let id = match existing {
        Some(id) => {
            sqlx::query!(
                "UPDATE playlists SET title = $2, description = $3, rules = $4, updated_at = now() WHERE id = $1",
                id,
                title,
                description,
                rules,
            )
            .execute(&mut *tx)
            .await?;
            sqlx::query!("DELETE FROM playlist_items WHERE playlist_id = $1", id)
                .execute(&mut *tx)
                .await?;
            id
        }
        None => {
            sqlx::query_scalar!(
                "INSERT INTO playlists (user_id, title, description, kind, rules) VALUES ($1, $2, $3, 'mix', $4)
                 RETURNING id",
                user_id,
                title,
                description,
                rules,
            )
            .fetch_one(&mut *tx)
            .await?
        }
    };
    sqlx::query!(
        "INSERT INTO playlist_items (playlist_id, track_id, position)
         SELECT $1, t, p::double precision FROM unnest($2::uuid[]) WITH ORDINALITY AS x(t, p)",
        id,
        tracks,
    )
    .execute(&mut *tx)
    .await?;
    changed(&mut tx, user_id).await;
    tx.commit().await?;
    Ok(id)
}

async fn changed(conn: &mut sqlx::PgConnection, user_id: Uuid) {
    realtime::publish(
        conn,
        Audience::User(user_id),
        ServerEvent::LibraryChanged(LibraryChange {
            entities: vec![SyncEntity::Playlists, SyncEntity::PlaylistItems],
        }),
    )
    .await;
}

fn artists_line(names: &[String], tail: &str) -> String {
    let names = names.iter().take(3).cloned().collect::<Vec<_>>().join(", ");
    if names.is_empty() {
        tail.to_string()
    } else {
        format!("{names} {tail}")
    }
}

/// «Миксы дня»: по плейлисту на грань, «Микс дня 1» — самая весомая. Лишние (граней стало меньше)
/// удаляются.
pub async fn refresh_daily_mixes(
    db: &PgPool,
    user_id: Uuid,
    facets: &[Facet],
    _settings: &Settings,
) -> Result<usize, sqlx::Error> {
    let mut made = 0;
    for f in facets {
        let tracks = daily_mix(db, user_id, f).await?;
        if tracks.len() < 5 {
            continue;
        }
        made += 1;
        let names: Vec<String> = f.artists.iter().map(|(_, n)| n.clone()).collect();
        save_mix(
            db,
            user_id,
            DAILY_MIX,
            made,
            &format!("Микс дня {made}"),
            &artists_line(&names, "и другие"),
            &tracks,
        )
        .await?;
    }
    let mut tx = db.begin().await?;
    let removed = sqlx::query!(
        "DELETE FROM playlists WHERE user_id = $1 AND kind = 'mix' AND rules ->> 'generator' = 'daily_mix'
           AND (rules ->> 'slot')::int > $2",
        user_id,
        made,
    )
    .execute(&mut *tx)
    .await?;
    if removed.rows_affected() > 0 {
        changed(&mut tx, user_id).await;
    }
    tx.commit().await?;
    Ok(made as usize)
}

/// «Открытия недели»: незнакомые треки индекса у граней вкуса — по доле каждой грани, не больше двух
/// у артиста, артисты, которых человек ещё не слушал, — первыми.
pub async fn refresh_discovery_mix(
    db: &PgPool,
    user_id: Uuid,
    facets: &[Facet],
    _settings: &Settings,
) -> Result<usize, sqlx::Error> {
    if facets.is_empty() {
        return Ok(0);
    }
    let now = Utc::now();
    let mut used = HashSet::new();
    let mut counts = HashMap::new();
    let mut picked: Vec<Uuid> = Vec::new();
    let mut new_artists: Vec<String> = Vec::new();
    for f in facets {
        let quota = ((MIX_SIZE as f32 * f.weight).round() as usize).max(2);
        let mut pool: Vec<(Cand, f32)> = around(db, user_id, &f.centroid, 250)
            .await?
            .into_iter()
            .filter(|(c, _)| !c.familiar() && c.plays_30d == 0 && mixable(c, now))
            .collect();
        pool.sort_by(|a, b| {
            let score = |x: &(Cand, f32)| x.1 + if x.0.artist_familiar { 0.0 } else { 0.15 };
            score(b).total_cmp(&score(a))
        });
        let take = take_diverse(
            pool.iter().map(|(c, _)| c),
            DISCOVERY_PER_ARTIST,
            quota.min(MIX_SIZE.saturating_sub(picked.len())),
            &mut used,
            &mut counts,
        );
        for id in &take {
            if let Some((c, _)) = pool.iter().find(|(c, _)| c.track_id == *id)
                && !c.artist_familiar
                && let Some((_, name)) = c.main_artist()
                && !new_artists.iter().any(|n| n == name)
            {
                new_artists.push(name.to_string());
            }
        }
        picked.extend(take);
        if picked.len() >= MIX_SIZE {
            break;
        }
    }
    if picked.len() < 5 {
        return Ok(0);
    }
    save_mix(
        db,
        user_id,
        DISCOVER_WEEKLY,
        1,
        "Открытия недели",
        &artists_line(&new_artists, "и другие новые для тебя"),
        &picked,
    )
    .await?;
    Ok(picked.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weights_and_counts() {
        assert!((listened_weight(Some(0.0), 0.0, 0.0) - 1.0).abs() < 1e-6);
        assert!(listened_weight(Some(365.0), 0.0, 0.0) < 0.6);
        assert!(
            listened_weight(None, 3.0, 0.0) > 1.0,
            "часто дослушанное весит как лайк"
        );
        assert!(listened_weight(None, 0.0, 2.0) < 0.0, "пропуски тянут вниз");
        assert_eq!(listened_weight(None, 100.0, 0.0), 0.35 * 6.0);
        assert_eq!(facet_count(3), 1);
        assert_eq!(facet_count(8), 2);
        assert_eq!(facet_count(50), 5);
        assert_eq!(facet_count(1000), MAX_FACETS);
        assert_eq!(style_name("Hip Hop---Trap"), "Trap");
        assert_eq!(style_name("Phonk"), "Phonk");
        assert_eq!(
            facet_label(
                Some("Trap"),
                &["Lowborn".into(), "Mzlff".into(), "5opka".into()]
            ),
            "Trap · Lowborn, Mzlff"
        );
        assert_eq!(facet_label(None, &["Lowborn".into()]), "Lowborn");
        assert_eq!(facet_label(None, &[]), "Ваш вкус");
    }

    #[test]
    fn context_lifts_follow_listening_time() {
        let settings = Settings {
            tracks_per_day: 1,
            max_index: 1,
            max_hops: 1,
            hop_decay: 0.5,
            utc_offset_min: 0,
            updated_at: Utc::now(),
        };
        let night = DateTime::parse_from_rfc3339("2026-11-10T02:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let day = DateTime::parse_from_rfc3339("2026-11-10T14:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let a = Uuid::from_u128(1);
        let b = Uuid::from_u128(2);
        let mut plays = Vec::new();
        for _ in 0..10 {
            plays.push((a, night, Some("macos".into())));
            plays.push((b, day, Some("ios".into())));
        }
        let ctx = context(&HashSet::from([a]), &plays, &settings);
        assert!(ctx["timeOfDay"]["night"].as_f64().unwrap() > 1.0);
        assert!(ctx["timeOfDay"]["day"].as_f64().unwrap() < 1.0);
        assert!(ctx["platform"]["macos"].as_f64().unwrap() > 1.0);
        let facet = Facet {
            id: Uuid::nil(),
            position: 0,
            label: String::new(),
            centroid: vec![],
            weight: 1.0,
            tracks: 1,
            context: ctx,
            examples: vec![],
            artists: vec![],
        };
        assert!(facet.lift(DayPart::Night, Some("macos")) > facet.lift(DayPart::Day, Some("ios")));
        assert_eq!(
            Facet {
                context: json!({}),
                ..facet
            }
            .lift(DayPart::Night, None),
            1.0
        );
    }
}
