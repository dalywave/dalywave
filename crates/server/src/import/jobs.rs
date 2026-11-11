//! Задачи импорта: `import_match` — сопоставление позиций пачками (прогресс виден сразу, сбой Deezer
//! не теряет сделанного), `import_apply` — раскладка в медиатеку и завершение.

use std::{collections::HashMap, sync::Arc};

use dalywave_protocol::imports::{ImportCandidate, ImportItemKind, ImportJobPayload};
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

use super::{
    cache_key,
    matching::{self, ACCEPT, Cand, Wanted, normalize},
};
use crate::{
    catalog,
    deezer::Deezer,
    jobs::{
        JobRow,
        worker::{Context, JobError, JobResult},
    },
};

/// Позиций за проход: после каждого прохода обновляется прогресс.
const BATCH: i64 = 25;
/// Артист находится, только если имя совпало почти полностью: имена короткие и часто одинаковые.
const ARTIST_ACCEPT: f32 = 0.88;
/// «Не нашли» в кеше живёт неделю: каталог Deezer пополняется.
const MISS_TTL_DAYS: f64 = 7.0;

pub struct Pending {
    pub id: Uuid,
    pub kind: String,
    pub artist: String,
    pub title: String,
    pub album: Option<String>,
    pub duration_ms: Option<i32>,
    pub isrc: Option<String>,
    pub source_ref: Option<String>,
}

/// Чем кончилось сопоставление позиции.
#[derive(Debug, Default)]
pub struct Outcome {
    pub track_id: Option<Uuid>,
    pub album_id: Option<Uuid>,
    pub artist_id: Option<Uuid>,
    pub confidence: Option<f32>,
    pub resolved_by: &'static str,
    pub candidates: Vec<ImportCandidate>,
}

impl Outcome {
    fn matched(&self) -> bool {
        self.track_id.is_some() || self.album_id.is_some() || self.artist_id.is_some()
    }
}

/// Сколько ещё позиций того же альбома ждёт: стоит ли загрузить альбом целиком (два запроса вместо
/// поиска на каждый трек).
pub type AlbumWaiting = HashMap<(String, String), usize>;

/// Задача `import_match`.
pub async fn run_match(ctx: Arc<Context>, job: JobRow) -> JobResult {
    let p: ImportJobPayload = job.payload()?;
    if !running(&ctx.db, p.import_id).await? {
        return Ok(json!({ "skipped": "not_running" }).into());
    }
    let mut waiting: AlbumWaiting = HashMap::new();
    for r in sqlx::query!(
        "SELECT artist, album AS \"album!\" FROM import_items
         WHERE import_id = $1 AND status = 'pending' AND kind = 'track' AND album IS NOT NULL",
        p.import_id
    )
    .fetch_all(&ctx.db)
    .await?
    {
        *waiting
            .entry((normalize(&r.artist), normalize(&r.album)))
            .or_default() += 1;
    }
    let (mut matched, mut unmatched) = (0u32, 0u32);
    loop {
        let batch = sqlx::query_as!(
            Pending,
            "SELECT id, kind, artist, title, album, duration_ms, isrc, source_ref FROM import_items
             WHERE import_id = $1 AND status = 'pending' AND kind <> 'file'
             ORDER BY priority DESC, position
             LIMIT $2",
            p.import_id,
            BATCH,
        )
        .fetch_all(&ctx.db)
        .await?;
        if batch.is_empty() {
            break;
        }
        for item in &batch {
            let out = match_item(&ctx.db, &ctx.deezer, item, &mut waiting).await?;
            if out.matched() {
                matched += 1;
            } else {
                unmatched += 1;
            }
            save(&ctx.db, item.id, &out).await?;
        }
        let mut conn = ctx.db.acquire().await?;
        super::refresh_counts(&mut conn, p.import_id).await?;
        drop(conn);
        if !running(&ctx.db, p.import_id).await? {
            return Ok(
                json!({ "stopped": true, "matched": matched, "unmatched": unmatched }).into(),
            );
        }
    }
    let mut tx = ctx.db.begin().await?;
    sqlx::query!(
        "UPDATE imports SET phase = 'applying', updated_at = now() WHERE id = $1 AND phase = 'matching'",
        p.import_id
    )
    .execute(&mut *tx)
    .await?;
    super::enqueue_apply(&mut tx, p.import_id).await?;
    tx.commit().await?;
    Ok(json!({ "matched": matched, "unmatched": unmatched }).into())
}

