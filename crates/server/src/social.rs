//! Друзья: кем люди приходятся друг другу, что слушают друзья, приватность и совпадение вкусов.
//! HTTP — [`crate::routes::friends`], совместные плейлисты — [`crate::routes::collab`].
//!
//! Что видят друзья. Сейчас играет — у человека есть активное устройство на связи и снимок «играет»
//! (`playback_state`, его ведёт управление между устройствами). Иначе — последнее прослушивание из
//! истории. Ничего — если выключено «Показывать друзьям, что я слушаю» или идёт приватная сессия;
//! сыгранное в приватной сессии не показывается и потом (ни «последним», ни в топе за месяц).

use std::collections::HashMap;

use chrono::{Duration, Utc};
use dalywave_protocol::{
    connect::{ONLINE_WINDOW_SECS, PlaybackState},
    events::ContextType,
    jam::IDLE_HOURS,
    realtime::ServerEvent,
    social::{
        ActivityContext, FriendActivity, FriendActivityEvent, FriendTopArtist, FriendsChange,
        PersonRef, Privacy, Relation, TasteMatch,
    },
};
use sqlx::{PgExecutor, PgPool};
use uuid::Uuid;

use crate::{
    catalog::views,
    connect,
    crypto::{b64, random_bytes},
    error::AppResult,
    realtime::{self, Audience},
    state::AppState,
};

/// «Приватная сессия на 6 часов».
pub const PRIVATE_HOURS: i64 = 6;
/// «До выключения»: сессия кончается через сто лет (бесконечность время Rust не принимает).
pub const UNTIL_OFF_DAYS: i64 = 36_500;
/// Окно для совпадения вкусов по артистам.
const MATCH_DAYS: i32 = 180;
/// Меньше прослушиваний у кого-то из двоих — совпадение не считается: цифра была бы случайной.
const MATCH_MIN_PLAYS: i64 = 20;
/// Прослушиванием считается хотя бы столько.
const PLAYED_MS: i32 = 30_000;
/// Косинус центров граней вкуса: ниже — «ничего общего», выше — «то же самое». Центры граней гладкие:
/// даже несвязанные грани похожи на 0,8, поэтому шкала — от него (грани одного человека сливаются на 0,93).
const SOUND_LO: f64 = 0.80;
const SOUND_HI: f64 = 0.95;
/// Общих артистов на странице друга.
const COMMON_ARTISTS: usize = 12;
const TOP_ARTISTS: i64 = 10;

/// Код ссылки: 9 случайных байт → 12 знаков base64url.
pub fn new_code() -> AppResult<String> {
    Ok(b64(random_bytes::<9>()?))
}

pub async fn people(db: &PgPool, ids: &[Uuid]) -> Result<HashMap<Uuid, PersonRef>, sqlx::Error> {
    if ids.is_empty() {
        return Ok(HashMap::new());
    }
    Ok(sqlx::query_as!(
        PersonRef,
        "SELECT id, name, avatar_url FROM users WHERE id = ANY($1)",
        ids
    )
    .fetch_all(db)
    .await?
    .into_iter()
    .map(|p| (p.id, p))
    .collect())
}

/// Человек с доступом к сервису (роль есть, не заблокирован).
pub async fn person(db: &PgPool, id: Uuid) -> Result<Option<PersonRef>, sqlx::Error> {
    sqlx::query_as!(
        PersonRef,
        "SELECT id, name, avatar_url FROM users
         WHERE id = $1 AND NOT disabled AND cardinality(roles) > 0",
        id
    )
    .fetch_optional(db)
    .await
}

pub async fn are_friends(db: impl PgExecutor<'_>, a: Uuid, b: Uuid) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar!(
        r#"SELECT EXISTS (SELECT 1 FROM friendships WHERE user_id = $1 AND friend_id = $2) AS "e!""#,
        a,
        b
    )
    .fetch_one(db)
    .await
}

