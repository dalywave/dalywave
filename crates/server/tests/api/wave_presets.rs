use axum::http::{Method, StatusCode};
use serde_json::{Value, json};
use sqlx::PgPool;

use crate::common::{Auth, TestApp};

async fn names(app: &TestApp, auth: &Auth) -> Vec<String> {
    let res = app.get("/api/wave/presets", auth).await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    res.body["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["name"].as_str().unwrap().to_string())
        .collect()
}

async fn ids(app: &TestApp, auth: &Auth) -> Vec<String> {
    app.get("/api/wave/presets", auth).await.body["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["id"].as_str().unwrap().to_string())
        .collect()
}

async fn patch(app: &TestApp, id: &str, body: Value, auth: &Auth) -> crate::common::Resp {
    app.call(
        Method::PATCH,
        &format!("/api/wave/presets/{id}"),
        Some(body),
        auth,
    )
    .await
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn presets_are_seeded_saved_renamed_reordered_and_deleted(db: PgPool) {
    let app = TestApp::new(db).await;
    let me = app.listener("ann").await;
    let other = app.listener("bob").await;

    // Новый человек начинает с трёх сочетаний — ряд под волной не пустой
    assert_eq!(names(&app, &me).await, vec!["Работа", "Дорога", "Ночь"]);
    let res = app.get("/api/wave/presets", &me).await;
    let work = &res.body["items"][0];
    assert_eq!(work["icon"], "laptop");
    assert_eq!(work["settings"]["activity"], "work");
    assert_eq!(work["settings"]["language"], "none");

    // Сохранить текущее — в конец ряда
    let res = app
        .post(
            "/api/wave/presets",
            json!({
                "name": "  Спокойное ·  без слов ",
                "icon": "waves",
                "settings": { "mood": "calm", "activity": "any", "character": "mix", "language": "none" },
            }),
            &me,
        )
        .await;
    assert_eq!(res.status, StatusCode::CREATED, "{}", res.text);
    let calm = res.body["id"].as_str().unwrap().to_string();
    assert_eq!(res.body["name"], "Спокойное · без слов");
    assert_eq!(res.body["settings"]["mood"], "calm");
    assert!(res.body["position"].as_f64().unwrap() > 3072.0);
    assert_eq!(
        names(&app, &me).await.last().unwrap(),
        "Спокойное · без слов"
    );

    // Неверное: пустое название, длинное, иконка с префиксом, радио, лишнее поле
    for body in [
        json!({ "name": " ", "icon": "waves", "settings": {} }),
        json!({ "name": "я".repeat(41), "icon": "waves", "settings": {} }),
        json!({ "name": "Ок", "icon": "ph:waves", "settings": {} }),
        json!({ "name": "Ок", "icon": "waves", "settings": { "seed": { "type": "text", "text": "дождь" } } }),
        json!({ "name": "Ок", "icon": "waves", "settings": { "speed": 2 } }),
    ] {
        let res = app.post("/api/wave/presets", body.clone(), &me).await;
        assert!(
            res.status == StatusCode::BAD_REQUEST || res.status == StatusCode::UNPROCESSABLE_ENTITY,
            "{body}: {} {}",
            res.status,
            res.text
        );
    }

    // Переименовать и сменить иконку; чужое — 404
    let res = patch(
        &app,
        &calm,
        json!({ "name": "Фокус", "icon": "headphones" }),
        &me,
    )
    .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    assert_eq!(res.body["name"], "Фокус");
    assert_eq!(res.body["icon"], "headphones");
    assert_eq!(res.body["settings"]["mood"], "calm", "настройки не тронуты");
    let res = patch(&app, &calm, json!({ "name": "Чужое" }), &other).await;
    assert_eq!(res.status, StatusCode::NOT_FOUND, "{}", res.text);
    assert_eq!(names(&app, &other).await, vec!["Работа", "Дорога", "Ночь"]);

    // Порядок: названные — первыми, неизвестные пропускаются, остальные — как были
    let before = ids(&app, &me).await;
    let res = app
        .call(
            Method::PUT,
            "/api/wave/presets/order",
            Some(json!({ "ids": [calm, before[2], uuid::Uuid::now_v7()] })),
            &me,
        )
        .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    assert_eq!(
        names(&app, &me).await,
        vec!["Фокус", "Ночь", "Работа", "Дорога"]
    );

    // Удалить; повтор и чужое — 404
    let res = app
        .delete(&format!("/api/wave/presets/{calm}"), &other)
        .await;
    assert_eq!(res.status, StatusCode::NOT_FOUND, "{}", res.text);
    let res = app.delete(&format!("/api/wave/presets/{calm}"), &me).await;
    assert_eq!(res.status, StatusCode::NO_CONTENT, "{}", res.text);
    let res = app.delete(&format!("/api/wave/presets/{calm}"), &me).await;
    assert_eq!(res.status, StatusCode::NOT_FOUND, "{}", res.text);
    assert_eq!(names(&app, &me).await, vec!["Ночь", "Работа", "Дорога"]);

    // Удалённые сиды не возвращаются: пустой ряд остаётся пустым
    for id in ids(&app, &me).await {
        app.delete(&format!("/api/wave/presets/{id}"), &me).await;
    }
    assert!(names(&app, &me).await.is_empty());

    // Предел — 24
    for i in 0..24 {
        let res = app
            .post(
                "/api/wave/presets",
                json!({ "name": format!("№{i}"), "icon": "sparkle", "settings": {} }),
                &me,
            )
            .await;
        assert_eq!(res.status, StatusCode::CREATED, "{}", res.text);
    }
    let res = app
        .post(
            "/api/wave/presets",
            json!({ "name": "лишнее", "icon": "sparkle", "settings": {} }),
            &me,
        )
        .await;
    assert_eq!(res.status, StatusCode::BAD_REQUEST, "{}", res.text);
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn presets_sync_with_rev_and_tombstones(db: PgPool) {
    let app = TestApp::new(db).await;
    let me = app.listener("ann").await;

    let res = app.get("/api/sync", &me).await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    let first = res.body["wavePresets"].as_array().unwrap();
    assert_eq!(first.len(), 3);
    assert_eq!(first[0]["settings"]["mood"], "calm");
    assert!(first[0]["rev"].as_i64().unwrap() > 0);
    let cursor = res.body["cursor"].as_str().unwrap().to_string();

    // Переименование поднимает rev, удаление пишет tombstone
    let night = first[2]["id"].as_str().unwrap().to_string();
    let road = first[1]["id"].as_str().unwrap().to_string();
    let res = app
        .call(
            Method::PATCH,
            &format!("/api/wave/presets/{road}"),
            Some(json!({ "name": "Трасса" })),
            &me,
        )
        .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    app.delete(&format!("/api/wave/presets/{night}"), &me).await;

    let res = app.get(&format!("/api/sync?since={cursor}"), &me).await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    // Пока в кластере идут чужие пишущие транзакции, прежние строки могут прийти повторно (см. sync.rs)
    let changed = res.body["wavePresets"].as_array().unwrap();
    let road_row = changed.iter().find(|p| p["id"] == road.as_str()).unwrap();
    assert_eq!(road_row["name"], "Трасса");
    assert!(road_row["rev"].as_i64() > first[1]["rev"].as_i64());
    let gone: Vec<&Value> = res.body["tombstones"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|t| t["entity"] == "wavePresets")
        .collect();
    assert_eq!(gone.len(), 1, "{}", res.text);
    assert_eq!(gone[0]["key"], json!({ "id": night }));
}
