//! Сведения о музыке: чарты, отрывок с припева перед включением, кто сделал трек, об артисте.

use axum::{
    Router,
    extract::State,
    http::header,
    response::{IntoResponse, Response},
    routing::get,
};
use chrono::Datelike;
use dalywave_protocol::{
    ErrorBody,
    info::{ArtistInfo, ChartDetail, ChartList, HookFrom, TrackCredits, TrackHook},
};
use uuid::Uuid;

use crate::{
    auth::Principal,
    catalog,
    charts::{self, RefreshError},
    chorus,
    deezer::DeezerError,
    error::{AppError, AppResult},
    extract::{Json, Path},
    info::{about, credits},
    state::AppState,
};

/// Сколько звучит отрывок перед включением.
pub const HOOK_MS: i64 = 15_000;

/// Каталог: чарты, авторы, об артисте.
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/charts", get(list_charts))
        .route("/charts/{id}", get(chart))
        .route("/tracks/{id}/credits", get(track_credits))
        .route("/artists/{id}/about", get(artist_about))
}

/// Воспроизведение: отрывок с припева.
pub fn play_routes() -> Router<AppState> {
    Router::new().route("/tracks/{id}/hook", get(hook))
}

#[utoipa::path(
    get,
    path = "/api/charts",
    tag = "catalog",
    security(("session" = []), ("device" = [])),
    responses(
        (status = 200, description = "Чарты: Россия, мир, жанры — с обложкой первого места", body = ChartList),
    ),
)]
pub async fn list_charts(
    State(st): State<AppState>,
    principal: Principal,
) -> AppResult<Json<ChartList>> {
    Ok(Json(ChartList {
        items: charts::list(&st, principal.user.id).await?,
    }))
}

#[utoipa::path(
    get,
    path = "/api/charts/{id}",
    tag = "catalog",
    security(("session" = []), ("device" = [])),
    params(("id" = String, Path, description = "`ru`, `world` или `genre-…` из списка чартов")),
    responses(
        (status = 200, description = "Треки по местам, альбомы и артисты по трекам чарта. `updating` — \
                                      чарт собирается, стоит спросить ещё раз через несколько секунд", body = ChartDetail),
        (status = 404, description = "Нет такого чарта", body = ErrorBody),
        (status = 503, description = "Deezer не ответил, а собранного чарта ещё нет", body = ErrorBody),
    ),
)]
pub async fn chart(
    State(st): State<AppState>,
    principal: Principal,
    Path(id): Path<String>,
) -> AppResult<Json<ChartDetail>> {
    let def = charts::find(&id).ok_or(AppError::NotFound)?;
    match charts::detail(&st, def, principal.user.id).await {
        Ok(d) => Ok(Json(d)),
        Err(RefreshError::Db(e)) => Err(e.into()),
        Err(e) => {
            tracing::warn!(chart = def.id, error = %e, "chart is unavailable");
            Err(AppError::Unavailable("чарт сейчас недоступен".into()))
        }
    }
}

#[utoipa::path(
    get,
    path = "/api/tracks/{id}/hook",
    tag = "playback",
    security(("session" = []), ("device" = [])),
    params(("id" = Uuid, Path, description = "id трека")),
    responses(
        (status = 200, description = "Отрывок ~15 с с припева: из звука на сервере (место — по анализу или \
                                      30 % трека) или превью Deezer. Трек не ставится на скачивание", body = TrackHook),
        (status = 404, description = "Нет такого трека", body = ErrorBody),
        (status = 503, description = "Deezer не ответил", body = ErrorBody),
    ),
)]
pub async fn hook(State(st): State<AppState>, Path(id): Path<Uuid>) -> AppResult<Response> {
    let row = sqlx::query!(
        r#"SELECT t.deezer_id, r.audio_status AS "audio_status?"
           FROM tracks t LEFT JOIN recordings r ON r.id = t.recording_id WHERE t.id = $1"#,
        id
    )
    .fetch_optional(&st.db)
    .await?
    .ok_or(AppError::NotFound)?;
    let body = if row.audio_status.as_deref() == Some("ready") {
        let (start_ms, analysed) = chorus::starts(&st.db, &[id], HOOK_MS)
            .await?
            .remove(&id)
            .unwrap_or((0, false));
        TrackHook {
            track_id: id,
            ready: true,
            start_ms,
            length_ms: HOOK_MS,
            from: if analysed {
                HookFrom::Analysis
            } else {
                HookFrom::Estimate
            },
            preview_url: None,
        }
    } else {
        let preview_url = match row.deezer_id {
            Some(d) => match st.deezer.track(d).await {
                Ok(t) => t.preview,
                Err(DeezerError::NotFound) => None,
                Err(e) => {
                    tracing::warn!(track_id = %id, error = %e, "deezer preview lookup failed");
                    return Err(AppError::Unavailable("Deezer не ответил".into()));
                }
            },
            None => None,
        };
        TrackHook {
            track_id: id,
            ready: false,
            start_ms: 0,
            length_ms: HOOK_MS,
            from: HookFrom::Preview,
            preview_url,
        }
    };
    // Ссылка на превью подписана и живёт недолго
    Ok(([(header::CACHE_CONTROL, "no-store")], Json(body)).into_response())
}

