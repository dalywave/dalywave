//! Где заиграет музыка, выбранная на пульте: присутствие у активного устройства, медиаплеер, настройки
//! устройства, наушники телефона, поправки и гонка двух пультов (правила — crates/server/src/choice.rs).

use axum::http::{Method, StatusCode};
use serde_json::{Value, json};
use sqlx::PgPool;

use crate::{
    common::{Auth, TestApp},
    connect::{device, next_of, none_of, send, state},
};

type Socket =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

fn album(id: &str) -> Value {
    json!({
        "type": "connect.command", "id": id, "to": "pc",
        "command": {
            "action": "playTracks", "trackIds": ["0193a8a0-0000-7000-8000-000000000001"],
            "context": { "type": "album", "id": "a1", "title": "Альбом" },
        },
        "pick": {},
    })
}

/// ПК играет, его трогали `idle_ms` назад.
async fn pc_playing(pc: &mut Socket, idle_ms: u64) {
    send(pc, json!({ "type": "connect.claim" })).await;
    next_of(pc, "connect.active").await;
    send(
        pc,
        json!({ "type": "connect.state", "state": state(1000, true) }),
    )
    .await;
    next_of(pc, "connect.state").await;
    presence(pc, idle_ms).await;
}

async fn presence(pc: &mut Socket, idle_ms: u64) {
    send(
        pc,
        json!({ "type": "connect.presence", "idleMs": idle_ms, "source": "system" }),
    )
    .await;
    // Сообщения одного соединения обрабатываются по порядку: ответ на ping — после записи присутствия
    none_of(pc, "connect.placed").await;
}

