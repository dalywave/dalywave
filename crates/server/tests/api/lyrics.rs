//! Тексты песен: LRCLIB (заглушка на локальном порту) → кеш `track_lyrics` → `GET /api/tracks/{id}/lyrics`;
//! «не нашли» на 30 дней, «без слов», 429, задача заранее и поиск по строчке.

use std::sync::{Arc, Mutex};

use axum::{
    Json, Router,
    extract::{Query, State},
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
    routing::get,
};
use serde_json::{Value, json};
use sqlx::PgPool;
use std::collections::HashMap;
use uuid::Uuid;

use crate::common::{Auth, TestApp};

#[derive(Default)]
struct Inner {
    /// Ответ `/api/get`: `None` — 404
    get: Option<Value>,
    search: Vec<Value>,
    /// Следующие ответы — 429
    limited: usize,
    calls: Vec<(String, HashMap<String, String>, String)>,
}

#[derive(Clone)]
struct FakeLrclib {
    base: String,
    inner: Arc<Mutex<Inner>>,
}

async fn handle(
    path: &'static str,
    fake: FakeLrclib,
    q: HashMap<String, String>,
    headers: HeaderMap,
) -> Response {
    let ua = headers
        .get(header::USER_AGENT)
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default()
        .to_string();
    let mut inner = fake.inner.lock().unwrap();
    inner.calls.push((path.into(), q, ua));
    if inner.limited > 0 {
        inner.limited -= 1;
        return (StatusCode::TOO_MANY_REQUESTS, [(header::RETRY_AFTER, "0")]).into_response();
    }
    match path {
        "get" => match inner.get.clone() {
            Some(v) => Json(v).into_response(),
            None => (
                StatusCode::NOT_FOUND,
                Json(json!({ "code": 404, "name": "TrackNotFound", "message": "Failed to find specified track" })),
            )
                .into_response(),
        },
        _ => Json(Value::Array(inner.search.clone())).into_response(),
    }
}

impl FakeLrclib {
    async fn start() -> Self {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let fake = Self {
            base: format!("http://{}", listener.local_addr().unwrap()),
            inner: Arc::new(Mutex::new(Inner::default())),
        };
        let app = Router::new()
            .route(
                "/api/get",
                get(
                    |State(f): State<FakeLrclib>,
                     Query(q): Query<HashMap<String, String>>,
                     h: HeaderMap| handle("get", f, q, h),
                ),
            )
            .route(
                "/api/search",
                get(
                    |State(f): State<FakeLrclib>,
                     Query(q): Query<HashMap<String, String>>,
                     h: HeaderMap| handle("search", f, q, h),
                ),
            )
            .with_state(fake.clone());
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        fake
    }

    fn set_get(&self, v: Option<Value>) {
        self.inner.lock().unwrap().get = v;
    }

    fn set_search(&self, v: Vec<Value>) {
        self.inner.lock().unwrap().search = v;
    }

    fn limit(&self, n: usize) {
        self.inner.lock().unwrap().limited = n;
    }

    fn calls(&self) -> Vec<(String, HashMap<String, String>, String)> {
        self.inner.lock().unwrap().calls.clone()
    }
}

fn record(id: i64, title: &str, duration: f64, synced: Option<&str>, plain: Option<&str>) -> Value {
    json!({
        "id": id,
        "name": title,
        "trackName": title,
        "artistName": "Lowborn",
        "albumName": "Light Me Up",
        "duration": duration,
        "instrumental": false,
        "plainLyrics": plain,
        "syncedLyrics": synced,
    })
}

/// Трек «Light Me Up» (193 с) артиста Lowborn с альбомом.
async fn seed(db: &PgPool, title: &str) -> Uuid {
    let artist: Uuid =
        sqlx::query_scalar("INSERT INTO artists (name) VALUES ('Lowborn') RETURNING id")
            .fetch_one(db)
            .await
            .unwrap();
    let album: Uuid = sqlx::query_scalar(
        "INSERT INTO albums (title, artist_id) VALUES ('Light Me Up', $1) RETURNING id",
    )
    .bind(artist)
    .fetch_one(db)
    .await
    .unwrap();
    let track: Uuid = sqlx::query_scalar(
        "INSERT INTO tracks (title, album_id, duration_ms) VALUES ($1, $2, 193000) RETURNING id",
    )
    .bind(title)
    .bind(album)
    .fetch_one(db)
    .await
    .unwrap();
    sqlx::query("INSERT INTO track_artists (track_id, artist_id) VALUES ($1, $2)")
        .bind(track)
        .bind(artist)
        .execute(db)
        .await
        .unwrap();
    track
}

async fn setup(db: PgPool) -> (TestApp, FakeLrclib, Auth) {
    let fake = FakeLrclib::start().await;
    let base = fake.base.clone();
    let app = TestApp::with(db, |c| c.lrclib.url = base).await;
    let auth = app.listener("lyrics-listener").await;
    (app, fake, auth)
}

