//! События прослушивания и реакции, история. Приложения шлют пачки, накопленные и без сети;
//! повтор по `clientEventId` не создаёт вторую запись.

use std::collections::{HashMap, HashSet};

use axum::{
    Router,
    extract::State,
    http::StatusCode,
    routing::{delete, get, post},
};
use chrono::{DateTime, Duration, Utc};
use dalywave_protocol::{
    ErrorBody,
    events::{
        EventBatch, EventResult, HistoryDay, HistoryDays, HistoryDaysQuery, HistoryItem,
        HistoryPage, HistoryQuery, RejectedEvent, WrongRecordingReport, WrongRecordingResult,
    },
    jobs::AcquireReason,
};
use serde_json::json;
use uuid::Uuid;

use crate::{
    acquire::{self, Enqueue, MANUAL_PRIORITY, Request},
    audit,
    auth::Principal,
    catalog::views,
    cursor,
    error::{AppError, AppResult, bad},
    extract::{Json, Path, Query},
    jobs::lanes,
    state::AppState,
};

pub const MAX_BATCH: usize = 500;
/// Пауза длиннее — новая сессия прослушивания.
const SESSION_GAP_MIN: i64 = 30;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/events", post(ingest))
        .route("/me/history", get(history).delete(clear_history))
        .route("/me/history/days", get(history_days))
        .route("/me/history/{id}", delete(hide_play))
        .route("/tracks/{id}/wrong-recording", post(report_wrong_recording))
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
    source: String,
}

/// Строка поиска для `LIKE`: `%`, `_` и `\` — буквально.
fn like_escape(s: &str) -> String {
    s.chars()
        .flat_map(|c| match c {
            '%' | '_' | '\\' => vec!['\\', c],
            c => vec![c],
        })
        .collect()
}

/// Названия источников истории: альбомы, плейлисты, артисты — по id из `context_id`.
async fn context_titles(
    st: &AppState,
    rows: &[HistoryRow],
) -> AppResult<HashMap<(String, Uuid), String>> {
    let ids = |kind: &str| -> Vec<Uuid> {
        rows.iter()
            .filter(|r| r.context_type.as_deref() == Some(kind))
            .filter_map(|r| r.context_id.as_deref()?.parse().ok())
            .collect::<HashSet<Uuid>>()
            .into_iter()
            .collect()
    };
    let mut out = HashMap::new();
    for r in sqlx::query!(
        "SELECT id, title FROM albums WHERE id = ANY($1)",
        &ids("album")
    )
    .fetch_all(&st.db)
    .await?
    {
        out.insert(("album".to_string(), r.id), r.title);
    }
    for r in sqlx::query!(
        "SELECT id, title FROM playlists WHERE id = ANY($1)",
        &ids("playlist")
    )
    .fetch_all(&st.db)
    .await?
    {
        out.insert(("playlist".to_string(), r.id), r.title);
    }
    for r in sqlx::query!(
        "SELECT id, name FROM artists WHERE id = ANY($1)",
        &ids("artist")
    )
    .fetch_all(&st.db)
    .await?
    {
        out.insert(("artist".to_string(), r.id), r.name);
    }
    Ok(out)
}

