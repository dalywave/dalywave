//! Управление между устройствами: список устройств и последнее состояние. Живое — по WebSocket
//! (`connect.*`, см. [`crate::connect`]); `GET /api/devices` — для первого показа и проверок.
//!
//! Пульт по HTTP (`/api/devices/remote*`) — для Apple Watch: watchOS даёт приложению WebSocket только
//! при своём звуке. Часы раз в пару секунд отмечаются и забирают состояние, команды шлют запросом
//! и получают подтверждение устройства в ответе.

use axum::{
    Router,
    extract::{Path, State},
    routing::{get, patch, post},
};
use dalywave_protocol::{
    ErrorBody,
    connect::{
        ConnectAck, ConnectDevicePatch, ConnectDevices, ConnectHello, ConnectOverview, ConnectSend,
        ConnectTransfer,
    },
};

use crate::{auth::Principal, error::AppResult, extract::Json, state::AppState};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/devices", get(devices))
        .route("/devices/remote", post(remote_presence))
        .route("/devices/remote/command", post(remote_command))
        .route("/devices/remote/transfer", post(remote_transfer))
        .route("/devices/{id}", patch(update_device))
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
        crate::connect::overview(&st.db, principal.scope().id).await?,
    ))
}

#[utoipa::path(
    post,
    path = "/api/devices/remote",
    tag = "playback",
    security(("device" = [])),
    request_body = ConnectHello,
    responses(
        (status = 200, description = "Пульт отметился (в сети ещё 15 с). Устройства и снимок активного — \
                                      без очереди, истории и формы волны", body = ConnectOverview),
        (status = 401, description = "Нет сессии", body = ErrorBody),
    ),
)]
pub async fn remote_presence(
    State(st): State<AppState>,
    principal: Principal,
    Json(hello): Json<ConnectHello>,
) -> AppResult<Json<ConnectOverview>> {
    Ok(Json(
        crate::connect::remote_presence(&st, &principal, hello).await?,
    ))
}

#[utoipa::path(
    post,
    path = "/api/devices/remote/command",
    tag = "playback",
    security(("device" = [])),
    request_body = ConnectSend,
    responses(
        (status = 200, description = "Подтверждение устройства: `ok`, или `error` — `offline` (не в сети), \
                                      `timeout` (не ответило за 4 с), `failed`, `unsupported`", body = ConnectAck),
        (status = 401, description = "Нет сессии", body = ErrorBody),
    ),
)]
pub async fn remote_command(
    State(st): State<AppState>,
    principal: Principal,
    Json(send): Json<ConnectSend>,
) -> AppResult<Json<ConnectAck>> {
    Ok(Json(
        crate::connect::remote_command(&st, &principal, send).await?,
    ))
}

#[utoipa::path(
    post,
    path = "/api/devices/remote/transfer",
    tag = "playback",
    security(("device" = [])),
    request_body = ConnectTransfer,
    responses(
        (status = 200, description = "Перенос: `ok`, или `error` — `offline`, `unsupported` (на часы звук \
                                      не переносится)", body = ConnectAck),
        (status = 401, description = "Нет сессии", body = ErrorBody),
    ),
)]
pub async fn remote_transfer(
    State(st): State<AppState>,
    principal: Principal,
    Json(t): Json<ConnectTransfer>,
) -> AppResult<Json<ConnectAck>> {
    Ok(Json(
        crate::connect::remote_transfer(&st, &principal, t).await?,
    ))
}

#[utoipa::path(
    patch,
    path = "/api/devices/{id}",
    tag = "playback",
    security(("session" = []), ("device" = [])),
    params(("id" = String, Path, description = "Id устройства из списка «Где играть»")),
    request_body = ConnectDevicePatch,
    responses(
        (status = 200, description = "Настройки сохранены; свежий список — и всем устройствам по WebSocket", body = ConnectDevices),
        (status = 401, description = "Нет сессии", body = ErrorBody),
        (status = 404, description = "Нет такого устройства", body = ErrorBody),
    ),
)]
pub async fn update_device(
    State(st): State<AppState>,
    principal: Principal,
    Path(id): Path<String>,
    Json(patch): Json<ConnectDevicePatch>,
) -> AppResult<Json<ConnectDevices>> {
    Ok(Json(
        crate::choice::update_device(&st, principal.scope(), &id, patch).await?,
    ))
}
