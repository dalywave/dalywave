//! Выгрузка медиатеки в файл: вся медиатека (ZIP или JSON), любимые треки и плейлист (CSV, M3U8,
//! JSON). Только свои данные: чужой плейлист — 404. Файл собирается потоком (`crate::export`).

use axum::{Router, extract::State, response::Response, routing::get};
use chrono::Utc;
use dalywave_protocol::{
    ErrorBody,
    export::{ExportFormat, ExportQuery},
};
use uuid::Uuid;

use crate::{
    auth::Principal,
    error::{AppError, AppResult, bad},
    export::{self, List},
    extract::{Path, Query},
    state::AppState,
};

/// Под слоем медиатеки (право library).
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/export/library", get(export_library))
        .route("/export/liked", get(export_liked))
        .route("/export/playlists/{id}", get(export_playlist))
}

fn permit() -> AppResult<tokio::sync::OwnedSemaphorePermit> {
    export::limiter()
        .try_acquire_owned()
        .map_err(|_| AppError::TooManyRequests)
}

/// Формат списка: CSV по умолчанию; архив — только для всей медиатеки.
fn list_format(q: &ExportQuery) -> AppResult<ExportFormat> {
    match q.format.unwrap_or(ExportFormat::Csv) {
        ExportFormat::Zip => Err(bad("архивом выгружается только вся медиатека")),
        f => Ok(f),
    }
}

#[utoipa::path(
    get,
    path = "/api/export/library",
    tag = "library",
    security(("session" = []), ("device" = [])),
    params(ExportQuery),
    responses(
        (status = 200, description = "Файл (`Content-Disposition: attachment`). `zip` — CSV и M3U8 любимых и \
                                      каждого плейлиста (по папкам), таблицы альбомов и артистов, полный JSON; \
                                      `json` — всё одним файлом. `history=true` — с историей прослушиваний",
         content_type = "application/zip"),
        (status = 400, description = "Формат не `zip` и не `json`", body = ErrorBody),
        (status = 429, description = "Сервер уже собирает несколько выгрузок — попробовать через минуту", body = ErrorBody),
    ),
)]
pub async fn export_library(
    State(st): State<AppState>,
    principal: Principal,
    Query(q): Query<ExportQuery>,
) -> AppResult<Response> {
    let format = q.format.unwrap_or(ExportFormat::Zip);
    if !matches!(format, ExportFormat::Zip | ExportFormat::Json) {
        return Err(bad("вся медиатека выгружается архивом или JSON"));
    }
    let permit = permit()?;
    let name = format!(
        "DalyWave {}.{}",
        Utc::now().format("%Y-%m-%d"),
        format.extension()
    );
    let (db, url, user, history) = (
        st.db.clone(),
        st.config.public_url.clone(),
        principal.user.id,
        q.history,
    );
    Ok(export::respond(format, &name, &name, permit, move |out| {
        export::library(out, db, url, user, format, history)
    }))
}

#[utoipa::path(
    get,
    path = "/api/export/liked",
    tag = "library",
    security(("session" = []), ("device" = [])),
    params(ExportQuery),
    responses(
        (status = 200, description = "Любимые треки файлом: `csv` (по умолчанию), `m3u8` или `json`", content_type = "text/csv"),
        (status = 400, description = "Архив — только для всей медиатеки", body = ErrorBody),
        (status = 429, description = "Сервер уже собирает несколько выгрузок", body = ErrorBody),
    ),
)]
pub async fn export_liked(
    State(st): State<AppState>,
    principal: Principal,
    Query(q): Query<ExportQuery>,
) -> AppResult<Response> {
    let format = list_format(&q)?;
    let permit = permit()?;
    let title = "Любимые треки".to_string();
    let name = format!("{title}.{}", format.extension());
    let fallback = format!("liked.{}", format.extension());
    let (db, url, user) = (
        st.db.clone(),
        st.config.public_url.clone(),
        principal.user.id,
    );
    Ok(export::respond(
        format,
        &name,
        &fallback,
        permit,
        move |out| export::single(out, db, url, List::Liked(user), title, format),
    ))
}

#[utoipa::path(
    get,
    path = "/api/export/playlists/{id}",
    tag = "library",
    security(("session" = []), ("device" = [])),
    params(("id" = Uuid, Path, description = "id плейлиста"), ExportQuery),
    responses(
        (status = 200, description = "Плейлист файлом в его порядке: `csv` (по умолчанию), `m3u8` или `json`", content_type = "text/csv"),
        (status = 400, description = "Архив — только для всей медиатеки", body = ErrorBody),
        (status = 404, description = "Нет такого плейлиста или он чужой (совместный — только участникам)", body = ErrorBody),
        (status = 429, description = "Сервер уже собирает несколько выгрузок", body = ErrorBody),
    ),
)]
pub async fn export_playlist(
    State(st): State<AppState>,
    principal: Principal,
    Path(id): Path<Uuid>,
    Query(q): Query<ExportQuery>,
) -> AppResult<Response> {
    let format = list_format(&q)?;
    let user = principal.user.id;
    let title = sqlx::query_scalar!(
        "SELECT p.title FROM playlists p
         WHERE p.id = $1 AND (p.user_id = $2 OR (p.collaborative AND EXISTS (
             SELECT 1 FROM playlist_members m WHERE m.playlist_id = p.id AND m.user_id = $2)))",
        id,
        user
    )
    .fetch_optional(&st.db)
    .await?
    .ok_or(AppError::NotFound)?;
    let permit = permit()?;
    let name = format!(
        "{}.{}",
        export::file_name(&title, "Плейлист"),
        format.extension()
    );
    let fallback = format!("playlist.{}", format.extension());
    let (db, url) = (st.db.clone(), st.config.public_url.clone());
    Ok(export::respond(
        format,
        &name,
        &fallback,
        permit,
        move |out| export::single(out, db, url, List::Playlist(id), title, format),
    ))
}
