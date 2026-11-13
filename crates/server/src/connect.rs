//! Управление между устройствами (этап 3). Устройство — соединение WebSocket, представившееся
//! `connect.hello`; список — таблица `connect_devices`, последнее состояние и активное устройство —
//! `playback_state`. Команды, подтверждения и снимки пересылаются внутри процесса API
//! ([`realtime::deliver`]): все соединения здесь, а снимок с очередью не влезает в NOTIFY.
//!
//! Сервер не исполняет команды и не держит очередь: её владелец — активное устройство. Сервер помнит
//! последний снимок (новое устройство сразу видит, что играет), принимает снимки только от активного
//! (опоздавший снимок прежнего после переноса отбрасывается) и при переносе отдаёт новому активному
//! снимок с досчитанной позицией.

use std::time::{Duration, Instant};

use chrono::Utc;
use dalywave_protocol::{
    auth::{Permission, Platform},
    connect::{
        ConnectAck, ConnectActive, ConnectCapabilities, ConnectCommand, ConnectDevice,
        ConnectDevices, ConnectHello, ConnectKind, ConnectOverview, ConnectSend, ConnectState,
        ConnectTransfer, MAX_DEVICE_ID, ONLINE_WINDOW_SECS, PlaybackState,
    },
    realtime::{ClientMessage, ServerEvent},
};
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    auth::Principal,
    error::AppResult,
    realtime::{self, Audience},
    state::AppState,
};

/// Без сообщений от устройства дольше этого — оно пропало (пульты предложат «Продолжить здесь»).
pub const STALE: Duration = Duration::from_secs(ONLINE_WINDOW_SECS as u64);
/// Как часто соединение проверяет, не пропало ли его устройство.
pub const CHECK: Duration = Duration::from_secs(5);
/// Самое длинное имя устройства.
const MAX_NAME: usize = 80;

/// Устройство, которым представилось соединение.
#[derive(Debug)]
pub struct Link {
    pub device: Option<String>,
    /// Id соединения: строка устройства принадлежит последнему, кто представился этим id.
    pub connection: Uuid,
    pub last_seen: Instant,
    /// Устройство уже объявлено пропавшим.
    pub stale: bool,
    /// Только пульт (часы): активным не становится, снимков не публикует.
    pub remote: bool,
}

impl Default for Link {
    fn default() -> Self {
        Self {
            device: None,
            connection: Uuid::now_v7(),
            last_seen: Instant::now(),
            stale: false,
            remote: false,
        }
    }
}

/// Сообщение для этого соединения? События `Device` — только соединению с этим id устройства.
pub fn addressed(link: &Link, user: Uuid, to: &Audience) -> bool {
    match to {
        Audience::Device { user: u, device } => {
            *u == user && link.device.as_deref() == Some(device.as_str())
        }
        _ => false,
    }
}

/// Обрабатывает сообщение `connect.*`. Ответ этому же соединению — в результате; остальным — через хаб.
/// Ошибки базы не рвут соединение: команда просто не выполнится, а пульт не дождётся подтверждения.
pub async fn handle(
    st: &AppState,
    principal: &Principal,
    link: &mut Link,
    msg: ClientMessage,
) -> Option<ServerEvent> {
    if !principal.can(Permission::Play) {
        return None;
    }
    link.last_seen = Instant::now();
    let user = principal.user.id;
    let result = match msg {
        ClientMessage::Ping(_) => return None,
        ClientMessage::ConnectHello(hello) => hello_device(st, principal, link, hello).await,
        // Без hello остальное бессмысленно: устройство неизвестно
        _ if link.device.is_none() => return None,
        ClientMessage::ConnectHeartbeat(_) => heartbeat(st, user, link).await.map(|_| None),
        ClientMessage::ConnectState(p) => publish(st, user, link, *p.state).await,
        ClientMessage::ConnectCommand(send) => command(st, user, link, send).await,
        ClientMessage::ConnectAck(ack) => {
            if let (Some(to), Some(from)) = (ack.to.clone(), link.device.clone()) {
                realtime::deliver(
                    &st.hub,
                    Audience::Device { user, device: to },
                    ServerEvent::ConnectAck(ConnectAck {
                        to: None,
                        from: Some(from),
                        ..ack
                    }),
                );
            }
            Ok(None)
        }
        ClientMessage::ConnectClaim(_) => claim(st, user, link).await.map(|_| None),
        ClientMessage::ConnectTransfer(t) => transfer(st, user, t).await,
        ClientMessage::ConnectSignal(s) => crate::handoff::signal(st, user, link, s).await,
    };
    result.unwrap_or_else(|e| {
        tracing::warn!(error = %e, "connect message failed");
        None
    })
}

