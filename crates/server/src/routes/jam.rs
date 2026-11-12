//! «Слушать вместе»: управление сессией у хозяина (`/api/jam`, право play), вход гостя по коду
//! (`/api/jam/join/{code}`, без входа) и всё, что может гость (`/api/jam/guest/*`, только токен
//! `dwj_…` этой сессии). Логика — [`crate::jam`].

use std::time::{Duration, Instant};

use axum::{
    Router,
    extract::{
        FromRequestParts, State, WebSocketUpgrade,
        ws::{CloseFrame, Message, WebSocket, rejection::WebSocketUpgradeRejection},
    },
    http::{HeaderMap, StatusCode, header, request::Parts},
    response::{IntoResponse, Response},
    routing::{delete, get, post},
};
use chrono::Utc;
use dalywave_protocol::{
    ErrorBody,
    connect::RemoteCommand,
    jam::{
        Jam, JamAdd, JamGuestView, JamInvite, JamJoin, JamJoined, JamNowEvent, JamOverview,
        JamPatch, JamReact, JamReaction, JamRequest, JamRequestStatus, JamSearchQuery,
        JamSearchResults, JamSkip, JamStream,
    },
    realtime::{ClientMessage, Pong, ServerEvent},
};
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use tokio::sync::broadcast::error::RecvError;
use utoipa::IntoParams;
use uuid::Uuid;

use crate::{
    acquire,
    auth::{self, Principal},
    catalog::{self, Types},
    error::{AppError, AppResult, bad},
    extract::{Json, Path, Query},
    jam::{self, Guest, Session},
    realtime::{self, Audience, Envelope},
    reco,
    state::{AppState, ClientIp},
};

const MINUTE: Duration = Duration::from_secs(60);
/// Ссылка на аудио гостю живёт недолго: только текущий и следующий трек.
const GUEST_AUDIO_TTL: Duration = Duration::from_secs(45 * 60);
/// Как часто соединение гостя перепроверяет токен и напоминает, что гость на связи.
const RECHECK: Duration = Duration::from_secs(30);
const HEARTBEAT: Duration = Duration::from_secs(25);
const CLOSE_SESSION: u16 = 4001;
const CLOSE_RESTART: u16 = 1012;

/// Хозяин: под слоем права play.
pub fn host_routes() -> Router<AppState> {
    Router::new()
        .route("/jam", get(overview).post(start).patch(update).delete(end))
        .route("/jam/guests/{id}", delete(remove_guest))
        .route("/jam/requests/{id}/approve", post(approve))
        .route("/jam/requests/{id}/decline", post(decline))
}

/// Гости: без слоёв входа — каждый обработчик проверяет код или токен гостя сам.
pub fn guest_routes() -> Router<AppState> {
    Router::new()
        .route("/jam/join/{code}", get(invite).post(join))
        .route("/jam/guest", get(guest_view))
        .route("/jam/guest/search", get(search))
        .route("/jam/guest/queue", post(add))
        .route("/jam/guest/react", post(react))
        .route("/jam/guest/skip", post(skip))
        .route("/jam/guest/stream/{trackId}", get(stream))
        .route("/jam/guest/leave", post(leave))
        .route("/jam/guest/ws", get(guest_ws))
}

fn too_many() -> AppError {
    AppError::TooManyRequests
}

// ---------- хозяин ----------

async fn open(st: &AppState, principal: &Principal) -> AppResult<Session> {
    jam::open_of(&st.db, principal.user.id)
        .await?
        .ok_or_else(jam::ended)
}

#[utoipa::path(
    get,
    path = "/api/jam",
    tag = "jam",
    operation_id = "jam_overview",
    security(("session" = []), ("device" = [])),
    responses(
        (status = 200, description = "Открытая сессия «Слушать вместе» или `jam: null`", body = JamOverview),
    ),
)]
pub async fn overview(
    State(st): State<AppState>,
    principal: Principal,
) -> AppResult<Json<JamOverview>> {
    let jam = match jam::open_of(&st.db, principal.user.id).await? {
        Some(s) => Some(jam::host_view(&st, &s).await?),
        None => None,
    };
    Ok(Json(JamOverview { jam }))
}

