//! Клипы ленты открытий: 10–15 секунд официального видео вокруг припева, 480p без звука.
//!
//! Лента просит видео для ближайших карточек (`POST /api/clips`): готовые отдаются ссылкой,
//! остальные ставятся задачами `clip` в полосу «Клипы». Задачи берёт домашний загрузчик
//! (`/api/fetcher/clips/*`): он ищет клип на YouTube, вырезает отрывок с места припева (сервер
//! даёт место по анализу трека) и кладёт MP4 по подписанной ссылке. Клип живёт неделю с последнего
//! показа, трек в любимых — два месяца; общий объём ограничен, сверх него уходят давно не
//! показанные.

use std::{collections::HashMap, time::Duration};

use chrono::Utc;
use dalywave_protocol::{
    fetcher::UploadTarget,
    jobs::kind,
    video::{ClipJob, ClipOutcome, ClipResult, TrackClip, VideoState},
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

use super::{RECHECK_DAYS, keys, signed};
use crate::{
    chorus,
    error::AppError,
    jobs::{self, Claim, ClaimOptions, JobRow, Kinds, NewJob},
    storage::{Storage, UPLOAD_PUT_TTL},
};

pub const LANE: &str = "clips";
/// Длина отрывка.
pub const LENGTH_MS: i32 = 12_000;
/// Аренда задания: поиск, скачивание куска и перекодирование укладываются в минуту.
pub const LEASE: Duration = Duration::from_secs(5 * 60);
/// Сколько видео просит лента за раз.
pub const MAX_REQUEST: usize = 20;
/// Клип не показывали столько дней — уходит (трек в любимых — [`KEEP_LIKED_DAYS`]).
const KEEP_DAYS: i32 = 7;
const KEEP_LIKED_DAYS: i32 = 60;
/// Предел клипов в хранилище; `DALYWAVE_CLIPS_MAX_GB`.
const DEFAULT_LIMIT_GB: u64 = 3;
/// Сбои загрузчика кончились попытками — не искать неделю.
const FAILED_RECHECK_DAYS: i32 = 7;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipPayload {
    pub track_id: Uuid,
}

/// Видео для карточек ленты: клипы и живые обложки альбомов. Неизвестное ставится в очередь.
pub async fn prepare(
    db: &PgPool,
    storage: &Storage,
    motion_enabled: bool,
    track_ids: &[Uuid],
) -> Result<Vec<TrackClip>, sqlx::Error> {
    let tracks = sqlx::query!(
        r#"SELECT t.id AS "id!", t.album_id, c.state AS "state?", c.storage_key, c.duration_ms AS "clip_ms?",
                  (c.checked_at < now() - make_interval(days => $2)) AS "stale?"
           FROM tracks t LEFT JOIN track_clips c ON c.track_id = t.id
           WHERE t.id = ANY($1)"#,
        track_ids,
        RECHECK_DAYS,
    )
    .fetch_all(db)
    .await?;
    let albums: Vec<Uuid> = tracks.iter().filter_map(|t| t.album_id).collect();
    let motions = super::motions(db, storage, motion_enabled, &albums).await?;
    let by_id: HashMap<Uuid, _> = tracks.into_iter().map(|t| (t.id, t)).collect();
    let mut out = Vec::new();
    let mut wanted = Vec::new();
    let mut shown = Vec::new();
    for id in track_ids {
        let Some(t) = by_id.get(id) else { continue };
        if out.iter().any(|c: &TrackClip| c.track_id == *id) {
            continue;
        }
        let (state, url) = match t.state.as_deref() {
            Some("ready") => match signed(storage, t.storage_key.as_deref()).await {
                Some(url) => {
                    shown.push(*id);
                    (VideoState::Ready, Some(url))
                }
                None => (VideoState::Pending, None),
            },
            Some(_) if t.stale != Some(true) => (VideoState::None, None),
            _ => {
                wanted.push(*id);
                (VideoState::Pending, None)
            }
        };
        out.push(TrackClip {
            track_id: *id,
            state,
            duration_ms: url.as_ref().and(t.clip_ms),
            url,
            motion: t.album_id.and_then(|a| motions.get(&a).cloned()),
        });
    }
    if !shown.is_empty() {
        sqlx::query!(
            "UPDATE track_clips SET last_used_at = now()
             WHERE track_id = ANY($1) AND last_used_at < now() - interval '1 hour'",
            &shown
        )
        .execute(db)
        .await?;
    }
    if !wanted.is_empty() {
        request(db, &wanted).await?;
    }
    Ok(out)
}

/// Поставить поиск клипов (одна задача на трек). Первые в списке — раньше.
pub async fn request(db: &PgPool, track_ids: &[Uuid]) -> Result<(), sqlx::Error> {
    let mut conn = db.acquire().await?;
    let n = track_ids.len() as i16;
    for (i, &track_id) in track_ids.iter().enumerate() {
        jobs::enqueue(
            &mut conn,
            NewJob::new(kind::CLIP, LANE, ClipPayload { track_id })
                .dedupe(format!("clip:{track_id}"))
                .priority(n - i as i16),
        )
        .await?;
    }
    Ok(())
}

/// Есть ли клип, который загрузчик может взять прямо сейчас: тогда запрос задания скачивания
/// отвечает сразу, и загрузчик спрашивает клип, не дожидаясь конца ожидания.
pub async fn ready_to_claim(db: &PgPool) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar!(
        r#"SELECT (EXISTS (SELECT 1 FROM jobs WHERE lane = $1 AND kind = $2 AND status = 'queued' AND run_after <= now())
              AND NOT EXISTS (SELECT 1 FROM jobs WHERE lane = $1 AND status = 'running')
              AND EXISTS (SELECT 1 FROM job_lanes WHERE lane = $1 AND NOT paused)) AS "ready!""#,
        LANE,
        kind::CLIP,
    )
    .fetch_one(db)
    .await
}

