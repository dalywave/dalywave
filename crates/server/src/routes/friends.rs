//! Друзья: поиск людей сервиса, запросы, ссылка «Добавить в друзья», что слушают друзья, страница
//! друга и приватность. Логика — [`crate::social`].

use axum::{
    Router,
    extract::State,
    http::StatusCode,
    routing::{delete, get, post},
};
use chrono::{Duration, Utc};
use dalywave_protocol::{
    ErrorBody,
    notifications::NotificationKind,
    social::{
        Friend, FriendList, FriendProfile, FriendRequest, FriendRequestResult, InviteLink,
        NewFriendRequest, PeopleList, PeopleQuery, PersonCard, PersonRef, Privacy, PrivacyPatch,
        PrivateSessionMode, Relation,
    },
};
use sqlx::PgConnection;
use uuid::Uuid;

use super::playlists::{Scope, summaries_in};
use crate::{
    auth::Principal,
    catalog::views,
    error::{AppError, AppResult},
    extract::{Json, Path, Query},
    notify::{self, Draft},
    social::{self, PRIVATE_HOURS, UNTIL_OFF_DAYS},
    state::AppState,
};

/// Людей в поиске — не больше.
const PEOPLE_LIMIT: i64 = 50;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/friends", get(list))
        .route("/friends/people", get(people))
        .route("/friends/requests", post(request))
        .route("/friends/requests/{userId}", delete(cancel_request))
        .route("/friends/requests/{userId}/accept", post(accept))
        .route("/friends/requests/{userId}/decline", post(decline))
        .route("/friends/invite", get(invite).post(reset_invite))
        .route(
            "/friends/invite/{code}",
            get(invite_preview).post(use_invite),
        )
        .route("/friends/privacy", get(get_privacy).patch(update_privacy))
        .route("/friends/{userId}", get(profile).delete(unfriend))
}

fn invite_url(st: &AppState, code: &str) -> String {
    format!("{}/friends/add/{code}", st.config.public_url)
}

/// Месяц события: повторный запрос в тот же месяц не тревожит человека второй раз.
fn month() -> String {
    Utc::now().format("%Y-%m").to_string()
}

/// Дружба в обе стороны; запросы между ними больше не нужны.
async fn befriend(conn: &mut PgConnection, a: Uuid, b: Uuid) -> Result<bool, sqlx::Error> {
    let fresh = sqlx::query!(
        "INSERT INTO friendships (user_id, friend_id) VALUES ($1, $2), ($2, $1) ON CONFLICT DO NOTHING",
        a,
        b
    )
    .execute(&mut *conn)
    .await?
    .rows_affected()
        > 0;
    sqlx::query!(
        "DELETE FROM friend_requests WHERE (from_user = $1 AND to_user = $2) OR (from_user = $2 AND to_user = $1)",
        a,
        b
    )
    .execute(&mut *conn)
    .await?;
    Ok(fresh)
}

/// «X принял запрос» — тому, кто звал.
async fn notify_accepted(
    conn: &mut PgConnection,
    by: &PersonRef,
    to: Uuid,
) -> Result<(), sqlx::Error> {
    notify::emit_in(
        conn,
        to,
        Draft::new(
            NotificationKind::Friends,
            format!("{} теперь в друзьях", by.name),
            "Видно, что слушает, и можно слушать вместе",
        )
        .link(format!("/friends/{}", by.id))
        .image(by.avatar_url.clone())
        .once(format!("friend_ok:{}:{}", by.id, month())),
    )
    .await?;
    Ok(())
}

fn me_ref(principal: &Principal) -> PersonRef {
    PersonRef {
        id: principal.user.id,
        name: principal.user.name.clone(),
        avatar_url: principal.user.avatar_url.clone(),
    }
}

