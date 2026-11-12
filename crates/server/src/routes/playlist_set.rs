//! «Собрать как сет»: порядок плейлиста, в котором треки сводятся как DJ-сет (подбор —
//! [`dalywave_transitions::set`]). Предпросмотр плейлист не меняет; применение — одна операция:
//! позиции перенумеровываются в новом порядке тем же путём, что и перенумерация после вставок
//! (строки получают новый номер изменения и уходят на устройства), а прежний порядок возвращается
//! в ответе — его кладут обратно через `PUT /api/playlists/{id}/order` («Отменить»).
//!
//! Миксы и подборки собираются рекомендациями: предпросмотр для них есть, применить можно только к
//! своей копии.

use std::collections::{BTreeMap, HashSet};

use axum::{
    Router,
    extract::State,
    http::StatusCode,
    routing::{post, put},
};
use dalywave_protocol::{
    ErrorBody,
    library::{
        PlaylistKind, PlaylistOrder, SetApplied, SetCurve, SetEntry, SetFit, SetOptions, SetPreview,
    },
    sync::SyncEntity,
};
use dalywave_transitions::set::{self, Curve, Fit, SetTrack};
use sqlx::PgConnection;
use uuid::Uuid;

use super::playlists::{changed_playlist, lock_own_editable, touch};
use crate::{
    auth::Principal,
    error::{AppError, AppResult, bad},
    extract::{Json, JsonOrDefault, Path},
    state::AppState,
};

/// Больше — сет не собирается: матрицы переходов растут квадратично.
pub const MAX_SET: usize = 1000;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/playlists/{id}/set/preview", post(preview))
        .route("/playlists/{id}/set", post(apply))
        .route("/playlists/{id}/order", put(set_order))
}

/// Позиция плейлиста и что о её треке известно для сборки.
struct Row {
    item_id: Uuid,
    track_id: Uuid,
    track: SetTrack,
}

/// Позиции в нынешнем порядке с анализом треков.
async fn rows(conn: &mut PgConnection, playlist_id: Uuid) -> Result<Vec<Row>, sqlx::Error> {
    let rows = sqlx::query!(
        r#"SELECT i.id, i.track_id, a.source AS "source?", a.bpm AS "bpm?", a.key_camelot AS "key?",
                  a.key_confidence AS "key_confidence?", a.energy AS "energy?",
                  a.energy_curve AS "energy_curve?",
                  ARRAY(SELECT ta.artist_id FROM track_artists ta
                        WHERE ta.track_id = i.track_id AND ta.role = 'main') AS "artists!"
           FROM playlist_items i
           JOIN tracks t ON t.id = i.track_id
           LEFT JOIN track_analysis a ON a.recording_id = t.recording_id
           WHERE i.playlist_id = $1
           ORDER BY i.position, i.id"#,
        playlist_id,
    )
    .fetch_all(conn)
    .await?;
    // Исполнители — номерами по порядку id: одинаково при одинаковом плейлисте
    let mut artist_no: BTreeMap<Uuid, u32> = rows
        .iter()
        .flat_map(|r| r.artists.iter().copied())
        .map(|a| (a, 0))
        .collect();
    for (n, no) in artist_no.values_mut().enumerate() {
        *no = n as u32;
    }
    Ok(rows
        .into_iter()
        .map(|r| {
            let track = SetTrack {
                bpm: r.bpm.map(f64::from),
                key: r.key,
                key_confidence: r.key_confidence.map(f64::from),
                energy: r.energy.map(f64::from),
                head: None,
                tail: None,
                artists: r.artists.iter().map(|a| artist_no[a]).collect(),
            };
            // Края кривой — только у полного анализа: превью — это 30 секунд из середины
            let track = match (r.source.as_deref(), r.energy_curve) {
                (Some("full"), Some(curve)) => {
                    let curve: Vec<f64> = curve.into_iter().map(f64::from).collect();
                    track.with_curve(&curve)
                }
                _ => track,
            };
            Row {
                item_id: r.id,
                track_id: r.track_id,
                track,
            }
        })
        .collect())
}

fn curve_of(c: SetCurve) -> Curve {
    match c {
        SetCurve::Rise => Curve::Rise,
        SetCurve::Wave => Curve::Wave,
        SetCurve::Flat => Curve::Flat,
    }
}