/// Задание клипа загрузчику; `None` — клипов нет.
pub async fn claim(
    db: &PgPool,
    storage: &Storage,
    executor: &str,
) -> Result<Option<ClipJob>, AppError> {
    let opts = ClaimOptions {
        kinds: Kinds::only(&[kind::CLIP]),
        lease: Some(LEASE),
    };
    // Задачи удалённых треков закрываются по пути; предел — от зацикливания
    for _ in 0..20 {
        let job = match jobs::claim_with(db, LANE, executor, opts.clone()).await? {
            Claim::Job(job) => *job,
            _ => return Ok(None),
        };
        let Ok(p) = job.payload::<ClipPayload>() else {
            jobs::fail(db, job.id, executor, "неверные данные задачи", false).await?;
            continue;
        };
        let Some(t) = sqlx::query!(
            r#"SELECT t.title, t.title_version, t.duration_ms, t.isrc,
                      COALESCE((SELECT array_agg(a.name ORDER BY ta.position, a.name)
                                FROM track_artists ta JOIN artists a ON a.id = ta.artist_id
                                WHERE ta.track_id = t.id), '{}') AS "artists!",
                      COALESCE(c.rejected_videos, '{}') AS "rejected!"
               FROM tracks t LEFT JOIN track_clips c ON c.track_id = t.id WHERE t.id = $1"#,
            p.track_id
        )
        .fetch_optional(db)
        .await?
        else {
            jobs::complete(db, job.id, executor, json!({ "skipped": "gone" })).await?;
            continue;
        };
        let hook = chorus::starts(db, &[p.track_id], i64::from(LENGTH_MS))
            .await?
            .remove(&p.track_id)
            .map_or(0, |(at, _)| at);
        let key = keys::clip(p.track_id, job.id);
        let url = match storage.presign_put(&key, UPLOAD_PUT_TTL).await {
            Ok(url) => url,
            Err(e) => {
                jobs::postpone(
                    db,
                    job.id,
                    executor,
                    &format!("хранилище не подписало ссылку: {e}"),
                    Duration::from_secs(60),
                )
                .await?;
                return Err(AppError::Unavailable(
                    "хранилище недоступно, повторите позже".into(),
                ));
            }
        };
        let now = Utc::now();
        let title = if t.title_version.is_empty() {
            t.title
        } else {
            format!("{} ({})", t.title, t.title_version)
        };
        return Ok(Some(ClipJob {
            id: job.id,
            track_id: p.track_id,
            title,
            artists: t.artists,
            duration_ms: t.duration_ms,
            hook_start_ms: i32::try_from(hook).unwrap_or(0),
            length_ms: LENGTH_MS,
            isrc: t.isrc,
            exclude_video_ids: t.rejected,
            upload: UploadTarget {
                url: url.to_string(),
                method: "PUT".into(),
                headers: [("Content-Type".to_string(), "video/mp4".to_string())].into(),
                expires_at: now + chrono::Duration::from_std(UPLOAD_PUT_TTL).unwrap_or_default(),
            },
            lease_expires_at: job
                .lease_expires_at
                .unwrap_or_else(|| now + chrono::Duration::from_std(LEASE).unwrap_or_default()),
        }));
    }
    Ok(None)
}

