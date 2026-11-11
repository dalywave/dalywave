//! «Моя волна» и рекомендации слушателя: сессии волны и радио, выдача с причинами, реакции, похожие
//! треки и поиск словами по звучанию. Логика — [`crate::reco::wave`].

use axum::{
    Router,
    extract::State,
    http::StatusCode,
    routing::{get, post},
};
use dalywave_protocol::{
    ErrorBody,
    wave::{
        NewWaveSession, NextQuery, SimilarQuery, SoundSearch, SoundSearchResults, WaveFeedback,
        WaveItems, WaveSession,
    },
};
use uuid::Uuid;

use crate::{
    auth::Principal,
    cursor,
    error::AppResult,
    extract::{Json, Path, Query},
    reco::{prep, wave},
    state::AppState,
};

/// Сессии волны: под правом play — выдача ставит следующие треки на скачивание.
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/wave/sessions", post(create_session))
        .route("/wave/sessions/{id}", get(session))
        .route("/wave/sessions/{id}/next", get(next))
        .route("/wave/sessions/{id}/feedback", post(feedback))
}

/// Похожие и поиск по звучанию — часть каталога.
pub fn catalog_routes() -> Router<AppState> {
    Router::new()
        .route("/tracks/{id}/similar", get(similar))
        .route("/search/sound", post(sound_search))
}

#[utoipa::path(
    post,
    path = "/api/wave/sessions",
    tag = "wave",
    security(("session" = []), ("device" = [])),
    request_body = NewWaveSession,
    responses(
        (status = 201, description = "Сессия волны; с `seed` — радио от трека, артиста, альбома, плейлиста или описания. \
                                      Начало (`queue`) уже подобрано под настройки. `state = preparing` — готовых на сервере \
                                      меньше `need`: лучшие из неготовых качаются вне очереди, ход — `GET` сессии (и \
                                      `acquisition.updated` по трекам очереди); не дольше полутора минут", body = WaveSession),
        (status = 400, description = "Неверные настройки: нет id или текста затравки, темп вне 100–220", body = ErrorBody),
        (status = 404, description = "Нет трека, артиста, альбома или плейлиста затравки", body = ErrorBody),
        (status = 503, description = "Волна по описанию: сервис анализа не ответил", body = ErrorBody),
    ),
)]
pub async fn create_session(
    State(st): State<AppState>,
    principal: Principal,
    Json(body): Json<NewWaveSession>,
) -> AppResult<(StatusCode, Json<WaveSession>)> {
    let mut session = wave::create(&st, &principal, body.settings).await?;
    let env = wave::Env::of(&st).await?;
    let platform = wave::platform_of(&st, &principal).await?;
    prep::start(&env, &mut session, Some(&platform)).await?;
    Ok((
        StatusCode::CREATED,
        Json(wave::view(&st.db, &session).await?),
    ))
}

#[utoipa::path(
    get,
    path = "/api/wave/sessions/{id}",
    tag = "wave",
    security(("session" = []), ("device" = [])),
    params(("id" = Uuid, Path, description = "id сессии")),
    responses(
        (status = 200, description = "Сессия: настройки, подпись, сколько выдано и ход подготовки начала: готовое — \
                                      первым; что не скачалось — заменено; за полторы минуты не успели — начало из того, \
                                      что уже на сервере", body = WaveSession),
        (status = 404, description = "Нет такой сессии у этого человека", body = ErrorBody),
    ),
)]
pub async fn session(
    State(st): State<AppState>,
    principal: Principal,
    Path(id): Path<Uuid>,
) -> AppResult<Json<WaveSession>> {
    let mut session = wave::load(&st.db, principal.user.id, id).await?;
    if session.state.prep.as_ref().is_some_and(|p| !p.done) {
        let env = wave::Env::of(&st).await?;
        let platform = wave::platform_of(&st, &principal).await?;
        prep::refresh(&env, &mut session, Some(&platform)).await?;
    }
    Ok(Json(wave::view(&st.db, &session).await?))
}

#[utoipa::path(
    get,
    path = "/api/wave/sessions/{id}/next",
    tag = "wave",
    security(("session" = []), ("device" = [])),
    params(("id" = Uuid, Path, description = "id сессии"), NextQuery),
    responses(
        (status = 200, description = "Следующие треки с причиной, темпом и тональностью; то, чего нет на сервере, \
                                      уже скачивается (полоса «Волна»). Выданное в сессии не повторяется", body = WaveItems),
        (status = 400, description = "count вне 1–30", body = ErrorBody),
        (status = 404, description = "Нет такой сессии", body = ErrorBody),
    ),
)]
pub async fn next(
    State(st): State<AppState>,
    principal: Principal,
    Path(id): Path<Uuid>,
    Query(q): Query<NextQuery>,
) -> AppResult<Json<WaveItems>> {
    let count = cursor::limit(q.count, 10, wave::MAX_COUNT as u32)?;
    let items = wave::next(&st, &principal, id, count as usize).await?;
    Ok(Json(WaveItems { items }))
}

#[utoipa::path(
    post,
    path = "/api/wave/sessions/{id}/feedback",
    tag = "wave",
    security(("session" = []), ("device" = [])),
    params(("id" = Uuid, Path, description = "id сессии")),
    request_body = WaveFeedback,
    responses(
        (status = 204, description = "Реакция учтена: следующая выдача её учитывает; лайк — ещё и в медиатеке"),
        (status = 404, description = "Нет такой сессии или трека", body = ErrorBody),
    ),
)]
pub async fn feedback(
    State(st): State<AppState>,
    principal: Principal,
    Path(id): Path<Uuid>,
    Json(body): Json<WaveFeedback>,
) -> AppResult<StatusCode> {
    wave::feedback(&st, &principal, id, body).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    get,
    path = "/api/tracks/{id}/similar",
    tag = "catalog",
    security(("session" = []), ("device" = [])),
    params(("id" = Uuid, Path, description = "id трека"), SimilarQuery),
    responses(
        (status = 200, description = "Похожие по звучанию, не больше двух у артиста. Трек ещё не разобран — \
                                      популярное похожих артистов, а превью уже анализируется", body = WaveItems),
        (status = 400, description = "limit вне 1–50", body = ErrorBody),
        (status = 404, description = "Нет такого трека", body = ErrorBody),
    ),
)]
pub async fn similar(
    State(st): State<AppState>,
    principal: Principal,
    Path(id): Path<Uuid>,
    Query(q): Query<SimilarQuery>,
) -> AppResult<Json<WaveItems>> {
    let limit = cursor::limit(q.limit, 20, 50)?;
    let items = wave::similar(&st, principal.user.id, id, limit as usize).await?;
    Ok(Json(WaveItems { items }))
}

#[utoipa::path(
    post,
    path = "/api/search/sound",
    tag = "catalog",
    security(("session" = []), ("device" = [])),
    request_body = SoundSearch,
    responses(
        (status = 200, description = "Ближайшее по звучанию к описанию: из медиатеки и из индекса каталога", body = SoundSearchResults),
        (status = 400, description = "Пустой или слишком длинный запрос, limit вне 1–50", body = ErrorBody),
        (status = 503, description = "Сервис анализа не ответил", body = ErrorBody),
    ),
)]
pub async fn sound_search(
    State(st): State<AppState>,
    principal: Principal,
    Json(body): Json<SoundSearch>,
) -> AppResult<Json<SoundSearchResults>> {
    let limit = cursor::limit(body.limit, 20, 50)?;
    Ok(Json(
        wave::sound_search(&st, principal.user.id, &body.text, limit as usize).await?,
    ))
}