fn fit_of(f: Fit) -> SetFit {
    match f {
        Fit::InKey => SetFit::InKey,
        Fit::Smooth => SetFit::Smooth,
        Fit::Lift => SetFit::Lift,
        Fit::Calm => SetFit::Calm,
        Fit::Switch => SetFit::Switch,
    }
}

fn smooth_count(tracks: &[SetTrack], order: &[usize]) -> i64 {
    order
        .windows(2)
        .filter(|w| set::fit(&tracks[w[0]], &tracks[w[1]]).is_some_and(Fit::is_smooth))
        .count() as i64
}

/// Сет для плейлиста: предпросмотр и новый порядок позиций.
async fn plan(
    conn: &mut PgConnection,
    playlist_id: Uuid,
    opts: &SetOptions,
    read_only: bool,
) -> AppResult<(Vec<Uuid>, SetPreview, Vec<Uuid>)> {
    let rows = rows(conn, playlist_id).await?;
    if rows.len() > MAX_SET {
        return Err(AppError::Unprocessable {
            code: "too_long",
            message: format!("сет собирается из плейлистов до {MAX_SET} треков"),
        });
    }
    let first = match opts.first_item_id {
        Some(item) => Some(
            rows.iter()
                .position(|r| r.item_id == item)
                .ok_or_else(|| bad("нет такой позиции в плейлисте"))?,
        ),
        None => None,
    };
    let tracks: Vec<SetTrack> = rows.iter().map(|r| r.track.clone()).collect();
    let options = set::Options {
        curve: curve_of(opts.curve),
        first,
        spread_artists: opts.spread_artists,
    };
    let arranged = tokio::task::spawn_blocking({
        let tracks = tracks.clone();
        move || set::arrange(&tracks, &options)
    })
    .await
    .map_err(|e| AppError::Internal(e.into()))?;

    let order = &arranged.order;
    let entries = order
        .iter()
        .enumerate()
        .map(|(k, &i)| SetEntry {
            item_id: rows[i].item_id,
            track_id: rows[i].track_id,
            energy: tracks[i].energy.map(|e| e as f32),
            fit: k
                .checked_sub(1)
                .and_then(|p| set::fit(&tracks[order[p]], &tracks[i]))
                .map(fit_of),
            placed: k < arranged.placed,
        })
        .collect();
    let current: Vec<Uuid> = rows.iter().map(|r| r.item_id).collect();
    let next: Vec<Uuid> = order.iter().map(|&i| rows[i].item_id).collect();
    let preview = SetPreview {
        curve: opts.curve,
        entries,
        unplaced: (rows.len() - arranged.placed) as i64,
        smooth_before: smooth_count(&tracks, &(0..rows.len()).collect::<Vec<_>>()),
        smooth_after: smooth_count(&tracks, order),
        changed: next != current,
        read_only,
    };
    Ok((current, preview, next))
}

/// Позиции плейлиста по порядку `order`: 1024, 2048, … Меняются (и уходят на устройства) только
/// строки, чья позиция стала другой.
async fn reorder(
    conn: &mut PgConnection,
    playlist_id: Uuid,
    order: &[Uuid],
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        "UPDATE playlist_items i SET position = (x.rn * 1024)::float8
         FROM unnest($2::uuid[]) WITH ORDINALITY AS x(id, rn)
         WHERE i.id = x.id AND i.playlist_id = $1 AND i.position <> (x.rn * 1024)::float8",
        playlist_id,
        order,
    )
    .execute(conn)
    .await?;
    Ok(())
}

#[utoipa::path(
    post,
    path = "/api/playlists/{id}/set/preview",
    tag = "library",
    security(("session" = []), ("device" = [])),
    params(("id" = Uuid, Path, description = "id плейлиста или микса")),
    request_body = SetOptions,
    responses(
        (status = 200, description = "Порядок сета; плейлист не меняется", body = SetPreview),
        (status = 400, description = "Нет позиции `firstItemId`", body = ErrorBody),
        (status = 404, description = "Нет такого плейлиста", body = ErrorBody),
        (status = 422, description = "Слишком длинный плейлист (`too_long`)", body = ErrorBody),
    ),
)]
pub async fn preview(
    State(st): State<AppState>,
    principal: Principal,
    Path(id): Path<Uuid>,
    JsonOrDefault(opts): JsonOrDefault<SetOptions>,
) -> AppResult<Json<SetPreview>> {
    let mut conn = st.db.acquire().await?;
    let kind = sqlx::query_scalar!(
        "SELECT kind FROM playlists WHERE id = $1 AND user_id = $2",
        id,
        principal.user.id
    )
    .fetch_optional(&mut *conn)
    .await?
    .ok_or(AppError::NotFound)?;
    let read_only = kind != PlaylistKind::User.as_str();
    let (_, preview, _) = plan(&mut conn, id, &opts, read_only).await?;
    Ok(Json(preview))
}

