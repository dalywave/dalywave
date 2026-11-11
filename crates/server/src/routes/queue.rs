//! Разделы «Обзор» и «Загрузки» админки: состояние сервера, очередь по полосам, повтор и отмена задач,
//! пауза и настройки полос. Весь роутер закрыт правом admin (см. `routes::router`).

use axum::{
    Router,
    extract::State,
    routing::{get, patch, post},
};
use dalywave_protocol::{
    ErrorBody,
    catalog::AudioStatus,
    jobs::{
        AcquisitionStatus, AdminOverview, DayStats, Job, JobPage, JobsQuery, Lane, LaneList,
        LanePatch, LibraryStats, StatusCount, StorageStats, kind,
    },
};
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    acquire, audit,
    auth::Principal,
    error::{AppError, AppResult, bad},
    extract::{Json, Path, Query},
    jobs::{self, JobRow},
    state::AppState,
};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/overview", get(overview))
        .route("/jobs", get(list_jobs))
        .route("/jobs/{id}/retry", post(retry_job))
        .route("/jobs/{id}/cancel", post(cancel_job))
        .route("/lanes", get(list_lanes))
        .route("/lanes/{lane}", patch(update_lane))
}

/// Полосы со счётчиками в порядке важности.
pub async fn lanes(db: &PgPool) -> Result<Vec<Lane>, sqlx::Error> {
    let rows = sqlx::query!(
        r#"SELECT l.lane, l.title, l.paused, l.concurrency, l.min_interval_ms, l.updated_at,
                  count(j.id) FILTER (WHERE j.status = 'queued') AS "queued!",
                  count(j.id) FILTER (WHERE j.status = 'running') AS "running!",
                  count(j.id) FILTER (WHERE j.status = 'failed') AS "failed!",
                  count(j.id) FILTER (WHERE j.status = 'done' AND j.finished_at > now() - interval '24 hours') AS "done_24h!"
           FROM job_lanes l LEFT JOIN jobs j ON j.lane = l.lane
           GROUP BY l.lane
           ORDER BY array_position(ARRAY['now', 'wave', 'library', 'discography', 'discovery', 'analysis',
                                         'scout', 'mirror', 'maintenance'], l.lane) NULLS LAST, l.lane"#,
    )
    .fetch_all(db)
    .await?;
    Ok(rows
        .into_iter()
        .map(|r| Lane {
            lane: r.lane,
            title: r.title,
            paused: r.paused,
            concurrency: r.concurrency,
            min_interval_ms: r.min_interval_ms,
            queued: r.queued,
            running: r.running,
            failed: r.failed,
            done_24h: r.done_24h,
            updated_at: r.updated_at,
        })
        .collect())
}

#[utoipa::path(
    get,
    path = "/api/admin/overview",
    tag = "admin",
    security(("session" = []), ("device" = [])),
    responses(
        (status = 200, description = "Размер библиотеки, записи по состоянию аудио, хранилище, очередь, сутки", body = AdminOverview),
        (status = 403, description = "Нет права admin", body = ErrorBody),
    ),
)]
pub async fn overview(State(st): State<AppState>) -> AppResult<Json<AdminOverview>> {
    let lib = sqlx::query!(
        r#"SELECT
              (SELECT count(*) FROM artists) AS "artists!",
              (SELECT count(*) FROM albums) AS "albums!",
              (SELECT count(*) FROM tracks) AS "tracks!",
              (SELECT count(*) FROM recordings) AS "recordings!",
              (SELECT count(*) FROM liked_tracks) AS "liked!",
              (SELECT count(*) FROM saved_albums) AS "saved!",
              (SELECT count(*) FROM followed_artists) AS "followed!",
              (SELECT count(*) FROM playlists) AS "playlists!""#,
    )
    .fetch_one(&st.db)
    .await?;
    let by_status = sqlx::query!(
        r#"SELECT audio_status, count(*) AS "n!" FROM recordings GROUP BY audio_status"#
    )
    .fetch_all(&st.db)
    .await?;
    let recordings_by_status = AudioStatus::ALL
        .iter()
        .map(|s| StatusCount {
            status: *s,
            count: by_status
                .iter()
                .find(|r| r.audio_status == s.as_str())
                .map_or(0, |r| r.n),
        })
        .collect();
    let storage = sqlx::query!(
        r#"SELECT
              (SELECT count(*) FROM audio_files) AS "audio_files!",
              (SELECT COALESCE(sum(size_bytes), 0) FROM audio_files)::bigint AS "audio_bytes!",
              ((SELECT count(*) FROM albums WHERE cover_key IS NOT NULL)
               + (SELECT count(*) FROM artists WHERE picture_key IS NOT NULL)) AS "images!",
              ((SELECT COALESCE(sum(cover_bytes), 0) FROM albums)
               + (SELECT COALESCE(sum(picture_bytes), 0) FROM artists))::bigint AS "image_bytes!""#,
    )
    .fetch_one(&st.db)
    .await?;
    let day = sqlx::query!(
        r#"SELECT
              (SELECT count(*) FROM jobs WHERE status = 'done' AND finished_at > now() - interval '24 hours') AS "jobs_done!",
              (SELECT count(*) FROM jobs WHERE status = 'failed' AND finished_at > now() - interval '24 hours') AS "jobs_failed!",
              (SELECT count(*) FROM acquisitions WHERE status = 'ready' AND finished_at > now() - interval '24 hours') AS "acq_ready!",
              (SELECT count(*) FROM acquisitions WHERE status IN ('failed', 'unavailable', 'disputed')
                 AND finished_at > now() - interval '24 hours') AS "acq_failed!",
              (SELECT count(*) FROM play_events WHERE created_at > now() - interval '24 hours') AS "plays!",
              (SELECT count(*) FROM liked_tracks WHERE created_at > now() - interval '24 hours') AS "likes!",
              (SELECT count(*) FROM tracks WHERE created_at > now() - interval '24 hours') AS "new_tracks!""#,
    )
    .fetch_one(&st.db)
    .await?;
    Ok(Json(AdminOverview {
        library: LibraryStats {
            artists: lib.artists,
            albums: lib.albums,
            tracks: lib.tracks,
            recordings: lib.recordings,
            liked_tracks: lib.liked,
            saved_albums: lib.saved,
            followed_artists: lib.followed,
            playlists: lib.playlists,
        },
        recordings_by_status,
        storage: StorageStats {
            audio_files: storage.audio_files,
            audio_bytes: storage.audio_bytes,
            images: storage.images,
            image_bytes: storage.image_bytes,
        },
        lanes: lanes(&st.db).await?,
        last_24h: DayStats {
            jobs_done: day.jobs_done,
            jobs_failed: day.jobs_failed,
            acquisitions_ready: day.acq_ready,
            acquisitions_failed: day.acq_failed,
            plays: day.plays,
            likes: day.likes,
            new_tracks: day.new_tracks,
        },
    }))
}

