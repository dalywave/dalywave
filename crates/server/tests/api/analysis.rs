//! Анализ трека для переходов: только полный анализ, фразы и точки сведения, кеш по ETag.

use axum::{
    body::Body,
    http::{Request, StatusCode, header},
};
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

use crate::common::{Auth, Resp, TestApp};

/// Трек с записью; `analysis` — источник анализа записи (`full`, `preview`) или без анализа.
async fn seed(db: &PgPool, analysis: Option<&str>) -> (Uuid, Uuid) {
    let recording: Uuid = sqlx::query_scalar(
        "INSERT INTO recordings (duration_ms, audio_status) VALUES (192000, 'ready') RETURNING id",
    )
    .fetch_one(db)
    .await
    .unwrap();
    let track: Uuid = sqlx::query_scalar(
        "INSERT INTO tracks (recording_id, title, duration_ms) VALUES ($1, 'Grid', 192000) RETURNING id",
    )
    .bind(recording)
    .fetch_one(db)
    .await
    .unwrap();
    let file: Uuid = sqlx::query_scalar(
        "INSERT INTO audio_files (recording_id, storage_key, codec, container, duration_ms, size_bytes, source)
         VALUES ($1, 'audio/' || $1 || '.m4a', 'opus', 'mp4', 192000, 1000, 'youtube_music') RETURNING id",
    )
    .bind(recording)
    .fetch_one(db)
    .await
    .unwrap();
    let Some(source) = analysis else {
        return (track, file);
    };
    // 120 BPM, 192 с: доля — 500 мс, такт — 2 с; первые 2 с тишины, вступление до 18 с, вокал 20…140 с
    let beats: Vec<i32> = (0..4 * 96).map(|i| i * 500).collect();
    let downbeats: Vec<i32> = beats.iter().step_by(4).copied().collect();
    let mut curve = vec![600i16; 192];
    curve[0] = 0;
    curve[1] = 0;
    sqlx::query(
        "INSERT INTO track_analysis (recording_id, source, file_id, models, duration_ms, bpm, bpm_confidence,
                                     key_camelot, key_confidence, energy, intro_end_ms, beats_ms, downbeats_ms,
                                     vocal_segments, energy_curve)
         VALUES ($1, $2, $3, '{}', 192000, 120, 0.95, '8A', 0.7, 0.6, 18000, $4, $5, $6, $7)",
    )
    .bind(recording)
    .bind(source)
    .bind((source == "full").then_some(file))
    .bind(&beats)
    .bind(&downbeats)
    .bind(json!([[20000, 140000], "junk", [5], [9000, 8000]]))
    .bind(&curve)
    .execute(db)
    .await
    .unwrap();
    (track, file)
}

