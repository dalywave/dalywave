use std::collections::HashMap;

use axum::http::{Method, StatusCode};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sqlx::PgPool;

use crate::{
    common::{Auth, TestApp},
    deezer::{LIGHT_ME_UP, fixture},
    mock::Person,
};

const LIGHT_ME_UP_ISRC: &str = "GBWUL2035890";
const DEVIL: i64 = 3657134502;

async fn recognize(app: &TestApp, body: Value, auth: &Auth) -> Value {
    let res = app.post("/api/recognize/resolve", body, auth).await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    res.body
}

async fn rows(app: &TestApp) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM recognitions")
        .fetch_one(&app.state.db)
        .await
        .unwrap()
}

/// Токен приложения `dwd_…` того же человека: вход через системный браузер и обмен кода.
async fn device_token(app: &TestApp, sub: &str) -> Auth {
    let verifier = "recognize-verifier-0123456789-abcdefghijklmnopqrstuvwxyz";
    let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
    let (authorize, cookie) = app
        .start(&format!(
            "/api/auth/app/start?redirect_uri=dalywave%3A%2F%2Fauth&code_challenge={challenge}\
             &code_challenge_method=S256&state=s1&device_name=iPhone&platform=ios"
        ))
        .await;
    let (code, state) = app.provider.approve(&authorize, &Person::new(sub));
    let back = app.callback(&code, &state, Some(&cookie)).await.location();
    let q: HashMap<String, String> = url::Url::parse(&back)
        .unwrap()
        .query_pairs()
        .into_owned()
        .collect();
    let res = app
        .call(
            Method::POST,
            "/api/auth/app/token",
            Some(json!({ "code": q["code"], "codeVerifier": verifier, "appVersion": "0.1.0" })),
            &Auth::None,
        )
        .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    Auth::Bearer(res.body["token"].as_str().unwrap().to_string())
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn isrc_hit_becomes_a_catalog_track(db: PgPool) {
    let app = TestApp::new(db).await;
    let me = app.listener("ann").await;
    app.deezer.set(
        &format!("/track/isrc:{LIGHT_ME_UP_ISRC}"),
        fixture(&format!("track_{LIGHT_ME_UP}")),
    );
    let body = recognize(
        &app,
        json!({
            "isrc": "gb-wul-20-35890",
            "title": "  Light Me Up ",
            "artist": "Lowborn & vista",
            "appleMusicId": "1534567890",
            "shazamId": "552112233",
            "artworkUrl": "https://is1-ssl.mzstatic.com/image/thumb/a.jpg/400x400bb.jpg",
        }),
        &me,
    )
    .await;
    assert_eq!(body["title"], "Light Me Up");
    assert_eq!(body["artist"], "Lowborn & vista");
    assert_eq!(body["isrc"], LIGHT_ME_UP_ISRC);
    assert!(body["recognizedAt"].is_string());
    assert!(body["artworkUrl"].as_str().unwrap().starts_with("https://"));
    let track = &body["track"];
    assert_eq!(track["deezerId"], LIGHT_ME_UP);
    assert_eq!(track["title"], "Light Me Up");
    assert_eq!(track["isrc"], LIGHT_ME_UP_ISRC);
    assert_eq!(track["audioStatus"], "none");
    // Полный трек из ответа `track/isrc:`: оба участника на месте, поиск не понадобился
    assert_eq!(track["artists"].as_array().unwrap().len(), 2);
    assert_eq!(app.deezer.hits("/search/track"), 0);
    // Тот же трек, что отдаёт каталог
    let id = track["id"].as_str().unwrap();
    let page = app.get(&format!("/api/tracks/{id}"), &me).await;
    assert_eq!(page.status, StatusCode::OK);
    assert_eq!(page.body["deezerId"], LIGHT_ME_UP);

    // Второй раз ISRC уже в каталоге — в Deezer не ходим; работает и с токеном приложения
    let before = app.deezer.total_hits();
    let phone = device_token(&app, "ann").await;
    let again = recognize(
        &app,
        json!({ "isrc": LIGHT_ME_UP_ISRC, "title": "Light Me Up", "artist": "Lowborn" }),
        &phone,
    )
    .await;
    assert_eq!(again["track"]["id"], track["id"]);
    assert!(again["artworkUrl"].is_null());
    assert_eq!(app.deezer.total_hits(), before);
    assert_eq!(rows(&app).await, 2);
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn isrc_miss_falls_back_to_search(db: PgPool) {
    let app = TestApp::new(db).await;
    let me = app.listener("ann").await;
    app.deezer
        .set("/search/track", fixture("search_track_lowborn"));
    let devil = fixture("search_track_lowborn")["data"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["id"] == DEVIL)
        .unwrap()
        .clone();
    app.deezer.set(&format!("/track/{DEVIL}"), devil);

    let body = recognize(
        &app,
        json!({ "isrc": "XX0000000000", "title": "Devil", "artist": "Lowborn" }),
        &me,
    )
    .await;
    assert_eq!(app.deezer.hits("/track/isrc:XX0000000000"), 1);
    assert!(app.deezer.hits("/search/track") >= 1);
    assert_eq!(body["track"]["deezerId"], DEVIL);
    assert_eq!(body["track"]["title"], "Devil");
    // В истории — как прислал Shazam, а не как в каталоге
    assert_eq!(body["isrc"], "XX0000000000");
    // Трек целиком догружен по id
    assert_eq!(app.deezer.hits(&format!("/track/{DEVIL}")), 1);
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn unknown_song_is_still_remembered(db: PgPool) {
    let app = TestApp::new(db).await;
    let me = app.listener("ann").await;
    let body = recognize(
        &app,
        json!({ "isrc": null, "title": "Песня, которой нет", "artist": "Никто", "artworkUrl": null }),
        &me,
    )
    .await;
    assert!(body["track"].is_null());
    assert!(body["isrc"].is_null());
    let history = app.get("/api/recognize/history", &me).await;
    assert_eq!(history.status, StatusCode::OK, "{}", history.text);
    let items = history.body["items"].as_array().unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["id"], body["id"]);
    assert_eq!(items[0]["title"], "Песня, которой нет");
    assert!(items[0]["track"].is_null());
    assert!(history.body["nextCursor"].is_null());
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn bad_input_and_deezer_outage_save_nothing(db: PgPool) {
    let app = TestApp::new(db).await;
    let me = app.listener("ann").await;
    for body in [
        json!({ "title": "  ", "artist": "Lowborn" }),
        json!({ "title": "Devil", "artist": "" }),
        json!({ "title": "x".repeat(301), "artist": "Lowborn" }),
        json!({ "title": "Devil", "artist": "Lowborn", "artworkUrl": "http://example.com/a.jpg" }),
        json!({ "title": "Devil" }),
    ] {
        let res = app.post("/api/recognize/resolve", body.clone(), &me).await;
        assert_eq!(res.status, StatusCode::BAD_REQUEST, "{body}: {}", res.text);
        assert_eq!(res.code(), "bad_request");
    }
    // Без входа — 401
    let res = app
        .post(
            "/api/recognize/resolve",
            json!({ "title": "Devil", "artist": "Lowborn" }),
            &Auth::None,
        )
        .await;
    assert_eq!(res.status, StatusCode::UNAUTHORIZED);

    // Deezer молчит — 503, а в историю ничего: приложение повторит
    app.deezer.quota_errors(100);
    let res = app
        .post(
            "/api/recognize/resolve",
            json!({ "title": "Devil", "artist": "Lowborn" }),
            &me,
        )
        .await;
    assert_eq!(res.status, StatusCode::SERVICE_UNAVAILABLE, "{}", res.text);
    assert_eq!(res.code(), "unavailable");
    app.deezer.quota_errors(0);
    assert_eq!(rows(&app).await, 0);
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn history_pages_and_belongs_to_its_owner(db: PgPool) {
    let app = TestApp::new(db).await;
    let ann = app.listener("ann").await;
    let bob = app.listener("bob").await;
    let mut ids = Vec::new();
    for title in ["Первая", "Вторая", "Третья"] {
        let body = recognize(&app, json!({ "title": title, "artist": "Никто" }), &ann).await;
        ids.push(body["id"].as_str().unwrap().to_string());
    }
    let bobs = recognize(&app, json!({ "title": "Чужая", "artist": "Никто" }), &bob).await;

    let first = app.get("/api/recognize/history?limit=2", &ann).await;
    assert_eq!(first.status, StatusCode::OK, "{}", first.text);
    let titles: Vec<&str> = first.body["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["title"].as_str().unwrap())
        .collect();
    assert_eq!(titles, ["Третья", "Вторая"]);
    let cursor = first.body["nextCursor"].as_str().unwrap();
    let second = app
        .get(
            &format!("/api/recognize/history?limit=2&cursor={cursor}"),
            &ann,
        )
        .await;
    let items = second.body["items"].as_array().unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["title"], "Первая");
    assert!(second.body["nextCursor"].is_null());

    let theirs = app.get("/api/recognize/history", &bob).await;
    let items = theirs.body["items"].as_array().unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["id"], bobs["id"]);

    let res = app.get("/api/recognize/history?limit=0", &ann).await;
    assert_eq!(res.status, StatusCode::BAD_REQUEST);
    let res = app.get("/api/recognize/history?cursor=nope", &ann).await;
    assert_eq!(res.status, StatusCode::BAD_REQUEST);

    // Чужую запись не удалить и не узнать, есть ли она
    let res = app
        .delete(&format!("/api/recognize/history/{}", ids[0]), &bob)
        .await;
    assert_eq!(res.status, StatusCode::NOT_FOUND);
    let res = app
        .delete(&format!("/api/recognize/history/{}", ids[0]), &ann)
        .await;
    assert_eq!(res.status, StatusCode::NO_CONTENT, "{}", res.text);
    let res = app
        .delete(&format!("/api/recognize/history/{}", ids[0]), &ann)
        .await;
    assert_eq!(res.status, StatusCode::NOT_FOUND);
    let left = app.get("/api/recognize/history", &ann).await;
    assert_eq!(left.body["items"].as_array().unwrap().len(), 2);
    assert_eq!(rows(&app).await, 3);
}
