//! «Слушать вместе» (как Jam у Spotify): сессия хозяина, гости по ссылке без аккаунта, треки от
//! гостей в очереди его активного устройства. Типы — [`dalywave_protocol::jam`].
//!
//! Гость получает токен `dwj_…`: он проверяется на каждом запросе к `/api/jam/guest/*` и больше нигде
//! не принимается (обычный вход ждёт `dwd_…` или cookie). Сессия закрыта, если хозяин её закончил, она
//! простояла без дел [`IDLE_HOURS`] часов или хозяин потерял доступ к сервису.
//!
//! Очередь по-прежнему у активного устройства хозяина: трек гостя уходит ему командой `enqueue`
//! (как с пульта), а подпись «добавил Миша» строится по записям `jam_requests`.

use std::collections::HashMap;

use chrono::{DateTime, Duration, Utc};
use dalywave_protocol::{
    catalog::Track,
    connect::{ConnectCommand, PlaybackState, QueueSource, RemoteCommand, RemoteTrack},
    jam::{
        IDLE_HOURS, Jam, JamChanged, JamGuest, JamNow, JamQueueItem, JamRequest, JamRequestStatus,
        JamSettings, JamSkip, JamTrack, MAX_GUEST_NAME,
    },
    realtime::ServerEvent,
};
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    catalog::views,
    connect,
    crypto::{b64, random_bytes, random_token, sha256},
    error::{AppError, AppResult},
    realtime::{self, Audience},
    state::AppState,
};

pub const TOKEN_PREFIX: &str = "dwj_";
/// Гость «на связи», если страница напоминала о себе за последние 2 минуты.
const ONLINE_SECS: f64 = 120.0;
/// Ближайших треков в очереди для гостя.
const QUEUE_WINDOW: usize = 30;
/// Сколько треков гостя может ждать одобрения разом.
pub const MAX_PENDING: i64 = 5;
/// От кого команды хозяину: ответ на них никому не нужен.
const COMMAND_FROM: &str = "jam";

/// Открытая сессия.
#[derive(Debug, Clone)]
pub struct Session {
    pub id: Uuid,
    pub host: Uuid,
    pub code: String,
    pub settings: JamSettings,
    pub created_at: DateTime<Utc>,
    pub last_activity_at: DateTime<Utc>,
}

impl Session {
    pub fn expires_at(&self) -> DateTime<Utc> {
        self.last_activity_at + Duration::hours(IDLE_HOURS)
    }
}

/// Гость, чей токен прошёл проверку.
#[derive(Debug, Clone)]
pub struct Guest {
    pub id: Uuid,
    pub name: String,
    pub member: bool,
    pub joined_at: DateTime<Utc>,
    pub session: Session,
    pub host_name: String,
}

pub fn ended() -> AppError {
    AppError::Missing {
        code: "jam_ended",
        message: "Эта сессия уже закончилась".into(),
    }
}

/// Код ссылки: 96 случайных бит.
pub fn new_code() -> AppResult<String> {
    Ok(b64(random_bytes::<12>()?))
}

