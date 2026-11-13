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
    auth::Permission,
    jobs::{JobStatus, kind},
    video::{
        AlbumMotion, ClipChoice, ClipClaimRequest, ClipClaimResponse, ClipList, ClipReport,
        ClipReportItem, ClipReportList, ClipView, ClipsRequest,
    },
};
use serde_json::json;
use uuid::Uuid;

use crate::{
    audit,
    auth::Principal,
    catalog::views,
    downloaders::{self, DownloaderAuth},
    error::{AppError, AppResult, bad},
    extract::{Json, Path},
    jobs,
    routes::acquisitions::{Pasted, parse_pasted, valid_video_id},
    state::{AppState, ClientIp},
    video::{self, clips},
};

/// Воспроизведение: видео ленты, живые обложки, «Клип» у трека.
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/clips", post(prepare_clips))
        .route("/albums/{id}/motion", get(album_motion))
        .route("/tracks/{id}/clip", get(track_clip).put(choose_clip))
        .route(
            "/tracks/{id}/clip/hide",
            post(hide_clip).delete(unhide_clip),
        )
}

/// Админка: жалобы на клипы.
pub fn admin_routes() -> Router<AppState> {
    Router::new().route("/clips/reports", get(clip_reports))
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
    principal: Principal,
    Json(req): Json<ClipsRequest>,
) -> AppResult<Response> {
    if req.track_ids.len() > clips::MAX_REQUEST {
        return Err(bad(format!("не больше {} треков", clips::MAX_REQUEST)));
    }
    let items = clips::prepare(
        &st.db,
        &st.storage,
        st.config.info.musickit.is_some(),
        principal.user.id,
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
pub async fn album_motion(State(st): State<AppState>, Path(id): Path<Uuid>) -> AppResult<Response> {
    let exists = sqlx::query_scalar!(
        "SELECT EXISTS (SELECT 1 FROM albums WHERE id = $1) AS \"e!\"",
        id
    )
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
    match clips::claim(&st.db, &st.storage, &downloader.executor(), &req.features).await? {
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

async fn clip_view(st: &AppState, id: Uuid, principal: &Principal) -> AppResult<ClipView> {
    clips::view(
        &st.db,
        &st.storage,
        id,
        principal.user.id,
        principal.can(Permission::Admin),
    )
    .await?
    .ok_or(AppError::NotFound)
}

#[utoipa::path(
    get,
    path = "/api/tracks/{id}/clip",
    tag = "playback",
    security(("session" = []), ("device" = [])),
    params(("id" = Uuid, Path, description = "id трека")),
    responses(
        (status = 200, description = "Клип вместо обложки: что нарезано и откуда, скрыт ли у меня. \
                                      Администратору — кандидаты и число жалоб", body = ClipView),
        (status = 404, description = "Нет такого трека", body = ErrorBody),
    ),
)]
pub async fn track_clip(
    State(st): State<AppState>,
    principal: Principal,
    Path(id): Path<Uuid>,
) -> AppResult<Response> {
    Ok(private(clip_view(&st, id, &principal).await?))
}

#[utoipa::path(
    put,
    path = "/api/tracks/{id}/clip",
    tag = "playback",
    security(("session" = []), ("device" = [])),
    params(("id" = Uuid, Path, description = "id трека")),
    request_body = ClipChoice,
    responses(
        (status = 200, description = "Выбор принят: ролик закреплён и режется впереди очереди (до тех пор \
                                      `pending`), «без клипа», «заново» или «оставить»", body = ClipView),
        (status = 400, description = "Ссылка не на видео YouTube", body = ErrorBody),
        (status = 403, description = "Выбирать клип может только администратор", body = ErrorBody),
        (status = 404, description = "Нет такого трека", body = ErrorBody),
    ),
)]
pub async fn choose_clip(
    State(st): State<AppState>,
    principal: Principal,
    Path(id): Path<Uuid>,
    Json(req): Json<ClipChoice>,
) -> AppResult<Response> {
    principal.need(Permission::Admin)?;
    let choice = match &req {
        ClipChoice::Video { video_id } if valid_video_id(video_id) => {
            clips::Choice::Video(video_id.clone())
        }
        ClipChoice::Video { .. } => return Err(bad("неверный id видео YouTube")),
        ClipChoice::Url { url } => match parse_pasted(url)? {
            Pasted::Video(video) => clips::Choice::Video(video),
            Pasted::Direct(_) => return Err(bad("нужна ссылка на видео YouTube")),
        },
        ClipChoice::None => clips::Choice::Off,
        ClipChoice::Auto => clips::Choice::Auto,
        ClipChoice::Keep => clips::Choice::Keep,
        ClipChoice::Search => clips::Choice::Search,
    };
    let exists = sqlx::query_scalar!(
        "SELECT EXISTS (SELECT 1 FROM tracks WHERE id = $1) AS \"e!\"",
        id
    )
    .fetch_one(&st.db)
    .await?;
    if !exists {
        return Err(AppError::NotFound);
    }
    clips::choose(&st.db, id, principal.user.id, &choice).await?;
    if choice != clips::Choice::Search {
        audit::record(
            &st.db,
            Some(principal.user.id),
            audit::CLIP_CHOSEN,
            Some(id.to_string()),
            json!(req),
        )
        .await?;
    }
    Ok(private(clip_view(&st, id, &principal).await?))
}

