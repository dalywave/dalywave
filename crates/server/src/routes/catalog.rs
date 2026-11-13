//! Каталог: поиск, страницы артиста, альбома и трека, разрешение id Deezer. Данные Deezer подгружаются
//! при первом обращении и освежаются по срокам (см. [`crate::catalog`]).

use axum::{
    Router,
    extract::State,
    routing::{get, post},
};
use dalywave_protocol::{
    ErrorBody,
    catalog::{
        AlbumDetail, ArtistDetail, AudioStatus, CatalogEntity, DeezerKind, FollowState, RecordType,
        Releases, SearchQuery, SearchResults, SearchScope, Track, TrackList, TrackLookup,
    },
};
use uuid::Uuid;

use crate::{
    auth::Principal,
    catalog::{self, Types, find, views},
    cursor,
    error::{AppError, AppResult, bad},
    extract::{Json, Path, Query},
    state::AppState,
};

pub const MAX_LOOKUP: usize = 500;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/search", get(search))
        .route("/artists/{id}", get(artist))
        .route("/albums/{id}", get(album))
        .route("/tracks/lookup", post(lookup_tracks))
        .route("/tracks/{id}", get(track))
        .route("/catalog/deezer/{kind}/{deezerId}", get(resolve_deezer))
}

fn parse_types(raw: Option<&str>) -> AppResult<Types> {
    let Some(raw) = raw.filter(|r| !r.trim().is_empty()) else {
        return Ok(Types::ALL);
    };
    let mut types = Types {
        track: false,
        album: false,
        artist: false,
    };
    for part in raw.split(',').map(str::trim).filter(|p| !p.is_empty()) {
        match part {
            "track" => types.track = true,
            "album" => types.album = true,
            "artist" => types.artist = true,
            other => return Err(bad(format!("неизвестный вид в types: {other}"))),
        }
    }
    Ok(types)
}

#[utoipa::path(
    get,
    path = "/api/search",
    tag = "catalog",
    security(("session" = []), ("device" = [])),
    params(SearchQuery),
    responses(
        (status = 200, description = "Найденное: треки, альбомы, артисты. Находит и по русской записи \
                                      латинского имени, и в другой раскладке, и с опечаткой", body = SearchResults),
        (status = 400, description = "Пустой запрос, неверные types или limit", body = ErrorBody),
        (status = 503, description = "Deezer не ответил (только `scope=catalog`)", body = ErrorBody),
    ),
)]
pub async fn search(
    State(st): State<AppState>,
    principal: Principal,
    Query(q): Query<SearchQuery>,
) -> AppResult<Json<SearchResults>> {
    let text = q.q.trim();
    if text.is_empty() {
        return Err(bad("пустой запрос"));
    }
    if text.chars().count() > 200 {
        return Err(bad("слишком длинный запрос"));
    }
    let limit = cursor::limit(q.limit, 20, 50)?;
    let types = parse_types(q.types.as_deref())?;
    let scope = q.scope.unwrap_or_default();
    let user = principal.user.id;
    let found = match scope {
        SearchScope::Catalog => {
            find::catalog(&st.db, &st.deezer, text, types, limit as u32).await?
        }
        SearchScope::Library => find::library(&st.db, user, text, types, limit).await?,
    };
    let hits = found.hits;
    Ok(Json(SearchResults {
        scope,
        tracks: views::tracks(&st.db, &hits.tracks, user).await?,
        albums: views::albums(&st.db, &hits.albums, user).await?,
        artists: views::artists(&st.db, &hits.artists, user).await?,
        top: found.top,
        suggestion: found.suggestion,
    }))
}