pub async fn relation(db: &PgPool, me: Uuid, other: Uuid) -> Result<Relation, sqlx::Error> {
    if me == other {
        return Ok(Relation::Me);
    }
    let r = sqlx::query!(
        r#"SELECT EXISTS (SELECT 1 FROM friendships WHERE user_id = $1 AND friend_id = $2) AS "friend!",
                  EXISTS (SELECT 1 FROM friend_requests WHERE from_user = $1 AND to_user = $2) AS "outgoing!",
                  EXISTS (SELECT 1 FROM friend_requests
                          WHERE from_user = $2 AND to_user = $1 AND declined_at IS NULL) AS "incoming!""#,
        me,
        other
    )
    .fetch_one(db)
    .await?;
    Ok(if r.friend {
        Relation::Friend
    } else if r.incoming {
        Relation::Incoming
    } else if r.outgoing {
        Relation::Outgoing
    } else {
        Relation::None
    })
}

pub async fn friend_ids(db: impl PgExecutor<'_>, user: Uuid) -> Result<Vec<Uuid>, sqlx::Error> {
    sqlx::query_scalar!("SELECT friend_id FROM friendships WHERE user_id = $1", user)
        .fetch_all(db)
        .await
}

/// Друзья или запросы изменились — обеим сторонам (в транзакции: после коммита).
pub async fn changed(conn: &mut sqlx::PgConnection, users: &[Uuid], what: &str) {
    for &user in users {
        realtime::publish(
            &mut *conn,
            Audience::User(user),
            ServerEvent::FriendsChanged(FriendsChange { what: what.into() }),
        )
        .await;
    }
}

pub async fn privacy(db: &PgPool, user: Uuid) -> Result<Privacy, sqlx::Error> {
    let r = sqlx::query!(
        r#"SELECT COALESCE((SELECT share_listening FROM social_settings WHERE user_id = $1), true) AS "share!",
                  (SELECT max(ended_at) FROM private_sessions
                   WHERE user_id = $1 AND started_at <= now() AND ended_at > now()) AS "until?""#,
        user
    )
    .fetch_one(db)
    .await?;
    Ok(Privacy {
        share_listening: r.share,
        private_session: r.until.is_some(),
        // «До выключения» хранится концом через сто лет
        private_until: r
            .until
            .filter(|u| *u < Utc::now() + Duration::days(UNTIL_OFF_DAYS / 2)),
    })
}

/// Видно ли друзьям, что человек слушает сейчас.
pub async fn sharing(db: &PgPool, user: Uuid) -> Result<bool, sqlx::Error> {
    let p = privacy(db, user).await?;
    Ok(p.share_listening && !p.private_session)
}

/// Код ссылки «Добавить в друзья»; нет — создаётся. `reset` — новый (старая ссылка перестаёт работать).
pub async fn invite_code(db: &PgPool, user: Uuid, reset: bool) -> AppResult<String> {
    if !reset
        && let Some(code) = sqlx::query_scalar!(
            "SELECT invite_code FROM social_settings WHERE user_id = $1",
            user
        )
        .fetch_optional(db)
        .await?
        .flatten()
    {
        return Ok(code);
    }
    let code = new_code()?;
    sqlx::query!(
        "INSERT INTO social_settings (user_id, invite_code) VALUES ($1, $2)
         ON CONFLICT (user_id) DO UPDATE SET invite_code = $2, updated_at = now()",
        user,
        code
    )
    .execute(db)
    .await?;
    Ok(code)
}