/// Задача `import_apply`.
pub async fn run_apply(ctx: Arc<Context>, job: JobRow) -> JobResult {
    let p: ImportJobPayload = job.payload()?;
    if !running(&ctx.db, p.import_id).await? {
        return Ok(json!({ "skipped": "not_running" }).into());
    }
    let stats = super::apply(&ctx.db, &ctx.deezer, p.import_id, None).await?;
    super::finish(&ctx.db, p.import_id).await?;
    Ok(serde_json::to_value(stats)?.into())
}

async fn running(db: &PgPool, import_id: Uuid) -> Result<bool, sqlx::Error> {
    Ok(sqlx::query_scalar!(
        r#"SELECT status = 'running' AS "running!" FROM imports WHERE id = $1"#,
        import_id
    )
    .fetch_optional(db)
    .await?
    .unwrap_or(false))
}

async fn save(db: &PgPool, item_id: Uuid, out: &Outcome) -> Result<(), sqlx::Error> {
    sqlx::query!(
        "UPDATE import_items SET status = $2, track_id = $3, album_id = $4, artist_id = $5, confidence = $6,
                resolved_by = $7, candidates = $8, updated_at = now()
         WHERE id = $1 AND status = 'pending'",
        item_id,
        if out.matched() { "matched" } else { "unmatched" },
        out.track_id,
        out.album_id,
        out.artist_id,
        out.confidence,
        out.resolved_by,
        serde_json::to_value(&out.candidates).unwrap_or_else(|_| json!([])),
    )
    .execute(db)
    .await?;
    Ok(())
}

struct Cached {
    deezer_id: Option<i64>,
    confidence: Option<f32>,
    candidates: serde_json::Value,
    fresh: bool,
}

async fn cached(db: &PgPool, key: &str) -> Result<Option<Cached>, sqlx::Error> {
    sqlx::query_as!(
        Cached,
        r#"SELECT deezer_id, confidence, candidates,
                  (deezer_id IS NOT NULL OR matched_at > now() - make_interval(days => $2::float8::int)) AS "fresh!"
           FROM import_matches WHERE key = $1"#,
        key,
        MISS_TTL_DAYS,
    )
    .fetch_optional(db)
    .await
}

async fn remember(
    db: &PgPool,
    key: &str,
    kind: ImportItemKind,
    deezer_id: Option<i64>,
    confidence: Option<f32>,
    candidates: &[ImportCandidate],
) -> Result<(), sqlx::Error> {
    let mut conn = db.acquire().await?;
    super::remember(
        &mut conn, key, kind, deezer_id, confidence, candidates, false,
    )
    .await
}

/// Сопоставляет одну позицию: кеш, свой каталог, Deezer.
pub async fn match_item(
    db: &PgPool,
    deezer: &Deezer,
    item: &Pending,
    waiting: &mut AlbumWaiting,
) -> Result<Outcome, JobError> {
    let kind: ImportItemKind = item.kind.parse().unwrap_or(ImportItemKind::Track);
    let key = cache_key(kind, &item.artist, &item.title, item.source_ref.as_deref());
    match kind {
        ImportItemKind::Album => match_album(db, deezer, item, &key).await,
        ImportItemKind::Artist => match_artist(db, deezer, item, &key).await,
        ImportItemKind::Track | ImportItemKind::File => {
            let w = Wanted {
                artist: item.artist.clone(),
                title: item.title.clone(),
                album: item.album.clone(),
                duration_ms: item.duration_ms,
                isrc: item.isrc.clone(),
            };
            match_track(db, deezer, &w, &key, waiting).await
        }
    }
}

