//! Воспроизведение: подписанная ссылка на аудио трека или, пока его нет на сервере, скачивание вне
//! очереди и его состояние. Байты плеер качает из хранилища сам; сервер только выдаёт ссылку.

use axum::{
    Router,
    extract::State,
    http::{StatusCode, header},
    response::{IntoResponse, Response},
    routing::get,
};
use chrono::Utc;
use dalywave_protocol::{
    ErrorBody,
    catalog::AudioStatus,
    jobs::{AcquireReason, Acquisition},
    stream::{StreamPending, StreamPreparing, StreamQuery, StreamReady, TrackStream},
};
use uuid::Uuid;

use crate::{
    acquire::{self, Enqueue, PLAY_PRIORITY, Request},
    auth::Principal,
    error::{AppError, AppResult},
    extract::{Json, Path, Query},
    jobs::lanes,
    renditions::{self, ApplePick, Picked},
    routes::settings,
    state::AppState,
    storage::AUDIO_GET_TTL,
};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/tracks/{id}/stream", get(stream))
        .route("/acquisitions/{id}", get(acquisition))
}

async fn ready(st: &AppState, picked: Picked) -> AppResult<Response> {
    let Picked {
        file,
        quality,
        lossless,
    } = picked;
    let url = st
        .storage
        .presign_get(&file.storage_key, AUDIO_GET_TTL)
        .await
        .map_err(|e| {
            AppError::Unavailable(format!("хранилище не подписало ссылку на аудио: {e}"))
        })?;
    let body = TrackStream::Ready(StreamReady {
        url: url.to_string(),
        expires_at: Utc::now() + AUDIO_GET_TTL,
        file_id: file.id,
        mime_type: file.container.mime(file.codec),
        codec: file.codec,
        container: file.container,
        bitrate_kbps: file.bitrate_kbps,
        duration_ms: file.duration_ms,
        loudness_lufs: file.loudness_lufs,
        true_peak_db: file.true_peak_db,
        waveform: file.waveform,
        quality,
        lossless,
    });
    // В ответе подписанная ссылка со сроком: кешировать его нельзя
    Ok((
        StatusCode::OK,
        [(header::CACHE_CONTROL, "no-store")],
        Json(body),
    )
        .into_response())
}

/// Готовый файл или «копию делают, спросить через 2 с» (AVPlayer, своя музыка в формате, который клиенты
/// не играют).
async fn respond(st: &AppState, pick: ApplePick) -> AppResult<Response> {
    match pick {
        ApplePick::Ready(picked) => ready(st, picked).await,
        ApplePick::Preparing => Ok((
            StatusCode::ACCEPTED,
            [(header::RETRY_AFTER, "2")],
            Json(TrackStream::Preparing(StreamPreparing {
                retry_after_ms: 2000,
            })),
        )
            .into_response()),
    }
}

/// Почему запись не скачать: она ждёт решения в «Спорных» или её нет в источниках.
async fn not_playable(st: &AppState, track_id: Uuid) -> AppResult<AppError> {
    let status = sqlx::query_scalar!(
        "SELECT r.audio_status FROM tracks t JOIN recordings r ON r.id = t.recording_id WHERE t.id = $1",
        track_id
    )
    .fetch_optional(&st.db)
    .await?;
    Ok(
        if status.as_deref() == Some(AudioStatus::Disputed.as_str()) {
            AppError::Conflict {
                code: "audio_disputed",
                message: "запись ещё не подтверждена: она ждёт решения в разделе «Спорные»".into(),
            }
        } else {
            AppError::Conflict {
                code: "audio_unavailable",
                message: "этого трека нет в источниках".into(),
            }
        },
    )
}

