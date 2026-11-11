//! Домашний загрузчик против API: задание, загрузка по подписанной ссылке, отчёт, аренда, порядок полос,
//! разбор результатов и раздельный вход. Хранилище — в памяти процесса: «загрузку по ссылке» тест делает
//! сам, кладя байты по ключу из ссылки.

use std::{
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use axum::{
    Json, Router,
    extract::State,
    http::StatusCode,
    routing::{get, post},
};
use dalywave_protocol::jobs::AcquireReason;
use dalywave_server::{
    acquire::{self, Request},
    jobs::{self, NewJob, worker::Outcome},
    storage::Storage,
};
use serde_json::{Value, json};
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    common::{Auth, Resp, TestApp},
    deezer::{LIGHT_ME_UP, MODERN_SADNESS},
};

/// Новый загрузчик: (токен `dwk_…`, id).
pub async fn new_downloader(app: &TestApp, admin: &Auth, name: &str) -> (String, String) {
    let res = app
        .post("/api/admin/downloaders", json!({ "name": name }), admin)
        .await;
    assert_eq!(res.status, StatusCode::CREATED, "{}", res.text);
    let token = res.body["token"].as_str().unwrap().to_string();
    assert!(token.starts_with("dwk_"));
    (
        token,
        res.body["downloader"]["id"].as_str().unwrap().to_string(),
    )
}

pub async fn claim(app: &TestApp, token: &str) -> Resp {
    let hello = json!({
        "workerId": "test-mac",
        "version": "0.2.0",
        "capabilities": { "kinds": ["acquire"], "potProvider": true, "cookies": false,
                          "playerClients": [], "ytDlp": "2026.10.26" }
    });
    app.post(
        "/api/fetcher/jobs/claim",
        hello,
        &Auth::Bearer(token.into()),
    )
    .await
}

pub async fn claimed(app: &TestApp, token: &str) -> Value {
    let res = claim(app, token).await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    res.body["job"].clone()
}

/// Ключ файла из подписанной ссылки: путь после бакета.
pub fn upload_key(url: &str) -> String {
    url::Url::parse(url)
        .unwrap()
        .path()
        .trim_start_matches("/dalywave/")
        .to_string()
}

/// «Загрузчик положил файл по ссылке».
pub async fn upload(storage: &Storage, url: &str, bytes: &[u8]) {
    storage
        .put(
            &upload_key(url),
            bytes.to_vec(),
            "audio/mp4; codecs=\"opus\"",
            None,
        )
        .await
        .unwrap();
}

/// Ответ fetcher «скачано и проверено отпечатком» — как его шлёт `fetcher worker`.
pub fn verified(deezer_track_id: &Value, size: usize, video: &str) -> Value {
    let waveform: Vec<i64> = (0..512).map(|i| (i * 7) % 1001).collect();
    json!({
        "status": "verified",
        "deezerTrackId": deezer_track_id,
        "track": { "deezerId": deezer_track_id, "title": "Light Me Up" },
        "chosen": {
            "candidate": { "videoId": video, "title": "Light Me Up", "artists": ["Lowborn"], "source": "album",
                           "kind": "atv", "score": 0.97, "breakdown": { "duration": { "score": 1.0 } } },
            "download": { "videoId": video, "itag": "251", "codec": "opus", "container": "mp4",
                          "mimeType": "audio/mp4; codecs=\"opus\"", "abrKbps": 136.1, "sampleRate": 48000,
                          "channels": 2, "sizeBytes": size, "durationMs": 180500, "title": "Light Me Up" },
            "verify": { "verified": true, "similarity": 0.9761, "offsetS": 57.03, "threshold": 0.8 }
        },
        "file": { "sha256": "ab".repeat(32), "sizeBytes": size, "container": "mp4",
                  "mimeType": "audio/mp4; codecs=\"opus\"", "codec": "opus", "durationMs": 180500,
                  "loudnessLufs": -9.1, "truePeakDb": 0.4, "waveform": waveform },
        "upload": { "ok": true, "status": 200, "etag": "\"e\"" },
        "attempts": [{ "videoId": video, "score": 0.97, "source": "album", "download": { "videoId": video },
                       "verify": { "verified": true, "similarity": 0.9761, "offsetS": 57.03 }, "error": null }],
        "candidates": [
            { "videoId": video, "url": format!("https://music.youtube.com/watch?v={video}"), "title": "Light Me Up",
              "artists": ["Lowborn"], "album": "Light Me Up", "durationMs": 180000, "kind": "atv", "source": "album",
              "score": 0.97, "rejected": null, "breakdown": { "duration": { "score": 1.0 } } },
            { "videoId": "otherVideo1", "title": "Light Me Up (Live)", "artists": ["Lowborn"], "kind": "omv",
              "source": "video", "score": 0.31, "breakdown": { "markers": [{ "marker": "live" }] } }
        ],
        "error": null,
        "timingsMs": { "total": 9900 },
        "versions": { "fetcher": "0.2.0", "ytDlp": "2026.10.26" }
    })
}

pub async fn report(app: &TestApp, token: &str, job: &Value, result: Value) -> Resp {
    let id = job["id"].as_str().unwrap();
    app.post(
        &format!("/api/fetcher/jobs/{id}/result"),
        json!({ "workerId": "test-mac", "result": result }),
        &Auth::Bearer(token.into()),
    )
    .await
}