const LRC: &str = "[ar:Lowborn]\n[00:00.00]\n[00:12.50]Standing face to face\n[00:16.00]<00:16.00>Ashes <00:17.00>in the air\n[00:30.00]\n[00:41.20]Fires burning blue";

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn synced_lyrics_are_fetched_once_and_cached(db: PgPool) {
    let (app, fake, auth) = setup(db.clone()).await;
    let track = seed(&db, "Light Me Up").await;
    fake.set_get(Some(record(
        7,
        "Light Me Up",
        193.4,
        Some(LRC),
        Some("Standing face to face\nAshes in the air\nFires burning blue"),
    )));

    let res = app.get(&format!("/api/tracks/{track}/lyrics"), &auth).await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    assert_eq!(res.body["kind"], "synced");
    assert_eq!(res.body["source"], "lrclib");
    assert_eq!(
        res.body["lines"],
        json!([
            { "tMs": 12500, "text": "Standing face to face" },
            { "tMs": 16000, "text": "Ashes in the air" },
            { "tMs": 30000, "text": "" },
            { "tMs": 41200, "text": "Fires burning blue" },
        ])
    );
    assert!(res.body["plain"].as_str().unwrap().starts_with("Standing"));
    assert!(
        res.headers[header::CACHE_CONTROL]
            .to_str()
            .unwrap()
            .contains("max-age=86400")
    );

    let calls = fake.calls();
    assert_eq!(calls.len(), 1);
    let (path, q, ua) = &calls[0];
    assert_eq!(path, "get");
    assert_eq!(q["track_name"], "Light Me Up");
    assert_eq!(q["artist_name"], "Lowborn");
    assert_eq!(q["album_name"], "Light Me Up");
    assert_eq!(q["duration"], "193");
    assert_eq!(ua, "DalyWave (wave.daly.rest)");

    // Второй раз — из кеша
    let again = app.get(&format!("/api/tracks/{track}/lyrics"), &auth).await;
    assert_eq!(again.body, res.body);
    assert_eq!(fake.calls().len(), 1);
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn search_fallback_checks_duration_and_prefers_synced(db: PgPool) {
    let (app, fake, auth) = setup(db.clone()).await;
    let track = seed(&db, "Light Me Up").await;
    // Точного совпадения нет; в поиске — запись на 4 с длиннее (не подходит), простой текст и синхронный
    fake.set_get(None);
    fake.set_search(vec![
        record(1, "Light Me Up", 197.5, Some("[00:01.00]wrong"), None),
        record(
            2,
            "Light Me Up (feat. VISTA)",
            193.0,
            None,
            Some("plain one"),
        ),
        record(
            3,
            "Light Me Up (Remix)",
            191.0,
            Some("[00:02.00]right"),
            Some("right"),
        ),
    ]);
    let res = app.get(&format!("/api/tracks/{track}/lyrics"), &auth).await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    assert_eq!(res.body["kind"], "synced");
    assert_eq!(res.body["lines"], json!([{ "tMs": 2000, "text": "right" }]));
    let paths: Vec<String> = fake.calls().into_iter().map(|c| c.0).collect();
    assert_eq!(paths, ["get", "search"]);

    // Только простой текст
    let other = seed(&db, "Light Me Up").await;
    fake.set_search(vec![record(
        2,
        "Light Me Up",
        193.0,
        None,
        Some("Line one\r\nLine two\n"),
    )]);
    let res = app.get(&format!("/api/tracks/{other}/lyrics"), &auth).await;
    assert_eq!(res.body["kind"], "plain");
    assert_eq!(res.body["lines"], json!([]));
    assert_eq!(res.body["plain"], "Line one\nLine two");
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn instrumental_and_not_found_are_remembered(db: PgPool) {
    let (app, fake, auth) = setup(db.clone()).await;
    let instrumental = seed(&db, "Light Me Up").await;
    let mut rec = record(9, "Light Me Up", 193.0, None, None);
    rec["instrumental"] = json!(true);
    fake.set_get(Some(rec));
    let res = app
        .get(&format!("/api/tracks/{instrumental}/lyrics"), &auth)
        .await;
    assert_eq!(res.body["kind"], "instrumental");
    assert_eq!(res.body["plain"], Value::Null);
    let (lines, plain): (Option<Value>, Option<String>) =
        sqlx::query_as("SELECT lines, plain FROM track_lyrics WHERE track_id = $1")
            .bind(instrumental)
            .fetch_one(&db)
            .await
            .unwrap();
    assert!(lines.is_none() && plain.is_none());

    // Не нашли: «не нашли» на 30 дней, повторный запрос LRCLIB не трогает
    let missing = seed(&db, "Light Me Up").await;
    fake.set_get(None);
    fake.set_search(Vec::new());
    let before = fake.calls().len();
    let res = app
        .get(&format!("/api/tracks/{missing}/lyrics"), &auth)
        .await;
    assert_eq!(res.status, StatusCode::OK);
    assert_eq!(res.body["kind"], "none");
    assert_eq!(res.body["source"], Value::Null);
    assert_eq!(fake.calls().len(), before + 2);
    let days: f64 = sqlx::query_scalar(
        "SELECT extract(epoch FROM not_found_until - now())::float8 / 86400 FROM track_lyrics WHERE track_id = $1",
    )
    .bind(missing)
    .fetch_one(&db)
    .await
    .unwrap();
    assert!((29.9..=30.1).contains(&days), "{days}");
    app.get(&format!("/api/tracks/{missing}/lyrics"), &auth)
        .await;
    assert_eq!(fake.calls().len(), before + 2);

    // Срок вышел — спрашиваем снова и находим
    sqlx::query(
        "UPDATE track_lyrics SET not_found_until = now() - interval '1 minute' WHERE track_id = $1",
    )
    .bind(missing)
    .execute(&db)
    .await
    .unwrap();
    fake.set_get(Some(record(
        10,
        "Light Me Up",
        193.0,
        None,
        Some("found later"),
    )));
    let res = app
        .get(&format!("/api/tracks/{missing}/lyrics"), &auth)
        .await;
    assert_eq!(res.body["kind"], "plain");
    assert_eq!(res.body["plain"], "found later");
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn rate_limit_is_temporary_and_unknown_track_is_404(db: PgPool) {
    let (app, fake, auth) = setup(db.clone()).await;
    let track = seed(&db, "Light Me Up").await;
    // Короткая пауза пережидается и запрос повторяется один раз; второй 429 — «повторите позже»
    fake.limit(2);
    let res = app.get(&format!("/api/tracks/{track}/lyrics"), &auth).await;
    assert_eq!(res.status, StatusCode::SERVICE_UNAVAILABLE, "{}", res.text);
    assert_eq!(res.code(), "unavailable");
    let cached: i64 = sqlx::query_scalar("SELECT count(*) FROM track_lyrics")
        .fetch_one(&db)
        .await
        .unwrap();
    assert_eq!(cached, 0);

    fake.limit(1);
    fake.set_get(Some(record(
        7,
        "Light Me Up",
        193.0,
        None,
        Some("after a pause"),
    )));
    let res = app.get(&format!("/api/tracks/{track}/lyrics"), &auth).await;
    assert_eq!(res.body["kind"], "plain", "{}", res.text);

    let res = app
        .get(&format!("/api/tracks/{}/lyrics", Uuid::now_v7()), &auth)
        .await;
    assert_eq!(res.status, StatusCode::NOT_FOUND);

    let res = app
        .get(&format!("/api/tracks/{track}/lyrics"), &Auth::None)
        .await;
    assert_eq!(res.status, StatusCode::UNAUTHORIZED);
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn prefetch_job_fills_the_cache(db: PgPool) {
    let (app, fake, _auth) = setup(db.clone()).await;
    let first = seed(&db, "Light Me Up").await;
    let second = seed(&db, "Light Me Up").await;
    let done = seed(&db, "Light Me Up").await;
    sqlx::query("INSERT INTO track_lyrics (track_id, plain) VALUES ($1, 'already')")
        .bind(done)
        .execute(&db)
        .await
        .unwrap();
    fake.set_get(Some(record(7, "Light Me Up", 193.0, Some(LRC), None)));

    let created = dalywave_server::lyrics::prefetch(&db, &[first, done, second, Uuid::now_v7()])
        .await
        .unwrap();
    assert_eq!(created, 2);
    // Повтор, пока задачи в очереди, ничего не добавляет
    let again = dalywave_server::lyrics::prefetch(&db, &[first, second])
        .await
        .unwrap();
    assert_eq!(again, 0);

    let (kind, _) = app.run_job("lyrics").await.expect("a lyrics job");
    assert_eq!(kind, "lyrics");
    let synced: bool = sqlx::query_scalar("SELECT synced FROM track_lyrics WHERE track_id = $1")
        .bind(first)
        .fetch_one(&db)
        .await
        .unwrap();
    assert!(synced);
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn search_by_line(db: PgPool) {
    let (app, _fake, auth) = setup(db.clone()).await;
    let track = seed(&db, "Ёжик").await;
    sqlx::query(
        "INSERT INTO track_lyrics (track_id, plain) VALUES ($1, 'Первая строка\nЁжик в тумане идёт\nпоследняя')",
    )
    .bind(track)
    .execute(&db)
    .await
    .unwrap();

    let res = app
        .get("/api/lyrics/search?q=ежик%20в%20тумане", &auth)
        .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    let items = res.body["items"].as_array().unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["track"]["id"], track.to_string());
    assert_eq!(items[0]["line"], "Ёжик в тумане идёт");
    // Не в той раскладке: «t;br d nevfyt»
    let res = app
        .get("/api/lyrics/search?q=t%3Bbr%20d%20nevfyt", &auth)
        .await;
    assert_eq!(res.body["items"][0]["line"], "Ёжик в тумане идёт");

    let res = app.get("/api/lyrics/search?q=100%25", &auth).await;
    assert_eq!(res.body["items"], json!([]));
    let res = app.get("/api/lyrics/search?q=ой", &auth).await;
    assert_eq!(res.status, StatusCode::BAD_REQUEST);
}
