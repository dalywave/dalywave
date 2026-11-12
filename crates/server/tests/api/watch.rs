//! Apple Watch: сессию часам выдаёт вошедший iPhone (`POST /api/auth/app/pair`), часы — пульт
//! в «Где играть»: видят, что играет, и шлют команды, но активными не становятся.

use std::time::Duration;

use axum::http::{Method, StatusCode};
use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use sqlx::PgPool;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async, tungstenite::Message};

use crate::common::{Auth, TestApp};

type Socket = WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>;

/// Вошедшее приложение iPhone человека `auth`: токен устройства `dwd_…`.
async fn iphone(app: &TestApp, auth: &Auth) -> String {
    let user: uuid::Uuid = app.user_id(auth).await.parse().unwrap();
    let device: uuid::Uuid = sqlx::query_scalar(
        "INSERT INTO devices (user_id, name, platform) VALUES ($1, 'iPhone Захара', 'ios') RETURNING id",
    )
    .bind(user)
    .fetch_one(&app.state.db)
    .await
    .unwrap();
    let mut conn = app.state.db.acquire().await.unwrap();
    let (_, token) = dalywave_server::session::create(
        &mut conn,
        dalywave_server::session::NewSession {
            user_id: user,
            device_id: Some(device),
            method: dalywave_server::session::Method::Oidc,
            user_agent: None,
            ip: std::net::Ipv4Addr::LOCALHOST.into(),
        },
    )
    .await
    .unwrap();
    token
}

