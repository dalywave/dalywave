//! Тексты песен: текст трека со строками по времени и поиск песни по строчке среди уже взятых текстов.

use std::collections::HashMap;

use axum::{
    Router,
    extract::State,
    http::header,
    response::{IntoResponse, Response},
    routing::get,
};
use dalywave_protocol::{
    ErrorBody,
    lyrics::{LyricsHit, LyricsKind, LyricsSearchQuery, LyricsSearchResults, TrackLyrics},
};
use uuid::Uuid;

use crate::{
    auth::Principal,
    catalog::views,
    error::{AppError, AppResult, bad},
    extract::{Json, Path, Query},
    lyrics::{self, LyricsError},
    state::AppState,
};

const MAX_SEARCH: u32 = 50;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/tracks/{id}/lyrics", get(track_lyrics))
        .route("/lyrics/search", get(search_lyrics))
}

#[utoipa::path(
    get,
    path = "/api/tracks/{id}/lyrics",
    tag = "catalog",
    security(("session" = []), ("device" = [])),
    params(("id" = Uuid, Path, description = "id трека")),
    responses(
        (status = 200, description = "Текст трека: строки с временем (`synced`), простой текст (`plain`), \
                                      «без слов» (`instrumental`) или «не нашли» (`none`). Берётся у LRCLIB \
                                      при первом запросе и хранится; «не нашли» — 30 дней", body = TrackLyrics),
        (status = 404, description = "Нет такого трека", body = ErrorBody),
        (status = 503, description = "LRCLIB не ответил или просит подождать: стоит повторить позже", body = ErrorBody),
    ),
)]
pub async fn track_lyrics(State(st): State<AppState>, Path(id): Path<Uuid>) -> AppResult<Response> {
    let body = match lyrics::resolve(&st.db, &st.lrclib, id).await {
        Ok(l) => l,
        Err(LyricsError::NoTrack) => return Err(AppError::NotFound),
        Err(LyricsError::Db(e)) => return Err(e.into()),
        Err(LyricsError::Source(e)) => {
            tracing::warn!(track_id = %id, error = %e, "lyrics source failed");
            return Err(AppError::Unavailable("тексты сейчас недоступны".into()));
        }
    };
    // Найденный текст не меняется; «не нашли» может смениться находкой — держим недолго
    let cache = match body.kind {
        LyricsKind::None => "private, max-age=600",
        _ => "private, max-age=86400",
    };
    Ok(([(header::CACHE_CONTROL, cache)], Json(body)).into_response())
}

/// `%`, `_` и `\` в запросе — буквы, а не шаблон LIKE.
fn like_escape(s: &str) -> String {
    s.chars()
        .flat_map(|c| match c {
            '%' | '_' | '\\' => vec!['\\', c],
            c => vec![c],
        })
        .collect()
}

#[utoipa::path(
    get,
    path = "/api/lyrics/search",
    tag = "catalog",
    security(("session" = []), ("device" = [])),
    params(LyricsSearchQuery),
    responses(
        (status = 200, description = "Треки, в тексте которых есть строчка (среди уже взятых текстов), \
                                      и строка, где она нашлась", body = LyricsSearchResults),
        (status = 400, description = "Строчка короче 3 символов", body = ErrorBody),
    ),
)]
pub async fn search_lyrics(
    State(st): State<AppState>,
    principal: Principal,
    Query(q): Query<LyricsSearchQuery>,
) -> AppResult<Json<LyricsSearchResults>> {
    let text = q.q.trim();
    if text.chars().count() < 3 {
        return Err(bad("строчка — от 3 символов"));
    }
    let limit = i64::from(q.limit.unwrap_or(20).clamp(1, MAX_SEARCH));
    // Строчка, набранная не в той раскладке («z yt pyf.»), ищется и исправленной
    let fixed = dalywave_textmatch::fix_layout(text);
    let mut text = text;
    let mut rows = Vec::new();
    for line in std::iter::once(text).chain(fixed.as_deref()) {
        rows = sqlx::query!(
            r#"SELECT track_id, plain AS "plain!"
                 FROM track_lyrics
                WHERE plain IS NOT NULL AND NOT instrumental
                  AND search_norm(plain) LIKE '%' || search_norm($1) || '%'
                ORDER BY fetched_at DESC
                LIMIT $2"#,
            like_escape(line),
            limit,
        )
        .fetch_all(&st.db)
        .await?;
        text = line;
        if !rows.is_empty() {
            break;
        }
    }
    let ids: Vec<Uuid> = rows.iter().map(|r| r.track_id).collect();
    let mut tracks: HashMap<Uuid, _> = views::tracks(&st.db, &ids, principal.user.id)
        .await?
        .into_iter()
        .map(|t| (t.id, t))
        .collect();
    let items = rows
        .into_iter()
        .filter_map(|r| {
            let track = tracks.remove(&r.track_id)?;
            Some(LyricsHit {
                track,
                line: lyrics::matching_line(&r.plain, text),
            })
        })
        .collect();
    Ok(Json(LyricsSearchResults { items }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_like_patterns() {
        assert_eq!(like_escape(r"50% off_\x"), r"50\% off\_\\x");
    }
}
