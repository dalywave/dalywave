//! API домашнего загрузчика (`fetcher worker` на Mac): задание, продление аренды, отчёт,
//! возврат брошенного задания. Вход — `Authorization: Bearer dwk_…` ([`DownloaderAuth`]) с единственным
//! правом `downloader`; сессии людей и токены приложений сюда не подходят.

use std::time::{Duration, Instant};

use axum::{
    Router,
    extract::State,
    http::{StatusCode, header},
    response::{IntoResponse, Response},
    routing::post,
};
use dalywave_protocol::{
    ErrorBody,
    fetcher::{ClaimRequest, ClaimResponse, Lease, LeaseRequest, ReleaseRequest, ResultReport},
    jobs::{AcquirePayload, JobStatus, kind},
};
use serde_json::Value;
use uuid::Uuid;

use crate::{
    acquire::{
        self, Claimed,
        result::{self, REMOTE_POSTPONE},
    },
    downloaders::{self, DownloaderAuth},
    error::{AppError, AppResult, bad},
    extract::{Json, JsonOrDefault, Path},
    jobs,
    state::{AppState, ClientIp},
};

/// Без сигнала NOTIFY очередь перепроверяется так часто.
const RECHECK: Duration = Duration::from_secs(1);
/// После пустого ответа загрузчик спрашивает снова почти сразу: ожидание уже прошло на сервере.
const RETRY_AFTER_S: &str = "1";

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/fetcher/jobs/claim", post(claim))
        .route("/fetcher/jobs/{id}/lease", post(renew_lease))
        .route("/fetcher/jobs/{id}/result", post(report))
        .route("/fetcher/jobs/{id}/release", post(release))
}

fn not_yours() -> AppError {
    AppError::Conflict {
        code: "job_not_yours",
        message: "задание уже не ваше: аренда кончилась или его отменили".into(),
    }
}

#[utoipa::path(
    post,
    path = "/api/fetcher/jobs/claim",
    tag = "fetcher",
    security(("downloader" = [])),
    request_body = ClaimRequest,
    responses(
        (status = 200, description = "Задание: трек Deezer, подписанная ссылка для файла, срок аренды", body = ClaimResponse),
        (status = 204, description = "Заданий нет: сервер подождал до 25 секунд. `Retry-After` — через сколько секунд спросить снова"),
        (status = 401, description = "Токен загрузчика не принят", body = ErrorBody),
    ),
)]
pub async fn claim(
    State(st): State<AppState>,
    downloader: DownloaderAuth,
    ClientIp(ip): ClientIp,
    Json(req): Json<ClaimRequest>,
) -> AppResult<Response> {
    let worker_id = req.worker_id.trim();
    if worker_id.is_empty() || worker_id.chars().count() > 100 {
        return Err(bad("workerId — от 1 до 100 символов"));
    }
    downloaders::seen(&st.db, downloader.id, ip, Some(&req)).await?;
    let kinds = &req.capabilities.kinds;
    if kinds.is_empty() || kinds.iter().any(|k| k == kind::ACQUIRE) {
        let executor = downloader.executor();
        // Кончившиеся аренды — снова в очередь, не дожидаясь сборщика воркера
        acquire::reap_leases(&st.db).await?;
        let deadline = Instant::now() + st.config.fetcher.claim_wait;
        let mut stopping = st.hub.stopping();
        loop {
            // Подписка на сигнал — до проверки очереди: задача, поставленная между ними, не потеряется
            let changed = st.hub.jobs_changed();
            tokio::pin!(changed);
            changed.as_mut().enable();
            let soonest = match acquire::claim_for_downloader(
                &st.db,
                &st.storage,
                &executor,
                st.config.fetcher.lease,
            )
            .await?
            {
                Claimed::Job(job) => {
                    tracing::info!(job = %job.id, downloader = %downloader.name, deezer_track_id = job.deezer_track_id, "job handed to a downloader");
                    return Ok(Json(ClaimResponse { job: *job }).into_response());
                }
                Claimed::Nothing(soonest) => soonest,
            };
            let now = Instant::now();
            if now >= deadline || *stopping.borrow() {
                break;
            }
            let wait = soonest
                .unwrap_or(RECHECK)
                .clamp(Duration::from_millis(20), RECHECK)
                .min(deadline - now);
            tokio::select! {
                () = &mut changed => {}
                () = tokio::time::sleep(wait) => {}
                _ = stopping.changed() => break,
            }
        }
    }
    Ok((
        StatusCode::NO_CONTENT,
        [(header::RETRY_AFTER, RETRY_AFTER_S)],
    )
        .into_response())
}

