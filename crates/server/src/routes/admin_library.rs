//! Админка «Библиотека»: папки своей музыки, ход сканирования, файлы с ошибками, «Пересканировать».

use axum::{
    Router,
    extract::State,
    http::StatusCode,
    routing::{get, post},
};
use dalywave_protocol::{ErrorBody, admin::LibraryStatus};

use crate::{
    error::{AppError, AppResult},
    extract::Json,
    library,
    state::AppState,
};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/library", get(status))
        .route("/library/scan", post(rescan))
}

#[utoipa::path(
    get,
    path = "/api/admin/library",
    tag = "admin",
    security(("session" = []), ("device" = [])),
    responses(
        (status = 200, description = "Папки своей музыки, идущий или последний проход, файлы с ошибками", body = LibraryStatus),
        (status = 403, description = "Нет права admin", body = ErrorBody),
    ),
)]
pub async fn status(State(st): State<AppState>) -> AppResult<Json<LibraryStatus>> {
    Ok(Json(library::status(&st.db, &st.storage).await?))
}

#[utoipa::path(
    post,
    path = "/api/admin/library/scan",
    tag = "admin",
    security(("session" = []), ("device" = [])),
    responses(
        (status = 202, description = "Полный проход поставлен в очередь (уже стоящий не дублируется)"),
        (status = 409, description = "`no_music_dirs` — папки своей музыки не заданы (DALYWAVE_MUSIC_DIRS)", body = ErrorBody),
        (status = 403, description = "Нет права admin", body = ErrorBody),
    ),
)]
pub async fn rescan(State(st): State<AppState>) -> AppResult<StatusCode> {
    if st.storage.music_dirs().is_empty() {
        return Err(AppError::Conflict {
            code: "no_music_dirs",
            message: "папки своей музыки не заданы".into(),
        });
    }
    library::request_scan(&st.db, Vec::new()).await?;
    Ok(StatusCode::ACCEPTED)
}
