//! События прослушивания и реакции, история. Приложения шлют пачки, накопленные и без сети;
//! повтор по `clientEventId` не создаёт вторую запись.

use std::collections::{HashMap, HashSet};

use axum::{
    Router,
    extract::State,
    routing::{get, post},
};
use chrono::{DateTime, Duration, Utc};
use dalywave_protocol::{
    ErrorBody,
    events::{EventBatch, EventResult, HistoryItem, HistoryPage, RejectedEvent},
    library::PageQuery,
};
use uuid::Uuid;

use crate::{
    auth::Principal,
    catalog::views,
    cursor,
    error::{AppResult, bad},
    extract::{Json, Query},
    state::AppState,
};

pub const MAX_BATCH: usize = 500;
/// Пауза длиннее — новая сессия прослушивания.
const SESSION_GAP_MIN: i64 = 30;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/events", post(ingest))
        .route("/me/history", get(history))
}

fn reject(out: &mut EventResult, id: Uuid, reason: &str) {
    out.rejected.push(RejectedEvent {
        client_event_id: id,
        reason: reason.into(),
    });
}

#[utoipa::path(
    post,
    path = "/api/events",
    tag = "events",
    security(("session" = []), ("device" = [])),
    request_body = EventBatch,
    responses(
        (status = 200, description = "Сколько принято, сколько уже было, что отклонено и почему", body = EventResult),
        (status = 400, description = "Больше 500 событий одного вида", body = ErrorBody),
    ),
)]
pub async fn ingest(
    State(st): State<AppState>,
    principal: Principal,
    Json(batch): Json<EventBatch>,
) -> AppResult<Json<EventResult>> {
    if batch.plays.len() > MAX_BATCH || batch.feedback.len() > MAX_BATCH {
        return Err(bad(format!(
            "не больше {MAX_BATCH} событий каждого вида за раз"
        )));
    }
    let user = principal.user.id;
    let now = Utc::now();
    let mut out = EventResult::default();

    let track_ids: Vec<Uuid> = batch
        .plays
        .iter()
        .map(|p| p.track_id)
        .chain(batch.feedback.iter().filter_map(|f| f.track_id))
        .collect();
    let artist_ids: Vec<Uuid> = batch.feedback.iter().filter_map(|f| f.artist_id).collect();
    let known_tracks: HashSet<Uuid> =
        sqlx::query_scalar!("SELECT id FROM tracks WHERE id = ANY($1)", &track_ids)
            .fetch_all(&st.db)
            .await?
            .into_iter()
            .collect();
    let known_artists: HashSet<Uuid> =
        sqlx::query_scalar!("SELECT id FROM artists WHERE id = ANY($1)", &artist_ids)
            .fetch_all(&st.db)
            .await?
            .into_iter()
            .collect();

    let mut plays = batch.plays;
    // По времени: так сессия прослушивания считается по соседним событиям, даже если пачка перемешана
    plays.sort_by_key(|p| p.started_at);
    let mut tx = st.db.begin().await?;
    for p in plays {
        let valid = p.played_ms >= 0
            && p.duration_ms > 0
            && p.position_start_ms >= 0
            && p.started_at <= now + Duration::days(1)
            && p.context_id.as_ref().is_none_or(|c| c.len() <= 200);
        if !valid {
            reject(&mut out, p.client_event_id, "invalid");
            continue;
        }
        if !known_tracks.contains(&p.track_id) {
            reject(&mut out, p.client_event_id, "unknown_track");
            continue;
        }
        let previous = sqlx::query!(
            "SELECT started_at, played_ms, listening_session FROM play_events
             WHERE user_id = $1 AND started_at <= $2
             ORDER BY started_at DESC LIMIT 1",
            user,
            p.started_at,
        )
        .fetch_optional(&mut *tx)
        .await?;
        let session = match previous {
            Some(prev)
                if prev.listening_session.is_some()
                    && p.started_at
                        - (prev.started_at + Duration::milliseconds(i64::from(prev.played_ms)))
                        < Duration::minutes(SESSION_GAP_MIN) =>
            {
                prev.listening_session
            }
            _ => Some(Uuid::now_v7()),
        };
        let inserted = sqlx::query_scalar!(
            "INSERT INTO play_events (client_event_id, user_id, device_id, track_id, started_at, played_ms,
                                      position_start_ms, duration_ms, end_reason, context_type, context_id,
                                      wave_session_id, listening_session, source, transition, output)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16)
             ON CONFLICT (client_event_id) DO NOTHING
             RETURNING id",
            p.client_event_id,
            user,
            principal.device_id,
            p.track_id,
            p.started_at,
            p.played_ms,
            p.position_start_ms,
            p.duration_ms,
            p.end_reason.as_str(),
            p.context_type.map(|c| c.as_str()),
            p.context_id,
            p.wave_session_id,
            session,
            p.source.as_str(),
            p.transition.map(|t| t.chars().take(32).collect::<String>()),
            p.output.map(|o| o.chars().take(32).collect::<String>()),
        )
        .fetch_optional(&mut *tx)
        .await?;
        match inserted {
            Some(_) => out.accepted += 1,
            None => out.duplicates += 1,
        }
    }
    for f in batch.feedback {
        if f.track_id.is_none() && f.artist_id.is_none() {
            reject(&mut out, f.client_event_id, "invalid");
            continue;
        }
        if f.track_id.is_some_and(|t| !known_tracks.contains(&t)) {
            reject(&mut out, f.client_event_id, "unknown_track");
            continue;
        }
        if f.artist_id.is_some_and(|a| !known_artists.contains(&a)) {
            reject(&mut out, f.client_event_id, "unknown_artist");
            continue;
        }
        let inserted = sqlx::query_scalar!(
            "INSERT INTO feedback_events (client_event_id, user_id, track_id, artist_id, kind, context_type,
                                          context_id, wave_session_id, at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
             ON CONFLICT (client_event_id) DO NOTHING
             RETURNING id",
            f.client_event_id,
            user,
            f.track_id,
            f.artist_id,
            f.kind.as_str(),
            f.context_type.map(|c| c.as_str()),
            f.context_id,
            f.wave_session_id,
            f.at,
        )
        .fetch_optional(&mut *tx)
        .await?;
        match inserted {
            Some(_) => out.accepted += 1,
            None => out.duplicates += 1,
        }
    }
    tx.commit().await?;
    Ok(Json(out))
}