async fn get_if_none_match(app: &TestApp, uri: &str, auth: &Auth, etag: &str) -> Resp {
    let Auth::Cookie(cookie) = auth else {
        panic!("cookie expected");
    };
    let req = Request::builder()
        .uri(uri)
        .header(header::COOKIE, cookie)
        .header(header::IF_NONE_MATCH, etag)
        .body(Body::empty())
        .unwrap();
    app.send(req).await
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn full_analysis_with_phrases_cues_and_etag(db: PgPool) {
    let app = TestApp::new(db).await;
    let me = app.listener("ann").await;
    let (track, file) = seed(&app.state.db, Some("full")).await;
    let uri = format!("/api/tracks/{track}/analysis");

    let res = app.get(&uri, &me).await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    assert_eq!(res.headers[header::CACHE_CONTROL], "private, max-age=3600");
    let etag = res.headers[header::ETAG].to_str().unwrap().to_string();
    let b = &res.body;
    assert_eq!(b["trackId"], track.to_string());
    assert_eq!(b["fileId"], file.to_string());
    assert_eq!(b["durationMs"], 192000);
    assert_eq!(b["bpm"], 120.0);
    assert_eq!(b["bpmConfidence"], 0.95);
    assert_eq!(b["keyCamelot"], "8A");
    assert_eq!(b["keyConfidence"], 0.7);
    assert_eq!(b["energy"], 0.6);
    assert_eq!(b["introEndMs"], 18000);
    assert!(b["outroStartMs"].is_null());
    assert_eq!(b["beatsMs"].as_array().unwrap().len(), 384);
    assert_eq!(b["downbeatsMs"][1], 2000);
    assert_eq!(b["energyCurve"].as_array().unwrap().len(), 192);
    // Мусор в jsonb пропущен
    assert_eq!(b["vocalSegments"], json!([[20000, 140000]]));
    // Фразы по 8 тактов (16 с) — с перемены энергии на 2-й секунде; по 16 — каждая вторая
    let phrases: Vec<i64> = b["phrasesMs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_i64().unwrap())
        .collect();
    assert_eq!(phrases[0], 2000);
    assert!(phrases.windows(2).all(|w| w[1] - w[0] == 16000));
    let major: Vec<i64> = b["majorPhrasesMs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_i64().unwrap())
        .collect();
    assert!(!major.is_empty() && major.iter().all(|m| phrases.contains(m)));
    assert!(major.windows(2).all(|w| w[1] - w[0] == 32000));
    // Вход — первая фраза со звуком, выход — первая граница фразы после вокала
    assert_eq!(b["cues"], json!({ "mixInMs": 2000, "mixOutMs": 146000 }));

    // Тот же анализ — 304 без тела; список и слабое сравнение тоже понимаются
    let res = get_if_none_match(&app, &uri, &me, &etag).await;
    assert_eq!(res.status, StatusCode::NOT_MODIFIED);
    assert!(res.bytes.is_empty());
    assert_eq!(res.headers[header::ETAG], etag.as_str());
    let listed = format!("\"nope\", {}", etag.trim_start_matches("W/"));
    let res = get_if_none_match(&app, &uri, &me, &listed).await;
    assert_eq!(res.status, StatusCode::NOT_MODIFIED);

    // Анализ обновился — новый ETag, старый не подходит
    sqlx::query("UPDATE track_analysis SET bpm = 121")
        .execute(&app.state.db)
        .await
        .unwrap();
    let res = get_if_none_match(&app, &uri, &me, &etag).await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    assert_ne!(res.headers[header::ETAG], etag.as_str());
    assert_eq!(res.body["bpm"], 121.0);
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn missing_analysis_is_told_apart_from_a_missing_track(db: PgPool) {
    let app = TestApp::new(db).await;
    let me = app.listener("ann").await;
    let (preview, _) = seed(&app.state.db, Some("preview")).await;
    let (bare, _) = seed(&app.state.db, None).await;
    let no_recording: Uuid = sqlx::query_scalar(
        "INSERT INTO tracks (title, duration_ms) VALUES ('Unknown', 1000) RETURNING id",
    )
    .fetch_one(&app.state.db)
    .await
    .unwrap();

    for track in [preview, bare, no_recording] {
        let res = app.get(&format!("/api/tracks/{track}/analysis"), &me).await;
        assert_eq!(res.status, StatusCode::NOT_FOUND, "{track}: {}", res.text);
        assert_eq!(res.code(), "analysis_missing");
    }
    let res = app
        .get(&format!("/api/tracks/{}/analysis", Uuid::now_v7()), &me)
        .await;
    assert_eq!(res.status, StatusCode::NOT_FOUND);
    assert_eq!(res.code(), "not_found");

    // Как и поток — только с правом play
    let res = app
        .get(&format!("/api/tracks/{preview}/analysis"), &Auth::None)
        .await;
    assert_eq!(res.status, StatusCode::UNAUTHORIZED);
    let waiting = Auth::Cookie(app.login(&crate::mock::Person::new("nobody")).await);
    let res = app
        .get(&format!("/api/tracks/{preview}/analysis"), &waiting)
        .await;
    assert_eq!(res.status, StatusCode::FORBIDDEN);
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn openapi_describes_the_analysis(db: PgPool) {
    let app = TestApp::new(db).await;
    let spec = app.get("/api/openapi.json", &Auth::None).await.body;
    let op = &spec["paths"]["/api/tracks/{id}/analysis"]["get"];
    for status in ["200", "304", "404"] {
        assert!(op["responses"][status].is_object(), "{status}");
    }
    let schema = &spec["components"]["schemas"]["TrackAnalysis"];
    let required: Vec<&str> = schema["required"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    for field in [
        "trackId",
        "fileId",
        "phrasesMs",
        "majorPhrasesMs",
        "vocalSegments",
        "cues",
    ] {
        assert!(required.contains(&field), "{field}");
    }
    assert!(spec["components"]["schemas"]["AnalysisCues"].is_object());
}
