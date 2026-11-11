//! «Разведчик»: растит и держит индекс каталога вокруг вкуса слушателей.
//!
//! Семена — артисты из лайков, подписок и прослушиваний; от них — похожие Deezer на 2–3 шага с падающим
//! весом (`scout_artists`). У артистов окрестности — популярное, свежие релизы и радио, у любимых жанров —
//! чарты и новинки редакции (`scout_tracks`). Самые близкие к вкусу треки уходят на анализ превью
//! в пределах суточного бюджета; индекс не растёт выше предела — дальние превью вытесняются. Медиатека,
//! которой ещё нет на сервере, анализируется по превью первой: по ней строятся грани вкуса.

use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use dalywave_protocol::jobs::{ScoutArtistPayload, kind};
use serde_json::json;
use sqlx::{PgConnection, PgPool};
use uuid::Uuid;

use super::{Settings, analysis, retry_deadlocks};
use crate::{
    catalog::{self, ArtistParts},
    deezer::DeezerError,
    jobs::{
        self, JobRow, NewJob, lanes,
        worker::{Context, JobError, JobResult},
    },
};

/// Проход разведчика — раньше всего остального в полосе.
pub const PLAN_PRIORITY: i16 = 40;
/// Разведка артиста дешёвая и открывает кандидатов: раньше анализа превью.
const ARTIST_PRIORITY: i16 = 30;
/// Превью медиатеки — раньше находок: без них нет граней вкуса, пока загрузчик не скачал любимое.
const LIBRARY_PRIORITY: i16 = 20;
const LIBRARY_PER_RUN: i64 = 500;
/// Сколько артистов держит окрестность: дальше — шум и лишние запросы к Deezer.
const MAX_ARTISTS: usize = 600;
/// Сколько артистов разведывать за проход: по 4–6 запросов к Deezer на каждого.
const ARTISTS_PER_RUN: i64 = 40;
const RADIO_LIMIT: u32 = 40;
/// Свежие релизы — за полгода, не больше трёх на артиста.
const RECENT_RELEASES: i64 = 3;
/// Проход отдаёт на анализ не больше шестой части суточного бюджета: анализ размазан по суткам.
const SHARE_PER_RUN: i32 = 6;
const CHART_LIMIT: u32 = 50;
const EDITORIAL_LIMIT: u32 = 20;
const EDITORIAL_ALBUMS: usize = 5;
const GENRES: i64 = 4;
const NIL: Uuid = Uuid::nil();

/// Артист окрестности.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Node {
    pub weight: f32,
    pub hops: i16,
    pub via: Option<Uuid>,
}

/// Семена окрестности: артисты, которых слушают. Подписка — 1; лайки — 0,25 + 0,15 за каждый;
/// дослушанное за полгода — 0,1 за прослушивание с затуханием (полтора месяца). Не больше 1.
pub async fn seeds(db: &PgPool) -> Result<Vec<(Uuid, f32)>, sqlx::Error> {
    let rows = sqlx::query!(
        r#"WITH liked AS (
               SELECT ta.artist_id, count(*)::real AS n
               FROM liked_tracks l JOIN track_artists ta ON ta.track_id = l.track_id AND ta.role = 'main'
               GROUP BY ta.artist_id
           ), played AS (
               SELECT ta.artist_id,
                      sum(exp(-extract(epoch FROM now() - p.started_at) / 86400.0 / 45.0))::real AS n
               FROM play_events p JOIN track_artists ta ON ta.track_id = p.track_id AND ta.role = 'main'
               WHERE p.started_at > now() - interval '180 days'
                 AND (p.end_reason = 'completed' OR p.played_ms >= 0.8 * p.duration_ms)
               GROUP BY ta.artist_id
           ), followed AS (
               SELECT DISTINCT artist_id FROM followed_artists
           )
           SELECT a.id AS "artist_id!",
                  LEAST(1.0, CASE WHEN f.artist_id IS NOT NULL THEN 1.0 ELSE 0.0 END
                             + COALESCE(0.25 + 0.15 * l.n, 0.0) + COALESCE(0.1 * p.n, 0.0))::real AS "weight!"
           FROM artists a
           LEFT JOIN liked l ON l.artist_id = a.id
           LEFT JOIN played p ON p.artist_id = a.id
           LEFT JOIN followed f ON f.artist_id = a.id
           WHERE (l.artist_id IS NOT NULL OR p.artist_id IS NOT NULL OR f.artist_id IS NOT NULL)
           ORDER BY 2 DESC, a.id"#,
    )
    .fetch_all(db)
    .await?;
    Ok(rows.into_iter().map(|r| (r.artist_id, r.weight)).collect())
}

