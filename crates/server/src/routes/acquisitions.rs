//! Разделы «Загрузки» и «Спорные» админки: скачивания с кандидатами и разбором оценки, решения
//! по спорным записям. Весь роутер закрыт правом admin (см. `routes::router`).

use std::collections::HashMap;

use axum::{
    Router,
    extract::State,
    routing::{get, post},
};
use chrono::{DateTime, Utc};
use dalywave_protocol::{
    ErrorBody,
    catalog::AudioStatus,
    jobs::{
        AcquirePayload, AcquireReason, Acquisition, AcquisitionAttempt, AcquisitionCandidate,
        AcquisitionStatus, AcquisitionsQuery, AdminAcquisition, AdminAcquisitionPage,
        AttemptOutcome, JobStatus, ResolveAction, ResolveRequest,
    },
};
use serde_json::{Value, json};
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    acquire::{self, Enqueue, MANUAL_PRIORITY, Request},
    audit,
    auth::Principal,
    catalog::views,
    cursor, downloaders,
    error::{AppError, AppResult, bad},
    extract::{Json, Path, Query},
    jobs::{self, lanes},
    state::AppState,
};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/acquisitions", get(list))
        .route("/acquisitions/{id}", get(one))
        .route("/acquisitions/{id}/resolve", post(resolve))
}

struct Row {
    id: Uuid,
    recording_id: Uuid,
    track_id: Uuid,
    status: String,
    reason: String,
    lane: String,
    error: Option<String>,
    attempts: i16,
    job_id: Option<Uuid>,
    requested_by: Option<Uuid>,
    worker: Option<String>,
    candidates: Option<Value>,
    chosen: Option<Value>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    finished_at: Option<DateTime<Utc>>,
    audio_status: String,
    job_status: Option<String>,
    job_payload: Option<Value>,
}

/// Скачивания с треком, кандидатами, решением и попытками — в порядке `rows`.
async fn views_of(st: &AppState, rows: Vec<Row>, viewer: Uuid) -> AppResult<Vec<AdminAcquisition>> {
    let track_ids: Vec<Uuid> = rows.iter().map(|r| r.track_id).collect();
    let tracks: HashMap<Uuid, _> = views::tracks(&st.db, &track_ids, viewer)
        .await?
        .into_iter()
        .map(|t| (t.id, t))
        .collect();
    let ids: Vec<Uuid> = rows.iter().map(|r| r.id).collect();
    let attempts = sqlx::query!(
        "SELECT id, acquisition_id, executor, status, outcome, error_code, error, video_id, yt_dlp, total_ms,
                created_at
         FROM acquisition_attempts WHERE acquisition_id = ANY($1)
         ORDER BY created_at DESC, id DESC",
        &ids,
    )
    .fetch_all(&st.db)
    .await?;
    let mut log: HashMap<Uuid, Vec<AcquisitionAttempt>> = HashMap::new();
    for a in attempts {
        log.entry(a.acquisition_id)
            .or_default()
            .push(AcquisitionAttempt {
                id: a.id,
                executor: a.executor,
                status: a.status,
                outcome: a.outcome.parse().unwrap_or(AttemptOutcome::Failed),
                error_code: a.error_code,
                error: a.error,
                video_id: a.video_id,
                yt_dlp_version: a.yt_dlp,
                total_ms: a.total_ms,
                created_at: a.created_at,
            });
    }
    let mut conn = st.db.acquire().await?;
    let online = downloaders::online(&mut *conn).await?;
    let mut out = Vec::with_capacity(rows.len());
    for r in rows {
        let status = r.status.parse().unwrap_or(AcquisitionStatus::Failed);
        let queue_position = match (status.is_active(), r.job_id) {
            (true, Some(job)) => acquire::queue_position(&mut conn, job).await?,
            _ => None,
        };
        let payload = r
            .job_payload
            .and_then(|p| serde_json::from_value::<AcquirePayload>(p).ok());
        let candidates: Vec<AcquisitionCandidate> = r
            .candidates
            .and_then(|c| serde_json::from_value(c).ok())
            .unwrap_or_default();
        out.push(AdminAcquisition {
            acquisition: Acquisition {
                id: r.id,
                recording_id: r.recording_id,
                track_id: r.track_id,
                status,
                audio_status: r.audio_status.parse().unwrap_or(AudioStatus::None),
                reason: r.reason.parse().unwrap_or(AcquireReason::Manual),
                lane: r.lane,
                error: r.error,
                attempts: r.attempts,
                queue_position,
                downloaders_online: online,
                waiting_for_downloader: status.is_active()
                    && online == 0
                    && !st.config.fetcher.server,
                created_at: r.created_at,
                updated_at: r.updated_at,
                finished_at: r.finished_at,
            },
            track: tracks.get(&r.track_id).cloned(),
            requested_by: r.requested_by,
            worker: r.worker,
            job_id: r.job_id,
            job_status: r.job_status.and_then(|s| s.parse::<JobStatus>().ok()),
            prefer_video_ids: payload
                .as_ref()
                .map(|p| p.prefer_video_ids.clone())
                .unwrap_or_default(),
            direct_url: payload.and_then(|p| p.direct_url),
            candidates,
            chosen: r.chosen,
            attempts_log: log.remove(&r.id).unwrap_or_default(),
        });
    }
    Ok(out)
}

