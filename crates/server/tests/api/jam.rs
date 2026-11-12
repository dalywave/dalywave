//! «Слушать вместе»: сессия хозяина, вход гостя по коду, треки гостей в очереди активного
//! устройства, «что играет» у гостя. Безопасность: токен гостя не открывает ничего, кроме
//! `/api/jam/guest/*`; закрытая, простоявшая и чужая сессия не принимается; лимиты.

use std::time::Duration;

use axum::http::{Method, StatusCode, header};
use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use sqlx::PgPool;
use tokio_tungstenite::{
    MaybeTlsStream, WebSocketStream, connect_async,
    tungstenite::{Message, client::IntoClientRequest},
};

use crate::{
    common::{Auth, PUBLIC_URL, TestApp},
    deezer::{CHURCH_ON_SUNDAY, LIGHT_ME_UP},
};

type Socket = WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>;

async fn start(app: &TestApp, host: &Auth) -> Value {
    let res = app.post("/api/jam", json!({}), host).await;
    assert_eq!(res.status, StatusCode::CREATED, "{}", res.text);
    res.body
}

async fn join(app: &TestApp, code: &str, name: &str) -> (Auth, Value) {
    let res = app
        .post(
            &format!("/api/jam/join/{code}"),
            json!({ "name": name }),
            &Auth::None,
        )
        .await;
    assert_eq!(res.status, StatusCode::CREATED, "{}", res.text);
    let token = res.body["token"].as_str().unwrap().to_string();
    assert!(token.starts_with("dwj_"));
    (Auth::Bearer(token), res.body["guest"].clone())
}

fn cookie(auth: &Auth) -> String {
    match auth {
        Auth::Cookie(c) => c.clone(),
        _ => unreachable!(),
    }
}

async fn send(ws: &mut Socket, msg: Value) {
    ws.send(Message::Text(msg.to_string().into()))
        .await
        .unwrap();
}

async fn next_of(ws: &mut Socket, kind: &str) -> Value {
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            match ws.next().await.expect("socket closed").unwrap() {
                Message::Text(t) => {
                    let v: Value = serde_json::from_str(t.as_str()).unwrap();
                    if v["type"] == kind {
                        return v;
                    }
                }
                Message::Close(f) => panic!("closed: {f:?}"),
                _ => continue,
            }
        }
    })
    .await
    .unwrap_or_else(|_| panic!("no {kind} in 5 s"))
}

/// Активное устройство хозяина: представилось и опубликовало снимок с треком `track`.
async fn host_device(addr: std::net::SocketAddr, host: &Auth, track: &Value) -> Socket {
    let mut req = format!("ws://{addr}/api/ws").into_client_request().unwrap();
    req.headers_mut()
        .insert(header::COOKIE, cookie(host).parse().unwrap());
    req.headers_mut()
        .insert(header::ORIGIN, PUBLIC_URL.parse().unwrap());
    let (mut ws, _) = connect_async(req).await.unwrap();
    next_of(&mut ws, "hello").await;
    send(
        &mut ws,
        json!({
            "type": "connect.hello", "deviceId": "web-host", "name": "Mac", "kind": "web",
            "platform": "web", "capabilities": { "volume": true },
        }),
    )
    .await;
    next_of(&mut ws, "connect.active").await;
    send(
        &mut ws,
        json!({ "type": "connect.state", "state": {
            "current": { "uid": "u1", "track": track, "source": "context" },
            "upcoming": [],
            "context": { "type": "playlist", "id": "p1", "title": "Секретный плейлист" },
            "playing": true, "positionMs": 1000, "durationMs": 200000, "volume": 0.5,
        }}),
    )
    .await;
    next_of(&mut ws, "connect.state").await;
    ws
}

async fn track(app: &TestApp, auth: &Auth, deezer_id: i64) -> Value {
    let id = app.resolve_id("track", deezer_id, auth).await;
    app.get(&format!("/api/tracks/{id}"), auth).await.body
}

