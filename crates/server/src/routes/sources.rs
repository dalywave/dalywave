//! Раздел «Источники» админки: домашние загрузчики (токены, отзыв, работа за сутки) и серверный fetcher.
//! Весь роутер закрыт правом admin (см. `routes::router`).

use axum::{
    Router,
    extract::State,
    http::StatusCode,
    routing::{delete, get, post},
};
use dalywave_protocol::{
    ErrorBody,
    fetcher::{
        DownloaderList, DownloaderToken, NewDownloader, ServerFetcher, SourceCheck, SourceDay,
        Sources,
    },
    jobs::{AcquireReason, AdminAcquisition},
};
use serde_json::json;
use uuid::Uuid;

use super::acquisitions::admin_view;
use crate::{
    acquire::{self, Enqueue, MANUAL_PRIORITY, Request, server},
    audit,
    auth::Principal,
    downloaders,
    error::{AppError, AppResult, bad},
    extract::{Json, Path},
    jobs::{self, lanes},
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
        .route("/sources/check", post(check_download))
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
    let daily = sqlx::query!(
        r#"SELECT to_char(d.day, 'YYYY-MM-DD') AS "day!",
                  count(a.id) FILTER (WHERE a.outcome = 'ready') AS "ok!",
                  count(a.id) FILTER (WHERE a.outcome = 'disputed') AS "disputed!",
                  count(a.id) FILTER (WHERE a.outcome = 'unavailable') AS "unavailable!",
                  count(a.id) FILTER (WHERE a.outcome IN ('retry', 'failed', 'postponed')) AS "failed!"
           FROM generate_series((now() AT TIME ZONE 'UTC')::date - 13, (now() AT TIME ZONE 'UTC')::date,
                                interval '1 day') AS d(day)
           LEFT JOIN acquisition_attempts a ON (a.created_at AT TIME ZONE 'UTC')::date = d.day::date
           GROUP BY d.day ORDER BY d.day"#
    )
    .fetch_all(&st.db)
    .await?
    .into_iter()
    .map(|r| SourceDay {
        day: r.day,
        ok: r.ok,
        disputed: r.disputed,
        unavailable: r.unavailable,
        failed: r.failed,
    })
    .collect();
    Ok(Json(Sources {
        daily,
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

#[utoipa::path(
    post,
    path = "/api/admin/sources/check",
    tag = "admin",
    security(("session" = []), ("device" = [])),
    request_body = SourceCheck,
    responses(
        (status = 200, description = "Трек поставлен на скачивание вне очереди (или уже качается) — \
                                      за ним видно, как работают источники", body = AdminAcquisition),
        (status = 404, description = "Нет такого трека", body = ErrorBody),
        (status = 409, description = "Скачивать нечего: у всех треков уже есть файл (`nothing_to_check`)", body = ErrorBody),
    ),
)]
pub async fn check_download(
    State(st): State<AppState>,
    principal: Principal,
    Json(body): Json<SourceCheck>,
) -> AppResult<Json<AdminAcquisition>> {
    let admin = principal.user.id;
    // Свой трек — он; иначе свежие лайки без файла, затем популярное каталога без файла
    let candidates: Vec<uuid::Uuid> = match body.track_id {
        Some(id) => {
            let found = sqlx::query_scalar!("SELECT id FROM tracks WHERE id = $1", id)
                .fetch_optional(&st.db)
                .await?;
            vec![found.ok_or(AppError::NotFound)?]
        }
        None => {
            sqlx::query_scalar!(
                r#"SELECT t.id AS "id!" FROM (
                  (SELECT t.id, 0 AS tier, l.created_at AS at, 0 AS rank
                   FROM liked_tracks l JOIN tracks t ON t.id = l.track_id
                   JOIN recordings r ON r.id = t.recording_id
                   WHERE r.audio_status IN ('none', 'failed') AND t.deezer_id IS NOT NULL
                   ORDER BY l.created_at DESC LIMIT 10)
                  UNION ALL
                  (SELECT t.id, 1, t.created_at, COALESCE(t.rank, 0)
                   FROM tracks t JOIN recordings r ON r.id = t.recording_id
                   WHERE r.audio_status = 'none' AND t.deezer_id IS NOT NULL
                   ORDER BY t.rank DESC NULLS LAST LIMIT 10)
               ) t
               ORDER BY t.tier, t.at DESC, t.rank DESC"#
            )
            .fetch_all(&st.db)
            .await?
        }
    };
    let mut acquisition = None;
    for track_id in candidates {
        let mut request = Request::new(track_id, AcquireReason::Manual, lanes::NOW)
            .by(admin)
            .priority(MANUAL_PRIORITY);
        request.force = body.track_id.is_some();
        match acquire::request(&st.db, &st.deezer, request).await? {
            Enqueue::Queued { acquisition_id, .. } | Enqueue::Active { acquisition_id } => {
                acquisition = Some(acquisition_id);
                break;
            }
            Enqueue::Ready | Enqueue::Skipped(_) => {}
        }
    }
    let Some(acquisition_id) = acquisition else {
        return Err(AppError::Conflict {
            code: "nothing_to_check",
            message: "скачивать нечего: у всех подходящих треков уже есть файл".into(),
        });
    };
    audit::record(
        &st.db,
        Some(admin),
        audit::SOURCE_CHECK,
        Some(acquisition_id.to_string()),
        json!({ "trackId": body.track_id }),
    )
    .await?;
    Ok(Json(admin_view(&st, acquisition_id, admin).await?))
}
