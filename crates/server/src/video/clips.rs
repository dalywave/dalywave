//! Клипы: 10–15 секунд официального видео вокруг припева, 480p без звука — вместо обложки в полном
//! плеере и в ленте открытий.
//!
//! Лента и плеер просят видео (`POST /api/clips`): готовые отдаются ссылкой, остальные ставятся
//! задачами `clip` в полосу «Клипы». Задачи берёт домашний загрузчик (`/api/fetcher/clips/*`): он
//! ищет клип на YouTube — только официальный, в приоритете с канала автора (`clip_pick` загрузчика),
//! вырезает отрывок с места припева (сервер даёт место по анализу трека) и кладёт MP4 по подписанной
//! ссылке. Клип живёт неделю с последнего показа, трек в любимых — два месяца; общий объём ограничен,
//! сверх него уходят давно не показанные.
//!
//! Администратор может выбрать клип сам (`PUT /api/tracks/{id}/clip`): ролик из кандидатов, ссылку
//! или «без клипа». Выбор закрепляется (`track_clip_pins`) и автопоиском не перезаписывается. Любой
//! слушатель может скрыть клип у себя — это же жалоба в «Спорные». Клипы, нарезанные старым отбором,
//! воркер понемногу перепроверяет новым ([`review`]).

use std::{collections::HashMap, time::Duration};

