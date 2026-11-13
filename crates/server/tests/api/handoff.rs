//! Подхват на другом устройстве: сигналы `connect.signal` сводятся на сервере, по совпадению наушников
//! музыка переходит сама («Сразу») или устройство получает предложение «Продолжить здесь?».

use axum::http::{Method, StatusCode};
use serde_json::{Value, json};
use sqlx::PgPool;

use crate::{
    common::{Auth, TestApp},
    connect::{device, next_of, none_of, send, state},
};

fn signal(kind: &str, output: Option<&str>, playing: bool) -> Value {
    json!({
        "type": "connect.signal", "signal": kind, "playing": playing,
        "output": output.map(|name| json!({ "name": name, "kind": "bluetooth" })),
    })
}

async fn set_mode(app: &TestApp, me: &Auth, mode: &str) {
    let res = app
        .call(
            Method::PATCH,
            "/api/settings",
            Some(json!({ "handoff": mode })),
            me,
        )
        .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    assert_eq!(res.body["handoff"], mode);
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn headphones_moving_to_the_phone_bring_a_suggestion(db: PgPool) {
    let app = TestApp::new(db).await;
    let me = app.listener("ann").await;
    assert_eq!(
        app.get("/api/settings", &me).await.body["handoff"],
        "suggest",
        "по умолчанию — предложение"
    );
    let addr = app.serve().await;
    let mut mac = device(addr, &me, "mac", "laptop").await;
    let mut phone = device(addr, &me, "phone", "phone").await;
    send(
        &mut mac,
        json!({ "type": "connect.state", "state": state(5000, true) }),
    )
    .await;
    assert_eq!(
        next_of(&mut phone, "connect.active").await["deviceId"],
        "mac"
    );

    // Выключено — сигналы помнятся, но ничего не происходит
    set_mode(&app, &me, "off").await;
    send(&mut mac, signal("output_lost", Some("AirPods Ann"), true)).await;
    send(
        &mut phone,
        signal("output_added", Some("AirPods Ann"), false),
    )
    .await;
    none_of(&mut phone, "connect.handoff").await;

    // Включено: те же наушники пришли на телефон — ему предложение, Mac ничего не получает
    set_mode(&app, &me, "suggest").await;
    send(
        &mut phone,
        signal("output_added", Some("airpods  ann"), false),
    )
    .await;
    let h = next_of(&mut phone, "connect.handoff").await;
    assert_eq!(h["to"], "phone");
    assert_eq!(h["from"], "mac");
    assert_eq!(h["reason"], "headphones");
    assert_eq!(h["transferred"], false);
    assert_eq!(h["play"], true);
    assert_eq!(h["output"], "airpods  ann");
    none_of(&mut mac, "connect.handoff").await;
    // Активное не сменилось
    assert_eq!(
        app.get("/api/devices", &me).await.body["activeDeviceId"],
        "mac"
    );
    // Повтор того же сигнала не дёргает человека второй раз
    send(
        &mut phone,
        signal("output_added", Some("AirPods Ann"), false),
    )
    .await;
    none_of(&mut phone, "connect.handoff").await;
    assert!(
        app.audit_kinds()
            .await
            .contains(&"connect.handoff".to_string()),
        "причина — в журнале"
    );
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn in_auto_mode_music_follows_the_headphones(db: PgPool) {
    let app = TestApp::new(db).await;
    let me = app.listener("bob").await;
    set_mode(&app, &me, "auto").await;
    let addr = app.serve().await;
    let mut mac = device(addr, &me, "mac", "laptop").await;
    let mut phone = device(addr, &me, "phone", "phone").await;
    send(
        &mut mac,
        json!({ "type": "connect.state", "state": state(5000, true) }),
    )
    .await;
    for ws in [&mut mac, &mut phone] {
        assert_eq!(next_of(ws, "connect.active").await["deviceId"], "mac");
    }

    // Телефон увидел наушники раньше, чем Mac сообщил о пропаже — порядок не важен
    send(
        &mut phone,
        signal("output_added", Some("AirPods Bob"), false),
    )
    .await;
    send(&mut mac, signal("output_lost", Some("AirPods Bob"), true)).await;
    for ws in [&mut mac, &mut phone] {
        let moved = next_of(ws, "connect.active").await;
        assert_eq!(moved["deviceId"], "phone");
        assert_eq!(moved["transfer"], true);
        assert_eq!(moved["state"]["playing"], true);
        assert!(moved["state"]["positionMs"].as_u64().unwrap() >= 5000);
        let h = next_of(ws, "connect.handoff").await;
        assert_eq!(h["transferred"], true);
        assert_eq!(h["to"], "phone");
    }
    assert_eq!(
        app.get("/api/devices", &me).await.body["activeDeviceId"],
        "phone"
    );
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn unrelated_signals_change_nothing(db: PgPool) {
    let app = TestApp::new(db).await;
    let me = app.listener("cid").await;
    set_mode(&app, &me, "auto").await;
    let addr = app.serve().await;
    let mut mac = device(addr, &me, "mac", "laptop").await;
    let mut phone = device(addr, &me, "phone", "phone").await;
    send(
        &mut mac,
        json!({ "type": "connect.state", "state": state(5000, true) }),
    )
    .await;
    assert_eq!(
        next_of(&mut phone, "connect.active").await["deviceId"],
        "mac"
    );
    // Другие наушники; колонка; блокировка только что
    send(&mut mac, signal("output_lost", Some("AirPods Cid"), true)).await;
    send(
        &mut phone,
        signal("output_added", Some("Sony WH-1000XM5"), false),
    )
    .await;
    send(
        &mut phone,
        json!({ "type": "connect.signal", "signal": "foreground",
                "output": { "name": "AirPods Cid", "kind": "airplay" } }),
    )
    .await;
    send(&mut mac, signal("locked", None, false)).await;
    send(
        &mut phone,
        signal("foreground", Some("Sony WH-1000XM5"), false),
    )
    .await;
    none_of(&mut phone, "connect.handoff").await;
    none_of(&mut phone, "connect.active").await;
    // Незнакомый сигнал — не ошибка соединения
    send(
        &mut phone,
        json!({ "type": "connect.signal", "signal": "dance" }),
    )
    .await;
    none_of(&mut phone, "connect.handoff").await;
}