#[utoipa::path(
    post,
    path = "/api/tracks/{id}/clip/hide",
    tag = "playback",
    security(("session" = []), ("device" = [])),
    params(("id" = Uuid, Path, description = "id трека")),
    responses(
        (status = 204, description = "Клип скрыт у меня; администратор видит жалобу в «Спорных»"),
        (status = 404, description = "У трека нет клипа", body = ErrorBody),
    ),
)]
pub async fn hide_clip(
    State(st): State<AppState>,
    principal: Principal,
    Path(id): Path<Uuid>,
) -> AppResult<StatusCode> {
    if !clips::hide(&st.db, id, principal.user.id).await? {
        return Err(AppError::NotFound);
    }
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    delete,
    path = "/api/tracks/{id}/clip/hide",
    tag = "playback",
    security(("session" = []), ("device" = [])),
    params(("id" = Uuid, Path, description = "id трека")),
    responses((status = 204, description = "Клип снова показывается")),
)]
pub async fn unhide_clip(
    State(st): State<AppState>,
    principal: Principal,
    Path(id): Path<Uuid>,
) -> AppResult<StatusCode> {
    clips::unhide(&st.db, id, principal.user.id).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    get,
    path = "/api/admin/clips/reports",
    tag = "admin",
    security(("session" = []), ("device" = [])),
    responses(
        (status = 200, description = "Треки, где слушатели скрыли клип, а администратор его ещё не \
                                      подтвердил и не заменил: больше жалоб — выше", body = ClipReportList),
    ),
)]
pub async fn clip_reports(
    State(st): State<AppState>,
    principal: Principal,
) -> AppResult<Json<ClipReportList>> {
    let rows = clips::reported(&st.db, 100).await?;
    let ids: Vec<Uuid> = rows.iter().map(|r| r.track_id).collect();
    let tracks = views::tracks(&st.db, &ids, principal.user.id).await?;
    let mut by_id: std::collections::HashMap<Uuid, _> =
        tracks.into_iter().map(|t| (t.id, t)).collect();
    let items = rows
        .into_iter()
        .filter_map(|r| {
            Some(ClipReportItem {
                track: by_id.remove(&r.track_id)?,
                video_id: r.video_id,
                video_title: r.video_title,
                channel: r.channel,
                reports: r.reports,
                last_at: r.last_at,
            })
        })
        .collect();
    Ok(Json(ClipReportList { items }))
}