pub(crate) async fn admin_view(
    st: &AppState,
    id: Uuid,
    viewer: Uuid,
) -> AppResult<AdminAcquisition> {
    let rows = fetch(&st.db, None, None, None, Some(id), 1).await?;
    views_of(st, rows, viewer)
        .await?
        .into_iter()
        .next()
        .ok_or(AppError::NotFound)
}

async fn fetch(
    db: &PgPool,
    status: Option<AcquisitionStatus>,
    active: Option<bool>,
    after: Option<(DateTime<Utc>, Uuid)>,
    only: Option<Uuid>,
    limit: i64,
) -> Result<Vec<Row>, sqlx::Error> {
    sqlx::query_as!(
        Row,
        r#"SELECT a.id, a.recording_id, a.track_id, a.status, a.reason, a.lane, a.error, a.attempts, a.job_id,
                  a.requested_by, a.worker, a.candidates, a.chosen, a.created_at, a.updated_at, a.finished_at,
                  r.audio_status, j.status AS "job_status?", j.payload AS "job_payload?"
           FROM acquisitions a
           JOIN recordings r ON r.id = a.recording_id
           LEFT JOIN jobs j ON j.id = a.job_id
           WHERE ($1::text IS NULL OR a.status = $1)
             AND ($2::timestamptz IS NULL OR (a.updated_at, a.id) < ($2, $3))
             AND ($4::uuid IS NULL OR a.id = $4)
             AND ($6::bool IS NULL OR (a.status IN ('queued', 'matching', 'downloading', 'verifying', 'processing')) = $6)
           ORDER BY a.updated_at DESC, a.id DESC
           LIMIT $5"#,
        status.map(|s| s.as_str()),
        after.map(|(at, _)| at),
        after.map(|(_, id)| id).unwrap_or_default(),
        only,
        limit,
        active,
    )
    .fetch_all(db)
    .await
}

#[utoipa::path(
    get,
    path = "/api/admin/acquisitions",
    tag = "admin",
    security(("session" = []), ("device" = [])),
    params(AcquisitionsQuery),
    responses(
        (status = 200, description = "Скачивания, недавние первыми: трек, кандидаты с разбором оценки, попытки", body = AdminAcquisitionPage),
        (status = 400, description = "Неверный cursor или limit", body = ErrorBody),
    ),
)]
pub async fn list(
    State(st): State<AppState>,
    principal: Principal,
    Query(q): Query<AcquisitionsQuery>,
) -> AppResult<Json<AdminAcquisitionPage>> {
    let limit = cursor::limit(q.limit, 50, 200)?;
    let after = cursor::decode_opt(q.cursor.as_deref())?;
    let mut rows = fetch(&st.db, q.status, q.active, after, None, limit + 1).await?;
    let next_cursor = cursor::page(&mut rows, limit, |r| (r.updated_at, r.id));
    Ok(Json(AdminAcquisitionPage {
        items: views_of(&st, rows, principal.user.id).await?,
        next_cursor,
    }))
}

