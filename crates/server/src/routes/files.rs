//! Файлы по подписанным ссылкам API (`storage::links`): аудио, обложки и прочее из хранилища на диске и
//! своя музыка из папок на сервере. Вход не нужен — ссылку подписал сервер, она короткая. Отдача с Range
//! (перемотка в `<audio>`, AVPlayer), загрузка `PUT` — для загрузчика и выпусков, когда хранилище — диск.

use std::ops::Range;

use axum::{
    Router,
    body::Body,
    extract::{DefaultBodyLimit, Request, State},
    http::{HeaderMap, HeaderValue, Method, StatusCode, header},
    response::{IntoResponse, Response},
    routing::get,
};
use futures_util::TryStreamExt;
use serde::Deserialize;
use tokio::io::AsyncWriteExt;

use crate::{
    error::{AppError, AppResult},
    extract::{Path, Query},
    state::AppState,
    storage::{StorageError, mime_of},
};

pub fn routes() -> Router<AppState> {
    Router::new().route(
        "/files/{*key}",
        get(serve)
            .head(serve)
            .put(upload)
            .layer(DefaultBodyLimit::disable()),
    )
}

#[derive(Debug, Deserialize)]
pub struct FileQuery {
    e: u64,
    s: String,
    dl: Option<String>,
}

/// Кусок из заголовка Range: `bytes=a-b`, `bytes=a-`, `bytes=-n`. Несколько кусков не поддерживаются —
/// тогда отдаётся весь файл (так разрешает RFC 9110).
pub fn parse_range(value: &str, size: u64) -> Option<Result<Range<u64>, ()>> {
    let spec = value.trim().strip_prefix("bytes=")?;
    if spec.contains(',') {
        return None;
    }
    let (a, b) = spec.split_once('-')?;
    let (a, b) = (a.trim(), b.trim());
    let range = if a.is_empty() {
        let n: u64 = b.parse().ok()?;
        if n == 0 {
            return Some(Err(()));
        }
        size.saturating_sub(n)..size
    } else {
        let start: u64 = a.parse().ok()?;
        let end = if b.is_empty() {
            size
        } else {
            b.parse::<u64>().ok()?.saturating_add(1).min(size)
        };
        if start >= size || end <= start {
            return Some(Err(()));
        }
        start..end
    };
    Some(Ok(range))
}

fn check(
    st: &AppState,
    method: &str,
    key: &str,
    q: &FileQuery,
) -> AppResult<crate::storage::Signed> {
    st.storage
        .links()
        .verify(method, key, q.e, q.dl.as_deref(), &q.s)
        .ok_or_else(|| AppError::Forbidden("ссылка на файл неверна или устарела".into()))
}

fn storage_error(e: StorageError) -> AppError {
    match e {
        StorageError::NotFound | StorageError::Key(_) => AppError::NotFound,
        e => AppError::Unavailable(format!("хранилище не ответило: {e}")),
    }
}

/// `Content-Disposition` с именем файла: `filename*` в UTF-8 для кириллицы, `filename` — запасное.
fn disposition(name: &str) -> HeaderValue {
    let ascii: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_graphic() && c != '"' && c != '\\' || c == ' ' {
                c
            } else {
                '_'
            }
        })
        .collect();
    let encoded: String = url::form_urlencoded::byte_serialize(name.as_bytes())
        .collect::<String>()
        .replace('+', "%20");
    HeaderValue::from_str(&format!(
        "attachment; filename=\"{ascii}\"; filename*=UTF-8''{encoded}"
    ))
    .unwrap_or_else(|_| HeaderValue::from_static("attachment"))
}