/// Отчёт загрузчика. `false` — задание уже не его.
pub async fn settle(
    db: &PgPool,
    storage: &Storage,
    job: &JobRow,
    executor: &str,
    r: &ClipResult,
) -> Result<bool, AppError> {
    let p: ClipPayload = job
        .payload()
        .map_err(|e| AppError::Internal(anyhow::anyhow!("clip payload: {e}")))?;
    let log = serde_json::to_value(r).unwrap_or_default();
    match r.status {
        ClipOutcome::Ready => {
            let key = keys::clip(p.track_id, job.id);
            let Some(meta) = storage
                .head(&key)
                .await
                .map_err(|e| AppError::Unavailable(format!("хранилище недоступно: {e}")))?
            else {
                let closed = jobs::fail(db, job.id, executor, "файл клипа не загружен", true)
                    .await?
                    .is_some();
                return Ok(closed);
            };
            let mut tx = db.begin().await?;
            let Some(_) = jobs::complete_in(&mut tx, job.id, executor, log).await? else {
                return Ok(false);
            };
            let saved = sqlx::query!(
                "INSERT INTO track_clips (track_id, state, video_id, storage_key, size_bytes, duration_ms,
                                          clip_start_ms, video_duration_ms, width, height, checked_at, last_used_at)
                 VALUES ($1, 'ready', $2, $3, $4, $5, $6, $7, $8, $9, now(), now())
                 ON CONFLICT (track_id) DO UPDATE SET state = 'ready', video_id = EXCLUDED.video_id,
                     storage_key = EXCLUDED.storage_key, size_bytes = EXCLUDED.size_bytes,
                     duration_ms = EXCLUDED.duration_ms, clip_start_ms = EXCLUDED.clip_start_ms,
                     video_duration_ms = EXCLUDED.video_duration_ms, width = EXCLUDED.width,
                     height = EXCLUDED.height, checked_at = now(), last_used_at = now()",
                p.track_id,
                r.video_id,
                key,
                meta.size as i64,
                r.duration_ms.unwrap_or(LENGTH_MS),
                r.clip_start_ms,
                r.video_duration_ms,
                r.width,
                r.height,
            )
            .execute(&mut *tx)
            .await;
            match saved {
                Ok(_) => tx.commit().await?,
                // Трек удалили, пока шла нарезка
                Err(sqlx::Error::Database(e)) if e.is_foreign_key_violation() => {
                    drop(tx);
                    jobs::complete(db, job.id, executor, json!({ "skipped": "gone" })).await?;
                    let _ = storage.delete(&key).await;
                }
                Err(e) => return Err(e.into()),
            }
            Ok(true)
        }
        ClipOutcome::None => {
            let mut tx = db.begin().await?;
            let Some(_) = jobs::complete_in(&mut tx, job.id, executor, log).await? else {
                return Ok(false);
            };
            remember_none(&mut tx, p.track_id, RECHECK_DAYS).await?;
            tx.commit().await?;
            Ok(true)
        }
        ClipOutcome::Error => {
            let message: String = r
                .error
                .as_deref()
                .unwrap_or("сбой загрузчика")
                .chars()
                .take(300)
                .collect();
            let mut tx = db.begin().await?;
            let Some(row) = jobs::fail_in(&mut tx, job.id, executor, &message, true).await? else {
                return Ok(false);
            };
            // Попытки кончились: не искать неделю, а не при каждом показе ленты
            if row.status == "failed" {
                remember_none(&mut tx, p.track_id, FAILED_RECHECK_DAYS).await?;
            }
            tx.commit().await?;
            Ok(true)
        }
    }
}