#[utoipa::path(
    get,
    path = "/api/tracks/{id}/stream",
    tag = "playback",
    security(("session" = []), ("device" = [])),
    params(("id" = Uuid, Path, description = "id трека"), StreamQuery),
    responses(
        (status = 200, description = "`status: ready` — подписанная ссылка на файл (6 часов), формат, громкость, форма волны. \
                                      Файл — под качество: явное `quality` или по `network` и настройкам слушателя; \
                                      «Экономия» до готовности копии отдаёт обычный файл (`quality: normal`)", body = TrackStream),
        (status = 202, description = "`status: acquiring` — трек скачивается вне очереди; `waiting_for_downloader` — \
                                      в очереди, но качать некому: загрузчик на Mac не на связи; `preparing` (`aac=true`) — \
                                      делается копия AAC, спросить снова через 2 с", body = TrackStream),
        (status = 404, description = "Нет такого трека", body = ErrorBody),
        (status = 409, description = "Запись ждёт решения в «Спорных» (`audio_disputed`) или её нет в источниках (`audio_unavailable`)", body = ErrorBody),
        (status = 503, description = "Deezer не ответил (запись трека ещё неизвестна) или хранилище недоступно", body = ErrorBody),
    ),
)]
pub async fn stream(
    State(st): State<AppState>,
    principal: Principal,
    Path(id): Path<Uuid>,
    Query(q): Query<StreamQuery>,
) -> AppResult<Response> {
    let recording = sqlx::query_scalar!("SELECT recording_id FROM tracks WHERE id = $1", id)
        .fetch_optional(&st.db)
        .await?
        .ok_or(AppError::NotFound)?;
    let user_settings = settings::of(&st.db, principal.user.id).await?;
    let quality = renditions::quality_for(q.quality, q.network, &user_settings);
    // AVPlayer (Apple TV): только то, что он играет, иначе копия AAC
    if q.aac == Some(true)
        && let Some(recording) = recording
        && let Some(pick) = renditions::pick_apple(&st.db, recording, quality).await?
    {
        return respond(&st, pick).await;
    }
    if let Some(recording) = recording
        && let Some(pick) = renditions::pick(&st.db, recording, quality).await?
    {
        return respond(&st, pick).await;
    }
    // Скачивать некому: загрузчик выключен на этом сервере
    if !st.config.features.downloader {
        return Err(not_playable(&st, id).await?);
    }
    let req = Request::new(id, AcquireReason::Play, lanes::NOW)
        .by(principal.user.id)
        .priority(PLAY_PRIORITY);
    let acquisition_id = match acquire::request(&st.db, &st.deezer, req).await? {
        Enqueue::Queued { acquisition_id, .. } | Enqueue::Active { acquisition_id } => {
            acquisition_id
        }
        // Файл появился, пока шёл запрос
        Enqueue::Ready => {
            let recording =
                sqlx::query_scalar!("SELECT recording_id FROM tracks WHERE id = $1", id)
                    .fetch_one(&st.db)
                    .await?;
            return match recording {
                Some(r) => match renditions::pick(&st.db, r, quality).await? {
                    Some(pick) => respond(&st, pick).await,
                    None => Err(AppError::Unavailable("файл записи недоступен".into())),
                },
                None => Err(AppError::NotFound),
            };
        }
        Enqueue::Skipped("needs_review" | "no_deezer_id") => {
            return Err(not_playable(&st, id).await?);
        }
        Enqueue::Skipped("no_recording") => {
            return Err(AppError::Unavailable(
                "каталог Deezer сейчас недоступен: трек не сопоставить с записью, попробуйте позже"
                    .into(),
            ));
        }
        Enqueue::Skipped(_) => return Err(AppError::NotFound),
    };
    let info = acquire::view(&st.db, st.config.fetcher.server, acquisition_id)
        .await?
        .ok_or(AppError::NotFound)?;
    let pending = StreamPending {
        acquisition_id,
        queue_position: info.queue_position.unwrap_or(0),
        downloaders_online: info.downloaders_online,
    };
    let body = if info.waiting_for_downloader {
        TrackStream::WaitingForDownloader(pending)
    } else {
        TrackStream::Acquiring(pending)
    };
    Ok((StatusCode::ACCEPTED, Json(body)).into_response())
}

#[utoipa::path(
    get,
    path = "/api/acquisitions/{id}",
    tag = "playback",
    security(("session" = []), ("device" = [])),
    params(("id" = Uuid, Path, description = "id скачивания (`acquisitionId` из ответа `/stream`)")),
    responses(
        (status = 200, description = "Шаг скачивания, место в очереди, есть ли кому качать", body = Acquisition),
        (status = 404, description = "Нет такого скачивания", body = ErrorBody),
    ),
)]
pub async fn acquisition(
    State(st): State<AppState>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<Acquisition>> {
    acquire::view(&st.db, st.config.fetcher.server, id)
        .await?
        .map(Json)
        .ok_or(AppError::NotFound)
}