#[utoipa::path(
    post,
    path = "/api/jam",
    tag = "jam",
    operation_id = "jam_start",
    security(("session" = []), ("device" = [])),
    responses(
        (status = 200, description = "Сессия уже открыта — она же", body = Jam),
        (status = 201, description = "Новая сессия: код, ссылка для гостей", body = Jam),
    ),
)]
pub async fn start(
    State(st): State<AppState>,
    principal: Principal,
) -> AppResult<(StatusCode, Json<Jam>)> {
    if let Some(s) = jam::open_of(&st.db, principal.user.id).await? {
        jam::touch(&st.db, s.id, None).await?;
        return Ok((StatusCode::OK, Json(jam::host_view(&st, &s).await?)));
    }
    let code = jam::new_code()?;
    // Гонка двух вкладок: уникальный индекс оставит одну сессию, вторая получит её же
    sqlx::query!(
        "INSERT INTO jam_sessions (user_id, code) VALUES ($1, $2) ON CONFLICT DO NOTHING",
        principal.user.id,
        code,
    )
    .execute(&st.db)
    .await?;
    let s = open(&st, &principal).await?;
    jam::changed(&st, &s, false, None);
    Ok((StatusCode::CREATED, Json(jam::host_view(&st, &s).await?)))
}

#[utoipa::path(
    patch,
    path = "/api/jam",
    tag = "jam",
    operation_id = "jam_update",
    security(("session" = []), ("device" = [])),
    request_body = JamPatch,
    responses(
        (status = 200, description = "Сессия с новыми настройками", body = Jam),
        (status = 404, description = "Открытой сессии нет (`jam_ended`)", body = ErrorBody),
    ),
)]
pub async fn update(
    State(st): State<AppState>,
    principal: Principal,
    Json(patch): Json<JamPatch>,
) -> AppResult<Json<Jam>> {
    let s = open(&st, &principal).await?;
    sqlx::query!(
        "UPDATE jam_sessions SET guests_can_add = coalesce($2, guests_can_add),
             approval = coalesce($3, approval), guests_can_listen = coalesce($4, guests_can_listen),
             last_activity_at = now()
         WHERE id = $1",
        s.id,
        patch.guests_can_add,
        patch.approval,
        patch.guests_can_listen,
    )
    .execute(&st.db)
    .await?;
    let s = open(&st, &principal).await?;
    jam::changed(&st, &s, false, None);
    Ok(Json(jam::host_view(&st, &s).await?))
}

#[utoipa::path(
    delete,
    path = "/api/jam",
    tag = "jam",
    operation_id = "jam_end",
    security(("session" = []), ("device" = [])),
    responses((status = 204, description = "Сессия закончена: токены гостей больше не действуют")),
)]
pub async fn end(State(st): State<AppState>, principal: Principal) -> AppResult<StatusCode> {
    if let Some(s) = jam::open_of(&st.db, principal.user.id).await? {
        sqlx::query!(
            "UPDATE jam_sessions SET ended_at = now() WHERE id = $1",
            s.id
        )
        .execute(&st.db)
        .await?;
        jam::changed(&st, &s, true, None);
    }
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    delete,
    path = "/api/jam/guests/{id}",
    tag = "jam",
    operation_id = "jam_remove_guest",
    security(("session" = []), ("device" = [])),
    params(("id" = Uuid, Path, description = "id гостя")),
    responses(
        (status = 204, description = "Гость убран: его токен больше не действует"),
        (status = 404, description = "Нет такого гостя в открытой сессии", body = ErrorBody),
    ),
)]
pub async fn remove_guest(
    State(st): State<AppState>,
    principal: Principal,
    Path(id): Path<Uuid>,
) -> AppResult<StatusCode> {
    let s = open(&st, &principal).await?;
    let res = sqlx::query!(
        "UPDATE jam_guests SET removed_at = now() WHERE id = $1 AND jam_id = $2 AND removed_at IS NULL",
        id,
        s.id,
    )
    .execute(&st.db)
    .await?;
    if res.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }
    // Его ждущие треки — отклонены
    sqlx::query!(
        "UPDATE jam_requests SET status = 'declined', decided_at = now() WHERE guest_id = $1 AND status = 'pending'",
        id,
    )
    .execute(&st.db)
    .await?;
    jam::changed(&st, &s, false, Some(id));
    Ok(StatusCode::NO_CONTENT)
}

