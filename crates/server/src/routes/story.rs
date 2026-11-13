//! «Поделиться → История (видео)»: звук истории и сборка MP4 (story.rs).

use axum::{
    Router,
    body::Bytes,
    extract::{DefaultBodyLimit, State},
    http::header,
    response::{IntoResponse, Response},
    routing::{get, post},
};
use dalywave_protocol::ErrorBody;
use uuid::Uuid;

use crate::{error::AppResult, extract::Path, state::AppState, story};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/tracks/{id}/story/audio", get(story_audio))
        .route(
            "/tracks/{id}/story",
            post(story_video).layer(DefaultBodyLimit::max(story::MAX_VIDEO_BYTES)),
        )
}

#[utoipa::path(
    get,
    path = "/api/tracks/{id}/story/audio",
    tag = "playback",
    security(("session" = []), ("device" = [])),
    params(("id" = Uuid, Path, description = "id трека")),
    responses(
        (status = 200, description = "15 с с припева в WAV (PCM 16 бит, 44,1 кГц, стерео) с мягким входом и выходом: \
                                      из звука на сервере или превью Deezer", content_type = "audio/wav"),
        (status = 404, description = "Нет такого трека (`not_found`) или звука для истории (`no_audio`)", body = ErrorBody),
        (status = 503, description = "Deezer, хранилище или ffmpeg недоступны", body = ErrorBody),
    ),
)]
pub async fn story_audio(State(st): State<AppState>, Path(id): Path<Uuid>) -> AppResult<Response> {
    let wav = story::audio_wav(&st, id).await?;
    Ok((
        [
            (header::CONTENT_TYPE, "audio/wav"),
            (header::CACHE_CONTROL, "private, max-age=600"),
        ],
        wav,
    )
        .into_response())
}

#[utoipa::path(
    post,
    path = "/api/tracks/{id}/story",
    tag = "playback",
    security(("session" = []), ("device" = [])),
    params(("id" = Uuid, Path, description = "id трека")),
    request_body(content = Vec<u8>, content_type = "video/mp4",
                 description = "Видео истории без звука (MP4 с H.264 или WebM), до 64 МБ"),
    responses(
        (status = 200, description = "Готовая история: MP4, H.264 + AAC, звук — 15 с с припева", content_type = "video/mp4"),
        (status = 404, description = "Нет такого трека (`not_found`) или звука для истории (`no_audio`)", body = ErrorBody),
        (status = 413, description = "Видео больше 64 МБ", body = ErrorBody),
        (status = 422, description = "Видео не читается (`not_a_video`, `story_failed`)", body = ErrorBody),
    ),
)]
pub async fn story_video(
    State(st): State<AppState>,
    Path(id): Path<Uuid>,
    body: Bytes,
) -> AppResult<Response> {
    let mp4 = story::mux(&st, id, &body).await?;
    Ok((
        [
            (header::CONTENT_TYPE, "video/mp4"),
            (header::CACHE_CONTROL, "no-store"),
            (
                header::CONTENT_DISPOSITION,
                "attachment; filename=\"dalywave-story.mp4\"",
            ),
        ],
        mp4,
    )
        .into_response())
}