fn kind_of(name: &str) -> ConnectKind {
    name.parse().unwrap_or(ConnectKind::Web)
}

fn platform_of(name: &str) -> Platform {
    name.parse().unwrap_or(Platform::Web)
}

fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= MAX_DEVICE_ID && !id.chars().any(char::is_control)
}

async fn hello_device(
    st: &AppState,
    principal: &Principal,
    link: &mut Link,
    hello: ConnectHello,
) -> AppResult<Option<ServerEvent>> {
    if !valid_id(&hello.device_id) {
        return Ok(None);
    }
    let user = principal.user.id;
    // То же соединение представилось другим id (вкладка сменила id после совпадения с соседней)
    if link.device.as_deref().is_some_and(|d| d != hello.device_id) {
        release(st, user, link).await;
    }
    upsert(st, principal, &hello, link.connection).await?;
    link.device = Some(hello.device_id);
    link.stale = false;
    link.remote = hello.capabilities.remote;
    announce(&st.db, &st.hub, user).await?;
    // Новому устройству — кто играет и что: пульт сразу показывает трек и позицию
    let (active, state) = playback(&st.db, user).await?;
    Ok(Some(ServerEvent::ConnectActive(ConnectActive {
        device_id: active,
        from: None,
        transfer: false,
        state: state.map(|s| Box::new(s.advanced(Utc::now()))),
    })))
}

/// Строка устройства в списке: представилось соединение `connection` (WebSocket или пульт по HTTP).
async fn upsert(
    st: &AppState,
    principal: &Principal,
    hello: &ConnectHello,
    connection: Uuid,
) -> AppResult<()> {
    let user = principal.user.id;
    // У приложения имя — из входа: так оно и в админке, и в списке одно
    let app_name = match principal.device_id {
        Some(id) => {
            sqlx::query_scalar!("SELECT name FROM devices WHERE id = $1", id)
                .fetch_optional(&st.db)
                .await?
        }
        None => None,
    };
    let name: String = app_name
        .unwrap_or_else(|| hello.name.clone())
        .trim()
        .chars()
        .filter(|c| !c.is_control())
        .take(MAX_NAME)
        .collect();
    let name = if name.is_empty() {
        crate::devices::default_name(hello.platform).to_string()
    } else {
        name
    };
    let capabilities = serde_json::to_value(hello.capabilities).unwrap_or_default();
    let version = hello
        .app_version
        .as_ref()
        .map(|v| v.chars().take(40).collect::<String>());
    sqlx::query!(
        "INSERT INTO connect_devices (user_id, id, device_id, name, kind, platform, app_version, capabilities,
                                      connection, last_seen_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, now())
         ON CONFLICT (user_id, id) DO UPDATE SET
             device_id = EXCLUDED.device_id, name = EXCLUDED.name, kind = EXCLUDED.kind,
             platform = EXCLUDED.platform, app_version = EXCLUDED.app_version,
             capabilities = EXCLUDED.capabilities, connection = EXCLUDED.connection, last_seen_at = now()",
        user,
        hello.device_id,
        principal.device_id,
        name,
        hello.kind.as_str(),
        hello.platform.as_str(),
        version,
        capabilities,
        connection,
    )
    .execute(&st.db)
    .await?;
    // Закрытые вкладки не копятся: через сутки их уже нет и в «недавних»
    sqlx::query!(
        "DELETE FROM connect_devices
         WHERE user_id = $1 AND kind = 'web' AND connection IS NULL AND last_seen_at < now() - interval '1 day'",
        user,
    )
    .execute(&st.db)
    .await?;
    Ok(())
}