async fn pending_request(st: &AppState, s: &Session, id: Uuid) -> AppResult<Uuid> {
    sqlx::query_scalar!(
        "SELECT track_id FROM jam_requests WHERE id = $1 AND jam_id = $2 AND status = 'pending'",
        id,
        s.id,
    )
    .fetch_optional(&st.db)
    .await?
    .ok_or(AppError::NotFound)
}

#[utoipa::path(
    post,
    path = "/api/jam/requests/{id}/approve",
    tag = "jam",
    operation_id = "jam_approve",
    security(("session" = []), ("device" = [])),
    params(("id" = Uuid, Path, description = "id трека гостя")),
    responses(
        (status = 200, description = "Трек ушёл в очередь активного устройства", body = Jam),
        (status = 404, description = "Нет такого ждущего трека", body = ErrorBody),
        (status = 409, description = "Активного устройства нет на связи (`nothing_playing`)", body = ErrorBody),
    ),
)]
pub async fn approve(
    State(st): State<AppState>,
    principal: Principal,
    Path(id): Path<Uuid>,
) -> AppResult<Json<Jam>> {
    let s = open(&st, &principal).await?;
    let track = pending_request(&st, &s, id).await?;
    if !jam::enqueue(&st, s.host, track).await? {
        return Err(jam::nothing_playing());
    }
    sqlx::query!(
        "UPDATE jam_requests SET status = 'added', decided_at = now() WHERE id = $1",
        id
    )
    .execute(&st.db)
    .await?;
    jam::touch(&st.db, s.id, None).await?;
    jam::changed(&st, &s, false, None);
    Ok(Json(jam::host_view(&st, &s).await?))
}

#[utoipa::path(
    post,
    path = "/api/jam/requests/{id}/decline",
    tag = "jam",
    operation_id = "jam_decline",
    security(("session" = []), ("device" = [])),
    params(("id" = Uuid, Path, description = "id трека гостя")),
    responses(
        (status = 200, description = "Трек отклонён", body = Jam),
        (status = 404, description = "Нет такого ждущего трека", body = ErrorBody),
    ),
)]
pub async fn decline(
    State(st): State<AppState>,
    principal: Principal,
    Path(id): Path<Uuid>,
) -> AppResult<Json<Jam>> {
    let s = open(&st, &principal).await?;
    pending_request(&st, &s, id).await?;
    sqlx::query!(
        "UPDATE jam_requests SET status = 'declined', decided_at = now() WHERE id = $1",
        id
    )
    .execute(&st.db)
    .await?;
    jam::changed(&st, &s, false, None);
    Ok(Json(jam::host_view(&st, &s).await?))
}

// ---------- вход гостя ----------

/// Человек со своим аккаунтом (cookie или токен приложения) — имя возьмётся оттуда. Ошибки входа
/// не мешают: гостю аккаунт не нужен.
async fn member(st: &AppState, headers: &HeaderMap) -> Option<Principal> {
    if auth::bearer(headers).is_some_and(|t| t.starts_with(jam::TOKEN_PREFIX)) {
        return None;
    }
    auth::authenticate(st, headers)
        .await
        .ok()
        .flatten()
        .filter(|p| !p.roles.is_empty())
}

#[utoipa::path(
    get,
    path = "/api/jam/join/{code}",
    tag = "jam",
    operation_id = "jam_invite",
    params(("code" = String, Path, description = "Код из ссылки")),
    responses(
        (status = 200, description = "Кто приглашает и что играет", body = JamInvite),
        (status = 404, description = "Нет такой открытой сессии (`jam_ended`)", body = ErrorBody),
        (status = 429, description = "Слишком часто с одного адреса", body = ErrorBody),
    ),
)]
pub async fn invite(
    State(st): State<AppState>,
    ClientIp(ip): ClientIp,
    headers: HeaderMap,
    Path(code): Path<String>,
) -> AppResult<Json<JamInvite>> {
    if !st.limiter.check("jam_invite", ip, 60, MINUTE) {
        return Err(too_many());
    }
    let (s, host_name, host_avatar) = jam::by_code(&st.db, &code).await?.ok_or_else(jam::ended)?;
    let guests = jam::guests(&st.db, s.id).await?.len() as u32;
    let current = jam::now_playing(&st.db, &s)
        .await?
        .and_then(|n| n.current)
        .map(|c| c.track);
    let member_name = member(&st, &headers)
        .await
        .and_then(|p| jam::clean_name(&p.user.name));
    Ok(Json(JamInvite {
        host_name,
        host_avatar,
        guests,
        current,
        member_name,
    }))
}