use chrono::Utc;
use dalywave_protocol::{
    fetcher::UploadTarget,
    jobs::kind,
    video::{
        CLIP_MODES_FEATURE, CLIP_WHOLE_FEATURE, ClipCandidate, ClipJob, ClipMode, ClipOutcome,
        ClipResult, ClipSync, ClipView, TrackClip, VideoState,
    },
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
/// Длина отрывка по умолчанию (в админке — от 6 до 30 секунд).
pub const LENGTH_MS: i32 = 12_000;
/// Аренда задания: поиск, скачивание куска и перекодирование укладываются в минуту.
pub const LEASE: Duration = Duration::from_secs(5 * 60);
/// Сколько видео просит лента за раз.
pub const MAX_REQUEST: usize = 20;
/// Сбои загрузчика кончились попытками — не искать неделю.
const FAILED_RECHECK_DAYS: i32 = 7;
/// Сбой не трека, а загрузчика: в его ffmpeg нет видео (нет кодировщика H.264, нет https). Новый
/// загрузчик такие клипы и не берёт; старый (приложение без видео в ffmpeg) падает на скачивании
/// куска с «ffmpeg exited with code 8». Задача ждёт загрузчик, который сможет: попытка не
/// тратится, «клипа нет» не запоминается.
const TOOL_ERRORS: &[&str] = &["no_encoder", "no_https"];
const OLD_FFMPEG_ERROR: &str = "ffmpeg exited with code";
const TOOL_RETRY: Duration = Duration::from_secs(60 * 60);
/// Выбор администратора и его «Найти варианты» — впереди ленты.
const CHOICE_PRIORITY: i16 = 100;
/// Перепроверка старых клипов — после всего остального.
const REVIEW_PRIORITY: i16 = -10;
/// Столько перепроверок стоит в очереди одновременно (воркер доливает раз в час).
const REVIEW_BATCH: i64 = 10;

fn tool_failure(error: &str) -> bool {
    let code = error.split(':').next().unwrap_or_default().trim();
    TOOL_ERRORS.contains(&code) || error.contains(OLD_FFMPEG_ERROR)
}

fn dedupe(track_id: Uuid) -> String {
    format!("clip:{track_id}")
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipPayload {
    pub track_id: Uuid,
    /// `auto`, `search` или `review`; `pinned` решается при выдаче по выбору администратора.
    #[serde(default)]
    pub mode: ClipMode,
}

/// Видео для карточек ленты и полного плеера: клипы и живые обложки альбомов. Неизвестное ставится
/// в очередь. Клип, который этот человек скрыл, и «без клипа» администратора — `none`.
pub async fn prepare(
    db: &PgPool,
    storage: &Storage,
    motion_enabled: bool,
    user_id: Uuid,
    track_ids: &[Uuid],
) -> Result<Vec<TrackClip>, sqlx::Error> {
    let tracks = sqlx::query!(
        r#"SELECT t.id AS "id!", t.album_id, c.state AS "state?", c.storage_key, c.duration_ms AS "clip_ms?",
                  c.sync_offset_ms, c.sync_rate,
                  (c.checked_at < now() - make_interval(days => $2)) AS "stale?",
                  (p.track_id IS NOT NULL AND p.video_id IS NULL) AS "off!",
                  EXISTS (SELECT 1 FROM clip_hidden h
                          WHERE h.user_id = $3 AND h.track_id = t.id AND h.video_id = c.video_id) AS "hidden!"
           FROM tracks t
           LEFT JOIN track_clips c ON c.track_id = t.id
           LEFT JOIN track_clip_pins p ON p.track_id = t.id
           WHERE t.id = ANY($1)"#,
        track_ids,
        RECHECK_DAYS,
        user_id,
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
            _ if t.off || t.hidden => (VideoState::None, None),
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
            sync: url
                .as_ref()
                .and(t.sync_offset_ms.zip(t.sync_rate))
                .map(|(offset_ms, rate)| ClipSync {
                    offset_ms,
                    rate: f64::from(rate),
                }),
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
    // Автопоиск выключен — клипы только по выбору администратора
    if !wanted.is_empty() && super::clip_settings::load(db).await?.auto {
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
            NewJob::new(
                kind::CLIP,
                LANE,
                ClipPayload {
                    track_id,
                    mode: ClipMode::Auto,
                },
            )
            .dedupe(dedupe(track_id))
            .priority(n - i as i16),
        )
        .await?;
    }
    Ok(())
}

/// Поставить задачу клипа с этим режимом впереди очереди. Задача трека уже ждёт — ей меняется
/// режим и она поднимается; уже режется — останется как есть (её итог сверится с выбором).
async fn enqueue_now(
    db: &PgPool,
    track_id: Uuid,
    mode: ClipMode,
    priority: i16,
) -> Result<(), sqlx::Error> {
    let payload = serde_json::to_value(ClipPayload { track_id, mode }).unwrap_or_default();
    let mut conn = db.acquire().await?;
    let updated = sqlx::query!(
        "UPDATE jobs SET payload = $2, priority = GREATEST(priority, $3), run_after = LEAST(run_after, now()),
                updated_at = now()
         WHERE dedupe_key = $1 AND status = 'queued'",
        dedupe(track_id),
        payload,
        priority,
    )
    .execute(&mut *conn)
    .await?
    .rows_affected();
    if updated > 0 {
        jobs::wake(&mut conn, LANE).await?;
        return Ok(());
    }
    jobs::enqueue(
        &mut conn,
        NewJob::new(kind::CLIP, LANE, ClipPayload { track_id, mode })
            .dedupe(dedupe(track_id))
            .priority(priority),
    )
    .await?;
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

/// Задание клипа загрузчику; `None` — клипов нет. `features` — что загрузчик умеет сверх `auto`
/// (старому не выдаются выбор администратора, поиск кандидатов и перепроверка: они ждут нового).
pub async fn claim(
    db: &PgPool,
    storage: &Storage,
    executor: &str,
    features: &[String],
) -> Result<Option<ClipJob>, AppError> {
    let opts = ClaimOptions {
        kinds: Kinds::only(&[kind::CLIP]),
        lease: Some(LEASE),
    };
    let modes = features.iter().any(|f| f == CLIP_MODES_FEATURE);
    let can_whole = features.iter().any(|f| f == CLIP_WHOLE_FEATURE);
    let settings = super::clip_settings::load(db).await?;
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
            r#"SELECT t.title, t.title_version, t.duration_ms, t.isrc, al.label,
                      COALESCE((SELECT array_agg(a.name ORDER BY ta.position, a.name)
                                FROM track_artists ta JOIN artists a ON a.id = ta.artist_id
                                WHERE ta.track_id = t.id), '{}') AS "artists!",
                      COALESCE(c.rejected_videos, '{}') AS "rejected!",
                      CASE WHEN c.state = 'ready' THEN c.video_id END AS current_video,
                      p.track_id IS NOT NULL AS "pinned!", p.video_id AS pin_video
               FROM tracks t
               LEFT JOIN albums al ON al.id = t.album_id
               LEFT JOIN track_clips c ON c.track_id = t.id
               LEFT JOIN track_clip_pins p ON p.track_id = t.id
               WHERE t.id = $1"#,
            p.track_id
        )
        .fetch_optional(db)
        .await?
        else {
            jobs::complete(db, job.id, executor, json!({ "skipped": "gone" })).await?;
            continue;
        };
        // Выбор администратора важнее режима задачи (кроме поиска кандидатов)
        let (mode, video_id) = match (p.mode, t.pinned, t.pin_video) {
            (ClipMode::Search, _, _) => (ClipMode::Search, None),
            (_, true, None) => {
                let mut tx = db.begin().await?;
                if jobs::complete_in(&mut tx, job.id, executor, json!({ "skipped": "off" }))
                    .await?
                    .is_some()
                {
                    remember_none(&mut tx, p.track_id, RECHECK_DAYS).await?;
                }
                tx.commit().await?;
                continue;
            }
            (_, true, Some(v)) if t.current_video.as_deref() == Some(v.as_str()) => {
                jobs::complete(db, job.id, executor, json!({ "skipped": "pinned" })).await?;
                continue;
            }
            (_, true, Some(v)) => (ClipMode::Pinned, Some(v)),
            (mode, false, _) => (mode, None),
        };
        if mode == ClipMode::Review && t.current_video.is_none() {
            // Клип ушёл по сроку, пока перепроверка ждала: новый нарежет новый отбор
            jobs::complete(db, job.id, executor, json!({ "skipped": "gone" })).await?;
            continue;
        }
        if mode != ClipMode::Auto && !modes {
            jobs::postpone(
                db,
                job.id,
                executor,
                "загрузчик без режимов клипа — ждёт нового",
                TOOL_RETRY,
            )
            .await?;
            continue;
        }
        let hook = chorus::starts(db, &[p.track_id], i64::from(settings.length_ms))
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
        // Клип целиком в такт: загрузчику — ссылка на звук трека, с которым он сверит ролик
        let whole = settings.whole && can_whole && mode != ClipMode::Search;
        let sync_audio_url = if whole && settings.sync {
            sync_audio(db, storage, p.track_id).await
        } else {
            None
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
            length_ms: settings.length_ms,
            isrc: t.isrc,
            exclude_video_ids: t.rejected,
            label: t.label.filter(|l| !l.trim().is_empty()),
            mode,
            video_id,
            current_video_id: (mode == ClipMode::Review)
                .then_some(t.current_video)
                .flatten(),
            upload: UploadTarget {
                url: url.to_string(),
                method: "PUT".into(),
                headers: [("Content-Type".to_string(), "video/mp4".to_string())].into(),
                expires_at: now + chrono::Duration::from_std(UPLOAD_PUT_TTL).unwrap_or_default(),
            },
            lease_expires_at: job
                .lease_expires_at
                .unwrap_or_else(|| now + chrono::Duration::from_std(LEASE).unwrap_or_default()),
            height: settings.height,
            whole,
            sync_audio_url,
        }));
    }
    Ok(None)
}