#[utoipa::path(
    get,
    path = "/api/admin/jobs",
    tag = "admin",
    security(("session" = []), ("device" = [])),
    params(JobsQuery),
    responses(
        (status = 200, description = "Задачи, новые первыми", body = JobPage),
        (status = 400, description = "Неверный cursor или limit", body = ErrorBody),
    ),
)]
pub async fn list_jobs(
    State(st): State<AppState>,
    Query(q): Query<JobsQuery>,
) -> AppResult<Json<JobPage>> {
    let limit = crate::cursor::limit(q.limit, 50, 200)?;
    let cursor = match q.cursor.as_deref().filter(|c| !c.is_empty()) {
        Some(c) => Some(c.parse::<Uuid>().map_err(|_| bad("неверный cursor"))?),
        None => None,
    };
    let mut rows = sqlx::query_as!(
        JobRow,
        "SELECT id, kind, lane, priority, payload, status, attempts, max_attempts, run_after, locked_by,
                heartbeat_at, last_error, result, dedupe_key, created_at, updated_at, finished_at
         FROM jobs
         WHERE ($1::text IS NULL OR lane = $1) AND ($2::text IS NULL OR status = $2)
           AND ($3::text IS NULL OR kind = $3) AND ($4::uuid IS NULL OR id < $4)
         ORDER BY id DESC
         LIMIT $5",
        q.lane,
        q.status.map(|s| s.as_str()),
        q.kind,
        cursor,
        limit + 1,
    )
    .fetch_all(&st.db)
    .await?;
    let next_cursor = if rows.len() > limit as usize {
        rows.truncate(limit as usize);
        rows.last().map(|r| r.id.to_string())
    } else {
        None
    };
    Ok(Json(JobPage {
        items: rows.into_iter().map(JobRow::view).collect(),
        next_cursor,
    }))
}

async fn job_exists(db: &PgPool, id: Uuid) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar!(
        r#"SELECT EXISTS (SELECT 1 FROM jobs WHERE id = $1) AS "e!""#,
        id
    )
    .fetch_one(db)
    .await
}

#[utoipa::path(
    post,
    path = "/api/admin/jobs/{id}/retry",
    tag = "admin",
    security(("session" = []), ("device" = [])),
    params(("id" = Uuid, Path, description = "id задачи")),
    responses(
        (status = 200, description = "Задача снова в очереди, попытки с нуля", body = Job),
        (status = 404, description = "Нет такой задачи", body = ErrorBody),
        (status = 409, description = "Задача ещё в работе (`job_active`) или такая же уже стоит в очереди (`job_duplicate`)", body = ErrorBody),
    ),
)]
pub async fn retry_job(
    State(st): State<AppState>,
    principal: Principal,
    Path(id): Path<Uuid>,
) -> AppResult<Json<Job>> {
    let mut tx = st.db.begin().await?;
    let job = match jobs::retry(&mut tx, id).await {
        Ok(job) => job,
        Err(e)
            if e.as_database_error()
                .is_some_and(|d| d.is_unique_violation()) =>
        {
            return Err(AppError::Conflict {
                code: "job_duplicate",
                message: "такая же задача уже стоит в очереди".into(),
            });
        }
        Err(e) => return Err(e.into()),
    };
    let Some(job) = job else {
        return Err(if job_exists(&st.db, id).await? {
            AppError::Conflict {
                code: "job_active",
                message: "задача ещё в очереди или в работе".into(),
            }
        } else {
            AppError::NotFound
        });
    };
    audit::record(
        &mut *tx,
        Some(principal.user.id),
        audit::JOB_RETRIED,
        Some(job.id.to_string()),
        json!({ "kind": job.kind, "lane": job.lane }),
    )
    .await?;
    tx.commit().await?;
    Ok(Json(job.view()))
}