#[utoipa::path(
    post,
    path = "/api/jam/join/{code}",
    tag = "jam",
    operation_id = "jam_join",
    params(("code" = String, Path, description = "Код из ссылки")),
    request_body = JamJoin,
    responses(
        (status = 201, description = "Гость в сессии: токен `dwj_…` (показывается один раз)", body = JamJoined),
        (status = 400, description = "Нет имени", body = ErrorBody),
        (status = 404, description = "Нет такой открытой сессии (`jam_ended`)", body = ErrorBody),
        (status = 429, description = "Слишком много входов с одного адреса", body = ErrorBody),
    ),
)]
pub async fn join(
    State(st): State<AppState>,
    ClientIp(ip): ClientIp,
    headers: HeaderMap,
    Path(code): Path<String>,
    Json(body): Json<JamJoin>,
) -> AppResult<(StatusCode, Json<JamJoined>)> {
    if !st
        .limiter
        .check("jam_join", ip, 10, Duration::from_secs(600))
    {
        return Err(too_many());
    }
    let (s, _, _) = jam::by_code(&st.db, &code).await?.ok_or_else(jam::ended)?;
    let member = member(&st, &headers).await;
    let name = body
        .name
        .as_deref()
        .and_then(jam::clean_name)
        .or_else(|| member.as_ref().and_then(|p| jam::clean_name(&p.user.name)))
        .ok_or_else(|| bad("как вас зовут?"))?;
    // Хозяин в своей же сессии — не гость
    let member_id = member.map(|p| p.user.id).filter(|id| *id != s.host);
    let (token, guest) = jam::join(&st.db, s.id, &name, member_id).await?;
    jam::changed(&st, &s, false, None);
    Ok((StatusCode::CREATED, Json(JamJoined { token, guest })))
}

// ---------- гость ----------

/// Гость с действующим токеном `dwj_…`: проверка на каждом запросе.
pub struct GuestAuth(pub Guest);

impl FromRequestParts<AppState> for GuestAuth {
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, st: &AppState) -> Result<Self, Self::Rejection> {
        let token = auth::bearer(&parts.headers).ok_or(AppError::Unauthorized)?;
        let guest = jam::guest_by_token(&st.db, token).await?;
        jam::touch(&st.db, guest.session.id, Some(guest.id)).await?;
        Ok(Self(guest))
    }
}

#[utoipa::path(
    get,
    path = "/api/jam/guest",
    tag = "jam",
    operation_id = "jam_guest_view",
    security(("jam" = [])),
    responses(
        (status = 200, description = "Страница гостя: что играет, очередь, участники, свои треки", body = JamGuestView),
        (status = 401, description = "Нет токена гостя", body = ErrorBody),
        (status = 404, description = "Сессия закончилась (`jam_ended`) или хозяин убрал гостя (`jam_removed`)", body = ErrorBody),
    ),
)]
pub async fn guest_view(
    State(st): State<AppState>,
    GuestAuth(g): GuestAuth,
) -> AppResult<Json<JamGuestView>> {
    let s = &g.session;
    let guests = jam::guests(&st.db, s.id).await?;
    let me =
        guests
            .iter()
            .find(|x| x.id == g.id)
            .cloned()
            .unwrap_or(dalywave_protocol::jam::JamGuest {
                id: g.id,
                name: g.name.clone(),
                member: g.member,
                online: true,
                joined_at: g.joined_at,
            });
    let now = jam::now_playing(&st.db, s).await?;
    let current = now
        .as_ref()
        .and_then(|n| n.current.as_ref())
        .map(|c| c.track.id);
    Ok(Json(JamGuestView {
        jam_id: s.id,
        host_name: g.host_name.clone(),
        settings: s.settings,
        me,
        guests,
        now,
        mine: jam::requests(&st.db, s.id, Some(g.id), JamRequestStatus::ALL).await?,
        skip: jam::skip_state(&st.db, s.id, current, Some(g.id)).await?,
        expires_at: s.expires_at(),
    }))
}