async fn heartbeat(st: &AppState, user: Uuid, link: &mut Link) -> AppResult<()> {
    let Some(device) = link.device.as_deref() else {
        return Ok(());
    };
    sqlx::query!(
        "UPDATE connect_devices SET last_seen_at = now() WHERE user_id = $1 AND id = $2 AND connection = $3",
        user,
        device,
        link.connection,
    )
    .execute(&st.db)
    .await?;
    if link.stale {
        link.stale = false;
        announce(&st.db, &st.hub, user).await?;
    }
    Ok(())
}

/// Раз в [`CHECK`]: устройство молчит дольше [`STALE`] — объявить его пропавшим (соединение не рвётся:
/// вкладка в фоне может просто притормозить таймеры, а вернувшись — снова напомнить о себе).
pub async fn check(st: &AppState, user: Uuid, link: &mut Link) {
    let Some(device) = link.device.as_deref() else {
        return;
    };
    if link.stale || link.last_seen.elapsed() <= STALE {
        return;
    }
    link.stale = true;
    // Последний знак жизни мог записаться чуть позже, чем его заметило соединение: окно «в сети» в
    // запросе списка — по базе, поэтому отметка сдвигается на его границу
    let res = sqlx::query!(
        "UPDATE connect_devices SET last_seen_at = least(last_seen_at, now() - make_interval(secs => $4))
         WHERE user_id = $1 AND id = $2 AND connection = $3",
        user,
        device,
        link.connection,
        ONLINE_WINDOW_SECS as f64,
    )
    .execute(&st.db)
    .await;
    let res = match res {
        Ok(_) => announce(&st.db, &st.hub, user).await,
        Err(e) => Err(e.into()),
    };
    if let Err(e) = res {
        tracing::warn!(error = %e, "connect stale check failed");
    }
}

/// Соединение закрылось: устройство не в сети (если строку не забрало более новое соединение).
pub async fn release(st: &AppState, user: Uuid, link: &Link) {
    let Some(device) = link.device.as_deref() else {
        return;
    };
    let res = sqlx::query!(
        "UPDATE connect_devices
         SET connection = NULL, last_seen_at = CASE WHEN $4 THEN last_seen_at ELSE now() END
         WHERE user_id = $1 AND id = $2 AND connection = $3",
        user,
        device,
        link.connection,
        link.stale,
    )
    .execute(&st.db)
    .await;
    let res = match res {
        Ok(r) if r.rows_affected() > 0 => {
            friends_see_stop(st, user, device).await;
            announce(&st.db, &st.hub, user).await
        }
        Ok(_) => Ok(()),
        Err(e) => Err(e.into()),
    };
    if let Err(e) = res {
        tracing::warn!(error = %e, "connect release failed");
    }
}

/// Пропало активное устройство — у друзей «сейчас играет» сменится на «последнее».
async fn friends_see_stop(st: &AppState, user: Uuid, device: &str) {
    let active = sqlx::query_scalar!(
        "SELECT active_connect_id FROM playback_state WHERE user_id = $1",
        user
    )
    .fetch_optional(&st.db)
    .await
    .ok()
    .flatten()
    .flatten();
    if active.as_deref() == Some(device) {
        let st = st.clone();
        tokio::spawn(async move { crate::social::announce_activity(&st, user).await });
    }
}

/// Тексты текущего и ближайших треков — заранее, в фоне: снимок не ждёт базы ради этого.
fn prefetch_lyrics(st: &AppState, state: &PlaybackState) {
    let ids: Vec<Uuid> = state
        .current
        .iter()
        .chain(&state.manual)
        .chain(&state.upcoming)
        .map(|q| q.track.id)
        .take(crate::lyrics::PREFETCH)
        .collect();
    if ids.is_empty() {
        return;
    }
    let db = st.db.clone();
    tokio::spawn(async move {
        if let Err(e) = crate::lyrics::prefetch(&db, &ids).await {
            tracing::warn!(error = %e, "lyrics prefetch failed");
        }
    });
}