#[utoipa::path(
    get,
    path = "/api/artists/{id}",
    tag = "catalog",
    security(("session" = []), ("device" = [])),
    params(("id" = Uuid, Path, description = "id артиста")),
    responses(
        (status = 200, description = "Карточка, популярное, дискография по типам, похожие, подписка", body = ArtistDetail),
        (status = 404, description = "Нет такого артиста", body = ErrorBody),
    ),
)]
pub async fn artist(
    State(st): State<AppState>,
    principal: Principal,
    Path(id): Path<Uuid>,
) -> AppResult<Json<ArtistDetail>> {
    catalog::ensure_artist(&st.db, &st.deezer, id).await?;
    let user = principal.user.id;
    let artist = views::artist(&st.db, id, user)
        .await?
        .ok_or(AppError::NotFound)?;
    let top_ids = sqlx::query_scalar!(
        "SELECT track_id FROM artist_top_tracks WHERE artist_id = $1 ORDER BY position",
        id
    )
    .fetch_all(&st.db)
    .await?;
    let release_ids = sqlx::query_scalar!(
        "SELECT al.id FROM artist_releases r JOIN albums al ON al.id = r.album_id
         WHERE r.artist_id = $1
         ORDER BY al.release_date DESC NULLS LAST, al.id DESC",
        id
    )
    .fetch_all(&st.db)
    .await?;
    let related_ids = sqlx::query_scalar!(
        "SELECT related_id FROM artist_related WHERE artist_id = $1 ORDER BY position",
        id
    )
    .fetch_all(&st.db)
    .await?;
    let mut releases = Releases::default();
    for album in views::albums(&st.db, &release_ids, user).await? {
        match album.record_type {
            RecordType::Album => releases.albums.push(album),
            RecordType::Ep => releases.eps.push(album),
            RecordType::Single => releases.singles.push(album),
            RecordType::Compile => releases.compilations.push(album),
        }
    }
    let follow = sqlx::query!(
        "SELECT download_policy, notify, created_at FROM followed_artists WHERE user_id = $1 AND artist_id = $2",
        user,
        id
    )
    .fetch_optional(&st.db)
    .await?
    .map(|f| FollowState {
        policy: serde_json::from_value(f.download_policy).unwrap_or_default(),
        notify: f.notify,
        followed_at: f.created_at,
    });
    let (tracks_on_server, tracks_total) = views::discography_progress(&st.db, &[id])
        .await?
        .remove(&id)
        .unwrap_or_default();
    Ok(Json(ArtistDetail {
        artist,
        top_tracks: views::tracks(&st.db, &top_ids, user).await?,
        releases,
        related: views::artists(&st.db, &related_ids, user).await?,
        follow,
        tracks_on_server,
        tracks_total,
    }))
}

#[utoipa::path(
    get,
    path = "/api/albums/{id}",
    tag = "catalog",
    security(("session" = []), ("device" = [])),
    params(("id" = Uuid, Path, description = "id альбома")),
    responses(
        (status = 200, description = "Альбом и треки по порядку", body = AlbumDetail),
        (status = 404, description = "Нет такого альбома", body = ErrorBody),
    ),
)]
pub async fn album(
    State(st): State<AppState>,
    principal: Principal,
    Path(id): Path<Uuid>,
) -> AppResult<Json<AlbumDetail>> {
    catalog::ensure_album(&st.db, &st.deezer, id).await?;
    let user = principal.user.id;
    let summary = views::album(&st.db, id, user)
        .await?
        .ok_or(AppError::NotFound)?;
    let extra = sqlx::query!(
        "SELECT upc, label, genres, fans FROM albums WHERE id = $1",
        id
    )
    .fetch_one(&st.db)
    .await?;
    let track_ids = sqlx::query_scalar!(
        "SELECT id FROM tracks WHERE album_id = $1 ORDER BY disc_number, track_position NULLS LAST, id",
        id
    )
    .fetch_all(&st.db)
    .await?;
    let tracks = views::tracks(&st.db, &track_ids, user).await?;
    Ok(Json(AlbumDetail {
        total_duration_ms: tracks.iter().map(|t| i64::from(t.duration_ms)).sum(),
        ready_tracks: tracks
            .iter()
            .filter(|t| t.audio_status == AudioStatus::Ready)
            .count() as i64,
        album: summary,
        upc: extra.upc,
        label: extra.label,
        genres: extra.genres,
        fans: extra.fans,
        tracks,
    }))
}

