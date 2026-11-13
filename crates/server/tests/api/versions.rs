//! Версии песни в один клик: slowed, super slowed, sped up — только настоящие релизы. Поиск у Deezer
//! (записанная выдача «Rushex ZAKHELO»), кеш семьи, обратная связь от версии к оригиналу, ложные
//! находки, сбой Deezer, запоминание выбора и скачивание версии вне очереди.

use axum::http::{Method, StatusCode};
use serde_json::{Value, json};

use crate::{common::TestApp, deezer::fixture};

const ZAKHELO: i64 = 3898084841;
const ZAKHELO_SLOWED: i64 = 3898084851;
const SEARCH: &str = "/search/track";

fn kinds(body: &Value) -> Vec<(String, i64)> {
    body["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| {
            (
                v["kind"].as_str().unwrap().to_string(),
                v["track"]["deezerId"].as_i64().unwrap(),
            )
        })
        .collect()
}

fn id_of(body: &Value, kind: &str) -> String {
    body["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|v| v["kind"] == kind)
        .unwrap_or_else(|| panic!("no {kind}: {body}"))["track"]["id"]
        .as_str()
        .unwrap()
        .to_string()
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn versions_are_found_cached_and_work_both_ways(db: sqlx::PgPool) {
    let app = TestApp::new(db).await;
    let me = app.listener("ann").await;
    let slowed = app.resolve_id("track", ZAKHELO_SLOWED, &me).await;

    let res = app
        .get(&format!("/api/tracks/{slowed}/versions"), &me)
        .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    assert_eq!(res.body["kind"], "slowed");
    assert_eq!(res.body["trackId"], slowed.as_str());
    assert!(res.body["preferred"].is_null());
    // По одной на вид, по порядку; super slowed — популярнее из Ultra и Hyper
    assert_eq!(
        kinds(&res.body),
        vec![
            ("original".into(), ZAKHELO),
            ("slowed".into(), ZAKHELO_SLOWED),
            ("super_slowed".into(), 3898084861),
            ("sped_up".into(), 3898084871),
        ]
    );
    assert_eq!(app.deezer.hits(SEARCH), 1);

    // Повтор — из кеша, без Deezer
    let again = app
        .get(&format!("/api/tracks/{slowed}/versions"), &me)
        .await;
    assert_eq!(kinds(&again.body), kinds(&res.body));
    // От оригинала — та же семья: slowed и sped up обратно, тоже без нового поиска
    let original = id_of(&res.body, "original");
    let back = app
        .get(&format!("/api/tracks/{original}/versions"), &me)
        .await;
    assert_eq!(back.status, StatusCode::OK, "{}", back.text);
    assert_eq!(back.body["kind"], "original");
    assert_eq!(kinds(&back.body), kinds(&res.body));
    let sped = id_of(&res.body, "sped_up");
    let from_sped = app.get(&format!("/api/tracks/{sped}/versions"), &me).await;
    assert_eq!(from_sped.body["kind"], "sped_up");
    assert_eq!(kinds(&from_sped.body).len(), 4);
    assert_eq!(app.deezer.hits(SEARCH), 1, "семья закеширована целиком");

    // Переключение на версию, которой нет на сервере: её скачивание начинается вне очереди
    let stream = app.get(&format!("/api/tracks/{sped}/stream"), &me).await;
    assert_eq!(stream.status, StatusCode::ACCEPTED, "{}", stream.text);
    assert!(
        ["acquiring", "waiting_for_downloader", "queued"]
            .contains(&stream.body["status"].as_str().unwrap()),
        "{}",
        stream.text
    );

    // Нет такого трека
    let missing = app
        .get(&format!("/api/tracks/{}/versions", uuid::Uuid::nil()), &me)
        .await;
    assert_eq!(missing.status, StatusCode::NOT_FOUND);
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn false_matches_are_not_versions(db: sqlx::PgPool) {
    let app = TestApp::new(db).await;
    let me = app.listener("ann").await;
    // Выдача с шумом: ремикс, «Slow Down», чужой «ускоритель», slowed короче оригинала, 8D
    let mut search = fixture("search_track_rushex zakhelo");
    let base = search["data"][0].clone();
    let noise = [
        (9001, "ZAKHELO (Remix)", "(Remix)", 120, None),
        (9002, "Slow Down", "", 140, None),
        (
            9003,
            "ZAKHELO (Slowed + Reverb)",
            "(Slowed + Reverb)",
            135,
            Some("Slowed Kid"),
        ),
        (9004, "ZAKHELO - Slowed", "", 90, None),
        (9005, "ZAKHELO (8D Audio)", "(8D Audio)", 117, None),
    ];
    let data = search["data"].as_array_mut().unwrap();
    // Только оригинал и шум: настоящих версий в этой выдаче нет
    data.retain(|t| t["id"] == ZAKHELO);
    for (id, title, version, secs, artist) in noise {
        let mut t = base.clone();
        t["id"] = json!(id);
        t["title"] = json!(title);
        t["title_short"] = json!(title.trim_end_matches(version).trim());
        t["title_version"] = json!(version);
        t["duration"] = json!(secs);
        t["isrc"] = json!(format!("TEST{id}"));
        if let Some(name) = artist {
            t["artist"]["id"] = json!(id * 10);
            t["artist"]["name"] = json!(name);
        }
        data.push(t);
    }
    app.deezer.set(SEARCH, search);

    // Сама выдача: от оригинала ZAKHELO ни одна из шумных находок не версия
    let zakhelo = app.resolve_id("track", ZAKHELO, &me).await;
    let res = app
        .get(&format!("/api/tracks/{zakhelo}/versions"), &me)
        .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    assert_eq!(kinds(&res.body), vec![("original".into(), ZAKHELO)]);
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn deezer_failure_falls_back_to_catalog(db: sqlx::PgPool) {
    let app = TestApp::new(db).await;
    let me = app.listener("ann").await;
    let slowed = app.resolve_id("track", ZAKHELO_SLOWED, &me).await;
    app.deezer.quota_errors(100);
    let res = app
        .get(&format!("/api/tracks/{slowed}/versions"), &me)
        .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    assert_eq!(kinds(&res.body), vec![("slowed".into(), ZAKHELO_SLOWED)]);
    // Поиск не удался — проверка не засчитана: в следующий раз Deezer спросят снова
    app.deezer.quota_errors(0);
    let before = app.deezer.hits(SEARCH);
    let res = app
        .get(&format!("/api/tracks/{slowed}/versions"), &me)
        .await;
    assert_eq!(app.deezer.hits(SEARCH), before + 1);
    assert_eq!(kinds(&res.body).len(), 4, "{}", res.text);
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn choice_is_remembered_per_artist(db: sqlx::PgPool) {
    let app = TestApp::new(db).await;
    let me = app.listener("ann").await;
    let bob = app.listener("bob").await;
    let slowed = app.resolve_id("track", ZAKHELO_SLOWED, &me).await;
    let res = app
        .get(&format!("/api/tracks/{slowed}/versions"), &me)
        .await;
    let original = id_of(&res.body, "original");
    let artist = res.body["items"][0]["track"]["artists"][0]["id"].clone();

    let res = app
        .call(
            Method::PUT,
            &format!("/api/tracks/{slowed}/versions/choice"),
            Some(json!({ "kind": "slowed" })),
            &me,
        )
        .await;
    assert_eq!(res.status, StatusCode::NO_CONTENT, "{}", res.text);
    let choices = app.get("/api/versions/choices", &me).await;
    assert_eq!(
        choices.body,
        json!({ "items": [{ "artistId": artist, "kind": "slowed" }] })
    );
    // Предлагается и у оригинала того же исполнителя; у другого слушателя — нет
    let res = app
        .get(&format!("/api/tracks/{original}/versions"), &me)
        .await;
    assert_eq!(res.body["preferred"], "slowed");
    let res = app
        .get(&format!("/api/tracks/{original}/versions"), &bob)
        .await;
    assert!(res.body["preferred"].is_null());

    // Вернулся к оригиналу — выбор забыт
    let res = app
        .call(
            Method::PUT,
            &format!("/api/tracks/{original}/versions/choice"),
            Some(json!({ "kind": "original" })),
            &me,
        )
        .await;
    assert_eq!(res.status, StatusCode::NO_CONTENT);
    let choices = app.get("/api/versions/choices", &me).await;
    assert_eq!(choices.body, json!({ "items": [] }));

    let bad = app
        .call(
            Method::PUT,
            &format!("/api/tracks/{original}/versions/choice"),
            Some(json!({ "kind": "nightcore" })),
            &me,
        )
        .await;
    assert_eq!(bad.status, StatusCode::BAD_REQUEST, "{}", bad.text);
}
