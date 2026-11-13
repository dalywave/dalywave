//! «Мой вкус» слушателя: что волна знает о нём и ручки управления — «меньше такого» для грани и сброс
//! вкуса. «Не рекомендовать» — `/api/library/blocked`. Логика — [`crate::reco::profile`].
//!
//! `/api/taste/training/*` — обучение сети вкуса на компьютере с приложением ([`crate::reco::training`]).

use axum::{
    Router,
    extract::{DefaultBodyLimit, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{delete, get, post},
};
use chrono::Utc;
use dalywave_protocol::{
    ErrorBody,
    taste::{
        TasteDamper, TasteOverview, TasteReset, TrainClaimBody, TrainFailure, TrainOutcome,
        TrainSubmit,
    },
};
use uuid::Uuid;

use crate::{
    auth::Principal,
    error::{AppError, AppResult, bad},
    extract::{Json, Path},
    reco::{profile, taste, training},
    state::AppState,
};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/taste", get(overview))
        .route("/taste/facets/{id}/less", post(less))
        .route("/taste/dampers/{id}", delete(remove_damper))
        .route("/taste/reset", post(reset))
        .route("/taste/training/claim", post(train_claim))
        .route(
            "/taste/training/{id}/result",
            post(train_result).layer(DefaultBodyLimit::max(TRAIN_BODY_LIMIT)),
        )
        .route("/taste/training/{id}/fail", post(train_fail))
}

/// Веса сети с запасом: обычно 100–300 КБ.
const TRAIN_BODY_LIMIT: usize = 8 * 1024 * 1024;

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

#[utoipa::path(
    post,
    path = "/api/taste/training/claim",
    tag = "library",
    security(("device" = [])),
    request_body = TrainClaimBody,
    responses(
        (status = 200, description = "Задание: обучающая часть набора и описание обучения. Отложенной части \
                                      в нём нет — её проверяет сервер", body = dalywave_protocol::taste::TrainClaim),
        (status = 204, description = "Учить сейчас нечего: сеть недавно училась, мало данных или задание уже у \
                                      другого компьютера"),
        (status = 400, description = "Не приложение", body = ErrorBody),
        (status = 403, description = "Не компьютер, или общую сеть просит не администратор", body = ErrorBody),
    ),
)]
pub async fn train_claim(
    State(st): State<AppState>,
    principal: Principal,
    Json(body): Json<TrainClaimBody>,
) -> AppResult<Response> {
    Ok(
        match training::claim(&st.db, &principal, body.scope).await? {
            Some(claim) => Json(claim).into_response(),
            None => StatusCode::NO_CONTENT.into_response(),
        },
    )
}

#[utoipa::path(
    post,
    path = "/api/taste/training/{id}/result",
    tag = "library",
    security(("device" = [])),
    params(("id" = Uuid, Path, description = "id задания")),
    request_body = TrainSubmit,
    responses(
        (status = 200, description = "Веса проверены на отложенной части сервера; включены, только если сеть \
                                      точнее правил и текущей версии", body = TrainOutcome),
        (status = 403, description = "Общую сеть присылает только администратор", body = ErrorBody),
        (status = 404, description = "Нет такого задания у этого компьютера", body = ErrorBody),
        (status = 409, description = "Задание закрыто или просрочено", body = ErrorBody),
        (status = 422, description = "Веса не приняты: размеры, числа или ONNX не загружается", body = ErrorBody),
    ),
)]
pub async fn train_result(
    State(st): State<AppState>,
    principal: Principal,
    Path(id): Path<Uuid>,
    Json(body): Json<TrainSubmit>,
) -> AppResult<Json<TrainOutcome>> {
    Ok(Json(
        training::submit(&st.db, &st.storage, &principal, id, body).await?,
    ))
}

#[utoipa::path(
    post,
    path = "/api/taste/training/{id}/fail",
    tag = "library",
    security(("device" = [])),
    params(("id" = Uuid, Path, description = "id задания")),
    request_body = TrainFailure,
    responses(
        (status = 204, description = "Задание закрыто; следующее компьютер получит позже"),
        (status = 404, description = "Нет такого задания у этого компьютера", body = ErrorBody),
        (status = 409, description = "Задание уже закрыто", body = ErrorBody),
    ),
)]
pub async fn train_fail(
    State(st): State<AppState>,
    principal: Principal,
    Path(id): Path<Uuid>,
    Json(body): Json<TrainFailure>,
) -> AppResult<StatusCode> {
    training::fail(&st.db, &principal, id, &body.message).await?;
    Ok(StatusCode::NO_CONTENT)
}
