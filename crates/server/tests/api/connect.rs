//! Управление между устройствами: присутствие, снимки активного устройства, команды с подтверждением,
//! перенос воспроизведения и пропажа активного устройства.

use std::time::Duration;

use axum::http::{StatusCode, header};
use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use sqlx::PgPool;
use tokio_tungstenite::{
    MaybeTlsStream, WebSocketStream, connect_async,
    tungstenite::{Message, client::IntoClientRequest},
};

use crate::common::{Auth, PUBLIC_URL, TestApp};

type Socket = WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>;

fn cookie(auth: &Auth) -> String {
    match auth {
        Auth::Cookie(c) => c.clone(),
        _ => unreachable!(),
    }
}

/// Устройство: соединение, представившееся `connect.hello`. Ответ на hello — `connect.active`
/// (кто играет и что) — приходит этому соединению первым, раньше списка устройств.
async fn device(addr: std::net::SocketAddr, auth: &Auth, id: &str, kind: &str) -> Socket {
    device_with(addr, auth, id, kind).await.0
}

async fn device_with(
    addr: std::net::SocketAddr,
    auth: &Auth,
    id: &str,
    kind: &str,
) -> (Socket, Value) {
    let mut req = format!("ws://{addr}/api/ws").into_client_request().unwrap();
    req.headers_mut()
        .insert(header::COOKIE, cookie(auth).parse().unwrap());
    req.headers_mut()
        .insert(header::ORIGIN, PUBLIC_URL.parse().unwrap());
    let (mut ws, _) = connect_async(req).await.unwrap();
    assert_eq!(next_of(&mut ws, "hello").await["type"], "hello");
    send(
        &mut ws,
        json!({
            "type": "connect.hello", "deviceId": id, "name": format!("Устройство {id}"),
            "kind": kind, "platform": "web", "capabilities": { "volume": true },
        }),
    )
    .await;
    let active = next_any(&mut ws).await;
    assert_eq!(active["type"], "connect.active");
    (ws, active)
}

async fn send(ws: &mut Socket, msg: Value) {
    ws.send(Message::Text(msg.to_string().into()))
        .await
        .unwrap();
}

/// Следующее сообщение этого типа; остальные пропускаются.
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

/// Ничего этого типа не приходит (пока не ответит ping).
async fn none_of(ws: &mut Socket, kind: &str) {
    send(ws, json!({ "type": "ping", "id": "probe" })).await;
    loop {
        let v = next_any(ws).await;
        assert_ne!(v["type"], kind, "unexpected {v}");
        if v["type"] == "pong" {
            return;
        }
    }
}

async fn next_any(ws: &mut Socket) -> Value {
    loop {
        let msg = tokio::time::timeout(Duration::from_secs(5), ws.next())
            .await
            .expect("no message in 5 s")
            .expect("socket closed")
            .unwrap();
        if let Message::Text(t) = msg {
            return serde_json::from_str(t.as_str()).unwrap();
        }
    }
}

fn track(title: &str) -> Value {
    json!({
        "id": "0193a8a0-0000-7000-8000-000000000001", "deezerId": null, "title": title, "version": "",
        "durationMs": 200000, "explicit": false, "discNumber": 1, "trackPosition": 1,
        "artists": [], "album": null, "recordingId": null, "isrc": null, "audioStatus": "ready", "liked": false,
    })
}

fn state(position: u32, playing: bool) -> Value {
    json!({
        "current": { "uid": "u1", "track": track("Первый"), "source": "context" },
        "upcoming": [{ "uid": "u2", "track": track("Второй"), "source": "context" }],
        "context": { "type": "album", "id": "a1", "title": "Альбом" },
        "playing": playing, "positionMs": position, "durationMs": 200000,
        "volume": 0.4, "transition": "smart",
    })
}