/// Артисты, которых кто-то не хочет слышать: «не рекомендовать» или последняя реакция — «не нравится».
async fn unwanted(db: &PgPool) -> Result<HashSet<Uuid>, sqlx::Error> {
    let ids = sqlx::query_scalar!(
        r#"SELECT ref_id AS "id!" FROM blocked_items WHERE kind = 'artist'
           UNION
           SELECT f.artist_id AS "id!" FROM feedback_events f
           WHERE f.artist_id IS NOT NULL AND f.track_id IS NULL AND f.kind = 'dislike'
             AND NOT EXISTS (SELECT 1 FROM feedback_events g
                             WHERE g.user_id = f.user_id AND g.artist_id = f.artist_id AND g.track_id IS NULL
                               AND g.kind = 'undislike' AND g.at > f.at)"#,
    )
    .fetch_all(db)
    .await?;
    Ok(ids.into_iter().collect())
}

/// Шаг по похожим: вес похожего — вес артиста × затухание × место в списке Deezer (первые ближе).
/// Возвращает тех, чей вес вырос на этом шаге, — следующий фронт.
pub fn spread(
    nodes: &mut HashMap<Uuid, Node>,
    edges: &[(Uuid, i16, Uuid)],
    hop: i16,
    decay: f32,
    unwanted: &HashSet<Uuid>,
) -> Vec<Uuid> {
    let mut grown: Vec<Uuid> = Vec::new();
    for (from, position, to) in edges {
        if unwanted.contains(to) {
            continue;
        }
        let Some(parent) = nodes.get(from).copied() else {
            continue;
        };
        let closeness = (1.0 - f32::from(*position) / 40.0).max(0.4);
        let weight = parent.weight * decay * closeness;
        let better = nodes.get(to).is_none_or(|n| weight > n.weight);
        if better {
            nodes.insert(
                *to,
                Node {
                    weight,
                    hops: hop,
                    via: Some(*from),
                },
            );
            if !grown.contains(to) {
                grown.push(*to);
            }
        }
    }
    grown
}

/// Окрестность вкуса из семян и сохранённых похожих (`artist_related`).
pub async fn neighbourhood(
    db: &PgPool,
    settings: &Settings,
) -> Result<HashMap<Uuid, Node>, sqlx::Error> {
    let unwanted = unwanted(db).await?;
    let mut nodes: HashMap<Uuid, Node> = seeds(db)
        .await?
        .into_iter()
        .filter(|(id, _)| !unwanted.contains(id))
        .map(|(id, weight)| {
            (
                id,
                Node {
                    weight,
                    hops: 0,
                    via: None,
                },
            )
        })
        .collect();
    let mut frontier: Vec<Uuid> = nodes.keys().copied().collect();
    for hop in 1..=settings.max_hops {
        if frontier.is_empty() {
            break;
        }
        let edges: Vec<(Uuid, i16, Uuid)> = sqlx::query!(
            "SELECT artist_id, position, related_id FROM artist_related WHERE artist_id = ANY($1)
             ORDER BY artist_id, position",
            &frontier,
        )
        .fetch_all(db)
        .await?
        .into_iter()
        .map(|r| (r.artist_id, r.position, r.related_id))
        .collect();
        frontier = spread(&mut nodes, &edges, hop, settings.hop_decay, &unwanted);
    }
    // Ближние — все семена и самые весомые из найденных
    if nodes.len() > MAX_ARTISTS {
        let mut all: Vec<(Uuid, Node)> = nodes.into_iter().collect();
        all.sort_by(|a, b| {
            (a.1.hops > 0)
                .cmp(&(b.1.hops > 0))
                .then(b.1.weight.total_cmp(&a.1.weight))
        });
        all.truncate(MAX_ARTISTS);
        nodes = all.into_iter().collect();
    }
    Ok(nodes)
}

