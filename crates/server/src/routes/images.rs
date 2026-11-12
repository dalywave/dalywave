//! Обложки и фото: `/api/images/{albums|artists}/{id}/{250|500|1000}.jpg`. Отдаются из хранилища с
//! вечным кешем (их кеширует Cloudflare); пока копии нет — временный редирект на Deezer и задача
//! `mirror_image`. Адреса открыты без входа: это картинки каталога, а `<img>` в приложениях не шлёт токен.

use axum::{
    Router,
    body::Body,
    extract::State,
    http::{HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
    routing::get,
};
use dalywave_protocol::{ErrorBody, jobs::ImageOwner};
use uuid::Uuid;

use crate::{
    error::{AppError, AppResult},
    extract::Path,
    jobs::handlers::request_mirror,
    state::AppState,
    storage::{IMAGE_SIZES, IMMUTABLE, StorageError, keys},
};

pub fn routes() -> Router<AppState> {
    Router::new().route("/images/{kind}/{id}/{file}", get(image))
}

/// Адрес Deezer нужного размера: в имени файла у Deezer размер, например `/500x500-000000-80-0-0.jpg`.
pub fn sized_source(source: &str, size: u32) -> String {
    source.replace("/1000x1000-", &format!("/{size}x{size}-"))
}

#[utoipa::path(
    get,
    path = "/api/images/{kind}/{id}/{file}",
    tag = "catalog",
    params(
        ("kind" = String, Path, description = "albums, artists или playlists (своя обложка плейлиста)"),
        ("id" = Uuid, Path, description = "id альбома или артиста"),
        ("file" = String, Path, description = "250.jpg, 500.jpg или 1000.jpg"),
    ),
    responses(
        (status = 200, description = "JPEG; Cache-Control: public, max-age=31536000, immutable", content_type = "image/jpeg"),
        (status = 302, description = "Копии ещё нет: картинка у Deezer, копия поставлена в очередь"),
        (status = 404, description = "Нет такой картинки", body = ErrorBody),
    ),
)]
pub async fn image(
    State(st): State<AppState>,
    Path((kind, id, file)): Path<(String, Uuid, String)>,
) -> AppResult<Response> {
    let size: u32 = file
        .strip_suffix(".jpg")
        .and_then(|s| s.parse().ok())
        .filter(|s| IMAGE_SIZES.contains(s))
        .ok_or(AppError::NotFound)?;
    let owner = match kind.as_str() {
        "albums" => ImageOwner::Album,
        "artists" => ImageOwner::Artist,
        "playlists" => return playlist_cover(&st, id, size).await,
        _ => return Err(AppError::NotFound),
    };
    let row = match owner {
        ImageOwner::Album => sqlx::query!(
            "SELECT cover_key AS key, cover_source AS source FROM albums WHERE id = $1",
            id
        )
        .fetch_optional(&st.db)
        .await?
        .map(|r| (r.key, r.source)),
        ImageOwner::Artist => sqlx::query!(
            "SELECT picture_key AS key, picture_source AS source FROM artists WHERE id = $1",
            id
        )
        .fetch_optional(&st.db)
        .await?
        .map(|r| (r.key, r.source)),
    };
    let (key, source) = row.ok_or(AppError::NotFound)?;

    let mut force = false;
    if let Some(key) = key {
        match st.storage.get(&keys::image(&key, size)).await {
            Ok(object) => return Ok(jpeg(object)),
            // Запись в базе есть, а файла нет (хранилище почистили): копия делается заново
            Err(StorageError::NotFound) => force = true,
            Err(e) => tracing::warn!(%id, error = %e, "could not read an image from storage"),
        }
    }
    let source = source.ok_or(AppError::NotFound)?;
    if let Err(e) = request_mirror(&st.db, owner, id, force).await {
        tracing::warn!(%id, error = %e, "could not queue an image copy");
    }
    // Редирект не кешируется: как только копия появится, адрес должен отдавать её
    Ok((
        StatusCode::FOUND,
        [
            (header::LOCATION, sized_source(&source, size)),
            (header::CACHE_CONTROL, "no-store".to_string()),
        ],
    )
        .into_response())
}

fn jpeg(object: crate::storage::Object) -> Response {
    let mut res = Body::from_stream(object.stream).into_response();
    let headers = res.headers_mut();
    headers.insert(header::CONTENT_TYPE, HeaderValue::from_static("image/jpeg"));
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static(IMMUTABLE));
    headers.insert(header::CONTENT_LENGTH, HeaderValue::from(object.size));
    if let Some(etag) = object.e_tag.and_then(|e| HeaderValue::from_str(&e).ok()) {
        headers.insert(header::ETAG, etag);
    }
    res
}

/// Своя обложка плейлиста: источника вне хранилища у неё нет. Адрес открыт, как и у обложек
/// каталога (`<img>` в приложениях не шлёт токен), — угадать его нельзя: в нём id плейлиста.
async fn playlist_cover(st: &AppState, id: Uuid, size: u32) -> AppResult<Response> {
    let key = sqlx::query_scalar!("SELECT cover_key FROM playlists WHERE id = $1", id)
        .fetch_optional(&st.db)
        .await?
        .flatten()
        .ok_or(AppError::NotFound)?;
    match st.storage.get(&keys::image(&key, size)).await {
        Ok(object) => Ok(jpeg(object)),
        Err(StorageError::NotFound) => Err(AppError::NotFound),
        Err(e) => Err(AppError::Unavailable(format!("хранилище не ответило: {e}"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deezer_sizes() {
        let src = "https://cdn-images.dzcdn.net/images/cover/266b/1000x1000-000000-80-0-0.jpg";
        assert_eq!(
            sized_source(src, 250),
            "https://cdn-images.dzcdn.net/images/cover/266b/250x250-000000-80-0-0.jpg"
        );
        assert_eq!(sized_source(src, 1000), src);
    }
}