async fn remember_none(
    conn: &mut sqlx::PgConnection,
    track_id: Uuid,
    recheck_in_days: i32,
) -> Result<(), sqlx::Error> {
    let r = sqlx::query!(
        "INSERT INTO track_clips (track_id, state, checked_at)
         VALUES ($1, 'none', now() - make_interval(days => $2))
         ON CONFLICT (track_id) DO UPDATE SET state = 'none', storage_key = NULL, size_bytes = 0,
             video_id = NULL, checked_at = EXCLUDED.checked_at",
        track_id,
        RECHECK_DAYS - recheck_in_days.clamp(0, RECHECK_DAYS),
    )
    .execute(conn)
    .await;
    match r {
        Ok(_) => Ok(()),
        Err(sqlx::Error::Database(e)) if e.is_foreign_key_violation() => Ok(()),
        Err(e) => Err(e),
    }
}

/// Предел клипов в байтах.
pub fn limit_bytes() -> i64 {
    let gb = std::env::var("DALYWAVE_CLIPS_MAX_GB")
        .ok()
        .and_then(|v| v.trim().parse::<u64>().ok())
        .unwrap_or(DEFAULT_LIMIT_GB);
    (gb.saturating_mul(1024 * 1024 * 1024)).min(i64::MAX as u64) as i64
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Trimmed {
    pub expired: u64,
    pub evicted: u64,
}

/// Чистка: давно не показанные клипы и всё сверх предела (сначала не из любимых, по давности
/// показа). Файлы стирает чистка хранилища ([`crate::renditions::trim`]) по `storage_trash`.
pub async fn trim(db: &PgPool, limit: i64) -> Result<Trimmed, sqlx::Error> {
    let expired = sqlx::query!(
        "DELETE FROM track_clips c WHERE c.state = 'ready' AND c.last_used_at < now() - make_interval(days =>
             CASE WHEN EXISTS (SELECT 1 FROM liked_tracks l WHERE l.track_id = c.track_id) THEN $1::int ELSE $2::int END)",
        KEEP_LIKED_DAYS,
        KEEP_DAYS,
    )
    .execute(db)
    .await?
    .rows_affected();
    let evicted = sqlx::query!(
        "DELETE FROM track_clips WHERE track_id IN (
             SELECT track_id FROM (
                 SELECT c.track_id, (sum(c.size_bytes) OVER (
                     ORDER BY EXISTS (SELECT 1 FROM liked_tracks l WHERE l.track_id = c.track_id) DESC,
                              c.last_used_at DESC, c.track_id DESC))::bigint AS kept
                 FROM track_clips c WHERE c.state = 'ready') x
             WHERE x.kept > $1)",
        limit,
    )
    .execute(db)
    .await?
    .rows_affected();
    // «Клипа нет» старше перепроверки — пусть ищется заново при показе
    sqlx::query!(
        "DELETE FROM track_clips WHERE state = 'none' AND checked_at < now() - make_interval(days => $1)",
        RECHECK_DAYS * 3,
    )
    .execute(db)
    .await?;
    Ok(Trimmed { expired, evicted })
}