#[utoipa::path(
    get,
    path = "/api/tracks/{id}/credits",
    tag = "catalog",
    security(("session" = []), ("device" = [])),
    params(("id" = Uuid, Path, description = "id трека")),
    responses(
        (status = 200, description = "Авторы музыки и слов, продюсеры и звукорежиссёры (MusicBrainz по ISRC), \
                                      лейбл и год. Пустые списки — MusicBrainz трека не знает или не ответил. \
                                      `resolving` — ссылки на страницы людей ещё ищутся", body = TrackCredits),
        (status = 404, description = "Нет такого трека", body = ErrorBody),
    ),
)]
pub async fn track_credits(
    State(st): State<AppState>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<TrackCredits>> {
    catalog::ensure_track(&st.db, &st.deezer, id).await?;
    let t = sqlx::query!(
        r#"SELECT t.isrc, t.duration_ms, al.label AS "label?", al.release_date AS "release_date?"
           FROM tracks t LEFT JOIN albums al ON al.id = t.album_id WHERE t.id = $1"#,
        id
    )
    .fetch_optional(&st.db)
    .await?
    .ok_or(AppError::NotFound)?;
    let people = match &t.isrc {
        Some(isrc) => {
            match credits::load(&st.db, &st.info.musicbrainz, isrc, i64::from(t.duration_ms)).await
            {
                Ok(p) => p,
                Err(credits::LoadError::Db(e)) => return Err(e.into()),
                // MusicBrainz занят — показываем лейбл и год, авторов спросим в другой раз
                Err(credits::LoadError::Source(e)) => {
                    tracing::warn!(track_id = %id, error = %e, "musicbrainz is unavailable");
                    Vec::new()
                }
            }
        }
        None => Vec::new(),
    };
    let (body, unchecked) = credits::view(
        &st.db,
        id,
        &people,
        t.label,
        t.release_date.map(|d| d.year()),
    )
    .await?;
    if !unchecked.is_empty() {
        let st = st.clone();
        tokio::spawn(async move {
            credits::resolve_people(&st.db, &st.info.musicbrainz, &st.deezer, unchecked).await;
        });
    }
    Ok(Json(body))
}

#[utoipa::path(
    get,
    path = "/api/artists/{id}/about",
    tag = "catalog",
    security(("session" = []), ("device" = [])),
    params(("id" = Uuid, Path, description = "id артиста")),
    responses(
        (status = 200, description = "Начало статьи Википедии и факты: годы на сцене, откуда, жанры. \
                                      `about: null` — статьи нет", body = ArtistInfo),
        (status = 404, description = "Нет такого артиста", body = ErrorBody),
        (status = 503, description = "Википедия не ответила", body = ErrorBody),
    ),
)]
pub async fn artist_about(State(st): State<AppState>, Path(id): Path<Uuid>) -> AppResult<Response> {
    let exists = sqlx::query_scalar!(
        "SELECT EXISTS (SELECT 1 FROM artists WHERE id = $1) AS \"e!\"",
        id
    )
    .fetch_one(&st.db)
    .await?;
    if !exists {
        return Err(AppError::NotFound);
    }
    let about = match about::load(&st.db, &st.info, id).await {
        Ok(a) => a,
        Err(about::FetchError::Db(e)) => return Err(e.into()),
        Err(about::FetchError::Source(e)) => {
            tracing::warn!(artist_id = %id, error = %e, "wiki is unavailable");
            return Err(AppError::Unavailable(
                "сведения об артисте сейчас недоступны".into(),
            ));
        }
    };
    Ok((
        [(header::CACHE_CONTROL, "private, max-age=3600")],
        Json(ArtistInfo {
            artist_id: id,
            about,
        }),
    )
        .into_response())
}
