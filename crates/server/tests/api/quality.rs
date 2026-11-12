//! Качество звука: какой файл отдаёт поток по сети и настройкам слушателя (lossless только при «Высоком»),
//! «Экономия» — облегчённая копия Opus 64 кбит/с, которую воркер делает при первом запросе, и её чистка
//! по пределу; загрузки на устройство — по своему качеству.

use axum::http::{Method, StatusCode};
use dalywave_server::renditions;
use serde_json::{Value, json};
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    common::{Auth, TestApp},
    fetcher::ep_tracks,
};

fn ffmpeg() -> String {
    std::env::var("DALYWAVE_FFMPEG").unwrap_or_else(|_| "ffmpeg".into())
}

fn ffmpeg_available() -> bool {
    std::process::Command::new(ffmpeg())
        .arg("-version")
        .output()
        .is_ok_and(|o| o.status.success())
}

/// Три секунды синуса в Opus внутри MP4 — как файлы с YouTube.
fn opus_bytes() -> Vec<u8> {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("a.m4a");
    let res = std::process::Command::new(ffmpeg())
        .args([
            "-v",
            "error",
            "-f",
            "lavfi",
            "-i",
            "sine=frequency=440:duration=3",
            "-ac",
            "2",
            "-c:a",
            "libopus",
            "-b:a",
            "160k",
            "-f",
            "mp4",
        ])
        .arg(&out)
        .output()
        .unwrap();
    assert!(
        res.status.success(),
        "{}",
        String::from_utf8_lossy(&res.stderr)
    );
    std::fs::read(out).unwrap()
}

async fn recording_of(db: &PgPool, track: Uuid) -> Uuid {
    sqlx::query_scalar("SELECT recording_id FROM tracks WHERE id = $1")
        .bind(track)
        .fetch_one(db)
        .await
        .unwrap()
}

async fn add_file(
    db: &PgPool,
    rec: Uuid,
    codec: &str,
    container: &str,
    lossless: bool,
    kbps: Option<i32>,
) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO audio_files (recording_id, storage_key, codec, container, lossless, bitrate_kbps, duration_ms,
                                  size_bytes, source, loudness_lufs, true_peak_db, sha256)
         VALUES ($1, 'audio/' || $1 || '/' || gen_random_uuid() || '.' || $2, $2, $3, $4, $5, 3000, 4096,
                 CASE WHEN $4 THEN 'upload' ELSE 'youtube' END, -9.5, -0.3, $6)
         RETURNING id",
    )
    .bind(rec)
    .bind(codec)
    .bind(container)
    .bind(lossless)
    .bind(kbps)
    .bind("ab".repeat(32))
    .fetch_one(db)
    .await
    .unwrap()
}

async fn make_primary(db: &PgPool, rec: Uuid, file: Uuid) {
    sqlx::query("UPDATE recordings SET audio_status = 'ready', primary_file_id = $2 WHERE id = $1")
        .bind(rec)
        .bind(file)
        .execute(db)
        .await
        .unwrap();
}

async fn stream(app: &TestApp, auth: &Auth, track: Uuid, query: &str) -> Value {
    let res = app
        .get(&format!("/api/tracks/{track}/stream{query}"), auth)
        .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    res.body
}

async fn settings(app: &TestApp, auth: &Auth, body: Value) {
    let res = app
        .call(Method::PATCH, "/api/settings", Some(body), auth)
        .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
}