async fn serve(
    State(st): State<AppState>,
    method: Method,
    Path(key): Path<String>,
    Query(q): Query<FileQuery>,
    headers: HeaderMap,
) -> AppResult<Response> {
    let signed = check(&st, "GET", &key, &q)?;
    let meta = st
        .storage
        .head(&key)
        .await
        .map_err(storage_error)?
        .ok_or(AppError::NotFound)?;
    let range = match headers.get(header::RANGE).and_then(|v| v.to_str().ok()) {
        Some(v) => match parse_range(v, meta.size) {
            Some(Ok(r)) => Some(r),
            Some(Err(())) => {
                return Ok((
                    StatusCode::RANGE_NOT_SATISFIABLE,
                    [(header::CONTENT_RANGE, format!("bytes */{}", meta.size))],
                )
                    .into_response());
            }
            None => None,
        },
        None => None,
    };
    if method == Method::HEAD {
        let mut res = StatusCode::OK.into_response();
        let h = res.headers_mut();
        h.insert(header::CONTENT_LENGTH, HeaderValue::from(meta.size));
        h.insert(header::ACCEPT_RANGES, HeaderValue::from_static("bytes"));
        if let Ok(v) = HeaderValue::from_str(mime_of(&key)) {
            h.insert(header::CONTENT_TYPE, v);
        }
        return Ok(res);
    }
    let object = st
        .storage
        .get_range(&key, range)
        .await
        .map_err(storage_error)?;
    let size = object.size;
    let content_type = object
        .content_type
        .clone()
        .unwrap_or_else(|| mime_of(&key).to_string());
    let e_tag = object.e_tag.clone();
    let part = object.range.clone();
    let body = Body::from_stream(object.stream.map_err(std::io::Error::other));
    let mut res = body.into_response();
    let h = res.headers_mut();
    match part {
        Some(r) => {
            *res.status_mut() = StatusCode::PARTIAL_CONTENT;
            let h = res.headers_mut();
            h.insert(header::CONTENT_LENGTH, HeaderValue::from(r.end - r.start));
            if let Ok(v) = HeaderValue::from_str(&format!(
                "bytes {}-{}/{size}",
                r.start,
                r.end.saturating_sub(1)
            )) {
                h.insert(header::CONTENT_RANGE, v);
            }
        }
        None => {
            h.insert(header::CONTENT_LENGTH, HeaderValue::from(size));
        }
    }
    let h = res.headers_mut();
    h.insert(header::ACCEPT_RANGES, HeaderValue::from_static("bytes"));
    if let Ok(v) = HeaderValue::from_str(&content_type) {
        h.insert(header::CONTENT_TYPE, v);
    }
    // Ссылка подписана на часы: в кеше браузера она живёт столько же, в общих кешах — нет
    h.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("private, max-age=3600"),
    );
    if let Some(tag) = e_tag.and_then(|t| HeaderValue::from_str(&t).ok()) {
        h.insert(header::ETAG, tag);
    }
    if let Some(name) = signed.download {
        h.insert(header::CONTENT_DISPOSITION, disposition(&name));
    }
    Ok(res)
}

async fn upload(
    State(st): State<AppState>,
    Path(key): Path<String>,
    Query(q): Query<FileQuery>,
    req: Request,
) -> AppResult<StatusCode> {
    let signed = check(&st, "PUT", &key, &q)?;
    if !signed.put {
        return Err(AppError::Forbidden("ссылка только для чтения".into()));
    }
    let content_type = req
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or_else(|| mime_of(&key))
        .to_string();
    let tmp = tempfile::NamedTempFile::new()
        .map_err(|e| AppError::Unavailable(format!("временный файл: {e}")))?;
    let mut file = tokio::fs::File::create(tmp.path())
        .await
        .map_err(|e| AppError::Unavailable(format!("временный файл: {e}")))?;
    let mut body = req.into_body().into_data_stream();
    while let Some(chunk) = body
        .try_next()
        .await
        .map_err(|e| AppError::BadRequest(format!("загрузка прервалась: {e}")))?
    {
        file.write_all(&chunk)
            .await
            .map_err(|e| AppError::Unavailable(format!("временный файл: {e}")))?;
    }
    file.flush()
        .await
        .map_err(|e| AppError::Unavailable(format!("временный файл: {e}")))?;
    drop(file);
    st.storage
        .put_file(&key, tmp.path(), &content_type)
        .await
        .map_err(|e| match e {
            StorageError::ReadOnly => AppError::Forbidden("этот файл только для чтения".into()),
            e => storage_error(e),
        })?;
    Ok(StatusCode::OK)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ranges() {
        assert_eq!(parse_range("bytes=0-99", 1000), Some(Ok(0..100)));
        assert_eq!(parse_range("bytes=500-", 1000), Some(Ok(500..1000)));
        assert_eq!(parse_range("bytes=-200", 1000), Some(Ok(800..1000)));
        assert_eq!(parse_range("bytes=900-5000", 1000), Some(Ok(900..1000)));
        assert_eq!(parse_range("bytes=1000-", 1000), Some(Err(())));
        assert_eq!(parse_range("bytes=-0", 1000), Some(Err(())));
        assert_eq!(parse_range("bytes=0-1,5-6", 1000), None);
        assert_eq!(parse_range("items=0-1", 1000), None);
        assert_eq!(parse_range("bytes=x-1", 1000), None);
    }

    #[test]
    fn download_names() {
        let v = disposition("Кино — Группа крови.flac");
        let s = v.to_str().unwrap();
        assert!(s.starts_with("attachment; filename=\""), "{s}");
        assert!(s.contains("filename*=UTF-8''%D0%9A"), "{s}");
    }
}
