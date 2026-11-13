//! «Скоро выйдет» и «Сохранить заранее» (presave.rs): анонсированный релиз ляжет в медиатеку в день
//! выхода, придёт уведомление.

use axum::{
    Router,
    extract::State,
    http::StatusCode,
    routing::{get, put},
};
use chrono::Utc;
use dalywave_protocol::{
    ErrorBody,
    library::{PresaveResult, UpcomingList},
};
use uuid::Uuid;

use crate::{
    auth::Principal,
    error::{AppError, AppResult},
    extract::{Json, Path},
    presave,
    routes::library,
    state::AppState,
};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/library/upcoming", get(upcoming))
        .route("/library/presaves/{id}", put(presave).delete(cancel))
}

#[utoipa::path(
    get,
    path = "/api/library/upcoming",
    tag = "library",
    security(("session" = []), ("device" = [])),
    responses(
        (status = 200, description = "Сохранённое заранее и анонсы артистов из подписок, ближайшие первыми. \
                                      «Сегодня» — по часовому поясу из настроек уведомлений", body = UpcomingList),
    ),
)]
pub async fn upcoming(
    State(st): State<AppState>,
    principal: Principal,
) -> AppResult<Json<UpcomingList>> {
    let user = principal.user.id;
    let mut conn = st.db.acquire().await?;
    let today = presave::user_today(&mut conn, user, Utc::now()).await?;
    drop(conn);
    Ok(Json(UpcomingList {
        items: presave::upcoming(&st.db, user, today).await?,
    }))
}

#[utoipa::path(
    put,
    path = "/api/library/presaves/{id}",
    tag = "library",
    security(("session" = []), ("device" = [])),
    params(("id" = Uuid, Path, description = "id альбома")),
    responses(
        (status = 200, description = "Сохранено заранее (повтор безопасен). Релиз уже вышел — альбом сразу \
                                      в медиатеке, как `PUT /api/library/albums/{id}` (`savedNow`)", body = PresaveResult),
        (status = 404, description = "Нет такого альбома", body = ErrorBody),
    ),
)]
pub async fn presave(
    State(st): State<AppState>,
    principal: Principal,
    Path(id): Path<Uuid>,
) -> AppResult<Json<PresaveResult>> {
    let user = principal.user.id;
    let release = sqlx::query_scalar!("SELECT release_date FROM albums WHERE id = $1", id)
        .fetch_optional(&st.db)
        .await?
        .ok_or(AppError::NotFound)?;
    let mut conn = st.db.acquire().await?;
    let today = presave::user_today(&mut conn, user, Utc::now()).await?;
    drop(conn);
    if release.is_none_or(|d| d <= today) {
        library::save_album(State(st), principal, Path(id)).await?;
        return Ok(Json(PresaveResult { saved_now: true }));
    }
    // Отменённое раньше и снова сохранённое — снова ждёт
    sqlx::query!(
        "INSERT INTO album_presaves (user_id, album_id) VALUES ($1, $2)
         ON CONFLICT (user_id, album_id) DO UPDATE SET created_at = now(), delivered_at = NULL, cancelled_at = NULL
         WHERE album_presaves.delivered_at IS NOT NULL OR album_presaves.cancelled_at IS NOT NULL",
        user,
        id
    )
    .execute(&st.db)
    .await?;
    Ok(Json(PresaveResult { saved_now: false }))
}

#[utoipa::path(
    delete,
    path = "/api/library/presaves/{id}",
    tag = "library",
    security(("session" = []), ("device" = [])),
    params(("id" = Uuid, Path, description = "id альбома")),
    responses((status = 204, description = "Предсохранение снято (если было)")),
)]
pub async fn cancel(
    State(st): State<AppState>,
    principal: Principal,
    Path(id): Path<Uuid>,
) -> AppResult<StatusCode> {
    sqlx::query!(
        "DELETE FROM album_presaves WHERE user_id = $1 AND album_id = $2 AND delivered_at IS NULL",
        principal.user.id,
        id
    )
    .execute(&st.db)
    .await?;
    Ok(StatusCode::NO_CONTENT)
}