/// Что из «откуда играет» показать другу: альбом и артист — всегда, плейлист — если он его видит
/// (свой, участник, открыт друзьям), миксы и волна — нет.
async fn visible_context(
    db: &PgPool,
    viewer: Uuid,
    kind: ContextType,
    id: &str,
    title: Option<String>,
) -> Result<Option<ActivityContext>, sqlx::Error> {
    let Ok(uuid) = id.parse::<Uuid>() else {
        return Ok(None);
    };
    let title = match kind {
        ContextType::Album => match title {
            Some(t) => Some(t),
            None => {
                sqlx::query_scalar!("SELECT title FROM albums WHERE id = $1", uuid)
                    .fetch_optional(db)
                    .await?
            }
        },
        ContextType::Artist => match title {
            Some(t) => Some(t),
            None => {
                sqlx::query_scalar!("SELECT name FROM artists WHERE id = $1", uuid)
                    .fetch_optional(db)
                    .await?
            }
        },
        ContextType::Playlist => {
            sqlx::query_scalar!(
                "SELECT p.title FROM playlists p
                 WHERE p.id = $1 AND p.kind = 'user'
                   AND (p.user_id = $2
                        OR EXISTS (SELECT 1 FROM playlist_members m WHERE m.playlist_id = p.id AND m.user_id = $2)
                        OR (p.friends_can_see
                            AND EXISTS (SELECT 1 FROM friendships f WHERE f.user_id = $2 AND f.friend_id = p.user_id)))",
                uuid,
                viewer
            )
            .fetch_optional(db)
            .await?
        }
        _ => None,
    };
    Ok(title.map(|title| ActivityContext {
        kind,
        id: id.to_string(),
        title,
    }))
}

/// Открытое «Слушать вместе» человека: код ссылки.
async fn jam_code(db: &PgPool, user: Uuid) -> Result<Option<String>, sqlx::Error> {
    sqlx::query_scalar!(
        "SELECT code FROM jam_sessions
         WHERE user_id = $1 AND ended_at IS NULL AND last_activity_at > now() - make_interval(hours => $2)",
        user,
        IDLE_HOURS as i32
    )
    .fetch_optional(db)
    .await
}

/// Играет прямо сейчас: снимок «играет» от активного устройства, которое на связи.
async fn now_playing(db: &PgPool, user: Uuid) -> AppResult<Option<PlaybackState>> {
    let (active, state) = connect::playback(db, user).await?;
    let (Some(active), Some(state)) = (active, state) else {
        return Ok(None);
    };
    if !state.playing || state.current.is_none() {
        return Ok(None);
    }
    let online = sqlx::query_scalar!(
        r#"SELECT EXISTS (SELECT 1 FROM connect_devices
                          WHERE user_id = $1 AND id = $2 AND connection IS NOT NULL
                            AND last_seen_at > now() - make_interval(secs => $3)) AS "e!""#,
        user,
        active,
        ONLINE_WINDOW_SECS as f64
    )
    .fetch_one(db)
    .await?;
    Ok(online.then_some(state))
}

/// Что слушает (или слушал последним) `user` — глазами `viewer`. `None` — скрывает или ничего.
pub async fn activity(db: &PgPool, viewer: Uuid, user: Uuid) -> AppResult<Option<FriendActivity>> {
    if !sharing(db, user).await? {
        return Ok(None);
    }
    let jam = jam_code(db, user).await?;
    if let Some(state) = now_playing(db, user).await?
        && let Some(current) = &state.current
        && let Some(track) = views::track(db, current.track.id, viewer).await?
    {
        let now = Utc::now();
        let at = now - Duration::milliseconds(i64::from(state.position_at(now)));
        let context = match &state.context {
            Some(c) => match &c.id {
                Some(id) => visible_context(db, viewer, c.kind, id, Some(c.title.clone())).await?,
                None => None,
            },
            None => None,
        };
        return Ok(Some(FriendActivity {
            track,
            now: true,
            at,
            context,
            jam_code: jam,
        }));
    }
    let last = sqlx::query!(
        "SELECT track_id, started_at, context_type, context_id FROM play_events
         WHERE user_id = $1 AND played_ms >= $2 AND NOT played_privately(user_id, started_at)
         ORDER BY started_at DESC LIMIT 1",
        user,
        PLAYED_MS
    )
    .fetch_optional(db)
    .await?;
    let Some(last) = last else {
        return Ok(None);
    };
    let Some(track) = views::track(db, last.track_id, viewer).await? else {
        return Ok(None);
    };
    let context = match (
        last.context_type
            .and_then(|t| t.parse::<ContextType>().ok()),
        last.context_id,
    ) {
        (Some(kind), Some(id)) => visible_context(db, viewer, kind, &id, None).await?,
        _ => None,
    };
    Ok(Some(FriendActivity {
        track,
        now: false,
        at: last.started_at,
        context,
        jam_code: jam,
    }))
}

