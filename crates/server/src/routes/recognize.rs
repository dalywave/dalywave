//! «Распознать»: приложение узнаёт песню через ShazamKit и присылает итог, сервер находит её трек
//! в каталоге и пишет в историю человека. Найденный трек — обычный трек каталога: аудио скачивается
//! при первом воспроизведении через `/api/tracks/{id}/stream`, как у любого другого.

use std::collections::HashMap;

use axum::{
    Router,
    extract::State,
    http::StatusCode,
    routing::{delete, get, post},
};
use dalywave_protocol::{
    ErrorBody,
    library::PageQuery,
    recognize::{Recognition, RecognitionPage, RecognizeRequest},
};
use uuid::Uuid;

use crate::{
    auth::Principal,
    catalog::{self, CatalogError, views},
    cursor,
    deezer::DeezerError,
    error::{AppError, AppResult, bad},
    extract::{Json, Path, Query},
    import::matching::{self, Wanted},
    state::AppState,
};

const MAX_TEXT: usize = 300;
const MAX_ID: usize = 64;
const MAX_URL: usize = 2048;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/recognize/resolve", post(resolve))
        .route("/recognize/history", get(history))
        .route("/recognize/history/{id}", delete(remove))
}

fn required(value: &str, field: &str) -> AppResult<String> {
    let value = value.trim();
    if value.is_empty() {
        return Err(bad(format!("пустое поле {field}")));
    }
    if value.chars().count() > MAX_TEXT {
        return Err(bad(format!("{field} длиннее {MAX_TEXT} символов")));
    }
    Ok(value.to_string())
}

fn optional(value: Option<String>, field: &str, max: usize) -> AppResult<Option<String>> {
    let Some(value) = value
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
    else {
        return Ok(None);
    };
    if value.chars().count() > max {
        return Err(bad(format!("{field} длиннее {max} символов")));
    }
    Ok(Some(value))
}

/// Картинку покажет приложение: чужой адрес по http — смешанный контент и подмена по дороге.
fn artwork(value: Option<String>) -> AppResult<Option<String>> {
    let Some(value) = optional(value, "artworkUrl", MAX_URL)? else {
        return Ok(None);
    };
    match url::Url::parse(&value) {
        Ok(u) if u.scheme() == "https" && u.host_str().is_some_and(|h| !h.is_empty()) => {
            Ok(Some(value))
        }
        _ => Err(bad("artworkUrl должен быть адресом https://")),
    }
}

/// Проверенный запрос: что ищем и что сохраняем в историю.
struct Input {
    wanted: Wanted,
    apple_music_id: Option<String>,
    shazam_id: Option<String>,
    artwork_url: Option<String>,
}

fn validate(body: RecognizeRequest) -> AppResult<Input> {
    let title = required(&body.title, "title")?;
    let artist = required(&body.artist, "artist")?;
    // Кривой ISRC не повод терять распознанное: ищем тогда по названию
    let isrc = optional(body.isrc, "isrc", MAX_ID)?
        .as_deref()
        .and_then(catalog::normalize_isrc);
    Ok(Input {
        wanted: Wanted {
            artist,
            title,
            album: None,
            duration_ms: None,
            isrc,
        },
        apple_music_id: optional(body.apple_music_id, "appleMusicId", MAX_ID)?,
        shazam_id: optional(body.shazam_id, "shazamId", MAX_ID)?,
        artwork_url: artwork(body.artwork_url)?,
    })
}

/// Трек каталога для распознанного; `None` — уверенно не нашёлся.
async fn find(st: &AppState, w: &Wanted) -> AppResult<Option<Uuid>> {
    if let Some(isrc) = &w.isrc {
        // Уже в каталоге — Deezer не нужен
        let local = sqlx::query_scalar!(
            "SELECT id FROM tracks WHERE isrc = $1 AND deezer_id IS NOT NULL
             ORDER BY fetched_at IS NULL, id LIMIT 1",
            isrc
        )
        .fetch_optional(&st.db)
        .await?;
        if local.is_some() {
            return Ok(local);
        }
    }
    let found = matching::find_track(&st.deezer, w)
        .await
        .map_err(|e| AppError::from(CatalogError::from(e)))?;
    if !found.accepted() {
        return Ok(None);
    }
    let Some((t, _)) = found.best else {
        return Ok(None);
    };
    // Нашёлся первым же запросом по ISRC: ответ `track/isrc:` — полный трек с участниками
    if w.isrc.is_some() && found.requests == 1 {
        return Ok(Some(catalog::save_track(&st.db, &t).await?));
    }
    // В поиске трек облегчённый: целиком (участники, ISRC) — отдельным запросом
    match catalog::resolve_track(&st.db, &st.deezer, t.id).await {
        Ok(id) => Ok(Some(id)),
        Err(CatalogError::Db(e)) => Err(e.into()),
        Err(CatalogError::Deezer(e)) => {
            if !matches!(e, DeezerError::NotFound) {
                tracing::warn!(deezer_id = t.id, error = %e, "could not load the recognized track");
            }
            // Найденное не теряем: облегчённая строка, подробности догрузит страница трека
            let ids = catalog::save_listed(&st.db, std::slice::from_ref(&t)).await?;
            Ok(ids.first().copied())
        }
    }
}