#[utoipa::path(
    get,
    path = "/api/friends",
    tag = "friends",
    security(("session" = []), ("device" = [])),
    responses((status = 200, description = "Друзья (слушающие сейчас — первыми) и запросы", body = FriendList)),
)]
pub async fn list(State(st): State<AppState>, principal: Principal) -> AppResult<Json<FriendList>> {
    let me = principal.user.id;
    let rows = sqlx::query!(
        "SELECT u.id, u.name, u.avatar_url, f.created_at FROM friendships f
         JOIN users u ON u.id = f.friend_id
         WHERE f.user_id = $1 AND NOT u.disabled ORDER BY u.name, u.id",
        me
    )
    .fetch_all(&st.db)
    .await?;
    let mut friends = Vec::with_capacity(rows.len());
    for r in rows {
        friends.push(Friend {
            activity: social::activity(&st.db, me, r.id).await?,
            person: PersonRef {
                id: r.id,
                name: r.name,
                avatar_url: r.avatar_url,
            },
            since: r.created_at,
        });
    }
    // Слушают сейчас — первыми, затем по свежести последнего
    friends.sort_by(|a, b| {
        let key = |f: &Friend| f.activity.as_ref().map(|x| (x.now, x.at));
        key(b).cmp(&key(a))
    });
    let requests = sqlx::query!(
        r#"SELECT r.from_user, r.to_user, r.created_at, u.id, u.name, u.avatar_url
           FROM friend_requests r
           JOIN users u ON u.id = CASE WHEN r.from_user = $1 THEN r.to_user ELSE r.from_user END
           WHERE (r.from_user = $1 OR (r.to_user = $1 AND r.declined_at IS NULL)) AND NOT u.disabled
           ORDER BY r.created_at DESC"#,
        me
    )
    .fetch_all(&st.db)
    .await?;
    let (mut incoming, mut outgoing) = (Vec::new(), Vec::new());
    for r in requests {
        let req = FriendRequest {
            person: PersonRef {
                id: r.id,
                name: r.name,
                avatar_url: r.avatar_url,
            },
            created_at: r.created_at,
        };
        if r.from_user == me {
            outgoing.push(req);
        } else {
            incoming.push(req);
        }
    }
    Ok(Json(FriendList {
        friends,
        incoming,
        outgoing,
    }))
}

#[utoipa::path(
    get,
    path = "/api/friends/people",
    tag = "friends",
    security(("session" = []), ("device" = [])),
    params(PeopleQuery),
    responses((status = 200, description = "Люди сервиса (с доступом) по имени и кем приходятся", body = PeopleList)),
)]
pub async fn people(
    State(st): State<AppState>,
    principal: Principal,
    Query(q): Query<PeopleQuery>,
) -> AppResult<Json<PeopleList>> {
    let me = principal.user.id;
    let q = q.q.as_deref().map(str::trim).unwrap_or("");
    let rows = sqlx::query!(
        r#"SELECT u.id, u.name, u.avatar_url,
                  EXISTS (SELECT 1 FROM friendships f WHERE f.user_id = $1 AND f.friend_id = u.id) AS "friend!",
                  EXISTS (SELECT 1 FROM friend_requests r WHERE r.from_user = $1 AND r.to_user = u.id) AS "outgoing!",
                  EXISTS (SELECT 1 FROM friend_requests r
                          WHERE r.from_user = u.id AND r.to_user = $1 AND r.declined_at IS NULL) AS "incoming!"
           FROM users u
           WHERE u.id <> $1 AND NOT u.disabled AND cardinality(u.roles) > 0 AND u.name <> ''
             AND ($2 = '' OR search_norm(u.name) LIKE '%' || search_norm($2) || '%')
           ORDER BY (search_norm(u.name) LIKE search_norm($2) || '%') DESC, u.name, u.id
           LIMIT $3"#,
        me,
        q,
        PEOPLE_LIMIT
    )
    .fetch_all(&st.db)
    .await?;
    Ok(Json(PeopleList {
        items: rows
            .into_iter()
            .map(|r| PersonCard {
                relation: if r.friend {
                    Relation::Friend
                } else if r.incoming {
                    Relation::Incoming
                } else if r.outgoing {
                    Relation::Outgoing
                } else {
                    Relation::None
                },
                person: PersonRef {
                    id: r.id,
                    name: r.name,
                    avatar_url: r.avatar_url,
                },
            })
            .collect(),
    }))
}