#[sqlx::test]
async fn guest_joins_by_code_without_account(db: PgPool) {
    let app = TestApp::new(db).await;
    let host = app.listener("host").await;
    assert_eq!(app.get("/api/jam", &host).await.body["jam"], Value::Null);
    let jam = start(&app, &host).await;
    let code = jam["code"].as_str().unwrap();
    assert_eq!(code.len(), 16);
    assert_eq!(
        jam["url"].as_str().unwrap(),
        format!("{PUBLIC_URL}/jam/{code}")
    );
    // Повторный «Слушать вместе» — та же сессия
    let again = app.post("/api/jam", json!({}), &host).await;
    assert_eq!(again.status, StatusCode::OK);
    assert_eq!(again.body["code"], jam["code"]);

    let invite = app.get(&format!("/api/jam/join/{code}"), &Auth::None).await;
    assert_eq!(invite.status, StatusCode::OK, "{}", invite.text);
    assert_eq!(invite.body["guests"], 0);
    assert_eq!(invite.body["memberName"], Value::Null);

    // Без имени нельзя
    let res = app
        .post(
            &format!("/api/jam/join/{code}"),
            json!({ "name": "  " }),
            &Auth::None,
        )
        .await;
    assert_eq!(res.status, StatusCode::BAD_REQUEST);

    let (guest, me) = join(&app, code, "  Миша ").await;
    assert_eq!(me["name"], "Миша");
    assert_eq!(me["member"], false);
    let view = app.get("/api/jam/guest", &guest).await;
    assert_eq!(view.status, StatusCode::OK, "{}", view.text);
    assert_eq!(view.body["me"]["name"], "Миша");
    assert_eq!(view.body["settings"]["guestsCanAdd"], true);
    assert_eq!(view.body["now"], Value::Null);

    // Хозяин видит гостя
    let jam = app.get("/api/jam", &host).await.body["jam"].clone();
    assert_eq!(jam["guests"][0]["name"], "Миша");

    // Неизвестный код — как закрытая сессия
    let res = app.get("/api/jam/join/AAAAAAAAAAAAAAAA", &Auth::None).await;
    assert_eq!(res.status, StatusCode::NOT_FOUND);
    assert_eq!(res.code(), "jam_ended");
}

#[sqlx::test]
async fn member_joins_with_account_name(db: PgPool) {
    let app = TestApp::new(db).await;
    let host = app.listener("host").await;
    let friend = app.listener("friend").await;
    let code = start(&app, &host).await["code"]
        .as_str()
        .unwrap()
        .to_string();
    let invite = app.get(&format!("/api/jam/join/{code}"), &friend).await;
    assert!(invite.body["memberName"].is_string(), "{}", invite.text);
    let res = app
        .post(&format!("/api/jam/join/{code}"), json!({}), &friend)
        .await;
    assert_eq!(res.status, StatusCode::CREATED, "{}", res.text);
    assert_eq!(res.body["guest"]["member"], true);
    assert_eq!(res.body["guest"]["name"], invite.body["memberName"]);
}