fn ids(devices: &Value) -> Vec<(String, bool)> {
    devices["devices"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| {
            (
                d["id"].as_str().unwrap().to_string(),
                d["online"].as_bool().unwrap(),
            )
        })
        .collect()
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn presence_and_state_relay(db: PgPool) {
    let app = TestApp::new(db).await;
    let me = app.listener("ann").await;
    let addr = app.serve().await;

    let (mut a, active) = device_with(addr, &me, "tab-a", "web").await;
    let list = next_of(&mut a, "connect.devices").await;
    assert_eq!(ids(&list), [("tab-a".to_string(), true)]);
    // Ничего ещё не играло: активного нет
    assert_eq!(active["deviceId"], Value::Null);
    assert_eq!(active["state"], Value::Null);

    let mut b = device(addr, &me, "mac", "laptop").await;
    for ws in [&mut a, &mut b] {
        let list = next_of(ws, "connect.devices").await;
        assert_eq!(list["devices"].as_array().unwrap().len(), 2);
    }

    // Звук заиграл на A: A — активное, пульт B видит снимок
    send(&mut a, json!({ "type": "connect.claim" })).await;
    for ws in [&mut a, &mut b] {
        let active = next_of(ws, "connect.active").await;
        assert_eq!(active["deviceId"], "tab-a");
        assert_eq!(active["transfer"], false);
    }
    send(
        &mut a,
        json!({ "type": "connect.state", "state": state(1000, true) }),
    )
    .await;
    let relayed = next_of(&mut b, "connect.state").await;
    assert_eq!(relayed["deviceId"], "tab-a");
    assert_eq!(relayed["state"]["current"]["track"]["title"], "Первый");
    assert_eq!(relayed["state"]["positionMs"], 1000);
    assert!(
        relayed["state"]["at"].is_string(),
        "время снимка ставит сервер"
    );

    // Свой снимок приходит и отправителю (клиент узнаёт его по deviceId)
    assert_eq!(next_of(&mut a, "connect.state").await["deviceId"], "tab-a");
    // Снимок не от активного устройства не принимается: отправитель узнаёт, кто активен
    send(
        &mut b,
        json!({ "type": "connect.state", "state": state(9, false) }),
    )
    .await;
    let refused = next_of(&mut b, "connect.active").await;
    assert_eq!(refused["deviceId"], "tab-a");
    none_of(&mut a, "connect.state").await;

    let res = app.get("/api/devices", &me).await;
    assert_eq!(res.status, StatusCode::OK);
    assert_eq!(res.body["activeDeviceId"], "tab-a");
    assert_eq!(res.body["devices"][0]["id"], "tab-a", "активное — первым");
    assert_eq!(res.body["devices"][0]["active"], true);
    assert_eq!(res.body["devices"][0]["capabilities"]["volume"], true);
    assert_eq!(res.body["state"]["current"]["uid"], "u1");
    assert!(res.body["state"]["positionMs"].as_u64().unwrap() >= 1000);

    // Новое устройство сразу видит, что играет, с позицией «на сейчас»
    tokio::time::sleep(Duration::from_millis(200)).await;
    let (c, active) = device_with(addr, &me, "phone", "phone").await;
    assert_eq!(active["deviceId"], "tab-a");
    assert_eq!(active["state"]["upcoming"][0]["uid"], "u2");
    assert!(active["state"]["positionMs"].as_u64().unwrap() >= 1200);

    // Чужой человек этих устройств не видит
    let other = app.listener("bob").await;
    let res = app.get("/api/devices", &other).await;
    assert_eq!(res.body["devices"], json!([]));
    assert_eq!(res.body["state"], Value::Null);

    // Соединение закрылось — устройство не в сети, но остаётся активным: пульты предложат продолжить
    a.close(None).await.unwrap();
    let list = loop {
        let list = next_of(&mut b, "connect.devices").await;
        if ids(&list)
            .iter()
            .any(|(id, online)| id == "tab-a" && !online)
        {
            break list;
        }
    };
    assert_eq!(list["activeDeviceId"], "tab-a");
    assert_eq!(list["devices"][0]["id"], "tab-a");
    drop(c);
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn commands_reach_their_device_and_come_back_acknowledged(db: PgPool) {
    let app = TestApp::new(db).await;
    let me = app.listener("ann").await;
    let other = app.listener("bob").await;
    let addr = app.serve().await;
    let mut a = device(addr, &me, "mac", "laptop").await;
    let mut b = device(addr, &me, "phone", "phone").await;
    let mut stranger = device(addr, &other, "mac", "laptop").await;

    send(
        &mut b,
        json!({ "type": "connect.command", "id": "c1", "to": "mac", "command": { "action": "seek", "positionMs": 42000 } }),
    )
    .await;
    let cmd = next_of(&mut a, "connect.command").await;
    assert_eq!(
        cmd,
        json!({ "type": "connect.command", "id": "c1", "from": "phone", "command": { "action": "seek", "positionMs": 42000 } })
    );
    // Тот же id устройства у другого человека команду не получает
    none_of(&mut stranger, "connect.command").await;

    send(
        &mut a,
        json!({ "type": "connect.ack", "id": "c1", "to": "phone", "ok": true }),
    )
    .await;
    let ack = next_of(&mut b, "connect.ack").await;
    assert_eq!(ack["id"], "c1");
    assert_eq!(ack["from"], "mac");
    assert_eq!(ack["ok"], true);

    // Устройства нет в сети — ответ сразу от сервера
    send(
        &mut b,
        json!({ "type": "connect.command", "id": "c2", "to": "ghost", "command": { "action": "next" } }),
    )
    .await;
    let ack = next_of(&mut b, "connect.ack").await;
    assert_eq!(ack["id"], "c2");
    assert_eq!(ack["ok"], false);
    assert_eq!(ack["error"], "offline");

    // Чужому устройству команда не уходит: у этого человека такого нет
    send(
        &mut stranger,
        json!({ "type": "connect.command", "id": "c3", "to": "phone", "command": { "action": "pause" } }),
    )
    .await;
    assert_eq!(
        next_of(&mut stranger, "connect.ack").await["error"],
        "offline"
    );
    none_of(&mut b, "connect.command").await;

    // Незнакомая команда (клиент новее) не рвёт соединение
    send(
        &mut b,
        json!({ "type": "connect.command", "id": "c4", "to": "mac", "command": { "action": "dance" } }),
    )
    .await;
    none_of(&mut a, "connect.command").await;

    // Без connect.hello команды не принимаются
    let mut req = format!("ws://{addr}/api/ws").into_client_request().unwrap();
    req.headers_mut()
        .insert(header::COOKIE, cookie(&me).parse().unwrap());
    req.headers_mut()
        .insert(header::ORIGIN, PUBLIC_URL.parse().unwrap());
    let (mut anon, _) = connect_async(req).await.unwrap();
    next_of(&mut anon, "hello").await;
    send(
        &mut anon,
        json!({ "type": "connect.command", "id": "c5", "to": "mac", "command": { "action": "pause" } }),
    )
    .await;
    none_of(&mut anon, "connect.ack").await;
    none_of(&mut a, "connect.command").await;
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn shuffle_and_repeat_travel_as_commands_and_in_the_snapshot(db: PgPool) {
    let app = TestApp::new(db).await;
    let me = app.listener("ann").await;
    let addr = app.serve().await;
    let mut a = device(addr, &me, "mac", "laptop").await;
    let mut b = device(addr, &me, "phone", "phone").await;

    for (id, command) in [
        ("s1", json!({ "action": "setShuffle", "shuffle": true })),
        ("r1", json!({ "action": "setRepeat", "mode": "one" })),
    ] {
        send(
            &mut b,
            json!({ "type": "connect.command", "id": id, "to": "mac", "command": command }),
        )
        .await;
        let cmd = next_of(&mut a, "connect.command").await;
        assert_eq!(cmd["command"], command);
    }
    // Неизвестный режим повтора не доходит
    send(
        &mut b,
        json!({ "type": "connect.command", "id": "r2", "to": "mac", "command": { "action": "setRepeat", "mode": "twice" } }),
    )
    .await;
    none_of(&mut a, "connect.command").await;

    send(&mut a, json!({ "type": "connect.claim" })).await;
    next_of(&mut b, "connect.active").await;
    let mut st = state(1000, true);
    st["shuffle"] = json!(true);
    st["repeat"] = json!("all");
    send(&mut a, json!({ "type": "connect.state", "state": st })).await;
    let relayed = next_of(&mut b, "connect.state").await;
    assert_eq!(relayed["state"]["shuffle"], true);
    assert_eq!(relayed["state"]["repeat"], "all");
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn transfer_hands_queue_and_position_to_the_new_device(db: PgPool) {
    let app = TestApp::new(db).await;
    let me = app.listener("ann").await;
    let addr = app.serve().await;
    let mut a = device(addr, &me, "mac", "laptop").await;
    let mut b = device(addr, &me, "phone", "phone").await;

    // Первый снимок без claim тоже делает отправителя активным, если активного нет
    send(
        &mut a,
        json!({ "type": "connect.state", "state": state(5000, true) }),
    )
    .await;
    for ws in [&mut a, &mut b] {
        assert_eq!(next_of(ws, "connect.active").await["deviceId"], "mac");
    }
    assert_eq!(
        next_of(&mut b, "connect.state").await["state"]["positionMs"],
        5000
    );
    tokio::time::sleep(Duration::from_millis(300)).await;

    send(
        &mut b,
        json!({ "type": "connect.transfer", "id": "t1", "to": "phone" }),
    )
    .await;
    // Подтверждение — сразу в ответ, раньше рассылки
    let ack = next_of(&mut b, "connect.ack").await;
    assert_eq!(
        (ack["id"].as_str(), ack["ok"].as_bool()),
        (Some("t1"), Some(true))
    );
    for ws in [&mut a, &mut b] {
        let moved = next_of(ws, "connect.active").await;
        assert_eq!(moved["deviceId"], "phone");
        assert_eq!(moved["from"], "mac");
        assert_eq!(moved["transfer"], true);
        assert_eq!(moved["state"]["current"]["uid"], "u1");
        assert_eq!(moved["state"]["upcoming"][0]["uid"], "u2");
        assert_eq!(moved["state"]["playing"], true);
        let pos = moved["state"]["positionMs"].as_u64().unwrap();
        assert!((5300..7000).contains(&pos), "позиция досчитана: {pos}");
    }

    // Опоздавший снимок прежнего устройства отбрасывается, а оно узнаёт, что больше не активно
    send(
        &mut a,
        json!({ "type": "connect.state", "state": state(5100, true) }),
    )
    .await;
    assert_eq!(next_of(&mut a, "connect.active").await["deviceId"], "phone");
    none_of(&mut b, "connect.state").await;
    let res = app.get("/api/devices", &me).await;
    assert_eq!(res.body["activeDeviceId"], "phone");
    assert_eq!(res.body["devices"][0]["id"], "phone");

    // Обратно, но на паузе; на устройство не в сети — отказ
    send(
        &mut b,
        json!({ "type": "connect.transfer", "to": "mac", "play": false }),
    )
    .await;
    let moved = next_of(&mut a, "connect.active").await;
    assert_eq!(moved["deviceId"], "mac");
    assert_eq!(moved["state"]["playing"], false);
    send(
        &mut a,
        json!({ "type": "connect.transfer", "id": "t2", "to": "tv" }),
    )
    .await;
    let ack = next_of(&mut a, "connect.ack").await;
    assert_eq!(
        (ack["id"].as_str(), ack["error"].as_str()),
        (Some("t2"), Some("offline"))
    );
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn silent_device_goes_offline(db: PgPool) {
    let app = TestApp::new(db).await;
    let me = app.listener("ann").await;
    let addr = app.serve().await;
    let mut a = device(addr, &me, "mac", "laptop").await;
    let mut b = device(addr, &me, "phone", "phone").await;
    send(&mut a, json!({ "type": "connect.claim" })).await;
    next_of(&mut b, "connect.active").await;

    // Последний знак жизни A — 20 секунд назад: соединение открыто, но устройства «нет»
    sqlx::query(
        "UPDATE connect_devices SET last_seen_at = now() - interval '20 seconds' WHERE id = 'mac'",
    )
    .execute(&app.state.db)
    .await
    .unwrap();
    let res = app.get("/api/devices", &me).await;
    let mac = &res.body["devices"][0];
    assert_eq!(
        (mac["id"].as_str(), mac["online"].as_bool()),
        (Some("mac"), Some(false))
    );
    assert_eq!(mac["active"], true);
    // Команды пропавшему устройству сразу получают отказ
    send(
        &mut b,
        json!({ "type": "connect.command", "id": "c1", "to": "mac", "command": { "action": "pause" } }),
    )
    .await;
    assert_eq!(next_of(&mut b, "connect.ack").await["error"], "offline");

    // Напомнило о себе — снова в сети, и пульты узнают об этом
    send(&mut a, json!({ "type": "connect.heartbeat" })).await;
    none_of(&mut a, "connect.ack").await;
    let res = app.get("/api/devices", &me).await;
    assert_eq!(res.body["devices"][0]["online"], true);

    // Закрытых вкладок в списке нет, приложения не в сети — остаются
    sqlx::query("UPDATE connect_devices SET connection = NULL, last_seen_at = now() - interval '11 minutes'")
        .execute(&app.state.db)
        .await
        .unwrap();
    sqlx::query("UPDATE playback_state SET active_connect_id = NULL")
        .execute(&app.state.db)
        .await
        .unwrap();
    sqlx::query("UPDATE connect_devices SET kind = 'web' WHERE id = 'phone'")
        .execute(&app.state.db)
        .await
        .unwrap();
    let res = app.get("/api/devices", &me).await;
    assert_eq!(ids(&res.body), [("mac".to_string(), false)]);
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn app_device_takes_its_name_from_login(db: PgPool) {
    let app = TestApp::new(db).await;
    let me = app.listener("ann").await;
    let addr = app.serve().await;
    let user = app.user_id(&me).await;
    let device_id: uuid::Uuid = sqlx::query_scalar(
        "INSERT INTO devices (user_id, name, platform) VALUES ($1::uuid, 'MacBook Pro Захара', 'macos') RETURNING id",
    )
    .bind(&user)
    .fetch_one(&app.state.db)
    .await
    .unwrap();
    let mut conn = app.state.db.acquire().await.unwrap();
    let (_, token) = dalywave_server::session::create(
        &mut conn,
        dalywave_server::session::NewSession {
            user_id: user.parse().unwrap(),
            device_id: Some(device_id),
            method: dalywave_server::session::Method::Oidc,
            user_agent: None,
            ip: std::net::Ipv4Addr::LOCALHOST.into(),
        },
    )
    .await
    .unwrap();
    // Приложение входит токеном устройства в адресе: Origin не нужен
    let (mut ws, _) = connect_async(format!("ws://{addr}/api/ws?token={token}"))
        .await
        .unwrap();
    next_of(&mut ws, "hello").await;
    send(
        &mut ws,
        json!({ "type": "connect.hello", "deviceId": "install-1", "name": "", "kind": "laptop", "platform": "macos", "capabilities": { "volume": true, "dj": true } }),
    )
    .await;
    let list = next_of(&mut ws, "connect.devices").await;
    assert_eq!(list["devices"][0]["name"], "MacBook Pro Захара");
    assert_eq!(list["devices"][0]["capabilities"]["dj"], true);
}
