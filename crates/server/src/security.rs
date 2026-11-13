//! Защита API от подделки запросов: изменяющий запрос с cookie-сессией должен прийти с нашего
//! Origin, а тело — быть JSON (простые формы браузер шлёт с чужих сайтов без preflight).
//! CORS открыт только WebView приложений Tauri: они ходят с токеном `dwd_…` в заголовке, без cookie.

use std::time::Duration;

use axum::{
    body::HttpBody,
    extract::{Request, State},
    http::{HeaderMap, HeaderValue, Method, header},
    middleware::Next,
    response::{IntoResponse, Response},
};
use axum_extra::extract::cookie::CookieJar;
use tower_http::cors::{AllowOrigin, CorsLayer};

use crate::{auth, config::Config, error::AppError, session::SESSION_COOKIE, state::AppState};

/// WebView приложений Tauri: macOS и iOS, Windows и Android.
pub const TAURI_ORIGINS: [&str; 3] = [
    "tauri://localhost",
    "http://tauri.localhost",
    "https://tauri.localhost",
];
/// Тела API — небольшие JSON; большие загрузки получат свой лимит на своём маршруте.
pub const API_BODY_LIMIT: usize = 1024 * 1024;

pub fn origin_allowed(config: &Config, origin: &str) -> bool {
    TAURI_ORIGINS.contains(&origin) || origin == config.public_origin()
}

/// CORS на `/api` для WebView Tauri (и сайта). Приложения входят токеном в заголовке, а не cookie,
/// поэтому credentials не разрешаются: чужая страница не получит ответ с cookie-сессией.
pub fn cors(config: &Config) -> CorsLayer {
    let origins: Vec<HeaderValue> = TAURI_ORIGINS
        .iter()
        .map(|o| o.to_string())
        .chain([config.public_origin()])
        .filter_map(|o| HeaderValue::from_str(&o).ok())
        .collect();
    CorsLayer::new()
        .allow_origin(AllowOrigin::list(origins))
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::PATCH,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_headers([header::AUTHORIZATION, header::CONTENT_TYPE])
        .max_age(Duration::from_secs(86400))
}

/// Слой на всё `/api`.
pub async fn guard(State(st): State<AppState>, req: Request, next: Next) -> Response {
    if !req.method().is_safe() {
        // Длину знает и само тело: внутри процесса Content-Length может не быть
        let body = HttpBody::size_hint(req.body()).lower() > 0;
        // Без тела проверять тип нечего: DELETE обложки идёт как обычный запрос
        let raw = (body || has_body(req.headers())) && raw_upload(req.uri().path());
        if let Err(e) = check_unsafe(&st.config, req.headers(), body && !raw, raw) {
            return e.into_response();
        }
    }
    next.run(req).await
}

/// Загрузки файлов импорта: тело — сам файл. Такой запрос (не форма и не text/plain) браузер шлёт
/// только после preflight, а CORS для чужих сайтов закрыт; Origin проверяется как обычно.
fn raw_upload(path: &str) -> bool {
    let path = path.strip_prefix("/api").unwrap_or(path);
    // Своя обложка плейлиста — тоже сам файл картинки
    if let Some(id) = path
        .strip_prefix("/playlists/")
        .and_then(|r| r.strip_suffix("/cover"))
    {
        return !id.is_empty() && !id.contains('/');
    }
    // Видео истории без звука — сервер добавляет звук (story.rs)
    if let Some(id) = path
        .strip_prefix("/tracks/")
        .and_then(|r| r.strip_suffix("/story"))
    {
        return !id.is_empty() && !id.contains('/');
    }
    let Some(rest) = path.strip_prefix("/imports/") else {
        return false;
    };
    rest == "spotify"
        || rest == "youtube/takeout"
        || rest
            .strip_suffix("/files")
            .is_some_and(|id| !id.contains('/'))
}

/// Типы тела, которые форма с чужого сайта может отправить без preflight.
fn simple_content_type(headers: &HeaderMap) -> bool {
    let mime = headers
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .split(';')
        .next()
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase();
    matches!(
        mime.as_str(),
        "" | "text/plain" | "application/x-www-form-urlencoded" | "multipart/form-data"
    )
}

fn check_unsafe(
    config: &Config,
    headers: &HeaderMap,
    body: bool,
    raw: bool,
) -> Result<(), AppError> {
    // Bearer-токен чужая страница подставить не может: такой запрос не подделать
    if auth::bearer(headers).is_none() {
        let with_cookie = CookieJar::from_headers(headers)
            .get(SESSION_COOKIE)
            .is_some();
        match headers
            .get(header::ORIGIN)
            .map(|o| o.to_str().unwrap_or_default())
        {
            Some(origin) if !origin_allowed(config, origin) => return Err(AppError::BadOrigin),
            // Браузер всегда шлёт Origin с изменяющими запросами; без него cookie не принимаем
            None if with_cookie => return Err(AppError::BadOrigin),
            _ => {}
        }
    }
    if raw {
        if simple_content_type(headers) && auth::bearer(headers).is_none() {
            return Err(AppError::UnsupportedMediaType);
        }
        return Ok(());
    }
    if (body || has_body(headers)) && !is_json(headers) {
        return Err(AppError::UnsupportedMediaType);
    }
    Ok(())
}