#[sqlx::test]
async fn guest_token_opens_nothing_else(db: PgPool) {
    let app = TestApp::new(db).await;
    let host = app.listener("host").await;
    let code = start(&app, &host).await["code"]
        .as_str()
        .unwrap()
        .to_string();
    let (guest, _) = join(&app, &code, "Миша").await;
    let track_id = app.resolve_id("track", LIGHT_ME_UP, &host).await;
    for path in [
        "/api/auth/me".to_string(),
        "/api/jam".to_string(),
        "/api/devices".to_string(),
        "/api/home".to_string(),
        "/api/library/liked".to_string(),
        "/api/playlists".to_string(),
        "/api/search?q=lowborn".to_string(),
        "/api/sync".to_string(),
        "/api/admin/users".to_string(),
        "/api/history".to_string(),
        format!("/api/tracks/{track_id}"),
        format!("/api/tracks/{track_id}/stream"),
    ] {
        let res = app.get(&path, &guest).await;
        assert!(
            res.status == StatusCode::UNAUTHORIZED || res.status == StatusCode::NOT_FOUND,
            "{path}: {} {}",
            res.status,
            res.text
        );
        assert_ne!(res.status, StatusCode::OK, "{path}");
    }
    // Изменить сессию хозяина гость не может
    let res = app
        .call(
            Method::PATCH,
            "/api/jam",
            Some(json!({ "guestsCanAdd": false })),
            &guest,
        )
        .await;
    assert_eq!(res.status, StatusCode::UNAUTHORIZED);
    assert_eq!(
        app.delete("/api/jam", &guest).await.status,
        StatusCode::UNAUTHORIZED
    );
    // Общий WebSocket с токеном гостя не открывается
    let addr = app.serve().await;
    let Auth::Bearer(token) = &guest else {
        unreachable!()
    };
    let err = connect_async(format!("ws://{addr}/api/ws?token={token}"))
        .await
        .unwrap_err();
    assert!(err.to_string().contains("401"), "{err}");
    // Слушать у себя — только текущий или следующий трек хозяина
    let res = app
        .get(&format!("/api/jam/guest/stream/{track_id}"), &guest)
        .await;
    assert_eq!(res.status, StatusCode::FORBIDDEN, "{}", res.text);
    // Чужой или выдуманный токен
    let res = app
        .get("/api/jam/guest", &Auth::Bearer("dwj_nope".into()))
        .await;
    assert_eq!(res.status, StatusCode::UNAUTHORIZED);
    let res = app.get("/api/jam/guest", &host).await;
    assert_eq!(res.status, StatusCode::UNAUTHORIZED);
}

#[sqlx::test]
async fn ended_expired_and_removed_are_rejected(db: PgPool) {
    let app = TestApp::new(db).await;
    let host = app.listener("host").await;
    let jam = start(&app, &host).await;
    let code = jam["code"].as_str().unwrap().to_string();
    let (misha, me) = join(&app, &code, "Миша").await;
    let (olya, _) = join(&app, &code, "Оля").await;

    // Убранный гость
    let res = app
        .delete(
            &format!("/api/jam/guests/{}", me["id"].as_str().unwrap()),
            &host,
        )
        .await;
    assert_eq!(res.status, StatusCode::NO_CONTENT);
    assert_eq!(
        app.get("/api/jam/guest", &misha).await.code(),
        "jam_removed"
    );
    assert_eq!(
        app.get("/api/jam/guest", &olya).await.status,
        StatusCode::OK
    );

    // Простоявшая 6 часов
    sqlx::query("UPDATE jam_sessions SET last_activity_at = now() - interval '7 hours'")
        .execute(&app.state.db)
        .await
        .unwrap();
    let res = app.get("/api/jam/guest", &olya).await;
    assert_eq!(res.status, StatusCode::NOT_FOUND);
    assert_eq!(res.code(), "jam_ended");
    let res = app
        .post(
            &format!("/api/jam/join/{code}"),
            json!({ "name": "Петя" }),
            &Auth::None,
        )
        .await;
    assert_eq!(res.code(), "jam_ended");
    assert_eq!(app.get("/api/jam", &host).await.body["jam"], Value::Null);

    // Новая сессия — новый код; законченная хозяином не пускает
    let jam = start(&app, &host).await;
    assert_ne!(jam["code"].as_str().unwrap(), code);
    let code = jam["code"].as_str().unwrap().to_string();
    let (petya, _) = join(&app, &code, "Петя").await;
    assert_eq!(
        app.get("/api/jam/guest", &petya).await.status,
        StatusCode::OK
    );
    assert_eq!(
        app.delete("/api/jam", &host).await.status,
        StatusCode::NO_CONTENT
    );
    assert_eq!(app.get("/api/jam/guest", &petya).await.code(), "jam_ended");
    let res = app
        .post("/api/jam/guest/react", json!({ "kind": "heart" }), &petya)
        .await;
    assert_eq!(res.code(), "jam_ended");
    // Старые гости в новой сессии не действуют
    assert_eq!(app.get("/api/jam/guest", &olya).await.code(), "jam_ended");
}