/// Трек: кеш → свой каталог → Deezer. Найденное запоминается для следующих импортов.
pub async fn match_track(
    db: &PgPool,
    deezer: &Deezer,
    w: &Wanted,
    key: &str,
    waiting: &mut AlbumWaiting,
) -> Result<Outcome, JobError> {
    if let Some(album) = &w.album {
        let k = (normalize(&w.artist), normalize(album));
        if let Some(n) = waiting.get_mut(&k) {
            *n = n.saturating_sub(1);
        }
    }
    if let Some(c) = cached(db, key).await? {
        if let Some(dz) = c.deezer_id {
            // Трек уже в каталоге (хоть облегчённой строкой) — Deezer не нужен
            let known = sqlx::query_scalar!("SELECT id FROM tracks WHERE deezer_id = $1", dz)
                .fetch_optional(db)
                .await?;
            if let Some(id) = known {
                return Ok(Outcome {
                    track_id: Some(id),
                    confidence: c.confidence,
                    resolved_by: "cache",
                    ..Outcome::default()
                });
            }
            match catalog::resolve_track(db, deezer, dz).await {
                Ok(id) => {
                    return Ok(Outcome {
                        track_id: Some(id),
                        confidence: c.confidence,
                        resolved_by: "cache",
                        ..Outcome::default()
                    });
                }
                Err(catalog::CatalogError::Deezer(crate::deezer::DeezerError::NotFound)) => {}
                Err(e) => return Err(e.into()),
            }
        } else if c.fresh {
            return Ok(Outcome {
                confidence: c.confidence,
                resolved_by: "cache",
                candidates: serde_json::from_value(c.candidates).unwrap_or_default(),
                ..Outcome::default()
            });
        }
    }
    if let Some((id, deezer_id, score)) = local_track(db, w).await?
        && score >= ACCEPT
    {
        remember(
            db,
            key,
            ImportItemKind::Track,
            Some(deezer_id),
            Some(score),
            &[],
        )
        .await?;
        return Ok(Outcome {
            track_id: Some(id),
            confidence: Some(score),
            resolved_by: "catalog",
            ..Outcome::default()
        });
    }
    let found = matching::find_track(deezer, w).await?;
    let candidates = found.views();
    match found.best {
        Some((t, score)) if score >= ACCEPT => {
            let ids = catalog::save_listed(db, std::slice::from_ref(&t)).await?;
            let Some(id) = ids.first().copied() else {
                return Err(JobError::retry("трек не сохранился в каталоге"));
            };
            remember(db, key, ImportItemKind::Track, Some(t.id), Some(score), &[]).await?;
            // Ещё несколько треков того же альбома ждут — альбом целиком, дальше они найдутся у себя
            if let (Some(album), Some(dz_album)) = (&w.album, &t.album) {
                let k = (normalize(&w.artist), normalize(album));
                let more = waiting.get(&k).copied().unwrap_or(0);
                if more >= 2
                    && matching::similar(&normalize(album), &normalize(&dz_album.title)) >= 0.85
                {
                    waiting.remove(&k);
                    if let Err(e) = catalog::resolve_album(db, deezer, dz_album.id).await {
                        tracing::debug!(album = dz_album.id, error = %e, "could not preload an album");
                    }
                }
            }
            Ok(Outcome {
                track_id: Some(id),
                confidence: Some(score),
                resolved_by: "deezer",
                ..Outcome::default()
            })
        }
        best => {
            let score = best.map(|(_, s)| s);
            remember(db, key, ImportItemKind::Track, None, score, &candidates).await?;
            Ok(Outcome {
                confidence: score,
                resolved_by: "deezer",
                candidates,
                ..Outcome::default()
            })
        }
    }
}

/// Лучший трек своего каталога с тем же названием.
async fn local_track(db: &PgPool, w: &Wanted) -> Result<Option<(Uuid, i64, f32)>, sqlx::Error> {
    if let Some(isrc) = &w.isrc {
        let hit = sqlx::query!(
            r#"SELECT id, deezer_id AS "deezer_id!" FROM tracks
               WHERE isrc = upper($1) AND deezer_id IS NOT NULL
               ORDER BY rank DESC NULLS LAST LIMIT 1"#,
            isrc
        )
        .fetch_optional(db)
        .await?;
        if let Some(h) = hit {
            return Ok(Some((h.id, h.deezer_id, 1.0)));
        }
    }
    let rows = sqlx::query!(
        r#"SELECT t.id, t.deezer_id AS "deezer_id!", t.title, t.title_version, t.duration_ms, t.isrc,
                  COALESCE(t.rank, 0) AS "rank!", al.title AS "album?",
                  ARRAY(SELECT a.name FROM track_artists ta JOIN artists a ON a.id = ta.artist_id
                        WHERE ta.track_id = t.id ORDER BY ta.position, a.name) AS "artists!"
           FROM tracks t LEFT JOIN albums al ON al.id = t.album_id
           WHERE search_norm(t.title) = search_norm($1) AND t.deezer_id IS NOT NULL
           ORDER BY t.rank DESC NULLS LAST
           LIMIT 30"#,
        matching::base_title(&w.title),
    )
    .fetch_all(db)
    .await?;
    let mut best: Option<(Uuid, i64, f32)> = None;
    for r in rows {
        let c = Cand {
            deezer_id: r.deezer_id,
            title: r.title,
            version: r.title_version,
            artists: r.artists,
            album: r.album,
            duration_ms: Some(r.duration_ms),
            isrc: r.isrc,
            rank: r.rank,
        };
        let s = matching::score_track(w, &c);
        if best.as_ref().is_none_or(|(_, _, b)| s > *b) {
            best = Some((r.id, r.deezer_id, s));
        }
    }
    Ok(best)
}

