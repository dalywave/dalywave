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
