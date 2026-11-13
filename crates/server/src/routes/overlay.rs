//! Оверлей «Сейчас играет» для стрима ([`crate::overlay`]). Владелец (`/api/overlay*`, право play):
//! ссылка, «Сменить ссылку», выключить, «прятать в приватной сессии». Открыто по секретной ссылке
//! (`?k=dwo_…`, без входа): `/api/overlay/now` — что играет сейчас, `/api/overlay/stream` — то же
//! потоком SSE (событие `now`; `gone` — ссылку сменили или выключили).

use std::{convert::Infallible, sync::Arc, time::Duration};

use axum::{
    Router,
    extract::State,
    http::{HeaderValue, StatusCode, header},
    response::{
        IntoResponse, Response,
        sse::{Event, KeepAlive, Sse},
    },
    routing::{get, post},
};
use chrono::Utc;
use dalywave_protocol::{
    ErrorBody,
    connect::PlaybackState,
    overlay::{OverlayInfo, OverlayLink, OverlayNow, OverlayPatch},
    realtime::ServerEvent,
};
use futures_util::stream;
use serde::Deserialize;
use tokio::sync::broadcast::{self, error::RecvError};
use utoipa::IntoParams;
use uuid::Uuid;

use crate::{
    auth::Principal,
    connect,
    error::{AppError, AppResult},
    extract::{Json, Query},
    overlay,
    realtime::{Audience, Envelope},
    state::AppState,
};

/// Как часто поток перепроверяет ссылку и приватную сессию и пересчитывает плашку (пропавшее устройство).
const RECHECK: Duration = Duration::from_secs(15);

/// Владелец: под правом play.
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/overlay", get(info).patch(update).delete(disable))
        .route("/overlay/link", post(rotate))
}

/// По секретной ссылке, без входа.
pub fn public_routes() -> Router<AppState> {
    Router::new()
        .route("/overlay/now", get(now))
        .route("/overlay/stream", get(stream_now))
}

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct OverlayQuery {
    /// Токен из ссылки (`dwo_…`).
    pub k: String,
}

#[utoipa::path(
    get,
    path = "/api/overlay",
    tag = "playback",
    security(("session" = []), ("device" = [])),
    responses((status = 200, description = "Ссылка на оверлей; `link: null` — ещё не создана", body = OverlayInfo)),
)]
pub async fn info(
    State(st): State<AppState>,
    principal: Principal,
) -> AppResult<Json<OverlayInfo>> {
    Ok(Json(OverlayInfo {
        link: overlay::link(&st, principal.user.id).await?,
    }))
}

#[utoipa::path(
    post,
    path = "/api/overlay/link",
    tag = "playback",
    security(("session" = []), ("device" = [])),
    responses((status = 200, description = "Новая ссылка; прежняя перестала работать", body = OverlayLink)),
)]
pub async fn rotate(
    State(st): State<AppState>,
    principal: Principal,
) -> AppResult<Json<OverlayLink>> {
    Ok(Json(overlay::rotate(&st, principal.user.id).await?))
}

#[utoipa::path(
    patch,
    path = "/api/overlay",
    tag = "playback",
    security(("session" = []), ("device" = [])),
    request_body = OverlayPatch,
    responses(
        (status = 200, description = "Ссылка после правки", body = OverlayInfo),
        (status = 404, description = "Ссылки ещё нет", body = ErrorBody),
    ),
)]
pub async fn update(
    State(st): State<AppState>,
    principal: Principal,
    Json(patch): Json<OverlayPatch>,
) -> AppResult<Json<OverlayInfo>> {
    let user = principal.user.id;
    if let Some(hide) = patch.hide_private
        && !overlay::set_hide_private(&st, user, hide).await?
    {
        return Err(AppError::NotFound);
    }
    Ok(Json(OverlayInfo {
        link: overlay::link(&st, user).await?,
    }))
}

#[utoipa::path(
    delete,
    path = "/api/overlay",
    tag = "playback",
    security(("session" = []), ("device" = [])),
    responses((status = 204, description = "Оверлей выключен: ссылка больше не работает")),
)]
pub async fn disable(State(st): State<AppState>, principal: Principal) -> AppResult<StatusCode> {
    overlay::disable(&st, principal.user.id).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn owner(st: &AppState, k: &str) -> AppResult<(Uuid, bool)> {
    overlay::owner(&st.db, k).await?.ok_or(AppError::NotFound)
}

async fn current(st: &AppState, user: Uuid, hide_private: bool) -> AppResult<OverlayNow> {
    let (_, state) = connect::playback(&st.db, user).await?;
    let private = overlay::private(&st.db, user, hide_private).await?;
    Ok(overlay::view(state.as_ref(), private, Utc::now()))
}

#[utoipa::path(
    get,
    path = "/api/overlay/now",
    tag = "playback",
    params(OverlayQuery),
    responses(
        (status = 200, description = "Что сейчас играет у владельца ссылки", body = OverlayNow),
        (status = 404, description = "Ссылка неверна, сменена или выключена", body = ErrorBody),
    ),
)]
pub async fn now(State(st): State<AppState>, Query(q): Query<OverlayQuery>) -> AppResult<Response> {
    let (user, hide) = owner(&st, &q.k).await?;
    let mut res = Json(current(&st, user, hide).await?).into_response();
    res.headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    Ok(res)
}