async fn publish(
    st: &AppState,
    user: Uuid,
    link: &Link,
    state: PlaybackState,
) -> AppResult<Option<ServerEvent>> {
    let Some(device) = link.device.clone().filter(|_| !link.remote) else {
        return Ok(None);
    };
    let state = PlaybackState {
        at: Some(Utc::now()),
        ..state
    };
    let json = serde_json::to_value(&state).unwrap_or_default();
    prefetch_lyrics(st, &state);
    // Что играло до этого снимка: друзьям сообщается только смена трека или паузы
    let before = sqlx::query!(
        r#"SELECT state -> 'current' -> 'track' ->> 'id' AS "track?", (state ->> 'playing')::boolean AS "playing?"
           FROM playback_state WHERE user_id = $1"#,
        user
    )
    .fetch_optional(&st.db)
    .await?;
    // Снимок принимается от активного устройства; если активного нет — отправитель им становится
    let row = sqlx::query!(
        r#"WITH prev AS (SELECT active_connect_id FROM playback_state WHERE user_id = $1 FOR UPDATE)
           INSERT INTO playback_state (user_id, active_connect_id, state, updated_at)
           VALUES ($1, $2, $3, now())
           ON CONFLICT (user_id) DO UPDATE SET state = EXCLUDED.state, updated_at = now(),
               active_connect_id = EXCLUDED.active_connect_id
           WHERE playback_state.active_connect_id IS NULL OR playback_state.active_connect_id = $2
           RETURNING (SELECT active_connect_id FROM prev) AS "prev""#,
        user,
        device,
        json,
    )
    .fetch_optional(&st.db)
    .await?;
    let Some(row) = row else {
        // Активно другое устройство (снимок опоздал после переноса): пусть отправитель остановится
        let (active, _) = playback(&st.db, user).await?;
        return Ok(Some(ServerEvent::ConnectActive(ConnectActive {
            device_id: active,
            from: None,
            transfer: false,
            state: None,
        })));
    };
    let track = state.current.as_ref().map(|c| c.track.id.to_string());
    let was = before.map(|b| (b.track, b.playing.unwrap_or(false)));
    if was != Some((track, state.playing)) {
        let st = st.clone();
        tokio::spawn(async move { crate::social::announce_activity(&st, user).await });
    }
    if row.prev.as_deref() != Some(device.as_str()) {
        realtime::deliver(
            &st.hub,
            Audience::User(user),
            ServerEvent::ConnectActive(ConnectActive {
                device_id: Some(device.clone()),
                from: row.prev,
                transfer: false,
                state: None,
            }),
        );
        announce(&st.db, &st.hub, user).await?;
    }
    realtime::deliver(
        &st.hub,
        Audience::User(user),
        ServerEvent::ConnectState(ConnectState {
            device_id: device,
            state: Box::new(state),
        }),
    );
    Ok(None)
}

async fn command(
    st: &AppState,
    user: Uuid,
    link: &Link,
    send: ConnectSend,
) -> AppResult<Option<ServerEvent>> {
    let Some(from) = link.device.clone() else {
        return Ok(None);
    };
    if !is_online(&st.db, user, &send.to).await? {
        return Ok(Some(ServerEvent::ConnectAck(ConnectAck {
            id: send.id,
            to: None,
            from: Some(send.to),
            ok: false,
            error: Some("offline".into()),
        })));
    }
    realtime::deliver(
        &st.hub,
        Audience::Device {
            user,
            device: send.to,
        },
        ServerEvent::ConnectCommand(ConnectCommand {
            id: send.id,
            from,
            command: send.command,
        }),
    );
    Ok(None)
}

async fn claim(st: &AppState, user: Uuid, link: &Link) -> AppResult<()> {
    let Some(device) = link.device.clone().filter(|_| !link.remote) else {
        return Ok(());
    };
    let prev = set_active(&st.db, user, &device).await?;
    if prev.as_deref() != Some(device.as_str()) {
        realtime::deliver(
            &st.hub,
            Audience::User(user),
            ServerEvent::ConnectActive(ConnectActive {
                device_id: Some(device),
                from: prev,
                transfer: false,
                state: None,
            }),
        );
        announce(&st.db, &st.hub, user).await?;
    }
    Ok(())
}