#[utoipa::path(
    get,
    path = "/api/admin/acquisitions/{id}",
    tag = "admin",
    security(("session" = []), ("device" = [])),
    params(("id" = Uuid, Path, description = "id скачивания")),
    responses(
        (status = 200, description = "Скачивание: трек, кандидаты с разбором оценки, попытки", body = AdminAcquisition),
        (status = 404, description = "Нет такого скачивания", body = ErrorBody),
    ),
)]
pub async fn one(
    State(st): State<AppState>,
    principal: Principal,
    Path(id): Path<Uuid>,
) -> AppResult<Json<AdminAcquisition>> {
    Ok(Json(admin_view(&st, id, principal.user.id).await?))
}

/// id видео YouTube: 11 символов из букв, цифр, `-` и `_`.
pub(crate) fn valid_video_id(id: &str) -> bool {
    id.len() == 11
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// Что за ссылку вставили: видео YouTube (его id) или что-то, что fetcher скачает по адресу.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Pasted {
    Video(String),
    Direct(String),
}

pub(crate) fn parse_pasted(raw: &str) -> AppResult<Pasted> {
    let raw = raw.trim();
    let url = url::Url::parse(raw).map_err(|_| bad("это не ссылка"))?;
    if !matches!(url.scheme(), "http" | "https") || raw.len() > 2000 {
        return Err(bad("нужна ссылка http(s) не длиннее 2000 символов"));
    }
    let host = url.host_str().unwrap_or_default().to_ascii_lowercase();
    let host = host.trim_start_matches("www.").trim_start_matches("m.");
    let video = match host {
        "youtube.com" | "music.youtube.com" => {
            let from_query = url
                .query_pairs()
                .find(|(k, _)| k == "v")
                .map(|(_, v)| v.to_string());
            let mut segments = url.path_segments().into_iter().flatten();
            let from_path = match segments.next() {
                Some("shorts" | "embed" | "live") => segments.next().map(str::to_string),
                _ => None,
            };
            from_query.or(from_path)
        }
        "youtu.be" => url
            .path_segments()
            .and_then(|mut s| s.next())
            .map(str::to_string),
        _ => None,
    };
    match video {
        Some(id) if valid_video_id(&id) => Ok(Pasted::Video(id)),
        Some(_) => Err(bad("в ссылке YouTube нет id видео")),
        None => Ok(Pasted::Direct(url.to_string())),
    }
}

