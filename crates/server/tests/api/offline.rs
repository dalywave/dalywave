//! Умный офлайн: закреплённое на устройстве, набор волны устройства и ссылки на файлы — и всё это
//! в синхронизации приложения.

use std::collections::HashMap;

use axum::http::StatusCode;
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    common::{Auth, TestApp},
    deezer::MODERN_SADNESS,
    fetcher::ep_tracks,
    mock::Person,
};

const VERIFIER: &str = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk-offline";

/// Слушатель вошёл в приложение: cookie сайта и токен устройства того же человека.
async fn app_listener(app: &TestApp, sub: &str) -> (Auth, Auth) {
    let web = app.listener(sub).await;
    let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(VERIFIER.as_bytes()));
    let (authorize, cookie) = app
        .start(&format!(
            "/api/auth/app/start?redirect_uri=dalywave%3A%2F%2Fauth&code_challenge={challenge}\
             &code_challenge_method=S256&state=s1&device_name=iPhone&platform=ios"
        ))
        .await;
    let (code, state) = app.provider.approve(&authorize, &Person::new(sub));
    let res = app.callback(&code, &state, Some(&cookie)).await;
    let back = res.location();
    let q: HashMap<String, String> = url::Url::parse(&back)
        .unwrap()
        .query_pairs()
        .into_owned()
        .collect();
    let res = app
        .post(
            "/api/auth/app/token",
            json!({ "code": q["code"], "codeVerifier": VERIFIER, "appVersion": "0.1.0" }),
            &Auth::None,
        )
        .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    (
        web,
        Auth::Bearer(res.body["token"].as_str().unwrap().to_string()),
    )
}

/// Запись трека готова: файл на сервере с размером и хешем.
async fn make_ready(app: &TestApp, track: Uuid, size: i64) {
    let db = &app.state.db;
    let rec: Uuid = sqlx::query_scalar("SELECT recording_id FROM tracks WHERE id = $1")
        .bind(track)
        .fetch_one(db)
        .await
        .unwrap();
    let file: Uuid = sqlx::query_scalar(
        "INSERT INTO audio_files (recording_id, storage_key, codec, container, duration_ms, size_bytes, source,
                                  loudness_lufs, sha256)
         VALUES ($1, $2, 'opus', 'mp4', 180000, $3, 'youtube', -9.5, $4) RETURNING id",
    )
    .bind(rec)
    .bind(format!("audio/r/{rec}.m4a"))
    .bind(size)
    .bind("ab".repeat(32))
    .fetch_one(db)
    .await
    .unwrap();
    sqlx::query("UPDATE recordings SET audio_status = 'ready', primary_file_id = $2 WHERE id = $1")
        .bind(rec)
        .bind(file)
        .execute(db)
        .await
        .unwrap();
}

/// Вся синхронизация с начала (все страницы).
async fn sync_all(app: &TestApp, auth: &Auth, since: Option<&str>) -> (Vec<Value>, String) {
    let mut pages = Vec::new();
    let mut cursor = since.map(str::to_string);
    loop {
        let uri = match &cursor {
            Some(c) => format!("/api/sync?since={c}"),
            None => "/api/sync".into(),
        };
        let res = app.get(&uri, auth).await;
        assert_eq!(res.status, StatusCode::OK, "{}", res.text);
        cursor = Some(res.body["cursor"].as_str().unwrap().to_string());
        let more = res.body["hasMore"].as_bool().unwrap();
        pages.push(res.body);
        if !more {
            break;
        }
    }
    (pages, cursor.unwrap())
}