async fn transcode_jobs(db: &PgPool) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM jobs WHERE kind = 'transcode' AND status = 'queued'")
        .fetch_one(db)
        .await
        .unwrap()
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn lossless_plays_only_on_high_quality(db: PgPool) {
    let app = TestApp::new(db).await;
    let me = app.listener("ann").await;
    let track = ep_tracks(&app, &me).await[0];
    let db = &app.state.db;
    let rec = recording_of(db, track).await;
    let opus = add_file(db, rec, "opus", "mp4", false, Some(136)).await;
    let flac = add_file(db, rec, "flac", "flac", true, Some(900)).await;
    make_primary(db, rec, flac).await;

    // Сайт (Wi-Fi), «Высокое» по умолчанию — загруженный FLAC
    let s = stream(&app, &me, track, "").await;
    assert_eq!(s["fileId"], flac.to_string());
    assert_eq!(
        (s["codec"].as_str(), s["lossless"].as_bool()),
        (Some("flac"), Some(true))
    );
    assert_eq!(s["quality"], "high");
    // Сотовая, «Обычное» — Opus
    let s = stream(&app, &me, track, "?network=cellular").await;
    assert_eq!(s["fileId"], opus.to_string());
    assert_eq!(s["quality"], "normal");
    assert_eq!(s["lossless"], false);
    assert_eq!(s["loudnessLufs"], -9.5);
    // Wi-Fi «Обычное» — тоже Opus; явное качество важнее настроек
    settings(&app, &me, json!({ "qualityWifi": "normal" })).await;
    assert_eq!(
        stream(&app, &me, track, "?network=wifi").await["fileId"],
        opus.to_string()
    );
    assert_eq!(
        stream(&app, &me, track, "?quality=high").await["fileId"],
        flac.to_string()
    );
    let bad = app
        .get(&format!("/api/tracks/{track}/stream?quality=ultra"), &me)
        .await;
    assert_eq!(bad.status, StatusCode::BAD_REQUEST);

    // Только lossless — его отдаёт и «Обычное»
    let other = ep_tracks(&app, &me).await[1];
    let rec2 = recording_of(db, other).await;
    let only = add_file(db, rec2, "flac", "flac", true, None).await;
    make_primary(db, rec2, only).await;
    assert_eq!(
        stream(&app, &me, other, "?quality=normal").await["fileId"],
        only.to_string()
    );
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn saver_rendition_is_made_once_and_trimmed(db: PgPool) {
    let app = TestApp::new(db).await;
    let me = app.listener("ann").await;
    let track = ep_tracks(&app, &me).await[0];
    let db = &app.state.db;
    let rec = recording_of(db, track).await;
    let source = add_file(db, rec, "opus", "mp4", false, Some(160)).await;
    make_primary(db, rec, source).await;
    settings(&app, &me, json!({ "qualityCellular": "saver" })).await;

    // Первый запрос: копии нет — играет обычный файл, копия встаёт в очередь (одна задача на файл)
    let s = stream(&app, &me, track, "?network=cellular").await;
    assert_eq!(s["fileId"], source.to_string());
    assert_eq!(s["quality"], "normal");
    assert_eq!(transcode_jobs(db).await, 1);
    stream(&app, &me, track, "?network=cellular").await;
    assert_eq!(transcode_jobs(db).await, 1);
    // По Wi-Fi копия не нужна
    assert_eq!(
        stream(&app, &me, track, "?network=wifi").await["quality"],
        "high"
    );

    if !ffmpeg_available() {
        eprintln!("ffmpeg not found: skipping the transcode part");
        return;
    }
    let key: String = sqlx::query_scalar("SELECT storage_key FROM audio_files WHERE id = $1")
        .bind(source)
        .fetch_one(db)
        .await
        .unwrap();
    app.state
        .storage
        .put(&key, opus_bytes(), "audio/mp4", None)
        .await
        .unwrap();
    let (kind, outcome) = app.run_job(renditions::LANE).await.expect("transcode job");
    assert_eq!(kind, "transcode");
    assert!(
        matches!(outcome, dalywave_server::jobs::worker::Outcome::Done),
        "{outcome:?}"
    );
    let s = stream(&app, &me, track, "?network=cellular").await;
    assert_eq!(s["quality"], "saver", "{s}");
    assert_eq!(
        (s["codec"].as_str(), s["container"].as_str()),
        (Some("opus"), Some("mp4"))
    );
    assert_eq!(s["bitrateKbps"], 64);
    assert_eq!(s["loudnessLufs"], -9.5, "громкость — источника");
    assert_ne!(s["fileId"], source.to_string());
    let copy_key = renditions::key(source);
    let bytes = app.state.storage.get_bytes(&copy_key).await.unwrap();
    assert!(bytes.len() > 1000 && &bytes[4..8] == b"ftyp", "MP4");
    // Повторная задача ничего не делает
    renditions::request(&mut app.state.db.acquire().await.unwrap(), source)
        .await
        .unwrap();

    // Чистка: в пределе — остаётся; предел ноль — уходит из базы и хранилища
    let t = renditions::trim(db, &app.state.storage, 1 << 30)
        .await
        .unwrap();
    assert_eq!(t, renditions::Trimmed::default());
    let t = renditions::trim(db, &app.state.storage, 0).await.unwrap();
    assert_eq!((t.evicted, t.deleted), (1, 1));
    assert!(app.state.storage.get_bytes(&copy_key).await.is_err());
    let s = stream(&app, &me, track, "?network=cellular").await;
    assert_eq!(s["quality"], "normal", "копии нет — снова обычный файл");

    // Сменился основной файл — копия старого устаревает
    let (kind, _) = app
        .run_job(renditions::LANE)
        .await
        .expect("transcode again");
    assert_eq!(kind, "transcode");
    let newer = add_file(db, rec, "opus", "mp4", false, Some(160)).await;
    make_primary(db, rec, newer).await;
    let t = renditions::trim(db, &app.state.storage, 1 << 30)
        .await
        .unwrap();
    assert_eq!((t.stale, t.deleted), (1, 1));
}