#[utoipa::path(
    post,
    path = "/api/friends/requests",
    tag = "friends",
    security(("session" = []), ("device" = [])),
    request_body = NewFriendRequest,
    responses(
        (status = 200, description = "Запрос отправлен (`outgoing`) или встречный уже был — друзья (`friend`)", body = FriendRequestResult),
        (status = 400, description = "Себе запрос не отправить", body = ErrorBody),
        (status = 404, description = "Нет такого человека", body = ErrorBody),
    ),
)]
pub async fn request(
    State(st): State<AppState>,
    principal: Principal,
    Json(body): Json<NewFriendRequest>,
) -> AppResult<Json<FriendRequestResult>> {
    let me = principal.user.id;
    let to = body.user_id;
    if to == me {
        return Err(AppError::BadRequest("себе запрос не отправить".into()));
    }
    social::person(&st.db, to)
        .await?
        .ok_or(AppError::NotFound)?;
    // Он уже звал (даже если тогда отклонили): запрос в ответ — это согласие
    let asked = sqlx::query_scalar!(
        r#"SELECT EXISTS (SELECT 1 FROM friend_requests WHERE from_user = $1 AND to_user = $2) AS "e!""#,
        to,
        me
    )
    .fetch_one(&st.db)
    .await?;
    match social::relation(&st.db, me, to).await? {
        Relation::Friend => {
            return Ok(Json(FriendRequestResult {
                relation: Relation::Friend,
            }));
        }
        _ if asked => {
            let mut tx = st.db.begin().await?;
            befriend(&mut tx, me, to).await?;
            notify_accepted(&mut tx, &me_ref(&principal), to).await?;
            social::changed(&mut tx, &[me, to], "friends").await;
            tx.commit().await?;
            return Ok(Json(FriendRequestResult {
                relation: Relation::Friend,
            }));
        }
        _ => {}
    }
    let mut tx = st.db.begin().await?;
    let fresh = sqlx::query!(
        "INSERT INTO friend_requests (from_user, to_user) VALUES ($1, $2) ON CONFLICT DO NOTHING",
        me,
        to
    )
    .execute(&mut *tx)
    .await?
    .rows_affected()
        > 0;
    // Отклонённый запрос повтором не оживает: человек уже ответил
    if fresh {
        notify::emit_in(
            &mut tx,
            to,
            Draft::new(
                NotificationKind::Friends,
                format!("{} хочет добавить вас в друзья", principal.user.name),
                "Друзья видят, что слушает каждый, и могут слушать вместе",
            )
            .link("/friends")
            .image(principal.user.avatar_url.clone())
            .once(format!("friend_req:{me}:{}", month())),
        )
        .await?;
        social::changed(&mut tx, &[me, to], "requests").await;
    }
    tx.commit().await?;
    Ok(Json(FriendRequestResult {
        relation: Relation::Outgoing,
    }))
}

