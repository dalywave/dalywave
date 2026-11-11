//! Собранный интерфейс из `DALYWAVE_STATIC_DIR`. Маршруты SPA разбирает сам интерфейс, поэтому
//! любой незнакомый GET вне `/api` получает index.html.

use std::path::Path;

use axum::{
    Router,
    http::{HeaderValue, StatusCode, Uri, header},
    middleware,
    response::Response,
};
use tower_http::services::{ServeDir, ServeFile};

/// Бандлы Nuxt с хешем в имени: содержимое по адресу никогда не меняется.
const IMMUTABLE: &str = "public, max-age=31536000, immutable";
/// Всё остальное каждый раз сверяется с сервером по ETag: после выкладки сразу новая версия.
const REVALIDATE: &str = "no-cache";
/// Скрипты и стили интерфейса здесь не ограничиваются: CSP только запрещает встраивать сайт в чужие фреймы.
pub const HTML_CSP: &str = "frame-ancestors 'none'; base-uri 'self'; object-src 'none'";

pub fn router(dir: &Path) -> Router {
    let index = ServeFile::new(dir.join("index.html"))
        .precompressed_br()
        .precompressed_gzip();
    // Неизвестный бандл — это 404, а не index.html: иначе браузер навсегда закеширует HTML под именем скрипта
    let assets = ServeDir::new(dir.join("_nuxt"))
        .precompressed_br()
        .precompressed_gzip();
    let pages = ServeDir::new(dir)
        .precompressed_br()
        .precompressed_gzip()
        .fallback(index);
    Router::new()
        .nest_service("/_nuxt", assets)
        .fallback_service(pages)
        .layer(middleware::map_response(cache_headers))
}

async fn cache_headers(uri: Uri, mut res: Response) -> Response {
    let found = res.status().is_success() || res.status() == StatusCode::NOT_MODIFIED;
    let headers = res.headers_mut();
    let html = headers
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.starts_with("text/html"));
    if uri.path().starts_with("/_nuxt/") && found {
        headers.insert(header::CACHE_CONTROL, HeaderValue::from_static(IMMUTABLE));
    } else {
        headers.insert(header::CACHE_CONTROL, HeaderValue::from_static(REVALIDATE));
    }
    if html {
        headers.insert(
            header::CONTENT_SECURITY_POLICY,
            HeaderValue::from_static(HTML_CSP),
        );
        headers.insert(header::X_FRAME_OPTIONS, HeaderValue::from_static("DENY"));
    }
    res
}
