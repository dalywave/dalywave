//! Собранный интерфейс из `DALYWAVE_STATIC_DIR`. Маршруты SPA разбирает сам интерфейс, поэтому
//! незнакомый GET вне `/api` без расширения в последнем сегменте получает index.html.
//! Незнакомый файл (`/favicon.ico`, `/robots.txt`, всё под `/_nuxt/`) — 404.

use std::{convert::Infallible, path::Path};

use axum::{
    Router,
    body::Body,
    extract::Request,
    http::{HeaderValue, StatusCode, Uri, header},
    middleware,
    response::{IntoResponse, Response},
};
use tower::ServiceExt;
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
    // То же для любого файла: бот за /robots.txt или браузер за иконкой получили бы страницу под видом файла
    let app_route = tower::service_fn(move |req: Request| {
        let index = index.clone();
        async move {
            if looks_like_file(req.uri().path()) {
                return Ok::<_, Infallible>(StatusCode::NOT_FOUND.into_response());
            }
            let res = index.oneshot(req).await?;
            Ok(res.map(Body::new))
        }
    });
    let pages = ServeDir::new(dir)
        .precompressed_br()
        .precompressed_gzip()
        .fallback(app_route);
    Router::new()
        .nest_service("/_nuxt", assets)
        .fallback_service(pages)
        .layer(middleware::map_response(cache_headers))
}

/// Последний сегмент пути с расширением (`icon.png`, `.well-known/x.json`): это файл, а не экран интерфейса.
pub fn looks_like_file(path: &str) -> bool {
    let last = path.rsplit('/').next().unwrap_or_default();
    last.rsplit_once('.').is_some_and(|(_, ext)| {
        !ext.is_empty() && ext.len() <= 16 && ext.chars().all(|c| c.is_ascii_alphanumeric())
    })
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn files_are_told_from_app_routes() {
        for file in [
            "/favicon.ico",
            "/robots.txt",
            "/apple-touch-icon.png",
            "/.well-known/assetlinks.json",
            "/deep/path/app.webmanifest",
        ] {
            assert!(looks_like_file(file), "{file}");
        }
        for route in [
            "/",
            "/library",
            "/library/albums",
            "/artist/0193f0c2-7a1e-7c3a-9b1d-2f4e5a6b7c8d",
            "/v1.2/settings",
            "/search/",
            "/trailing.",
        ] {
            assert!(!looks_like_file(route), "{route}");
        }
    }
}