#[utoipa::path(
    get,
    path = "/api/jam/guest/search",
    tag = "jam",
    operation_id = "jam_search",
    security(("jam" = [])),
    params(JamSearchQuery),
    responses(
        (status = 200, description = "Треки каталога (до 20)", body = JamSearchResults),
        (status = 429, description = "Больше 30 поисков в минуту", body = ErrorBody),
        (status = 503, description = "Deezer не ответил", body = ErrorBody),
    ),
)]
pub async fn search(
    State(st): State<AppState>,
    GuestAuth(g): GuestAuth,
    Query(q): Query<JamSearchQuery>,
) -> AppResult<Json<JamSearchResults>> {
    let text = q.q.trim();
    if text.is_empty() || text.chars().count() > 100 {
        return Err(bad("пустой или слишком длинный запрос"));
    }
    if !st.limiter.check_id("jam_search", g.id, 30, MINUTE) {
        return Err(too_many());
    }
    let types = Types {
        track: true,
        album: false,
        artist: false,
    };
    let hits = catalog::search(&st.db, &st.deezer, text, types, 20).await?;
    let mut items = jam::jam_tracks(&st.db, &hits.tracks).await?;
    // У хозяина «Без нецензурного» — гостю explicit не показываем
    if reco::hide_explicit(&st.db, g.session.host).await? {
        items.retain(|t| !t.explicit);
    }
    Ok(Json(JamSearchResults { items }))
}

#[utoipa::path(
    post,
    path = "/api/jam/guest/queue",
    tag = "jam",
    operation_id = "jam_add",
    security(("jam" = [])),
    request_body = JamAdd,
    responses(
        (status = 201, description = "Трек в очереди (`added`) или ждёт одобрения хозяина (`pending`)", body = JamRequest),
        (status = 403, description = "Хозяин выключил добавление гостями", body = ErrorBody),
        (status = 404, description = "Нет такого трека", body = ErrorBody),
        (status = 409, description = "`already_queued` — уже в очереди; `too_many_pending` — пять треков уже ждут; \
                                      `nothing_playing` — у хозяина ничего не на связи; \
                                      `explicit_off` — хозяин слушает без нецензурного, а трек explicit", body = ErrorBody),
        (status = 429, description = "Больше 10 треков в минуту", body = ErrorBody),
    ),
)]
pub async fn add(
    State(st): State<AppState>,
    GuestAuth(g): GuestAuth,
    Json(body): Json<JamAdd>,
) -> AppResult<(StatusCode, Json<JamRequest>)> {
    let s = &g.session;
    if !s.settings.guests_can_add {
        return Err(AppError::Forbidden(
            "хозяин выключил добавление треков".into(),
        ));
    }
    if !st.limiter.check_id("jam_add", g.id, 10, MINUTE) {
        return Err(too_many());
    }
    let exists = sqlx::query_scalar!(
        r#"SELECT EXISTS (SELECT 1 FROM tracks WHERE id = $1) AS "e!""#,
        body.track_id
    )
    .fetch_one(&st.db)
    .await?;
    if !exists {
        return Err(AppError::NotFound);
    }
    if reco::hide_explicit(&st.db, s.host).await?
        && jam::jam_tracks(&st.db, &[body.track_id])
            .await?
            .first()
            .is_some_and(|t| t.explicit)
    {
        return Err(AppError::Conflict {
            code: "explicit_off",
            message: "Хозяин слушает без нецензурного".into(),
        });
    }
    let row = sqlx::query!(
        r#"SELECT
             EXISTS (SELECT 1 FROM jam_requests WHERE jam_id = $1 AND track_id = $2
                     AND (status = 'pending' OR (status = 'added' AND created_at > now() - interval '1 hour'))) AS "dup!",
             (SELECT count(*) FROM jam_requests WHERE guest_id = $3 AND status = 'pending') AS "pending!""#,
        s.id,
        body.track_id,
        g.id,
    )
    .fetch_one(&st.db)
    .await?;
    if row.dup {
        return Err(AppError::Conflict {
            code: "already_queued",
            message: "Этот трек уже в очереди".into(),
        });
    }
    let status = if s.settings.approval {
        if row.pending >= jam::MAX_PENDING {
            return Err(AppError::Conflict {
                code: "too_many_pending",
                message: "Подождите, пока хозяин посмотрит предыдущие треки".into(),
            });
        }
        JamRequestStatus::Pending
    } else {
        if !jam::enqueue(&st, s.host, body.track_id).await? {
            return Err(jam::nothing_playing());
        }
        JamRequestStatus::Added
    };
    let id = sqlx::query_scalar!(
        "INSERT INTO jam_requests (jam_id, guest_id, track_id, status, decided_at)
         VALUES ($1, $2, $3, $4, CASE WHEN $4 = 'added' THEN now() END)
         RETURNING id",
        s.id,
        g.id,
        body.track_id,
        status.as_str(),
    )
    .fetch_one(&st.db)
    .await?;
    jam::changed(&st, s, false, None);
    let request = jam::requests(&st.db, s.id, Some(g.id), JamRequestStatus::ALL)
        .await?
        .into_iter()
        .find(|r| r.id == id)
        .ok_or(AppError::NotFound)?;
    Ok((StatusCode::CREATED, Json(request)))
}