async fn save_neighbourhood(db: &PgPool, nodes: &HashMap<Uuid, Node>) -> Result<(), sqlx::Error> {
    let ids: Vec<Uuid> = nodes.keys().copied().collect();
    let weights: Vec<f32> = ids.iter().map(|id| nodes[id].weight).collect();
    let hops: Vec<i16> = ids.iter().map(|id| nodes[id].hops).collect();
    let via: Vec<Uuid> = ids.iter().map(|id| nodes[id].via.unwrap_or(NIL)).collect();
    let mut tx = db.begin().await?;
    sqlx::query!(
        "DELETE FROM scout_artists WHERE NOT (artist_id = ANY($1))",
        &ids
    )
    .execute(&mut *tx)
    .await?;
    sqlx::query!(
        "INSERT INTO scout_artists (artist_id, weight, hops, via)
         SELECT a, w, h, NULLIF(v, '00000000-0000-0000-0000-000000000000'::uuid)
         FROM unnest($1::uuid[], $2::real[], $3::smallint[], $4::uuid[]) AS x(a, w, h, v)
         ON CONFLICT (artist_id) DO UPDATE SET
            weight = EXCLUDED.weight, hops = EXCLUDED.hops, via = EXCLUDED.via, updated_at = now()",
        &ids,
        &weights,
        &hops,
        &via,
    )
    .execute(&mut *tx)
    .await?;
    tx.commit().await
}

/// Кандидат в индекс: трек, по какому артисту найден, вид находки и базовая близость 0..1.
type Found = (Uuid, Option<Uuid>, &'static str, f32);

/// Добавляет находки; трек, найденный снова, сохраняет лучшую из них.
async fn add_tracks(db: &PgPool, found: Vec<Found>) -> Result<usize, sqlx::Error> {
    let mut best: HashMap<Uuid, Found> = HashMap::new();
    for f in found {
        if best.get(&f.0).is_none_or(|b| f.3 > b.3) {
            best.insert(f.0, f);
        }
    }
    let items: Vec<Found> = best.into_values().collect();
    let tracks: Vec<Uuid> = items.iter().map(|f| f.0).collect();
    let artists: Vec<Uuid> = items.iter().map(|f| f.1.unwrap_or(NIL)).collect();
    let sources: Vec<String> = items.iter().map(|f| f.2.to_string()).collect();
    let bases: Vec<f32> = items.iter().map(|f| f.3.clamp(0.0, 1.0)).collect();
    sqlx::query!(
        "INSERT INTO scout_tracks (track_id, artist_id, source, base)
         SELECT t, NULLIF(a, '00000000-0000-0000-0000-000000000000'::uuid), s, b
         FROM unnest($1::uuid[], $2::uuid[], $3::text[], $4::real[]) AS x(t, a, s, b)
         ON CONFLICT (track_id) DO UPDATE SET
            source = CASE WHEN EXCLUDED.base > scout_tracks.base THEN EXCLUDED.source ELSE scout_tracks.source END,
            artist_id = CASE WHEN EXCLUDED.base > scout_tracks.base THEN EXCLUDED.artist_id ELSE scout_tracks.artist_id END,
            base = GREATEST(scout_tracks.base, EXCLUDED.base)",
        &tracks,
        &artists,
        &sources,
        &bases,
    )
    .execute(db)
    .await?;
    Ok(items.len())
}

/// Запись разобрана: её треки больше не ждут анализа превью.
pub async fn mark_analyzed(conn: &mut PgConnection, recording_id: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query!(
        "UPDATE scout_tracks SET analyzed_at = now(), skipped = NULL
         WHERE track_id IN (SELECT id FROM tracks WHERE recording_id = $1)",
        recording_id,
    )
    .execute(conn)
    .await?;
    Ok(())
}

/// Трек больше не ждёт анализа: разобран (`None`) или пропущен по причине (нет превью, нет в Deezer).
pub async fn mark_track(
    db: &PgPool,
    track_id: Uuid,
    skipped: Option<&str>,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        "UPDATE scout_tracks SET analyzed_at = CASE WHEN $2::text IS NULL THEN now() ELSE analyzed_at END,
                skipped = $2
         WHERE track_id = $1",
        track_id,
        skipped,
    )
    .execute(db)
    .await?;
    Ok(())
}

