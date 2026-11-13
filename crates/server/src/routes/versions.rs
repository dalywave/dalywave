//! Версии песни в один клик: оригинал, slowed, super slowed, sped up — только выпущенные самими
//! исполнителями (см. [`crate::versions`]). Выбор версии запоминается по исполнителю.

use axum::{
    Router,
    extract::State,
    http::StatusCode,
    routing::{get, put},
};
use dalywave_protocol::{
    ErrorBody,
    versions::{TrackVersions, VersionChoice, VersionChoiceBody, VersionChoices},
};
use uuid::Uuid;

use crate::{
    auth::Principal,
    error::AppResult,
    extract::{Json, Path},
    state::AppState,
    versions,
};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/tracks/{id}/versions", get(track_versions))
        .route("/tracks/{id}/versions/choice", put(choose))
        .route("/versions/choices", get(choices))
}

#[utoipa::path(
    get,
    path = "/api/tracks/{id}/versions",
    tag = "catalog",
    security(("session" = []), ("device" = [])),
    params(("id" = Uuid, Path, description = "id трека")),
    responses(
        (status = 200, description = "Версии той же песни: оригинал, slowed, super slowed, sped up; раз в две недели ищутся у Deezer заново", body = TrackVersions),
        (status = 404, description = "Нет такого трека", body = ErrorBody),
    ),
)]
pub async fn track_versions(
    State(st): State<AppState>,
    principal: Principal,
    Path(id): Path<Uuid>,
) -> AppResult<Json<TrackVersions>> {
    Ok(Json(
        versions::versions(&st.db, &st.deezer, id, principal.user.id).await?,
    ))
}

#[utoipa::path(
    put,
    path = "/api/tracks/{id}/versions/choice",
    tag = "catalog",
    security(("session" = []), ("device" = [])),
    params(("id" = Uuid, Path, description = "id трека: выбор запоминается для его основного исполнителя")),
    request_body = VersionChoiceBody,
    responses(
        (status = 204, description = "Запомнено; `original` — выбор забыт"),
        (status = 404, description = "Нет такого трека", body = ErrorBody),
    ),
)]
pub async fn choose(
    State(st): State<AppState>,
    principal: Principal,
    Path(id): Path<Uuid>,
    Json(body): Json<VersionChoiceBody>,
) -> AppResult<StatusCode> {
    versions::choose(&st.db, principal.user.id, id, body.kind).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    get,
    path = "/api/versions/choices",
    tag = "catalog",
    security(("session" = []), ("device" = [])),
    responses((status = 200, description = "Исполнители, у которых слушатель выбрал не оригинал", body = VersionChoices)),
)]
pub async fn choices(
    State(st): State<AppState>,
    principal: Principal,
) -> AppResult<Json<VersionChoices>> {
    let items = sqlx::query!(
        "SELECT artist_id, kind FROM version_choices WHERE user_id = $1 ORDER BY chosen_at DESC",
        principal.user.id
    )
    .fetch_all(&st.db)
    .await?
    .into_iter()
    .filter_map(|r| {
        Some(VersionChoice {
            artist_id: r.artist_id,
            kind: r.kind.parse().ok()?,
        })
    })
    .collect();
    Ok(Json(VersionChoices { items }))
}