async fn match_album(
    db: &PgPool,
    deezer: &Deezer,
    item: &Pending,
    key: &str,
) -> Result<Outcome, JobError> {
    let (deezer_id, score) = match cached(db, key).await? {
        Some(Cached {
            deezer_id: Some(id),
            confidence,
            ..
        }) => (Some(id), confidence),
        Some(c) if c.fresh => (None, c.confidence),
        _ => {
            let local = sqlx::query!(
                r#"SELECT al.deezer_id AS "deezer_id!", al.title, a.name FROM albums al
                   JOIN artists a ON a.id = al.artist_id
                   WHERE search_norm(al.title) = search_norm($1) AND al.deezer_id IS NOT NULL
                   LIMIT 20"#,
                matching::base_title(&item.title),
            )
            .fetch_all(db)
            .await?
            .into_iter()
            .map(|r| {
                let s = matching::score_album(&item.artist, &item.title, &r.title, Some(&r.name));
                (r.deezer_id, s)
            })
            .max_by(|a, b| a.1.total_cmp(&b.1));
            match local {
                Some((id, s)) if s >= ACCEPT => (Some(id), Some(s)),
                _ => {
                    let (best, _) = matching::find_album(deezer, &item.artist, &item.title).await?;
                    match best {
                        Some((a, s)) if s >= ACCEPT => (Some(a.id), Some(s)),
                        other => (None, other.map(|(_, s)| s)),
                    }
                }
            }
        }
    };
    let Some(deezer_id) = deezer_id else {
        remember(db, key, ImportItemKind::Album, None, score, &[]).await?;
        return Ok(Outcome {
            confidence: score,
            resolved_by: "deezer",
            ..Outcome::default()
        });
    };
    let album_id = match catalog::resolve_album(db, deezer, deezer_id).await {
        Ok(id) => id,
        Err(catalog::CatalogError::Deezer(crate::deezer::DeezerError::NotFound)) => {
            remember(db, key, ImportItemKind::Album, None, None, &[]).await?;
            return Ok(Outcome {
                resolved_by: "deezer",
                ..Outcome::default()
            });
        }
        Err(e) => return Err(e.into()),
    };
    remember(db, key, ImportItemKind::Album, Some(deezer_id), score, &[]).await?;
    Ok(Outcome {
        album_id: Some(album_id),
        confidence: score.or(Some(1.0)),
        resolved_by: "deezer",
        ..Outcome::default()
    })
}

async fn match_artist(
    db: &PgPool,
    deezer: &Deezer,
    item: &Pending,
    key: &str,
) -> Result<Outcome, JobError> {
    let cached_id = cached(db, key).await?.and_then(|c| c.deezer_id);
    let deezer_id = match cached_id {
        Some(id) => Some(id),
        None => {
            let local = sqlx::query_scalar!(
                r#"SELECT deezer_id AS "deezer_id!" FROM artists
                   WHERE search_norm(name) = search_norm($1) AND deezer_id IS NOT NULL
                   ORDER BY fans DESC NULLS LAST LIMIT 1"#,
                item.artist.trim(),
            )
            .fetch_optional(db)
            .await?;
            match local {
                Some(id) => Some(id),
                None => {
                    let (best, _) = matching::find_artist(deezer, &item.artist).await?;
                    best.filter(|(_, s)| *s >= ARTIST_ACCEPT).map(|(a, _)| a.id)
                }
            }
        }
    };
    let Some(deezer_id) = deezer_id else {
        remember(db, key, ImportItemKind::Artist, None, None, &[]).await?;
        return Ok(Outcome {
            resolved_by: "deezer",
            ..Outcome::default()
        });
    };
    let artist_id = catalog::resolve_artist(db, deezer, deezer_id).await?;
    remember(
        db,
        key,
        ImportItemKind::Artist,
        Some(deezer_id),
        Some(1.0),
        &[],
    )
    .await?;
    Ok(Outcome {
        artist_id: Some(artist_id),
        confidence: Some(1.0),
        resolved_by: "deezer",
        ..Outcome::default()
    })
}
