use std::time::Duration;

use axum::{
    body::Body,
    http::{Request, StatusCode, header},
};
use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use sqlx::PgPool;
use tokio_tungstenite::{
    MaybeTlsStream, WebSocketStream, connect_async,
    tungstenite::{Error as WsError, Message, client::IntoClientRequest},
};

use crate::{
    common::{Auth, PUBLIC_URL, TestApp},
    deezer::MODERN_SADNESS,
    mock::Person,
};

type Socket = WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>;

fn cookie_of(auth: &Auth) -> String {
    match auth {
        Auth::Cookie(c) => c.clone(),
        _ => unreachable!(),
    }
}

async fn connect(
    addr: std::net::SocketAddr,
    query: &str,
    headers: &[(&str, &str)],
) -> Result<Socket, WsError> {
    let mut req = format!("ws://{addr}/api/ws{query}")
        .into_client_request()
        .unwrap();
    for (k, v) in headers {
        req.headers_mut().insert(
            header::HeaderName::from_bytes(k.as_bytes()).unwrap(),
            v.parse().unwrap(),
        );
    }
    connect_async(req).await.map(|(ws, _)| ws)
}

/// Следующее JSON-сообщение (ping-кадры пропускаются).
async fn next_json(ws: &mut Socket) -> Value {
    loop {
        let msg = tokio::time::timeout(Duration::from_secs(5), ws.next())
            .await
            .expect("no message in 5 s")
            .expect("socket closed")
            .unwrap();
        match msg {
            Message::Text(t) => return serde_json::from_str(t.as_str()).unwrap(),
            Message::Ping(_) | Message::Pong(_) => continue,
            other => panic!("unexpected {other:?}"),
        }
    }
}

