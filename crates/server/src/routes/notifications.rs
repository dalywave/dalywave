//! Центр уведомлений, настройки уведомлений и подписки устройств на push. Устройство подписывается
//! из своей сессии: выход или отзыв сессии выключает и подписку.

use axum::{
    Router,
    extract::State,
    http::StatusCode,
    routing::{get, post},
};
use dalywave_protocol::{
    ErrorBody,
    auth::Permission,
    notifications::{
        NotificationKind, NotificationPage, NotificationQuery, NotificationReadRequest,
        NotificationSettings, NotificationSettingsPatch, NotificationsRead, PushChannel,
        PushDevice, PushInfo, PushRegistered, PushRegistration, PushTest, PushUnregister,
    },
};
use uuid::Uuid;

use crate::{
    auth::Principal,
    cursor,
    error::{AppResult, bad},
    extract::{Json, Query},
    notify::{self, Draft},
    state::AppState,
};

/// Длиннее не бывает: токен APNs — 64 hex, адрес Web Push — пара сотен символов.
const MAX_TOKEN: usize = 2048;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/notifications", get(list))
        .route("/notifications/read", post(read))
        .route(
            "/notifications/settings",
            get(get_settings).patch(update_settings),
        )
        .route(
            "/notifications/push",
            get(push_info).post(subscribe).delete(unsubscribe),
        )
        .route("/notifications/test", post(test))
}

#[utoipa::path(
    get,
    path = "/api/notifications",
    tag = "notifications",
    security(("session" = []), ("device" = [])),
    params(NotificationQuery),
    responses(
        (status = 200, description = "Уведомления, новые первыми, и сколько непрочитанных", body = NotificationPage),
        (status = 400, description = "Неверный курсор или limit", body = ErrorBody),
    ),
)]
pub async fn list(
    State(st): State<AppState>,
    principal: Principal,
    Query(q): Query<NotificationQuery>,
) -> AppResult<Json<NotificationPage>> {
    let user = principal.user.id;
    let limit = cursor::limit(q.limit, 30, 100)?;
    let before = cursor::decode_opt(q.before.as_deref())?;
    let (items, next) = notify::list(&st.db, user, before, limit).await?;
    Ok(Json(NotificationPage {
        items,
        unread: notify::unread(&st.db, user).await?,
        next,
    }))
}

#[utoipa::path(
    post,
    path = "/api/notifications/read",
    tag = "notifications",
    security(("session" = []), ("device" = [])),
    request_body = NotificationReadRequest,
    responses(
        (status = 200, description = "Отмечено; другие устройства получат `notification.read`", body = NotificationsRead),
    ),
)]
pub async fn read(
    State(st): State<AppState>,
    principal: Principal,
    Json(body): Json<NotificationReadRequest>,
) -> AppResult<Json<NotificationsRead>> {
    if body.ids.len() > 500 {
        return Err(bad("не больше 500 уведомлений за раз"));
    }
    Ok(Json(
        notify::mark_read(&st.db, principal.user.id, &body.ids).await?,
    ))
}

#[utoipa::path(
    get,
    path = "/api/notifications/settings",
    tag = "notifications",
    security(("session" = []), ("device" = [])),
    responses((status = 200, description = "Какие уведомления присылать и тихие часы", body = NotificationSettings)),
)]
pub async fn get_settings(
    State(st): State<AppState>,
    principal: Principal,
) -> AppResult<Json<NotificationSettings>> {
    let mut conn = st.db.acquire().await?;
    let prefs = notify::prefs(&mut conn, principal.user.id).await?;
    Ok(Json(prefs.view(principal.can(Permission::Admin))))
}