/// Делает устройство активным и возвращает прежнее активное.
async fn set_active(db: &PgPool, user: Uuid, device: &str) -> AppResult<Option<String>> {
    let row = sqlx::query!(
        r#"WITH prev AS (SELECT active_connect_id FROM playback_state WHERE user_id = $1 FOR UPDATE)
           INSERT INTO playback_state (user_id, active_connect_id, state, updated_at)
           VALUES ($1, $2, '{}', now())
           ON CONFLICT (user_id) DO UPDATE SET active_connect_id = EXCLUDED.active_connect_id
           RETURNING (SELECT active_connect_id FROM prev) AS "prev""#,
        user,
        device,
    )
    .fetch_one(db)
    .await?;
    Ok(row.prev)
}

pub async fn transfer(
    st: &AppState,
    user: Uuid,
    t: ConnectTransfer,
) -> AppResult<Option<ServerEvent>> {
    let ack = |ok: bool, error: Option<&str>| {
        t.id.clone().map(|id| {
            ServerEvent::ConnectAck(ConnectAck {
                id,
                to: None,
                from: Some(t.to.clone()),
                ok,
                error: error.map(str::to_string),
            })
        })
    };
    if !is_online(&st.db, user, &t.to).await? {
        return Ok(ack(false, Some("offline")));
    }
    // Звук на часах не играет: перенос туда оставил бы человека в тишине
    if is_remote(&st.db, user, &t.to).await? {
        return Ok(ack(false, Some("unsupported")));
    }
    let mut tx = st.db.begin().await?;
    let row = sqlx::query!(
        "SELECT active_connect_id, state FROM playback_state WHERE user_id = $1 FOR UPDATE",
        user,
    )
    .fetch_optional(&mut *tx)
    .await?;
    let (prev, state) = match row {
        Some(r) => (r.active_connect_id, parse_state(r.state)),
        None => (None, None),
    };
    // Новое активное продолжает с позиции «на сейчас»: прежнее ещё играло, пока шла команда
    let state = state.map(|s| {
        let mut s = s.advanced(Utc::now());
        if let Some(play) = t.play {
            s.playing = play && s.current.is_some();
        }
        s
    });
    let json = match &state {
        Some(s) => serde_json::to_value(s).unwrap_or_default(),
        None => serde_json::json!({}),
    };
    sqlx::query!(
        "INSERT INTO playback_state (user_id, active_connect_id, state, updated_at) VALUES ($1, $2, $3, now())
         ON CONFLICT (user_id) DO UPDATE SET active_connect_id = EXCLUDED.active_connect_id,
             state = EXCLUDED.state, updated_at = now()",
        user,
        t.to,
        json,
    )
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    realtime::deliver(
        &st.hub,
        Audience::User(user),
        ServerEvent::ConnectActive(ConnectActive {
            device_id: Some(t.to.clone()),
            from: prev,
            transfer: true,
            state: state.map(Box::new),
        }),
    );
    announce(&st.db, &st.hub, user).await?;
    Ok(ack(true, None))
}

async fn is_online(db: &PgPool, user: Uuid, device: &str) -> AppResult<bool> {
    let online = sqlx::query_scalar!(
        r#"SELECT (connection IS NOT NULL AND last_seen_at > now() - make_interval(secs => $3)) AS "online!"
           FROM connect_devices WHERE user_id = $1 AND id = $2"#,
        user,
        device,
        ONLINE_WINDOW_SECS as f64,
    )
    .fetch_optional(db)
    .await?;
    Ok(online.unwrap_or(false))
}

async fn is_remote(db: &PgPool, user: Uuid, device: &str) -> AppResult<bool> {
    let remote = sqlx::query_scalar!(
        r#"SELECT coalesce((capabilities->>'remote')::boolean, false) AS "remote!"
           FROM connect_devices WHERE user_id = $1 AND id = $2"#,
        user,
        device,
    )
    .fetch_optional(db)
    .await?;
    Ok(remote.unwrap_or(false))
}

