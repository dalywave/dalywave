//! Видео без звука: клипы ленты открытий (задания загрузчику, отчёты, чистка) и живые обложки
//! альбомов (поддельный Apple Music API и HLS, собранный ffmpeg на лету).

use std::{path::PathBuf, sync::Arc};

use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::get,
};
use dalywave_server::{config::MusicKitConfig, jobs::worker::Outcome, video::clips};
use serde_json::{Value, json};
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    common::{Auth, Resp, TestApp},
    deezer::LIGHT_ME_UP,
    fetcher::{ep_tracks, new_downloader, no_intervals, upload_key},
};

async fn hello_claim(app: &TestApp, token: &str) -> Resp {
    app.post(
        "/api/fetcher/jobs/claim",
        json!({ "workerId": "test-mac", "version": "0.3.0",
                "capabilities": { "kinds": ["acquire", "clip"], "ytDlp": "2026.10.26" } }),
        &Auth::Bearer(token.into()),
    )
    .await
}

async fn claim_clip(app: &TestApp, token: &str) -> Resp {
    app.post(
        "/api/fetcher/clips/claim",
        json!({ "workerId": "test-mac" }),
        &Auth::Bearer(token.into()),
    )
    .await
}

async fn report_clip(app: &TestApp, token: &str, id: &str, result: Value) -> Resp {
    app.post(
        &format!("/api/fetcher/clips/{id}/result"),
        json!({ "workerId": "test-mac", "result": result }),
        &Auth::Bearer(token.into()),
    )
    .await
}