pub fn valid_code(code: &str) -> bool {
    (12..=32).contains(&code.len())
        && code
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

/// Имя гостя: без управляющих символов и лишних пробелов, до [`MAX_GUEST_NAME`] символов.
pub fn clean_name(raw: &str) -> Option<String> {
    let name: String = raw
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .filter(|c| !c.is_control())
        .take(MAX_GUEST_NAME)
        .collect();
    let name = name.trim().to_string();
    (!name.is_empty()).then_some(name)
}

pub fn host_label(name: &str) -> String {
    let name = name.trim();
    if name.is_empty() {
        "DalyWave".into()
    } else {
        name.to_string()
    }
}

struct SessionRow {
    id: Uuid,
    user_id: Uuid,
    code: String,
    guests_can_add: bool,
    approval: bool,
    guests_can_listen: bool,
    created_at: DateTime<Utc>,
    last_activity_at: DateTime<Utc>,
}

impl From<SessionRow> for Session {
    fn from(r: SessionRow) -> Self {
        Self {
            id: r.id,
            host: r.user_id,
            code: r.code,
            settings: JamSettings {
                guests_can_add: r.guests_can_add,
                approval: r.approval,
                guests_can_listen: r.guests_can_listen,
            },
            created_at: r.created_at,
            last_activity_at: r.last_activity_at,
        }
    }
}

/// Открытая сессия хозяина. Простоявшую без дел закрывает тут же.
pub async fn open_of(db: &PgPool, host: Uuid) -> AppResult<Option<Session>> {
    sqlx::query!(
        "UPDATE jam_sessions SET ended_at = now()
         WHERE user_id = $1 AND ended_at IS NULL AND last_activity_at <= now() - make_interval(hours => $2)",
        host,
        IDLE_HOURS as i32,
    )
    .execute(db)
    .await?;
    let row = sqlx::query_as!(
        SessionRow,
        "SELECT id, user_id, code, guests_can_add, approval, guests_can_listen, created_at, last_activity_at
         FROM jam_sessions WHERE user_id = $1 AND ended_at IS NULL",
        host,
    )
    .fetch_optional(db)
    .await?;
    Ok(row.map(Session::from))
}

/// Сессия по коду ссылки — только открытая и у хозяина с доступом к сервису.
pub async fn by_code(
    db: &PgPool,
    code: &str,
) -> AppResult<Option<(Session, String, Option<String>)>> {
    if !valid_code(code) {
        return Ok(None);
    }
    let row = sqlx::query!(
        r#"SELECT j.id, j.user_id, j.code, j.guests_can_add, j.approval, j.guests_can_listen, j.created_at,
                  j.last_activity_at, u.name AS host_name, u.avatar_url
           FROM jam_sessions j JOIN users u ON u.id = j.user_id
           WHERE j.code = $1 AND j.ended_at IS NULL
             AND j.last_activity_at > now() - make_interval(hours => $2)
             AND NOT u.disabled AND cardinality(u.roles) > 0"#,
        code,
        IDLE_HOURS as i32,
    )
    .fetch_optional(db)
    .await?;
    Ok(row.map(|r| {
        (
            Session {
                id: r.id,
                host: r.user_id,
                code: r.code,
                settings: JamSettings {
                    guests_can_add: r.guests_can_add,
                    approval: r.approval,
                    guests_can_listen: r.guests_can_listen,
                },
                created_at: r.created_at,
                last_activity_at: r.last_activity_at,
            },
            host_label(&r.host_name),
            r.avatar_url,
        )
    }))
}

/// Гость по токену. `Err(jam_ended)` — сессия закрыта; `Err(jam_removed)` — хозяин убрал гостя.
pub async fn guest_by_token(db: &PgPool, token: &str) -> AppResult<Guest> {
    if !token.starts_with(TOKEN_PREFIX) {
        return Err(AppError::Unauthorized);
    }
    let row = sqlx::query!(
        r#"SELECT g.id, g.name, g.user_id IS NOT NULL AS "member!", g.joined_at, g.removed_at,
                  j.id AS jam_id, j.user_id, j.code, j.guests_can_add, j.approval, j.guests_can_listen,
                  j.created_at, j.last_activity_at, j.ended_at,
                  (j.ended_at IS NULL AND j.last_activity_at > now() - make_interval(hours => $2)
                   AND NOT u.disabled AND cardinality(u.roles) > 0) AS "open!",
                  u.name AS host_name
           FROM jam_guests g JOIN jam_sessions j ON j.id = g.jam_id JOIN users u ON u.id = j.user_id
           WHERE g.token_hash = $1"#,
        &sha256(token)[..],
        IDLE_HOURS as i32,
    )
    .fetch_optional(db)
    .await?
    .ok_or(AppError::Unauthorized)?;
    if !row.open {
        return Err(ended());
    }
    if row.removed_at.is_some() {
        return Err(AppError::Missing {
            code: "jam_removed",
            message: "Хозяин убрал вас из сессии".into(),
        });
    }
    Ok(Guest {
        id: row.id,
        name: row.name,
        member: row.member,
        joined_at: row.joined_at,
        host_name: host_label(&row.host_name),
        session: Session {
            id: row.jam_id,
            host: row.user_id,
            code: row.code,
            settings: JamSettings {
                guests_can_add: row.guests_can_add,
                approval: row.approval,
                guests_can_listen: row.guests_can_listen,
            },
            created_at: row.created_at,
            last_activity_at: row.last_activity_at,
        },
    })
}

/// Сессия жива: сдвинуть срок. В базу — не чаще раза в минуту.
pub async fn touch(db: &PgPool, jam: Uuid, guest: Option<Uuid>) -> AppResult<()> {
    sqlx::query!(
        "UPDATE jam_sessions SET last_activity_at = now()
         WHERE id = $1 AND ended_at IS NULL AND last_activity_at < now() - interval '1 minute'",
        jam,
    )
    .execute(db)
    .await?;
    if let Some(guest) = guest {
        sqlx::query!(
            "UPDATE jam_guests SET last_seen_at = now()
             WHERE id = $1 AND last_seen_at < now() - interval '30 seconds'",
            guest,
        )
        .execute(db)
        .await?;
    }
    Ok(())
}

/// Новый гость: токен возвращается один раз.
pub async fn join(
    db: &PgPool,
    jam: Uuid,
    name: &str,
    member: Option<Uuid>,
) -> AppResult<(String, JamGuest)> {
    let token = format!("{TOKEN_PREFIX}{}", random_token()?);
    let row = sqlx::query!(
        "INSERT INTO jam_guests (jam_id, name, user_id, token_hash) VALUES ($1, $2, $3, $4)
         RETURNING id, joined_at",
        jam,
        name,
        member,
        &sha256(&token)[..],
    )
    .fetch_one(db)
    .await?;
    touch(db, jam, None).await?;
    Ok((
        token,
        JamGuest {
            id: row.id,
            name: name.to_string(),
            member: member.is_some(),
            online: true,
            joined_at: row.joined_at,
        },
    ))
}

pub fn jam_track(t: Track) -> JamTrack {
    JamTrack {
        id: t.id,
        title: t.title,
        version: t.version,
        artists: t.artists.into_iter().map(|a| a.name).collect(),
        album: t.album.as_ref().map(|a| a.title.clone()),
        image: t.album.and_then(|a| a.image),
        duration_ms: t.duration_ms,
        explicit: t.explicit,
    }
}

/// Треки по id для гостей — без отметок медиатеки хозяина.
pub async fn jam_tracks(db: &PgPool, ids: &[Uuid]) -> AppResult<Vec<JamTrack>> {
    Ok(views::tracks(db, ids, Uuid::nil())
        .await?
        .into_iter()
        .map(jam_track)
        .collect())
}

pub async fn guests(db: &PgPool, jam: Uuid) -> AppResult<Vec<JamGuest>> {
    let rows = sqlx::query!(
        r#"SELECT id, name, user_id IS NOT NULL AS "member!", joined_at,
                  (last_seen_at > now() - make_interval(secs => $2)) AS "online!"
           FROM jam_guests WHERE jam_id = $1 AND removed_at IS NULL
           ORDER BY joined_at"#,
        jam,
        ONLINE_SECS,
    )
    .fetch_all(db)
    .await?;
    Ok(rows
        .into_iter()
        .map(|r| JamGuest {
            id: r.id,
            name: r.name,
            member: r.member,
            online: r.online,
            joined_at: r.joined_at,
        })
        .collect())
}

/// Треки гостей: `guest` — только его, `statuses` — какие.
pub async fn requests(
    db: &PgPool,
    jam: Uuid,
    guest: Option<Uuid>,
    statuses: &[JamRequestStatus],
) -> AppResult<Vec<JamRequest>> {
    let statuses: Vec<String> = statuses.iter().map(|s| s.as_str().to_string()).collect();
    let rows = sqlx::query!(
        "SELECT r.id, r.track_id, r.guest_id, g.name AS guest_name, r.status, r.created_at
         FROM jam_requests r JOIN jam_guests g ON g.id = r.guest_id
         WHERE r.jam_id = $1 AND ($2::uuid IS NULL OR r.guest_id = $2) AND r.status = ANY($3)
         ORDER BY r.created_at DESC
         LIMIT 200",
        jam,
        guest,
        &statuses,
    )
    .fetch_all(db)
    .await?;
    let ids: Vec<Uuid> = rows.iter().map(|r| r.track_id).collect();
    let tracks: HashMap<Uuid, JamTrack> = jam_tracks(db, &ids)
        .await?
        .into_iter()
        .map(|t| (t.id, t))
        .collect();
    Ok(rows
        .into_iter()
        .filter_map(|r| {
            Some(JamRequest {
                id: r.id,
                track: tracks.get(&r.track_id)?.clone(),
                guest_id: r.guest_id,
                guest_name: r.guest_name,
                status: r.status.parse().ok()?,
                created_at: r.created_at,
            })
        })
        .collect())
}

/// Сессия глазами хозяина.
pub async fn host_view(st: &AppState, s: &Session) -> AppResult<Jam> {
    Ok(Jam {
        id: s.id,
        code: s.code.clone(),
        url: format!("{}/jam/{}", st.config.public_url, s.code),
        settings: s.settings,
        guests: guests(&st.db, s.id).await?,
        requests: requests(
            &st.db,
            s.id,
            None,
            &[JamRequestStatus::Pending, JamRequestStatus::Added],
        )
        .await?,
        created_at: s.created_at,
        expires_at: s.expires_at(),
    })
}

/// Кто добавил трек: track id → имя гостя (последнее добавление).
pub async fn added_by(db: &PgPool, jam: Uuid) -> AppResult<HashMap<Uuid, String>> {
    let rows = sqlx::query!(
        "SELECT DISTINCT ON (r.track_id) r.track_id, g.name
         FROM jam_requests r JOIN jam_guests g ON g.id = r.guest_id
         WHERE r.jam_id = $1 AND r.status = 'added'
         ORDER BY r.track_id, r.created_at DESC",
        jam,
    )
    .fetch_all(db)
    .await?;
    Ok(rows.into_iter().map(|r| (r.track_id, r.name)).collect())
}

/// Что играет — для гостя: текущий трек, позиция и ближайшая очередь, без остального состояния.
pub fn now_of(state: &PlaybackState, added: &HashMap<Uuid, String>, live: bool) -> JamNow {
    let item = |q: &dalywave_protocol::connect::QueueItem| JamQueueItem {
        track: jam_track(q.track.clone()),
        // Подпись — только у добавленного вручную: тот же трек из альбома добавил не гость
        added_by: (q.source == QueueSource::Manual)
            .then(|| added.get(&q.track.id).cloned())
            .flatten(),
    };
    let at = state.at.unwrap_or_else(Utc::now);
    JamNow {
        current: state.current.as_ref().map(item),
        queue: state
            .manual
            .iter()
            .chain(&state.upcoming)
            .take(QUEUE_WINDOW)
            .map(item)
            .collect(),
        playing: live && state.playing && state.pending.is_none(),
        position_ms: state.position_ms,
        duration_ms: state.duration_ms,
        at,
        live,
    }
}

/// Активное устройство хозяина, если оно на связи.
pub async fn live_device(db: &PgPool, host: Uuid) -> AppResult<Option<String>> {
    let list = connect::devices(db, host).await?;
    Ok(list
        .devices
        .into_iter()
        .find(|d| d.active && d.online)
        .map(|d| d.id))
}

/// Что играет у хозяина сейчас (позиция досчитана).
pub async fn now_playing(db: &PgPool, s: &Session) -> AppResult<Option<JamNow>> {
    let (_, state) = connect::playback(db, s.host).await?;
    let Some(state) = state else {
        return Ok(None);
    };
    let live = live_device(db, s.host).await?.is_some();
    let added = added_by(db, s.id).await?;
    Ok(Some(now_of(&state.advanced(Utc::now()), &added, live)))
}

/// Команда активному устройству хозяина. `false` — у него ничего не на связи.
pub async fn command_host(st: &AppState, host: Uuid, command: RemoteCommand) -> AppResult<bool> {
    let Some(device) = live_device(&st.db, host).await? else {
        return Ok(false);
    };
    realtime::deliver(
        &st.hub,
        Audience::Device { user: host, device },
        ServerEvent::ConnectCommand(ConnectCommand {
            id: Uuid::now_v7().to_string(),
            from: COMMAND_FROM.into(),
            command: Box::new(command),
        }),
    );
    Ok(true)
}

/// Трек — в очередь хозяина (в конец добавленного вручную).
pub async fn enqueue(st: &AppState, host: Uuid, track_id: Uuid) -> AppResult<bool> {
    let track = views::track(&st.db, track_id, host)
        .await?
        .ok_or(AppError::NotFound)?;
    command_host(st, host, RemoteCommand::Enqueue(RemoteTrack { track })).await
}

pub fn nothing_playing() -> AppError {
    AppError::Conflict {
        code: "nothing_playing",
        message: "Сейчас у хозяина ничего не играет — попробуйте, когда включится музыка".into(),
    }
}

/// Всем в сессии: перечитать.
pub fn changed(st: &AppState, s: &Session, ended: bool, removed: Option<Uuid>) {
    realtime::deliver(
        &st.hub,
        Audience::Jam {
            host: s.host,
            jam: s.id,
        },
        ServerEvent::JamChanged(JamChanged {
            jam_id: s.id,
            ended,
            removed,
        }),
    );
}

/// Голоса «пропустить» за текущий трек.
pub async fn skip_state(
    db: &PgPool,
    jam: Uuid,
    track: Option<Uuid>,
    guest: Option<Uuid>,
) -> AppResult<JamSkip> {
    let online = sqlx::query_scalar!(
        r#"SELECT count(*) AS "n!" FROM jam_guests
           WHERE jam_id = $1 AND removed_at IS NULL AND last_seen_at > now() - make_interval(secs => $2)"#,
        jam,
        ONLINE_SECS,
    )
    .fetch_one(db)
    .await?;
    // Больше половины гостей на связи
    let needed = (online.max(1) / 2 + 1) as u32;
    let Some(track) = track else {
        return Ok(JamSkip {
            votes: 0,
            needed,
            voted: false,
        });
    };
    let row = sqlx::query!(
        r#"SELECT count(*) AS "votes!", coalesce(bool_or(guest_id = $3), false) AS "voted!"
           FROM jam_skip_votes WHERE jam_id = $1 AND track_id = $2"#,
        jam,
        track,
        guest,
    )
    .fetch_one(db)
    .await?;
    Ok(JamSkip {
        votes: row.votes as u32,
        needed,
        voted: row.voted,
    })
}

/// Текущий трек хозяина (по последнему снимку).
pub async fn current_track(db: &PgPool, host: Uuid) -> AppResult<Option<Uuid>> {
    let (_, state) = connect::playback(db, host).await?;
    Ok(state.and_then(|s| s.current.map(|q| q.track.id)))
}

/// Треки, которые гость может слушать у себя: текущий и следующий.
pub async fn listenable(db: &PgPool, host: Uuid) -> AppResult<Vec<Uuid>> {
    let (_, state) = connect::playback(db, host).await?;
    Ok(state
        .map(|s| {
            s.current
                .iter()
                .chain(s.manual.iter().chain(&s.upcoming).take(1))
                .map(|q| q.track.id)
                .collect()
        })
        .unwrap_or_default())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_cleaned() {
        assert_eq!(clean_name("  Миша  ").as_deref(), Some("Миша"));
        assert_eq!(
            clean_name("Миша\u{0}\n Петров").as_deref(),
            Some("Миша Петров")
        );
        assert_eq!(clean_name("   "), None);
        assert_eq!(
            clean_name(&"я".repeat(80)).unwrap().chars().count(),
            MAX_GUEST_NAME
        );
    }

    #[test]
    fn codes_are_url_safe() {
        let code = new_code().unwrap();
        assert_eq!(code.len(), 16);
        assert!(valid_code(&code));
        assert!(!valid_code("short"));
        assert!(!valid_code("abc/def/ghi/jkl1"));
        assert!(!valid_code(&"a".repeat(40)));
    }
}