async fn pair(app: &TestApp, phone: &Auth) -> String {
    let res = app
        .post(
            "/api/auth/app/pair",
            json!({ "platform": "watchos", "appVersion": "0.1.2" }),
            phone,
        )
        .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    res.body["token"].as_str().unwrap().to_string()
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn iphone_hands_the_watch_its_own_session(db: PgPool) {
    let app = TestApp::new(db).await;
    let me = app.listener("ann").await;
    let phone = Auth::Bearer(iphone(&app, &me).await);

    let res = app
        .post(
            "/api/auth/app/pair",
            json!({ "platform": "watchos" }),
            &phone,
        )
        .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    let token = res.body["token"].as_str().unwrap().to_string();
    assert!(token.starts_with("dwd_"));
    assert_eq!(res.body["device"]["name"], "Apple Watch");
    assert_eq!(res.body["device"]["platform"], "watchos");
    assert_eq!(res.body["user"]["email"], "ann@example.com");

    // Часы — отдельное устройство со своей сессией и правами человека
    let watch = Auth::Bearer(token.clone());
    let me_watch = app.get("/api/auth/me", &watch).await;
    assert_eq!(me_watch.status, StatusCode::OK, "{}", me_watch.text);
    assert_eq!(me_watch.body["device"]["platform"], "watchos");
    assert_eq!(me_watch.body["roles"], json!(["listener"]));
    assert_eq!(
        app.get("/api/library/tracks", &watch).await.status,
        StatusCode::OK
    );
    assert!(app.audit_kinds().await.contains(&"auth.sign_in".into()));

    // Часы сами сессий не выдают; сайт — тоже (cookie могла подставить чужая страница)
    let res = app
        .post(
            "/api/auth/app/pair",
            json!({ "platform": "watchos" }),
            &watch,
        )
        .await;
    assert_eq!(res.status, StatusCode::FORBIDDEN, "{}", res.text);
    let res = app
        .post("/api/auth/app/pair", json!({ "platform": "watchos" }), &me)
        .await;
    assert_eq!(res.status, StatusCode::FORBIDDEN, "{}", res.text);
    // Только сопряжённые устройства: телефону нужен свой вход через daly.rest
    let res = app
        .post("/api/auth/app/pair", json!({ "platform": "ios" }), &phone)
        .await;
    assert_eq!(res.status, StatusCode::BAD_REQUEST, "{}", res.text);
    assert_eq!(
        app.post(
            "/api/auth/app/pair",
            json!({ "platform": "watchos" }),
            &Auth::None
        )
        .await
        .status,
        StatusCode::UNAUTHORIZED
    );

    // Повторная выдача заменяет прежние часы этого iPhone: в «Доступе» не копятся
    let again = Auth::Bearer(pair(&app, &phone).await);
    assert_eq!(
        app.get("/api/auth/me", &watch).await.status,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(app.get("/api/auth/me", &again).await.status, StatusCode::OK);
    let live: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM devices WHERE platform = 'watchos' AND revoked_at IS NULL",
    )
    .fetch_one(&app.state.db)
    .await
    .unwrap();
    assert_eq!(live, 1);

    // Выход на iPhone отзывает и часы
    let res = app
        .call(Method::POST, "/api/auth/logout", None, &phone)
        .await;
    assert_eq!(res.status, StatusCode::NO_CONTENT);
    assert_eq!(
        app.get("/api/auth/me", &again).await.status,
        StatusCode::UNAUTHORIZED
    );
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn revoking_the_iphone_in_access_revokes_its_watch(db: PgPool) {
    let app = TestApp::new(db).await;
    let (_, admin) = app.admin("boss").await;
    let phone = Auth::Bearer(iphone(&app, &admin).await);
    let watch = Auth::Bearer(pair(&app, &phone).await);

    let list = app.get("/api/admin/sessions", &admin).await;
    let sessions = list.body["items"].as_array().unwrap();
    let watch_row = sessions
        .iter()
        .find(|s| s["device"]["platform"] == "watchos")
        .expect("часы в «Доступе»");
    assert_eq!(watch_row["device"]["name"], "Apple Watch");
    let phone_id = sessions
        .iter()
        .find(|s| s["device"]["platform"] == "ios")
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();

    let res = app
        .delete(&format!("/api/admin/sessions/{phone_id}"), &admin)
        .await;
    assert_eq!(res.status, StatusCode::NO_CONTENT);
    assert_eq!(
        app.get("/api/auth/me", &watch).await.status,
        StatusCode::UNAUTHORIZED
    );
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

async fn connect(addr: std::net::SocketAddr, token: &str, hello: Value) -> Socket {
    let (mut ws, _) = connect_async(format!("ws://{addr}/api/ws?token={token}"))
        .await
        .unwrap();
    next_of(&mut ws, "hello").await;
    send(&mut ws, hello).await;
    next_of(&mut ws, "connect.active").await;
    ws
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn watch_is_a_remote_in_where_to_play(db: PgPool) {
    let app = TestApp::new(db).await;
    let me = app.listener("ann").await;
    let phone_token = iphone(&app, &me).await;
    let watch_token = pair(&app, &Auth::Bearer(phone_token.clone())).await;
    let addr = app.serve().await;

    let mut phone = connect(
        addr,
        &phone_token,
        json!({ "type": "connect.hello", "deviceId": "app-phone", "name": "", "kind": "phone",
                "platform": "ios", "capabilities": { "volume": true, "dj": true } }),
    )
    .await;
    send(
        &mut phone,
        json!({ "type": "connect.state", "state": { "playing": true, "positionMs": 1000, "durationMs": 200000 } }),
    )
    .await;
    next_of(&mut phone, "connect.state").await;

    let mut watch = connect(
        addr,
        &watch_token,
        json!({ "type": "connect.hello", "deviceId": "watch-1", "name": "", "kind": "watch",
                "platform": "watchos", "capabilities": { "volume": false, "remote": true } }),
    )
    .await;
    let list = next_of(&mut watch, "connect.devices").await;
    let row = list["devices"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["id"] == "watch-1")
        .unwrap()
        .clone();
    assert_eq!(row["kind"], "watch");
    assert_eq!(row["platform"], "watchos");
    assert_eq!(row["name"], "Apple Watch", "имя — из сопряжения");
    assert_eq!(row["capabilities"]["remote"], true);
    assert_eq!(list["activeDeviceId"], "app-phone");

    // Команда с часов доходит до iPhone, подтверждение — обратно
    send(
        &mut watch,
        json!({ "type": "connect.command", "id": "w1", "to": "app-phone", "command": { "action": "pause" } }),
    )
    .await;
    let cmd = next_of(&mut phone, "connect.command").await;
    assert_eq!(
        (cmd["from"].as_str(), cmd["command"]["action"].as_str()),
        (Some("watch-1"), Some("pause"))
    );
    send(
        &mut phone,
        json!({ "type": "connect.ack", "id": "w1", "to": "watch-1", "ok": true }),
    )
    .await;
    assert_eq!(next_of(&mut watch, "connect.ack").await["ok"], true);

    // Звук на часах не играет: перенос туда — отказ, а claim и снимки часов не меняют активное
    send(
        &mut phone,
        json!({ "type": "connect.transfer", "id": "t1", "to": "watch-1" }),
    )
    .await;
    let ack = next_of(&mut phone, "connect.ack").await;
    assert_eq!(
        (ack["ok"].as_bool(), ack["error"].as_str()),
        (Some(false), Some("unsupported"))
    );
    send(&mut watch, json!({ "type": "connect.claim" })).await;
    send(
        &mut watch,
        json!({ "type": "connect.state", "state": { "playing": false } }),
    )
    .await;
    tokio::time::sleep(Duration::from_millis(300)).await;
    let res = app.get("/api/devices", &me).await;
    assert_eq!(res.body["activeDeviceId"], "app-phone");
    assert_eq!(res.body["state"]["playing"], true);
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn watch_controls_over_http_without_a_websocket(db: PgPool) {
    let app = TestApp::new(db).await;
    let me = app.listener("ann").await;
    let phone_token = iphone(&app, &me).await;
    let watch = Auth::Bearer(pair(&app, &Auth::Bearer(phone_token.clone())).await);
    let addr = app.serve().await;

    let mut phone = connect(
        addr,
        &phone_token,
        json!({ "type": "connect.hello", "deviceId": "app-phone", "name": "", "kind": "phone",
                "platform": "ios", "capabilities": { "volume": true } }),
    )
    .await;
    let queue = json!([{ "uid": "u2", "source": "context", "track": {
        "id": "0193a8a0-0000-7000-8000-000000000002", "deezerId": null, "title": "Второй", "version": "",
        "durationMs": 200000, "explicit": false, "discNumber": 1, "trackPosition": 1, "artists": [],
        "album": null, "recordingId": null, "isrc": null, "audioStatus": "ready", "liked": false } }]);
    send(
        &mut phone,
        json!({ "type": "connect.state", "state": {
            "current": queue[0], "upcoming": queue, "peaks": [0.5, 0.7],
            "playing": true, "positionMs": 1000, "durationMs": 200000 } }),
    )
    .await;
    next_of(&mut phone, "connect.state").await;

    // Часы отмечаются: они в списке, в сети, а снимок — без очереди
    let hello = json!({ "deviceId": "watch-1", "name": "", "kind": "watch", "platform": "watchos",
                        "capabilities": { "remote": true } });
    let res = app.post("/api/devices/remote", hello, &watch).await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    assert_eq!(res.body["activeDeviceId"], "app-phone");
    assert_eq!(res.body["state"]["current"]["track"]["title"], "Второй");
    assert_eq!(res.body["state"]["upcoming"], json!([]));
    assert_eq!(res.body["state"]["peaks"], json!([]));
    let row = res.body["devices"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["id"] == "watch-1")
        .unwrap()
        .clone();
    assert_eq!(
        (row["kind"].as_str(), row["online"].as_bool()),
        (Some("watch"), Some(true))
    );
    // Остальные устройства узнают, что часы появились
    let list = next_of(&mut phone, "connect.devices").await;
    assert!(
        list["devices"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["id"] == "watch-1")
    );

    // Команда запросом: устройство получает её от часов, подтверждение приходит в ответе
    let command = json!({ "id": "c1", "to": "app-phone", "command": { "action": "next" } });
    let (res, ()) = tokio::join!(
        app.post("/api/devices/remote/command", command, &watch),
        async {
            let cmd = next_of(&mut phone, "connect.command").await;
            assert_eq!(cmd["from"], "watch-1");
            assert_eq!(cmd["command"]["action"], "next");
            send(
                &mut phone,
                json!({ "type": "connect.ack", "id": "c1", "to": "watch-1", "ok": true }),
            )
            .await;
        }
    );
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    assert_eq!(
        (res.body["ok"].as_bool(), res.body["from"].as_str()),
        (Some(true), Some("app-phone"))
    );

    // Не в сети — отказ сразу; на часы звук не переносится
    let res = app
        .post(
            "/api/devices/remote/command",
            json!({ "id": "c2", "to": "tv", "command": { "action": "pause" } }),
            &watch,
        )
        .await;
    assert_eq!(res.body["error"], "offline");
    let res = app
        .post(
            "/api/devices/remote/transfer",
            json!({ "to": "watch-1" }),
            &watch,
        )
        .await;
    assert_eq!(res.body["error"], "unsupported");

    // Перенос запросом — как по WebSocket
    let mut mac = connect(
        addr,
        &phone_token,
        json!({ "type": "connect.hello", "deviceId": "mac", "name": "Mac", "kind": "laptop",
                "platform": "macos", "capabilities": { "volume": true } }),
    )
    .await;
    let res = app
        .post(
            "/api/devices/remote/transfer",
            json!({ "id": "t1", "to": "mac", "play": true }),
            &watch,
        )
        .await;
    assert_eq!(res.body["ok"], true, "{}", res.text);
    let moved = next_of(&mut mac, "connect.active").await;
    assert_eq!(moved["deviceId"], "mac");
    assert_eq!(moved["transfer"], true);
    assert_eq!(
        moved["state"]["upcoming"][0]["uid"], "u2",
        "очередь — целиком"
    );
}
