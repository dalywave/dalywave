//! Видео без звука: клипы ленты открытий и живые обложки альбомов ([`crate::video`]); задания клипов
//! домашнему загрузчику.

use axum::{
    Router,
    extract::State,
    http::{StatusCode, header},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use dalywave_protocol::{
    ErrorBody,
    jobs::{JobStatus, kind},
    video::{AlbumMotion, ClipClaimRequest, ClipClaimResponse, ClipList, ClipReport, ClipsRequest},
};
use uuid::Uuid;

use crate::{
    downloaders::{self, DownloaderAuth},
    error::{AppError, AppResult, bad},
    extract::{Json, Path},
    jobs,
    state::{AppState, ClientIp},
    video::{self, clips},
};

/// Воспроизведение: видео ленты и живые обложки.
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/clips", post(prepare_clips))
        .route("/albums/{id}/motion", get(album_motion))
}

/// Домашний загрузчик: задания клипов.
pub fn fetcher_routes() -> Router<AppState> {
    Router::new()
        .route("/fetcher/clips/claim", post(claim_clip))
        .route("/fetcher/clips/{id}/result", post(report_clip))
}

fn private<T: serde::Serialize>(body: T) -> Response {
    // Ссылки подписаны и живут несколько часов
    ([(header::CACHE_CONTROL, "no-store")], Json(body)).into_response()
}

#[utoipa::path(
    post,
    path = "/api/clips",
    tag = "playback",
    security(("session" = []), ("device" = [])),
    request_body = ClipsRequest,
    responses(
        (status = 200, description = "Видео карточек ленты: клип (отрывок официального видео вокруг припева) \
                                      и живая обложка альбома. `pending` — ищется, спросить снова через \
                                      несколько секунд", body = ClipList),
        (status = 400, description = "Больше 20 треков", body = ErrorBody),
    ),
)]
pub async fn prepare_clips(
    State(st): State<AppState>,
    Json(req): Json<ClipsRequest>,
) -> AppResult<Response> {
    if req.track_ids.len() > clips::MAX_REQUEST {
        return Err(bad(format!("не больше {} треков", clips::MAX_REQUEST)));
    }
    let items = clips::prepare(
        &st.db,
        &st.storage,
        st.config.info.musickit.is_some(),
        &req.track_ids,
    )
    .await?;
    Ok(private(ClipList { items }))
}

#[utoipa::path(
    get,
    path = "/api/albums/{id}/motion",
    tag = "playback",
    security(("session" = []), ("device" = [])),
    params(("id" = Uuid, Path, description = "id альбома")),
    responses(
        (status = 200, description = "Живая обложка альбома (Apple Music): квадратная и вертикальная. \
                                      `pending` — ищется; `none` — нет", body = AlbumMotion),
        (status = 404, description = "Нет такого альбома", body = ErrorBody),
    ),
)]
pub async fn album_motion(
    State(st): State<AppState>,
    Path(id): Path<Uuid>,
) -> AppResult<Response> {
    let exists = sqlx::query_scalar!("SELECT EXISTS (SELECT 1 FROM albums WHERE id = $1) AS \"e!\"", id)
        .fetch_one(&st.db)
        .await?;
    if !exists {
        return Err(AppError::NotFound);
    }
    let mut found = video::motions(
        &st.db,
        &st.storage,
        st.config.info.musickit.is_some(),
        &[id],
    )
    .await?;
    let motion = found.remove(&id).ok_or(AppError::NotFound)?;
    Ok(private(motion))
}

fn not_yours() -> AppError {
    AppError::Conflict {
        code: "job_not_yours",
        message: "задание уже не ваше: аренда кончилась или его отменили".into(),
    }
}

#[utoipa::path(
    post,
    path = "/api/fetcher/clips/claim",
    tag = "fetcher",
    security(("downloader" = [])),
    request_body = ClipClaimRequest,
    responses(
        (status = 200, description = "Задание клипа: трек, место припева, подписанная ссылка для MP4", body = ClipClaimResponse),
        (status = 204, description = "Клипов нет (сразу, без ожидания)"),
        (status = 401, description = "Токен загрузчика не принят", body = ErrorBody),
    ),
)]
pub async fn claim_clip(
    State(st): State<AppState>,
    downloader: DownloaderAuth,
    ClientIp(ip): ClientIp,
    Json(req): Json<ClipClaimRequest>,
) -> AppResult<Response> {
    if req.worker_id.trim().is_empty() || req.worker_id.chars().count() > 100 {
        return Err(bad("workerId — от 1 до 100 символов"));
    }
    downloaders::seen(&st.db, downloader.id, ip, None).await?;
    match clips::claim(&st.db, &st.storage, &downloader.executor()).await? {
        Some(job) => {
            tracing::info!(job = %job.id, track = %job.track_id, downloader = %downloader.name, "clip job handed to a downloader");
            Ok(Json(ClipClaimResponse { job }).into_response())
        }
        None => Ok(StatusCode::NO_CONTENT.into_response()),
    }
}

#[utoipa::path(
    post,
    path = "/api/fetcher/clips/{id}/result",
    tag = "fetcher",
    security(("downloader" = [])),
    params(("id" = Uuid, Path, description = "id задания")),
    request_body = ClipReport,
    responses(
        (status = 204, description = "Отчёт принят"),
        (status = 401, description = "Токен загрузчика не принят", body = ErrorBody),
        (status = 404, description = "Нет такого задания", body = ErrorBody),
        (status = 409, description = "Задание уже не ваше (`job_not_yours`)", body = ErrorBody),
    ),
)]
pub async fn report_clip(
    State(st): State<AppState>,
    downloader: DownloaderAuth,
    ClientIp(ip): ClientIp,
    Path(id): Path<Uuid>,
    Json(body): Json<ClipReport>,
) -> AppResult<StatusCode> {
    downloaders::seen(&st.db, downloader.id, ip, None).await?;
    let executor = downloader.executor();
    let job = jobs::get(&st.db, id).await?.ok_or(AppError::NotFound)?;
    if job.kind != kind::CLIP
        || job.status() != JobStatus::Running
        || job.locked_by.as_deref() != Some(executor.as_str())
    {
        return Err(not_yours());
    }
    if clips::settle(&st.db, &st.storage, &job, &executor, &body.result).await? {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(not_yours())
    }
}