fn has_body(headers: &HeaderMap) -> bool {
    let length = headers
        .get(header::CONTENT_LENGTH)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<u64>().ok());
    length.is_some_and(|n| n > 0) || headers.contains_key(header::TRANSFER_ENCODING)
}

fn is_json(headers: &HeaderMap) -> bool {
    let Some(value) = headers
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
    else {
        return false;
    };
    let mime = value
        .split(';')
        .next()
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase();
    mime == "application/json" || (mime.starts_with("application/") && mime.ends_with("+json"))
}

#[cfg(test)]
mod tests {
    use axum::http::HeaderValue;

    use super::*;

    fn config() -> Config {
        Config {
            database_url: String::new(),
            listen: "127.0.0.1:0".parse().unwrap(),
            public_url: "https://wave.daly.rest".into(),
            secret_key: [0; 32],
            static_dir: None,
            oidc: None,
            admin_emails: Vec::new(),
            admin_password: None,
            s3: None,
            deezer: Default::default(),
            fetcher: Default::default(),
            ml: Default::default(),
            lrclib: Default::default(),
            yandex_music: Default::default(),
            info: Default::default(),
            release_token: None,
            push: Default::default(),
        }
    }

    fn headers(pairs: &[(&'static str, &'static str)]) -> HeaderMap {
        pairs
            .iter()
            .map(|(k, v)| {
                (
                    header::HeaderName::from_static(k),
                    HeaderValue::from_static(v),
                )
            })
            .collect()
    }

    #[test]
    fn origins() {
        let c = config();
        assert!(origin_allowed(&c, "https://wave.daly.rest"));
        assert!(origin_allowed(&c, "tauri://localhost"));
        assert!(!origin_allowed(&c, "https://wave.daly.rest.evil.example"));
        assert!(!origin_allowed(&c, "null"));
    }

    #[test]
    fn raw_uploads() {
        let c = config();
        assert!(raw_upload("/imports/spotify"));
        assert!(raw_upload("/api/imports/youtube/takeout"));
        assert!(!raw_upload("/api/imports/youtube"));
        assert!(raw_upload("/api/imports/0193/files"));
        assert!(!raw_upload("/imports/list"));
        assert!(raw_upload("/api/playlists/0193/cover"));
        assert!(!raw_upload("/playlists//cover"));
        assert!(!raw_upload("/playlists/a/b/cover"));
        assert!(!raw_upload("/imports/a/b/files"));
        let cookie = ("cookie", "dw_session=dws_x");
        let origin = ("origin", "https://wave.daly.rest");
        let zip = ("content-type", "application/zip");
        assert!(check_unsafe(&c, &headers(&[cookie, origin, zip]), false, true).is_ok());
        // Форма с чужого сайта не пройдёт ни без Origin, ни с простым типом
        assert!(check_unsafe(&c, &headers(&[cookie, zip]), false, true).is_err());
        let form = ("content-type", "multipart/form-data; boundary=x");
        assert!(check_unsafe(&c, &headers(&[cookie, origin, form]), false, true).is_err());
    }

    #[test]
    fn cookie_requests_need_our_origin() {
        let c = config();
        let cookie = ("cookie", "dw_session=dws_x");
        let json = ("content-type", "application/json");
        let body = ("content-length", "2");
        assert!(
            check_unsafe(
                &c,
                &headers(&[cookie, json, body, ("origin", "https://wave.daly.rest")]),
                false,
                false
            )
            .is_ok()
        );
        assert!(
            check_unsafe(&c, &headers(&[cookie, json, body]), false, false).is_err(),
            "без Origin"
        );
        assert!(
            check_unsafe(
                &c,
                &headers(&[cookie, ("origin", "https://evil.example")]),
                false,
                false
            )
            .is_err()
        );
        // Приложение с токеном и без cookie
        assert!(
            check_unsafe(
                &c,
                &headers(&[("authorization", "Bearer dwd_x"), json, body]),
                false,
                false
            )
            .is_ok()
        );
        // …в том числе из WebView Tauri
        for origin in TAURI_ORIGINS {
            assert!(
                check_unsafe(
                    &c,
                    &headers(&[
                        ("authorization", "Bearer dwd_x"),
                        ("origin", origin),
                        json,
                        body
                    ]),
                    false,
                    false
                )
                .is_ok(),
                "{origin}"
            );
        }
        // Обмен кода приложения: ни cookie, ни Origin
        assert!(check_unsafe(&c, &headers(&[json, body]), false, false).is_ok());
    }

    #[test]
    fn bodies_must_be_json() {
        let c = config();
        let origin = ("origin", "https://wave.daly.rest");
        let form = headers(&[
            origin,
            ("content-type", "text/plain"),
            ("content-length", "5"),
        ]);
        assert!(check_unsafe(&c, &form, false, false).is_err());
        let empty = headers(&[origin, ("content-length", "0")]);
        assert!(check_unsafe(&c, &empty, false, false).is_ok());
        // Тело без заголовков длины всё равно проверяется
        assert!(check_unsafe(&c, &headers(&[origin]), true, false).is_err());
        let chunked = headers(&[origin, ("transfer-encoding", "chunked")]);
        assert!(check_unsafe(&c, &chunked, false, false).is_err());
        let patch = headers(&[
            origin,
            ("content-type", "application/merge-patch+json"),
            ("content-length", "5"),
        ]);
        assert!(check_unsafe(&c, &patch, false, false).is_ok());
    }
}