fn parse_state(json: serde_json::Value) -> Option<PlaybackState> {
    if json.as_object().is_none_or(|o| o.is_empty()) {
        return None;
    }
    serde_json::from_value(json)
        .inspect_err(|e| tracing::warn!(error = %e, "stored playback state is unreadable"))
        .ok()
}

/// Активное устройство и последний снимок.
pub async fn playback(
    db: &PgPool,
    user: Uuid,
) -> AppResult<(Option<String>, Option<PlaybackState>)> {
    let row = sqlx::query!(
        "SELECT active_connect_id, state FROM playback_state WHERE user_id = $1",
        user,
    )
    .fetch_optional(db)
    .await?;
    Ok(match row {
        Some(r) => (r.active_connect_id, parse_state(r.state)),
        None => (None, None),
    })
}

/// Список устройств: активное первым, затем в сети, затем недавние. Закрытая вкладка не вернётся
/// (новая вкладка — новое устройство), поэтому её нет в списке; приложения не в сети видны месяц.
pub async fn devices(db: &PgPool, user: Uuid) -> AppResult<ConnectDevices> {
    let active = sqlx::query_scalar!(
        "SELECT active_connect_id FROM playback_state WHERE user_id = $1",
        user,
    )
    .fetch_optional(db)
    .await?
    .flatten();
    let rows = sqlx::query!(
        r#"SELECT id, name, kind, platform, app_version, capabilities, last_seen_at,
                  (connection IS NOT NULL AND last_seen_at > now() - make_interval(secs => $3)) AS "online!"
           FROM connect_devices
           WHERE user_id = $1
             AND (id = $2
                  OR (connection IS NOT NULL AND last_seen_at > now() - make_interval(secs => $3))
                  OR (kind <> 'web' AND last_seen_at > now() - interval '30 days'))
           ORDER BY (id = $2) DESC NULLS LAST,
                    (connection IS NOT NULL AND last_seen_at > now() - make_interval(secs => $3)) DESC,
                    last_seen_at DESC
           LIMIT 30"#,
        user,
        active.as_deref().unwrap_or(""),
        ONLINE_WINDOW_SECS as f64,
    )
    .fetch_all(db)
    .await?;
    let devices = rows
        .into_iter()
        .map(|r| ConnectDevice {
            active: active.as_deref() == Some(r.id.as_str()),
            capabilities: serde_json::from_value::<ConnectCapabilities>(r.capabilities)
                .unwrap_or_default(),
            kind: kind_of(&r.kind),
            platform: platform_of(&r.platform),
            id: r.id,
            name: r.name,
            app_version: r.app_version,
            online: r.online,
            last_seen_at: r.last_seen_at,
        })
        .collect();
    Ok(ConnectDevices {
        devices,
        active_device_id: active,
    })
}

/// Свежий список — всем устройствам человека.
async fn announce(db: &PgPool, hub: &realtime::Hub, user: Uuid) -> AppResult<()> {
    let list = devices(db, user).await?;
    realtime::deliver(hub, Audience::User(user), ServerEvent::ConnectDevices(list));
    Ok(())
}

/// `GET /api/devices`.
pub async fn overview(db: &PgPool, user: Uuid) -> AppResult<ConnectOverview> {
    let list = devices(db, user).await?;
    let (_, state) = playback(db, user).await?;
    Ok(ConnectOverview {
        devices: list.devices,
        active_device_id: list.active_device_id,
        state: state.map(|s| s.advanced(Utc::now())),
    })
}

// ---------- Пульт по HTTP ----------
//
// На часах WebSocket доступен только при своём звуке (watchOS ограничивает «низкоуровневую» сеть),
// поэтому часы — пульт по HTTP: раз в пару секунд отмечаются и забирают состояние, команды шлют
// запросом и ждут подтверждения устройства. Для остальных устройств это то же «Где играть».