#[utoipa::path(
    delete,
    path = "/api/friends/requests/{userId}",
    tag = "friends",
    security(("session" = []), ("device" = [])),
    params(("userId" = Uuid, Path, description = "Кому отправлен запрос")),
    responses((status = 204, description = "Запрос отменён (если был)")),
)]
pub async fn cancel_request(
    State(st): State<AppState>,
    principal: Principal,
    Path(to): Path<Uuid>,
) -> AppResult<StatusCode> {
    let me = principal.user.id;
    let mut tx = st.db.begin().await?;
    let gone = sqlx::query!(
        "DELETE FROM friend_requests WHERE from_user = $1 AND to_user = $2",
        me,
        to
    )
    .execute(&mut *tx)
    .await?
    .rows_affected();
    if gone > 0 {
        social::changed(&mut tx, &[me, to], "requests").await;
    }
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    post,
    path = "/api/friends/requests/{userId}/accept",
    tag = "friends",
    security(("session" = []), ("device" = [])),
    params(("userId" = Uuid, Path, description = "Кто прислал запрос")),
    responses(
        (status = 200, description = "Теперь друзья", body = FriendRequestResult),
        (status = 404, description = "Запроса нет", body = ErrorBody),
    ),
)]
pub async fn accept(
    State(st): State<AppState>,
    principal: Principal,
    Path(from): Path<Uuid>,
) -> AppResult<Json<FriendRequestResult>> {
    let me = principal.user.id;
    let mut tx = st.db.begin().await?;
    // Отклонённый раньше тоже можно принять — передумал
    let exists = sqlx::query_scalar!(
        r#"SELECT EXISTS (SELECT 1 FROM friend_requests WHERE from_user = $1 AND to_user = $2) AS "e!""#,
        from,
        me
    )
    .fetch_one(&mut *tx)
    .await?;
    if !exists {
        if social::are_friends(&mut *tx, me, from).await? {
            return Ok(Json(FriendRequestResult {
                relation: Relation::Friend,
            }));
        }
        return Err(AppError::NotFound);
    }
    befriend(&mut tx, me, from).await?;
    notify_accepted(&mut tx, &me_ref(&principal), from).await?;
    social::changed(&mut tx, &[me, from], "friends").await;
    tx.commit().await?;
    Ok(Json(FriendRequestResult {
        relation: Relation::Friend,
    }))
}

#[utoipa::path(
    post,
    path = "/api/friends/requests/{userId}/decline",
    tag = "friends",
    security(("session" = []), ("device" = [])),
    params(("userId" = Uuid, Path, description = "Кто прислал запрос")),
    responses((status = 204, description = "Запрос отклонён: отправитель об этом не узнает, повтор не тревожит")),
)]
pub async fn decline(
    State(st): State<AppState>,
    principal: Principal,
    Path(from): Path<Uuid>,
) -> AppResult<StatusCode> {
    let me = principal.user.id;
    let mut tx = st.db.begin().await?;
    sqlx::query!(
        "UPDATE friend_requests SET declined_at = now()
         WHERE from_user = $1 AND to_user = $2 AND declined_at IS NULL",
        from,
        me
    )
    .execute(&mut *tx)
    .await?;
    social::changed(&mut tx, &[me], "requests").await;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    delete,
    path = "/api/friends/{userId}",
    tag = "friends",
    security(("session" = []), ("device" = [])),
    params(("userId" = Uuid, Path, description = "Друг")),
    responses((status = 204, description = "Больше не друзья (с обеих сторон)")),
)]
pub async fn unfriend(
    State(st): State<AppState>,
    principal: Principal,
    Path(other): Path<Uuid>,
) -> AppResult<StatusCode> {
    let me = principal.user.id;
    let mut tx = st.db.begin().await?;
    let gone = sqlx::query!(
        "DELETE FROM friendships WHERE (user_id = $1 AND friend_id = $2) OR (user_id = $2 AND friend_id = $1)",
        me,
        other
    )
    .execute(&mut *tx)
    .await?
    .rows_affected();
    if gone > 0 {
        social::changed(&mut tx, &[me, other], "friends").await;
    }
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    get,
    path = "/api/friends/{userId}",
    tag = "friends",
    security(("session" = []), ("device" = [])),
    params(("userId" = Uuid, Path, description = "Человек")),
    responses(
        (status = 200, description = "Страница друга: что слушает, открытые плейлисты, топ за месяц, совпадение вкусов. Не другу — только имя и кем приходится", body = FriendProfile),
        (status = 404, description = "Нет такого человека", body = ErrorBody),
    ),
)]
pub async fn profile(
    State(st): State<AppState>,
    principal: Principal,
    Path(other): Path<Uuid>,
) -> AppResult<Json<FriendProfile>> {
    let me = principal.user.id;
    let person = social::person(&st.db, other)
        .await?
        .ok_or(AppError::NotFound)?;
    let relation = social::relation(&st.db, me, other).await?;
    let mut out = FriendProfile {
        person,
        relation,
        since: None,
        activity: None,
        playlists: Vec::new(),
        top_artists: Vec::new(),
        taste_match: None,
        common_artists: Vec::new(),
    };
    if relation != Relation::Friend {
        return Ok(Json(out));
    }
    out.since = sqlx::query_scalar!(
        "SELECT created_at FROM friendships WHERE user_id = $1 AND friend_id = $2",
        me,
        other
    )
    .fetch_optional(&st.db)
    .await?;
    out.activity = social::activity(&st.db, me, other).await?;
    out.playlists = summaries_in(&st.db, me, Scope::Friend(other), None, false).await?;
    // То, что скрыто, скрыто и в топе: «показывать друзьям» выключено — топа нет
    if social::privacy(&st.db, other).await?.share_listening {
        out.top_artists = social::top_artists(&st.db, me, other).await?;
    }
    let (taste, common) = social::taste_match(&st.db, me, other).await?;
    out.taste_match = taste;
    out.common_artists = views::artists(&st.db, &common, me).await?;
    Ok(Json(out))
}