#[utoipa::path(
    post,
    path = "/api/fetcher/jobs/{id}/lease",
    tag = "fetcher",
    security(("downloader" = [])),
    params(("id" = Uuid, Path, description = "id задания")),
    request_body(content = LeaseRequest, description = "Можно не передавать"),
    responses(
        (status = 200, description = "Аренда продлена", body = Lease),
        (status = 401, description = "Токен загрузчика не принят", body = ErrorBody),
        (status = 409, description = "Задание уже не ваше (`job_not_yours`): аренда кончилась или его отменили — работу можно бросить", body = ErrorBody),
    ),
)]
pub async fn renew_lease(
    State(st): State<AppState>,
    downloader: DownloaderAuth,
    ClientIp(ip): ClientIp,
    Path(id): Path<Uuid>,
    JsonOrDefault(_): JsonOrDefault<LeaseRequest>,
) -> AppResult<Json<Lease>> {
    downloaders::seen(&st.db, downloader.id, ip, None).await?;
    let until = jobs::renew_lease(&st.db, id, &downloader.executor(), st.config.fetcher.lease)
        .await?
        .ok_or_else(not_yours)?;
    Ok(Json(Lease {
        lease_expires_at: until,
    }))
}

#[utoipa::path(
    post,
    path = "/api/fetcher/jobs/{id}/result",
    tag = "fetcher",
    security(("downloader" = [])),
    params(("id" = Uuid, Path, description = "id задания")),
    request_body = ResultReport,
    responses(
        (status = 204, description = "Отчёт принят: файл записан, отдан в «Спорные» или задание вернулось в очередь"),
        (status = 401, description = "Токен загрузчика не принят", body = ErrorBody),
        (status = 404, description = "Нет такого задания", body = ErrorBody),
        (status = 409, description = "Задание уже не ваше (`job_not_yours`): аренда кончилась или его отменили", body = ErrorBody),
    ),
)]
pub async fn report(
    State(st): State<AppState>,
    downloader: DownloaderAuth,
    ClientIp(ip): ClientIp,
    Path(id): Path<Uuid>,
    Json(body): Json<ResultReport>,
) -> AppResult<StatusCode> {
    downloaders::seen(&st.db, downloader.id, ip, None).await?;
    let executor = downloader.executor();
    let job = jobs::get(&st.db, id).await?.ok_or(AppError::NotFound)?;
    if job.kind != kind::ACQUIRE
        || job.status() != JobStatus::Running
        || job.locked_by.as_deref() != Some(executor.as_str())
    {
        return Err(not_yours());
    }
    let yt_dlp = body
        .result
        .pointer("/versions/ytDlp")
        .and_then(Value::as_str);
    downloaders::report_versions(&st.db, downloader.id, yt_dlp).await?;
    let settled = result::settle(
        &st.db,
        &st.storage,
        &job,
        &executor,
        &executor,
        &body.result,
        REMOTE_POSTPONE,
    )
    .await?;
    match settled {
        Some(_) => Ok(StatusCode::NO_CONTENT),
        None => Err(not_yours()),
    }
}

#[utoipa::path(
    post,
    path = "/api/fetcher/jobs/{id}/release",
    tag = "fetcher",
    security(("downloader" = [])),
    params(("id" = Uuid, Path, description = "id задания")),
    request_body(content = ReleaseRequest, description = "Можно не передавать"),
    responses(
        (status = 204, description = "Задание снова в очереди и его можно брать сразу; попытка не потрачена"),
        (status = 401, description = "Токен загрузчика не принят", body = ErrorBody),
        (status = 404, description = "Нет такого задания", body = ErrorBody),
        (status = 409, description = "Задание уже не ваше (`job_not_yours`): аренда кончилась, его отменили или оно закончено", body = ErrorBody),
    ),
)]
pub async fn release(
    State(st): State<AppState>,
    downloader: DownloaderAuth,
    ClientIp(ip): ClientIp,
    Path(id): Path<Uuid>,
    JsonOrDefault(body): JsonOrDefault<ReleaseRequest>,
) -> AppResult<StatusCode> {
    downloaders::seen(&st.db, downloader.id, ip, None).await?;
    let executor = downloader.executor();
    let job = jobs::get(&st.db, id).await?.ok_or(AppError::NotFound)?;
    if job.kind != kind::ACQUIRE
        || job.status() != JobStatus::Running
        || job.locked_by.as_deref() != Some(executor.as_str())
    {
        return Err(not_yours());
    }
    let reason: String = body
        .reason
        .as_deref()
        .map(str::trim)
        .filter(|r| !r.is_empty())
        .unwrap_or("без причины")
        .chars()
        .take(200)
        .collect();
    let message = format!("загрузчик вернул задание: {reason}");
    // Бросил исполнитель, а не запись: попытка не тратится, взять задание можно сразу
    let released = match job.payload::<AcquirePayload>() {
        Ok(payload) => {
            result::give_back(&st.db, &job, &payload, &executor, &message, Duration::ZERO).await?
        }
        Err(_) => jobs::postpone(&st.db, id, &executor, &message, Duration::ZERO)
            .await?
            .is_some(),
    };
    if !released {
        return Err(not_yours());
    }
    tracing::info!(job = %id, downloader = %downloader.name, %reason, "job released by a downloader");
    Ok(StatusCode::NO_CONTENT)
}