fn collect(pages: &[Value], key: &str) -> Vec<Value> {
    pages
        .iter()
        .flat_map(|p| p[key].as_array().cloned().unwrap_or_default())
        .collect()
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn pinned_album_reaches_the_device_with_files(db: PgPool) {
    let app = TestApp::new(db).await;
    let (web, device) = app_listener(&app, "ann").await;
    let tracks = ep_tracks(&app, &web).await;
    let ep = app.resolve_id("album", MODERN_SADNESS, &web).await;

    // Сайт файлов не хранит
    let res = app
        .post(
            "/api/devices/current/downloads",
            json!({ "kind": "album", "refId": ep }),
            &web,
        )
        .await;
    assert_eq!(res.status, StatusCode::BAD_REQUEST);

    let res = app
        .post(
            "/api/devices/current/downloads",
            json!({ "kind": "album", "refId": ep }),
            &device,
        )
        .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    assert_eq!(res.body["tracksTotal"], tracks.len());
    assert_eq!(res.body["tracksReady"], 0);
    let pin = res.body["id"].as_str().unwrap().to_string();
    // Повтор — то же закрепление; недостающее на сервере уже в очереди скачивания
    let again = app
        .post(
            "/api/devices/current/downloads",
            json!({ "kind": "album", "refId": ep }),
            &device,
        )
        .await;
    assert_eq!(again.body["id"], pin.as_str());
    let queued: i64 = sqlx::query_scalar("SELECT count(*) FROM acquisitions")
        .fetch_one(&app.state.db)
        .await
        .unwrap();
    assert_eq!(queued, tracks.len() as i64);
    let missing = app
        .post(
            "/api/devices/current/downloads",
            json!({ "kind": "playlist", "refId": Uuid::now_v7() }),
            &device,
        )
        .await;
    assert_eq!(missing.status, StatusCode::NOT_FOUND);

    // Синхронизация приносит закрепление с треками альбома
    let (pages, cursor) = sync_all(&app, &device, None).await;
    let downloads = collect(&pages, "deviceDownloads");
    assert_eq!(downloads.len(), 1);
    assert_eq!(downloads[0]["kind"], "album");
    let synced: Vec<String> = collect(&pages, "tracks")
        .iter()
        .map(|t| t["id"].as_str().unwrap().to_string())
        .collect();
    for t in &tracks {
        assert!(
            synced.contains(&t.to_string()),
            "трек альбома в базе приложения"
        );
    }

    // Ссылки: готовое — с размером и хешем, остальное ждёт
    make_ready(&app, tracks[0], 4_000_000).await;
    let res = app
        .post(
            "/api/devices/current/files",
            json!({ "trackIds": tracks }),
            &device,
        )
        .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    let items = res.body["items"].as_array().unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["trackId"], tracks[0].to_string());
    assert_eq!(items[0]["sizeBytes"], 4_000_000);
    assert_eq!(items[0]["sha256"], "ab".repeat(32));
    assert_eq!(items[0]["container"], "mp4");
    assert!(items[0]["url"].as_str().unwrap().starts_with("http"));
    assert_eq!(
        res.body["pending"].as_array().unwrap().len(),
        tracks.len() - 1
    );

    // Открепили — tombstone уходит в приложение
    let res = app
        .delete(&format!("/api/devices/current/downloads/{pin}"), &device)
        .await;
    assert_eq!(res.status, StatusCode::NO_CONTENT);
    let (pages, _) = sync_all(&app, &device, Some(&cursor)).await;
    let gone = collect(&pages, "tombstones");
    assert!(
        gone.iter()
            .any(|t| t["entity"] == "deviceDownloads" && t["key"]["id"] == pin.as_str()),
        "{gone:?}"
    );
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn device_set_mixes_favorites_and_replaces_played(db: PgPool) {
    let app = TestApp::new(db).await;
    let (web, device) = app_listener(&app, "bob").await;
    let tracks = ep_tracks(&app, &web).await;
    for t in &tracks[..3] {
        make_ready(&app, *t, 3_000_000).await;
        assert_eq!(
            app.put(&format!("/api/library/tracks/{t}"), &web)
                .await
                .status,
            StatusCode::NO_CONTENT
        );
    }
    // Признаки звучания — с набором в приложение (волна без сети)
    sqlx::query(
        "INSERT INTO track_analysis (recording_id, source, models, energy, valence, bpm)
         SELECT recording_id, 'full', '{}', 0.8, 0.3, 128 FROM tracks WHERE id = $1",
    )
    .bind(tracks[0])
    .execute(&app.state.db)
    .await
    .unwrap();

    let res = app
        .post(
            "/api/devices/current/set",
            json!({ "favorites": 2, "fresh": 0 }),
            &device,
        )
        .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    assert_eq!(res.body["favorites"], 2);
    let (pages, cursor) = sync_all(&app, &device, None).await;
    let set = collect(&pages, "deviceSets");
    assert_eq!(set.len(), 2);
    assert!(set.iter().all(|s| s["kind"] == "favorite"));
    // Сайт набора не видит
    let (web_pages, _) = sync_all(&app, &web, None).await;
    assert!(collect(&web_pages, "deviceSets").is_empty());
    let features = collect(&pages, "trackFeatures");
    assert!(
        features
            .iter()
            .any(|f| f["energy"].as_f64().is_some_and(|e| (e - 0.8).abs() < 1e-6)),
        "{features:?}"
    );

    // Набор меньше — лишнее уходит tombstone'ом
    let res = app
        .post(
            "/api/devices/current/set",
            json!({ "favorites": 1, "fresh": 0 }),
            &device,
        )
        .await;
    assert_eq!(res.body["removed"], 1);
    let (pages, _) = sync_all(&app, &device, Some(&cursor)).await;
    let gone: Vec<Value> = collect(&pages, "tombstones")
        .into_iter()
        .filter(|t| t["entity"] == "deviceSets")
        .collect();
    assert_eq!(gone.len(), 1);
    assert!(gone[0]["key"]["deviceId"].is_string());
    assert!(gone[0]["key"]["trackId"].is_string());

    // Новое подбирается правилами волны — из готового и неслышанного; лайки туда не попадают
    let res = app
        .post(
            "/api/devices/current/set",
            json!({ "favorites": 1, "fresh": 5 }),
            &device,
        )
        .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    let fresh: Vec<Uuid> =
        sqlx::query_scalar("SELECT track_id FROM device_sets WHERE kind = 'fresh'")
            .fetch_all(&app.state.db)
            .await
            .unwrap();
    assert!(fresh.iter().all(|t| !tracks[..3].contains(t)));
}