/// Ставит на анализ превью самые близкие к вкусу неразобранные находки — в пределах суточного бюджета.
/// Индекс полон — только те, что ближе самых дальних в нём (их потом вытеснит [`evict`]).
///
/// Близость находки к вкусу — base × вес артиста в окрестности, у чартов и подборок жанров — только base;
/// тот же расчёт — в вытеснении. Артист выпал из окрестности — его находки не ставятся и уходят первыми.
pub async fn enqueue_previews(db: &PgPool, settings: &Settings) -> Result<usize, sqlx::Error> {
    let used = sqlx::query_scalar!(
        r#"SELECT count(*) AS "n!" FROM jobs
           WHERE kind = 'analyze_preview' AND status <> 'cancelled' AND created_at > now() - interval '24 hours'"#
    )
    .fetch_one(db)
    .await?;
    let per_run = i64::from((settings.tracks_per_day / SHARE_PER_RUN).max(1));
    let budget = (i64::from(settings.tracks_per_day) - used).clamp(0, per_run);
    if budget == 0 {
        return Ok(0);
    }
    let size = index_size(db).await?;
    let floor = if size >= i64::from(settings.max_index) {
        sqlx::query_scalar!(
            r#"SELECT COALESCE(percentile_cont(0.1) WITHIN GROUP (
                   ORDER BY st.base * COALESCE(sa.weight, CASE WHEN st.source IN ('chart', 'editorial') THEN 1 ELSE 0 END)
               ), 0)::real AS "floor!"
               FROM scout_tracks st LEFT JOIN scout_artists sa ON sa.artist_id = st.artist_id
               WHERE st.analyzed_at IS NOT NULL"#
        )
        .fetch_one(db)
        .await?
    } else {
        0.0
    };
    let picks = sqlx::query!(
        r#"SELECT st.track_id AS "track_id!",
                  (st.base * COALESCE(sa.weight, CASE WHEN st.source IN ('chart', 'editorial') THEN 1 ELSE 0 END))::real
                      AS "closeness!"
           FROM scout_tracks st
           LEFT JOIN scout_artists sa ON sa.artist_id = st.artist_id
           JOIN tracks t ON t.id = st.track_id
           WHERE st.analyzed_at IS NULL AND st.skipped IS NULL
             AND (st.queued_at IS NULL OR st.queued_at < now() - interval '3 days')
             AND t.deezer_id IS NOT NULL
             AND NOT EXISTS (SELECT 1 FROM track_analysis a WHERE a.recording_id = t.recording_id)
           ORDER BY 2 DESC, st.track_id
           LIMIT $1"#,
        budget,
    )
    .fetch_all(db)
    .await?;
    let mut queued = Vec::new();
    let mut conn = db.acquire().await?;
    for p in picks {
        if p.closeness <= floor.max(0.01) {
            continue;
        }
        let priority = (p.closeness * 10.0).round().clamp(0.0, 10.0) as i16;
        analysis::request_preview(&mut conn, p.track_id, priority).await?;
        queued.push(p.track_id);
    }
    sqlx::query!(
        "UPDATE scout_tracks SET queued_at = now() WHERE track_id = ANY($1)",
        &queued
    )
    .execute(&mut *conn)
    .await?;
    Ok(queued.len())
}