#[utoipa::path(
    post,
    path = "/api/jam/guest/react",
    tag = "jam",
    operation_id = "jam_react",
    security(("jam" = [])),
    request_body = JamReact,
    responses(
        (status = 204, description = "❤ ушло всем в сессии"),
        (status = 429, description = "Больше 30 реакций в минуту", body = ErrorBody),
    ),
)]
pub async fn react(
    State(st): State<AppState>,
    GuestAuth(g): GuestAuth,
    Json(body): Json<JamReact>,
) -> AppResult<StatusCode> {
    if !st.limiter.check_id("jam_react", g.id, 30, MINUTE) {
        return Err(too_many());
    }
    let s = &g.session;
    let track_id = jam::current_track(&st.db, s.host).await?;
    realtime::deliver(
        &st.hub,
        Audience::Jam {
            host: s.host,
            jam: s.id,
        },
        ServerEvent::JamReaction(JamReaction {
            jam_id: s.id,
            guest_name: g.name.clone(),
            kind: body.kind,
            track_id,
        }),
    );
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    post,
    path = "/api/jam/guest/skip",
    tag = "jam",
    operation_id = "jam_skip",
    security(("jam" = [])),
    responses(
        (status = 200, description = "Голос учтён; набралось больше половины гостей на связи — трек пропущен", body = JamSkip),
        (status = 409, description = "Ничего не играет (`nothing_playing`)", body = ErrorBody),
        (status = 429, description = "Слишком часто", body = ErrorBody),
    ),
)]
pub async fn skip(State(st): State<AppState>, GuestAuth(g): GuestAuth) -> AppResult<Json<JamSkip>> {
    if !st.limiter.check_id("jam_skip", g.id, 10, MINUTE) {
        return Err(too_many());
    }
    let s = &g.session;
    let track = jam::current_track(&st.db, s.host)
        .await?
        .ok_or_else(jam::nothing_playing)?;
    sqlx::query!(
        "INSERT INTO jam_skip_votes (jam_id, track_id, guest_id) VALUES ($1, $2, $3) ON CONFLICT DO NOTHING",
        s.id,
        track,
        g.id,
    )
    .execute(&st.db)
    .await?;
    let state = jam::skip_state(&st.db, s.id, Some(track), Some(g.id)).await?;
    if state.votes >= state.needed && jam::command_host(&st, s.host, RemoteCommand::Next).await? {
        sqlx::query!("DELETE FROM jam_skip_votes WHERE jam_id = $1", s.id)
            .execute(&st.db)
            .await?;
    }
    jam::changed(&st, s, false, None);
    Ok(Json(state))
}

