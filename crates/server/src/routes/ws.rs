//! WebSocket `/api/ws`: события в реальном времени (скачивания, задачи, медиатека) и управление
//! между устройствами (`connect.*`, [`crate::connect`]). Вход — cookie сайта, `Authorization: Bearer` или `?token=` (не все клиенты
//! WebSocket умеют ставить заголовки). С cookie проверяется Origin: браузер шлёт cookie и на рукопожатие,
//! начатое чужой страницей.

use std::time::{Duration, Instant};

use axum::{
    Router,
    extract::{
        State, WebSocketUpgrade,
        ws::{CloseFrame, Message, WebSocket, rejection::WebSocketUpgradeRejection},
    },
    http::{HeaderMap, header},
    response::Response,
    routing::get,
};
use chrono::Utc;
use dalywave_protocol::{
    ErrorBody,
    auth::{Role, SessionKind},
    realtime::{ClientMessage, Hello, Pong, Resync, ServerEvent},
};
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use tokio::sync::broadcast::error::RecvError;
use utoipa::IntoParams;

use crate::{
    auth::{self, Principal},
    connect::{self, Link},
    error::{AppError, AppResult, bad},
    extract::Query,
    realtime::{Audience, Envelope},
    security,
    state::AppState,
    users::parse_roles,
};

/// Как часто сервер шлёт ping-кадры.
pub const HEARTBEAT: Duration = Duration::from_secs(25);
/// Как часто перепроверять сессию: отозванная закрывает соединение.
const RECHECK: Duration = Duration::from_secs(60);
/// Код закрытия: сессия больше не действует.
const CLOSE_SESSION: u16 = 4001;
/// Код закрытия: сервер перезапускается (RFC 6455, 1012).
const CLOSE_RESTART: u16 = 1012;
/// Код закрытия: клиент не отвечает на ping.
const CLOSE_TIMEOUT: u16 = 4002;

pub fn routes() -> Router<AppState> {
    Router::new().route("/ws", get(ws))
}

#[derive(Debug, Default, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct WsQuery {
    /// Токен сессии (`dwd_…` или `dws_…`) для клиентов без заголовков.
    pub token: Option<String>,
}

#[utoipa::path(
    get,
    path = "/api/ws",
    tag = "realtime",
    security(("session" = []), ("device" = [])),
    params(WsQuery),
    responses(
        (status = 101, description = "WebSocket: сообщения `ServerEvent` от сервера, `ClientMessage` от клиента"),
        (status = 400, description = "Это не запрос WebSocket", body = ErrorBody),
        (status = 401, description = "Нет сессии", body = ErrorBody),
        (status = 403, description = "Нет ролей (`no_access`) или чужой Origin у cookie (`bad_origin`)", body = ErrorBody),
    ),
)]
pub async fn ws(
    State(st): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<WsQuery>,
    upgrade: Result<WebSocketUpgrade, WebSocketUpgradeRejection>,
) -> AppResult<Response> {
    let principal = match q.token.as_deref().filter(|t| !t.is_empty()) {
        Some(token) => auth::authenticate_token(&st, token).await?,
        None => auth::authenticate(&st, &headers).await?,
    }
    .ok_or(AppError::Unauthorized)?;
    if principal.roles.is_empty() {
        return Err(AppError::NoAccess);
    }
    let by_cookie = principal.kind == SessionKind::Web && q.token.is_none();
    if by_cookie {
        let origin = headers.get(header::ORIGIN).and_then(|o| o.to_str().ok());
        if !origin.is_some_and(|o| security::origin_allowed(&st.config, o)) {
            return Err(AppError::BadOrigin);
        }
    }
    let upgrade = upgrade.map_err(|_| bad("нужно соединение WebSocket"))?;
    // Снимок состояния с очередью (до пары сотен треков) — самое большое сообщение клиента
    Ok(upgrade
        .max_message_size(1024 * 1024)
        .on_upgrade(move |socket| connection(st, principal, socket)))
}