/// Превью медиатеки — лайков, прослушанного и своих плейлистов, — которой ещё нет на сервере: грани вкуса
/// строятся по звучанию любимого, а скачанное ждёт загрузчик. Вне бюджета разведки: медиатека невелика.
pub async fn enqueue_library(db: &PgPool) -> Result<usize, sqlx::Error> {
    let tracks = sqlx::query_scalar!(
        r#"SELECT t.id FROM tracks t
           WHERE t.deezer_id IS NOT NULL
             AND (t.id IN (SELECT track_id FROM liked_tracks)
                  OR t.id IN (SELECT track_id FROM play_events)
                  OR t.id IN (SELECT i.track_id FROM playlist_items i JOIN playlists p ON p.id = i.playlist_id
                              WHERE p.kind <> 'mix'))
             AND NOT EXISTS (SELECT 1 FROM track_analysis a WHERE a.recording_id = t.recording_id)
             AND NOT EXISTS (SELECT 1 FROM jobs j WHERE j.dedupe_key = 'analyze_preview:' || t.id
                               AND (j.status IN ('queued', 'running')
                                    OR (j.status IN ('done', 'failed') AND j.finished_at > now() - interval '3 days')))
           ORDER BY t.id
           LIMIT $1"#,
        LIBRARY_PER_RUN,
    )
    .fetch_all(db)
    .await?;
    let mut conn = db.acquire().await?;
    let mut queued = 0;
    for id in tracks {
        queued += usize::from(
            analysis::request_preview(&mut conn, id, LIBRARY_PRIORITY)
                .await?
                .created,
        );
    }
    Ok(queued)
}

pub async fn index_size(db: &PgPool) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar!(r#"SELECT count(*) AS "n!" FROM track_analysis WHERE source = 'preview'"#)
        .fetch_one(db)
        .await
}

/// Предел индекса: лишние анализы превью — самые далёкие от вкуса — удаляются. Треки медиатеки
/// (лайки, плейлисты, в том числе миксы, и прослушанное) не трогаются.
pub async fn evict(db: &PgPool, max_index: i32) -> Result<u64, sqlx::Error> {
    let over = index_size(db).await? - i64::from(max_index);
    if over <= 0 {
        return Ok(0);
    }
    let mut tx = db.begin().await?;
    let gone = sqlx::query_scalar!(
        r#"WITH victims AS (
               SELECT a.recording_id
               FROM track_analysis a
               LEFT JOIN LATERAL (
                   SELECT max(st.base * COALESCE(sa.weight, CASE WHEN st.source IN ('chart', 'editorial') THEN 1 ELSE 0 END))
                          AS closeness
                   FROM tracks t JOIN scout_tracks st ON st.track_id = t.id
                   LEFT JOIN scout_artists sa ON sa.artist_id = st.artist_id
                   WHERE t.recording_id = a.recording_id
               ) c ON true
               WHERE a.source = 'preview'
                 AND NOT EXISTS (SELECT 1 FROM tracks t WHERE t.recording_id = a.recording_id
                                   AND (EXISTS (SELECT 1 FROM liked_tracks l WHERE l.track_id = t.id)
                                        OR EXISTS (SELECT 1 FROM playlist_items i WHERE i.track_id = t.id)
                                        OR EXISTS (SELECT 1 FROM play_events p WHERE p.track_id = t.id)))
               ORDER BY c.closeness ASC NULLS FIRST, a.analyzed_at
               LIMIT $1
           )
           DELETE FROM track_analysis WHERE recording_id IN (SELECT recording_id FROM victims)
           RETURNING recording_id"#,
        over,
    )
    .fetch_all(&mut *tx)
    .await?;
    sqlx::query!(
        "UPDATE scout_tracks SET skipped = 'evicted', analyzed_at = NULL
         WHERE track_id IN (SELECT id FROM tracks WHERE recording_id = ANY($1))",
        &gone,
    )
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    if !gone.is_empty() {
        tracing::info!(evicted = gone.len(), max_index, "catalog index trimmed");
    }
    Ok(gone.len() as u64)
}