#[utoipa::path(
    get,
    path = "/api/jam/guest/stream/{trackId}",
    tag = "jam",
    operation_id = "jam_stream",
    security(("jam" = [])),
    params(("trackId" = Uuid, Path, description = "Текущий или следующий трек хозяина")),
    responses(
        (status = 200, description = "Подписанная ссылка на аудио (45 минут) для «Слушать у себя»", body = JamStream),
        (status = 403, description = "Хозяин выключил «Слушать у себя» или трек не текущий и не следующий", body = ErrorBody),
        (status = 409, description = "Файла трека ещё нет на сервере (`audio_unavailable`)", body = ErrorBody),
        (status = 429, description = "Слишком часто", body = ErrorBody),
    ),
)]
pub async fn stream(
    State(st): State<AppState>,
    GuestAuth(g): GuestAuth,
    Path(track_id): Path<Uuid>,
) -> AppResult<Response> {
    let s = &g.session;
    if !s.settings.guests_can_listen {
        return Err(AppError::Forbidden(
            "хозяин выключил прослушивание у гостей".into(),
        ));
    }
    if !st.limiter.check_id("jam_stream", g.id, 30, MINUTE) {
        return Err(too_many());
    }
    // Гость слушает только то, что играет у хозяина, и следующий трек — каталог так не скачать
    if !jam::listenable(&st.db, s.host).await?.contains(&track_id) {
        return Err(AppError::Forbidden("этот трек сейчас не играет".into()));
    }
    let unavailable = || AppError::Conflict {
        code: "audio_unavailable",
        message: "Этот трек ещё не готов".into(),
    };
    let recording = sqlx::query_scalar!("SELECT recording_id FROM tracks WHERE id = $1", track_id)
        .fetch_optional(&st.db)
        .await?
        .flatten()
        .ok_or_else(unavailable)?;
    let file = acquire::playable_file(&st.db, recording)
        .await?
        .ok_or_else(unavailable)?;
    let url = st
        .storage
        .presign_get(&file.storage_key, GUEST_AUDIO_TTL)
        .await
        .map_err(|e| AppError::Unavailable(format!("хранилище не подписало ссылку: {e}")))?;
    let body = JamStream {
        track_id,
        url: url.to_string(),
        expires_at: Utc::now() + GUEST_AUDIO_TTL,
        mime_type: file.container.mime(file.codec),
        duration_ms: file.duration_ms,
        loudness_lufs: file.loudness_lufs,
        true_peak_db: file.true_peak_db,
    };
    Ok((
        StatusCode::OK,
        [(header::CACHE_CONTROL, "no-store")],
        Json(body),
    )
        .into_response())
}

#[utoipa::path(
    post,
    path = "/api/jam/guest/leave",
    tag = "jam",
    operation_id = "jam_leave",
    security(("jam" = [])),
    responses((status = 204, description = "Гость вышел: токен больше не действует")),
)]
pub async fn leave(State(st): State<AppState>, GuestAuth(g): GuestAuth) -> AppResult<StatusCode> {
    sqlx::query!(
        "UPDATE jam_guests SET removed_at = now() WHERE id = $1",
        g.id
    )
    .execute(&st.db)
    .await?;
    jam::changed(&st, &g.session, false, None);
    Ok(StatusCode::NO_CONTENT)
}

// ---------- WebSocket гостя ----------

#[derive(Debug, Default, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct GuestWsQuery {
    /// Токен гостя `dwj_…`.
    pub token: String,
}

#[utoipa::path(
    get,
    path = "/api/jam/guest/ws",
    tag = "jam",
    operation_id = "jam_guest_ws",
    params(GuestWsQuery),
    responses(
        (status = 101, description = "WebSocket гостя: `jam.now`, `jam.changed`, `jam.reaction`, `pong`"),
        (status = 401, description = "Неверный токен", body = ErrorBody),
        (status = 404, description = "Сессия закончилась (`jam_ended`)", body = ErrorBody),
    ),
)]
pub async fn guest_ws(
    State(st): State<AppState>,
    Query(q): Query<GuestWsQuery>,
    upgrade: Result<WebSocketUpgrade, WebSocketUpgradeRejection>,
) -> AppResult<Response> {
    let guest = jam::guest_by_token(&st.db, &q.token).await?;
    let upgrade = upgrade.map_err(|_| bad("нужно соединение WebSocket"))?;
    Ok(upgrade
        .max_message_size(16 * 1024)
        .on_upgrade(move |socket| guest_connection(st, guest, q.token, socket)))
}

fn text(event: &ServerEvent) -> Option<Message> {
    serde_json::to_string(event)
        .ok()
        .map(|json| Message::Text(json.into()))
}

fn close(code: u16, reason: &'static str) -> Message {
    Message::Close(Some(CloseFrame {
        code,
        reason: reason.into(),
    }))
}