#[utoipa::path(
    post,
    path = "/api/playlists/{id}/set",
    tag = "library",
    security(("session" = []), ("device" = [])),
    params(("id" = Uuid, Path, description = "id плейлиста")),
    request_body = SetOptions,
    responses(
        (status = 200, description = "Сет применён; прежний порядок — для «Отменить»", body = SetApplied),
        (status = 400, description = "Нет позиции `firstItemId`", body = ErrorBody),
        (status = 404, description = "Нет такого плейлиста", body = ErrorBody),
        (status = 403, description = "Совместный плейлист: собирает только владелец", body = ErrorBody),
        (status = 409, description = "Плейлист собирается сам (микс): сначала сохранить копию", body = ErrorBody),
        (status = 422, description = "Слишком длинный плейлист (`too_long`)", body = ErrorBody),
    ),
)]
pub async fn apply(
    State(st): State<AppState>,
    principal: Principal,
    Path(id): Path<Uuid>,
    JsonOrDefault(opts): JsonOrDefault<SetOptions>,
) -> AppResult<Json<SetApplied>> {
    let user = principal.user.id;
    let mut tx = st.db.begin().await?;
    // Сет собирает только владелец: в совместном плейлисте порядок общий
    lock_own_editable(&mut tx, user, id).await?;
    let (previous, preview, next) = plan(&mut tx, id, &opts, false).await?;
    if preview.changed {
        reorder(&mut tx, id, &next).await?;
        touch(&mut tx, id).await?;
        changed_playlist(
            &mut tx,
            id,
            vec![SyncEntity::Playlists, SyncEntity::PlaylistItems],
        )
        .await;
    }
    tx.commit().await?;
    Ok(Json(SetApplied {
        set: preview,
        previous_item_ids: previous,
    }))
}

#[utoipa::path(
    put,
    path = "/api/playlists/{id}/order",
    tag = "library",
    security(("session" = []), ("device" = [])),
    params(("id" = Uuid, Path, description = "id плейлиста")),
    request_body = PlaylistOrder,
    responses(
        (status = 204, description = "Порядок задан: названные позиции — так, остальные — за ними в прежнем порядке; чужие id пропущены"),
        (status = 404, description = "Нет такого плейлиста", body = ErrorBody),
        (status = 409, description = "Плейлист собирается сам (микс)", body = ErrorBody),
    ),
)]
pub async fn set_order(
    State(st): State<AppState>,
    principal: Principal,
    Path(id): Path<Uuid>,
    Json(body): Json<PlaylistOrder>,
) -> AppResult<StatusCode> {
    let user = principal.user.id;
    let mut tx = st.db.begin().await?;
    // Сет собирает только владелец: в совместном плейлисте порядок общий
    lock_own_editable(&mut tx, user, id).await?;
    let current = sqlx::query_scalar!(
        "SELECT id FROM playlist_items WHERE playlist_id = $1 ORDER BY position, id",
        id
    )
    .fetch_all(&mut *tx)
    .await?;
    let known: HashSet<Uuid> = current.iter().copied().collect();
    let mut seen = HashSet::new();
    let mut next: Vec<Uuid> = body
        .item_ids
        .iter()
        .copied()
        .filter(|x| known.contains(x) && seen.insert(*x))
        .collect();
    next.extend(current.iter().copied().filter(|x| !seen.contains(x)));
    if next != current {
        reorder(&mut tx, id, &next).await?;
        touch(&mut tx, id).await?;
        changed_playlist(
            &mut tx,
            id,
            vec![SyncEntity::Playlists, SyncEntity::PlaylistItems],
        )
        .await;
    }
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}