#[sqlx::test]
async fn rate_limits(db: PgPool) {
    let app = TestApp::new(db).await;
    let host = app.listener("host").await;
    let code = start(&app, &host).await["code"]
        .as_str()
        .unwrap()
        .to_string();
    let (guest, _) = join(&app, &code, "Миша").await;
    for _ in 0..30 {
        let res = app
            .post("/api/jam/guest/react", json!({ "kind": "heart" }), &guest)
            .await;
        assert_eq!(res.status, StatusCode::NO_CONTENT, "{}", res.text);
    }
    let res = app
        .post("/api/jam/guest/react", json!({ "kind": "heart" }), &guest)
        .await;
    assert_eq!(res.status, StatusCode::TOO_MANY_REQUESTS);
    // Вход: 10 за 10 минут с одного адреса (один уже был)
    for i in 0..9 {
        join(&app, &code, &format!("Гость {i}")).await;
    }
    let res = app
        .post(
            &format!("/api/jam/join/{code}"),
            json!({ "name": "Лишний" }),
            &Auth::None,
        )
        .await;
    assert_eq!(res.status, StatusCode::TOO_MANY_REQUESTS);
    // Поиск — 30 в минуту на гостя
    for _ in 0..30 {
        let res = app.get("/api/jam/guest/search?q=lowborn", &guest).await;
        assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    }
    let res = app.get("/api/jam/guest/search?q=lowborn", &guest).await;
    assert_eq!(res.status, StatusCode::TOO_MANY_REQUESTS);
}

#[sqlx::test]
async fn guest_adds_track_to_host_queue(db: PgPool) {
    let app = TestApp::new(db).await;
    let host = app.listener("host").await;
    let code = start(&app, &host).await["code"]
        .as_str()
        .unwrap()
        .to_string();
    let (guest, _) = join(&app, &code, "Миша").await;

    // Поиск гостя: треки без отметок медиатеки хозяина
    let found = app.get("/api/jam/guest/search?q=lowborn", &guest).await;
    assert_eq!(found.status, StatusCode::OK, "{}", found.text);
    let first = &found.body["items"][0];
    assert!(first["title"].is_string());
    assert!(first.get("liked").is_none());

    let add_id = app.resolve_id("track", CHURCH_ON_SUNDAY, &host).await;
    // Ничего не играет — добавить некуда
    let res = app
        .post("/api/jam/guest/queue", json!({ "trackId": add_id }), &guest)
        .await;
    assert_eq!(res.status, StatusCode::CONFLICT);
    assert_eq!(res.code(), "nothing_playing");

    let addr = app.serve().await;
    let playing = track(&app, &host, LIGHT_ME_UP).await;
    let mut device = host_device(addr, &host, &playing).await;

    // Гость видит, что играет, — без контекста хозяина
    let Auth::Bearer(token) = &guest else {
        unreachable!()
    };
    let (mut guest_ws, _) = connect_async(format!("ws://{addr}/api/jam/guest/ws?token={token}"))
        .await
        .unwrap();
    let now = next_of(&mut guest_ws, "jam.now").await;
    assert_eq!(now["now"]["current"]["track"]["id"], playing["id"]);
    assert_eq!(now["now"]["playing"], true);
    assert!(!now.to_string().contains("Секретный плейлист"));

    // Сразу в очередь активного устройства
    let res = app
        .post("/api/jam/guest/queue", json!({ "trackId": add_id }), &guest)
        .await;
    assert_eq!(res.status, StatusCode::CREATED, "{}", res.text);
    assert_eq!(res.body["status"], "added");
    assert_eq!(res.body["guestName"], "Миша");
    let cmd = next_of(&mut device, "connect.command").await;
    assert_eq!(cmd["command"]["action"], "enqueue");
    assert_eq!(cmd["command"]["track"]["id"], add_id.as_str());
    // Хозяину — событие сессии
    next_of(&mut device, "jam.changed").await;
    // Повтор того же трека
    let res = app
        .post("/api/jam/guest/queue", json!({ "trackId": add_id }), &guest)
        .await;
    assert_eq!(res.code(), "already_queued");

    // Устройство поставило трек в очередь: гость видит подпись «добавил Миша»
    send(
        &mut device,
        json!({ "type": "connect.state", "state": {
            "current": { "uid": "u1", "track": playing, "source": "context" },
            "manual": [{ "uid": "u2", "track": track(&app, &host, CHURCH_ON_SUNDAY).await, "source": "manual" }],
            "playing": true, "positionMs": 5000, "durationMs": 200000,
        }}),
    )
    .await;
    let now = loop {
        let v = next_of(&mut guest_ws, "jam.now").await;
        if !v["now"]["queue"].as_array().unwrap().is_empty() {
            break v;
        }
    };
    assert_eq!(now["now"]["queue"][0]["addedBy"], "Миша");

    // Хозяин включил одобрение: трек ждёт
    let res = app
        .call(
            Method::PATCH,
            "/api/jam",
            Some(json!({ "approval": true })),
            &host,
        )
        .await;
    assert_eq!(res.status, StatusCode::OK);
    let other = app.resolve_id("track", LIGHT_ME_UP, &host).await;
    let res = app
        .post("/api/jam/guest/queue", json!({ "trackId": other }), &guest)
        .await;
    assert_eq!(res.body["status"], "pending", "{}", res.text);
    let request_id = res.body["id"].as_str().unwrap().to_string();
    let jam = app.get("/api/jam", &host).await.body["jam"].clone();
    assert!(
        jam["requests"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["id"] == request_id.as_str() && r["status"] == "pending")
    );
    let res = app
        .post(
            &format!("/api/jam/requests/{request_id}/approve"),
            json!({}),
            &host,
        )
        .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    let cmd = next_of(&mut device, "connect.command").await;
    assert_eq!(cmd["command"]["track"]["id"], other.as_str());

    // Хозяин выключил добавление
    app.call(
        Method::PATCH,
        "/api/jam",
        Some(json!({ "guestsCanAdd": false })),
        &host,
    )
    .await;
    let res = app
        .post("/api/jam/guest/queue", json!({ "trackId": add_id }), &guest)
        .await;
    assert_eq!(res.status, StatusCode::FORBIDDEN);

    // ❤ видят все
    let res = app
        .post("/api/jam/guest/react", json!({ "kind": "heart" }), &guest)
        .await;
    assert_eq!(res.status, StatusCode::NO_CONTENT);
    let r = next_of(&mut device, "jam.reaction").await;
    assert_eq!(r["guestName"], "Миша");
    assert_eq!(r["trackId"], playing["id"]);

    // Голос за пропуск: единственный гость на связи — сразу
    let res = app.post("/api/jam/guest/skip", json!({}), &guest).await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    assert_eq!(res.body["votes"], 1);
    let cmd = next_of(&mut device, "connect.command").await;
    assert_eq!(cmd["command"]["action"], "next");

    // Конец сессии закрывает соединение гостя
    app.delete("/api/jam", &host).await;
    loop {
        if next_of(&mut guest_ws, "jam.changed").await["ended"] == true {
            break;
        }
    }
}

