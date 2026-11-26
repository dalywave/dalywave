//! Друзья, приватность и совместные плейлисты: запросы и уведомления, ссылка-приглашение, что слушают
//! друзья, страница друга, права участников и синхронизация совместного плейлиста.

use axum::http::{Method, StatusCode};
use chrono::{Duration, Utc};
use serde_json::{Value, json};
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    common::{Auth, TestApp},
    deezer::MODERN_SADNESS,
};

async fn tracks(app: &TestApp, auth: &Auth) -> (String, Vec<Value>) {
    let ep = app.resolve_id("album", MODERN_SADNESS, auth).await;
    let res = app.get(&format!("/api/albums/{ep}"), auth).await;
    (ep, res.body["tracks"].as_array().unwrap().clone())
}

fn ids(tracks: &[Value]) -> Vec<String> {
    tracks
        .iter()
        .map(|t| t["id"].as_str().unwrap().to_string())
        .collect()
}

async fn notifications(app: &TestApp, auth: &Auth) -> Vec<Value> {
    let res = app.get("/api/notifications", auth).await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    res.body["items"].as_array().unwrap().clone()
}

/// Двое — друзья (запрос и согласие).
async fn befriend(app: &TestApp, a: &Auth, b: &Auth) {
    let b_id = app.user_id(b).await;
    let a_id = app.user_id(a).await;
    let res = app
        .post("/api/friends/requests", json!({ "userId": b_id }), a)
        .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    let res = app
        .post(
            &format!("/api/friends/requests/{a_id}/accept"),
            json!({}),
            b,
        )
        .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
}