/// Подписанная ссылка на файл записи трека (для сопоставления клипа целиком со звуком).
async fn sync_audio(db: &PgPool, storage: &Storage, track_id: Uuid) -> Option<String> {
    let recording = sqlx::query_scalar!("SELECT recording_id FROM tracks WHERE id = $1", track_id)
        .fetch_optional(db)
        .await
        .ok()
        .flatten()
        .flatten()?;
    let file = crate::acquire::primary_file(db, recording)
        .await
        .ok()
        .flatten()?;
    storage
        .presign_get(&file.storage_key, crate::storage::AUDIO_GET_TTL)
        .await
        .ok()
        .map(|u| u.to_string())
}

fn candidates_json(c: &[ClipCandidate]) -> serde_json::Value {
    serde_json::to_value(c).unwrap_or_else(|_| json!([]))
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
    let candidates = candidates_json(&r.candidates);
    let score = r.score.map(|s| s as f32);
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
            // Пока резалось, администратор выбрал другое: этот файл не нужен, режем выбранное
            let pin = sqlx::query!(
                "SELECT video_id FROM track_clip_pins WHERE track_id = $1",
                p.track_id
            )
            .fetch_optional(db)
            .await?;
            if let Some(pin) = pin
                && pin.video_id.as_deref() != r.video_id.as_deref()
            {
                let closed = jobs::complete(db, job.id, executor, json!({ "superseded": log }))
                    .await?
                    .is_some();
                let _ = storage.delete(&key).await;
                if closed && pin.video_id.is_some() {
                    enqueue_now(db, p.track_id, ClipMode::Auto, CHOICE_PRIORITY).await?;
                }
                return Ok(closed);
            }
            let mut tx = db.begin().await?;
            let Some(_) = jobs::complete_in(&mut tx, job.id, executor, log).await? else {
                return Ok(false);
            };
            let saved = sqlx::query!(
                "INSERT INTO track_clips (track_id, state, video_id, storage_key, size_bytes, duration_ms,
                                          clip_start_ms, video_duration_ms, width, height, video_title, channel,
                                          source, score, candidates, candidates_at, reviewed_at, checked_at, last_used_at,
                                          whole, sync_offset_ms, sync_rate)
                 VALUES ($1, 'ready', $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14,
                         CASE WHEN jsonb_array_length($14) > 0 THEN now() END, now(), now(), now(), $15, $16, $17)
                 ON CONFLICT (track_id) DO UPDATE SET state = 'ready', video_id = EXCLUDED.video_id,
                     storage_key = EXCLUDED.storage_key, size_bytes = EXCLUDED.size_bytes,
                     duration_ms = EXCLUDED.duration_ms, clip_start_ms = EXCLUDED.clip_start_ms,
                     video_duration_ms = EXCLUDED.video_duration_ms, width = EXCLUDED.width,
                     height = EXCLUDED.height, video_title = EXCLUDED.video_title, channel = EXCLUDED.channel,
                     source = EXCLUDED.source, score = EXCLUDED.score,
                     candidates = CASE WHEN jsonb_array_length(EXCLUDED.candidates) > 0
                                       THEN EXCLUDED.candidates ELSE track_clips.candidates END,
                     candidates_at = COALESCE(EXCLUDED.candidates_at, track_clips.candidates_at),
                     reviewed_at = now(), checked_at = now(), last_used_at = now(),
                     whole = EXCLUDED.whole, sync_offset_ms = EXCLUDED.sync_offset_ms, sync_rate = EXCLUDED.sync_rate",
                p.track_id,
                r.video_id,
                key,
                meta.size as i64,
                r.duration_ms.unwrap_or(LENGTH_MS),
                r.clip_start_ms,
                r.video_duration_ms,
                r.width,
                r.height,
                r.video_title,
                r.channel,
                r.source,
                score,
                candidates,
                r.whole,
                r.sync.map(|s| s.offset_ms),
                r.sync.map(|s| s.rate as f32).filter(|r| (0.5..=2.0).contains(r)),
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
        ClipOutcome::Kept => {
            let mut tx = db.begin().await?;
            let Some(_) = jobs::complete_in(&mut tx, job.id, executor, log).await? else {
                return Ok(false);
            };
            sqlx::query!(
                "UPDATE track_clips SET video_title = COALESCE($3, video_title), channel = COALESCE($4, channel),
                        source = COALESCE($5, source), score = COALESCE($6, score),
                        candidates = CASE WHEN jsonb_array_length($7) > 0 THEN $7 ELSE candidates END,
                        candidates_at = CASE WHEN jsonb_array_length($7) > 0 THEN now() ELSE candidates_at END,
                        reviewed_at = now()
                 WHERE track_id = $1 AND video_id = $2",
                p.track_id,
                r.video_id,
                r.video_title,
                r.channel,
                r.source,
                score,
                candidates,
            )
            .execute(&mut *tx)
            .await?;
            tx.commit().await?;
            Ok(true)
        }
        ClipOutcome::None => {
            let mut tx = db.begin().await?;
            let Some(_) = jobs::complete_in(&mut tx, job.id, executor, log).await? else {
                return Ok(false);
            };
            if p.mode != ClipMode::Search {
                remember_none(&mut tx, p.track_id, RECHECK_DAYS).await?;
            }
            // Кандидаты — и когда клипа нет: из них администратор выберет сам
            let saved = sqlx::query!(
                "INSERT INTO track_clips (track_id, state, candidates, candidates_at, reviewed_at, checked_at)
                 VALUES ($1, 'none', $2, now(), now(), now() - make_interval(days => $3))
                 ON CONFLICT (track_id) DO UPDATE SET candidates = EXCLUDED.candidates, candidates_at = now(),
                     reviewed_at = CASE WHEN $4 THEN track_clips.reviewed_at ELSE now() END",
                p.track_id,
                candidates,
                RECHECK_DAYS,
                p.mode == ClipMode::Search,
            )
            .execute(&mut *tx)
            .await;
            match saved {
                Ok(_) => tx.commit().await?,
                Err(sqlx::Error::Database(e)) if e.is_foreign_key_violation() => drop(tx),
                Err(e) => return Err(e.into()),
            }
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
            if tool_failure(&message) {
                let postponed = jobs::postpone(db, job.id, executor, &message, TOOL_RETRY).await?;
                return Ok(postponed.is_some());
            }
            let mut tx = db.begin().await?;
            let Some(row) = jobs::fail_in(&mut tx, job.id, executor, &message, true).await? else {
                return Ok(false);
            };
            // Попытки кончились: не искать неделю, а не при каждом показе ленты. Поиск кандидатов
            // и перепроверка нарезанный клип не трогают
            if row.status == "failed" && matches!(p.mode, ClipMode::Auto) {
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
             video_id = NULL, video_title = NULL, channel = NULL, source = NULL, score = NULL,
             checked_at = EXCLUDED.checked_at",
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

// ───────────── «Клип» у трека ─────────────

/// Лист «Клип»: что нарезано, выбор администратора, кандидаты (администратору), скрыт ли у меня.
/// `None` — трека нет.
pub async fn view(
    db: &PgPool,
    storage: &Storage,
    track_id: Uuid,
    user_id: Uuid,
    admin: bool,
) -> Result<Option<ClipView>, sqlx::Error> {
    let Some(r) = sqlx::query!(
        r#"SELECT c.state AS "state?", c.video_id, c.storage_key, c.video_title, c.channel, c.source,
                  COALESCE(c.candidates, '[]') AS "candidates!",
                  p.track_id IS NOT NULL AS "pinned!", p.video_id AS pin_video,
                  EXISTS (SELECT 1 FROM clip_hidden h
                          WHERE h.user_id = $2 AND h.track_id = t.id AND h.video_id = c.video_id) AS "hidden!",
                  (SELECT count(*) FROM clip_hidden h
                   WHERE h.track_id = t.id AND h.video_id = c.video_id
                     AND (p.chosen_at IS NULL OR h.created_at > p.chosen_at)) AS "reports!",
                  (SELECT j.payload->>'mode' FROM jobs j
                   WHERE j.dedupe_key = 'clip:' || t.id::text AND j.status IN ('queued', 'running')
                   LIMIT 1) AS job_mode
           FROM tracks t
           LEFT JOIN track_clips c ON c.track_id = t.id
           LEFT JOIN track_clip_pins p ON p.track_id = t.id
           WHERE t.id = $1"#,
        track_id,
        user_id,
    )
    .fetch_optional(db)
    .await?
    else {
        return Ok(None);
    };
    let ready = r.state.as_deref() == Some("ready") && r.storage_key.is_some();
    let off = r.pinned && r.pin_video.is_none();
    let wrong = r.pinned && r.pin_video.is_some() && r.pin_video != r.video_id;
    let cutting = r.job_mode.as_deref().is_some_and(|m| m != "search");
    let (state, url) = if off {
        (VideoState::None, None)
    } else if ready && !wrong {
        (
            VideoState::Ready,
            signed(storage, r.storage_key.as_deref()).await,
        )
    } else if cutting || wrong {
        (VideoState::Pending, None)
    } else {
        (VideoState::None, None)
    };
    let shown = state == VideoState::Ready;
    let candidates: Vec<ClipCandidate> = if admin {
        serde_json::from_value(r.candidates).unwrap_or_default()
    } else {
        Vec::new()
    };
    Ok(Some(ClipView {
        track_id,
        state,
        url,
        video_id: if shown {
            r.video_id
        } else {
            r.pin_video.clone()
        },
        video_title: shown.then_some(r.video_title).flatten(),
        channel: shown.then_some(r.channel).flatten(),
        source: if r.pinned {
            Some("pinned".into())
        } else {
            shown.then_some(r.source).flatten()
        },
        pinned: r.pinned,
        hidden: r.hidden,
        can_choose: admin,
        searching: admin && r.job_mode.is_some(),
        candidates,
        reports: if admin { r.reports } else { 0 },
    }))
}

/// Что выбрал администратор.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Choice {
    Video(String),
    Off,
    Auto,
    Keep,
    Search,
}

/// Выбор администратора. Ролик — закрепить и нарезать впереди очереди (до тех пор у трека
/// обложка); «без клипа» — убрать и не искать; «заново» — снять выбор и перепроверить.
pub async fn choose(
    db: &PgPool,
    track_id: Uuid,
    admin: Uuid,
    choice: &Choice,
) -> Result<(), sqlx::Error> {
    let current = sqlx::query!(
        r#"SELECT CASE WHEN state = 'ready' THEN video_id END AS video FROM track_clips WHERE track_id = $1"#,
        track_id
    )
    .fetch_optional(db)
    .await?
    .and_then(|r| r.video);
    match choice {
        Choice::Video(video) => {
            pin(db, track_id, Some(video), admin).await?;
            if current.as_deref() != Some(video.as_str()) {
                // Старый клип уходит сразу: пока режется выбранный — обложка
                drop_clip(db, track_id).await?;
                enqueue_now(db, track_id, ClipMode::Auto, CHOICE_PRIORITY).await?;
            }
        }
        Choice::Off => {
            pin(db, track_id, None, admin).await?;
            let mut conn = db.acquire().await?;
            remember_none(&mut conn, track_id, RECHECK_DAYS).await?;
        }
        Choice::Keep => {
            // Жалобы разобраны: нынешний клип закреплён как выбранный
            if let Some(video) = current {
                pin(db, track_id, Some(&video), admin).await?;
            }
        }
        Choice::Auto => {
            sqlx::query!("DELETE FROM track_clip_pins WHERE track_id = $1", track_id)
                .execute(db)
                .await?;
            if current.is_some() {
                // Нынешний останется, если он и есть лучший
                enqueue_now(db, track_id, ClipMode::Review, CHOICE_PRIORITY).await?;
            } else {
                drop_clip(db, track_id).await?;
                enqueue_now(db, track_id, ClipMode::Auto, CHOICE_PRIORITY).await?;
            }
        }
        Choice::Search => {
            let pinned = sqlx::query_scalar!(
                r#"SELECT EXISTS (SELECT 1 FROM track_clip_pins WHERE track_id = $1) AS "e!""#,
                track_id
            )
            .fetch_one(db)
            .await?;
            // Клипа нет и не выбран — поиск сразу и нарежет лучший
            let mode = if current.is_some() || pinned {
                ClipMode::Search
            } else {
                ClipMode::Auto
            };
            enqueue_now(db, track_id, mode, CHOICE_PRIORITY).await?;
        }
    }
    Ok(())
}

async fn pin(
    db: &PgPool,
    track_id: Uuid,
    video: Option<&str>,
    admin: Uuid,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        "INSERT INTO track_clip_pins (track_id, video_id, chosen_by, chosen_at) VALUES ($1, $2, $3, now())
         ON CONFLICT (track_id) DO UPDATE SET video_id = EXCLUDED.video_id, chosen_by = EXCLUDED.chosen_by,
             chosen_at = now()",
        track_id,
        video,
        admin,
    )
    .execute(db)
    .await?;
    Ok(())
}

/// Убрать нарезанный файл (его сотрёт чистка хранилища). «Клипа нет» сразу устаревает: следующий
/// показ поставит поиск, если задача почему-то пропала. Кандидаты остаются.
async fn drop_clip(db: &PgPool, track_id: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query!(
        "UPDATE track_clips SET state = 'none', storage_key = NULL, size_bytes = 0, video_id = NULL,
                video_title = NULL, channel = NULL, source = NULL, score = NULL,
                checked_at = now() - make_interval(days => $2 + 1)
         WHERE track_id = $1",
        track_id,
        RECHECK_DAYS,
    )
    .execute(db)
    .await?;
    Ok(())
}

/// «Скрыть клип» (личное, и жалоба администратору). `false` — у трека нет нарезанного клипа.
pub async fn hide(db: &PgPool, track_id: Uuid, user_id: Uuid) -> Result<bool, sqlx::Error> {
    let r = sqlx::query!(
        "INSERT INTO clip_hidden (user_id, track_id, video_id)
         SELECT $2, c.track_id, c.video_id FROM track_clips c
         WHERE c.track_id = $1 AND c.state = 'ready' AND c.video_id IS NOT NULL
         ON CONFLICT DO NOTHING",
        track_id,
        user_id,
    )
    .execute(db)
    .await?;
    if r.rows_affected() > 0 {
        return Ok(true);
    }
    sqlx::query_scalar!(
        r#"SELECT EXISTS (SELECT 1 FROM track_clips WHERE track_id = $1 AND state = 'ready') AS "e!""#,
        track_id
    )
    .fetch_one(db)
    .await
}

/// Вернуть скрытый клип трека.
pub async fn unhide(db: &PgPool, track_id: Uuid, user_id: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query!(
        "DELETE FROM clip_hidden WHERE track_id = $1 AND user_id = $2",
        track_id,
        user_id
    )
    .execute(db)
    .await?;
    Ok(())
}

/// Строка «Спорных»: трек, ролик, сколько скрыли и когда последний раз.
#[derive(Debug, Clone)]
pub struct Reported {
    pub track_id: Uuid,
    pub video_id: String,
    pub video_title: Option<String>,
    pub channel: Option<String>,
    pub reports: i64,
    pub last_at: chrono::DateTime<Utc>,
}

/// Треки, где скрыли нынешний клип, а администратор после этого его не подтверждал и не менял.
pub async fn reported(db: &PgPool, limit: i64) -> Result<Vec<Reported>, sqlx::Error> {
    sqlx::query_as!(
        Reported,
        r#"SELECT c.track_id, h.video_id, c.video_title, c.channel,
                  count(*) AS "reports!", max(h.created_at) AS "last_at!"
           FROM clip_hidden h
           JOIN track_clips c ON c.track_id = h.track_id AND c.video_id = h.video_id AND c.state = 'ready'
           LEFT JOIN track_clip_pins p ON p.track_id = h.track_id
           WHERE p.chosen_at IS NULL OR h.created_at > p.chosen_at
           GROUP BY c.track_id, h.video_id, c.video_title, c.channel
           ORDER BY count(*) DESC, max(h.created_at) DESC
           LIMIT $1"#,
        limit,
    )
    .fetch_all(db)
    .await
}

/// Перепроверка клипов, нарезанных старым отбором (брал «Music Video» с любого канала): понемногу,
/// в хвосте очереди, сначала недавно показанные. Сколько поставлено.
pub async fn review(db: &PgPool) -> Result<u64, sqlx::Error> {
    let ids = sqlx::query_scalar!(
        r#"SELECT c.track_id FROM track_clips c
           WHERE c.state = 'ready' AND c.reviewed_at IS NULL
             AND NOT EXISTS (SELECT 1 FROM track_clip_pins p WHERE p.track_id = c.track_id)
             AND NOT EXISTS (SELECT 1 FROM jobs j WHERE j.dedupe_key = 'clip:' || c.track_id::text
                                AND j.status IN ('queued', 'running'))
           ORDER BY c.last_used_at DESC
           LIMIT GREATEST(0, $1 - (SELECT count(*) FROM jobs
                                   WHERE lane = $2 AND status IN ('queued', 'running')
                                     AND payload->>'mode' = 'review'))"#,
        REVIEW_BATCH,
        LANE,
    )
    .fetch_all(db)
    .await?;
    let mut conn = db.acquire().await?;
    for &track_id in &ids {
        jobs::enqueue(
            &mut conn,
            NewJob::new(
                kind::CLIP,
                LANE,
                ClipPayload {
                    track_id,
                    mode: ClipMode::Review,
                },
            )
            .dedupe(dedupe(track_id))
            .priority(REVIEW_PRIORITY),
        )
        .await?;
    }
    Ok(ids.len() as u64)
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Trimmed {
    pub expired: u64,
    pub evicted: u64,
}

/// Чистка: давно не показанные клипы и всё сверх предела (сначала не из любимых, по давности
/// показа). Файлы стирает чистка хранилища ([`crate::renditions::trim`]) по `storage_trash`.
/// Выбор администратора живёт отдельно (`track_clip_pins`): ушедший клип нарежется из того же ролика.
pub async fn trim(
    db: &PgPool,
    settings: &super::clip_settings::Settings,
) -> Result<Trimmed, sqlx::Error> {
    let limit = settings.max_bytes;
    let expired = sqlx::query!(
        "DELETE FROM track_clips c WHERE c.state = 'ready' AND c.last_used_at < now() - make_interval(days =>
             CASE WHEN EXISTS (SELECT 1 FROM liked_tracks l WHERE l.track_id = c.track_id) THEN $1::int ELSE $2::int END)",
        settings.keep_liked_days,
        settings.keep_days,
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