#[utoipa::path(
    patch,
    path = "/api/notifications/settings",
    tag = "notifications",
    security(("session" = []), ("device" = [])),
    request_body = NotificationSettingsPatch,
    responses(
        (status = 200, description = "Настройки после правки", body = NotificationSettings),
        (status = 400, description = "Неверное время тихих часов или часовой пояс", body = ErrorBody),
    ),
)]
pub async fn update_settings(
    State(st): State<AppState>,
    principal: Principal,
    Json(patch): Json<NotificationSettingsPatch>,
) -> AppResult<Json<NotificationSettings>> {
    let user = principal.user.id;
    if let Some(q) = &patch.quiet
        && (q.from > 1439 || q.to > 1439)
    {
        return Err(bad("время тихих часов — минуты от полуночи, 0–1439"));
    }
    if patch
        .utc_offset_min
        .is_some_and(|o| !(-840..=840).contains(&o))
    {
        return Err(bad("часовой пояс — от −840 до 840 минут к UTC"));
    }
    let mut tx = st.db.begin().await?;
    let current = notify::prefs(&mut tx, user).await?;
    let mut disabled = current.disabled.clone();
    for k in patch.kinds.iter().flatten() {
        if !k.kind.configurable() {
            continue;
        }
        disabled.retain(|d| *d != k.kind);
        if !k.enabled {
            disabled.push(k.kind);
        }
    }
    let disabled: Vec<&str> = disabled.iter().map(|k| k.as_str()).collect();
    let quiet = patch.quiet.unwrap_or(current.quiet);
    sqlx::query!(
        "INSERT INTO notification_settings (user_id, disabled_kinds, quiet_enabled, quiet_from, quiet_to, utc_offset_min)
         VALUES ($1, $2, $3, $4, $5, $6)
         ON CONFLICT (user_id) DO UPDATE SET
             disabled_kinds = excluded.disabled_kinds, quiet_enabled = excluded.quiet_enabled,
             quiet_from = excluded.quiet_from, quiet_to = excluded.quiet_to,
             utc_offset_min = COALESCE(excluded.utc_offset_min, notification_settings.utc_offset_min),
             updated_at = now()",
        user,
        &disabled as &[&str],
        quiet.enabled,
        quiet.from as i16,
        quiet.to as i16,
        patch.utc_offset_min,
    )
    .execute(&mut *tx)
    .await?;
    let prefs = notify::prefs(&mut tx, user).await?;
    tx.commit().await?;
    Ok(Json(prefs.view(principal.can(Permission::Admin))))
}

/// Каналы, настроенные на сервере. APNs: ключ лежит у воркера, API видит только настройки.
fn channel_enabled(st: &AppState, channel: PushChannel) -> bool {
    match channel {
        PushChannel::Apns => st.config.push.apns.is_some(),
        PushChannel::Webpush => st.config.push.vapid.is_some(),
        PushChannel::Fcm => false,
    }
}

#[utoipa::path(
    get,
    path = "/api/notifications/push",
    tag = "notifications",
    security(("session" = []), ("device" = [])),
    responses((status = 200, description = "Каналы push на сервере и подписанные устройства человека", body = PushInfo)),
)]
pub async fn push_info(
    State(st): State<AppState>,
    principal: Principal,
) -> AppResult<Json<PushInfo>> {
    let rows = sqlx::query!(
        "SELECT p.id, p.channel, p.name, p.session_id, p.created_at, p.last_sent_at
         FROM push_subscriptions p LEFT JOIN sessions s ON s.id = p.session_id
         WHERE p.user_id = $1 AND p.disabled_at IS NULL
           AND (p.session_id IS NULL OR (s.revoked_at IS NULL AND s.expires_at > now()))
         ORDER BY p.created_at DESC",
        principal.user.id
    )
    .fetch_all(&st.db)
    .await?;
    let devices = rows
        .into_iter()
        .filter_map(|r| {
            Some(PushDevice {
                id: r.id,
                channel: r.channel.parse().ok()?,
                name: r.name,
                current: r.session_id == Some(principal.session_id),
                created_at: r.created_at,
                last_sent_at: r.last_sent_at,
            })
        })
        .collect();
    Ok(Json(PushInfo {
        web_push_key: st.config.push.vapid.as_ref().map(|v| v.public_key.clone()),
        apns: channel_enabled(&st, PushChannel::Apns),
        fcm: channel_enabled(&st, PushChannel::Fcm),
        devices,
    }))
}

/// Имя устройства для списка: приложение — по устройству сессии, сайт — по браузеру.
async fn device_name(st: &AppState, principal: &Principal) -> AppResult<String> {
    if let Some(device) = principal.device_id {
        let name = sqlx::query_scalar!("SELECT name FROM devices WHERE id = $1", device)
            .fetch_optional(&st.db)
            .await?;
        if let Some(name) = name.filter(|n| !n.trim().is_empty()) {
            return Ok(name);
        }
    }
    let ua = sqlx::query_scalar!(
        "SELECT user_agent FROM sessions WHERE id = $1",
        principal.session_id
    )
    .fetch_optional(&st.db)
    .await?
    .flatten()
    .unwrap_or_default();
    Ok(browser_name(&ua))
}