/// Человек начал слушать другое (или перестал, или скрыл): его друзьям — живое событие. Друзей
/// немного, у каждого свой взгляд (лайки, видимые плейлисты), поэтому считается на каждого.
pub async fn announce_activity(st: &AppState, user: Uuid) {
    let run = async {
        for friend in friend_ids(&st.db, user).await? {
            let activity = activity(&st.db, friend, user).await?;
            realtime::deliver(
                &st.hub,
                Audience::User(friend),
                ServerEvent::FriendActivity(FriendActivityEvent {
                    user_id: user,
                    activity,
                }),
            );
        }
        Ok::<_, crate::error::AppError>(())
    };
    if let Err(e) = run.await {
        tracing::warn!(error = %e, "friend activity announce failed");
    }
}

/// Доли прослушиваний по главным артистам за полгода (без приватных сессий) и сколько всего.
async fn artist_shares(db: &PgPool, user: Uuid) -> Result<(HashMap<Uuid, f64>, i64), sqlx::Error> {
    let rows = sqlx::query!(
        r#"SELECT ta.artist_id, count(*) AS "n!"
           FROM play_events e
           JOIN track_artists ta ON ta.track_id = e.track_id AND ta.role = 'main'
           WHERE e.user_id = $1 AND e.started_at > now() - make_interval(days => $2)
             AND e.played_ms >= $3 AND NOT played_privately(e.user_id, e.started_at)
           GROUP BY ta.artist_id"#,
        user,
        MATCH_DAYS,
        PLAYED_MS
    )
    .fetch_all(db)
    .await?;
    let plays = sqlx::query_scalar!(
        r#"SELECT count(*) AS "n!" FROM play_events e
           WHERE e.user_id = $1 AND e.started_at > now() - make_interval(days => $2)
             AND e.played_ms >= $3 AND NOT played_privately(e.user_id, e.started_at)"#,
        user,
        MATCH_DAYS,
        PLAYED_MS
    )
    .fetch_one(db)
    .await?;
    let total: i64 = rows.iter().map(|r| r.n).sum();
    let shares = rows
        .into_iter()
        .map(|r| (r.artist_id, r.n as f64 / total.max(1) as f64))
        .collect();
    Ok((shares, plays))
}

/// Пересечение двух распределений: сумма меньших долей (1 — одинаковые пропорции, 0 — ничего общего).
pub fn overlap(a: &HashMap<Uuid, f64>, b: &HashMap<Uuid, f64>) -> f64 {
    a.iter()
        .filter_map(|(id, pa)| b.get(id).map(|pb| pa.min(*pb)))
        .sum()
}

/// Косинус центров граней → 0…1 по шкале [`SOUND_LO`]…[`SOUND_HI`].
pub fn sound_scale(cos: f64) -> f64 {
    ((cos - SOUND_LO) / (SOUND_HI - SOUND_LO)).clamp(0.0, 1.0)
}

/// Средняя (по весу граней `a`) близость каждой грани `a` к ближайшей грани `b`.
async fn facet_closeness(db: &PgPool, a: Uuid, b: Uuid) -> Result<Option<f64>, sqlx::Error> {
    let rows = sqlx::query!(
        r#"SELECT fa.weight AS "weight!", max(1 - (fa.centroid <=> fb.centroid)) AS "best!"
           FROM taste_facets fa CROSS JOIN taste_facets fb
           WHERE fa.user_id = $1 AND fb.user_id = $2
           GROUP BY fa.id, fa.weight"#,
        a,
        b
    )
    .fetch_all(db)
    .await?;
    let weight: f64 = rows.iter().map(|r| f64::from(r.weight)).sum();
    if rows.is_empty() || weight <= 0.0 {
        return Ok(None);
    }
    Ok(Some(
        rows.iter()
            .map(|r| f64::from(r.weight) * sound_scale(r.best))
            .sum::<f64>()
            / weight,
    ))
}