/// Гостю — события его сессии и «что играет» из снимков активного устройства хозяина (только трек,
/// позиция и очередь: остальное состояние хозяина гость не видит).
async fn guest_connection(st: AppState, guest: Guest, token: String, socket: WebSocket) {
    let (mut sink, mut stream) = socket.split();
    let mut events = st.hub.subscribe();
    let mut stopping = st.hub.stopping();
    let s = guest.session.clone();
    let jam_id = s.id;
    let host = s.host;
    let now_event = |now| ServerEvent::JamNow(JamNowEvent { jam_id, now });
    let mut added = jam::added_by(&st.db, jam_id).await.unwrap_or_default();
    let mut live = jam::live_device(&st.db, host)
        .await
        .ok()
        .flatten()
        .is_some();
    let mut last: Option<dalywave_protocol::connect::PlaybackState> =
        crate::connect::playback(&st.db, host)
            .await
            .ok()
            .and_then(|(_, state)| state);
    if let Some(state) = &last
        && let Some(msg) = text(&now_event(jam::now_of(
            &state.clone().advanced(Utc::now()),
            &added,
            live,
        )))
        && sink.send(msg).await.is_err()
    {
        return;
    }
    let _ = jam::touch(&st.db, jam_id, Some(guest.id)).await;
    let mut ping = tokio::time::interval(HEARTBEAT);
    ping.tick().await;
    let mut recheck = tokio::time::interval(RECHECK);
    recheck.tick().await;
    let mut last_seen = Instant::now();
    loop {
        let outgoing: Option<Message> = tokio::select! {
            incoming = stream.next() => match incoming {
                Some(Ok(Message::Text(body))) => {
                    last_seen = Instant::now();
                    match serde_json::from_str::<ClientMessage>(body.as_str()) {
                        Ok(ClientMessage::Ping(p)) => text(&ServerEvent::Pong(Pong { id: p.id })),
                        _ => None,
                    }
                }
                Some(Ok(Message::Pong(_) | Message::Ping(_) | Message::Binary(_))) => {
                    last_seen = Instant::now();
                    None
                }
                Some(Ok(Message::Close(_))) | None | Some(Err(_)) => break,
            },
            event = events.recv() => match event {
                Ok(envelope) => {
                    let Envelope { to, event } = envelope.as_ref();
                    match (to, event) {
                        (Audience::Jam { jam, .. }, ServerEvent::JamChanged(c)) if *jam == jam_id => {
                            if c.ended || c.removed == Some(guest.id) {
                                let _ = sink.send(text(event).unwrap_or(Message::Text("{}".into()))).await;
                                let _ = sink.send(close(CLOSE_SESSION, "jam ended")).await;
                                break;
                            }
                            added = jam::added_by(&st.db, jam_id).await.unwrap_or(added);
                            text(event)
                        }
                        (Audience::Jam { jam, .. }, ServerEvent::JamReaction(_)) if *jam == jam_id => text(event),
                        (Audience::User(u), ServerEvent::ConnectState(c)) if *u == host => {
                            live = true;
                            last = Some((*c.state).clone());
                            text(&now_event(jam::now_of(&c.state, &added, live)))
                        }
                        (Audience::User(u), ServerEvent::ConnectActive(a)) if *u == host => match &a.state {
                            Some(state) => {
                                last = Some((**state).clone());
                                text(&now_event(jam::now_of(state, &added, live)))
                            }
                            None => None,
                        },
                        (Audience::User(u), ServerEvent::ConnectDevices(d)) if *u == host => {
                            let now_live = d.devices.iter().any(|x| x.active && x.online);
                            if now_live == live {
                                None
                            } else {
                                live = now_live;
                                last.as_ref().and_then(|state| {
                                    text(&now_event(jam::now_of(&state.clone().advanced(Utc::now()), &added, live)))
                                })
                            }
                        }
                        _ => None,
                    }
                }
                Err(RecvError::Lagged(_)) => None,
                Err(RecvError::Closed) => break,
            },
            _ = ping.tick() => {
                if last_seen.elapsed() > HEARTBEAT * 2 {
                    break;
                }
                Some(Message::Ping(Default::default()))
            }
            _ = recheck.tick() => {
                match jam::guest_by_token(&st.db, &token).await {
                    Ok(_) => {
                        let _ = jam::touch(&st.db, jam_id, Some(guest.id)).await;
                        None
                    }
                    Err(AppError::Db(e)) => {
                        tracing::warn!(error = %e, "jam guest recheck failed");
                        None
                    }
                    Err(_) => {
                        let _ = sink.send(close(CLOSE_SESSION, "jam ended")).await;
                        break;
                    }
                }
            }
            _ = stopping.changed() => {
                let _ = sink.send(close(CLOSE_RESTART, "server restart")).await;
                break;
            }
        };
        if let Some(msg) = outgoing
            && sink.send(msg).await.is_err()
        {
            break;
        }
    }
    let _ = sink.close().await;
}
