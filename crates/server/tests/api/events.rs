use axum::http::StatusCode;
use chrono::{Duration, Utc};
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

use crate::{common::TestApp, deezer::MODERN_SADNESS};

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn events_are_idempotent_and_grouped_into_sessions(db: PgPool) {
    let app = TestApp::new(db).await;
    let me = app.listener("ann").await;
    let ep = app.resolve_id("album", MODERN_SADNESS, &me).await;
    let album = app.get(&format!("/api/albums/{ep}"), &me).await;
    let t: Vec<String> = album.body["tracks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["id"].as_str().unwrap().to_string())
        .collect();
    let artist = album.body["album"]["artist"]["id"]
        .as_str()
        .unwrap()
        .to_string();

    let start = Utc::now() - Duration::hours(3);
    let play = |i: usize, track: &str, offset_min: i64, reason: &str| {
        json!({
            "clientEventId": Uuid::now_v7(),
            "trackId": track,
            "startedAt": start + Duration::minutes(offset_min),
            "playedMs": 120_000,
            "durationMs": 181_000,
            "endReason": reason,
            "contextType": "album",
            "contextId": ep,
            "source": if i.is_multiple_of(2) { "stream" } else { "offline" },
            "output": "headphones",
        })
    };
    // Две сессии: три трека подряд и ещё один через час. Пачка перемешана — как из офлайна
    let plays = vec![
        play(0, &t[2], 6, "skipped"),
        play(1, &t[0], 0, "completed"),
        play(2, &t[1], 3, "completed"),
        play(3, &t[3], 70, "stopped"),
    ];
    let feedback = vec![
        json!({ "clientEventId": Uuid::now_v7(), "kind": "like", "trackId": t[0], "at": start }),
        json!({ "clientEventId": Uuid::now_v7(), "kind": "more_like_this", "artistId": artist,
                "contextType": "wave", "at": start }),
    ];
    let batch = json!({ "plays": plays, "feedback": feedback });
    let res = app.post("/api/events", batch.clone(), &me).await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    assert_eq!(
        res.body,
        json!({ "accepted": 6, "duplicates": 0, "rejected": [] })
    );
    // Повтор той же пачки (приложение не получило ответ) не создаёт вторых записей
    let res = app.post("/api/events", batch, &me).await;
    assert_eq!(
        res.body,
        json!({ "accepted": 0, "duplicates": 6, "rejected": [] })
    );

    let sessions: Vec<Option<Uuid>> =
        sqlx::query_scalar("SELECT listening_session FROM play_events ORDER BY started_at")
            .fetch_all(&app.state.db)
            .await
            .unwrap();
    assert_eq!(sessions.len(), 4);
    assert!(sessions.iter().all(Option::is_some));
    assert_eq!(sessions[0], sessions[1]);
    assert_eq!(sessions[1], sessions[2]);
    assert_ne!(sessions[2], sessions[3], "час тишины — новая сессия");

    let bad_id = Uuid::now_v7();
    let unknown_id = Uuid::now_v7();
    let fb_id = Uuid::now_v7();
    let res = app
        .post(
            "/api/events",
            json!({
                "plays": [
                    { "clientEventId": bad_id, "trackId": t[0], "startedAt": start, "playedMs": -5,
                      "durationMs": 1000, "endReason": "completed" },
                    { "clientEventId": unknown_id, "trackId": Uuid::now_v7(), "startedAt": start,
                      "playedMs": 5, "durationMs": 1000, "endReason": "completed" },
                ],
                "feedback": [ { "clientEventId": fb_id, "kind": "dislike", "at": start } ],
            }),
            &me,
        )
        .await;
    assert_eq!(res.status, StatusCode::OK);
    assert_eq!(res.body["accepted"], 0);
    let rejected: Vec<(String, String)> = res.body["rejected"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| {
            (
                r["clientEventId"].as_str().unwrap().to_string(),
                r["reason"].as_str().unwrap().to_string(),
            )
        })
        .collect();
    assert!(rejected.contains(&(bad_id.to_string(), "invalid".into())));
    assert!(rejected.contains(&(unknown_id.to_string(), "unknown_track".into())));
    assert!(rejected.contains(&(fb_id.to_string(), "invalid".into())));
    // Неизвестный вид события — ошибка формата
    let res = app
        .post(
            "/api/events",
            json!({ "plays": [{ "clientEventId": Uuid::now_v7(), "trackId": t[0], "startedAt": start,
                                "playedMs": 1, "durationMs": 1, "endReason": "teleported" }] }),
            &me,
        )
        .await;
    assert_eq!(res.status, StatusCode::BAD_REQUEST);

    // История: новые первыми, курсором
    let res = app.get("/api/me/history?limit=3", &me).await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    let items = res.body["items"].as_array().unwrap();
    assert_eq!(items.len(), 3);
    assert_eq!(items[0]["track"]["id"], t[3].as_str());
    assert_eq!(items[0]["endReason"], "stopped");
    assert_eq!(items[0]["contextType"], "album");
    let cursor = res.body["nextCursor"].as_str().unwrap();
    let res = app
        .get(&format!("/api/me/history?limit=3&cursor={cursor}"), &me)
        .await;
    assert_eq!(res.body["items"].as_array().unwrap().len(), 1);
    assert_eq!(res.body["items"][0]["track"]["id"], t[0].as_str());
    assert!(res.body["nextCursor"].is_null());

    // Чужая история не видна
    let other = app.listener("bob").await;
    assert!(
        app.get("/api/me/history", &other).await.body["items"]
            .as_array()
            .unwrap()
            .is_empty()
    );
}