/// Совпадение вкусов и общие артисты (самые общие первыми).
pub async fn taste_match(
    db: &PgPool,
    me: Uuid,
    other: Uuid,
) -> Result<(Option<TasteMatch>, Vec<Uuid>), sqlx::Error> {
    let (mine, my_plays) = artist_shares(db, me).await?;
    let (theirs, their_plays) = artist_shares(db, other).await?;
    let mut common: Vec<(Uuid, f64)> = mine
        .iter()
        .filter_map(|(id, a)| theirs.get(id).map(|b| (*id, a.min(*b))))
        .collect();
    common.sort_by(|x, y| y.1.total_cmp(&x.1).then(x.0.cmp(&y.0)));
    let common: Vec<Uuid> = common
        .into_iter()
        .take(COMMON_ARTISTS)
        .map(|c| c.0)
        .collect();
    let artists = (my_plays >= MATCH_MIN_PLAYS && their_plays >= MATCH_MIN_PLAYS)
        .then(|| overlap(&mine, &theirs));
    let sound = match (
        facet_closeness(db, me, other).await?,
        facet_closeness(db, other, me).await?,
    ) {
        (Some(a), Some(b)) => Some((a + b) / 2.0),
        _ => None,
    };
    let parts: Vec<f64> = artists.into_iter().chain(sound).collect();
    let result = (!parts.is_empty()).then(|| TasteMatch {
        percent: (100.0 * parts.iter().sum::<f64>() / parts.len() as f64).round() as i32,
        artists: artists.map(|v| v as f32),
        sound: sound.map(|v| v as f32),
    });
    Ok((result, common))
}

/// Топ артистов за 30 дней (без приватных сессий).
pub async fn top_artists(
    db: &PgPool,
    viewer: Uuid,
    user: Uuid,
) -> Result<Vec<FriendTopArtist>, sqlx::Error> {
    let rows = sqlx::query!(
        r#"SELECT ta.artist_id, count(*) AS "n!"
           FROM play_events e
           JOIN track_artists ta ON ta.track_id = e.track_id AND ta.role = 'main'
           WHERE e.user_id = $1 AND e.started_at > now() - interval '30 days'
             AND e.played_ms >= $2 AND NOT played_privately(e.user_id, e.started_at)
           GROUP BY ta.artist_id ORDER BY count(*) DESC, ta.artist_id LIMIT $3"#,
        user,
        PLAYED_MS,
        TOP_ARTISTS
    )
    .fetch_all(db)
    .await?;
    let ids: Vec<Uuid> = rows.iter().map(|r| r.artist_id).collect();
    let plays: HashMap<Uuid, i64> = rows.iter().map(|r| (r.artist_id, r.n)).collect();
    Ok(views::artists(db, &ids, viewer)
        .await?
        .into_iter()
        .map(|artist| FriendTopArtist {
            plays: plays.get(&artist.id).copied().unwrap_or(0),
            artist,
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overlap_of_distributions() {
        let (x, y, z) = (Uuid::now_v7(), Uuid::now_v7(), Uuid::now_v7());
        let a = HashMap::from([(x, 0.5), (y, 0.5)]);
        let b = HashMap::from([(x, 0.5), (y, 0.5)]);
        assert!((overlap(&a, &b) - 1.0).abs() < 1e-9);
        let c = HashMap::from([(x, 0.2), (z, 0.8)]);
        assert!((overlap(&a, &c) - 0.2).abs() < 1e-9);
        assert_eq!(overlap(&a, &HashMap::from([(z, 1.0)])), 0.0);
    }

    #[test]
    fn sound_scale_clamps() {
        assert_eq!(sound_scale(0.5), 0.0);
        assert_eq!(sound_scale(0.99), 1.0);
        assert!((sound_scale(0.875) - 0.5).abs() < 1e-9);
    }
}