fn allowed(principal: &Principal, link: &Link, to: &Audience) -> bool {
    match to {
        Audience::User(id) => *id == principal.user.id,
        Audience::Admins => principal.roles.contains(&Role::Admin),
        Audience::All => true,
        Audience::Device { .. } => connect::addressed(link, principal.user.id, to),
    }
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

/// Сессия ещё действует? Заодно — свежие роли: снятый admin перестаёт получать события очереди.
async fn recheck(st: &AppState, principal: &mut Principal) -> bool {
    let row = sqlx::query!(
        "SELECT u.roles FROM sessions s JOIN users u ON u.id = s.user_id
         LEFT JOIN devices d ON d.id = s.device_id
         WHERE s.id = $1 AND s.revoked_at IS NULL AND s.expires_at > now()
           AND NOT u.disabled AND d.revoked_at IS NULL",
        principal.session_id,
    )
    .fetch_optional(&st.db)
    .await;
    match row {
        Ok(Some(r)) => {
            principal.roles = parse_roles(&r.roles);
            !principal.roles.is_empty()
        }
        Ok(None) => false,
        // База моргнула — соединение не рвём, проверим в следующий раз
        Err(e) => {
            tracing::warn!(error = %e, "websocket session recheck failed");
            true
        }
    }
}

async fn connection(st: AppState, mut principal: Principal, socket: WebSocket) {
    let (mut sink, mut stream) = socket.split();
    let mut events = st.hub.subscribe();
    let mut stopping = st.hub.stopping();
    let hello = ServerEvent::Hello(Hello {
        session_id: principal.session_id,
        user_id: principal.user.id,
        server_time: Utc::now(),
        heartbeat_ms: HEARTBEAT.as_millis() as u32,
    });
    if let Some(msg) = text(&hello)
        && sink.send(msg).await.is_err()
    {
        return;
    }
    tracing::debug!(user = %principal.user.id, "websocket connected");
    let mut ping = tokio::time::interval(HEARTBEAT);
    ping.tick().await;
    let mut session = tokio::time::interval(RECHECK);
    session.tick().await;
    let mut last_seen = Instant::now();
    let mut link = Link::default();
    let mut presence = tokio::time::interval(connect::CHECK);
    presence.tick().await;
    loop {
        let outgoing: Option<Message> = tokio::select! {
            incoming = stream.next() => match incoming {
                Some(Ok(Message::Text(body))) => {
                    last_seen = Instant::now();
                    // Незнакомые сообщения пропускаются: клиент может быть новее сервера
                    match serde_json::from_str::<ClientMessage>(body.as_str()) {
                        Ok(ClientMessage::Ping(p)) => text(&ServerEvent::Pong(Pong { id: p.id })),
                        Ok(msg) => connect::handle(&st, &principal, &mut link, msg)
                            .await
                            .and_then(|reply| text(&reply)),
                        Err(e) => {
                            if body.as_str().contains("\"connect.") {
                                tracing::debug!(error = %e, "unreadable connect message");
                            }
                            None
                        }
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
                    if allowed(&principal, &link, to) { text(event) } else { None }
                }
                Err(RecvError::Lagged(missed)) => {
                    tracing::warn!(missed, "websocket client lagged behind");
                    text(&ServerEvent::Resync(Resync { reason: "lagged".into() }))
                }
                Err(RecvError::Closed) => break,
            },
            _ = ping.tick() => {
                if last_seen.elapsed() > HEARTBEAT * 2 {
                    let _ = sink.send(close(CLOSE_TIMEOUT, "no pong")).await;
                    break;
                }
                Some(Message::Ping(Default::default()))
            }
            _ = presence.tick() => {
                connect::check(&st, principal.user.id, &mut link).await;
                None
            }
            _ = session.tick() => {
                if recheck(&st, &mut principal).await {
                    None
                } else {
                    let _ = sink.send(close(CLOSE_SESSION, "session ended")).await;
                    break;
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
    connect::release(&st, principal.user.id, &link).await;
    tracing::debug!(user = %principal.user.id, "websocket closed");
}