async fn clips_of(app: &TestApp, auth: &Auth, ids: &[&str]) -> Vec<Value> {
    let res = app
        .post("/api/clips", json!({ "trackIds": ids }), auth)
        .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    assert_eq!(res.headers["cache-control"], "no-store");
    res.body["items"].as_array().unwrap().clone()
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn clip_is_found_by_the_downloader_and_shown_in_the_feed(db: PgPool) {
    let app = TestApp::new(db).await;
    no_intervals(&app.state.db).await;
    let (_, admin) = app.admin("boss").await;
    let (token, _) = new_downloader(&app, &admin, "Mac").await;
    let me = app.listener("ann").await;
    let track = app.resolve_id("track", LIGHT_ME_UP, &me).await;

    // Без клипов в очереди загрузчик со способностью «клип» ждёт как обычно
    let res = hello_claim(&app, &token).await;
    assert_eq!(res.status, StatusCode::NO_CONTENT);
    assert_eq!(res.headers["retry-after"], "1");
    assert_eq!(
        claim_clip(&app, &token).await.status,
        StatusCode::NO_CONTENT
    );

    // Лента просит видео: клипа ещё нет — ищется; ключа MusicKit нет — живой обложки нет
    let items = clips_of(&app, &me, &[&track]).await;
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["state"], "pending");
    assert!(items[0]["url"].is_null());
    assert_eq!(items[0]["motion"]["state"], "none");
    // Повторный запрос не ставит вторую задачу
    clips_of(&app, &me, &[&track]).await;
    let queued: i64 =
        sqlx::query_scalar("SELECT count(*) FROM jobs WHERE kind = 'clip' AND status = 'queued'")
            .fetch_one(&app.state.db)
            .await
            .unwrap();
    assert_eq!(queued, 1);

    // Клип ждёт: запрос задания скачивания отвечает сразу, загрузчик берёт клип
    let res = hello_claim(&app, &token).await;
    assert_eq!(res.status, StatusCode::NO_CONTENT);
    assert_eq!(res.headers["retry-after"], "0");
    // Старый загрузчик (только acquire) о клипах не знает и ждёт как раньше
    let old = app
        .post(
            "/api/fetcher/jobs/claim",
            json!({ "workerId": "old-mac", "capabilities": { "kinds": ["acquire"] } }),
            &Auth::Bearer(token.clone()),
        )
        .await;
    assert_eq!(old.headers["retry-after"], "1");

    let res = claim_clip(&app, &token).await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    let job = res.body["job"].clone();
    assert_eq!(job["trackId"], track.as_str());
    assert_eq!(job["title"], "Light Me Up");
    assert_eq!(job["artists"], json!(["Lowborn", "vista"]));
    assert_eq!(job["lengthMs"], 12_000);
    assert_eq!(job["excludeVideoIds"], json!([]));
    let duration = job["durationMs"].as_i64().unwrap();
    // Анализа нет — 30 % трека
    assert_eq!(job["hookStartMs"].as_i64().unwrap(), duration * 3 / 10);
    assert_eq!(job["upload"]["method"], "PUT");
    assert_eq!(job["upload"]["headers"]["Content-Type"], "video/mp4");
    let key = upload_key(job["upload"]["url"].as_str().unwrap());
    assert!(key.starts_with(&format!("video/clips/{track}/")), "{key}");
    // Клип в работе: следующего нет, и запрос скачивания снова ждёт
    assert_eq!(
        claim_clip(&app, &token).await.status,
        StatusCode::NO_CONTENT
    );
    assert_eq!(hello_claim(&app, &token).await.headers["retry-after"], "1");

    // Отчёт «готово» без файла не принимается: задание вернётся в очередь
    let id = job["id"].as_str().unwrap();
    let ready = json!({ "status": "ready", "videoId": "dQw4w9WgXcQ", "videoDurationMs": 201_000,
                        "clipStartMs": 75_000, "durationMs": 12_000, "sizeBytes": 3, "width": 854,
                        "height": 480, "details": { "candidates": 3 } });
    assert_eq!(
        report_clip(&app, &token, id, ready.clone()).await.status,
        StatusCode::NO_CONTENT
    );
    let status: String = sqlx::query_scalar("SELECT status FROM jobs WHERE id = $1::uuid")
        .bind(id)
        .fetch_one(&app.state.db)
        .await
        .unwrap();
    assert_eq!(status, "queued");
    sqlx::query("UPDATE jobs SET run_after = now() WHERE id = $1::uuid")
        .bind(id)
        .execute(&app.state.db)
        .await
        .unwrap();

    let job = claim_clip(&app, &token).await.body["job"].clone();
    let id = job["id"].as_str().unwrap();
    let key = upload_key(job["upload"]["url"].as_str().unwrap());
    app.state
        .storage
        .put(&key, vec![0u8, 0, 0, 24], "video/mp4", None)
        .await
        .unwrap();
    let res = report_clip(&app, &token, id, ready.clone()).await;
    assert_eq!(res.status, StatusCode::NO_CONTENT, "{}", res.text);
    // Второй отчёт — задание уже закрыто
    let res = report_clip(&app, &token, id, ready).await;
    assert_eq!(res.status, StatusCode::CONFLICT);
    assert_eq!(res.code(), "job_not_yours");

    let items = clips_of(&app, &me, &[&track]).await;
    assert_eq!(items[0]["state"], "ready");
    assert_eq!(items[0]["durationMs"], 12_000);
    let url = items[0]["url"].as_str().unwrap();
    assert!(url.contains("/video/clips/"), "{url}");
    let row: (String, Option<String>, Option<i32>, i64) = sqlx::query_as(
        "SELECT state, video_id, clip_start_ms, size_bytes FROM track_clips WHERE track_id = $1::uuid",
    )
    .bind(&track)
    .fetch_one(&app.state.db)
    .await
    .unwrap();
    assert_eq!(
        row,
        ("ready".into(), Some("dQw4w9WgXcQ".into()), Some(75_000), 4)
    );

    // Неизвестные id пропускаются, больше 20 — ошибка
    let unknown = Uuid::now_v7().to_string();
    assert!(clips_of(&app, &me, &[&unknown]).await.is_empty());
    let many: Vec<String> = (0..21).map(|_| Uuid::now_v7().to_string()).collect();
    let res = app
        .post("/api/clips", json!({ "trackIds": many }), &me)
        .await;
    assert_eq!(res.status, StatusCode::BAD_REQUEST);
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn missing_clips_are_remembered_and_failures_retried(db: PgPool) {
    let app = TestApp::new(db).await;
    no_intervals(&app.state.db).await;
    let (_, admin) = app.admin("boss").await;
    let (token, _) = new_downloader(&app, &admin, "Mac").await;
    let me = app.listener("ann").await;
    let tracks: Vec<String> = ep_tracks(&app, &me)
        .await
        .into_iter()
        .take(2)
        .map(|t| t.to_string())
        .collect();
    let ids: Vec<&str> = tracks.iter().map(String::as_str).collect();
    clips_of(&app, &me, &ids).await;

    // Первым — первый в ленте
    let first = claim_clip(&app, &token).await.body["job"].clone();
    assert_eq!(first["trackId"], tracks[0].as_str());
    let res = report_clip(
        &app,
        &token,
        first["id"].as_str().unwrap(),
        json!({ "status": "none", "details": { "candidates": [] } }),
    )
    .await;
    assert_eq!(res.status, StatusCode::NO_CONTENT);

    let second = claim_clip(&app, &token).await.body["job"].clone();
    assert_eq!(second["trackId"], tracks[1].as_str());
    let res = report_clip(
        &app,
        &token,
        second["id"].as_str().unwrap(),
        json!({ "status": "error", "error": "bot_check: Sign in to confirm you're not a bot" }),
    )
    .await;
    assert_eq!(res.status, StatusCode::NO_CONTENT);
    let (status, error): (String, Option<String>) =
        sqlx::query_as("SELECT status, last_error FROM jobs WHERE id = $1::uuid")
            .bind(second["id"].as_str().unwrap())
            .fetch_one(&app.state.db)
            .await
            .unwrap();
    assert_eq!(status, "queued");
    assert!(error.unwrap().starts_with("bot_check"));

    let items = clips_of(&app, &me, &ids).await;
    assert_eq!(items[0]["state"], "none");
    assert_eq!(items[1]["state"], "pending");
    // «Клипа нет» не ставится снова, пока не прошёл месяц
    let queued: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM jobs WHERE kind = 'clip' AND status IN ('queued', 'running')",
    )
    .fetch_one(&app.state.db)
    .await
    .unwrap();
    assert_eq!(queued, 1);
    sqlx::query("UPDATE track_clips SET checked_at = now() - interval '31 days'")
        .execute(&app.state.db)
        .await
        .unwrap();
    assert_eq!(clips_of(&app, &me, &ids).await[0]["state"], "pending");
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn a_downloader_without_video_does_not_spend_clip_attempts(db: PgPool) {
    let app = TestApp::new(db).await;
    no_intervals(&app.state.db).await;
    let (_, admin) = app.admin("boss").await;
    let (token, _) = new_downloader(&app, &admin, "Mac").await;
    let me = app.listener("ann").await;
    let track = app.resolve_id("track", LIGHT_ME_UP, &me).await;
    clips_of(&app, &me, &[&track]).await;

    // Нет кодировщика (новый загрузчик) и ffmpeg без https (старое приложение): задача ждёт час,
    // попытки не тратятся, «клипа нет» не запоминается
    for error in [
        "no_encoder: нет видеокодировщика H.264 (h264_videotoolbox: Error while opening encoder)",
        "unknown: ffmpeg exited with code 8",
    ] {
        let job = claim_clip(&app, &token).await.body["job"].clone();
        let id = job["id"].as_str().unwrap();
        let res = report_clip(
            &app,
            &token,
            id,
            json!({ "status": "error", "error": error }),
        )
        .await;
        assert_eq!(res.status, StatusCode::NO_CONTENT, "{}", res.text);
        let (status, attempts, saved, later): (String, i16, Option<String>, bool) = sqlx::query_as(
            "SELECT status, attempts, last_error, run_after > now() + interval '50 minutes'
                 FROM jobs WHERE id = $1::uuid",
        )
        .bind(id)
        .fetch_one(&app.state.db)
        .await
        .unwrap();
        assert_eq!(status, "queued");
        assert_eq!(attempts, 0, "{error}");
        assert_eq!(saved.as_deref(), Some(error));
        assert!(later);
        sqlx::query("UPDATE jobs SET run_after = now() WHERE id = $1::uuid")
            .bind(id)
            .execute(&app.state.db)
            .await
            .unwrap();
    }
    let remembered: i64 = sqlx::query_scalar("SELECT count(*) FROM track_clips")
        .fetch_one(&app.state.db)
        .await
        .unwrap();
    assert_eq!(remembered, 0);
    assert_eq!(clips_of(&app, &me, &[&track]).await[0]["state"], "pending");
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn old_clips_leave_and_the_store_stays_under_the_limit(db: PgPool) {
    let app = TestApp::new(db).await;
    let me = app.listener("ann").await;
    let tracks = ep_tracks(&app, &me).await;
    let user = app.user_id(&me).await;
    // Четыре клипа по 100 байт: давний, давний из любимых, свежий, совсем свежий
    for (i, (track, days)) in tracks.iter().zip([10, 10, 1, 0]).enumerate() {
        sqlx::query(
            "INSERT INTO track_clips (track_id, state, storage_key, size_bytes, last_used_at)
             VALUES ($1, 'ready', $2, 100, now() - make_interval(days => $3))",
        )
        .bind(track)
        .bind(format!("video/clips/{track}/{i}.mp4"))
        .bind(days)
        .execute(&app.state.db)
        .await
        .unwrap();
    }
    sqlx::query("INSERT INTO liked_tracks (user_id, track_id) VALUES ($1::uuid, $2)")
        .bind(&user)
        .bind(tracks[1])
        .execute(&app.state.db)
        .await
        .unwrap();
    // Давний не из любимых уходит по сроку; из оставшихся трёх по пределу в 200 байт уходит
    // самый давний не из любимых
    let t = clips::trim(&app.state.db, 200).await.unwrap();
    assert_eq!((t.expired, t.evicted), (1, 1));
    let left: Vec<Uuid> = sqlx::query_scalar("SELECT track_id FROM track_clips ORDER BY track_id")
        .fetch_all(&app.state.db)
        .await
        .unwrap();
    let mut want = vec![tracks[1], tracks[3]];
    want.sort();
    assert_eq!(left, want);
    let trash: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM storage_trash WHERE storage_key LIKE 'video/clips/%'",
    )
    .fetch_one(&app.state.db)
    .await
    .unwrap();
    assert_eq!(trash, 2);
}

// ───────────── Живые обложки ─────────────

/// Ключ только для тестов (тот же, что у тестов APNs; нигде не зарегистрирован).
const TEST_KEY: &str = "-----BEGIN PRIVATE KEY-----
MIGHAgEAMBMGByqGSM49AgEGCCqGSM49AwEHBG0wawIBAQQg6SaekJxSZC4o4BWu
a0FcuNrh88dL09FQibl8gPEHg++hRANCAARwCAOsN4ViqE37jC8jSECODRd13ia4
hJQ3qUm5USAhKHOp+Ru9zn9P/xGT/50v4MOZUMS4WiYwXyy2BRKUh+p/
-----END PRIVATE KEY-----";

#[derive(Clone)]
struct FakeApple {
    base: Arc<std::sync::Mutex<String>>,
    hls: Option<PathBuf>,
    tokens: Arc<std::sync::Mutex<Vec<String>>>,
}

impl FakeApple {
    fn album(&self) -> Value {
        let base = self.base.lock().unwrap().clone();
        let mut attributes = json!({ "name": "Light Me Up", "isCompilation": false });
        if self.hls.is_some() {
            attributes["editorialVideo"] = json!({
                "motionDetailSquare": { "video": format!("{base}/hls/master.m3u8") },
            });
        }
        json!({ "id": "9001", "type": "albums", "attributes": attributes })
    }
}

async fn fake_albums(State(f): State<FakeApple>, headers: HeaderMap) -> Json<Value> {
    f.tokens.lock().unwrap().push(
        headers["authorization"]
            .to_str()
            .unwrap()
            .trim_start_matches("Bearer ")
            .to_string(),
    );
    Json(json!({ "data": [f.album()] }))
}

async fn fake_songs(
    State(f): State<FakeApple>,
    Query(q): Query<std::collections::HashMap<String, String>>,
) -> Json<Value> {
    assert!(q.contains_key("filter[isrc]"), "{q:?}");
    Json(json!({ "data": [{
        "id": "1", "type": "songs", "attributes": { "albumName": "Light Me Up" },
        "relationships": { "albums": { "data": [f.album()] } }
    }] }))
}

async fn fake_hls(State(f): State<FakeApple>, Path(file): Path<String>) -> impl IntoResponse {
    let dir = f.hls.clone().unwrap();
    match std::fs::read(dir.join(&file)) {
        Ok(bytes) => (StatusCode::OK, bytes).into_response(),
        Err(_) => StatusCode::NOT_FOUND.into_response(),
    }
}

/// HLS из синтетического видео: главный плейлист с одним вариантом. `None` — ffmpeg нет.
fn make_hls(dir: &std::path::Path) -> Option<PathBuf> {
    let out = std::process::Command::new("ffmpeg")
        .args([
            "-y",
            "-hide_banner",
            "-loglevel",
            "error",
            "-f",
            "lavfi",
            "-i",
        ])
        .arg("testsrc=size=320x320:rate=30")
        .args([
            "-t", "2", "-c:v", "libx264", "-pix_fmt", "yuv420p", "-f", "hls",
        ])
        .args(["-hls_time", "1", "-hls_playlist_type", "vod"])
        .arg(dir.join("v.m3u8"))
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    std::fs::write(
        dir.join("master.m3u8"),
        "#EXTM3U\n#EXT-X-STREAM-INF:BANDWIDTH=300000,RESOLUTION=320x320\nv.m3u8\n",
    )
    .ok()?;
    Some(dir.to_path_buf())
}

async fn start_fake(hls: Option<PathBuf>) -> (String, FakeApple) {
    let fake = FakeApple {
        base: Arc::default(),
        hls,
        tokens: Arc::default(),
    };
    let router = Router::new()
        .route("/v1/catalog/us/albums", get(fake_albums))
        .route("/v1/catalog/us/songs", get(fake_songs))
        .route("/hls/{file}", get(fake_hls))
        .with_state(fake.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    *fake.base.lock().unwrap() = base.clone();
    tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    (base, fake)
}

async fn album_of(app: &TestApp, track: &str) -> String {
    sqlx::query_scalar::<_, Uuid>("SELECT album_id FROM tracks WHERE id = $1::uuid")
        .bind(track)
        .fetch_one(&app.state.db)
        .await
        .unwrap()
        .to_string()
}

async fn motion_app(db: PgPool, hls: Option<PathBuf>) -> (TestApp, FakeApple, tempfile::TempDir) {
    let (base, fake) = start_fake(hls).await;
    let keys = tempfile::tempdir().unwrap();
    let key_path = keys.path().join("AuthKey_TESTKEY123.p8");
    std::fs::write(&key_path, TEST_KEY).unwrap();
    let app = TestApp::with(db, |c| {
        c.info.musickit = Some(MusicKitConfig {
            key_path,
            key_id: "TESTKEY123".into(),
            team_id: "TEAM123456".into(),
            storefront: "us".into(),
            url: base,
        });
    })
    .await;
    (app, fake, keys)
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn animated_cover_is_found_transcoded_and_served(db: PgPool) {
    let hls_dir = tempfile::tempdir().unwrap();
    let Some(hls) = make_hls(hls_dir.path()) else {
        eprintln!("ffmpeg is not available: skipping the transcode part");
        return;
    };
    let (app, fake, _keys) = motion_app(db, Some(hls)).await;
    let me = app.listener("ann").await;
    let track = app.resolve_id("track", LIGHT_ME_UP, &me).await;
    let album = album_of(&app, &track).await;

    let res = app.get(&format!("/api/albums/{album}/motion"), &me).await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    assert_eq!(res.body["state"], "pending");
    let (kind, outcome) = app.run_job("motion").await.unwrap();
    assert_eq!(kind, "album_motion");
    assert_eq!(outcome, Outcome::Done);

    // Токен разработчика: ES256 с Key ID в заголовке и Team ID в iss
    let token = fake.tokens.lock().unwrap()[0].clone();
    let header: Value = serde_json::from_slice(
        &dalywave_server::crypto::b64_decode(token.split('.').next().unwrap()).unwrap(),
    )
    .unwrap();
    assert_eq!(header, json!({ "alg": "ES256", "kid": "TESTKEY123" }));
    let claims: Value = serde_json::from_slice(
        &dalywave_server::crypto::b64_decode(token.split('.').nth(1).unwrap()).unwrap(),
    )
    .unwrap();
    assert_eq!(claims["iss"], "TEAM123456");
    let ttl = claims["exp"].as_i64().unwrap() - claims["iat"].as_i64().unwrap();
    assert!(ttl > 0 && ttl <= 15_777_000, "{ttl}");

    let res = app.get(&format!("/api/albums/{album}/motion"), &me).await;
    assert_eq!(res.body["state"], "ready", "{}", res.text);
    assert!(res.body["tall"].is_null());
    let url = res.body["square"].as_str().unwrap();
    let key = upload_key(url);
    assert!(key.starts_with(&format!("video/motion/{album}/")), "{key}");
    let bytes = app.state.storage.get_bytes(&key).await.unwrap();
    // MP4 с оглавлением в начале: ftyp, затем moov раньше mdat
    assert_eq!(&bytes[4..8], b"ftyp");
    let moov = bytes.windows(4).position(|w| w == b"moov").unwrap();
    let mdat = bytes.windows(4).position(|w| w == b"mdat").unwrap();
    assert!(moov < mdat);

    // Лента получает живую обложку вместе с клипом
    let res = app
        .post("/api/clips", json!({ "trackIds": [track] }), &me)
        .await;
    assert_eq!(res.body["items"][0]["motion"]["state"], "ready");
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn albums_without_an_animated_cover_are_rechecked_monthly(db: PgPool) {
    let (app, _fake, _keys) = motion_app(db, None).await;
    let me = app.listener("ann").await;
    let track = app.resolve_id("track", LIGHT_ME_UP, &me).await;
    let album = album_of(&app, &track).await;
    assert_eq!(
        app.get(&format!("/api/albums/{album}/motion"), &me)
            .await
            .body["state"],
        "pending"
    );
    let (_, outcome) = app.run_job("motion").await.unwrap();
    assert_eq!(outcome, Outcome::Done);
    let res = app.get(&format!("/api/albums/{album}/motion"), &me).await;
    assert_eq!(res.body["state"], "none");
    assert!(app.run_job("motion").await.is_none(), "no recheck yet");
    let apple: Option<String> =
        sqlx::query_scalar("SELECT apple_album_id FROM album_motion WHERE album_id = $1::uuid")
            .bind(&album)
            .fetch_one(&app.state.db)
            .await
            .unwrap();
    assert_eq!(apple.as_deref(), Some("9001"));
    sqlx::query("UPDATE album_motion SET checked_at = now() - interval '31 days'")
        .execute(&app.state.db)
        .await
        .unwrap();
    assert_eq!(
        app.get(&format!("/api/albums/{album}/motion"), &me)
            .await
            .body["state"],
        "pending"
    );
    assert_eq!(
        app.get(&format!("/api/albums/{}/motion", Uuid::now_v7()), &me)
            .await
            .status,
        StatusCode::NOT_FOUND
    );
}

// ───────────── Выбор клипа, «Скрыть клип», перепроверка ─────────────

/// Загрузчик с режимами клипа (`pinned`, `search`, `review`).
async fn claim_clip_v2(app: &TestApp, token: &str) -> Resp {
    app.post(
        "/api/fetcher/clips/claim",
        json!({ "workerId": "test-mac", "features": ["clip-modes"] }),
        &Auth::Bearer(token.into()),
    )
    .await
}

async fn choose(app: &TestApp, auth: &Auth, track: &str, body: Value) -> Resp {
    app.call(
        axum::http::Method::PUT,
        &format!("/api/tracks/{track}/clip"),
        Some(body),
        auth,
    )
    .await
}

/// Нарезать: положить файл по ссылке задания и отчитаться «готово».
async fn cut(app: &TestApp, token: &str, job: &Value, video: &str, extra: Value) -> Resp {
    let key = upload_key(job["upload"]["url"].as_str().unwrap());
    app.state
        .storage
        .put(&key, vec![0u8, 0, 0, 24], "video/mp4", None)
        .await
        .unwrap();
    let mut result = json!({ "status": "ready", "videoId": video, "videoDurationMs": 201_000,
                             "clipStartMs": 75_000, "durationMs": 12_000, "sizeBytes": 4,
                             "width": 854, "height": 480 });
    for (k, v) in extra.as_object().unwrap() {
        result[k] = v.clone();
    }
    report_clip(app, token, job["id"].as_str().unwrap(), result).await
}

fn candidates() -> Value {
    json!([
        { "videoId": "TbWV7gCPiYc", "title": "CG5 - Sleep Well", "channel": "CG5", "durationS": 208.0,
          "views": 321_169_270, "source": "artist", "score": 5.1, "rejected": null },
        { "videoId": "rC35_gNfdU0", "title": "\"Sleep Well\" | Animated Minecraft Music Video",
          "channel": "WilliamaYT", "durationS": 208.0, "views": 1_573_503, "source": null, "score": 1.2,
          "rejected": ["fan:minecraft", "not_official"] }
    ])
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn admin_picks_a_clip_and_the_pick_sticks(db: PgPool) {
    let app = TestApp::new(db).await;
    no_intervals(&app.state.db).await;
    let (_, admin) = app.admin("boss").await;
    let (token, _) = new_downloader(&app, &admin, "Mac").await;
    let me = app.listener("ann").await;
    let track = app.resolve_id("track", LIGHT_ME_UP, &me).await;

    // Автопоиск нарезал фанатское видео; кандидаты — в листе «Клип» у администратора
    clips_of(&app, &me, &[&track]).await;
    let job = claim_clip_v2(&app, &token).await.body["job"].clone();
    assert_eq!(job["mode"], "auto");
    assert!(job["videoId"].is_null() && job["currentVideoId"].is_null());
    let res = cut(&app, &token, &job, "rC35_gNfdU0",
        json!({ "videoTitle": "Minecraft Music Video", "channel": "WilliamaYT", "candidates": candidates() })).await;
    assert_eq!(res.status, StatusCode::NO_CONTENT, "{}", res.text);

    let view = app.get(&format!("/api/tracks/{track}/clip"), &admin).await;
    assert_eq!(view.status, StatusCode::OK, "{}", view.text);
    assert_eq!(view.body["state"], "ready");
    assert_eq!(view.body["videoId"], "rC35_gNfdU0");
    assert_eq!(view.body["channel"], "WilliamaYT");
    assert_eq!(view.body["canChoose"], true);
    assert_eq!(view.body["pinned"], false);
    assert_eq!(view.body["candidates"].as_array().unwrap().len(), 2);
    assert_eq!(view.body["candidates"][0]["source"], "artist");
    // Слушателю — без кандидатов и без выбора
    let mine = app.get(&format!("/api/tracks/{track}/clip"), &me).await;
    assert_eq!(mine.body["canChoose"], false);
    assert!(mine.body["candidates"].as_array().unwrap().is_empty());
    let res = choose(&app, &me, &track, json!({ "action": "none" })).await;
    assert_eq!(res.status, StatusCode::FORBIDDEN);

    // Администратор выбирает клип с канала автора: старый уходит сразу, пока режется — обложка
    let res = choose(
        &app,
        &admin,
        &track,
        json!({ "action": "video", "videoId": "TbWV7gCPiYc" }),
    )
    .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    assert_eq!(res.body["state"], "pending");
    assert_eq!(res.body["pinned"], true);
    assert_eq!(clips_of(&app, &me, &[&track]).await[0]["state"], "pending");
    // Старый загрузчик выбор не режет: задача ждёт нового, попытка не тратится
    assert_eq!(
        claim_clip(&app, &token).await.status,
        StatusCode::NO_CONTENT
    );
    let (attempts, later): (i16, bool) = sqlx::query_as(
        "SELECT attempts, run_after > now() + interval '50 minutes' FROM jobs
         WHERE kind = 'clip' AND status = 'queued'",
    )
    .fetch_one(&app.state.db)
    .await
    .unwrap();
    assert_eq!((attempts, later), (0, true));
    sqlx::query("UPDATE jobs SET run_after = now() WHERE kind = 'clip' AND status = 'queued'")
        .execute(&app.state.db)
        .await
        .unwrap();
    let job = claim_clip_v2(&app, &token).await.body["job"].clone();
    assert_eq!(job["mode"], "pinned");
    assert_eq!(job["videoId"], "TbWV7gCPiYc");
    let res = cut(
        &app,
        &token,
        &job,
        "TbWV7gCPiYc",
        json!({ "channel": "CG5", "source": "artist" }),
    )
    .await;
    assert_eq!(res.status, StatusCode::NO_CONTENT, "{}", res.text);
    let view = app.get(&format!("/api/tracks/{track}/clip"), &admin).await;
    assert_eq!(view.body["state"], "ready");
    assert_eq!(view.body["videoId"], "TbWV7gCPiYc");
    assert_eq!(view.body["source"], "pinned");
    // Кандидаты прошлого поиска остаются
    assert_eq!(view.body["candidates"].as_array().unwrap().len(), 2);

    // Выбранный клип ушёл по сроку хранения — нарезается снова из того же ролика
    sqlx::query("DELETE FROM track_clips")
        .execute(&app.state.db)
        .await
        .unwrap();
    assert_eq!(clips_of(&app, &me, &[&track]).await[0]["state"], "pending");
    let job = claim_clip_v2(&app, &token).await.body["job"].clone();
    assert_eq!(
        (job["mode"].clone(), job["videoId"].clone()),
        (json!("pinned"), json!("TbWV7gCPiYc"))
    );

    // Нарезка автопоиска, закончившаяся после выбора, не перезаписывает выбор
    let res = choose(
        &app,
        &admin,
        &track,
        json!({ "action": "url", "url": "https://youtu.be/28I_6qvI3Dk?si=x" }),
    )
    .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    let res = cut(&app, &token, &job, "TbWV7gCPiYc", json!({})).await;
    assert_eq!(res.status, StatusCode::NO_CONTENT, "{}", res.text);
    let ready: i64 = sqlx::query_scalar("SELECT count(*) FROM track_clips WHERE state = 'ready'")
        .fetch_one(&app.state.db)
        .await
        .unwrap();
    assert_eq!(ready, 0);
    let job = claim_clip_v2(&app, &token).await.body["job"].clone();
    assert_eq!(job["videoId"], "28I_6qvI3Dk");
    let res = choose(
        &app,
        &admin,
        &track,
        json!({ "action": "url", "url": "https://soundcloud.com/x/y" }),
    )
    .await;
    assert_eq!(res.status, StatusCode::BAD_REQUEST);

    // «Без клипа»: обложка, и больше не ищется
    let res = choose(&app, &admin, &track, json!({ "action": "none" })).await;
    assert_eq!(res.body["state"], "none");
    assert_eq!(clips_of(&app, &me, &[&track]).await[0]["state"], "none");
    let queued: i64 =
        sqlx::query_scalar("SELECT count(*) FROM jobs WHERE kind = 'clip' AND status = 'queued'")
            .fetch_one(&app.state.db)
            .await
            .unwrap();
    assert_eq!(queued, 0);
    assert!(app.audit_kinds().await.contains(&"clip.chosen".to_string()));
    let res = report_clip(
        &app,
        &token,
        job["id"].as_str().unwrap(),
        json!({ "status": "none" }),
    )
    .await;
    assert_eq!(res.status, StatusCode::NO_CONTENT, "{}", res.text);

    // «Заново»: выбор снят, клип ищется автопоиском впереди очереди
    let res = choose(&app, &admin, &track, json!({ "action": "auto" })).await;
    assert_eq!(res.body["pinned"], false);
    assert_eq!(res.body["state"], "pending");
    let job = claim_clip_v2(&app, &token).await.body["job"].clone();
    assert_eq!(job["mode"], "auto");
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn a_listener_hides_a_clip_and_the_admin_sees_it(db: PgPool) {
    let app = TestApp::new(db).await;
    let (_, admin) = app.admin("boss").await;
    let ann = app.listener("ann").await;
    let bob = app.listener("bob").await;
    let track = app.resolve_id("track", LIGHT_ME_UP, &ann).await;
    let hide = format!("/api/tracks/{track}/clip/hide");

    // Клипа нет — скрывать нечего
    let res = app.post(&hide, json!({}), &ann).await;
    assert_eq!(res.status, StatusCode::NOT_FOUND);
    sqlx::query(
        "INSERT INTO track_clips (track_id, state, video_id, storage_key, size_bytes, video_title, channel)
         VALUES ($1::uuid, 'ready', 'fanfanfan01', 'video/clips/x/1.mp4', 4, 'Minecraft Music Video', 'Fan')",
    )
    .bind(&track)
    .execute(&app.state.db)
    .await
    .unwrap();
    let res = app.post(&hide, json!({}), &ann).await;
    assert_eq!(res.status, StatusCode::NO_CONTENT, "{}", res.text);
    // Скрыт только у Ann
    assert_eq!(clips_of(&app, &ann, &[&track]).await[0]["state"], "none");
    assert_eq!(clips_of(&app, &bob, &[&track]).await[0]["state"], "ready");
    assert_eq!(
        app.get(&format!("/api/tracks/{track}/clip"), &ann)
            .await
            .body["hidden"],
        true
    );

    // Жалоба — в «Спорных»
    let res = app.get("/api/admin/clips/reports", &admin).await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    let items = res.body["items"].as_array().unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["track"]["id"], track.as_str());
    assert_eq!(items[0]["videoId"], "fanfanfan01");
    assert_eq!(items[0]["reports"], 1);
    assert_eq!(
        app.get(&format!("/api/tracks/{track}/clip"), &admin)
            .await
            .body["reports"],
        1
    );
    assert_eq!(
        app.get("/api/admin/clips/reports", &ann).await.status,
        StatusCode::FORBIDDEN
    );

    // Администратор оставил клип — жалоба разобрана, у Ann он по-прежнему скрыт
    let res = choose(&app, &admin, &track, json!({ "action": "keep" })).await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    assert_eq!(res.body["pinned"], true);
    assert!(
        app.get("/api/admin/clips/reports", &admin).await.body["items"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert_eq!(clips_of(&app, &ann, &[&track]).await[0]["state"], "none");

    // Вернуть
    let res = app.delete(&hide, &ann).await;
    assert_eq!(res.status, StatusCode::NO_CONTENT);
    assert_eq!(clips_of(&app, &ann, &[&track]).await[0]["state"], "ready");
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn old_clips_are_reviewed_by_the_new_pick(db: PgPool) {
    let app = TestApp::new(db).await;
    no_intervals(&app.state.db).await;
    let (_, admin) = app.admin("boss").await;
    let (token, _) = new_downloader(&app, &admin, "Mac").await;
    let me = app.listener("ann").await;
    let tracks = ep_tracks(&app, &me).await;
    // Три клипа старого отбора и один уже проверенный
    for (i, track) in tracks.iter().take(4).enumerate() {
        sqlx::query(
            "INSERT INTO track_clips (track_id, state, video_id, storage_key, size_bytes, reviewed_at)
             VALUES ($1, 'ready', $2, $3, 4, CASE WHEN $4 THEN now() END)",
        )
        .bind(track)
        .bind(format!("video{i:06}"))
        .bind(format!("video/clips/{track}/old.mp4"))
        .bind(i == 3)
        .execute(&app.state.db)
        .await
        .unwrap();
    }
    assert_eq!(clips::review(&app.state.db).await.unwrap(), 3);
    // Повтор не ставит вторых задач
    assert_eq!(clips::review(&app.state.db).await.unwrap(), 0);
    // Лента впереди перепроверок
    let other = ep_tracks(&app, &me).await[4].to_string();
    clips_of(&app, &me, &[&other]).await;
    let first = claim_clip_v2(&app, &token).await.body["job"].clone();
    assert_eq!(
        (first["trackId"].clone(), first["mode"].clone()),
        (json!(other), json!("auto"))
    );
    report_clip(
        &app,
        &token,
        first["id"].as_str().unwrap(),
        json!({ "status": "none" }),
    )
    .await;

    // Тот же клип и есть лучший — остаётся, отмечен проверенным
    let job = claim_clip_v2(&app, &token).await.body["job"].clone();
    assert_eq!(job["mode"], "review");
    let track: Uuid = job["trackId"].as_str().unwrap().parse().unwrap();
    let current = job["currentVideoId"].as_str().unwrap().to_string();
    let res = report_clip(
        &app,
        &token,
        job["id"].as_str().unwrap(),
        json!({ "status": "kept", "videoId": current, "channel": "Artist", "source": "artist",
                "candidates": candidates() }),
    )
    .await;
    assert_eq!(res.status, StatusCode::NO_CONTENT, "{}", res.text);
    let (state, reviewed, channel): (String, bool, Option<String>) = sqlx::query_as(
        "SELECT state, reviewed_at IS NOT NULL, channel FROM track_clips WHERE track_id = $1",
    )
    .bind(track)
    .fetch_one(&app.state.db)
    .await
    .unwrap();
    assert_eq!(
        (state.as_str(), reviewed, channel.as_deref()),
        ("ready", true, Some("Artist"))
    );

    // Фанатское, а официального нет — клип убран, кандидаты сохранены
    let job = claim_clip_v2(&app, &token).await.body["job"].clone();
    assert_eq!(job["mode"], "review");
    let track: Uuid = job["trackId"].as_str().unwrap().parse().unwrap();
    let res = report_clip(
        &app,
        &token,
        job["id"].as_str().unwrap(),
        json!({ "status": "none", "candidates": candidates() }),
    )
    .await;
    assert_eq!(res.status, StatusCode::NO_CONTENT, "{}", res.text);
    let (state, n): (String, i32) = sqlx::query_as(
        "SELECT state, jsonb_array_length(candidates) FROM track_clips WHERE track_id = $1",
    )
    .bind(track)
    .fetch_one(&app.state.db)
    .await
    .unwrap();
    assert_eq!((state.as_str(), n), ("none", 2));
    assert_eq!(
        clips_of(&app, &me, &[&track.to_string()]).await[0]["state"],
        "none"
    );

    // Нашёлся клип с канала автора — перерезан
    let job = claim_clip_v2(&app, &token).await.body["job"].clone();
    assert_eq!(job["mode"], "review");
    let track: Uuid = job["trackId"].as_str().unwrap().parse().unwrap();
    let res = cut(
        &app,
        &token,
        &job,
        "TbWV7gCPiYc",
        json!({ "source": "artist" }),
    )
    .await;
    assert_eq!(res.status, StatusCode::NO_CONTENT, "{}", res.text);
    let (video, source): (String, String) =
        sqlx::query_as("SELECT video_id, source FROM track_clips WHERE track_id = $1")
            .bind(track)
            .fetch_one(&app.state.db)
            .await
            .unwrap();
    assert_eq!((video.as_str(), source.as_str()), ("TbWV7gCPiYc", "artist"));
    let trashed: i64 =
        sqlx::query_scalar("SELECT count(*) FROM storage_trash WHERE storage_key LIKE '%/old.mp4'")
            .fetch_one(&app.state.db)
            .await
            .unwrap();
    assert_eq!(trashed, 2);

    // Старому загрузчику перепроверки не выдаются
    sqlx::query("UPDATE track_clips SET reviewed_at = NULL")
        .execute(&app.state.db)
        .await
        .unwrap();
    clips::review(&app.state.db).await.unwrap();
    assert_eq!(
        claim_clip(&app, &token).await.status,
        StatusCode::NO_CONTENT
    );
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn search_only_refreshes_candidates(db: PgPool) {
    let app = TestApp::new(db).await;
    no_intervals(&app.state.db).await;
    let (_, admin) = app.admin("boss").await;
    let (token, _) = new_downloader(&app, &admin, "Mac").await;
    let me = app.listener("ann").await;
    let track = app.resolve_id("track", LIGHT_ME_UP, &me).await;
    sqlx::query(
        "INSERT INTO track_clips (track_id, state, video_id, storage_key, size_bytes, reviewed_at)
         VALUES ($1::uuid, 'ready', 'TbWV7gCPiYc', 'video/clips/x/2.mp4', 4, now())",
    )
    .bind(&track)
    .execute(&app.state.db)
    .await
    .unwrap();
    let res = choose(&app, &admin, &track, json!({ "action": "search" })).await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    assert_eq!(res.body["searching"], true);
    // Поиск не убирает нарезанный клип
    assert_eq!(res.body["state"], "ready");
    let job = claim_clip_v2(&app, &token).await.body["job"].clone();
    assert_eq!(job["mode"], "search");
    let res = report_clip(
        &app,
        &token,
        job["id"].as_str().unwrap(),
        json!({ "status": "none", "candidates": candidates() }),
    )
    .await;
    assert_eq!(res.status, StatusCode::NO_CONTENT, "{}", res.text);
    let view = app.get(&format!("/api/tracks/{track}/clip"), &admin).await;
    assert_eq!(view.body["state"], "ready");
    assert_eq!(view.body["searching"], false);
    assert_eq!(view.body["candidates"].as_array().unwrap().len(), 2);
}