/// «Safari на iPhone», «Chrome на Android», «Firefox на Windows».
pub fn browser_name(ua: &str) -> String {
    let browser = if ua.contains("Edg/") {
        "Edge"
    } else if ua.contains("Firefox/") || ua.contains("FxiOS") {
        "Firefox"
    } else if ua.contains("Chrome/") || ua.contains("CriOS") {
        "Chrome"
    } else if ua.contains("Safari/") {
        "Safari"
    } else {
        "Браузер"
    };
    let os = if ua.contains("iPhone") {
        Some("iPhone")
    } else if ua.contains("iPad") {
        Some("iPad")
    } else if ua.contains("Android") {
        Some("Android")
    } else if ua.contains("Mac OS X") || ua.contains("Macintosh") {
        Some("Mac")
    } else if ua.contains("Windows") {
        Some("Windows")
    } else if ua.contains("Linux") {
        Some("Linux")
    } else {
        None
    };
    match os {
        Some(os) => format!("{browser} на {os}"),
        None => browser.to_string(),
    }
}

#[utoipa::path(
    post,
    path = "/api/notifications/push",
    tag = "notifications",
    security(("session" = []), ("device" = [])),
    request_body = PushRegistration,
    responses(
        (status = 201, description = "Устройство подписано (повтор с тем же токеном обновляет подписку)", body = PushRegistered),
        (status = 400, description = "Нет токена или подписки браузера, канал на сервере выключен", body = ErrorBody),
    ),
)]
pub async fn subscribe(
    State(st): State<AppState>,
    principal: Principal,
    Json(body): Json<PushRegistration>,
) -> AppResult<(StatusCode, Json<PushRegistered>)> {
    if !channel_enabled(&st, body.channel) {
        return Err(bad("этот канал уведомлений на сервере выключен"));
    }
    let (token, p256dh, auth) = match body.channel {
        PushChannel::Apns | PushChannel::Fcm => {
            let token = body
                .token
                .as_deref()
                .map(str::trim)
                .filter(|t| !t.is_empty())
                .ok_or_else(|| bad("нужен token устройства"))?;
            if body.channel == PushChannel::Apns && !token.chars().all(|c| c.is_ascii_hexdigit()) {
                return Err(bad("токен APNs — hex"));
            }
            (token.to_ascii_lowercase(), None, None)
        }
        PushChannel::Webpush => {
            let sub = body
                .subscription
                .as_ref()
                .ok_or_else(|| bad("нужна подписка браузера (subscription)"))?;
            // Службы push — только https; http — лишь на этой машине (тесты, разработка)
            let endpoint = url::Url::parse(&sub.endpoint)
                .ok()
                .filter(|u| {
                    u.scheme() == "https"
                        || (u.scheme() == "http"
                            && matches!(u.host_str(), Some("127.0.0.1" | "localhost")))
                })
                .ok_or_else(|| bad("адрес подписки — https-ссылка"))?;
            let key_ok = crate::crypto::b64_decode(&sub.keys.p256dh).is_some_and(|k| k.len() == 65);
            let auth_ok = crate::crypto::b64_decode(&sub.keys.auth).is_some_and(|k| k.len() == 16);
            if !key_ok || !auth_ok {
                return Err(bad("ключи подписки браузера неверные"));
            }
            (
                endpoint.to_string(),
                Some(sub.keys.p256dh.clone()),
                Some(sub.keys.auth.clone()),
            )
        }
    };
    if token.len() > MAX_TOKEN {
        return Err(bad("слишком длинный токен"));
    }
    let name = match body
        .name
        .as_deref()
        .map(str::trim)
        .filter(|n| !n.is_empty())
    {
        Some(n) => n.chars().take(80).collect(),
        None => device_name(&st, &principal).await?,
    };
    let environment =
        (body.channel == PushChannel::Apns).then(|| body.environment.unwrap_or_default().as_str());
    let mut tx = st.db.begin().await?;
    // Тот же токен у другого человека (на устройстве сменили вход) — теперь он этого
    let id = sqlx::query_scalar!(
        "INSERT INTO push_subscriptions (user_id, session_id, channel, token, p256dh, auth, environment, name)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
         ON CONFLICT (channel, token) DO UPDATE SET
             user_id = excluded.user_id, session_id = excluded.session_id, p256dh = excluded.p256dh,
             auth = excluded.auth, environment = excluded.environment, name = excluded.name,
             disabled_at = NULL, disabled_reason = NULL, updated_at = now()
         RETURNING id",
        principal.user.id,
        principal.session_id,
        body.channel.as_str(),
        token,
        p256dh,
        auth,
        environment,
        name,
    )
    .fetch_one(&mut *tx)
    .await?;
    // Одна подписка на канал у сессии: браузер сменил подписку — прежняя больше не нужна
    sqlx::query!(
        "UPDATE push_subscriptions SET disabled_at = now(), disabled_reason = 'replaced', updated_at = now()
         WHERE session_id = $1 AND channel = $2 AND id <> $3 AND disabled_at IS NULL",
        principal.session_id,
        body.channel.as_str(),
        id,
    )
    .execute(&mut *tx)
    .await?;
    if let Some(offset) = body.utc_offset_min.filter(|o| (-840..=840).contains(o)) {
        sqlx::query!(
            "INSERT INTO notification_settings (user_id, utc_offset_min) VALUES ($1, $2)
             ON CONFLICT (user_id) DO UPDATE SET utc_offset_min = excluded.utc_offset_min, updated_at = now()",
            principal.user.id,
            offset,
        )
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    Ok((StatusCode::CREATED, Json(PushRegistered { id })))
}

#[utoipa::path(
    delete,
    path = "/api/notifications/push",
    tag = "notifications",
    security(("session" = []), ("device" = [])),
    request_body = PushUnregister,
    responses(
        (status = 204, description = "Устройство больше не получает push (если подписки не было — тоже)"),
        (status = 400, description = "Нужен token или endpoint", body = ErrorBody),
    ),
)]
pub async fn unsubscribe(
    State(st): State<AppState>,
    principal: Principal,
    Json(body): Json<PushUnregister>,
) -> AppResult<StatusCode> {
    let token = match (body.token, body.endpoint) {
        (Some(t), _) => t.trim().to_ascii_lowercase(),
        (None, Some(e)) => url::Url::parse(e.trim())
            .map(String::from)
            .unwrap_or_else(|_| e.trim().to_string()),
        (None, None) => return Err(bad("нужен token или endpoint")),
    };
    sqlx::query!(
        "DELETE FROM push_subscriptions WHERE user_id = $1 AND token = $2",
        principal.user.id,
        token,
    )
    .execute(&st.db)
    .await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    post,
    path = "/api/notifications/test",
    tag = "notifications",
    security(("session" = []), ("device" = [])),
    responses(
        (status = 200, description = "Пробное уведомление: в центре уведомлений и push на подписанные устройства \
                                      (и в тихие часы)", body = PushTest),
    ),
)]
pub async fn test(State(st): State<AppState>, principal: Principal) -> AppResult<Json<PushTest>> {
    let user = principal.user.id;
    let channels: Vec<&str> = [PushChannel::Apns, PushChannel::Webpush, PushChannel::Fcm]
        .into_iter()
        .filter(|c| channel_enabled(&st, *c))
        .map(PushChannel::as_str)
        .collect();
    let devices = sqlx::query_scalar!(
        r#"SELECT count(*) AS "n!" FROM push_subscriptions p LEFT JOIN sessions s ON s.id = p.session_id
           WHERE p.user_id = $1 AND p.disabled_at IS NULL AND p.channel = ANY($2)
             AND (p.session_id IS NULL OR (s.revoked_at IS NULL AND s.expires_at > now()))"#,
        user,
        &channels as &[&str],
    )
    .fetch_one(&st.db)
    .await?;
    let id: Option<Uuid> = notify::emit(
        &st.db,
        user,
        Draft::new(
            NotificationKind::Test,
            "Уведомления работают",
            "Так будут выглядеть новые релизы и другие новости",
        )
        .link("/notifications"),
    )
    .await?;
    tracing::info!(notification = ?id, devices, "test notification");
    Ok(Json(PushTest { devices }))
}

#[cfg(test)]
mod tests {
    use super::browser_name;

    #[test]
    fn browsers() {
        assert_eq!(
            browser_name(
                "Mozilla/5.0 (iPhone; CPU iPhone OS 18_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.0 Mobile/15E148 Safari/604.1"
            ),
            "Safari на iPhone"
        );
        assert_eq!(
            browser_name(
                "Mozilla/5.0 (Linux; Android 14) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0 Mobile Safari/537.36"
            ),
            "Chrome на Android"
        );
        assert_eq!(
            browser_name(
                "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0 Safari/537.36 Edg/130.0"
            ),
            "Edge на Mac"
        );
        assert_eq!(browser_name(""), "Браузер");
    }
}