/// Задача `scout`: окрестность вкуса, разведка артистов, чарты раз в сутки, анализ превью в пределах
/// бюджета и предел индекса.
pub async fn plan(ctx: Arc<Context>, _job: JobRow) -> JobResult {
    let settings = Settings::load(&ctx.db).await?;
    let nodes = neighbourhood(&ctx.db, &settings).await?;
    save_neighbourhood(&ctx.db, &nodes).await?;
    let stale = sqlx::query_scalar!(
        "SELECT s.artist_id FROM scout_artists s JOIN artists a ON a.id = s.artist_id
         WHERE a.deezer_id IS NOT NULL AND (s.scouted_at IS NULL OR s.scouted_at < now() - interval '7 days')
         ORDER BY s.weight DESC, s.artist_id
         LIMIT $1",
        ARTISTS_PER_RUN,
    )
    .fetch_all(&ctx.db)
    .await?;
    let mut conn = ctx.db.acquire().await?;
    let mut expanding = 0;
    for artist_id in &stale {
        let job = jobs::enqueue(
            &mut conn,
            NewJob::new(
                kind::SCOUT_ARTIST,
                lanes::SCOUT,
                ScoutArtistPayload {
                    artist_id: *artist_id,
                },
            )
            .dedupe(format!("scout_artist:{artist_id}"))
            .priority(ARTIST_PRIORITY),
        )
        .await?;
        expanding += usize::from(job.created);
    }
    let charts_due = sqlx::query_scalar!(
        r#"SELECT NOT EXISTS (SELECT 1 FROM jobs WHERE kind = 'scout_charts'
                                AND (status IN ('queued', 'running')
                                     OR (status = 'done' AND finished_at > now() - interval '24 hours'))) AS "due!""#
    )
    .fetch_one(&mut *conn)
    .await?;
    if charts_due {
        jobs::enqueue(
            &mut conn,
            NewJob::new(kind::SCOUT_CHARTS, lanes::SCOUT, json!({}))
                .dedupe("scout_charts")
                .priority(ARTIST_PRIORITY),
        )
        .await?;
    }
    drop(conn);
    let library = enqueue_library(&ctx.db).await?;
    let queued = enqueue_previews(&ctx.db, &settings).await?;
    let evicted = evict(&ctx.db, settings.max_index).await?;
    Ok(json!({
        "artists": nodes.len(),
        "seeds": nodes.values().filter(|n| n.hops == 0).count(),
        "expanding": expanding,
        "charts": charts_due,
        "library": library,
        "queued": queued,
        "evicted": evicted,
    })
    .into())
}