#[sqlx::test]
async fn listen_along_only_current_track(db: PgPool) {
    let app = TestApp::new(db).await;
    let host = app.listener("host").await;
    let code = start(&app, &host).await["code"]
        .as_str()
        .unwrap()
        .to_string();
    let (guest, _) = join(&app, &code, "Миша").await;
    let addr = app.serve().await;
    let playing = track(&app, &host, LIGHT_ME_UP).await;
    let _device = host_device(addr, &host, &playing).await;
    let id = playing["id"].as_str().unwrap();
    // Файла ещё нет — честный ответ, без скачивания по просьбе гостя
    let res = app
        .get(&format!("/api/jam/guest/stream/{id}"), &guest)
        .await;
    assert_eq!(res.status, StatusCode::CONFLICT, "{}", res.text);
    assert_eq!(res.code(), "audio_unavailable");
    let jobs: i64 = sqlx::query_scalar("SELECT count(*) FROM jobs WHERE kind = 'acquire'")
        .fetch_one(&app.state.db)
        .await
        .unwrap();
    assert_eq!(jobs, 0);
    // Хозяин выключил «Слушать у себя»
    app.call(
        Method::PATCH,
        "/api/jam",
        Some(json!({ "guestsCanListen": false })),
        &host,
    )
    .await;
    let res = app
        .get(&format!("/api/jam/guest/stream/{id}"), &guest)
        .await;
    assert_eq!(res.status, StatusCode::FORBIDDEN);
}
