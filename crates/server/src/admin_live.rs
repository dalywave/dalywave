//! «Сейчас» в админке: кто в сети, на каких устройствах и что у него играет.
//!
//! «В сети» — то же, что у управления между устройствами: соединение открыто и устройство напоминало
//! о себе последние [`ONLINE_WINDOW_SECS`] секунд. Играет — снимок активного устройства, если оно на связи.
//! В приватной сессии трек и источник не показываются и админу: видно только, что сессия приватная.

use std::collections::HashMap;

use chrono::Utc;
use dalywave_protocol::{
    admin_users::{
        AdminPresence, LiveDevice, LiveNow, LiveOverview, LivePerson, LiveSummary, PersonBrief,
    },
    auth::Platform,
    connect::{ConnectKind, ONLINE_WINDOW_SECS, PlaybackState},
    realtime::ServerEvent,
};
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    error::AppResult,
    realtime::{self, Audience, Hub},
};

/// Админкам — «у человека что-то сменилось»: они перечитают «Сейчас». Только внутри процесса API:
/// все события, которые к этому ведут, рождаются здесь же.
pub fn notify(hub: &Hub, user: Uuid) {
    realtime::deliver(
        hub,
        Audience::Admins,
        ServerEvent::AdminPresence(AdminPresence { user_id: user }),
    );
}

/// Люди в сети (или один человек, если задан `only`): слушающие — первыми.
pub async fn people(db: &PgPool, only: Option<Uuid>) -> AppResult<Vec<LivePerson>> {
    let devices = sqlx::query!(
        r#"SELECT user_id, id, name, kind, platform, app_version
           FROM connect_devices
           WHERE connection IS NOT NULL AND last_seen_at > now() - make_interval(secs => $1)
             AND ($2::uuid IS NULL OR user_id = $2)
           ORDER BY last_seen_at DESC"#,
        ONLINE_WINDOW_SECS as f64,
        only,
    )
    .fetch_all(db)
    .await?;
    let mut by_user: HashMap<Uuid, Vec<LiveDevice>> = HashMap::new();
    for d in devices {
        by_user.entry(d.user_id).or_default().push(LiveDevice {
            id: d.id,
            name: d.name,
            kind: d.kind.parse().unwrap_or(ConnectKind::Web),
            platform: d.platform.parse().unwrap_or(Platform::Web),
            app_version: d.app_version,
            active: false,
        });
    }
    if by_user.is_empty() {
        return Ok(Vec::new());
    }
    let ids: Vec<Uuid> = by_user.keys().copied().collect();
    let rows = sqlx::query!(
        r#"SELECT u.id AS "id!", u.name AS "name!", u.avatar_url, p.active_connect_id AS "active?", p.state AS "state?",
                  EXISTS (SELECT 1 FROM private_sessions s
                          WHERE s.user_id = u.id AND s.started_at <= now() AND s.ended_at > now()) AS "private!"
           FROM users u
           -- У общего аккаунта демо снимков несколько (по входу гостя): последний
           LEFT JOIN LATERAL (SELECT active_connect_id, state FROM playback_state
                              WHERE user_id = u.id ORDER BY updated_at DESC LIMIT 1) p ON true
           WHERE u.id = ANY($1) AND NOT u.disabled"#,
        &ids,
    )
    .fetch_all(db)
    .await?;
    let now = Utc::now();
    let mut out: Vec<LivePerson> = rows
        .into_iter()
        .filter_map(|r| {
            let mut devices = by_user.remove(&r.id)?;
            let active = r.active.filter(|a| devices.iter().any(|d| &d.id == a));
            for d in &mut devices {
                d.active = active.as_deref() == Some(d.id.as_str());
            }
            devices.sort_by_key(|d| !d.active);
            let state = r
                .state
                .filter(|s| s.as_object().is_some_and(|o| !o.is_empty()))
                .and_then(|s| serde_json::from_value::<PlaybackState>(s).ok());
            let now_playing = match (&active, state) {
                (Some(device), Some(state)) if state.current.is_some() => Some(LiveNow {
                    private: r.private,
                    track: (!r.private)
                        .then(|| state.current.as_ref().map(|c| c.track.clone()))
                        .flatten(),
                    context: (!r.private).then(|| state.context.clone()).flatten(),
                    playing: state.playing,
                    position_ms: state.position_at(now),
                    duration_ms: state.duration_ms,
                    at: now,
                    device_id: Some(device.clone()),
                }),
                _ => None,
            };
            Some(LivePerson {
                user: PersonBrief {
                    id: r.id,
                    name: r.name,
                    avatar_url: r.avatar_url,
                },
                devices,
                now: now_playing,
            })
        })
        .collect();
    // Слушающие, затем на паузе, затем просто в сети; внутри — по имени
    out.sort_by(|a, b| {
        rank(a)
            .cmp(&rank(b))
            .then_with(|| a.user.name.to_lowercase().cmp(&b.user.name.to_lowercase()))
    });
    Ok(out)
}

fn rank(p: &LivePerson) -> u8 {
    match &p.now {
        Some(n) if n.playing => 0,
        Some(_) => 1,
        None => 2,
    }
}

pub async fn overview(db: &PgPool) -> AppResult<LiveOverview> {
    let people = people(db, None).await?;
    let counts = sqlx::query!(
        r#"SELECT (SELECT count(*) FROM play_events WHERE started_at > now() - interval '1 day') AS "plays!",
                  (SELECT count(*) FROM users WHERE created_at > now() - interval '7 days') AS "new_users!""#
    )
    .fetch_one(db)
    .await?;
    Ok(LiveOverview {
        summary: LiveSummary {
            online: people.len() as i64,
            listening: people
                .iter()
                .filter(|p| p.now.as_ref().is_some_and(|n| n.playing))
                .count() as i64,
            plays_24h: counts.plays,
            new_users_7d: counts.new_users,
        },
        people,
    })
}