/// Задача `scout_artist`: похожие (если окрестность растёт дальше), популярное, свежие релизы и радио.
pub async fn scout_artist(ctx: Arc<Context>, job: JobRow) -> JobResult {
    let p: ScoutArtistPayload = job.payload()?;
    let settings = Settings::load(&ctx.db).await?;
    let row = sqlx::query!(
        "SELECT s.hops, a.deezer_id FROM scout_artists s JOIN artists a ON a.id = s.artist_id
         WHERE s.artist_id = $1",
        p.artist_id,
    )
    .fetch_optional(&ctx.db)
    .await?;
    let Some(row) = row else {
        return Ok(json!({ "skipped": "not_in_neighbourhood" }).into());
    };
    let Some(deezer_id) = row.deezer_id else {
        return Ok(json!({ "skipped": "no_deezer" }).into());
    };
    let Some(fresh) = catalog::artist_freshness(&ctx.db, p.artist_id).await? else {
        return Err(JobError::permanent("нет такого артиста"));
    };
    let stale = fresh.stale();
    let parts = ArtistParts {
        card: false,
        top: stale.top,
        related: stale.related && row.hops < settings.max_hops,
        releases: stale.releases,
    };
    if parts.any() {
        retry_deadlocks(|| catalog::refresh_artist(&ctx.db, &ctx.deezer, p.artist_id, parts))
            .await?;
    }
    let radio = match ctx.deezer.artist_radio(deezer_id, RADIO_LIMIT).await {
        Ok(tracks) => tracks,
        Err(DeezerError::NotFound) => Vec::new(),
        Err(e) => return Err(e.into()),
    };
    let radio_ids = retry_deadlocks(|| catalog::save_listed(&ctx.db, &radio)).await?;
    let recent = sqlx::query!(
        r#"SELECT al.id, al.deezer_id, al.fetched_at FROM artist_releases r JOIN albums al ON al.id = r.album_id
           WHERE r.artist_id = $1 AND al.release_date > current_date - 180 AND al.release_date <= current_date
           ORDER BY al.release_date DESC, al.id
           LIMIT $2"#,
        p.artist_id,
        RECENT_RELEASES,
    )
    .fetch_all(&ctx.db)
    .await?;
    let mut release_albums = Vec::new();
    for album in &recent {
        if album.fetched_at.is_none()
            && let Some(id) = album.deezer_id
            && let Err(e) = retry_deadlocks(|| catalog::load_album(&ctx.db, &ctx.deezer, id)).await
        {
            tracing::warn!(album = %album.id, error = %e, "could not load a recent release");
            continue;
        }
        release_albums.push(album.id);
    }
    let top = sqlx::query!(
        "SELECT track_id, position FROM artist_top_tracks WHERE artist_id = $1 ORDER BY position",
        p.artist_id,
    )
    .fetch_all(&ctx.db)
    .await?;
    let releases = sqlx::query_scalar!(
        "SELECT id FROM tracks WHERE album_id = ANY($1) ORDER BY album_id, disc_number, track_position NULLS LAST, id",
        &release_albums,
    )
    .fetch_all(&ctx.db)
    .await?;
    let artist = Some(p.artist_id);
    let mut found: Vec<Found> = Vec::new();
    for t in &top {
        found.push((
            t.track_id,
            artist,
            "top",
            0.75 + 0.25 * (1.0 - f32::from(t.position) / 10.0),
        ));
    }
    for (i, id) in releases.iter().enumerate() {
        found.push((*id, artist, "release", 0.9 - 0.01 * i as f32));
    }
    // Радио — похожее на этого артиста: близость берётся от него, а не от исполнителя трека
    for (i, id) in radio_ids.iter().enumerate() {
        found.push((*id, artist, "radio", 0.6 * (1.0 - i as f32 / 80.0)));
    }
    let added = add_tracks(&ctx.db, found).await?;
    sqlx::query!(
        "UPDATE scout_artists SET scouted_at = now() WHERE artist_id = $1",
        p.artist_id
    )
    .execute(&ctx.db)
    .await?;
    Ok(json!({
        "refreshed": parts,
        "top": top.len(),
        "releases": releases.len(),
        "radio": radio_ids.len(),
        "candidates": added,
    })
    .into())
}