async fn ping(ws: &mut Socket, id: &str) -> Value {
    ws.send(Message::Text(
        json!({ "type": "ping", "id": id }).to_string().into(),
    ))
    .await
    .unwrap();
    next_json(ws).await
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn websocket_checks_session_and_origin(db: PgPool) {
    let app = TestApp::new(db).await;
    let waiting = Auth::Cookie(app.login(&Person::new("ann")).await);
    let me = app.listener("bob").await;
    let cookie = cookie_of(&me);

    // Без апгрейда видны ответы проверок входа в общем формате
    let call = |cookie: Option<&str>, origin: Option<&str>, uri: &str| {
        let mut req = Request::builder().uri(uri.to_string());
        if let Some(c) = cookie {
            req = req.header(header::COOKIE, c);
        }
        if let Some(o) = origin {
            req = req.header(header::ORIGIN, o);
        }
        req.body(Body::empty()).unwrap()
    };
    let res = app.send(call(None, None, "/api/ws")).await;
    assert_eq!(res.status, StatusCode::UNAUTHORIZED);
    let res = app
        .send(call(
            Some(&cookie_of(&waiting)),
            Some(PUBLIC_URL),
            "/api/ws",
        ))
        .await;
    assert_eq!(res.code(), "no_access");
    // Cookie со страницы чужого сайта (Cross-Site WebSocket Hijacking)
    let res = app
        .send(call(Some(&cookie), Some("https://evil.example"), "/api/ws"))
        .await;
    assert_eq!(res.code(), "bad_origin");
    let res = app.send(call(Some(&cookie), None, "/api/ws")).await;
    assert_eq!(res.code(), "bad_origin");
    let res = app
        .send(call(Some(&cookie), Some(PUBLIC_URL), "/api/ws"))
        .await;
    assert_eq!(res.status, StatusCode::BAD_REQUEST, "не WebSocket");
    let res = app.send(call(None, None, "/api/ws?token=garbage")).await;
    assert_eq!(res.status, StatusCode::UNAUTHORIZED);

    let addr = app.serve().await;
    let mut ws = connect(addr, "", &[("cookie", &cookie), ("origin", PUBLIC_URL)])
        .await
        .unwrap();
    let hello = next_json(&mut ws).await;
    assert_eq!(hello["type"], "hello");
    assert_eq!(hello["userId"], app.user_id(&me).await.as_str());
    assert_eq!(hello["heartbeatMs"], 25000);
    assert_eq!(
        ping(&mut ws, "42").await,
        json!({ "type": "pong", "id": "42" })
    );
    // Незнакомое сообщение не рвёт соединение
    ws.send(Message::Text(r#"{"type":"dance"}"#.into()))
        .await
        .unwrap();
    assert_eq!(ping(&mut ws, "43").await["id"], "43");

    // Токен в адресе — для клиентов без заголовков; Origin тогда не нужен
    let token = cookie.trim_start_matches("dw_session=").to_string();
    let mut by_token = connect(addr, &format!("?token={token}"), &[])
        .await
        .unwrap();
    assert_eq!(next_json(&mut by_token).await["type"], "hello");
    let refused = connect(
        addr,
        "",
        &[("cookie", &cookie), ("origin", "https://evil.example")],
    )
    .await;
    assert!(refused.is_err());

    // Остановка сервера закрывает соединение кодом «перезапуск»
    app.state.hub.shutdown();
    let msg = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            match ws.next().await {
                Some(Ok(Message::Close(frame))) => return frame,
                Some(Ok(_)) => continue,
                _ => return None,
            }
        }
    })
    .await
    .unwrap();
    assert_eq!(u16::from(msg.unwrap().code), 1012);
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn events_reach_only_their_audience(db: PgPool) {
    let app = TestApp::new(db).await;
    let (_, admin) = app.admin("boss").await;
    let me = app.listener("ann").await;
    let ep = app.resolve_id("album", MODERN_SADNESS, &me).await;
    let album = app.get(&format!("/api/albums/{ep}"), &me).await;
    let track = album.body["tracks"][0]["id"].as_str().unwrap().to_string();

    let addr = app.serve().await;
    app.state.hub.wait_listening().await;
    let mut mine = connect(
        addr,
        "",
        &[("cookie", &cookie_of(&me)), ("origin", PUBLIC_URL)],
    )
    .await
    .unwrap();
    let mut admins = connect(
        addr,
        "",
        &[("cookie", &cookie_of(&admin)), ("origin", PUBLIC_URL)],
    )
    .await
    .unwrap();
    assert_eq!(next_json(&mut mine).await["type"], "hello");
    assert_eq!(next_json(&mut admins).await["type"], "hello");

    let recording: String =
        sqlx::query_scalar("SELECT recording_id::text FROM tracks WHERE id = $1::uuid")
            .bind(&track)
            .fetch_one(&app.state.db)
            .await
            .unwrap();
    // Изменение медиатеки (через другой процесс — тот же путь через NOTIFY) — только её владельцу.
    // Лайк заодно ставит запись на скачивание, а скачивание видят все
    assert_eq!(
        app.put(&format!("/api/library/tracks/{track}"), &me)
            .await
            .status,
        StatusCode::NO_CONTENT
    );
    let event = next_json(&mut mine).await;
    assert_eq!(
        event,
        json!({ "type": "library.changed", "entities": ["likedTracks"] })
    );
    for ws in [&mut mine, &mut admins] {
        let event = next_json(ws).await;
        assert_eq!(event["type"], "acquisition.updated");
        assert_eq!(event["recordingId"], recording.as_str());
        assert_eq!(event["status"], "queued");
        assert_eq!(event["audioStatus"], "queued");
    }
    assert_eq!(
        ping(&mut admins, "a").await["type"],
        "pong",
        "чужая медиатека не приходит"
    );

    // Состояние очереди — только администраторам
    let res = app
        .get(&format!("/api/images/albums/{ep}/250.jpg"), &Auth::None)
        .await;
    assert_eq!(res.status, StatusCode::FOUND);
    let (kind, _) = app.run_job("mirror").await.unwrap();
    assert_eq!(kind, "mirror_image");
    let running = next_json(&mut admins).await;
    assert_eq!(running["type"], "job.updated");
    assert_eq!(running["kind"], "mirror_image");
    assert_eq!(running["lane"], "mirror");
    assert_eq!(running["status"], "running");
    let done = next_json(&mut admins).await;
    assert_eq!(done["status"], "done");
    assert_eq!(
        ping(&mut mine, "m").await["type"],
        "pong",
        "слушателю очередь не приходит"
    );

    // Повторная постановка не создаёт второго скачивания
    let again = dalywave_server::acquire::enqueue(
        &app.state.db,
        dalywave_server::acquire::Request::new(
            track.parse().unwrap(),
            dalywave_protocol::jobs::AcquireReason::Play,
            "now",
        )
        .priority(10),
    )
    .await
    .unwrap();
    assert!(
        matches!(again, dalywave_server::acquire::Enqueue::Active { .. }),
        "{again:?}"
    );
}