#[utoipa::path(
    post,
    path = "/api/admin/acquisitions/{id}/resolve",
    tag = "admin",
    security(("session" = []), ("device" = [])),
    params(("id" = Uuid, Path, description = "id скачивания")),
    request_body = ResolveRequest,
    responses(
        (status = 200, description = "`choose`, `url`, `retry` — новое скачивание вне очереди; `unavailable` — это же скачивание, \
                                      запись помечена «нет в источниках»", body = AdminAcquisition),
        (status = 400, description = "Нет или неверный `videoId` / `url`", body = ErrorBody),
        (status = 404, description = "Нет такого скачивания", body = ErrorBody),
        (status = 409, description = "Скачивание записи ещё идёт (`acquisition_active`)", body = ErrorBody),
    ),
)]
pub async fn resolve(
    State(st): State<AppState>,
    principal: Principal,
    Path(id): Path<Uuid>,
    Json(req): Json<ResolveRequest>,
) -> AppResult<Json<AdminAcquisition>> {
    let acq = sqlx::query!(
        "SELECT track_id, recording_id, status, job_id FROM acquisitions WHERE id = $1",
        id
    )
    .fetch_optional(&st.db)
    .await?
    .ok_or(AppError::NotFound)?;
    let active = acq
        .status
        .parse::<AcquisitionStatus>()
        .is_ok_and(AcquisitionStatus::is_active);
    let admin = principal.user.id;
    let audit_payload = json!({ "action": req.action, "videoId": req.video_id, "url": req.url });

    if req.action == ResolveAction::Unavailable {
        let mut tx = st.db.begin().await?;
        if active && let Some(job) = acq.job_id {
            jobs::cancel(&mut tx, job).await?;
        }
        sqlx::query!(
            "UPDATE acquisitions SET status = 'unavailable', error = 'в админке: записи нет в источниках',
                    updated_at = now(), finished_at = now()
             WHERE id = $1",
            id
        )
        .execute(&mut *tx)
        .await?;
        acquire::set_audio_status_unless_ready(&mut tx, acq.recording_id, AudioStatus::Unavailable)
            .await?;
        acquire::publish(&mut tx, id).await?;
        acquire::recording_changed(&mut tx, acq.recording_id).await?;
        audit::record(
            &mut *tx,
            Some(admin),
            audit::ACQUISITION_RESOLVED,
            Some(id.to_string()),
            audit_payload,
        )
        .await?;
        tx.commit().await?;
        return Ok(Json(admin_view(&st, id, admin).await?));
    }

    if active {
        return Err(AppError::Conflict {
            code: "acquisition_active",
            message: "скачивание этой записи ещё идёт: дождитесь его или отмените задачу".into(),
        });
    }
    let mut request = Request::new(acq.track_id, AcquireReason::Manual, lanes::NOW)
        .by(admin)
        .priority(MANUAL_PRIORITY);
    request.force = true;
    match req.action {
        ResolveAction::Choose => {
            let video = req
                .video_id
                .as_deref()
                .map(str::trim)
                .filter(|v| valid_video_id(v))
                .ok_or_else(|| bad("нужен videoId — id видео YouTube из 11 символов"))?;
            request.prefer_video_ids = vec![video.to_string()];
        }
        ResolveAction::Url => {
            let raw = req.url.as_deref().ok_or_else(|| bad("нужна ссылка url"))?;
            match parse_pasted(raw)? {
                Pasted::Video(video) => request.prefer_video_ids = vec![video],
                Pasted::Direct(url) => request.direct_url = Some(url),
            }
        }
        ResolveAction::Retry | ResolveAction::Unavailable => {}
    }
    let new_id = match acquire::enqueue(&st.db, request).await? {
        Enqueue::Queued { acquisition_id, .. } => acquisition_id,
        Enqueue::Active { .. } => {
            return Err(AppError::Conflict {
                code: "acquisition_active",
                message: "у этой записи уже идёт другое скачивание".into(),
            });
        }
        Enqueue::Ready | Enqueue::Skipped(_) => {
            return Err(bad(
                "трек нельзя поставить на скачивание: у него нет записи Deezer",
            ));
        }
    };
    // Решённое уходит из «Спорных»: дальше за запись отвечает новое скачивание
    let note = match req.action {
        ResolveAction::Choose => "в админке выбрано другое видео",
        ResolveAction::Url => "в админке вставлена ссылка",
        ResolveAction::Retry | ResolveAction::Unavailable => "в админке поставлено заново",
    };
    let mut tx = st.db.begin().await?;
    sqlx::query!(
        "UPDATE acquisitions SET status = 'cancelled', error = $2, updated_at = now()
         WHERE id = $1 AND status IN ('disputed', 'failed', 'unavailable')",
        id,
        note,
    )
    .execute(&mut *tx)
    .await?;
    acquire::publish(&mut tx, id).await?;
    audit::record(
        &mut *tx,
        Some(admin),
        audit::ACQUISITION_RESOLVED,
        Some(id.to_string()),
        json!({ "action": req.action, "videoId": req.video_id, "url": req.url, "acquisitionId": new_id }),
    )
    .await?;
    tx.commit().await?;
    Ok(Json(admin_view(&st, new_id, admin).await?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pasted_links() {
        let video = |s: &str| parse_pasted(s).unwrap();
        assert_eq!(
            video("https://www.youtube.com/watch?v=afkL3uLNeXA&t=10"),
            Pasted::Video("afkL3uLNeXA".into())
        );
        assert_eq!(
            video("https://music.youtube.com/watch?v=afkL3uLNeXA&si=x"),
            Pasted::Video("afkL3uLNeXA".into())
        );
        assert_eq!(
            video("https://youtu.be/afkL3uLNeXA?si=abc"),
            Pasted::Video("afkL3uLNeXA".into())
        );
        assert_eq!(
            video("https://youtube.com/shorts/afkL3uLNeXA"),
            Pasted::Video("afkL3uLNeXA".into())
        );
        assert_eq!(
            video("https://soundcloud.com/artist/track"),
            Pasted::Direct("https://soundcloud.com/artist/track".into())
        );
        assert!(parse_pasted("https://www.youtube.com/watch?v=short").is_err());
        assert!(parse_pasted("ftp://example.com/a.mp3").is_err());
        assert!(parse_pasted("не ссылка").is_err());
        assert!(valid_video_id("afkL3uLNeXA"));
        assert!(!valid_video_id("afkL3uLNeX!"));
    }
}