#[utoipa::path(
    get,
    path = "/api/friends/invite",
    tag = "friends",
    security(("session" = []), ("device" = [])),
    responses((status = 200, description = "Своя ссылка «Добавить в друзья»", body = InviteLink)),
)]
pub async fn invite(
    State(st): State<AppState>,
    principal: Principal,
) -> AppResult<Json<InviteLink>> {
    let code = social::invite_code(&st.db, principal.user.id, false).await?;
    Ok(Json(InviteLink {
        url: invite_url(&st, &code),
        code,
    }))
}

#[utoipa::path(
    post,
    path = "/api/friends/invite",
    tag = "friends",
    security(("session" = []), ("device" = [])),
    responses((status = 200, description = "Новая ссылка; прежняя больше не работает", body = InviteLink)),
)]
pub async fn reset_invite(
    State(st): State<AppState>,
    principal: Principal,
) -> AppResult<Json<InviteLink>> {
    let code = social::invite_code(&st.db, principal.user.id, true).await?;
    Ok(Json(InviteLink {
        url: invite_url(&st, &code),
        code,
    }))
}

async fn by_code(st: &AppState, code: &str) -> AppResult<PersonRef> {
    let id = sqlx::query_scalar!(
        "SELECT user_id FROM social_settings WHERE invite_code = $1",
        code
    )
    .fetch_optional(&st.db)
    .await?
    .ok_or(AppError::Missing {
        code: "invite_expired",
        message: "Ссылка больше не работает — попросите новую".into(),
    })?;
    social::person(&st.db, id).await?.ok_or(AppError::NotFound)
}

#[utoipa::path(
    get,
    path = "/api/friends/invite/{code}",
    tag = "friends",
    security(("session" = []), ("device" = [])),
    params(("code" = String, Path, description = "Код из ссылки")),
    responses(
        (status = 200, description = "Кто зовёт в друзья и кем уже приходится", body = PersonCard),
        (status = 404, description = "Ссылка больше не работает (`invite_expired`)", body = ErrorBody),
    ),
)]
pub async fn invite_preview(
    State(st): State<AppState>,
    principal: Principal,
    Path(code): Path<String>,
) -> AppResult<Json<PersonCard>> {
    let person = by_code(&st, &code).await?;
    let relation = social::relation(&st.db, principal.user.id, person.id).await?;
    Ok(Json(PersonCard { person, relation }))
}

