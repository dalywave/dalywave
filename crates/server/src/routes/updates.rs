//! Автообновление приложений (tauri-plugin-updater). Пока обновлений нет: ответ 204, плагин понимает
//! его как «у вас последняя версия». Открыто без входа — приложение спрашивает и до входа.

use axum::{Router, http::StatusCode, routing::get};

use crate::{extract::Path, state::AppState};

pub fn routes() -> Router<AppState> {
    Router::new().route("/updates/{target}/{arch}/{current_version}", get(check))
}

#[utoipa::path(
    get,
    path = "/api/updates/{target}/{arch}/{current_version}",
    tag = "system",
    params(
        ("target" = String, Path, description = "Платформа: `darwin`, `windows`, `linux`, `ios`"),
        ("arch" = String, Path, description = "Архитектура: `aarch64`, `x86_64`"),
        ("current_version" = String, Path, description = "Установленная версия, например `0.1.0`"),
    ),
    responses((status = 204, description = "Обновлений нет")),
)]
pub async fn check(Path(_): Path<(String, String, String)>) -> StatusCode {
    StatusCode::NO_CONTENT
}
