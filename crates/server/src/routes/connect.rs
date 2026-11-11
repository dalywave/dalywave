//! Управление между устройствами: список устройств и последнее состояние. Живое — по WebSocket
//! (`connect.*`, см. [`crate::connect`]); этот запрос — для первого показа и проверок.

use axum::{Router, extract::State, routing::get};
use dalywave_protocol::{ErrorBody, connect::ConnectOverview};

use crate::{auth::Principal, error::AppResult, extract::Json, state::AppState};

pub fn routes() -> Router<AppState> {
    Router::new().route("/devices", get(devices))
}

#[utoipa::path(
    get,
    path = "/api/devices",
    tag = "playback",
    security(("session" = []), ("device" = [])),
    responses(
        (status = 200, description = "Устройства (активное первым) и последний снимок с позицией на сейчас", body = ConnectOverview),
        (status = 401, description = "Нет сессии", body = ErrorBody),
    ),
)]
pub async fn devices(
    State(st): State<AppState>,
    principal: Principal,
) -> AppResult<Json<ConnectOverview>> {
    Ok(Json(
        crate::connect::overview(&st.db, principal.user.id).await?,
    ))
}
