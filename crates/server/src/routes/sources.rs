//! Раздел «Источники» админки: домашние загрузчики (токены, отзыв, работа за сутки) и серверный fetcher.
//! Весь роутер закрыт правом admin (см. `routes::router`).

use axum::{
    Router,
    extract::State,
    http::StatusCode,
    routing::{delete, get},
};
use dalywave_protocol::{
    ErrorBody,
    fetcher::{DownloaderList, DownloaderToken, NewDownloader, ServerFetcher, Sources},
};
use serde_json::json;
use uuid::Uuid;

use crate::{
    acquire::{self, server},
    audit,
    auth::Principal,
    downloaders,
    error::{AppError, AppResult, bad},
    extract::{Json, Path},
    jobs,
    state::AppState,
};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route(
            "/downloaders",
            get(list_downloaders).post(create_downloader),
        )
        .route("/downloaders/{id}", delete(revoke_downloader))
        .route("/sources", get(sources))
}

#[utoipa::path(
    get,
    path = "/api/admin/downloaders",
    tag = "admin",
    security(("session" = []), ("device" = [])),
    responses(
        (status = 200, description = "Загрузчики: на связи первыми, отозванные в конце", body = DownloaderList),
        (status = 403, description = "Нет права admin", body = ErrorBody),
    ),
)]
pub async fn list_downloaders(State(st): State<AppState>) -> AppResult<Json<DownloaderList>> {
    Ok(Json(DownloaderList {
        items: downloaders::list(&st.db, None).await?,
    }))
}

#[utoipa::path(
    post,
    path = "/api/admin/downloaders",
    tag = "admin",
    security(("session" = []), ("device" = [])),
    request_body = NewDownloader,
    responses(
        (status = 201, description = "Загрузчик заведён; токен `dwk_…` показывается один раз", body = DownloaderToken),
        (status = 400, description = "Имя — от 1 до 60 символов", body = ErrorBody),
    ),
)]
pub async fn create_downloader(
    State(st): State<AppState>,
    principal: Principal,
    Json(body): Json<NewDownloader>,
) -> AppResult<(StatusCode, Json<DownloaderToken>)> {
    let name = body.name.trim();
    if name.is_empty() || name.chars().count() > 60 {
        return Err(bad("имя загрузчика — от 1 до 60 символов"));
    }
    let mut tx = st.db.begin().await?;
    let (id, token) = downloaders::create(&mut tx, name, principal.user.id).await?;
    audit::record(
        &mut *tx,
        Some(principal.user.id),
        audit::DOWNLOADER_CREATED,
        Some(id.to_string()),
        json!({ "name": name }),
    )
    .await?;
    tx.commit().await?;
    let downloader = downloaders::get(&st.db, id)
        .await?
        .ok_or(AppError::NotFound)?;
    Ok((
        StatusCode::CREATED,
        Json(DownloaderToken { token, downloader }),
    ))
}

#[utoipa::path(
    delete,
    path = "/api/admin/downloaders/{id}",
    tag = "admin",
    security(("session" = []), ("device" = [])),
    params(("id" = Uuid, Path, description = "id загрузчика")),
    responses(
        (status = 204, description = "Токен больше не действует; задания, взятые загрузчиком, вернулись в очередь"),
        (status = 404, description = "Нет такого живого загрузчика", body = ErrorBody),
    ),
)]
pub async fn revoke_downloader(
    State(st): State<AppState>,
    principal: Principal,
    Path(id): Path<Uuid>,
) -> AppResult<StatusCode> {
    let mut tx = st.db.begin().await?;
    if !downloaders::revoke(&mut tx, id).await? {
        return Err(AppError::NotFound);
    }
    audit::record(
        &mut *tx,
        Some(principal.user.id),
        audit::DOWNLOADER_REVOKED,
        Some(id.to_string()),
        json!({}),
    )
    .await?;
    tx.commit().await?;
    // Отозванный не отчитается: его задания — сразу обратно в очередь, не дожидаясь конца аренды
    let released = jobs::release(&st.db, &downloaders::executor(id)).await?;
    acquire::after_reap(&st.db, &released).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    get,
    path = "/api/admin/sources",
    tag = "admin",
    security(("session" = []), ("device" = [])),
    responses(
        (status = 200, description = "Загрузчики с работой за сутки и состояние серверного fetcher", body = Sources),
        (status = 403, description = "Нет права admin", body = ErrorBody),
    ),
)]
pub async fn sources(State(st): State<AppState>) -> AppResult<Json<Sources>> {
    let stats = sqlx::query!(
        r#"SELECT
              (SELECT count(*) FROM acquisitions
               WHERE worker = $1 AND status IN ('matching', 'downloading', 'verifying', 'processing')) AS "running!",
              (SELECT count(*) FROM acquisition_attempts
               WHERE executor = $1 AND created_at > now() - interval '24 hours' AND outcome = 'ready') AS "ok!",
              (SELECT count(*) FROM acquisition_attempts
               WHERE executor = $1 AND created_at > now() - interval '24 hours'
                 AND outcome IN ('retry', 'failed', 'postponed')) AS "failed!""#,
        server::EXECUTOR,
    )
    .fetch_one(&st.db)
    .await?;
    let fetcher = &st.config.fetcher;
    let (reachable, health, error) = if fetcher.server {
        match server::health(&st.http, &fetcher.url).await {
            Ok(h) => (Some(true), Some(h), None),
            Err(e) => (Some(false), None, Some(e)),
        }
    } else {
        (None, None, None)
    };
    Ok(Json(Sources {
        downloaders: downloaders::list(&st.db, None).await?,
        server: ServerFetcher {
            enabled: fetcher.server,
            url: fetcher.server.then(|| fetcher.url.clone()),
            reachable,
            health,
            error,
            running: stats.running,
            ok_24h: stats.ok,
            failed_24h: stats.failed,
        },
    }))
}
