//! Разделы админки «Каталог», «Хранилище», «Импорт», «Настройки», «Источники» и «Журнал»:
//! только для администратора, фильтры, перекачка записи, политика подписки, окно работы полосы.

use axum::http::{Method, StatusCode};
use chrono::{Timelike, Utc};
use dalywave_server::jobs::{self, Claim};
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    common::TestApp,
    synth::{Feat, Synth},
};

/// Файл записи: основной, с нужным источником и проверкой.
async fn give_file(db: &PgPool, track: Uuid, size: i64, verified: bool) -> Uuid {
    let recording: Uuid = sqlx::query_scalar("SELECT recording_id FROM tracks WHERE id = $1")
        .bind(track)
        .fetch_one(db)
        .await
        .unwrap();
    let file: Uuid = sqlx::query_scalar(
        "INSERT INTO audio_files (recording_id, storage_key, codec, container, duration_ms, size_bytes, source,
                                  bitrate_kbps, loudness_lufs, match_score, verified)
         VALUES ($1, 'audio/' || $1::text || '.m4a', 'opus', 'mp4', 180000, $2, 'youtube_music', 160, -9.5, 0.93, $3)
         RETURNING id",
    )
    .bind(recording)
    .bind(size)
    .bind(verified)
    .fetch_one(db)
    .await
    .unwrap();
    sqlx::query("UPDATE recordings SET primary_file_id = $2, audio_status = 'ready' WHERE id = $1")
        .bind(recording)
        .bind(file)
        .execute(db)
        .await
        .unwrap();
    recording
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn sections_are_admin_only(db: PgPool) {
    let app = TestApp::new(db).await;
    let listener = app.listener("ann").await;
    for uri in [
        "/api/admin/recordings",
        "/api/admin/storage",
        "/api/admin/imports",
        "/api/admin/settings",
        "/api/admin/sources",
    ] {
        let res = app.get(uri, &listener).await;
        assert_eq!(res.status, StatusCode::FORBIDDEN, "{uri}: {}", res.text);
    }
    let res = app
        .post("/api/admin/sources/check", json!({}), &listener)
        .await;
    assert_eq!(res.status, StatusCode::FORBIDDEN);
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn catalog_lists_files_and_reacquires(db: PgPool) {
    let app = TestApp::new(db.clone()).await;
    let (_, admin) = app.admin("boss").await;
    let s = Synth::new(db.clone());
    let moby = s.artist("Moby").await;
    let porcelain = s.track(moby, "Porcelain", Feat::default()).await;
    let natural = s.track(moby, "Natural Blues", Feat::default()).await;
    let _catalog_only = s.track(moby, "Extreme Ways", Feat::default()).await;
    let rec = give_file(&db, porcelain, 4_000_000, true).await;
    give_file(&db, natural, 3_000_000, false).await;

    // По умолчанию — только записи с файлом или запросом на скачивание
    let page = app.get("/api/admin/recordings", &admin).await;
    assert_eq!(page.status, StatusCode::OK, "{}", page.text);
    let items = page.body["items"].as_array().unwrap();
    assert_eq!(items.len(), 2);
    let first = items.iter().find(|i| i["id"] == rec.to_string()).unwrap();
    assert_eq!(first["track"]["title"], "Porcelain");
    assert_eq!(first["file"]["codec"], "opus");
    assert_eq!(first["file"]["bitrateKbps"], 160);
    assert_eq!(first["file"]["verified"], true);
    assert_eq!(first["files"], 1);
    assert_eq!(first["tracks"], 1);

    let found = app.get("/api/admin/recordings?q=porc", &admin).await;
    assert_eq!(found.body["items"].as_array().unwrap().len(), 1);
    let by_artist = app.get("/api/admin/recordings?q=moby", &admin).await;
    assert_eq!(by_artist.body["items"].as_array().unwrap().len(), 2);
    let unverified = app
        .get("/api/admin/recordings?verified=false", &admin)
        .await;
    assert_eq!(
        unverified.body["items"][0]["track"]["title"],
        "Natural Blues"
    );
    let none = app.get("/api/admin/recordings?status=none", &admin).await;
    assert_eq!(none.body["items"][0]["track"]["title"], "Extreme Ways");
    let paged = app.get("/api/admin/recordings?limit=1", &admin).await;
    let cursor = paged.body["nextCursor"].as_str().unwrap().to_string();
    let next = app
        .get(
            &format!("/api/admin/recordings?limit=1&cursor={cursor}"),
            &admin,
        )
        .await;
    assert_eq!(next.body["items"].as_array().unwrap().len(), 1);
    assert_ne!(next.body["items"][0]["id"], paged.body["items"][0]["id"]);
    assert_eq!(
        app.get("/api/admin/recordings?source=ftp", &admin)
            .await
            .status,
        StatusCode::BAD_REQUEST
    );

    // Хранилище: звук по кодекам, самый тяжёлый артист
    let storage = app.get("/api/admin/storage", &admin).await;
    assert_eq!(storage.status, StatusCode::OK, "{}", storage.text);
    assert_eq!(storage.body["kinds"][0]["key"], "audio");
    assert_eq!(storage.body["kinds"][0]["bytes"], 7_000_000);
    assert_eq!(storage.body["codecs"][0]["key"], "opus");
    assert_eq!(storage.body["artists"][0]["artist"]["name"], "Moby");
    assert_eq!(storage.body["artists"][0]["files"], 2);
    assert_eq!(storage.body["growth"].as_array().unwrap().len(), 30);
    assert_eq!(storage.body["replacedFiles"], 0);

    // Перекачка по ссылке: выбор человека, вне очереди; второй раз — уже идёт
    let res = app
        .post(
            &format!("/api/admin/recordings/{rec}/reacquire"),
            json!({ "url": "https://youtu.be/afkL3uLNeXA" }),
            &admin,
        )
        .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    assert_eq!(res.body["acquisition"]["status"], "queued");
    assert_eq!(res.body["acquisition"]["lane"], "now");
    assert_eq!(res.body["preferVideoIds"], json!(["afkL3uLNeXA"]));
    let again = app
        .post(
            &format!("/api/admin/recordings/{rec}/reacquire"),
            json!({}),
            &admin,
        )
        .await;
    assert_eq!(again.status, StatusCode::CONFLICT);
    assert_eq!(again.code(), "acquisition_active");
    let active = app.get("/api/admin/acquisitions?active=true", &admin).await;
    assert_eq!(active.body["items"].as_array().unwrap().len(), 1);
    let finished = app
        .get("/api/admin/acquisitions?active=false", &admin)
        .await;
    assert_eq!(finished.body["items"], json!([]));
    let one = app
        .get(
            &format!(
                "/api/admin/acquisitions/{}",
                res.body["acquisition"]["id"].as_str().unwrap()
            ),
            &admin,
        )
        .await;
    assert_eq!(one.status, StatusCode::OK);
    assert_eq!(one.body["track"]["title"], "Porcelain");
    assert_eq!(
        app.get(
            &format!("/api/admin/acquisitions/{}", Uuid::now_v7()),
            &admin
        )
        .await
        .status,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        app.post(
            &format!("/api/admin/recordings/{}/reacquire", Uuid::now_v7()),
            json!({}),
            &admin
        )
        .await
        .status,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        app.get(
            &format!("/api/admin/tracks/{}/preview", Uuid::now_v7()),
            &admin
        )
        .await
        .status,
        StatusCode::NOT_FOUND
    );
    assert!(
        app.audit_kinds()
            .await
            .contains(&"recording.reacquire".to_string())
    );

    let imports = app.get("/api/admin/imports", &admin).await;
    assert_eq!(imports.status, StatusCode::OK);
    assert_eq!(imports.body["items"], json!([]));
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn settings_give_new_follows_their_policy(db: PgPool) {
    let app = TestApp::new(db.clone()).await;
    let (_, admin) = app.admin("boss").await;
    let s = Synth::new(db.clone());
    let moby = s.artist("Moby").await;

    let settings = app.get("/api/admin/settings", &admin).await;
    assert_eq!(settings.status, StatusCode::OK, "{}", settings.text);
    assert_eq!(settings.body["followPolicy"]["albums"], true);
    assert_eq!(settings.body["followPolicy"]["live"], false);
    assert_eq!(settings.body["utcOffsetMin"], 180);

    let policy = json!({ "albums": true, "eps": false, "singles": false, "compilations": false,
                         "live": true, "appearsOn": false });
    let res = app
        .call(
            Method::PATCH,
            "/api/admin/settings",
            Some(json!({ "followPolicy": policy, "utcOffsetMin": 120 })),
            &admin,
        )
        .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    assert_eq!(res.body["followPolicy"], policy);
    assert_eq!(res.body["utcOffsetMin"], 120);
    let bad = app
        .call(
            Method::PATCH,
            "/api/admin/settings",
            Some(json!({ "utcOffsetMin": 9000 })),
            &admin,
        )
        .await;
    assert_eq!(bad.status, StatusCode::BAD_REQUEST);

    // Подписка без своей политики берёт политику из настроек
    let res = app
        .call(
            Method::PUT,
            &format!("/api/library/artists/{moby}"),
            Some(json!({})),
            &admin,
        )
        .await;
    assert!(res.status.is_success(), "{}", res.text);
    let stored: serde_json::Value =
        sqlx::query_scalar("SELECT download_policy FROM followed_artists WHERE artist_id = $1")
            .bind(moby)
            .fetch_one(&db)
            .await
            .unwrap();
    assert_eq!(stored, policy);
    assert!(
        app.audit_kinds()
            .await
            .contains(&"settings.changed".to_string())
    );
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn lane_window_closes_the_lane(db: PgPool) {
    let app = TestApp::new(db.clone()).await;
    let (_, admin) = app.admin("boss").await;
    let now = Utc::now();
    // Часовой пояс по умолчанию — UTC+3
    let minute = ((now.hour() * 60 + now.minute() + 180) % 1440) as i64;
    let at = |shift: i64| (minute + shift).rem_euclid(1440);
    let patch = |schedule: serde_json::Value| {
        let app = &app;
        let admin = &admin;
        async move {
            app.call(
                Method::PATCH,
                "/api/admin/lanes/discovery",
                Some(json!({ "schedule": schedule })),
                admin,
            )
            .await
        }
    };

    let closed = patch(json!({ "from": at(60), "until": at(120) })).await;
    assert_eq!(closed.status, StatusCode::OK, "{}", closed.text);
    assert_eq!(closed.body["activeFrom"], at(60));
    assert_eq!(closed.body["open"], false);
    assert!(matches!(
        jobs::claim(&db, "discovery", "test").await.unwrap(),
        Claim::Paused
    ));

    let open = patch(json!({ "from": at(-10), "until": at(10) })).await;
    assert_eq!(open.body["open"], true);
    assert!(!matches!(
        jobs::claim(&db, "discovery", "test").await.unwrap(),
        Claim::Paused
    ));

    let always = patch(json!({ "from": null, "until": null })).await;
    assert!(always.body["activeFrom"].is_null());
    assert_eq!(always.body["open"], true);

    for wrong in [
        json!({ "from": 60 }),
        json!({ "from": 60, "until": 60 }),
        json!({ "from": 1440, "until": 60 }),
    ] {
        assert_eq!(patch(wrong).await.status, StatusCode::BAD_REQUEST);
    }

    // Журнал с фильтрами: группа событий и человек
    let lanes = app.get("/api/admin/audit?kind=lane.", &admin).await;
    let items = lanes.body["items"].as_array().unwrap();
    assert_eq!(items.len(), 3);
    assert!(items.iter().all(|e| e["kind"] == "lane.changed"));
    let exact = app.get("/api/admin/audit?kind=auth.sign_in", &admin).await;
    assert!(
        exact.body["items"]
            .as_array()
            .unwrap()
            .iter()
            .all(|e| e["kind"] == "auth.sign_in")
    );
    let nobody = app
        .get(
            &format!("/api/admin/audit?actor={}", Uuid::now_v7()),
            &admin,
        )
        .await;
    assert_eq!(nobody.body["items"], json!([]));
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn sources_count_days_and_check_downloads(db: PgPool) {
    let app = TestApp::new(db.clone()).await;
    let (admin_id, admin) = app.admin("boss").await;
    let sources = app.get("/api/admin/sources", &admin).await;
    assert_eq!(sources.status, StatusCode::OK, "{}", sources.text);
    let daily = sources.body["daily"].as_array().unwrap();
    assert_eq!(daily.len(), 14);
    assert_eq!(daily[13]["ok"], 0);

    // Скачивать нечего
    let res = app
        .post("/api/admin/sources/check", json!({}), &admin)
        .await;
    assert_eq!(res.status, StatusCode::CONFLICT, "{}", res.text);
    assert_eq!(res.code(), "nothing_to_check");

    // Свежий лайк без файла — его и проверяем
    let s = Synth::new(db.clone());
    let moby = s.artist("Moby").await;
    let track = s.track(moby, "Porcelain", Feat::default()).await;
    s.like(admin_id.parse().unwrap(), track, 0).await;
    let res = app
        .post("/api/admin/sources/check", json!({}), &admin)
        .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    assert_eq!(res.body["track"]["title"], "Porcelain");
    assert_eq!(res.body["acquisition"]["lane"], "now");
    assert!(
        app.audit_kinds()
            .await
            .contains(&"source.check".to_string())
    );
    let missing = app
        .post(
            "/api/admin/sources/check",
            json!({ "trackId": Uuid::now_v7() }),
            &admin,
        )
        .await;
    assert_eq!(missing.status, StatusCode::NOT_FOUND);
}