/// Пауза между задачами полос (вежливость к YouTube) в тестах не нужна.
pub async fn no_intervals(db: &PgPool) {
    sqlx::query("UPDATE job_lanes SET min_interval_ms = 0")
        .execute(db)
        .await
        .unwrap();
}

/// Треки EP «Modern Sadness» (наши id) по порядку: пять разных записей.
pub async fn ep_tracks(app: &TestApp, auth: &Auth) -> Vec<Uuid> {
    let ep = app.resolve_id("album", MODERN_SADNESS, auth).await;
    let res = app.get(&format!("/api/albums/{ep}"), auth).await;
    res.body["tracks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["id"].as_str().unwrap().parse().unwrap())
        .collect()
}

/// Основной файл записи: исполнитель, источник, ссылка, название, проверка, сходство, смещение,
/// размер, SHA-256.
type FileRow = (
    String,
    String,
    String,
    Option<String>,
    bool,
    Option<f32>,
    Option<i32>,
    i64,
    Option<String>,
);

async fn status_of(db: &PgPool, track: &str) -> (String, String) {
    sqlx::query_as(
        "SELECT a.status, r.audio_status FROM acquisitions a
         JOIN recordings r ON r.id = a.recording_id
         JOIN tracks t ON t.recording_id = r.id
         WHERE t.id = $1::uuid ORDER BY a.id DESC LIMIT 1",
    )
    .bind(track)
    .fetch_one(db)
    .await
    .unwrap()
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn downloader_takes_a_job_uploads_and_reports(db: PgPool) {
    let app = TestApp::new(db).await;
    let (_, admin) = app.admin("boss").await;
    let (token, downloader_id) = new_downloader(&app, &admin, "Mac Захара").await;
    let me = app.listener("ann").await;
    let track = app.resolve_id("track", LIGHT_ME_UP, &me).await;

    // Загрузчик ещё не на связи: трек встаёт в очередь, но качать некому
    let res = app.get(&format!("/api/tracks/{track}/stream"), &me).await;
    assert_eq!(res.status, StatusCode::ACCEPTED, "{}", res.text);
    assert_eq!(res.body["status"], "waiting_for_downloader");
    assert_eq!(res.body["queuePosition"], 0);
    assert_eq!(res.body["downloadersOnline"], 0);
    let acquisition = res.body["acquisitionId"].as_str().unwrap().to_string();

    let job = claimed(&app, &token).await;
    assert_eq!(job["kind"], "acquire");
    assert_eq!(job["deezerTrackId"], LIGHT_ME_UP);
    assert_eq!(job["maxAttempts"], 3);
    assert_eq!(job["preferVideoIds"], json!([]));
    assert!(job["directUrl"].is_null());
    assert_eq!(job["upload"]["method"], "PUT");
    let url = job["upload"]["url"].as_str().unwrap().to_string();
    let key = upload_key(&url);
    assert!(key.starts_with("audio/") && key.ends_with(".m4a"), "{key}");
    assert!(job["leaseExpiresAt"].is_string() && job["upload"]["expiresAt"].is_string());

    // Пока качается: загрузчик на связи, шаг — поиск, запись «скачивается»
    let res = app.get(&format!("/api/tracks/{track}/stream"), &me).await;
    assert_eq!(res.status, StatusCode::ACCEPTED);
    assert_eq!(res.body["status"], "acquiring");
    assert_eq!(res.body["downloadersOnline"], 1);
    assert_eq!(res.body["acquisitionId"], acquisition.as_str());
    let info = app
        .get(&format!("/api/acquisitions/{acquisition}"), &me)
        .await;
    assert_eq!(info.status, StatusCode::OK, "{}", info.text);
    assert_eq!(info.body["status"], "matching");
    assert_eq!(info.body["audioStatus"], "acquiring");
    assert_eq!(info.body["reason"], "play");
    assert_eq!(info.body["lane"], "now");
    assert_eq!(info.body["waitingForDownloader"], false);

    // Файл по ссылке и отчёт
    let bytes = vec![7u8; 5000];
    upload(&app.state.storage, &url, &bytes).await;
    let res = report(
        &app,
        &token,
        &job,
        verified(&job["deezerTrackId"], 5000, "afkL3uLNeXA"),
    )
    .await;
    assert_eq!(res.status, StatusCode::NO_CONTENT, "{}", res.text);

    let res = app.get(&format!("/api/tracks/{track}/stream"), &me).await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    let s = &res.body;
    assert_eq!(s["status"], "ready");
    assert_eq!(s["codec"], "opus");
    assert_eq!(s["container"], "mp4");
    assert_eq!(s["mimeType"], "audio/mp4; codecs=\"opus\"");
    assert_eq!(s["bitrateKbps"], 136);
    assert_eq!(s["durationMs"], 180500);
    assert_eq!(s["loudnessLufs"].as_f64().unwrap() as f32, -9.1f32);
    assert_eq!(s["waveform"].as_array().unwrap().len(), 512);
    let link = s["url"].as_str().unwrap();
    assert!(link.contains(&upload_key(&url)), "{link}");
    assert!(link.contains("X-Amz-Expires=21600"), "{link}");
    assert_eq!(res.headers["cache-control"], "no-store");

    let file: FileRow =
        sqlx::query_as(
            "SELECT f.acquired_by, f.source, f.source_ref, f.source_title, f.verified, f.verify_similarity,
                    f.verify_offset_ms, f.size_bytes, f.sha256
             FROM audio_files f JOIN recordings r ON r.primary_file_id = f.id
             JOIN tracks t ON t.recording_id = r.id WHERE t.id = $1::uuid",
        )
        .bind(&track)
        .fetch_one(&app.state.db)
        .await
        .unwrap();
    assert_eq!(file.0, format!("device:{downloader_id}"));
    assert_eq!(file.1, "youtube_music");
    assert_eq!(file.2, "afkL3uLNeXA");
    assert_eq!(file.3.as_deref(), Some("Light Me Up"));
    assert!(file.4);
    assert!((file.5.unwrap() - 0.9761).abs() < 1e-4);
    assert_eq!(file.6, Some(57030));
    assert_eq!(file.7, 5000);
    assert_eq!(file.8, Some("ab".repeat(32)));
    assert_eq!(
        status_of(&app.state.db, &track).await,
        ("ready".into(), "ready".into())
    );
    let info = app
        .get(&format!("/api/acquisitions/{acquisition}"), &me)
        .await;
    assert_eq!(info.body["status"], "ready");
    assert!(info.body["queuePosition"].is_null());
    assert!(info.body["finishedAt"].is_string());
    let job_row = jobs::get(&app.state.db, job["id"].as_str().unwrap().parse().unwrap())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(job_row.status, "done");
    assert_eq!(job_row.result.unwrap()["videoId"], "afkL3uLNeXA");

    // Повторный отчёт о закрытом задании не принимается
    let res = report(
        &app,
        &token,
        &job,
        verified(&job["deezerTrackId"], 5000, "afkL3uLNeXA"),
    )
    .await;
    assert_eq!(res.status, StatusCode::CONFLICT);
    assert_eq!(res.code(), "job_not_yours");

    // «Источники»: загрузчик на связи, версия и сутки работы
    let res = app.get("/api/admin/sources", &admin).await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    let d = &res.body["downloaders"][0];
    assert_eq!(d["name"], "Mac Захара");
    assert_eq!(d["online"], true);
    assert_eq!(d["workerId"], "test-mac");
    assert_eq!(d["version"], "0.2.0");
    assert_eq!(d["ytDlpVersion"], "2026.10.26");
    assert_eq!(d["capabilities"]["potProvider"], true);
    assert_eq!(d["ok24h"], 1);
    assert_eq!(d["failed24h"], 0);
    assert_eq!(d["running"], 0);
    assert!(d["lastClaimAt"].is_string());
    assert!(
        d.get("token").is_none(),
        "токен показывается только при создании"
    );
    assert_eq!(res.body["server"]["enabled"], false);
    assert!(res.body["server"]["reachable"].is_null());
    // Решение по скачиванию видно в админке вместе с кандидатами и разбором оценки
    let res = app
        .get("/api/admin/acquisitions?status=ready", &admin)
        .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    let item = &res.body["items"][0];
    assert_eq!(item["acquisition"]["id"], acquisition.as_str());
    assert_eq!(item["track"]["id"], track.as_str());
    assert_eq!(item["worker"], format!("device:{downloader_id}"));
    let candidates = item["candidates"].as_array().unwrap();
    assert_eq!(candidates.len(), 2);
    assert_eq!(candidates[0]["attempt"]["verified"], true);
    assert_eq!(candidates[0]["attempt"]["offsetMs"], 57030);
    assert!(candidates[1]["attempt"].is_null());
    assert_eq!(candidates[1]["breakdown"]["markers"][0]["marker"], "live");
    assert_eq!(item["attemptsLog"][0]["outcome"], "ready");
    assert_eq!(
        item["attemptsLog"][0]["executor"],
        format!("device:{downloader_id}")
    );
    assert_eq!(item["attemptsLog"][0]["ytDlpVersion"], "2026.10.26");
    assert_eq!(item["chosen"]["verify"]["similarity"], 0.9761);
}

async fn lane_of(db: &PgPool, job: &Value) -> String {
    sqlx::query_scalar("SELECT lane FROM jobs WHERE id = $1::uuid")
        .bind(job["id"].as_str().unwrap())
        .fetch_one(db)
        .await
        .unwrap()
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn claims_follow_lane_order_pauses_and_kinds(db: PgPool) {
    let app = TestApp::new(db).await;
    let (_, admin) = app.admin("boss").await;
    let (token, _) = new_downloader(&app, &admin, "mac").await;
    let tracks = ep_tracks(&app, &admin).await;
    no_intervals(&app.state.db).await;
    // По одной записи в каждую полосу, в обратном порядке важности
    let lanes = ["discovery", "discography", "library", "wave", "now"];
    for (track, lane) in tracks.iter().zip(lanes) {
        let res = acquire::enqueue(
            &app.state.db,
            Request::new(*track, AcquireReason::Manual, lane),
        )
        .await
        .unwrap();
        assert!(matches!(res, acquire::Enqueue::Queued { .. }), "{res:?}");
    }
    // Задача другого вида в полосе скачивания загрузчику не достаётся
    let mut conn = app.state.db.acquire().await.unwrap();
    jobs::enqueue(
        &mut conn,
        NewJob::new("probe", "library", json!({})).priority(100),
    )
    .await
    .unwrap();
    drop(conn);
    // «Волна» на паузе — её задача ждёт
    sqlx::query("UPDATE job_lanes SET paused = true WHERE lane = 'wave'")
        .execute(&app.state.db)
        .await
        .unwrap();

    let mut order = Vec::new();
    for _ in 0..4 {
        let job = claimed(&app, &token).await;
        order.push(lane_of(&app.state.db, &job).await);
    }
    assert_eq!(order, ["now", "library", "discography", "discovery"]);
    // Больше ничего: пустой ответ после ожидания и совет спросить снова
    let res = claim(&app, &token).await;
    assert_eq!(res.status, StatusCode::NO_CONTENT);
    assert_eq!(res.headers["retry-after"], "1");
    sqlx::query("UPDATE job_lanes SET paused = false WHERE lane = 'wave'")
        .execute(&app.state.db)
        .await
        .unwrap();
    let job = claimed(&app, &token).await;
    assert_eq!(lane_of(&app.state.db, &job).await, "wave");
    let probe: String = sqlx::query_scalar("SELECT status FROM jobs WHERE kind = 'probe'")
        .fetch_one(&app.state.db)
        .await
        .unwrap();
    assert_eq!(probe, "queued");

    // Загрузчик, который не умеет acquire, ничего не получает
    let res = app
        .post(
            "/api/fetcher/jobs/claim",
            json!({ "workerId": "x", "capabilities": { "kinds": ["analysis"] } }),
            &Auth::Bearer(token.clone()),
        )
        .await;
    assert_eq!(res.status, StatusCode::NO_CONTENT);
    let res = app
        .post(
            "/api/fetcher/jobs/claim",
            json!({ "workerId": "" }),
            &Auth::Bearer(token.clone()),
        )
        .await;
    assert_eq!(res.status, StatusCode::BAD_REQUEST);
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn long_poll_hands_out_a_job_that_appears_while_waiting(db: PgPool) {
    let app = TestApp::with(db, |c| c.fetcher.claim_wait = Duration::from_secs(5)).await;
    let (_, admin) = app.admin("boss").await;
    let (token, _) = new_downloader(&app, &admin, "mac").await;
    let track = app.resolve_id("track", LIGHT_ME_UP, &admin).await;
    let started = Instant::now();
    let db = app.state.db.clone();
    let track_id: Uuid = track.parse().unwrap();
    let later = tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(300)).await;
        acquire::enqueue(&db, Request::new(track_id, AcquireReason::Like, "library"))
            .await
            .unwrap()
    });
    let job = claimed(&app, &token).await;
    let waited = started.elapsed();
    assert!(waited >= Duration::from_millis(250), "{waited:?}");
    assert!(waited < Duration::from_secs(3), "{waited:?}");
    assert_eq!(job["deezerTrackId"], LIGHT_ME_UP);
    later.await.unwrap();
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn expired_lease_goes_to_another_downloader(db: PgPool) {
    let app = TestApp::new(db).await;
    let (_, admin) = app.admin("boss").await;
    let (first, _) = new_downloader(&app, &admin, "first").await;
    let (second, second_id) = new_downloader(&app, &admin, "second").await;
    let track = app.resolve_id("track", LIGHT_ME_UP, &admin).await;
    no_intervals(&app.state.db).await;
    acquire::enqueue(
        &app.state.db,
        Request::new(track.parse().unwrap(), AcquireReason::Like, "library"),
    )
    .await
    .unwrap();
    let job = claimed(&app, &first).await;
    let id = job["id"].as_str().unwrap();

    // Пока работает — продлевает аренду; чужой продлить не может
    let res = app
        .post(
            &format!("/api/fetcher/jobs/{id}/lease"),
            json!({ "workerId": "a" }),
            &Auth::Bearer(first.clone()),
        )
        .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    assert!(
        res.body["leaseExpiresAt"].as_str().unwrap() >= job["leaseExpiresAt"].as_str().unwrap()
    );
    let res = app
        .call(
            axum::http::Method::POST,
            &format!("/api/fetcher/jobs/{id}/lease"),
            None,
            &Auth::Bearer(second.clone()),
        )
        .await;
    assert_eq!(res.status, StatusCode::CONFLICT);
    assert_eq!(res.code(), "job_not_yours");
    assert_eq!(claim(&app, &second).await.status, StatusCode::NO_CONTENT);

    // Первый пропал: аренда кончилась — задачу берёт второй, попытка засчитана
    sqlx::query(
        "UPDATE jobs SET lease_expires_at = now() - interval '1 second' WHERE id = $1::uuid",
    )
    .bind(id)
    .execute(&app.state.db)
    .await
    .unwrap();
    let again = claimed(&app, &second).await;
    assert_eq!(again["id"], job["id"]);
    assert_ne!(
        again["upload"]["url"], job["upload"]["url"],
        "у новой попытки — новый ключ"
    );
    let row = jobs::get(&app.state.db, id.parse().unwrap())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(row.attempts, 2);
    assert_eq!(row.locked_by, Some(format!("device:{second_id}")));
    let worker: String =
        sqlx::query_scalar("SELECT worker FROM acquisitions WHERE job_id = $1::uuid")
            .bind(id)
            .fetch_one(&app.state.db)
            .await
            .unwrap();
    assert_eq!(worker, format!("device:{second_id}"));
    // Опоздавший отчёт первого не принимается, второй отчитывается сам
    let res = report(
        &app,
        &first,
        &job,
        verified(&job["deezerTrackId"], 10, "afkL3uLNeXA"),
    )
    .await;
    assert_eq!(res.status, StatusCode::CONFLICT);
    let url = again["upload"]["url"].as_str().unwrap();
    upload(&app.state.storage, url, &[1u8; 10]).await;
    let res = report(
        &app,
        &second,
        &again,
        verified(&again["deezerTrackId"], 10, "afkL3uLNeXA"),
    )
    .await;
    assert_eq!(res.status, StatusCode::NO_CONTENT, "{}", res.text);
    assert_eq!(
        status_of(&app.state.db, &track).await,
        ("ready".into(), "ready".into())
    );

    // Аренду, кончившуюся без нового запроса задания, возвращает и сборщик воркера
    let other = ep_tracks(&app, &admin).await[0];
    acquire::enqueue(
        &app.state.db,
        Request::new(other, AcquireReason::Like, "library"),
    )
    .await
    .unwrap();
    let job = claimed(&app, &first).await;
    sqlx::query(
        "UPDATE jobs SET lease_expires_at = now() - interval '1 second' WHERE id = $1::uuid",
    )
    .bind(job["id"].as_str().unwrap())
    .execute(&app.state.db)
    .await
    .unwrap();
    let reaped = jobs::reap(&app.state.db, Some(Duration::from_secs(120)))
        .await
        .unwrap();
    assert_eq!(reaped.len(), 1);
    assert_eq!(
        reaped[0].last_error.as_deref(),
        Some("загрузчик не отчитался вовремя")
    );
    acquire::after_reap(&app.state.db, &reaped).await.unwrap();
    assert_eq!(
        status_of(&app.state.db, &other.to_string()).await,
        ("queued".into(), "queued".into())
    );
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn result_without_a_file_is_a_failure(db: PgPool) {
    let app = TestApp::new(db).await;
    let (_, admin) = app.admin("boss").await;
    let (token, _) = new_downloader(&app, &admin, "mac").await;
    let track = app.resolve_id("track", LIGHT_ME_UP, &admin).await;
    no_intervals(&app.state.db).await;
    acquire::enqueue(
        &app.state.db,
        Request::new(track.parse().unwrap(), AcquireReason::Like, "library"),
    )
    .await
    .unwrap();
    let job_id = |job: &Value| -> Uuid { job["id"].as_str().unwrap().parse().unwrap() };
    let ready_now = |id: Uuid| {
        let db = app.state.db.clone();
        async move {
            sqlx::query("UPDATE jobs SET run_after = now() WHERE id = $1")
                .bind(id)
                .execute(&db)
                .await
                .unwrap();
        }
    };

    // «Проверено», но файла в хранилище нет — сбой, задача повторится позже
    let job = claimed(&app, &token).await;
    let res = report(
        &app,
        &token,
        &job,
        verified(&job["deezerTrackId"], 5000, "afkL3uLNeXA"),
    )
    .await;
    assert_eq!(res.status, StatusCode::NO_CONTENT, "{}", res.text);
    let row = jobs::get(&app.state.db, job_id(&job))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(row.status, "queued");
    assert!(
        row.last_error
            .as_deref()
            .unwrap()
            .contains("нет в хранилище"),
        "{:?}",
        row.last_error
    );
    assert!(row.run_after > chrono::Utc::now() + chrono::Duration::seconds(15));
    assert_eq!(
        status_of(&app.state.db, &track).await,
        ("queued".into(), "queued".into())
    );

    // Файл есть, но другого размера
    ready_now(job_id(&job)).await;
    let job = claimed(&app, &token).await;
    upload(
        &app.state.storage,
        job["upload"]["url"].as_str().unwrap(),
        &[1u8; 100],
    )
    .await;
    report(
        &app,
        &token,
        &job,
        verified(&job["deezerTrackId"], 200, "afkL3uLNeXA"),
    )
    .await;
    let row = jobs::get(&app.state.db, job_id(&job))
        .await
        .unwrap()
        .unwrap();
    assert!(
        row.last_error.as_deref().unwrap().contains("100 байт"),
        "{:?}",
        row.last_error
    );
    // Сломанный файл из хранилища убирается
    assert!(
        app.state
            .storage
            .head(&upload_key(job["upload"]["url"].as_str().unwrap()))
            .await
            .unwrap()
            .is_none()
    );

    // Загрузка не удалась — попытки кончились: запись «не удалось»
    ready_now(job_id(&job)).await;
    let job = claimed(&app, &token).await;
    let mut result = verified(&job["deezerTrackId"], 5000, "afkL3uLNeXA");
    result["upload"] = json!({ "ok": false, "status": 403, "error": "SignatureDoesNotMatch" });
    report(&app, &token, &job, result).await;
    let row = jobs::get(&app.state.db, job_id(&job))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(row.status, "failed");
    assert!(
        row.last_error
            .as_deref()
            .unwrap()
            .contains("SignatureDoesNotMatch")
    );
    assert_eq!(
        status_of(&app.state.db, &track).await,
        ("failed".into(), "failed".into())
    );
    let outcomes: Vec<String> =
        sqlx::query_scalar("SELECT outcome FROM acquisition_attempts ORDER BY created_at, id")
            .fetch_all(&app.state.db)
            .await
            .unwrap();
    assert_eq!(outcomes, ["retry", "retry", "failed"]);

    // Непонятный результат — тоже сбой, а не 500
    acquire::enqueue(
        &app.state.db,
        Request::new(track.parse().unwrap(), AcquireReason::Play, "now"),
    )
    .await
    .unwrap();
    let job = claimed(&app, &token).await;
    let res = report(&app, &token, &job, json!({ "status": 42 })).await;
    assert_eq!(res.status, StatusCode::NO_CONTENT, "{}", res.text);
    let row = jobs::get(&app.state.db, job_id(&job))
        .await
        .unwrap()
        .unwrap();
    assert!(
        row.last_error
            .as_deref()
            .unwrap()
            .contains("непонятный результат")
    );
    // Тело без result — ошибка запроса
    let res = app
        .post(
            &format!("/api/fetcher/jobs/{}/result", job_id(&job)),
            json!({ "workerId": "x" }),
            &Auth::Bearer(token.clone()),
        )
        .await;
    assert_eq!(res.status, StatusCode::BAD_REQUEST);
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn doubtful_results_go_to_disputed_or_unavailable(db: PgPool) {
    let app = TestApp::new(db).await;
    let (_, admin) = app.admin("boss").await;
    let (token, _) = new_downloader(&app, &admin, "mac").await;
    let tracks = ep_tracks(&app, &admin).await;
    no_intervals(&app.state.db).await;
    for t in &tracks {
        acquire::enqueue(
            &app.state.db,
            Request::new(*t, AcquireReason::Album, "library"),
        )
        .await
        .unwrap();
    }
    let take = || async { claimed(&app, &token).await };
    let track_of = |job: &Value| -> String {
        let deezer = job["deezerTrackId"].as_i64().unwrap();
        tracks
            .iter()
            .map(|t| t.to_string())
            .zip([
                3661046662i64,
                3661046672,
                3661046682,
                3661046692,
                3661046702,
            ])
            .find(|(_, d)| *d == deezer)
            .unwrap()
            .0
    };

    // Скачанные кандидаты не совпали с превью — «Спорные», кандидаты сохранены
    let job = take().await;
    let mut result = verified(&job["deezerTrackId"], 0, "afkL3uLNeXA");
    result["status"] = json!("disputed");
    result["chosen"] = Value::Null;
    result["file"] = Value::Null;
    result["upload"] = Value::Null;
    result["attempts"][0]["verify"] =
        json!({ "verified": false, "similarity": 0.55, "offsetS": 3.0 });
    assert_eq!(
        report(&app, &token, &job, result).await.status,
        StatusCode::NO_CONTENT
    );
    let disputed = track_of(&job);
    assert_eq!(
        status_of(&app.state.db, &disputed).await,
        ("disputed".into(), "disputed".into())
    );
    let res = app
        .get(&format!("/api/tracks/{disputed}/stream"), &admin)
        .await;
    assert_eq!(res.status, StatusCode::CONFLICT);
    assert_eq!(res.code(), "audio_disputed");
    let res = app
        .get("/api/admin/acquisitions?status=disputed", &admin)
        .await;
    let item = &res.body["items"][0];
    assert_eq!(item["candidates"][0]["attempt"]["similarity"], 0.55);
    assert_eq!(item["candidates"][0]["attempt"]["verified"], false);
    assert!(
        item["acquisition"]["error"]
            .as_str()
            .unwrap()
            .contains("не совпал")
    );
    assert_eq!(item["jobStatus"], "done");

    // Подходящего нет — «нет в источниках»
    let job = take().await;
    let missing = track_of(&job);
    report(
        &app,
        &token,
        &job,
        json!({ "status": "not_found", "deezerTrackId": job["deezerTrackId"], "candidates": [] }),
    )
    .await;
    assert_eq!(
        status_of(&app.state.db, &missing).await,
        ("unavailable".into(), "unavailable".into())
    );
    let res = app
        .get(&format!("/api/tracks/{missing}/stream"), &admin)
        .await;
    assert_eq!(res.code(), "audio_unavailable");

    // Без превью проверить нечем: средняя оценка — «Спорные», а загруженный файл удаляется
    let job = take().await;
    let url = job["upload"]["url"].as_str().unwrap().to_string();
    upload(&app.state.storage, &url, &[1u8; 64]).await;
    let mut result = verified(&job["deezerTrackId"], 64, "afkL3uLNeXA");
    result["status"] = json!("unverified");
    result["chosen"]["verify"] = Value::Null;
    result["chosen"]["candidate"]["score"] = json!(0.85);
    result["error"] = json!({ "code": "no_preview", "message": "у трека нет превью в Deezer" });
    report(&app, &token, &job, result).await;
    assert_eq!(
        status_of(&app.state.db, &track_of(&job)).await,
        ("disputed".into(), "disputed".into())
    );
    assert!(
        app.state
            .storage
            .head(&upload_key(&url))
            .await
            .unwrap()
            .is_none()
    );

    // …а очень хорошая — принимается без проверки
    let job = take().await;
    let url = job["upload"]["url"].as_str().unwrap().to_string();
    upload(&app.state.storage, &url, &[1u8; 64]).await;
    let mut result = verified(&job["deezerTrackId"], 64, "afkL3uLNeXA");
    result["status"] = json!("unverified");
    result["chosen"]["verify"] = Value::Null;
    result["chosen"]["candidate"]["score"] = json!(0.95);
    report(&app, &token, &job, result).await;
    let accepted = track_of(&job);
    assert_eq!(
        status_of(&app.state.db, &accepted).await,
        ("ready".into(), "ready".into())
    );
    let verified_flag: bool = sqlx::query_scalar(
        "SELECT f.verified FROM audio_files f JOIN tracks t ON t.recording_id = f.recording_id WHERE t.id = $1::uuid",
    )
    .bind(&accepted)
    .fetch_one(&app.state.db)
    .await
    .unwrap();
    assert!(!verified_flag);

    // YouTube заподозрил бота: задача отложена, попытка не засчитана
    let job = take().await;
    let id: Uuid = job["id"].as_str().unwrap().parse().unwrap();
    report(
        &app,
        &token,
        &job,
        json!({ "status": "error", "deezerTrackId": job["deezerTrackId"],
        "error": { "code": "bot_check", "message": "Sign in to confirm you're not a bot" } }),
    )
    .await;
    let row = jobs::get(&app.state.db, id).await.unwrap().unwrap();
    assert_eq!((row.status.as_str(), row.attempts), ("queued", 0));
    assert!(row.locked_by.is_none());
    assert!(row.run_after > chrono::Utc::now() + chrono::Duration::seconds(60));
    assert_eq!(
        status_of(&app.state.db, &track_of(&job)).await,
        ("queued".into(), "queued".into())
    );
    let outcome: String =
        sqlx::query_scalar("SELECT outcome FROM acquisition_attempts WHERE job_id = $1")
            .bind(id)
            .fetch_one(&app.state.db)
            .await
            .unwrap();
    assert_eq!(outcome, "postponed");
    let res = app.get("/api/admin/downloaders", &admin).await;
    assert_eq!(res.body["items"][0]["failed24h"], 1);
    assert_eq!(res.body["items"][0]["ok24h"], 1);
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn downloader_tokens_and_sessions_do_not_mix(db: PgPool) {
    let app = TestApp::new(db).await;
    let (_, admin) = app.admin("boss").await;
    let listener = app.listener("ann").await;
    let (token, id) = new_downloader(&app, &admin, "mac").await;
    let downloader = Auth::Bearer(token.clone());

    // Токен загрузчика открывает только /api/fetcher/*
    for uri in [
        "/api/auth/me",
        "/api/library",
        "/api/admin/sources",
        "/api/search?q=x",
    ] {
        let res = app.get(uri, &downloader).await;
        assert_eq!(res.status, StatusCode::UNAUTHORIZED, "{uri}");
    }
    // …а сессии людей туда не подходят
    for auth in [
        &admin,
        &listener,
        &Auth::None,
        &Auth::Bearer("dwd_fake".into()),
    ] {
        let res = app
            .post("/api/fetcher/jobs/claim", json!({ "workerId": "x" }), auth)
            .await;
        assert_eq!(res.status, StatusCode::UNAUTHORIZED);
    }
    let missing = Uuid::now_v7();
    let res = app
        .post(
            &format!("/api/fetcher/jobs/{missing}/result"),
            json!({ "workerId": "x", "result": {} }),
            &downloader,
        )
        .await;
    assert_eq!(res.status, StatusCode::NOT_FOUND);

    // Слушатель загрузчиками не управляет
    for (method, uri) in [
        (
            axum::http::Method::GET,
            "/api/admin/downloaders".to_string(),
        ),
        (
            axum::http::Method::POST,
            "/api/admin/downloaders".to_string(),
        ),
        (
            axum::http::Method::DELETE,
            format!("/api/admin/downloaders/{id}"),
        ),
        (axum::http::Method::GET, "/api/admin/sources".to_string()),
        (
            axum::http::Method::GET,
            "/api/admin/acquisitions".to_string(),
        ),
        (
            axum::http::Method::POST,
            format!("/api/admin/acquisitions/{missing}/resolve"),
        ),
    ] {
        let body =
            (method == axum::http::Method::POST).then(|| json!({ "name": "x", "action": "retry" }));
        let res = app.call(method.clone(), &uri, body, &listener).await;
        assert_eq!(res.status, StatusCode::FORBIDDEN, "{method} {uri}");
    }
    let res = app
        .post("/api/admin/downloaders", json!({ "name": "  " }), &admin)
        .await;
    assert_eq!(res.status, StatusCode::BAD_REQUEST);

    // Отзыв: токен перестаёт действовать, взятое задание сразу возвращается в очередь
    let track = app.resolve_id("track", LIGHT_ME_UP, &admin).await;
    acquire::enqueue(
        &app.state.db,
        Request::new(track.parse().unwrap(), AcquireReason::Like, "library"),
    )
    .await
    .unwrap();
    let job = claimed(&app, &token).await;
    assert_eq!(
        app.delete(&format!("/api/admin/downloaders/{id}"), &admin)
            .await
            .status,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        app.delete(&format!("/api/admin/downloaders/{id}"), &admin)
            .await
            .status,
        StatusCode::NOT_FOUND
    );
    assert_eq!(claim(&app, &token).await.status, StatusCode::UNAUTHORIZED);
    let row = jobs::get(&app.state.db, job["id"].as_str().unwrap().parse().unwrap())
        .await
        .unwrap()
        .unwrap();
    assert_eq!((row.status.as_str(), row.attempts), ("queued", 0));
    assert_eq!(
        status_of(&app.state.db, &track).await,
        ("queued".into(), "queued".into())
    );
    let res = app.get("/api/admin/downloaders", &admin).await;
    assert!(res.body["items"][0]["revokedAt"].is_string());
    let kinds = app.audit_kinds().await;
    for kind in ["downloader.created", "downloader.revoked"] {
        assert!(kinds.iter().any(|k| k == kind), "{kind}");
    }
}

/// Заглушка серверного fetcher: кладёт файл по ссылке из задания и отвечает как настоящий.
#[derive(Clone)]
struct FakeFetcher {
    storage: Arc<Storage>,
    /// `verified` или код ошибки
    reply: Arc<Mutex<String>>,
    calls: Arc<Mutex<Vec<Value>>>,
}

async fn fake_acquire(State(f): State<FakeFetcher>, Json(body): Json<Value>) -> Json<Value> {
    f.calls.lock().unwrap().push(body.clone());
    let reply = f.reply.lock().unwrap().clone();
    if reply != "verified" {
        return Json(
            json!({ "status": "error", "deezerTrackId": body["deezerTrackId"],
                            "error": { "code": reply, "message": "заглушка" } }),
        );
    }
    let url = body["upload"]["url"].as_str().unwrap();
    upload(&f.storage, url, &[3u8; 333]).await;
    Json(verified(&body["deezerTrackId"], 333, "srvVideo001"))
}

async fn start_fake_fetcher(storage: Arc<Storage>) -> (String, FakeFetcher) {
    let fake = FakeFetcher {
        storage,
        reply: Arc::new(Mutex::new("verified".into())),
        calls: Arc::new(Mutex::new(Vec::new())),
    };
    let app = Router::new()
        .route("/v1/acquire", post(fake_acquire))
        .route(
            "/v1/healthz",
            get(|| async {
                Json(json!({ "ok": true, "version": "0.2.0", "tools": { "ytDlp": "2026.10.26" } }))
            }),
        )
        .with_state(fake.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (base, fake)
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn server_fetcher_is_the_second_executor(db: PgPool) {
    // Хранилище в памяти создаётся вместе с приложением, поэтому адрес заглушки подставляется после
    let mut app = TestApp::new(db).await;
    let (base, fake) = start_fake_fetcher(app.state.storage.clone()).await;
    let mut config = (*app.state.config).clone();
    config.fetcher.server = true;
    config.fetcher.url = base;
    app.state.config = Arc::new(config);
    app.router = dalywave_server::app(app.state.clone());
    let (_, admin) = app.admin("boss").await;
    let me = app.listener("ann").await;
    let track = app.resolve_id("track", LIGHT_ME_UP, &me).await;
    no_intervals(&app.state.db).await;

    // Серверный fetcher включён: загрузчиков нет, но трек не «ждёт загрузчик»
    let res = app.get(&format!("/api/tracks/{track}/stream"), &me).await;
    assert_eq!(res.body["status"], "acquiring");
    let (kind, outcome) = app.run_job("now").await.unwrap();
    assert_eq!(kind, "acquire");
    assert_eq!(outcome, Outcome::Done);
    let call = fake.calls.lock().unwrap()[0].clone();
    assert_eq!(call["deezerTrackId"], LIGHT_ME_UP);
    assert_eq!(call["maxAttempts"], 3);
    assert_eq!(call["upload"]["method"], "PUT");
    let res = app.get(&format!("/api/tracks/{track}/stream"), &me).await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    let acquired_by: String = sqlx::query_scalar(
        "SELECT f.acquired_by FROM audio_files f JOIN tracks t ON t.recording_id = f.recording_id WHERE t.id = $1::uuid",
    )
    .bind(&track)
    .fetch_one(&app.state.db)
    .await
    .unwrap();
    assert_eq!(acquired_by, "server");

    // Бот-проверка: задача отложена без траты попытки, серверный fetcher отдыхает
    *fake.reply.lock().unwrap() = "bot_check".into();
    let other = ep_tracks(&app, &me).await[0];
    acquire::enqueue(
        &app.state.db,
        Request::new(other, AcquireReason::Like, "library"),
    )
    .await
    .unwrap();
    let worker = app.worker();
    let (_, outcome) = worker.run_once("library").await.unwrap().unwrap();
    assert!(
        matches!(outcome, Outcome::Retry(ref m) if m.contains("bot_check")),
        "{outcome:?}"
    );
    let row: (String, i16) =
        sqlx::query_as("SELECT status, attempts FROM jobs WHERE payload->>'trackId' = $1")
            .bind(other.to_string())
            .fetch_one(&app.state.db)
            .await
            .unwrap();
    assert_eq!(row, ("queued".to_string(), 0));
    sqlx::query("UPDATE jobs SET run_after = now() WHERE kind = 'acquire'")
        .execute(&app.state.db)
        .await
        .unwrap();
    assert!(
        worker.run_once("library").await.unwrap().is_none(),
        "пауза после бот-проверки"
    );
    // …а загрузчик на Mac эту задачу взять может
    let (token, _) = new_downloader(&app, &admin, "mac").await;
    let job = claimed(&app, &token).await;
    assert_eq!(job["id"].as_str().unwrap().len(), 36);

    let res = app.get("/api/admin/sources", &admin).await;
    assert_eq!(res.body["server"]["enabled"], true);
    assert_eq!(res.body["server"]["reachable"], true);
    assert_eq!(res.body["server"]["health"]["tools"]["ytDlp"], "2026.10.26");
    assert_eq!(res.body["server"]["ok24h"], 1);
    assert_eq!(res.body["server"]["failed24h"], 1);
}