/// Сколько ждать подтверждения команды от устройства.
pub const REMOTE_ACK_WAIT: Duration = Duration::from_secs(4);

/// Пульт без WebSocket отметился: устройство в сети ещё [`ONLINE_WINDOW_SECS`] секунд. Соединение —
/// id сессии: строка остаётся за этим пультом, пока он отмечается. В ответ — список и снимок без
/// очереди (пульту на запястье она не нужна, а снимок с очередью большой).
pub async fn remote_presence(
    st: &AppState,
    principal: &Principal,
    hello: ConnectHello,
) -> AppResult<ConnectOverview> {
    let user = principal.user.id;
    if valid_id(&hello.device_id) {
        let was_online = is_online(&st.db, user, &hello.device_id).await?;
        upsert(st, principal, &hello, principal.session_id).await?;
        if !was_online {
            announce(&st.db, &st.hub, user).await?;
        }
    }
    let mut overview = overview(&st.db, user).await?;
    if let Some(s) = overview.state.as_mut() {
        s.manual.clear();
        s.upcoming.clear();
        s.history.clear();
        s.peaks.clear();
    }
    Ok(overview)
}

/// Id устройства «Где играть», которым отмечалась эта сессия (пульт по HTTP).
async fn remote_id(st: &AppState, principal: &Principal) -> AppResult<String> {
    let id = sqlx::query_scalar!(
        "SELECT id FROM connect_devices WHERE user_id = $1 AND connection = $2
         ORDER BY last_seen_at DESC LIMIT 1",
        principal.user.id,
        principal.session_id,
    )
    .fetch_optional(&st.db)
    .await?;
    Ok(id.unwrap_or_else(|| "remote".into()))
}

/// Команда с пульта по HTTP: доставить устройству и дождаться его подтверждения.
pub async fn remote_command(
    st: &AppState,
    principal: &Principal,
    send: ConnectSend,
) -> AppResult<ConnectAck> {
    let user = principal.user.id;
    let from = remote_id(st, principal).await?;
    let answer = |ok: bool, error: Option<&str>| ConnectAck {
        id: send.id.clone(),
        to: None,
        from: Some(send.to.clone()),
        ok,
        error: error.map(str::to_string),
    };
    if !is_online(&st.db, user, &send.to).await? {
        return Ok(answer(false, Some("offline")));
    }
    // Подписка — до отправки: быстрое подтверждение не проскочит мимо
    let mut events = st.hub.subscribe();
    realtime::deliver(
        &st.hub,
        Audience::Device {
            user,
            device: send.to.clone(),
        },
        ServerEvent::ConnectCommand(ConnectCommand {
            id: send.id.clone(),
            from: from.clone(),
            command: send.command.clone(),
        }),
    );
    let wait = async {
        loop {
            match events.recv().await {
                Ok(env) => {
                    let mine = matches!(&env.to, Audience::Device { user: u, device } if *u == user && *device == from);
                    if let (true, ServerEvent::ConnectAck(ack)) = (mine, &env.event)
                        && ack.id == send.id
                    {
                        return Some(ack.clone());
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                Err(tokio::sync::broadcast::error::RecvError::Closed) => return None,
            }
        }
    };
    Ok(match tokio::time::timeout(REMOTE_ACK_WAIT, wait).await {
        Ok(Some(ack)) => ConnectAck { to: None, ..ack },
        _ => answer(false, Some("timeout")),
    })
}

/// Перенос с пульта по HTTP. Ответ — как подтверждение по WebSocket.
pub async fn remote_transfer(
    st: &AppState,
    principal: &Principal,
    t: ConnectTransfer,
) -> AppResult<ConnectAck> {
    let id = t.id.clone().unwrap_or_else(|| "transfer".into());
    let to = t.to.clone();
    let t = ConnectTransfer {
        id: Some(id.clone()),
        ..t
    };
    Ok(match transfer(st, principal.user.id, t).await? {
        Some(ServerEvent::ConnectAck(ack)) => ack,
        _ => ConnectAck {
            id,
            to: None,
            from: Some(to),
            ok: true,
            error: None,
        },
    })
}
