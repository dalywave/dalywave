use axum::http::{Method, StatusCode};
use chrono::{Duration, Utc};
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    common::TestApp,
    deezer::{DEVIL_SINGLE, LOWBORN, MODERN_SADNESS},
};

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn home_shelves_follow_listening(db: PgPool) {
    let app = TestApp::new(db).await;
    let me = app.listener("ann").await;
    // Пустая медиатека — пустая главная, без пустых полок
    let res = app.get("/api/home", &me).await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    assert!(res.body["shelves"].as_array().unwrap().is_empty());
    assert!(res.body["generatedAt"].is_string());

    let artist = app.resolve_id("artist", LOWBORN, &me).await;
    let ep = app.resolve_id("album", MODERN_SADNESS, &me).await;
    let single = app.resolve_id("album", DEVIL_SINGLE, &me).await;
    let album = app.get(&format!("/api/albums/{ep}"), &me).await;
    let t: Vec<String> = album.body["tracks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["id"].as_str().unwrap().to_string())
        .collect();
    let res = app
        .post(
            "/api/playlists",
            json!({ "title": "Вечер", "trackIds": [t[1]] }),
            &me,
        )
        .await;
    let playlist = res.body["playlist"]["id"].as_str().unwrap().to_string();

    let now = Utc::now();
    let play = |track: &str, minutes_ago: i64, context: Option<(&str, &str)>| {
        let mut p = json!({
            "clientEventId": Uuid::now_v7(),
            "trackId": track,
            "startedAt": now - Duration::minutes(minutes_ago),
            "playedMs": 100_000,
            "durationMs": 180_000,
            "endReason": "completed",
        });
        if let Some((kind, id)) = context {
            p["contextType"] = json!(kind);
            p["contextId"] = json!(id);
        }
        p
    };
    let res = app
        .post(
            "/api/events",
            json!({ "plays": [
                play(&t[0], 50, Some(("album", &ep))),
                play(&t[1], 40, Some(("playlist", &playlist))),
                play(&t[2], 30, Some(("artist", &artist))),
                // Без источника — альбом трека
                play(&t[3], 20, None),
                play(&t[4], 10, Some(("search", "lowborn"))),
            ] }),
            &me,
        )
        .await;
    assert_eq!(res.body["accepted"], 5, "{}", res.text);
    app.put(&format!("/api/library/tracks/{}", t[2]), &me).await;
    app.put(&format!("/api/library/tracks/{}", t[4]), &me).await;
    app.call(
        Method::PUT,
        &format!("/api/library/artists/{artist}"),
        None,
        &me,
    )
    .await;
    // Подписка видит релизы артиста; сингл вышел 10 дней назад, EP — давно
    app.get(&format!("/api/artists/{artist}"), &me).await;
    sqlx::query("UPDATE albums SET release_date = current_date - 10 WHERE id = $1::uuid")
        .bind(&single)
        .execute(&app.state.db)
        .await
        .unwrap();
    sqlx::query("UPDATE albums SET release_date = current_date + 30 WHERE deezer_id = 1052752402")
        .execute(&app.state.db)
        .await
        .unwrap();

    // Утром: свежий сингл подписки — и на «Радаре новинок», и в «Новинках подписок»
    crate::taste::morning(&app.state.db).await;
    let res = app.get("/api/home", &me).await;
    let shelves = res.body["shelves"].as_array().unwrap();
    let ids: Vec<&str> = shelves.iter().map(|s| s["id"].as_str().unwrap()).collect();
    assert_eq!(
        ids,
        vec![
            "continue",
            "release-radar",
            "new-releases",
            "recent-likes",
            "top-artists"
        ]
    );
    assert_eq!(shelves[0]["title"], "Продолжить");
    assert_eq!(shelves[0]["kind"], "continue");

    // «Продолжить»: источники от свежих к старым, альбом EP — один раз
    let cont = shelves[0]["items"].as_array().unwrap();
    let kinds: Vec<(&str, &str)> = cont
        .iter()
        .map(|i| (i["type"].as_str().unwrap(), i["id"].as_str().unwrap()))
        .collect();
    assert_eq!(
        kinds,
        vec![
            ("album", ep.as_str()),
            ("artist", artist.as_str()),
            ("playlist", playlist.as_str()),
        ]
    );
    assert_eq!(cont[2]["title"], "Вечер");

    // «Новинки подписок»: релиз за 60 дней, без будущих
    let fresh = shelves[2]["items"].as_array().unwrap();
    assert!(fresh.iter().all(|i| i["type"] == "album"));
    assert!(fresh.iter().any(|i| i["id"] == single.as_str()));
    assert!(fresh.iter().all(|i| i["title"] != "melancholy"));
    assert!(fresh.iter().all(|i| i["id"] != ep.as_str()));

    let likes = shelves[3]["items"].as_array().unwrap();
    assert_eq!(likes[0]["type"], "track");
    assert_eq!(likes[0]["id"], t[4].as_str());
    assert_eq!(likes[0]["liked"], true);

    let top = shelves[4]["items"].as_array().unwrap();
    assert_eq!(top[0]["type"], "artist");
    assert_eq!(top[0]["name"], "Lowborn");
    assert_eq!(top[0]["followed"], true);
}
