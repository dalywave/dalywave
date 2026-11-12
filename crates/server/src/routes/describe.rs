//! «Плейлист по описанию»: предпросмотр по словам, «Ещё в том же духе», сохранение обычным плейлистом,
//! обновление раз в неделю. Логика — [`crate::reco::describe`]. Под правом play: подбор ставит треки на
//! скачивание, как волна.

use std::time::Duration;

use axum::{
    Router,
    extract::State,
    http::StatusCode,
    routing::{get, post},
};
use dalywave_protocol::{
    ErrorBody,
    describe::{
        DescribeMore, DescribePreview, DescribeRequest, DescribedAdded, DescribedMore,
        DescribedPatch, DescribedPlaylist, SaveDescribed,
    },
    library::PlaylistSummary,
    wave::WaveItems,
};
use uuid::Uuid;

use crate::{
    auth::Principal,
    error::{AppError, AppResult, bad},
    extract::{Json, JsonOrDefault, Path},
    reco::{describe, wave},
    state::AppState,
};

/// Сборок на человека за окно: подбор — сотни запросов к индексу.
const LIMIT: u32 = 30;
const WINDOW: Duration = Duration::from_secs(10 * 60);

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/describe", post(describe_preview))
        .route("/describe/more", post(describe_more))
        .route("/describe/save", post(save_described))
        .route(
            "/playlists/{id}/describe",
            get(described).patch(update_described),
        )
        .route("/playlists/{id}/describe/more", post(extend_described))
}

fn limited(st: &AppState, principal: &Principal) -> AppResult<()> {
    if st
        .limiter
        .check_id("describe", principal.user.id, LIMIT, WINDOW)
    {
        Ok(())
    } else {
        Err(AppError::TooManyRequests)
    }
}

fn count_of(count: Option<u32>) -> AppResult<usize> {
    match count {
        None => Ok(describe::MORE_COUNT),
        Some(n) if (1..=describe::MORE_MAX as u32).contains(&n) => Ok(n as usize),
        Some(_) => Err(bad(format!("count — от 1 до {}", describe::MORE_MAX))),
    }
}

#[utoipa::path(
    post,
    path = "/api/describe",
    tag = "wave",
    security(("session" = []), ("device" = [])),
    request_body = DescribeRequest,
    responses(
        (status = 200, description = "Предпросмотр: что понято, название, описание и треки в плавном порядке. Первые \
                                      треки, которых нет на сервере, уже качаются. Сервис анализа не ответил — подбор \
                                      без поиска по звучанию (по жанрам, ориентирам и вкусу)", body = DescribePreview),
        (status = 400, description = "Пустое или длиннее 300 символов описание", body = ErrorBody),
        (status = 429, description = "Слишком часто: не больше 30 сборок за 10 минут", body = ErrorBody),
    ),
)]
pub async fn describe_preview(
    State(st): State<AppState>,
    principal: Principal,
    Json(body): Json<DescribeRequest>,
) -> AppResult<Json<DescribePreview>> {
    limited(&st, &principal)?;
    if body.exclude.len() > 500 {
        return Err(bad("exclude — не больше 500 треков"));
    }
    let env = wave::Env::of(&st).await?;
    Ok(Json(
        describe::preview(&env, &st.ml, principal.user.id, &body.text, &body.exclude).await?,
    ))
}

#[utoipa::path(
    post,
    path = "/api/describe/more",
    tag = "wave",
    security(("session" = []), ("device" = [])),
    request_body = DescribeMore,
    responses(
        (status = 200, description = "Ещё треки в том же духе: без показанных, начало подстроено к `afterTrackId`", body = WaveItems),
        (status = 400, description = "count вне 1–30 или понятое не разобрать", body = ErrorBody),
        (status = 429, description = "Слишком часто", body = ErrorBody),
    ),
)]
pub async fn describe_more(
    State(st): State<AppState>,
    principal: Principal,
    Json(body): Json<DescribeMore>,
) -> AppResult<Json<WaveItems>> {
    limited(&st, &principal)?;
    describe::check(&body.brief)?;
    if body.exclude.len() > 500 {
        return Err(bad("exclude — не больше 500 треков"));
    }
    let count = count_of(body.count)?;
    let env = wave::Env::of(&st).await?;
    let user = principal.user.id;
    let picks = describe::more(
        &env,
        &st.ml,
        user,
        &body.brief,
        &body.exclude,
        body.after_track_id,
        count,
    )
    .await?;
    Ok(Json(WaveItems {
        items: describe::items(&st.db, user, picks).await?,
    }))
}

