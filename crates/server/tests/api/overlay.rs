//! Оверлей «Сейчас играет» для стрима: секретная ссылка, «Сменить ссылку», данные без входа — разово и
//! потоком SSE; пауза прячет плашку.

use std::time::Duration;

use axum::http::{Method, StatusCode};
use serde_json::{Value, json};
use sqlx::PgPool;

use crate::{
    common::{Auth, TestApp},
    connect::{device, next_of, send, state},
};

fn key(url: &str) -> String {
    url.split("?k=")
        .nth(1)
        .expect("ссылка с ключом")
        .to_string()
}

/// Следующее событие `now` потока: JSON из строки `data:`.
async fn next_now(res: &mut reqwest::Response, buf: &mut String) -> Value {
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if let Some(end) = buf.find("\n\n") {
                let block: String = buf.drain(..end + 2).collect();
                if block.contains("event: now") {
                    let data = block
                        .lines()
                        .find_map(|l| l.strip_prefix("data: "))
                        .expect("data");
                    return serde_json::from_str(data).unwrap();
                }
                continue;
            }
            let chunk = res.chunk().await.unwrap().expect("поток закрылся");
            buf.push_str(&String::from_utf8_lossy(&chunk));
        }
    })
    .await
    .expect("нет события now за 5 с")
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn secret_link_shows_what_plays(db: PgPool) {
    let app = TestApp::new(db).await;
    let me = app.listener("streamer").await;
    let res = app.get("/api/overlay", &me).await;
    assert_eq!(res.status, StatusCode::OK);
    assert_eq!(res.body["link"], Value::Null);
    let res = app.get("/api/overlay/now?k=dwo_nope", &Auth::None).await;
    assert_eq!(res.status, StatusCode::NOT_FOUND);

    let res = app.post("/api/overlay/link", json!({}), &me).await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    let url = res.body["url"].as_str().unwrap().to_string();
    assert!(url.contains("/overlay?k=dwo_"), "{url}");
    assert_eq!(res.body["hidePrivate"], true);
    // Ссылку можно скопировать снова: та же
    assert_eq!(app.get("/api/overlay", &me).await.body["link"]["url"], url);
    let k = key(&url);

    // Ничего не играет — плашка спрятана
    let res = app
        .get(&format!("/api/overlay/now?k={k}"), &Auth::None)
        .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    assert_eq!(res.body["visible"], false);
    assert_eq!(res.body["track"], Value::Null);

    let addr = app.serve().await;
    let mut mac = device(addr, &me, "mac", "laptop").await;
    send(
        &mut mac,
        json!({ "type": "connect.state", "state": state(5000, true) }),
    )
    .await;
    next_of(&mut mac, "connect.active").await;
    let res = app
        .get(&format!("/api/overlay/now?k={k}"), &Auth::None)
        .await;
    assert_eq!(res.body["visible"], true);
    assert_eq!(res.body["track"]["title"], "Первый");
    assert!(res.body["positionMs"].as_u64().unwrap() >= 5000);

    // Поток: сразу — что играет, на паузе — спрятать
    let mut stream = reqwest::get(format!("http://{addr}/api/overlay/stream?k={k}"))
        .await
        .unwrap();
    assert_eq!(stream.status(), 200);
    assert!(
        stream.headers()["content-type"]
            .to_str()
            .unwrap()
            .starts_with("text/event-stream")
    );
    let mut buf = String::new();
    let first = next_now(&mut stream, &mut buf).await;
    assert_eq!(
        (first["visible"].as_bool(), first["track"]["title"].as_str()),
        (Some(true), Some("Первый"))
    );
    send(
        &mut mac,
        json!({ "type": "connect.state", "state": state(9000, false) }),
    )
    .await;
    let paused = next_now(&mut stream, &mut buf).await;
    assert_eq!(paused["visible"], false);
    assert_eq!(paused["playing"], false);

    // «Сменить ссылку»: прежняя перестаёт работать
    let res = app.post("/api/overlay/link", json!({}), &me).await;
    let k2 = key(res.body["url"].as_str().unwrap());
    assert_ne!(k, k2);
    let res = app
        .get(&format!("/api/overlay/now?k={k}"), &Auth::None)
        .await;
    assert_eq!(res.status, StatusCode::NOT_FOUND);
    let res = app
        .get(&format!("/api/overlay/now?k={k2}"), &Auth::None)
        .await;
    assert_eq!(res.status, StatusCode::OK);
    assert!(
        app.audit_kinds()
            .await
            .contains(&"overlay.link".to_string())
    );

    // Приватная сессия: трек не виден вовсе (на паузе он остаётся, но плашка спрятана)
    let now = |k: String| {
        let app = &app;
        async move {
            app.get(&format!("/api/overlay/now?k={k}"), &Auth::None)
                .await
                .body
        }
    };
    assert_eq!(now(k2.clone()).await["track"]["title"], "Первый");
    let user: uuid::Uuid = app.user_id(&me).await.parse().unwrap();
    sqlx::query(
        "INSERT INTO private_sessions (user_id, ended_at) VALUES ($1, now() + interval '1 hour')",
    )
    .bind(user)
    .execute(&app.state.db)
    .await
    .unwrap();
    assert_eq!(now(k2.clone()).await["track"], Value::Null);
    // «Прятать в приватной сессии» выключили — трек снова виден
    let res = app
        .call(
            Method::PATCH,
            "/api/overlay",
            Some(json!({ "hidePrivate": false })),
            &me,
        )
        .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    assert_eq!(res.body["link"]["hidePrivate"], false);
    assert_eq!(now(k2.clone()).await["track"]["title"], "Первый");

    // Выключить: ссылка больше не работает
    let res = app.delete("/api/overlay", &me).await;
    assert_eq!(res.status, StatusCode::NO_CONTENT);
    let res = app
        .get(&format!("/api/overlay/now?k={k2}"), &Auth::None)
        .await;
    assert_eq!(res.status, StatusCode::NOT_FOUND);
    let res = app
        .call(
            Method::PATCH,
            "/api/overlay",
            Some(json!({ "hidePrivate": true })),
            &me,
        )
        .await;
    assert_eq!(res.status, StatusCode::NOT_FOUND);
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn link_belongs_to_its_owner_only(db: PgPool) {
    let app = TestApp::new(db).await;
    let ann = app.listener("ann").await;
    let bob = app.listener("bob").await;
    let a = app.post("/api/overlay/link", json!({}), &ann).await;
    let b = app.post("/api/overlay/link", json!({}), &bob).await;
    assert_ne!(a.body["url"], b.body["url"]);
    // Без входа управлять ссылкой нельзя
    let res = app.post("/api/overlay/link", json!({}), &Auth::None).await;
    assert_eq!(res.status, StatusCode::UNAUTHORIZED);
}