#[utoipa::path(
    get,
    path = "/api/me/history",
    tag = "events",
    security(("session" = []), ("device" = [])),
    params(HistoryQuery),
    responses(
        (status = 200, description = "Прослушивания, новые первыми, с названием источника; история из архива Spotify \
                                      (`imported`) — тоже. Убранное из истории не приходит", body = HistoryPage),
        (status = 400, description = "Неверный cursor или limit", body = ErrorBody),
    ),
)]
pub async fn history(
    State(st): State<AppState>,
    principal: Principal,
    Query(q): Query<HistoryQuery>,
) -> AppResult<Json<HistoryPage>> {
    let user = principal.user.id;
    let limit = cursor::limit(q.limit, 50, 200)?;
    let after = cursor::decode_opt(q.cursor.as_deref())?;
    // Курсор главнее даты: следующая страница после перехода к дате идёт от курсора
    let before = if after.is_some() { None } else { q.before };
    let needle =
        q.q.as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|s| like_escape(&s.chars().take(200).collect::<String>()));
    let mut rows = sqlx::query_as!(
        HistoryRow,
        r#"SELECT pe.id, pe.track_id, pe.started_at, pe.played_ms, pe.end_reason, pe.context_type, pe.context_id,
                  pe.source
           FROM play_events pe
           WHERE pe.user_id = $1 AND pe.hidden_at IS NULL
             -- Время из будущего — сбой часов или архива: в истории ему не место
             AND pe.started_at <= now() + interval '10 minutes'
             AND ($2::timestamptz IS NULL OR (pe.started_at, pe.id) < ($2, $3))
             AND ($5::timestamptz IS NULL OR pe.started_at < $5)
             AND ($6::text IS NULL OR EXISTS (
                 SELECT 1 FROM tracks t LEFT JOIN albums al ON al.id = t.album_id
                 WHERE t.id = pe.track_id
                   AND (search_norm(t.title) LIKE '%' || search_norm($6) || '%'
                        OR search_norm(COALESCE(al.title, '')) LIKE '%' || search_norm($6) || '%'
                        OR EXISTS (SELECT 1 FROM track_artists ta JOIN artists a ON a.id = ta.artist_id
                                   WHERE ta.track_id = t.id
                                     AND search_norm(a.name) LIKE '%' || search_norm($6) || '%'))))
           ORDER BY pe.started_at DESC, pe.id DESC
           LIMIT $4"#,
        user,
        after.map(|(at, _)| at),
        after.map(|(_, id)| id).unwrap_or_default(),
        limit + 1,
        before,
        needle,
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
    let titles = context_titles(&st, &rows).await?;
    let items = rows
        .into_iter()
        .filter_map(|r| {
            let context_title = match (r.context_type.as_deref(), r.context_id.as_deref()) {
                (Some(kind), Some(id)) => id
                    .parse::<Uuid>()
                    .ok()
                    .and_then(|id| titles.get(&(kind.to_string(), id)).cloned()),
                _ => None,
            };
            Some(HistoryItem {
                track: tracks.get(&r.track_id)?.clone(),
                id: r.id,
                started_at: r.started_at,
                played_ms: r.played_ms,
                end_reason: r.end_reason.parse().ok()?,
                context_type: r.context_type.and_then(|c| c.parse().ok()),
                context_id: r.context_id,
                context_title,
                imported: r.source == "import",
            })
        })
        .collect();
    Ok(Json(HistoryPage { items, next_cursor }))
}

#[utoipa::path(
    get,
    path = "/api/me/history/days",
    tag = "events",
    security(("session" = []), ("device" = [])),
    params(HistoryDaysQuery),
    responses(
        (status = 200, description = "Дни с прослушиваниями по часовому поясу слушателя, новые первыми — для календаря", body = HistoryDays),
        (status = 400, description = "offsetMin вне −840…840", body = ErrorBody),
    ),
)]
pub async fn history_days(
    State(st): State<AppState>,
    principal: Principal,
    Query(q): Query<HistoryDaysQuery>,
) -> AppResult<Json<HistoryDays>> {
    let offset = q.offset_min.unwrap_or(0);
    if !(-840..=840).contains(&offset) {
        return Err(bad("offsetMin — от −840 до 840"));
    }
    let days = sqlx::query!(
        r#"SELECT ((started_at AT TIME ZONE 'UTC') + make_interval(mins => $2))::date AS "day!",
                  count(*) AS "plays!"
           FROM play_events
           WHERE user_id = $1 AND hidden_at IS NULL AND started_at <= now() + interval '10 minutes'
           GROUP BY 1
           ORDER BY 1 DESC"#,
        principal.user.id,
        offset,
    )
    .fetch_all(&st.db)
    .await?
    .into_iter()
    .map(|r| HistoryDay {
        day: r.day,
        plays: r.plays,
    })
    .collect();
    Ok(Json(HistoryDays { days }))
}