async fn patch(app: &TestApp, me: &Auth, id: &str, body: Value) -> crate::common::Resp {
    app.call(Method::PATCH, &format!("/api/devices/{id}"), Some(body), me)
        .await
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn music_follows_the_person(db: PgPool) {
    let app = TestApp::new(db).await;
    let me = app.listener("ann").await;
    let addr = app.serve().await;
    let mut pc = device(addr, &me, "pc", "desktop").await;
    let mut mac = device(addr, &me, "mac", "laptop").await;
    pc_playing(&mut pc, 5_000).await;

    // У ПК кто-то есть: ноутбук — пульт, команда уходит на ПК
    send(&mut mac, album("p1")).await;
    let placed = next_of(&mut mac, "connect.placed").await;
    assert_eq!(
        placed,
        json!({ "type": "connect.placed", "id": "p1", "here": false, "deviceId": "pc", "other": "mac", "reason": "in_use" })
    );
    let cmd = next_of(&mut pc, "connect.command").await;
    assert_eq!(cmd["id"], "p1");
    assert_eq!(cmd["from"], "mac");
    assert_eq!(cmd["command"]["action"], "playTracks");

    // ПК не трогали 10 минут: играет на ноутбуке, ПК встаёт на паузу (его делают пультом)
    presence(&mut pc, 10 * 60 * 1000).await;
    send(&mut mac, album("p2")).await;
    let placed = next_of(&mut mac, "connect.placed").await;
    assert_eq!(placed["here"], true);
    assert_eq!(placed["deviceId"], "mac");
    assert_eq!(placed["other"], "pc");
    assert_eq!(placed["reason"], "away");
    let active = next_of(&mut pc, "connect.active").await;
    assert_eq!(active["deviceId"], "mac");
    assert_eq!(active["from"], "pc");
    assert_eq!(active["transfer"], false);
    none_of(&mut pc, "connect.command").await;
    assert_eq!(
        app.get("/api/devices", &me).await.body["activeDeviceId"],
        "mac"
    );

    // Обычные команды пульта решения не требуют
    send(
        &mut pc,
        json!({ "type": "connect.command", "id": "s1", "to": "mac", "command": { "action": "pause" }, "pick": {} }),
    )
    .await;
    assert_eq!(next_of(&mut mac, "connect.command").await["id"], "s1");
    none_of(&mut pc, "connect.placed").await;
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn media_player_and_device_settings(db: PgPool) {
    let app = TestApp::new(db).await;
    let me = app.listener("ann").await;
    let addr = app.serve().await;
    let mut pc = device(addr, &me, "pc", "desktop").await;
    let mut mac = device(addr, &me, "mac", "laptop").await;
    pc_playing(&mut pc, 60 * 60 * 1000).await;

    // ПК — медиаплеер (настроили с ноутбука): выбор идёт туда, хоть его и не трогали час
    let res = patch(&app, &me, "pc", json!({ "mediaPlayer": true })).await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    let pc_row = |list: &Value| {
        list["devices"]
            .as_array()
            .unwrap()
            .iter()
            .find(|d| d["id"] == "pc")
            .cloned()
            .unwrap()
    };
    assert_eq!(pc_row(&res.body)["mediaPlayer"], true);
    assert_eq!(pc_row(&res.body)["choice"], "auto");
    // Список с новой настройкой — всем устройствам
    loop {
        let list = next_of(&mut pc, "connect.devices").await;
        if pc_row(&list)["mediaPlayer"] == true {
            break;
        }
    }
    send(&mut mac, album("m1")).await;
    let placed = next_of(&mut mac, "connect.placed").await;
    assert_eq!(placed["here"], false);
    assert_eq!(placed["reason"], "media_player");
    assert_eq!(next_of(&mut pc, "connect.command").await["id"], "m1");

    // Ноутбук: «Всегда здесь» — сильнее медиаплеера
    let res = patch(&app, &me, "mac", json!({ "choice": "here" })).await;
    assert_eq!(res.status, StatusCode::OK);
    send(&mut mac, album("m2")).await;
    let placed = next_of(&mut mac, "connect.placed").await;
    assert_eq!(placed["here"], true);
    assert_eq!(placed["reason"], "setting");

    // Не своё и неизвестное устройство — 404; неверное значение — 400/422
    assert_eq!(
        patch(&app, &me, "ghost", json!({ "choice": "here" }))
            .await
            .status,
        StatusCode::NOT_FOUND
    );
    let bob = app.listener("bob").await;
    assert_eq!(
        patch(&app, &bob, "pc", json!({ "mediaPlayer": false }))
            .await
            .status,
        StatusCode::NOT_FOUND
    );
    assert!(
        patch(&app, &me, "pc", json!({ "choice": "nowhere" }))
            .await
            .status
            .is_client_error()
    );
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn phone_with_headphones_plays_on_the_phone(db: PgPool) {
    let app = TestApp::new(db).await;
    let me = app.listener("ann").await;
    let addr = app.serve().await;
    let mut pc = device(addr, &me, "pc", "desktop").await;
    let mut phone = device(addr, &me, "phone", "phone").await;
    pc_playing(&mut pc, 1_000).await;

    let mut pick = album("h1");
    pick["pick"] = json!({ "headphones": true });
    send(&mut phone, pick).await;
    let placed = next_of(&mut phone, "connect.placed").await;
    assert_eq!(placed["here"], true);
    assert_eq!(placed["reason"], "headphones");

    // Без наушников — обычное правило: у ПК только что был ввод
    send(
        &mut phone,
        json!({ "type": "connect.transfer", "to": "pc" }),
    )
    .await;
    next_of(&mut pc, "connect.active").await;
    send(&mut phone, album("h2")).await;
    assert_eq!(
        next_of(&mut phone, "connect.placed").await["reason"],
        "in_use"
    );
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn two_same_corrections_are_remembered(db: PgPool) {
    let app = TestApp::new(db).await;
    let me = app.listener("ann").await;
    let addr = app.serve().await;
    let mut pc = device(addr, &me, "pc", "desktop").await;
    let mut mac = device(addr, &me, "mac", "laptop").await;
    // ПК давно не трогали, но он у колонок: автоматика дважды ошибается, человек дважды поправляет
    pc_playing(&mut pc, 60 * 60 * 1000).await;

    for (i, learned) in [(1, false), (2, true)] {
        send(&mut mac, album(&format!("c{i}"))).await;
        let placed = next_of(&mut mac, "connect.placed").await;
        assert_eq!(placed["here"], true, "попытка {i}");
        assert_eq!(placed["reason"], "away");
        // «Включить на ПК» в тосте — перенос туда
        send(
            &mut mac,
            json!({ "type": "connect.transfer", "id": format!("t{i}"), "to": "pc" }),
        )
        .await;
        assert_eq!(next_of(&mut mac, "connect.ack").await["ok"], true);
        if learned {
            let l = next_of(&mut mac, "connect.learned").await;
            assert_eq!(l["from"], "mac");
            assert_eq!(l["to"], "pc");
            assert_eq!(l["here"], false);
            assert!(l["until"].is_string());
        } else {
            none_of(&mut mac, "connect.learned").await;
        }
    }

    // Запомнено: с ноутбука — на ПК
    send(&mut mac, album("c3")).await;
    let placed = next_of(&mut mac, "connect.placed").await;
    assert_eq!(placed["here"], false);
    assert_eq!(placed["reason"], "learned");
    assert_eq!(next_of(&mut pc, "connect.command").await["id"], "c3");

    // Обратная поправка («Играть здесь» на ноутбуке) — забыто
    send(&mut mac, json!({ "type": "connect.transfer", "to": "mac" })).await;
    next_of(&mut pc, "connect.active").await;
    send(&mut mac, json!({ "type": "connect.transfer", "to": "pc" })).await;
    next_of(&mut pc, "connect.active").await;
    send(&mut mac, album("c4")).await;
    assert_eq!(next_of(&mut mac, "connect.placed").await["reason"], "away");
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn uncorrected_choice_breaks_the_streak(db: PgPool) {
    let app = TestApp::new(db).await;
    let me = app.listener("ann").await;
    let addr = app.serve().await;
    let mut pc = device(addr, &me, "pc", "desktop").await;
    let mut mac = device(addr, &me, "mac", "laptop").await;
    pc_playing(&mut pc, 60 * 60 * 1000).await;

    let back_to_pc =
        |n: u32| json!({ "type": "connect.transfer", "id": format!("b{n}"), "to": "pc" });
    // Поправка, потом решение без поправки (человек согласился), потом снова поправка: не запоминается
    send(&mut mac, album("u1")).await;
    assert_eq!(next_of(&mut mac, "connect.placed").await["here"], true);
    send(&mut mac, back_to_pc(1)).await;
    next_of(&mut mac, "connect.ack").await;

    send(&mut mac, album("u2")).await;
    assert_eq!(next_of(&mut mac, "connect.placed").await["here"], true);
    // Окно поправки прошло: перенос обратно — уже не поправка
    sqlx::query("UPDATE connect_choices SET decided_at = now() - interval '10 minutes'")
        .execute(&app.state.db)
        .await
        .unwrap();
    send(&mut mac, back_to_pc(2)).await;
    next_of(&mut mac, "connect.ack").await;

    send(&mut mac, album("u3")).await;
    assert_eq!(next_of(&mut mac, "connect.placed").await["here"], true);
    send(&mut mac, back_to_pc(3)).await;
    next_of(&mut mac, "connect.ack").await;
    none_of(&mut mac, "connect.learned").await;
    send(&mut mac, album("u4")).await;
    assert_eq!(next_of(&mut mac, "connect.placed").await["reason"], "away");
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn two_remotes_choosing_at_once_agree(db: PgPool) {
    let app = TestApp::new(db).await;
    let me = app.listener("ann").await;
    let addr = app.serve().await;
    let mut pc = device(addr, &me, "pc", "desktop").await;
    let mut mac = device(addr, &me, "mac", "laptop").await;
    let mut tablet = device(addr, &me, "tab", "tablet").await;
    pc_playing(&mut pc, 60 * 60 * 1000).await;

    // Оба пульта выбирают одновременно: решения идут по одному под блокировкой
    let mut second = album("r2");
    second["to"] = json!("pc");
    send(&mut mac, album("r1")).await;
    send(&mut tablet, second).await;
    let a = next_of(&mut mac, "connect.placed").await;
    let b = next_of(&mut tablet, "connect.placed").await;
    let (first, later, later_from) = if a["here"] == true {
        (a, b, "tab")
    } else {
        (b, a, "mac")
    };
    assert_eq!(first["here"], true, "кто-то один играет у себя");
    // Второй решал уже относительно нового активного: о присутствии там ничего не известно — туда
    assert_eq!(later["here"], false);
    assert_eq!(later["deviceId"], first["deviceId"]);
    assert_eq!(later["other"], later_from);
    assert_eq!(later["reason"], "unknown");
    none_of(&mut pc, "connect.command").await;
}