#[utoipa::path(
    post,
    path = "/api/recognize/resolve",
    operation_id = "recognize_resolve",
    tag = "recognize",
    security(("session" = []), ("device" = [])),
    request_body = RecognizeRequest,
    responses(
        (status = 200, description = "Распознанное записано в историю; `track` — трек каталога (по ISRC, затем поиском) \
                                      или `null`, если не нашёлся. Аудио скачивается при первом воспроизведении", body = Recognition),
        (status = 400, description = "Пустые или слишком длинные название и артист, artworkUrl не https", body = ErrorBody),
        (status = 503, description = "Deezer не ответил: в историю не записано, стоит повторить", body = ErrorBody),
    ),
)]
pub async fn resolve(
    State(st): State<AppState>,
    principal: Principal,
    Json(body): Json<RecognizeRequest>,
) -> AppResult<Json<Recognition>> {
    let user = principal.user.id;
    let input = validate(body)?;
    let track_id = find(&st, &input.wanted).await?;
    let w = &input.wanted;
    let row = sqlx::query!(
        "INSERT INTO recognitions (user_id, track_id, title, artist, isrc, apple_music_id, shazam_id, artwork_url)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
         RETURNING id, created_at",
        user,
        track_id,
        w.title,
        w.artist,
        w.isrc,
        input.apple_music_id,
        input.shazam_id,
        input.artwork_url,
    )
    .fetch_one(&st.db)
    .await?;
    let track = match track_id {
        Some(id) => views::track(&st.db, id, user).await?,
        None => None,
    };
    Ok(Json(Recognition {
        id: row.id,
        recognized_at: row.created_at,
        title: input.wanted.title,
        artist: input.wanted.artist,
        isrc: input.wanted.isrc,
        artwork_url: input.artwork_url,
        track,
    }))
}

#[utoipa::path(
    get,
    path = "/api/recognize/history",
    operation_id = "recognize_history",
    tag = "recognize",
    security(("session" = []), ("device" = [])),
    params(PageQuery),
    responses(
        (status = 200, description = "История распознанного, новые первыми", body = RecognitionPage),
        (status = 400, description = "Неверный cursor или limit", body = ErrorBody),
    ),
)]
pub async fn history(
    State(st): State<AppState>,
    principal: Principal,
    Query(q): Query<PageQuery>,
) -> AppResult<Json<RecognitionPage>> {
    let user = principal.user.id;
    let limit = cursor::limit(q.limit, 50, 200)?;
    let after = cursor::decode_opt(q.cursor.as_deref())?;
    let mut rows = sqlx::query!(
        "SELECT id, track_id, title, artist, isrc, artwork_url, created_at FROM recognitions
         WHERE user_id = $1 AND ($2::timestamptz IS NULL OR (created_at, id) < ($2, $3))
         ORDER BY created_at DESC, id DESC
         LIMIT $4",
        user,
        after.map(|(at, _)| at),
        after.map(|(_, id)| id).unwrap_or_default(),
        limit + 1,
    )
    .fetch_all(&st.db)
    .await?;
    let next_cursor = cursor::page(&mut rows, limit, |r| (r.created_at, r.id));
    let mut ids: Vec<Uuid> = rows.iter().filter_map(|r| r.track_id).collect();
    ids.sort_unstable();
    ids.dedup();
    let tracks: HashMap<Uuid, _> = views::tracks(&st.db, &ids, user)
        .await?
        .into_iter()
        .map(|t| (t.id, t))
        .collect();
    let items = rows
        .into_iter()
        .map(|r| Recognition {
            id: r.id,
            recognized_at: r.created_at,
            title: r.title,
            artist: r.artist,
            isrc: r.isrc,
            artwork_url: r.artwork_url,
            track: r.track_id.and_then(|id| tracks.get(&id).cloned()),
        })
        .collect();
    Ok(Json(RecognitionPage { items, next_cursor }))
}

#[utoipa::path(
    delete,
    path = "/api/recognize/history/{id}",
    operation_id = "recognize_delete",
    tag = "recognize",
    security(("session" = []), ("device" = [])),
    params(("id" = Uuid, Path, description = "id записи истории")),
    responses(
        (status = 204, description = "Запись убрана из истории"),
        (status = 404, description = "Нет такой записи у этого человека", body = ErrorBody),
    ),
)]
pub async fn remove(
    State(st): State<AppState>,
    principal: Principal,
    Path(id): Path<Uuid>,
) -> AppResult<StatusCode> {
    let deleted = sqlx::query!(
        "DELETE FROM recognitions WHERE id = $1 AND user_id = $2",
        id,
        principal.user.id
    )
    .execute(&st.db)
    .await?;
    if deleted.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn req(title: &str, artist: &str) -> RecognizeRequest {
        RecognizeRequest {
            title: title.into(),
            artist: artist.into(),
            ..RecognizeRequest::default()
        }
    }

    #[test]
    fn validation() {
        let ok = validate(RecognizeRequest {
            isrc: Some(" gb-wul-20-35890 ".into()),
            artwork_url: Some("https://is1-ssl.mzstatic.com/a.jpg".into()),
            shazam_id: Some("  ".into()),
            ..req("  Light Me Up ", "Lowborn")
        })
        .unwrap();
        assert_eq!(ok.wanted.title, "Light Me Up");
        assert_eq!(ok.wanted.isrc.as_deref(), Some("GBWUL2035890"));
        assert_eq!(ok.shazam_id, None);
        assert!(validate(req(" ", "Lowborn")).is_err());
        assert!(validate(req("x", "")).is_err());
        assert!(validate(req(&"x".repeat(MAX_TEXT + 1), "a")).is_err());
        for url in ["http://a.b/c.jpg", "javascript:alert(1)", "https://"] {
            let r = RecognizeRequest {
                artwork_url: Some(url.into()),
                ..req("a", "b")
            };
            assert!(validate(r).is_err(), "{url}");
        }
        let bad_isrc = validate(RecognizeRequest {
            isrc: Some("nope".into()),
            ..req("a", "b")
        })
        .unwrap();
        assert_eq!(bad_isrc.wanted.isrc, None);
    }
}
