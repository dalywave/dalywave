//! «Поделиться → История (видео)»: звук истории (WAV) и сборка MP4 (H.264 + AAC, 1080×1920, 15 с).

use std::path::Path;

use axum::{
    body::Body,
    http::{Method, Request, StatusCode, header},
};
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    common::{Auth, PUBLIC_URL, TestApp},
    deezer::LIGHT_ME_UP,
};

fn tool(name: &str) -> String {
    std::env::var(format!("DALYWAVE_{}", name.to_ascii_uppercase())).unwrap_or_else(|_| name.into())
}

fn available() -> bool {
    ["ffmpeg", "ffprobe"].iter().all(|t| {
        std::process::Command::new(tool(t))
            .arg("-version")
            .output()
            .is_ok_and(|o| o.status.success())
    })
}

fn ffmpeg(args: &[&str]) {
    let out = std::process::Command::new(tool("ffmpeg"))
        .args(["-hide_banner", "-loglevel", "error", "-y"])
        .args(args)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// ffprobe: `codec_type,codec_name,width,height` потоков и длительность файла.
fn probe(bytes: &[u8]) -> (Vec<String>, f64) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("probe");
    std::fs::write(&path, bytes).unwrap();
    let out = std::process::Command::new(tool("ffprobe"))
        .args([
            "-v",
            "error",
            "-show_entries",
            "stream=codec_type,codec_name,width,height:format=duration",
            "-of",
            "csv=p=0",
        ])
        .arg(&path)
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&out.stdout).to_string();
    let mut lines: Vec<String> = text
        .lines()
        .map(str::to_string)
        .filter(|l| !l.is_empty())
        .collect();
    let duration = lines.pop().unwrap().parse::<f64>().unwrap();
    (lines, duration)
}

/// Трек со звуком на сервере: минута синуса в хранилище в памяти.
async fn ready_track(app: &TestApp, auth: &Auth, dir: &Path) -> String {
    let id = app.resolve_id("track", LIGHT_ME_UP, auth).await;
    let audio = dir.join("track.m4a");
    ffmpeg(&[
        "-f",
        "lavfi",
        "-i",
        "sine=frequency=440:duration=60",
        "-ac",
        "2",
        "-c:a",
        "aac",
        audio.to_str().unwrap(),
    ]);
    let recording: Uuid = sqlx::query_scalar("SELECT recording_id FROM tracks WHERE id = $1::uuid")
        .bind(&id)
        .fetch_one(&app.state.db)
        .await
        .unwrap();
    let key = format!("audio/{recording}.m4a");
    app.state
        .storage
        .put(&key, std::fs::read(&audio).unwrap(), "audio/mp4", None)
        .await
        .unwrap();
    let file: Uuid = sqlx::query_scalar(
        "INSERT INTO audio_files (recording_id, storage_key, codec, container, duration_ms, size_bytes, source)
         VALUES ($1, $2, 'aac', 'mp4', 60000, 1000, 'upload') RETURNING id",
    )
    .bind(recording)
    .bind(&key)
    .fetch_one(&app.state.db)
    .await
    .unwrap();
    sqlx::query("UPDATE recordings SET audio_status = 'ready', primary_file_id = $2 WHERE id = $1")
        .bind(recording)
        .bind(file)
        .execute(&app.state.db)
        .await
        .unwrap();
    id
}

async fn post_video(
    app: &TestApp,
    uri: &str,
    auth: &Auth,
    mime: &str,
    body: Vec<u8>,
) -> crate::common::Resp {
    let Auth::Cookie(cookie) = auth else {
        panic!("cookie auth")
    };
    let req = Request::builder()
        .method(Method::POST)
        .uri(uri)
        .header(header::COOKIE, cookie)
        .header(header::ORIGIN, PUBLIC_URL)
        .header(header::CONTENT_TYPE, mime)
        .body(Body::from(body))
        .unwrap();
    app.send(req).await
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn story_audio_and_video_are_playable(db: PgPool) {
    if !available() {
        eprintln!("ffmpeg not found: skipping the story test");
        return;
    }
    let app = TestApp::new(db).await;
    let me = app.listener("ann").await;
    let dir = tempfile::tempdir().unwrap();
    let id = ready_track(&app, &me, dir.path()).await;

    // Звук: 15 с WAV
    let res = app.get(&format!("/api/tracks/{id}/story/audio"), &me).await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    assert_eq!(res.headers[header::CONTENT_TYPE], "audio/wav");
    let (streams, duration) = probe(&res.bytes);
    assert_eq!(streams, vec!["pcm_s16le,audio".to_string()], "{streams:?}");
    assert!((duration - 15.0).abs() < 0.05, "{duration}");

    // Видео без звука (H.264, 1080×1920) → MP4 с AAC, видео копией
    let silent = dir.path().join("silent.mp4");
    ffmpeg(&[
        "-f",
        "lavfi",
        "-i",
        "testsrc2=size=1080x1920:rate=30:duration=15",
        "-an",
        "-c:v",
        "libx264",
        "-preset",
        "ultrafast",
        "-pix_fmt",
        "yuv420p",
        silent.to_str().unwrap(),
    ]);
    let res = post_video(
        &app,
        &format!("/api/tracks/{id}/story"),
        &me,
        "video/mp4",
        std::fs::read(&silent).unwrap(),
    )
    .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    assert_eq!(res.headers[header::CONTENT_TYPE], "video/mp4");
    let (streams, duration) = probe(&res.bytes);
    assert_eq!(
        streams,
        vec!["h264,video,1080,1920".to_string(), "aac,audio".to_string()],
        "{streams:?}"
    );
    assert!((duration - 15.0).abs() < 0.1, "{duration}");

    // WebM (VP9 из MediaRecorder) перекодируется в H.264
    let webm = dir.path().join("silent.webm");
    ffmpeg(&[
        "-f",
        "lavfi",
        "-i",
        "testsrc2=size=360x640:rate=30:duration=15",
        "-an",
        "-c:v",
        "libvpx-vp9",
        "-deadline",
        "realtime",
        "-cpu-used",
        "8",
        webm.to_str().unwrap(),
    ]);
    let res = post_video(
        &app,
        &format!("/api/tracks/{id}/story"),
        &me,
        "video/webm",
        std::fs::read(&webm).unwrap(),
    )
    .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    let (streams, _) = probe(&res.bytes);
    assert_eq!(streams[0], "h264,video,360,640");
    assert_eq!(streams[1], "aac,audio");

    // Не видео — 422
    let res = post_video(
        &app,
        &format!("/api/tracks/{id}/story"),
        &me,
        "video/mp4",
        b"not a video".to_vec(),
    )
    .await;
    assert_eq!(res.status, StatusCode::UNPROCESSABLE_ENTITY, "{}", res.text);
    // Нет такого трека — 404
    let res = app
        .get(&format!("/api/tracks/{}/story/audio", Uuid::nil()), &me)
        .await;
    assert_eq!(res.status, StatusCode::NOT_FOUND);
}