#[utoipa::path(
    delete,
    path = "/api/me/history",
    tag = "events",
    security(("session" = []), ("device" = [])),
    responses((status = 204, description = "История очищена: прослушивания больше не видны в истории, но вкус, \
                                            миксы и «Итоги года» их по-прежнему учитывают")),
)]
pub async fn clear_history(
    State(st): State<AppState>,
    principal: Principal,
) -> AppResult<StatusCode> {
    sqlx::query!(
        "UPDATE play_events SET hidden_at = now() WHERE user_id = $1 AND hidden_at IS NULL",
        principal.user.id,
    )
    .execute(&st.db)
    .await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    delete,
    path = "/api/me/history/{id}",
    tag = "events",
    security(("session" = []), ("device" = [])),
    params(("id" = Uuid, Path, description = "id прослушивания из истории")),
    responses(
        (status = 204, description = "Прослушивание убрано из истории"),
        (status = 404, description = "Нет такого прослушивания в истории", body = ErrorBody),
    ),
)]
pub async fn hide_play(
    State(st): State<AppState>,
    principal: Principal,
    Path(id): Path<Uuid>,
) -> AppResult<StatusCode> {
    let done = sqlx::query!(
        "UPDATE play_events SET hidden_at = now() WHERE id = $1 AND user_id = $2 AND hidden_at IS NULL",
        id,
        principal.user.id,
    )
    .execute(&st.db)
    .await?;
    if done.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }
    Ok(StatusCode::NO_CONTENT)
}

/// id видео YouTube из `source_ref` файла: 11 символов из букв, цифр, `-` и `_`.
fn video_of(source: &str, source_ref: Option<&str>) -> Option<String> {
    let r = source_ref?.trim();
    let ok = matches!(source, "youtube" | "youtube_music")
        && r.len() == 11
        && r.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    ok.then(|| r.to_string())
}

