//! «Итоги года» для историй и плейлист «Твои лучшие треки {год}». Логика — [`crate::rewind`].

use axum::{
    Router,
    extract::State,
    http::StatusCode,
    routing::{get, post},
};
use chrono::Utc;
use dalywave_protocol::{
    ErrorBody,
    library::PlaylistSummary,
    rewind::{Rewind, RewindYears},
};

use crate::{
    auth::Principal,
    error::{AppError, AppResult, bad},
    extract::{Json, Path},
    rewind,
    state::AppState,
};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/rewind", get(years))
        .route("/rewind/{year}", get(year))
        .route("/rewind/{year}/playlist", post(playlist))
}

#[utoipa::path(
    get,
    path = "/api/rewind",
    tag = "library",
    security(("session" = []), ("device" = [])),
    responses((status = 200, description = "Годы с прослушиваниями (новые первыми); текущий — всегда", body = RewindYears)),
)]
pub async fn years(
    State(st): State<AppState>,
    principal: Principal,
) -> AppResult<Json<RewindYears>> {
    Ok(Json(
        rewind::years(&st.db, principal.user.id, Utc::now()).await?,
    ))
}

#[utoipa::path(
    get,
    path = "/api/rewind/{year}",
    tag = "library",
    security(("session" = []), ("device" = [])),
    params(("year" = i32, Path, description = "Год по времени слушателя")),
    responses(
        (status = 200, description = "Итоги года; текущий — «по сей день» (`complete: false`). Без прослушиваний — \
                                      нули и пустые списки", body = Rewind),
        (status = 404, description = "Год ещё не наступил", body = ErrorBody),
    ),
)]
pub async fn year(
    State(st): State<AppState>,
    principal: Principal,
    Path(year): Path<i32>,
) -> AppResult<Json<Rewind>> {
    rewind::get(&st.db, principal.user.id, year, Utc::now())
        .await?
        .map(Json)
        .ok_or(AppError::NotFound)
}

#[utoipa::path(
    post,
    path = "/api/rewind/{year}/playlist",
    tag = "library",
    security(("session" = []), ("device" = [])),
    params(("year" = i32, Path, description = "Год по времени слушателя")),
    responses(
        (status = 201, description = "Плейлист «Твои лучшие треки {год}» создан", body = PlaylistSummary),
        (status = 200, description = "Уже был — обновлён по свежим итогам", body = PlaylistSummary),
        (status = 400, description = "За этот год ещё нечего собрать", body = ErrorBody),
        (status = 404, description = "Год ещё не наступил", body = ErrorBody),
    ),
)]
pub async fn playlist(
    State(st): State<AppState>,
    principal: Principal,
    Path(year): Path<i32>,
) -> AppResult<(StatusCode, Json<PlaylistSummary>)> {
    let now = Utc::now();
    let current = rewind::years(&st.db, principal.user.id, now).await?.current;
    if !(rewind::FIRST_YEAR..=current).contains(&year) {
        return Err(AppError::NotFound);
    }
    let (summary, created) = rewind::save_playlist(&st.db, principal.user.id, year)
        .await?
        .ok_or_else(|| bad("за этот год ещё нечего собрать"))?;
    let status = if created {
        StatusCode::CREATED
    } else {
        StatusCode::OK
    };
    Ok((status, Json(summary)))
}
