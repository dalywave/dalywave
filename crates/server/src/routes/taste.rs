//! «Мой вкус» слушателя: что волна знает о нём и ручки управления — «меньше такого» для грани и сброс
//! вкуса. «Не рекомендовать» — `/api/library/blocked`. Логика — [`crate::reco::profile`].

use axum::{
    Router,
    extract::State,
    http::StatusCode,
    routing::{delete, get, post},
};
use chrono::Utc;
use dalywave_protocol::{
    ErrorBody,
    taste::{TasteDamper, TasteOverview, TasteReset},
};
use uuid::Uuid;

use crate::{
    auth::Principal,
    error::{AppError, AppResult, bad},
    extract::{Json, Path},
    reco::{profile, taste},
    state::AppState,
};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/taste", get(overview))
        .route("/taste/facets/{id}/less", post(less))
        .route("/taste/dampers/{id}", delete(remove_damper))
        .route("/taste/reset", post(reset))
}

#[utoipa::path(
    get,
    path = "/api/taste",
    tag = "library",
    security(("session" = []), ("device" = [])),
    responses((status = 200, description = "Грани вкуса с примерами, «меньше такого», прослушивания за месяц \
                                            и из чего волна знает вкус", body = TasteOverview)),
)]
pub async fn overview(
    State(st): State<AppState>,
    principal: Principal,
) -> AppResult<Json<TasteOverview>> {
    Ok(Json(
        profile::overview(&st.db, principal.user.id, Utc::now()).await?,
    ))
}

#[utoipa::path(
    post,
    path = "/api/taste/facets/{id}/less",
    tag = "library",
    security(("session" = []), ("device" = [])),
    params(("id" = Uuid, Path, description = "id грани")),
    responses(
        (status = 200, description = "Грань приглушена: волна и миксы реже берут похожее, её доля во вкусе \
                                      меньше. Повтор — сильнее (до полной силы)", body = TasteDamper),
        (status = 404, description = "Нет такой грани у слушателя", body = ErrorBody),
    ),
)]
pub async fn less(
    State(st): State<AppState>,
    principal: Principal,
    Path(id): Path<Uuid>,
) -> AppResult<Json<TasteDamper>> {
    profile::dampen(&st.db, principal.user.id, id)
        .await?
        .map(Json)
        .ok_or(AppError::NotFound)
}

#[utoipa::path(
    delete,
    path = "/api/taste/dampers/{id}",
    tag = "library",
    security(("session" = []), ("device" = [])),
    params(("id" = Uuid, Path, description = "id приглушения")),
    responses((status = 204, description = "«Меньше такого» отменено (повтор безопасен)")),
)]
pub async fn remove_damper(
    State(st): State<AppState>,
    principal: Principal,
    Path(id): Path<Uuid>,
) -> AppResult<StatusCode> {
    profile::undampen(&st.db, principal.user.id, id).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    post,
    path = "/api/taste/reset",
    tag = "library",
    security(("session" = []), ("device" = [])),
    request_body = TasteReset,
    responses(
        (status = 204, description = "Вкус сброшен: грани, «меньше такого», миксы и заготовки волны собираются \
                                      заново по новым прослушиваниям. Медиатека и «не рекомендовать» остаются"),
        (status = 400, description = "Нет подтверждения (`confirm: true`)", body = ErrorBody),
    ),
)]
pub async fn reset(
    State(st): State<AppState>,
    principal: Principal,
    Json(body): Json<TasteReset>,
) -> AppResult<StatusCode> {
    if !body.confirm {
        return Err(bad("сброс вкуса нужно подтвердить"));
    }
    let user = principal.user.id;
    profile::reset(&st.db, user).await?;
    let mut conn = st.db.acquire().await?;
    taste::request(&mut conn, user, true).await?;
    Ok(StatusCode::NO_CONTENT)
}
