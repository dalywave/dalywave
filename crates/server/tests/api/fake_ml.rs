//! Заглушка сервиса анализа (services/ml) на случайном локальном порту: отвечает как настоящий —
//! векторы в base64 от float16, темп, тональность, энергия. Может притворяться занятым (409 `busy`)
//! заданное число раз и отвечать ошибкой.

use std::sync::{Arc, Mutex};

use axum::{
    Json, Router,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
};
use base64::{Engine, engine::general_purpose::STANDARD};
use serde_json::{Value, json};

#[derive(Default)]
struct Inner {
    busy: usize,
    /// Ответ ошибкой: статус и код
    fail: Option<(u16, String)>,
    calls: Vec<Value>,
    texts: Vec<String>,
}

#[derive(Clone)]
pub struct FakeMl {
    pub base: String,
    inner: Arc<Mutex<Inner>>,
}

/// Вектор из float16: единица в измерениях `ones`, остальное — ноль.
pub fn f16_vector(dim: usize, ones: &[usize]) -> String {
    let bytes: Vec<u8> = (0..dim)
        .flat_map(|i| {
            let bits: u16 = if ones.contains(&i) { 0x3c00 } else { 0 };
            bits.to_le_bytes()
        })
        .collect();
    STANDARD.encode(bytes)
}

/// Измерение «звучания» по адресу файла: одинаковые адреса — одинаковые векторы.
fn axis(url: &str) -> usize {
    url.bytes()
        .fold(7usize, |h, b| h.wrapping_mul(31).wrapping_add(b as usize))
        % 1280
}

impl FakeMl {
    pub async fn start() -> Self {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let fake = Self {
            base,
            inner: Arc::new(Mutex::new(Inner::default())),
        };
        let app = Router::new()
            .route("/v1/analyze", post(analyze))
            .route("/v1/embed-text", post(embed))
            .route(
                "/v1/healthz",
                get(|| async { Json(json!({ "ok": true, "modelsLoaded": true, "busy": false })) }),
            )
            .with_state(fake.clone());
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        fake
    }

    /// Следующие `n` анализов получат 409 `busy`.
    pub fn busy(&self, n: usize) {
        self.inner.lock().unwrap().busy = n;
    }

    pub fn fail(&self, status: u16, code: &str) {
        self.inner.lock().unwrap().fail = Some((status, code.into()));
    }

    pub fn heal(&self) {
        self.inner.lock().unwrap().fail = None;
    }

    /// Тела запросов анализа, включая отклонённые.
    pub fn calls(&self) -> Vec<Value> {
        self.inner.lock().unwrap().calls.clone()
    }

    pub fn texts(&self) -> Vec<String> {
        self.inner.lock().unwrap().texts.clone()
    }
}

fn error(status: u16, code: &str) -> Response {
    (
        StatusCode::from_u16(status).unwrap(),
        Json(json!({ "error": { "code": code, "message": "заглушка" } })),
    )
        .into_response()
}

async fn analyze(State(f): State<FakeMl>, Json(body): Json<Value>) -> Response {
    let (busy, fail) = {
        let mut inner = f.inner.lock().unwrap();
        inner.calls.push(body.clone());
        let busy = inner.busy > 0;
        if busy {
            inner.busy -= 1;
        }
        (busy, inner.fail.clone())
    };
    if busy {
        return error(409, "busy");
    }
    if let Some((status, code)) = fail {
        return error(status, &code);
    }
    let url = body["url"].as_str().unwrap_or_default();
    let full = body["mode"] == "full";
    let a = axis(url);
    Json(json!({
        "source": if full { "full" } else { "preview" },
        "models": { "sound": "discogs-effnet-bsdynamic-1", "service": "fake" },
        "durationMs": if full { 180_000 } else { 30_000 },
        "sound": f16_vector(1280, &[a, (a + 1) % 1280]),
        "semantic": f16_vector(512, &[a % 512]),
        "bpm": 128.0, "bpmConfidence": 0.9, "keyCamelot": "8A", "key": "A minor", "keyConfidence": 0.7,
        "energy": 0.72, "danceability": 0.8, "valence": 0.55,
        "moods": { "happy": 0.4, "sad": 0.2, "aggressive": 0.3, "relaxed": 0.3, "party": 0.6 },
        "voice": 0.9, "language": null,
        "styles": [{ "name": "Hip Hop---Trap", "p": 0.4 }],
        "introEndMs": if full { json!(12_000) } else { Value::Null },
        "outroStartMs": if full { json!(170_000) } else { Value::Null },
        "beatsMs": if full { json!([20, 490, 960]) } else { Value::Null },
        "downbeatsMs": if full { json!([20]) } else { Value::Null },
        "vocalSegments": if full { json!([[12_000, 170_000]]) } else { Value::Null },
        "energyCurve": if full { json!([300, 700, 800]) } else { Value::Null },
        "extra": { "approachability": 0.6, "engagement": 0.7, "loudnessLufs": -9.5 },
        "timingsMs": { "total": 42 }
    }))
    .into_response()
}

async fn embed(State(f): State<FakeMl>, Json(body): Json<Value>) -> Response {
    let text = body["text"].as_str().unwrap_or_default().to_string();
    let fail = {
        let mut inner = f.inner.lock().unwrap();
        inner.texts.push(text.clone());
        inner.fail.clone()
    };
    if let Some((status, code)) = fail {
        return error(status, &code);
    }
    // «Спокойное» — в измерение 7, остальное — в 300
    let dim = if text.contains("спокойн") {
        7
    } else {
        300
    };
    Json(json!({
        "semantic": f16_vector(512, &[dim]),
        "dim": 512,
        "query": "calm music",
        "translated": true,
        "unknownWords": [],
        "model": "fake"
    }))
    .into_response()
}