/// Задача `scout_charts`: чарты и новые релизы жанров, которые слушают (по альбомам любимого
/// и дослушанного), — раз в сутки.
pub async fn scout_charts(ctx: Arc<Context>, _job: JobRow) -> JobResult {
    let favourite = sqlx::query!(
        r#"SELECT g AS "genre!", count(*) AS "n!"
           FROM (
               SELECT unnest(al.genres) AS g FROM liked_tracks l
               JOIN tracks t ON t.id = l.track_id JOIN albums al ON al.id = t.album_id
               UNION ALL
               SELECT unnest(al.genres) FROM play_events p
               JOIN tracks t ON t.id = p.track_id JOIN albums al ON al.id = t.album_id
               WHERE p.started_at > now() - interval '180 days'
                 AND (p.end_reason = 'completed' OR p.played_ms >= 0.8 * p.duration_ms)
           ) x
           GROUP BY g ORDER BY count(*) DESC, g
           LIMIT $1"#,
        GENRES,
    )
    .fetch_all(&ctx.db)
    .await?;
    let Some(top) = favourite.first().map(|g| g.n) else {
        return Ok(json!({ "skipped": "no_genres" }).into());
    };
    let known = ctx.deezer.genres().await?;
    let mut found: Vec<Found> = Vec::new();
    let mut used = Vec::new();
    for g in &favourite {
        let Some(genre_id) = known
            .iter()
            .find(|k| k.name.eq_ignore_ascii_case(&g.genre))
            .and_then(|k| k.id)
            .filter(|id| *id > 0)
        else {
            continue;
        };
        let share = g.n as f32 / top as f32;
        used.push(g.genre.clone());
        match ctx.deezer.genre_chart(genre_id, CHART_LIMIT).await {
            Ok(tracks) => {
                let ids = retry_deadlocks(|| catalog::save_listed(&ctx.db, &tracks)).await?;
                for (i, id) in ids.iter().enumerate() {
                    found.push((*id, None, "chart", 0.45 * share * (1.0 - i as f32 / 100.0)));
                }
            }
            Err(DeezerError::NotFound) => {}
            Err(e) => return Err(e.into()),
        }
        let albums = match ctx
            .deezer
            .editorial_releases(genre_id, EDITORIAL_LIMIT)
            .await
        {
            Ok(albums) => albums,
            Err(DeezerError::NotFound) => Vec::new(),
            Err(e) => return Err(e.into()),
        };
        for album in albums.iter().take(EDITORIAL_ALBUMS) {
            let album_id = match retry_deadlocks(|| {
                catalog::load_album(&ctx.db, &ctx.deezer, album.id)
            })
            .await
            {
                Ok(id) => id,
                Err(e) => {
                    tracing::warn!(deezer_album = album.id, error = %e, "could not load an editorial release");
                    continue;
                }
            };
            let tracks = sqlx::query_scalar!("SELECT id FROM tracks WHERE album_id = $1", album_id)
                .fetch_all(&ctx.db)
                .await?;
            for id in tracks {
                found.push((id, None, "editorial", 0.35 * share));
            }
        }
    }
    let added = add_tracks(&ctx.db, found).await?;
    Ok(json!({ "genres": used, "candidates": added }).into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn closeness_spreads_with_decay_and_order() {
        let seed = Uuid::from_u128(1);
        let near = Uuid::from_u128(2);
        let far = Uuid::from_u128(3);
        let blocked = Uuid::from_u128(4);
        let mut nodes = HashMap::from([(
            seed,
            Node {
                weight: 1.0,
                hops: 0,
                via: None,
            },
        )]);
        let unwanted = HashSet::from([blocked]);
        let edges = vec![(seed, 0, near), (seed, 19, far), (seed, 1, blocked)];
        let grown = spread(&mut nodes, &edges, 1, 0.5, &unwanted);
        assert_eq!(grown, vec![near, far]);
        assert!((nodes[&near].weight - 0.5).abs() < 1e-6);
        assert!(
            nodes[&far].weight < nodes[&near].weight,
            "дальше в списке — дальше по вкусу"
        );
        assert!(!nodes.contains_key(&blocked));
        assert_eq!(nodes[&near].via, Some(seed));
        // Второй шаг: семя, найденное снова, своего веса не теряет
        let grown = spread(&mut nodes, &[(near, 0, seed)], 2, 0.5, &unwanted);
        assert!(grown.is_empty());
        assert_eq!(nodes[&seed].hops, 0);
        assert_eq!(nodes[&seed].weight, 1.0);
    }
}