/// Состояние потока одной плашки.
struct Feed {
    st: AppState,
    token: String,
    user: Uuid,
    private: bool,
    rx: broadcast::Receiver<Arc<Envelope>>,
    tick: tokio::time::Interval,
    /// Последний снимок активного устройства
    state: Option<PlaybackState>,
    /// Что плашка уже показывает
    shown: Option<OverlayNow>,
    done: bool,
}

enum Step {
    Show,
    Gone,
    Skip,
}

impl Feed {
    fn event(now: &OverlayNow) -> Event {
        Event::default()
            .event("now")
            .json_data(now)
            .unwrap_or_else(|_| Event::default().comment("unserializable"))
    }

    /// Снимок пересчитан: показать, если плашке есть что менять.
    fn next_view(&mut self) -> Option<Event> {
        let next = overlay::view(self.state.as_ref(), self.private, Utc::now());
        if self
            .shown
            .as_ref()
            .is_some_and(|prev| !overlay::changed(prev, &next))
        {
            return None;
        }
        let event = Self::event(&next);
        self.shown = Some(next);
        Some(event)
    }

    async fn wait(&mut self) -> Step {
        tokio::select! {
            msg = self.rx.recv() => match msg {
                Ok(env) => {
                    if env.to != Audience::User(self.user) {
                        return Step::Skip;
                    }
                    match &env.event {
                        ServerEvent::ConnectState(s) => self.state = Some((*s.state).clone()),
                        ServerEvent::ConnectActive(a) => match &a.state {
                            Some(s) => self.state = Some((**s).clone()),
                            // Активным стало другое устройство без снимка — свежий снимок из базы
                            None => self.reload().await,
                        },
                        _ => return Step::Skip,
                    }
                    Step::Show
                }
                Err(RecvError::Lagged(_)) => {
                    self.reload().await;
                    Step::Show
                }
                Err(RecvError::Closed) => Step::Gone,
            },
            _ = self.tick.tick() => {
                match overlay::owner(&self.st.db, &self.token).await {
                    Ok(Some((user, hide))) if user == self.user => {
                        if let Ok(p) = overlay::private(&self.st.db, user, hide).await {
                            self.private = p;
                        }
                        Step::Show
                    }
                    Ok(_) => Step::Gone,
                    // База моргнула — поток не рвём
                    Err(_) => Step::Skip,
                }
            }
        }
    }

    async fn reload(&mut self) {
        if let Ok((_, state)) = connect::playback(&self.st.db, self.user).await {
            self.state = state;
        }
    }
}

#[utoipa::path(
    get,
    path = "/api/overlay/stream",
    tag = "playback",
    params(OverlayQuery),
    responses(
        (status = 200, description = "SSE: событие `now` (OverlayNow) сразу и при каждой перемене; `gone` — ссылка больше не работает", content_type = "text/event-stream"),
        (status = 404, description = "Ссылка неверна, сменена или выключена", body = ErrorBody),
    ),
)]
pub async fn stream_now(
    State(st): State<AppState>,
    Query(q): Query<OverlayQuery>,
) -> AppResult<Response> {
    let (user, hide_private) = owner(&st, &q.k).await?;
    // Подписка — до чтения снимка: перемена между ними не потеряется
    let rx = st.hub.subscribe();
    let (_, state) = connect::playback(&st.db, user).await?;
    let private = overlay::private(&st.db, user, hide_private).await?;
    let mut tick = tokio::time::interval(RECHECK);
    tick.reset();
    let feed = Feed {
        st,
        token: q.k,
        user,
        private,
        rx,
        tick,
        state,
        shown: None,
        done: false,
    };
    let events = stream::unfold(feed, |mut feed| async move {
        if feed.done {
            return None;
        }
        if feed.shown.is_none() {
            let event = feed.next_view()?;
            return Some((Ok::<_, Infallible>(event), feed));
        }
        loop {
            match feed.wait().await {
                Step::Skip => continue,
                Step::Gone => {
                    feed.done = true;
                    return Some((Ok(Event::default().event("gone").data("{}")), feed));
                }
                Step::Show => {
                    if let Some(event) = feed.next_view() {
                        return Some((Ok(event), feed));
                    }
                }
            }
        }
    });
    let mut res = Sse::new(events)
        .keep_alive(KeepAlive::new().interval(Duration::from_secs(15)))
        .into_response();
    // Прокси (Traefik, Cloudflare) не копят поток
    res.headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    res.headers_mut()
        .insert("x-accel-buffering", HeaderValue::from_static("no"));
    Ok(res)
}