#[utoipa::path(
    post,
    path = "/api/describe/save",
    tag = "wave",
    security(("session" = []), ("device" = [])),
    request_body = SaveDescribed,
    responses(
        (status = 201, description = "Обычный плейлист человека; помнит понятое — для «Ещё в том же духе» и обновления \
                                      раз в неделю. Чего нет на сервере, ставится на скачивание", body = PlaylistSummary),
        (status = 400, description = "Пустое название, нет известных треков, понятое не разобрать", body = ErrorBody),
    ),
)]
pub async fn save_described(
    State(st): State<AppState>,
    principal: Principal,
    Json(body): Json<SaveDescribed>,
) -> AppResult<(StatusCode, Json<PlaylistSummary>)> {
    let summary = describe::save(
        &st.db,
        st.deezer.clone(),
        principal.user.id,
        body.brief,
        &body.title,
        body.description.as_deref().unwrap_or(""),
        &body.track_ids,
        body.weekly,
    )
    .await?;
    Ok((StatusCode::CREATED, Json(summary)))
}

#[utoipa::path(
    get,
    path = "/api/playlists/{id}/describe",
    tag = "library",
    security(("session" = []), ("device" = [])),
    params(("id" = Uuid, Path, description = "id плейлиста")),
    responses(
        (status = 200, description = "Плейлист собран по описанию: понятое и обновление раз в неделю", body = DescribedPlaylist),
        (status = 404, description = "Нет такого плейлиста или он собран не по описанию (`not_described`)", body = ErrorBody),
    ),
)]
pub async fn described(
    State(st): State<AppState>,
    principal: Principal,
    Path(id): Path<Uuid>,
) -> AppResult<Json<DescribedPlaylist>> {
    let r = describe::rules(&st.db, principal.user.id, id).await?;
    Ok(Json(describe::view(id, r)))
}

#[utoipa::path(
    patch,
    path = "/api/playlists/{id}/describe",
    tag = "library",
    security(("session" = []), ("device" = [])),
    params(("id" = Uuid, Path, description = "id плейлиста")),
    request_body = DescribedPatch,
    responses(
        (status = 200, description = "Обновление раз в неделю включено или выключено", body = DescribedPlaylist),
        (status = 404, description = "Нет такого плейлиста или он собран не по описанию", body = ErrorBody),
    ),
)]
pub async fn update_described(
    State(st): State<AppState>,
    principal: Principal,
    Path(id): Path<Uuid>,
    Json(body): Json<DescribedPatch>,
) -> AppResult<Json<DescribedPlaylist>> {
    let r = describe::set_weekly(&st.db, principal.user.id, id, body.weekly).await?;
    Ok(Json(describe::view(id, r)))
}

#[utoipa::path(
    post,
    path = "/api/playlists/{id}/describe/more",
    tag = "library",
    security(("session" = []), ("device" = [])),
    params(("id" = Uuid, Path, description = "id плейлиста")),
    request_body = DescribedMore,
    responses(
        (status = 200, description = "В конец плейлиста добавлены треки в том же духе (без тех, что уже есть)", body = DescribedAdded),
        (status = 400, description = "count вне 1–30", body = ErrorBody),
        (status = 404, description = "Нет такого плейлиста или он собран не по описанию", body = ErrorBody),
        (status = 429, description = "Слишком часто", body = ErrorBody),
    ),
)]
pub async fn extend_described(
    State(st): State<AppState>,
    principal: Principal,
    Path(id): Path<Uuid>,
    JsonOrDefault(body): JsonOrDefault<DescribedMore>,
) -> AppResult<Json<DescribedAdded>> {
    limited(&st, &principal)?;
    let count = count_of(body.count)?;
    let env = wave::Env::of(&st).await?;
    let track_ids = describe::extend(&env, &st.ml, principal.user.id, id, count).await?;
    Ok(Json(DescribedAdded { track_ids }))
}