#[utoipa::path(
    post,
    path = "/api/admin/jobs/{id}/cancel",
    tag = "admin",
    security(("session" = []), ("device" = [])),
    params(("id" = Uuid, Path, description = "id задачи")),
    responses(
        (status = 200, description = "Задача отменена; идущую исполнитель прервёт при следующем пульсе", body = Job),
        (status = 404, description = "Нет такой задачи", body = ErrorBody),
        (status = 409, description = "Задача уже закончилась (`job_finished`)", body = ErrorBody),
    ),
)]
pub async fn cancel_job(
    State(st): State<AppState>,
    principal: Principal,
    Path(id): Path<Uuid>,
) -> AppResult<Json<Job>> {
    let mut tx = st.db.begin().await?;
    let Some(job) = jobs::cancel(&mut tx, id).await? else {
        return Err(if job_exists(&st.db, id).await? {
            AppError::Conflict {
                code: "job_finished",
                message: "задача уже закончилась".into(),
            }
        } else {
            AppError::NotFound
        });
    };
    audit::record(
        &mut *tx,
        Some(principal.user.id),
        audit::JOB_CANCELLED,
        Some(job.id.to_string()),
        json!({ "kind": job.kind, "lane": job.lane }),
    )
    .await?;
    tx.commit().await?;
    // Отменённое скачивание: запись снова «скачается при запуске»
    if job.kind == kind::ACQUIRE
        && let Ok(payload) = job.payload::<dalywave_protocol::jobs::AcquirePayload>()
    {
        let mut conn = st.db.acquire().await?;
        acquire::set_status(
            &mut conn,
            payload.acquisition_id,
            AcquisitionStatus::Cancelled,
            None,
        )
        .await?;
        acquire::set_audio_status(&mut conn, payload.recording_id, AudioStatus::None).await?;
    }
    Ok(Json(job.view()))
}

#[utoipa::path(
    get,
    path = "/api/admin/lanes",
    tag = "admin",
    security(("session" = []), ("device" = [])),
    responses(
        (status = 200, description = "Полосы очереди со счётчиками", body = LaneList),
        (status = 403, description = "Нет права admin", body = ErrorBody),
    ),
)]
pub async fn list_lanes(State(st): State<AppState>) -> AppResult<Json<LaneList>> {
    Ok(Json(LaneList {
        items: lanes(&st.db).await?,
    }))
}

#[utoipa::path(
    patch,
    path = "/api/admin/lanes/{lane}",
    tag = "admin",
    security(("session" = []), ("device" = [])),
    params(("lane" = String, Path, description = "Имя полосы, например discography")),
    request_body = LanePatch,
    responses(
        (status = 200, description = "Полоса после изменения", body = Lane),
        (status = 400, description = "Параллельность 1–16, пауза 0–3 600 000 мс", body = ErrorBody),
        (status = 404, description = "Нет такой полосы", body = ErrorBody),
    ),
)]
pub async fn update_lane(
    State(st): State<AppState>,
    principal: Principal,
    Path(lane): Path<String>,
    Json(patch): Json<LanePatch>,
) -> AppResult<Json<Lane>> {
    if patch.concurrency.is_some_and(|c| !(1..=16).contains(&c)) {
        return Err(bad("параллельность — от 1 до 16"));
    }
    if patch
        .min_interval_ms
        .is_some_and(|m| !(0..=3_600_000).contains(&m))
    {
        return Err(bad("пауза между задачами — от 0 до 3 600 000 мс"));
    }
    let mut tx = st.db.begin().await?;
    let updated = sqlx::query!(
        "UPDATE job_lanes SET paused = COALESCE($2, paused), concurrency = COALESCE($3, concurrency),
                min_interval_ms = COALESCE($4, min_interval_ms), updated_at = now()
         WHERE lane = $1
         RETURNING lane",
        lane,
        patch.paused,
        patch.concurrency,
        patch.min_interval_ms,
    )
    .fetch_optional(&mut *tx)
    .await?;
    if updated.is_none() {
        return Err(AppError::NotFound);
    }
    audit::record(
        &mut *tx,
        Some(principal.user.id),
        audit::LANE_CHANGED,
        Some(lane.clone()),
        json!({ "paused": patch.paused, "concurrency": patch.concurrency, "minIntervalMs": patch.min_interval_ms }),
    )
    .await?;
    // Снятая пауза: воркер берёт задачи сразу, а не по таймеру
    sqlx::query("SELECT pg_notify($1, $2)")
        .bind(jobs::CHANNEL)
        .bind(&lane)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    lanes(&st.db)
        .await?
        .into_iter()
        .find(|l| l.lane == lane)
        .map(Json)
        .ok_or(AppError::NotFound)
}