#[utoipa::path(
    post,
    path = "/api/tracks/{id}/wrong-recording",
    tag = "events",
    security(("session" = []), ("device" = [])),
    params(("id" = Uuid, Path, description = "id трека")),
    request_body = WrongRecordingReport,
    responses(
        (status = 202, description = "Жалоба принята: спорная запись в «Спорных» админки и новое скачивание вне очереди \
                                      без видео этой и прежних жалоб; новый файл заменит основной", body = WrongRecordingResult),
        (status = 400, description = "Комментарий длиннее 500 символов", body = ErrorBody),
        (status = 404, description = "Нет такого трека", body = ErrorBody),
        (status = 409, description = "У трека ещё нет файла на сервере (`not_ready`)", body = ErrorBody),
    ),
)]
pub async fn report_wrong_recording(
    State(st): State<AppState>,
    principal: Principal,
    Path(track_id): Path<Uuid>,
    Json(body): Json<WrongRecordingReport>,
) -> AppResult<(StatusCode, Json<WrongRecordingResult>)> {
    let user = principal.user.id;
    let comment = body
        .comment
        .as_deref()
        .map(str::trim)
        .filter(|c| !c.is_empty())
        .map(str::to_string);
    if comment.as_ref().is_some_and(|c| c.chars().count() > 500) {
        return Err(bad("комментарий — не длиннее 500 символов"));
    }
    let row = sqlx::query!(
        r#"SELECT t.recording_id, r.primary_file_id AS "file_id?", f.source AS "source?", f.source_ref
           FROM tracks t
           LEFT JOIN recordings r ON r.id = t.recording_id
           LEFT JOIN audio_files f ON f.id = r.primary_file_id
           WHERE t.id = $1"#,
        track_id,
    )
    .fetch_optional(&st.db)
    .await?
    .ok_or(AppError::NotFound)?;
    let (Some(recording_id), Some(file_id)) = (row.recording_id, row.file_id) else {
        return Err(AppError::Conflict {
            code: "not_ready",
            message: "у трека ещё нет записи на сервере".into(),
        });
    };
    let video = video_of(
        row.source.as_deref().unwrap_or_default(),
        row.source_ref.as_deref(),
    );
    // Откуда взялся нынешний файл: кандидаты и выбор последнего удачного скачивания — админу в «Спорных»
    let last = sqlx::query!(
        "SELECT candidates, chosen FROM acquisitions
         WHERE recording_id = $1 AND status = 'ready'
         ORDER BY finished_at DESC NULLS LAST, id DESC LIMIT 1",
        recording_id,
    )
    .fetch_optional(&st.db)
    .await?;
    let reason = body.reason;
    let note = match &comment {
        Some(c) => format!("Жалоба слушателя: {} — «{c}»", reason.title_ru()),
        None => format!("Жалоба слушателя: {}", reason.title_ru()),
    };

    let mut tx = st.db.begin().await?;
    let dispute_id = sqlx::query_scalar!(
        "INSERT INTO acquisitions (recording_id, track_id, status, reason, lane, requested_by, candidates, chosen,
                                   error, finished_at)
         VALUES ($1, $2, 'disputed', 'manual', $3, $4, $5, $6, $7, now())
         RETURNING id",
        recording_id,
        track_id,
        lanes::NOW,
        user,
        last.as_ref().and_then(|l| l.candidates.clone()),
        last.as_ref().and_then(|l| l.chosen.clone()),
        note,
    )
    .fetch_one(&mut *tx)
    .await?;
    let report_id = sqlx::query_scalar!(
        "INSERT INTO recording_reports (recording_id, track_id, user_id, reason, comment, file_id, video_id, dispute_id)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
         RETURNING id",
        recording_id,
        track_id,
        user,
        reason.as_str(),
        comment,
        file_id,
        video,
        dispute_id,
    )
    .fetch_one(&mut *tx)
    .await?;
    // Видео всех жалоб на эту запись: второй раз не берётся ни одно
    let exclude: Vec<String> = sqlx::query_scalar!(
        r#"SELECT DISTINCT video_id AS "video_id!" FROM recording_reports
           WHERE recording_id = $1 AND video_id IS NOT NULL"#,
        recording_id,
    )
    .fetch_all(&mut *tx)
    .await?;
    acquire::publish(&mut tx, dispute_id).await?;
    audit::record(
        &mut *tx,
        Some(user),
        audit::RECORDING_REPORTED,
        Some(recording_id.to_string()),
        json!({ "trackId": track_id, "reason": reason, "comment": comment, "videoId": video,
                "acquisitionId": dispute_id }),
    )
    .await?;
    tx.commit().await?;

    let mut request = Request::new(track_id, AcquireReason::Manual, lanes::NOW)
        .by(user)
        .priority(MANUAL_PRIORITY);
    request.force = true;
    request.replace = true;
    request.exclude_video_ids = exclude;
    let acquisition_id = match acquire::enqueue(&st.db, request).await? {
        Enqueue::Queued { acquisition_id, .. } => Some(acquisition_id),
        Enqueue::Active { .. } | Enqueue::Ready | Enqueue::Skipped(_) => None,
    };
    if let Some(a) = acquisition_id {
        sqlx::query!(
            "UPDATE recording_reports SET acquisition_id = $2 WHERE id = $1",
            report_id,
            a
        )
        .execute(&st.db)
        .await?;
    }
    Ok((
        StatusCode::ACCEPTED,
        Json(WrongRecordingResult {
            dispute_id,
            acquisition_id,
        }),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reported_video_is_a_youtube_id() {
        assert_eq!(
            video_of("youtube_music", Some("afkL3uLNeXA")),
            Some("afkL3uLNeXA".into())
        );
        assert_eq!(video_of("soundcloud", Some("afkL3uLNeXA")), None);
        assert_eq!(video_of("youtube", Some("https://youtu.be/x")), None);
        assert_eq!(video_of("youtube", None), None);
        assert_eq!(like_escape("50%_\\"), "50\\%\\_\\\\");
    }
}
