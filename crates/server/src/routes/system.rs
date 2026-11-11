//! Служебное: проверка здоровья и описание API.

use std::{sync::LazyLock, time::Duration};

use axum::{Router, extract::State, http::StatusCode, routing::get};
use dalywave_protocol::system::{Health, HealthStatus};
use utoipa::OpenApi;

use crate::{extract::Json, openapi::ApiDoc, state::AppState};

static OPENAPI: LazyLock<utoipa::openapi::OpenApi> = LazyLock::new(ApiDoc::openapi);

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/healthz", get(healthz))
        .route("/openapi.json", get(openapi))
}

#[utoipa::path(
    get,
    path = "/api/healthz",
    tag = "system",
    responses(
        (status = 200, description = "API и база работают", body = Health),
        (status = 503, description = "База недоступна", body = Health),
    ),
)]
pub async fn healthz(State(st): State<AppState>) -> (StatusCode, Json<Health>) {
    let ping = sqlx::query_scalar!(r#"SELECT 1 AS "one!""#).fetch_one(&st.db);
    let db = match tokio::time::timeout(Duration::from_secs(2), ping).await {
        Ok(Ok(_)) => HealthStatus::Ok,
        _ => HealthStatus::Error,
    };
    let code = if db == HealthStatus::Ok {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };
    let health = Health {
        status: db,
        version: env!("CARGO_PKG_VERSION").to_string(),
        db,
        uptime_s: st.started.elapsed().as_secs(),
    };
    (code, Json(health))
}

#[utoipa::path(
    get,
    path = "/api/openapi.json",
    tag = "system",
    responses((status = 200, description = "Это описание API, OpenAPI 3.1", content_type = "application/json")),
)]
pub async fn openapi() -> Json<&'static utoipa::openapi::OpenApi> {
    Json(&*OPENAPI)
}