#[utoipa::path(
    get,
    path = "/api/tracks/{id}",
    tag = "catalog",
    security(("session" = []), ("device" = [])),
    params(("id" = Uuid, Path, description = "id трека")),
    responses(
        (status = 200, description = "Трек со всеми участниками", body = Track),
        (status = 404, description = "Нет такого трека", body = ErrorBody),
    ),
)]
pub async fn track(
    State(st): State<AppState>,
    principal: Principal,
    Path(id): Path<Uuid>,
) -> AppResult<Json<Track>> {
    catalog::ensure_track(&st.db, &st.deezer, id).await?;
    views::track(&st.db, id, principal.user.id)
        .await?
        .map(Json)
        .ok_or(AppError::NotFound)
}

#[utoipa::path(
    post,
    path = "/api/tracks/lookup",
    tag = "catalog",
    security(("session" = []), ("device" = [])),
    request_body = TrackLookup,
    responses(
        (status = 200, description = "Треки в том же порядке; неизвестные пропущены", body = TrackList),
        (status = 400, description = "Больше 500 id", body = ErrorBody),
    ),
)]
pub async fn lookup_tracks(
    State(st): State<AppState>,
    principal: Principal,
    Json(body): Json<TrackLookup>,
) -> AppResult<Json<TrackList>> {
    if body.ids.len() > MAX_LOOKUP {
        return Err(bad(format!("не больше {MAX_LOOKUP} id за раз")));
    }
    Ok(Json(TrackList {
        items: views::tracks(&st.db, &body.ids, principal.user.id).await?,
    }))
}

#[utoipa::path(
    get,
    path = "/api/catalog/deezer/{kind}/{deezerId}",
    tag = "catalog",
    security(("session" = []), ("device" = [])),
    params(
        ("kind" = DeezerKind, Path, description = "artist, album или track"),
        ("deezerId" = i64, Path, description = "id у Deezer"),
    ),
    responses(
        (status = 200, description = "Наша сущность; при первом обращении загружается из Deezer", body = CatalogEntity),
        (status = 404, description = "В Deezer такого нет", body = ErrorBody),
        (status = 503, description = "Deezer не ответил", body = ErrorBody),
    ),
)]
pub async fn resolve_deezer(
    State(st): State<AppState>,
    principal: Principal,
    Path((kind, deezer_id)): Path<(DeezerKind, i64)>,
) -> AppResult<Json<CatalogEntity>> {
    if deezer_id <= 0 {
        return Err(AppError::NotFound);
    }
    let user = principal.user.id;
    let entity = match kind {
        DeezerKind::Artist => {
            let id = catalog::resolve_artist(&st.db, &st.deezer, deezer_id).await?;
            CatalogEntity::Artist(
                views::artist(&st.db, id, user)
                    .await?
                    .ok_or(AppError::NotFound)?,
            )
        }
        DeezerKind::Album => {
            let id = catalog::resolve_album(&st.db, &st.deezer, deezer_id).await?;
            CatalogEntity::Album(
                views::album(&st.db, id, user)
                    .await?
                    .ok_or(AppError::NotFound)?,
            )
        }
        DeezerKind::Track => {
            let id = catalog::resolve_track(&st.db, &st.deezer, deezer_id).await?;
            CatalogEntity::Track(
                views::track(&st.db, id, user)
                    .await?
                    .ok_or(AppError::NotFound)?,
            )
        }
    };
    Ok(Json(entity))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_types() {
        assert_eq!(parse_types(None).unwrap(), Types::ALL);
        assert_eq!(parse_types(Some("")).unwrap(), Types::ALL);
        let t = parse_types(Some("track, artist")).unwrap();
        assert!(t.track && t.artist && !t.album);
        assert!(parse_types(Some("playlist")).is_err());
    }
}