struct HistoryRow {
    id: Uuid,
    track_id: Uuid,
    started_at: DateTime<Utc>,
    played_ms: i32,
    end_reason: String,
    context_type: Option<String>,
    context_id: Option<String>,
}

#[utoipa::path(
    get,
    path = "/api/me/history",
    tag = "events",
    security(("session" = []), ("device" = [])),
    params(PageQuery),
    responses(
        (status = 200, description = "Прослушивания, новые первыми", body = HistoryPage),
        (status = 400, description = "Неверный cursor или limit", body = ErrorBody),
    ),
)]
pub async fn history(
    State(st): State<AppState>,
    principal: Principal,
    Query(q): Query<PageQuery>,
) -> AppResult<Json<HistoryPage>> {
    let user = principal.user.id;
    let limit = cursor::limit(q.limit, 50, 200)?;
    let after = cursor::decode_opt(q.cursor.as_deref())?;
    let mut rows = sqlx::query_as!(
        HistoryRow,
        "SELECT id, track_id, started_at, played_ms, end_reason, context_type, context_id
         FROM play_events
         WHERE user_id = $1 AND ($2::timestamptz IS NULL OR (started_at, id) < ($2, $3))
         ORDER BY started_at DESC, id DESC
         LIMIT $4",
        user,
        after.map(|(at, _)| at),
        after.map(|(_, id)| id).unwrap_or_default(),
        limit + 1,
    )
    .fetch_all(&st.db)
    .await?;
    let next_cursor = cursor::page(&mut rows, limit, |r| (r.started_at, r.id));
    let ids: Vec<Uuid> = rows.iter().map(|r| r.track_id).collect();
    let tracks: HashMap<Uuid, _> = views::tracks(&st.db, &ids, user)
        .await?
        .into_iter()
        .map(|t| (t.id, t))
        .collect();
    let items = rows
        .into_iter()
        .filter_map(|r| {
            Some(HistoryItem {
                track: tracks.get(&r.track_id)?.clone(),
                id: r.id,
                started_at: r.started_at,
                played_ms: r.played_ms,
                end_reason: r.end_reason.parse().ok()?,
                context_type: r.context_type.and_then(|c| c.parse().ok()),
                context_id: r.context_id,
            })
        })
        .collect();
    Ok(Json(HistoryPage { items, next_cursor }))
}