#[utoipa::path(
    post,
    path = "/api/friends/invite/{code}",
    tag = "friends",
    security(("session" = []), ("device" = [])),
    params(("code" = String, Path, description = "Код из ссылки")),
    responses(
        (status = 200, description = "Теперь друзья: ссылку дал сам человек", body = PersonCard),
        (status = 404, description = "Ссылка больше не работает (`invite_expired`)", body = ErrorBody),
    ),
)]
pub async fn use_invite(
    State(st): State<AppState>,
    principal: Principal,
    Path(code): Path<String>,
) -> AppResult<Json<PersonCard>> {
    let me = principal.user.id;
    let person = by_code(&st, &code).await?;
    if person.id == me {
        return Ok(Json(PersonCard {
            person,
            relation: Relation::Me,
        }));
    }
    let mut tx = st.db.begin().await?;
    if befriend(&mut tx, me, person.id).await? {
        notify_accepted(&mut tx, &me_ref(&principal), person.id).await?;
        social::changed(&mut tx, &[me, person.id], "friends").await;
    }
    tx.commit().await?;
    Ok(Json(PersonCard {
        person,
        relation: Relation::Friend,
    }))
}

#[utoipa::path(
    get,
    path = "/api/friends/privacy",
    tag = "friends",
    security(("session" = []), ("device" = [])),
    responses((status = 200, description = "Показывать ли друзьям, что я слушаю, и идёт ли приватная сессия", body = Privacy)),
)]
pub async fn get_privacy(
    State(st): State<AppState>,
    principal: Principal,
) -> AppResult<Json<Privacy>> {
    Ok(Json(social::privacy(&st.db, principal.user.id).await?))
}

#[utoipa::path(
    patch,
    path = "/api/friends/privacy",
    tag = "friends",
    security(("session" = []), ("device" = [])),
    request_body = PrivacyPatch,
    responses((status = 200, description = "Приватность после изменения", body = Privacy)),
)]
pub async fn update_privacy(
    State(st): State<AppState>,
    principal: Principal,
    Json(patch): Json<PrivacyPatch>,
) -> AppResult<Json<Privacy>> {
    let me = principal.user.id;
    let mut tx = st.db.begin().await?;
    if let Some(share) = patch.share_listening {
        sqlx::query!(
            "INSERT INTO social_settings (user_id, share_listening) VALUES ($1, $2)
             ON CONFLICT (user_id) DO UPDATE SET share_listening = $2, updated_at = now()",
            me,
            share
        )
        .execute(&mut *tx)
        .await?;
    }
    if let Some(mode) = patch.private_session {
        // Идущая сессия заканчивается сейчас; новая начинается с этой минуты
        sqlx::query!(
            "UPDATE private_sessions SET ended_at = greatest(started_at, now())
             WHERE user_id = $1 AND ended_at > now()",
            me
        )
        .execute(&mut *tx)
        .await?;
        let until = match mode {
            PrivateSessionMode::Off => None,
            PrivateSessionMode::Hours6 => Some(Utc::now() + Duration::hours(PRIVATE_HOURS)),
            PrivateSessionMode::UntilOff => Some(Utc::now() + Duration::days(UNTIL_OFF_DAYS)),
        };
        if let Some(until) = until {
            sqlx::query!(
                "INSERT INTO private_sessions (user_id, ended_at) VALUES ($1, $2)",
                me,
                until
            )
            .execute(&mut *tx)
            .await?;
        }
    }
    social::changed(&mut tx, &[me], "privacy").await;
    tx.commit().await?;
    // Друзья сразу видят перемену: пропало «сейчас играет» или появилось
    social::announce_activity(&st, me).await;
    Ok(Json(social::privacy(&st.db, me).await?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn month_key_is_short() {
        assert_eq!(month().len(), 7);
    }
}
