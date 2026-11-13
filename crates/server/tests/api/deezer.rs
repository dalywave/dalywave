//! Заглушка Deezer на случайном локальном порту: отдаёт настоящие ответы, записанные в
//! `tests/fixtures/deezer`, картинки рисует сама. Адреса CDN в ответах подменяются на заглушку.

use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, Mutex},
};

use axum::{
    Router,
    extract::State,
    http::{StatusCode, Uri, header},
    response::{IntoResponse, Response},
};
use image::{DynamicImage, Rgb, RgbImage, codecs::jpeg::JpegEncoder};
use serde_json::{Value, json};

pub const LOWBORN: i64 = 13828465;
/// EP «Modern Sadness»: «Devil» и «sad. sad.» — те же ISRC, что в сингле «Devil».
pub const MODERN_SADNESS: i64 = 858280242;
pub const DEVIL_SINGLE: i64 = 856814942;
/// Сингл «Light Me Up»: Lowborn и приглашённая vista.
pub const LIGHT_ME_UP_ALBUM: i64 = 858280042;
pub const LIGHT_ME_UP: i64 = 3661045852;
pub const CHURCH_ON_SUNDAY: i64 = 3661046662;
/// «Лонг Айленд»: два основных артиста, MellSher и 5opka.
pub const LONG_ISLAND: i64 = 3005619421;

#[derive(Default)]
struct Inner {
    hits: HashMap<String, usize>,
    quota_errors: usize,
    overrides: HashMap<String, Value>,
}

#[derive(Clone)]
pub struct DeezerMock {
    pub base: String,
    inner: Arc<Mutex<Inner>>,
}

fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/deezer")
}

pub fn fixture(name: &str) -> Value {
    let text = std::fs::read_to_string(fixtures().join(format!("{name}.json")))
        .unwrap_or_else(|e| panic!("fixture {name}: {e}"));
    serde_json::from_str(&text).unwrap()
}

/// Обложка из трёх цветов, как в тесте палитры.
pub fn cover_jpeg(side: u32) -> Vec<u8> {
    let img = RgbImage::from_fn(side, side, |x, y| {
        if y < side / 10 {
            Rgb([250, 200, 30])
        } else if x < side / 2 {
            Rgb([200, 30, 60])
        } else {
            Rgb([20, 40, 160])
        }
    });
    let mut out = Vec::new();
    DynamicImage::ImageRgb8(img)
        .write_with_encoder(JpegEncoder::new_with_quality(&mut out, 90))
        .unwrap();
    out
}

impl DeezerMock {
    pub async fn start() -> Self {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let mock = Self {
            base,
            inner: Arc::new(Mutex::new(Inner::default())),
        };
        let app = Router::new().fallback(serve).with_state(mock.clone());
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        mock
    }

    /// Сколько раз спрашивали этот путь (без параметров).
    pub fn hits(&self, path: &str) -> usize {
        self.inner
            .lock()
            .unwrap()
            .hits
            .get(path)
            .copied()
            .unwrap_or(0)
    }

    pub fn total_hits(&self) -> usize {
        self.inner.lock().unwrap().hits.values().sum()
    }

    /// Следующие `n` запросов получат «Quota limit exceeded» с HTTP 200.
    pub fn quota_errors(&self, n: usize) {
        self.inner.lock().unwrap().quota_errors = n;
    }

    /// Свой ответ поиску с этой строкой (`q` без регистра) вместо записанного.
    pub fn set_search(&self, kind: &str, q: &str, body: Value) {
        self.set(&format!("/search/{kind}?q={}", q.to_lowercase()), body);
    }

    /// Свой ответ на путь вместо записанного.
    pub fn set(&self, path: &str, body: Value) {
        self.inner
            .lock()
            .unwrap()
            .overrides
            .insert(path.to_string(), body);
    }
}

fn no_data() -> Value {
    json!({ "error": { "type": "DataException", "message": "no data", "code": 800 } })
}

fn fixture_name(path: &str, query: &str) -> Option<String> {
    let parts: Vec<&str> = path.trim_matches('/').split('/').collect();
    let q = url::form_urlencoded::parse(query.as_bytes())
        .find(|(k, _)| k == "q")
        .map(|(_, v)| v.to_lowercase());
    match parts.as_slice() {
        ["artist", id] => Some(format!("artist_{id}")),
        ["artist", id, part @ ("top" | "albums" | "related")] => {
            Some(format!("artist_{id}_{part}"))
        }
        ["album", id] => Some(format!("album_{id}")),
        ["album", id, "tracks"] => Some(format!("album_{id}_tracks")),
        ["track", id] => Some(format!("track_{id}")),
        ["search", kind] => Some(format!("search_{kind}_{}", q?)),
        ["chart", "0"] => Some("chart_0".into()),
        ["chart", id, "tracks"] => Some(format!("chart_{id}_tracks")),
        ["playlist", id, "tracks"] => Some(format!("playlist_{id}_tracks")),
        _ => None,
    }
}

async fn serve(State(mock): State<DeezerMock>, uri: Uri) -> Response {
    let path = uri.path().to_string();
    let query = uri.query().unwrap_or("").to_string();
    let (quota, override_body) = {
        let mut inner = mock.inner.lock().unwrap();
        *inner.hits.entry(path.clone()).or_default() += 1;
        let quota = inner.quota_errors > 0;
        if quota {
            inner.quota_errors -= 1;
        }
        let q = url::form_urlencoded::parse(query.as_bytes())
            .find(|(k, _)| k == "q")
            .map(|(_, v)| v.to_lowercase());
        let by_query = q.and_then(|q| inner.overrides.get(&format!("{path}?q={q}")).cloned());
        (
            quota,
            by_query.or_else(|| inner.overrides.get(&path).cloned()),
        )
    };
    if quota {
        let body = json!({ "error": { "type": "Exception", "message": "Quota limit exceeded", "code": 4 } });
        return axum::Json(body).into_response();
    }
    if path.starts_with("/images/") {
        return (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "image/jpeg")],
            cover_jpeg(600),
        )
            .into_response();
    }
    // Вторая страница списков: в записанных ответах всё помещается в первую
    let later_page =
        url::form_urlencoded::parse(query.as_bytes()).any(|(k, v)| k == "index" && v != "0");
    let body = match override_body {
        Some(body) => body,
        None if later_page => json!({ "data": [], "total": 0 }),
        None => match fixture_name(&path, &query) {
            Some(name) if fixtures().join(format!("{name}.json")).exists() => fixture(&name),
            _ => no_data(),
        },
    };
    let text = body
        .to_string()
        .replace("https://cdn-images.dzcdn.net", &mock.base);
    ([(header::CONTENT_TYPE, "application/json")], text).into_response()
}