async fn play(app: &TestApp, auth: &Auth, track: &str, ago_min: i64, ctx: Option<(&str, &str)>) {
    let mut ev = json!({
        "clientEventId": Uuid::now_v7(),
        "trackId": track,
        "startedAt": Utc::now() - Duration::minutes(ago_min),
        "playedMs": 120_000,
        "durationMs": 181_000,
        "endReason": "completed",
    });
    if let Some((kind, id)) = ctx {
        ev["contextType"] = json!(kind);
        ev["contextId"] = json!(id);
    }
    let res = app
        .post(
            "/api/events",
            json!({ "plays": [ev], "feedback": [] }),
            auth,
        )
        .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    assert_eq!(res.body["accepted"], 1, "{}", res.text);
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn requests_accept_decline_and_notify_once(db: PgPool) {
    let app = TestApp::new(db).await;
    let ann = app.listener("ann").await;
    let bob = app.listener("bob").await;
    let cat = app.listener("cat").await;
    let (ann_id, bob_id) = (app.user_id(&ann).await, app.user_id(&bob).await);

    // Боб находит Анну по имени среди людей сервиса
    let res = app.get("/api/friends/people?q=ann", &bob).await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    let found = res.body["items"].as_array().unwrap();
    assert_eq!(found[0]["person"]["id"], ann_id);
    assert_eq!(found[0]["relation"], "none");
    assert!(
        found.iter().all(|p| p["person"]["id"] != bob_id),
        "себя нет"
    );

    // Запрос: у Анны — входящий и уведомление, у Боба — исходящий
    let res = app
        .post("/api/friends/requests", json!({ "userId": ann_id }), &bob)
        .await;
    assert_eq!(res.body["relation"], "outgoing", "{}", res.text);
    let list = app.get("/api/friends", &ann).await.body;
    assert_eq!(list["incoming"][0]["person"]["id"], bob_id);
    assert_eq!(
        app.get("/api/friends", &bob).await.body["outgoing"][0]["person"]["id"],
        ann_id
    );
    let n = notifications(&app, &ann).await;
    assert_eq!(n.len(), 1);
    assert_eq!(n[0]["kind"], "friends");
    assert!(
        n[0]["title"]
            .as_str()
            .unwrap()
            .contains("хочет добавить вас в друзья")
    );
    // Повтор не тревожит второй раз
    app.post("/api/friends/requests", json!({ "userId": ann_id }), &bob)
        .await;
    assert_eq!(notifications(&app, &ann).await.len(), 1);
    assert_eq!(
        app.get(&format!("/api/friends/people?q={}", "bob"), &ann)
            .await
            .body["items"][0]["relation"],
        "incoming"
    );

    // Принять: оба друзья, Бобу — «теперь в друзьях»
    let res = app
        .post(
            &format!("/api/friends/requests/{bob_id}/accept"),
            json!({}),
            &ann,
        )
        .await;
    assert_eq!(res.body["relation"], "friend", "{}", res.text);
    let ann_list = app.get("/api/friends", &ann).await.body;
    assert_eq!(ann_list["friends"][0]["person"]["id"], bob_id);
    assert!(ann_list["incoming"].as_array().unwrap().is_empty());
    let bob_list = app.get("/api/friends", &bob).await.body;
    assert_eq!(bob_list["friends"][0]["person"]["id"], ann_id);
    assert!(bob_list["outgoing"].as_array().unwrap().is_empty());
    let n = notifications(&app, &bob).await;
    assert_eq!(n.len(), 1);
    assert!(n[0]["title"].as_str().unwrap().contains("теперь в друзьях"));

    // Отклонённый запрос: отправитель видит «отправлен», повтор не тревожит, принять можно позже
    let cat_id = app.user_id(&cat).await;
    app.post("/api/friends/requests", json!({ "userId": ann_id }), &cat)
        .await;
    let res = app
        .post(
            &format!("/api/friends/requests/{cat_id}/decline"),
            json!({}),
            &ann,
        )
        .await;
    assert_eq!(res.status, StatusCode::NO_CONTENT);
    assert!(
        app.get("/api/friends", &ann).await.body["incoming"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        app.get("/api/friends", &cat).await.body["outgoing"][0]["person"]["id"],
        ann_id
    );
    app.post("/api/friends/requests", json!({ "userId": ann_id }), &cat)
        .await;
    assert!(
        app.get("/api/friends", &ann).await.body["incoming"]
            .as_array()
            .unwrap()
            .is_empty(),
        "отклонённый повтором не оживает"
    );

    // Встречный запрос — сразу друзья
    let res = app
        .post("/api/friends/requests", json!({ "userId": cat_id }), &ann)
        .await;
    assert_eq!(res.body["relation"], "friend", "{}", res.text);

    // Удалить из друзей — с обеих сторон
    let res = app.delete(&format!("/api/friends/{bob_id}"), &ann).await;
    assert_eq!(res.status, StatusCode::NO_CONTENT);
    assert!(
        app.get("/api/friends", &bob).await.body["friends"]
            .as_array()
            .unwrap()
            .is_empty()
    );

    // Себе — нельзя; неизвестному — 404
    let res = app
        .post("/api/friends/requests", json!({ "userId": ann_id }), &ann)
        .await;
    assert_eq!(res.status, StatusCode::BAD_REQUEST);
    let res = app
        .post(
            "/api/friends/requests",
            json!({ "userId": Uuid::now_v7() }),
            &ann,
        )
        .await;
    assert_eq!(res.status, StatusCode::NOT_FOUND);
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn invite_link_makes_friends(db: PgPool) {
    let app = TestApp::new(db).await;
    let ann = app.listener("ann").await;
    let bob = app.listener("bob").await;
    let link = app.get("/api/friends/invite", &ann).await.body;
    let code = link["code"].as_str().unwrap().to_string();
    assert!(
        link["url"]
            .as_str()
            .unwrap()
            .ends_with(&format!("/friends/add/{code}"))
    );
    // Ссылка постоянная, пока её не сменили
    assert_eq!(
        app.get("/api/friends/invite", &ann).await.body["code"],
        code
    );

    let preview = app.get(&format!("/api/friends/invite/{code}"), &bob).await;
    assert_eq!(preview.body["relation"], "none", "{}", preview.text);
    let res = app
        .post(&format!("/api/friends/invite/{code}"), json!({}), &bob)
        .await;
    assert_eq!(res.body["relation"], "friend", "{}", res.text);
    assert_eq!(
        app.get("/api/friends", &ann).await.body["friends"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    // Своя ссылка — «это я»
    let res = app
        .post(&format!("/api/friends/invite/{code}"), json!({}), &ann)
        .await;
    assert_eq!(res.body["relation"], "me");

    // Новая ссылка — старая не работает
    let fresh = app.post("/api/friends/invite", json!({}), &ann).await.body["code"]
        .as_str()
        .unwrap()
        .to_string();
    assert_ne!(fresh, code);
    let res = app.get(&format!("/api/friends/invite/{code}"), &bob).await;
    assert_eq!(res.status, StatusCode::NOT_FOUND);
    assert_eq!(res.code(), "invite_expired");
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn activity_respects_privacy_and_private_sessions(db: PgPool) {
    let app = TestApp::new(db).await;
    let ann = app.listener("ann").await;
    let bob = app.listener("bob").await;
    let stranger = app.listener("eve").await;
    befriend(&app, &ann, &bob).await;
    let ann_id = app.user_id(&ann).await;
    let (ep, t) = tracks(&app, &ann).await;
    let t = ids(&t);

    // Ничего не слушала — ничего не видно
    let list = app.get("/api/friends", &bob).await.body;
    assert!(list["friends"][0]["activity"].is_null());

    // Последнее прослушивание — с альбомом, откуда играло
    play(&app, &ann, &t[0], 30, Some(("album", &ep))).await;
    let a = &app.get("/api/friends", &bob).await.body["friends"][0]["activity"];
    assert_eq!(a["track"]["id"], t[0]);
    assert_eq!(a["now"], false);
    assert_eq!(a["context"]["type"], "album");
    assert_eq!(a["context"]["id"], ep);

    // Сейчас играет: активное устройство на связи и снимок «играет»
    let album = app.get(&format!("/api/albums/{ep}"), &ann).await.body;
    let state = json!({
        "current": { "uid": "1", "track": album["tracks"][1], "source": "context" },
        "playing": true,
        "positionMs": 5000,
        "durationMs": 181000,
        "context": { "type": "album", "id": ep, "title": "Modern Sadness" },
        "at": Utc::now(),
    });
    sqlx::query(
        "INSERT INTO connect_devices (user_id, scope, id, name, kind, platform, connection, last_seen_at)
         VALUES ($1, $1, 'mac', 'Mac', 'laptop', 'macos', gen_random_uuid(), now())",
    )
    .bind(ann_id.parse::<Uuid>().unwrap())
    .execute(&app.state.db)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO playback_state (user_id, scope, active_connect_id, state) VALUES ($1, $1, 'mac', $2)",
    )
    .bind(ann_id.parse::<Uuid>().unwrap())
    .bind(&state)
    .execute(&app.state.db)
    .await
    .unwrap();
    let a = &app.get("/api/friends", &bob).await.body["friends"][0]["activity"];
    assert_eq!(a["now"], true, "{a}");
    assert_eq!(a["track"]["id"], t[1]);

    // Открыто «Слушать вместе» — друг видит код
    let res = app.post("/api/jam", json!({}), &ann).await;
    assert!(res.status.is_success(), "{}", res.text);
    let a = &app.get("/api/friends", &bob).await.body["friends"][0]["activity"];
    assert_eq!(a["jamCode"], res.body["code"]);

    // Приватная сессия на 6 часов: ничего не видно, и сыгранное в ней — тоже
    let p = app
        .call(
            Method::PATCH,
            "/api/friends/privacy",
            Some(json!({ "privateSession": "hours6" })),
            &ann,
        )
        .await;
    assert_eq!(p.status, StatusCode::OK, "{}", p.text);
    assert_eq!(p.body["privateSession"], true);
    assert!(p.body["privateUntil"].is_string());
    assert!(app.get("/api/friends", &bob).await.body["friends"][0]["activity"].is_null());
    play(&app, &ann, &t[2], 0, None).await;
    // Сессия до выключения — «до» нет
    let p = app
        .call(
            Method::PATCH,
            "/api/friends/privacy",
            Some(json!({ "privateSession": "until_off" })),
            &ann,
        )
        .await;
    assert_eq!(p.body["privateSession"], true);
    assert!(p.body["privateUntil"].is_null(), "{}", p.text);
    let p = app
        .call(
            Method::PATCH,
            "/api/friends/privacy",
            Some(json!({ "privateSession": "off" })),
            &ann,
        )
        .await;
    assert_eq!(p.body["privateSession"], false);
    // Снова видно, что играет сейчас
    let a = &app.get("/api/friends", &bob).await.body["friends"][0]["activity"];
    assert_eq!(a["now"], true);
    // Устройство ушло: последнее — то, что до приватной сессии (сыгранное в ней скрыто)
    sqlx::query("UPDATE connect_devices SET connection = NULL")
        .execute(&app.state.db)
        .await
        .unwrap();
    let a = &app.get("/api/friends", &bob).await.body["friends"][0]["activity"];
    assert_eq!(a["now"], false);
    assert_eq!(a["track"]["id"], t[0], "приватное не показывается и потом");

    // «Показывать друзьям» выключено — ничего
    let p = app
        .call(
            Method::PATCH,
            "/api/friends/privacy",
            Some(json!({ "shareListening": false })),
            &ann,
        )
        .await;
    assert_eq!(p.body["shareListening"], false);
    assert!(app.get("/api/friends", &bob).await.body["friends"][0]["activity"].is_null());

    // Чужой (не друг) видит только имя
    let prof = app.get(&format!("/api/friends/{ann_id}"), &stranger).await;
    assert_eq!(prof.status, StatusCode::OK);
    assert_eq!(prof.body["relation"], "none");
    assert!(prof.body["activity"].is_null());
    assert!(prof.body["playlists"].as_array().unwrap().is_empty());
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn friend_page_shows_open_playlists_top_and_match(db: PgPool) {
    let app = TestApp::new(db).await;
    let ann = app.listener("ann").await;
    let bob = app.listener("bob").await;
    befriend(&app, &ann, &bob).await;
    let ann_id = app.user_id(&ann).await;
    let (_, t) = tracks(&app, &ann).await;
    let t = ids(&t);

    let open = app
        .post(
            "/api/playlists",
            json!({ "title": "Открытый", "trackIds": [t[0]] }),
            &ann,
        )
        .await
        .body["playlist"]["id"]
        .as_str()
        .unwrap()
        .to_string();
    let hidden = app
        .post(
            "/api/playlists",
            json!({ "title": "Свой", "trackIds": [t[1]] }),
            &ann,
        )
        .await
        .body["playlist"]["id"]
        .as_str()
        .unwrap()
        .to_string();
    let res = app
        .call(
            Method::PUT,
            &format!("/api/playlists/{hidden}/visibility"),
            Some(json!({ "friendsCanSee": false })),
            &ann,
        )
        .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    assert_eq!(res.body["friendsCanSee"], false);

    // Оба слушают одного артиста — совпадение и общие артисты
    for i in 0..25 {
        play(&app, &ann, &t[i % t.len()], 60 + i as i64, None).await;
        play(&app, &bob, &t[(i + 1) % t.len()], 60 + i as i64, None).await;
    }
    let prof = app.get(&format!("/api/friends/{ann_id}"), &bob).await;
    assert_eq!(prof.status, StatusCode::OK, "{}", prof.text);
    let b = &prof.body;
    assert_eq!(b["relation"], "friend");
    assert!(b["since"].is_string());
    let pls: Vec<&str> = b["playlists"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["id"].as_str().unwrap())
        .collect();
    assert_eq!(pls, vec![open.as_str()], "скрытый плейлист не виден");
    assert_eq!(b["playlists"][0]["access"], "viewer");
    assert_eq!(b["playlists"][0]["owner"]["id"], ann_id);
    assert_eq!(b["topArtists"].as_array().unwrap().len(), 1);
    assert_eq!(b["topArtists"][0]["plays"], 25);
    assert_eq!(b["tasteMatch"]["percent"], 100, "{}", b["tasteMatch"]);
    assert_eq!(b["commonArtists"].as_array().unwrap().len(), 1);

    // Плейлист друга открывается (только смотреть), скрытый — нет; править его нельзя
    let res = app.get(&format!("/api/playlists/{open}"), &bob).await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    assert_eq!(res.body["playlist"]["access"], "viewer");
    assert_eq!(
        app.get(&format!("/api/playlists/{hidden}"), &bob)
            .await
            .status,
        StatusCode::NOT_FOUND
    );
    let res = app
        .post(
            &format!("/api/playlists/{open}/items"),
            json!({ "trackIds": [t[2]] }),
            &bob,
        )
        .await;
    assert_eq!(res.status, StatusCode::NOT_FOUND, "{}", res.text);
    // Копию снять можно
    let res = app
        .post(&format!("/api/playlists/{open}/duplicate"), json!({}), &bob)
        .await;
    assert_eq!(res.status, StatusCode::CREATED, "{}", res.text);
    assert_eq!(res.body["access"], "owner");
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn collaborative_playlist_rights_and_notifications(db: PgPool) {
    let app = TestApp::new(db).await;
    let ann = app.listener("ann").await;
    let bob = app.listener("bob").await;
    let eve = app.listener("eve").await;
    befriend(&app, &ann, &bob).await;
    let (ann_id, bob_id, eve_id) = (
        app.user_id(&ann).await,
        app.user_id(&bob).await,
        app.user_id(&eve).await,
    );
    let (_, t) = tracks(&app, &ann).await;
    let t = ids(&t);
    let id = app
        .post(
            "/api/playlists",
            json!({ "title": "Вечер", "trackIds": [t[0]] }),
            &ann,
        )
        .await
        .body["playlist"]["id"]
        .as_str()
        .unwrap()
        .to_string();

    // Звать можно только друзей
    let res = app
        .post(
            &format!("/api/playlists/{id}/members"),
            json!({ "userIds": [eve_id] }),
            &ann,
        )
        .await;
    assert_eq!(res.status, StatusCode::BAD_REQUEST, "{}", res.text);
    let res = app
        .post(
            &format!("/api/playlists/{id}/members"),
            json!({ "userIds": [bob_id] }),
            &ann,
        )
        .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    assert_eq!(res.body["collaborative"], true);
    assert_eq!(res.body["members"][0]["id"], bob_id);
    let invited = notifications(&app, &bob).await;
    assert!(
        invited
            .iter()
            .any(|n| n["kind"] == "shared_playlist"
                && n["title"].as_str().unwrap().contains("Вечер"))
    );

    // У Боба плейлист в медиатеке: участник, владелец — Анна
    let list = app.get("/api/playlists", &bob).await.body;
    let mine = list["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["id"] == id.as_str())
        .expect("совместный в медиатеке участника");
    assert_eq!(mine["access"], "member");
    assert_eq!(mine["owner"]["id"], ann_id);

    // Участник добавляет — у трека видно, кто добавил; Анне — одно уведомление на всё
    let res = app
        .post(
            &format!("/api/playlists/{id}/items"),
            json!({ "trackIds": [t[1], t[2]] }),
            &bob,
        )
        .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    assert_eq!(res.body["items"][0]["addedBy"], bob_id);
    let res = app
        .post(
            &format!("/api/playlists/{id}/items"),
            json!({ "trackIds": [t[3]] }),
            &bob,
        )
        .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    let added: Vec<Value> = notifications(&app, &ann)
        .await
        .into_iter()
        .filter(|n| n["kind"] == "shared_playlist")
        .collect();
    assert_eq!(added.len(), 1, "{added:?}");
    assert!(
        added[0]["title"]
            .as_str()
            .unwrap()
            .contains("3 трека в «Вечер»"),
        "{}",
        added[0]["title"]
    );
    assert_eq!(added[0]["count"], 3);
    let detail = app.get(&format!("/api/playlists/{id}"), &ann).await.body;
    assert_eq!(detail["items"][0]["addedBy"], ann_id);
    assert_eq!(detail["items"][3]["addedBy"], bob_id);

    // Участник переставляет и убирает
    let items: Vec<String> = detail["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["id"].as_str().unwrap().to_string())
        .collect();
    let res = app
        .post(
            &format!("/api/playlists/{id}/items/{}/move", items[3]),
            json!({ "afterItemId": null }),
            &bob,
        )
        .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    let res = app
        .delete(&format!("/api/playlists/{id}/items/{}", items[2]), &bob)
        .await;
    assert_eq!(res.status, StatusCode::NO_CONTENT);

    // Только владелец: название, удаление, сет, обложка, убрать другого
    let res = app
        .call(
            Method::PATCH,
            &format!("/api/playlists/{id}"),
            Some(json!({ "title": "Моё" })),
            &bob,
        )
        .await;
    assert_eq!(res.status, StatusCode::FORBIDDEN, "{}", res.text);
    assert_eq!(
        app.delete(&format!("/api/playlists/{id}"), &bob)
            .await
            .status,
        StatusCode::FORBIDDEN
    );
    let res = app
        .post(&format!("/api/playlists/{id}/set"), json!({}), &bob)
        .await;
    assert_eq!(res.status, StatusCode::FORBIDDEN, "{}", res.text);
    let res = app
        .delete(&format!("/api/playlists/{id}/members/{ann_id}"), &bob)
        .await;
    assert_eq!(res.status, StatusCode::FORBIDDEN);

    // Чужой не может ни смотреть, ни править
    assert_eq!(
        app.get(&format!("/api/playlists/{id}"), &eve).await.status,
        StatusCode::NOT_FOUND
    );
    let res = app
        .post(
            &format!("/api/playlists/{id}/items"),
            json!({ "trackIds": [t[0]] }),
            &eve,
        )
        .await;
    assert_eq!(res.status, StatusCode::NOT_FOUND);

    // По ссылке входит кто угодно из сервиса
    let link = app.get(&format!("/api/playlists/{id}/invite"), &ann).await;
    assert_eq!(link.status, StatusCode::OK, "{}", link.text);
    let code = link.body["code"].as_str().unwrap().to_string();
    let preview = app.get(&format!("/api/playlists/join/{code}"), &eve).await;
    assert_eq!(preview.status, StatusCode::OK, "{}", preview.text);
    assert_eq!(preview.body["joined"], false);
    assert_eq!(preview.body["owner"]["id"], ann_id);
    let res = app
        .post(&format!("/api/playlists/join/{code}"), json!({}), &eve)
        .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    assert_eq!(res.body["access"], "member");
    // Владелец убирает участника
    let res = app
        .delete(&format!("/api/playlists/{id}/members/{eve_id}"), &ann)
        .await;
    assert_eq!(res.status, StatusCode::NO_CONTENT);
    assert_eq!(
        app.get(&format!("/api/playlists/{id}"), &eve).await.status,
        StatusCode::NOT_FOUND
    );
    // Новая ссылка — старая не работает
    app.post(&format!("/api/playlists/{id}/invite"), json!({}), &ann)
        .await;
    assert_eq!(
        app.get(&format!("/api/playlists/join/{code}"), &eve)
            .await
            .status,
        StatusCode::NOT_FOUND
    );

    // Участник выходит сам
    let res = app
        .delete(&format!("/api/playlists/{id}/members/{bob_id}"), &bob)
        .await;
    assert_eq!(res.status, StatusCode::NO_CONTENT);
    assert!(
        !app.get("/api/playlists", &bob).await.body["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|p| p["id"] == id.as_str())
    );
    // Владелец свой плейлист удаляет
    assert_eq!(
        app.delete(&format!("/api/playlists/{id}"), &ann)
            .await
            .status,
        StatusCode::NO_CONTENT
    );
}

/// Все страницы синхронизации с начала (или с курсора).
async fn sync_all(app: &TestApp, auth: &Auth, since: Option<&str>) -> (Vec<Value>, String) {
    let mut pages = Vec::new();
    let mut cursor = since.map(str::to_string);
    loop {
        let uri = match &cursor {
            Some(c) => format!("/api/sync?since={c}&limit=3"),
            None => "/api/sync?limit=3".to_string(),
        };
        let res = app.get(&uri, auth).await;
        assert_eq!(res.status, StatusCode::OK, "{}", res.text);
        cursor = Some(res.body["cursor"].as_str().unwrap().to_string());
        let more = res.body["hasMore"].as_bool().unwrap();
        pages.push(res.body);
        if !more {
            break;
        }
    }
    (pages, cursor.unwrap())
}

fn rows<'a>(pages: &'a [Value], key: &str) -> Vec<&'a Value> {
    pages
        .iter()
        .flat_map(|p| p[key].as_array().unwrap().iter())
        .collect()
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn shared_playlist_syncs_to_member_devices(db: PgPool) {
    let app = TestApp::new(db).await;
    let ann = app.listener("ann").await;
    let bob = app.listener("bob").await;
    befriend(&app, &ann, &bob).await;
    let (ann_id, bob_id) = (app.user_id(&ann).await, app.user_id(&bob).await);
    let (_, t) = tracks(&app, &ann).await;
    let t = ids(&t);
    // Плейлист с треками задолго до того, как Боб в него вошёл
    let id = app
        .post(
            "/api/playlists",
            json!({ "title": "Вечер", "trackIds": [t[0], t[1], t[2], t[3]] }),
            &ann,
        )
        .await
        .body["playlist"]["id"]
        .as_str()
        .unwrap()
        .to_string();
    let (_, cursor) = sync_all(&app, &bob, None).await;

    app.post(
        &format!("/api/playlists/{id}/members"),
        json!({ "userIds": [bob_id] }),
        &ann,
    )
    .await;
    // С курсора — плейлист и все его старые позиции (страницами по три), владелец и участники
    let (pages, cursor) = sync_all(&app, &bob, Some(&cursor)).await;
    let pls = rows(&pages, "playlists");
    let pl = pls
        .iter()
        .find(|p| p["id"] == id.as_str())
        .expect("плейлист");
    assert_eq!(pl["collaborative"], true);
    assert_eq!(pl["owner"]["id"], ann_id);
    assert_eq!(pl["members"][0]["id"], bob_id);
    assert!(pl["folderId"].is_null());
    let items: Vec<&Value> = rows(&pages, "playlistItems")
        .into_iter()
        .filter(|i| i["playlistId"] == id.as_str())
        .collect();
    let mut item_ids: Vec<&str> = items.iter().map(|i| i["id"].as_str().unwrap()).collect();
    item_ids.sort();
    item_ids.dedup();
    assert_eq!(item_ids.len(), 4, "все позиции пришли");
    assert!(items.iter().all(|i| i["addedBy"] == ann_id.as_str()));
    // Треки плейлиста пришли в каталог устройства
    assert!(
        rows(&pages, "tracks")
            .iter()
            .any(|tr| tr["id"] == t[3].as_str())
    );

    // Участник закрепил у себя — у владельца плейлист не закреплён
    assert_eq!(
        app.put(&format!("/api/playlists/{id}/pin"), &bob)
            .await
            .status,
        StatusCode::NO_CONTENT
    );
    let (pages, cursor) = sync_all(&app, &bob, Some(&cursor)).await;
    assert_eq!(
        rows(&pages, "playlists")
            .iter()
            .find(|p| p["id"] == id.as_str())
            .unwrap()["pinned"],
        true
    );
    let owner_view = app.get(&format!("/api/playlists/{id}"), &ann).await.body;
    assert_eq!(owner_view["playlist"]["pinned"], false);

    // Владелец убрал трек — tombstone позиции и участнику
    let first = owner_view["items"][0]["id"].as_str().unwrap().to_string();
    app.delete(&format!("/api/playlists/{id}/items/{first}"), &ann)
        .await;
    let (pages, cursor) = sync_all(&app, &bob, Some(&cursor)).await;
    assert!(
        rows(&pages, "tombstones")
            .iter()
            .any(|t| t["entity"] == "playlistItems" && t["key"]["id"] == first.as_str())
    );

    // Боб вышел — tombstone плейлиста
    app.delete(&format!("/api/playlists/{id}/members/{bob_id}"), &bob)
        .await;
    let (pages, _) = sync_all(&app, &bob, Some(&cursor)).await;
    assert!(
        rows(&pages, "tombstones")
            .iter()
            .any(|t| t["entity"] == "playlists" && t["key"]["id"] == id.as_str())
    );
}
