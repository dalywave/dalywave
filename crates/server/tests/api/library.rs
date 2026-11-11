use axum::http::{Method, StatusCode};
use dalywave_server::jobs::worker::Outcome;
use serde_json::{Value, json};
use sqlx::PgPool;

use crate::{
    common::{Auth, TestApp},
    deezer::{DEVIL_SINGLE, LIGHT_ME_UP_ALBUM, LOWBORN, MODERN_SADNESS, fixture},
};

/// Треки EP «Modern Sadness» (наши id) по порядку.
async fn ep_tracks(app: &TestApp, auth: &Auth) -> Vec<String> {
    let ep = app.resolve_id("album", MODERN_SADNESS, auth).await;
    let res = app.get(&format!("/api/albums/{ep}"), auth).await;
    res.body["tracks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["id"].as_str().unwrap().to_string())
        .collect()
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn likes_are_idempotent_and_paginated(db: PgPool) {
    let app = TestApp::new(db).await;
    let me = app.listener("ann").await;
    let tracks = ep_tracks(&app, &me).await;
    for id in &tracks {
        let res = app.put(&format!("/api/library/tracks/{id}"), &me).await;
        assert_eq!(res.status, StatusCode::NO_CONTENT, "{}", res.text);
    }
    // Повтор безопасен
    assert_eq!(
        app.put(&format!("/api/library/tracks/{}", tracks[0]), &me)
            .await
            .status,
        StatusCode::NO_CONTENT
    );
    let missing = uuid::Uuid::now_v7();
    assert_eq!(
        app.put(&format!("/api/library/tracks/{missing}"), &me)
            .await
            .status,
        StatusCode::NOT_FOUND
    );

    let page = app.get("/api/library/tracks?limit=2", &me).await;
    assert_eq!(page.status, StatusCode::OK, "{}", page.text);
    let items = page.body["items"].as_array().unwrap();
    assert_eq!(items.len(), 2);
    // Новые первыми
    assert_eq!(items[0]["track"]["id"], tracks[4].as_str());
    assert_eq!(items[0]["track"]["liked"], true);
    assert!(items[0]["likedAt"].is_string());
    let mut seen: Vec<String> = items
        .iter()
        .map(|i| i["track"]["id"].as_str().unwrap().to_string())
        .collect();
    let mut cursor = page.body["nextCursor"].as_str().map(str::to_string);
    while let Some(c) = cursor {
        let next = app
            .get(&format!("/api/library/tracks?limit=2&cursor={c}"), &me)
            .await;
        seen.extend(
            next.body["items"]
                .as_array()
                .unwrap()
                .iter()
                .map(|i| i["track"]["id"].as_str().unwrap().to_string()),
        );
        cursor = next.body["nextCursor"].as_str().map(str::to_string);
    }
    let mut expected = tracks.clone();
    expected.reverse();
    assert_eq!(seen, expected);

    assert_eq!(
        app.delete(&format!("/api/library/tracks/{}", tracks[0]), &me)
            .await
            .status,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        app.delete(&format!("/api/library/tracks/{}", tracks[0]), &me)
            .await
            .status,
        StatusCode::NO_CONTENT
    );
    let overview = app.get("/api/library", &me).await;
    assert_eq!(overview.status, StatusCode::OK);
    assert_eq!(overview.body["counts"]["likedTracks"], 4);
    assert_eq!(
        app.get("/api/library/tracks?cursor=junk", &me).await.status,
        StatusCode::BAD_REQUEST
    );

    // Лайки одного человека не видны другому
    let other = app.listener("bob").await;
    let res = app.get("/api/library/tracks", &other).await;
    assert!(res.body["items"].as_array().unwrap().is_empty());
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn saved_albums_and_blocked_items(db: PgPool) {
    let app = TestApp::new(db).await;
    let me = app.listener("ann").await;
    let ep = app.resolve_id("album", MODERN_SADNESS, &me).await;
    assert_eq!(
        app.put(&format!("/api/library/albums/{ep}"), &me)
            .await
            .status,
        StatusCode::NO_CONTENT
    );
    let res = app.get("/api/library/albums", &me).await;
    assert_eq!(res.body["items"][0]["album"]["id"], ep.as_str());
    assert_eq!(res.body["items"][0]["album"]["saved"], true);
    let album = app.get(&format!("/api/albums/{ep}"), &me).await;
    assert_eq!(album.body["album"]["saved"], true);
    assert_eq!(
        app.delete(&format!("/api/library/albums/{ep}"), &me)
            .await
            .status,
        StatusCode::NO_CONTENT
    );
    assert!(
        app.get("/api/library/albums", &me).await.body["items"]
            .as_array()
            .unwrap()
            .is_empty()
    );

    let artist = app.resolve_id("artist", LOWBORN, &me).await;
    let track = ep_tracks(&app, &me).await.remove(0);
    for uri in [
        format!("/api/library/blocked/artist/{artist}"),
        format!("/api/library/blocked/track/{track}"),
    ] {
        assert_eq!(
            app.put(&uri, &me).await.status,
            StatusCode::NO_CONTENT,
            "{uri}"
        );
    }
    let res = app.get("/api/library/blocked", &me).await;
    let items = res.body["items"].as_array().unwrap();
    assert_eq!(items.len(), 2);
    let kinds: Vec<&str> = items.iter().map(|i| i["kind"].as_str().unwrap()).collect();
    assert!(kinds.contains(&"artist") && kinds.contains(&"track"));
    let blocked_track = items.iter().find(|i| i["kind"] == "track").unwrap();
    assert_eq!(blocked_track["track"]["id"], track.as_str());
    assert!(blocked_track["artist"].is_null());
    assert_eq!(
        app.put(&format!("/api/library/blocked/album/{artist}"), &me)
            .await
            .status,
        StatusCode::BAD_REQUEST
    );
    let missing = uuid::Uuid::now_v7();
    assert_eq!(
        app.put(&format!("/api/library/blocked/track/{missing}"), &me)
            .await
            .status,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        app.delete(&format!("/api/library/blocked/track/{track}"), &me)
            .await
            .status,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        app.get("/api/library/blocked", &me).await.body["items"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
}

/// Список релизов Lowborn для подписки: три настоящих и два, которые политика по умолчанию пропускает.
fn small_discography() -> Value {
    let real = fixture(&format!("artist_{LOWBORN}_albums"));
    let mut data: Vec<Value> = real["data"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|a| {
            [MODERN_SADNESS, DEVIL_SINGLE, LIGHT_ME_UP_ALBUM].contains(&a["id"].as_i64().unwrap())
        })
        .cloned()
        .collect();
    data.push(
        json!({ "id": 900000001, "title": "Best of Lowborn", "record_type": "compile",
                      "release_date": "2022-01-01", "md5_image": "", "type": "album" }),
    );
    data.push(
        json!({ "id": 900000002, "title": "Live at Home", "record_type": "ep",
                      "release_date": "2022-02-02", "md5_image": "", "type": "album" }),
    );
    json!({ "data": data, "total": data.len() })
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn follow_syncs_discography_and_queues_acquisitions(db: PgPool) {
    let app = TestApp::new(db).await;
    let me = app.listener("ann").await;
    app.deezer
        .set(&format!("/artist/{LOWBORN}/albums"), small_discography());
    let artist = app.resolve_id("artist", LOWBORN, &me).await;

    // Подписка без тела — политика по умолчанию
    let res = app
        .call(
            Method::PUT,
            &format!("/api/library/artists/{artist}"),
            None,
            &me,
        )
        .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    assert_eq!(res.body["artist"]["followed"], true);
    assert_eq!(
        res.body["policy"],
        json!({ "albums": true, "eps": true, "singles": true, "compilations": false, "live": false, "appearsOn": false })
    );
    assert_eq!(res.body["notify"], true);
    let queued: Vec<String> = sqlx::query_scalar(
        "SELECT kind FROM jobs WHERE status = 'queued' AND lane = 'maintenance'",
    )
    .fetch_all(&app.state.db)
    .await
    .unwrap();
    assert_eq!(queued, vec!["sync_discography"]);
    // Та же политика ещё раз — новой сверки нет
    app.call(
        Method::PUT,
        &format!("/api/library/artists/{artist}"),
        Some(json!({})),
        &me,
    )
    .await;
    let jobs: i64 = sqlx::query_scalar("SELECT count(*) FROM jobs")
        .fetch_one(&app.state.db)
        .await
        .unwrap();
    assert_eq!(jobs, 1);

    let (kind, outcome) = app.run_job("maintenance").await.unwrap();
    assert_eq!(kind, "sync_discography");
    assert_eq!(outcome, Outcome::Done);
    let result: Value =
        sqlx::query_scalar("SELECT result FROM jobs WHERE kind = 'sync_discography'")
            .fetch_one(&app.state.db)
            .await
            .unwrap();
    assert_eq!(result["releases"], 5);
    assert_eq!(result["matched"], 3, "сборник и концертный пропущены");
    assert_eq!(result["albumsLoaded"], 3);
    // EP — 5 записей, сингл «Devil» — те же две, «Light Me Up» — одна
    assert_eq!(result["queued"], 6);
    assert_eq!(app.deezer.hits("/album/900000001"), 0);

    let acquire: Vec<Value> = sqlx::query_scalar(
        "SELECT payload FROM jobs WHERE kind = 'acquire' AND lane = 'discography' AND status = 'queued' ORDER BY id",
    )
    .fetch_all(&app.state.db)
    .await
    .unwrap();
    assert_eq!(acquire.len(), 6);
    for p in &acquire {
        for field in ["acquisitionId", "recordingId", "trackId"] {
            assert!(p[field].is_string(), "{p}");
        }
        assert!(p["deezerTrackId"].as_i64().unwrap() > 0);
        assert_eq!(p["preferVideoIds"], json!([]));
    }
    let statuses: Vec<String> = sqlx::query_scalar(
        "SELECT DISTINCT audio_status FROM recordings r JOIN acquisitions a ON a.recording_id = r.id",
    )
    .fetch_all(&app.state.db)
    .await
    .unwrap();
    assert_eq!(statuses, vec!["queued"]);
    let reasons: Vec<String> = sqlx::query_scalar("SELECT DISTINCT reason FROM acquisitions")
        .fetch_all(&app.state.db)
        .await
        .unwrap();
    assert_eq!(reasons, vec!["subscription"]);

    // Прогресс подписки: 0 из 8 на сервере — 6 записей загруженных релизов и по одному треку
    // на два незагруженных (политика их пропускает, а Deezer в списке не говорит, сколько там треков)
    let res = app.get("/api/library/artists", &me).await;
    assert_eq!(res.body["items"][0]["tracksOnServer"], 0);
    assert_eq!(res.body["items"][0]["tracksTotal"], 8);
    let page = app.get(&format!("/api/artists/{artist}"), &me).await;
    assert_eq!(page.body["follow"]["policy"]["eps"], true);
    assert_eq!(page.body["tracksTotal"], 8);
    // Одна запись на сервере — прогресс растёт
    sqlx::query(
        "UPDATE recordings SET audio_status = 'ready' WHERE id = (SELECT recording_id FROM acquisitions ORDER BY id DESC LIMIT 1)",
    )
    .execute(&app.state.db)
    .await
    .unwrap();
    let res = app.get("/api/library/artists", &me).await;
    assert_eq!(res.body["items"][0]["tracksOnServer"], 1);

    // Повторная сверка ничего не дублирует
    let mut conn = app.state.db.acquire().await.unwrap();
    dalywave_server::jobs::enqueue(
        &mut conn,
        dalywave_server::jobs::NewJob::new(
            "sync_discography",
            "maintenance",
            json!({ "artistId": artist }),
        ),
    )
    .await
    .unwrap();
    app.run_job("maintenance").await.unwrap();
    let total: i64 = sqlx::query_scalar("SELECT count(*) FROM jobs WHERE kind = 'acquire'")
        .fetch_one(&app.state.db)
        .await
        .unwrap();
    assert_eq!(total, 6);

    // Скачивание — дело загрузчика на Mac: серверный fetcher выключен, и воркер задачи acquire не берёт
    assert!(app.run_job("discography").await.is_none());
    let queued: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM jobs WHERE kind = 'acquire' AND status = 'queued'",
    )
    .fetch_one(&app.state.db)
    .await
    .unwrap();
    assert_eq!(queued, 6);

    // Отписка: скачанное и очередь остаются
    assert_eq!(
        app.delete(&format!("/api/library/artists/{artist}"), &me)
            .await
            .status,
        StatusCode::NO_CONTENT
    );
    assert!(
        app.get("/api/library/artists", &me).await.body["items"]
            .as_array()
            .unwrap()
            .is_empty()
    );
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn policy_change_resyncs(db: PgPool) {
    let app = TestApp::new(db).await;
    let me = app.listener("ann").await;
    app.deezer
        .set(&format!("/artist/{LOWBORN}/albums"), small_discography());
    let artist = app.resolve_id("artist", LOWBORN, &me).await;
    let only_eps = json!({ "downloadPolicy": { "albums": false, "eps": true, "singles": false,
                           "compilations": false, "live": false, "appearsOn": false }, "notify": false });
    let res = app
        .call(
            Method::PUT,
            &format!("/api/library/artists/{artist}"),
            Some(only_eps),
            &me,
        )
        .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    assert_eq!(res.body["policy"]["singles"], false);
    assert_eq!(res.body["notify"], false);
    app.run_job("maintenance").await.unwrap();
    let queued: i64 = sqlx::query_scalar("SELECT count(*) FROM jobs WHERE kind = 'acquire'")
        .fetch_one(&app.state.db)
        .await
        .unwrap();
    assert_eq!(queued, 5, "только EP");

    // Новая политика — новая сверка: добавляются синглы (их записи частично уже в очереди)
    let all = json!({ "downloadPolicy": { "albums": true, "eps": true, "singles": true,
                      "compilations": false, "live": false, "appearsOn": false } });
    let res = app
        .call(
            Method::PUT,
            &format!("/api/library/artists/{artist}"),
            Some(all),
            &me,
        )
        .await;
    assert_eq!(
        res.body["notify"], false,
        "notify не передан — остаётся прежним"
    );
    app.run_job("maintenance").await.unwrap();
    let queued: i64 = sqlx::query_scalar("SELECT count(*) FROM jobs WHERE kind = 'acquire'")
        .fetch_one(&app.state.db)
        .await
        .unwrap();
    assert_eq!(queued, 6);
    let res = app
        .call(
            Method::PUT,
            &format!("/api/library/artists/{artist}"),
            Some(json!({ "downloadPolicy": { "albums": true } })),
            &me,
        )
        .await;
    assert_eq!(
        res.status,
        StatusCode::BAD_REQUEST,
        "политика передаётся целиком"
    );
}
