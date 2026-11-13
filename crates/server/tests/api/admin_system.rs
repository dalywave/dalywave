//! «Система» и настройки клипов: здоровье, резервные копии, выпуски, переменные окружения,
//! клип целиком в такт с треком.

use axum::http::{Method, StatusCode};
use serde_json::{Value, json};
use sqlx::PgPool;

use crate::{
    common::{Auth, Resp, TestApp},
    deezer::LIGHT_ME_UP,
    fetcher::{new_downloader, no_intervals, upload_key},
};

async fn claim(app: &TestApp, token: &str, features: Value) -> Resp {
    app.post(
        "/api/fetcher/clips/claim",
        json!({ "workerId": "test-mac", "features": features }),
        &Auth::Bearer(token.into()),
    )
    .await
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn system_sections_need_admin(db: PgPool) {
    let app = TestApp::new(db).await;
    let ann = app.listener("ann").await;
    for uri in [
        "/api/admin/health",
        "/api/admin/backups",
        "/api/admin/releases",
        "/api/admin/settings/env",
    ] {
        assert_eq!(
            app.get(uri, &ann).await.status,
            StatusCode::FORBIDDEN,
            "{uri}"
        );
    }
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn health_releases_and_backups(db: PgPool) {
    let app = TestApp::new(db).await;
    let (_, admin) = app.admin("boss").await;

    let h = app.get("/api/admin/health", &admin).await;
    assert_eq!(h.status, StatusCode::OK, "{}", h.text);
    let state = |key: &str| {
        h.body["checks"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["key"] == key)
            .unwrap()["state"]
            .clone()
    };
    assert_eq!(state("database"), "ok");
    assert_eq!(state("storage"), "ok");
    assert_eq!(state("fetcher"), "off");
    assert!(h.body["dbBytes"].as_i64().unwrap() > 0);

    // Выпуск Mac 0.2.0 вышел; одно приложение на 0.1.0 — отстало, другое на 0.2.0
    sqlx::query(
        "INSERT INTO app_releases (target, arch, version, storage_key, size_bytes, signature)
         VALUES ('darwin', 'aarch64', '0.1.0', 'r/1', 10, 'sig'), ('darwin', 'aarch64', '0.2.0', 'r/2', 10, 'sig')",
    )
    .execute(&app.state.db)
    .await
    .unwrap();
    let boss: uuid::Uuid = app.user_id(&admin).await.parse().unwrap();
    sqlx::query(
        "INSERT INTO devices (user_id, name, platform, app_version, last_seen_at)
         VALUES ($1, 'Mac', 'macos', '0.1.0', now()), ($1, 'Mac 2', 'macos', '0.2.0', now())",
    )
    .bind(boss)
    .execute(&app.state.db)
    .await
    .unwrap();
    let r = app.get("/api/admin/releases", &admin).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text);
    assert_eq!(r.body["latest"][0]["version"], "0.2.0");
    let installs = r.body["installs"].as_array().unwrap();
    let old = installs.iter().find(|i| i["version"] == "0.1.0").unwrap();
    assert_eq!(old["outdated"], true);
    let new = installs.iter().find(|i| i["version"] == "0.2.0").unwrap();
    assert_eq!(new["outdated"], false);

    // Папка копий не задана — раздел честно говорит об этом, проверка — 409
    let b = app.get("/api/admin/backups", &admin).await;
    assert_eq!(b.status, StatusCode::OK);
    if std::env::var("DALYWAVE_BACKUP_DIR").is_err() {
        assert_eq!(b.body["configured"], false);
        let res = app
            .post("/api/admin/backups/check", json!({}), &admin)
            .await;
        assert_eq!(res.code(), "no_backups");
    }

    let env = app.get("/api/admin/settings/env", &admin).await;
    assert_eq!(env.status, StatusCode::OK);
    let items = env.body["items"].as_array().unwrap();
    // Секреты — без значения
    assert!(
        items
            .iter()
            .filter(|i| i["secret"] == true)
            .all(|i| i["value"].is_null())
    );
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn clip_settings_shape_the_jobs(db: PgPool) {
    let app = TestApp::new(db).await;
    no_intervals(&app.state.db).await;
    let (_, admin) = app.admin("boss").await;
    let (token, _) = new_downloader(&app, &admin, "Mac").await;
    let me = app.listener("ann").await;
    let track = app.resolve_id("track", LIGHT_ME_UP, &me).await;

    // По умолчанию — кусок 12 с в 480p, автопоиск включён
    let s = app.get("/api/admin/settings", &admin).await;
    assert_eq!(s.body["clips"]["height"], 480);
    assert_eq!(s.body["clips"]["whole"], false);
    assert_eq!(s.body["clips"]["autoSearch"], true);
    assert_eq!(s.body["clips"]["maxGb"], 3);

    let bad = app
        .call(
            Method::PATCH,
            "/api/admin/settings",
            Some(json!({ "clips": { "height": 500 } })),
            &admin,
        )
        .await;
    assert_eq!(bad.status, StatusCode::BAD_REQUEST);

    // Автопоиск выключен — лента клипов не заказывает
    let res = app
        .call(
            Method::PATCH,
            "/api/admin/settings",
            Some(json!({ "clips": { "autoSearch": false } })),
            &admin,
        )
        .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    app.post("/api/clips", json!({ "trackIds": [track] }), &me)
        .await;
    let queued: i64 = sqlx::query_scalar("SELECT count(*) FROM jobs WHERE kind = 'clip'")
        .fetch_one(&app.state.db)
        .await
        .unwrap();
    assert_eq!(queued, 0);

    // Целиком в 720p и в такт: новый загрузчик получает это в задании; звука трека нет — без ссылки
    let res = app
        .call(
            Method::PATCH,
            "/api/admin/settings",
            Some(json!({ "clips": { "autoSearch": true, "height": 720, "whole": true, "lengthS": 15 } })),
            &admin,
        )
        .await;
    assert_eq!(res.body["clips"]["height"], 720);
    app.post("/api/clips", json!({ "trackIds": [track] }), &me)
        .await;
    // Старый загрузчик (без clip-whole) режет кусок нужной длины
    let job = claim(&app, &token, json!(["clip-modes"])).await.body["job"].clone();
    assert_eq!(job["whole"], false);
    assert_eq!(job["height"], 720);
    assert_eq!(job["lengthMs"], 15_000);
    sqlx::query("UPDATE jobs SET status = 'queued', locked_by = NULL, run_after = now() WHERE kind = 'clip'")
        .execute(&app.state.db)
        .await
        .unwrap();
    let job = claim(&app, &token, json!(["clip-modes", "clip-whole"]))
        .await
        .body["job"]
        .clone();
    assert_eq!(job["whole"], true, "{job}");
    assert!(job["syncAudioUrl"].is_null());

    // Отчёт с сопоставлением — клип отдаётся ленте с ним
    let key = upload_key(job["upload"]["url"].as_str().unwrap());
    app.state
        .storage
        .put(&key, vec![0u8, 0, 0, 24], "video/mp4", None)
        .await
        .unwrap();
    let id = job["id"].as_str().unwrap();
    let res = app
        .post(
            &format!("/api/fetcher/clips/{id}/result"),
            json!({ "workerId": "test-mac", "result": {
                "status": "ready", "videoId": "dQw4w9WgXcQ", "videoDurationMs": 215_000,
                "clipStartMs": 0, "durationMs": 215_000, "sizeBytes": 4, "height": 720,
                "whole": true, "sync": { "offsetMs": -12_300, "rate": 1.02 } } }),
            &Auth::Bearer(token.clone()),
        )
        .await;
    assert_eq!(res.status, StatusCode::NO_CONTENT, "{}", res.text);
    let items = app
        .post("/api/clips", json!({ "trackIds": [track] }), &me)
        .await
        .body["items"]
        .clone();
    assert_eq!(items[0]["state"], "ready");
    assert_eq!(items[0]["sync"]["offsetMs"], -12_300);
    assert!((items[0]["sync"]["rate"].as_f64().unwrap() - 1.02).abs() < 1e-4);

    let s = app.get("/api/admin/settings", &admin).await;
    assert_eq!(s.body["clips"]["stored"], 1);
    assert!(
        app.audit_kinds()
            .await
            .iter()
            .any(|k| k == "settings.changed")
    );
}
