//! Большой экран: вход на телевизоре кодом с экрана (выдача, подтверждение, срок, одноразовость, отзыв),
//! режим вечеринки (настройки, QR «Слушать вместе», волна) и копия AAC для AVPlayer.

use axum::http::{Method, StatusCode};
use dalywave_server::renditions;
use serde_json::{Value, json};
use sqlx::PgPool;

use crate::{
    common::{Auth, TestApp},
    fetcher::ep_tracks,
    quality::{add_file, ffmpeg_available, make_primary, opus_bytes, recording_of, transcode_jobs},
};

async fn new_code(app: &TestApp, platform: &str) -> Value {
    let res = app
        .post(
            "/api/auth/tv/code",
            json!({ "platform": platform }),
            &Auth::None,
        )
        .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    res.body
}

async fn poll(app: &TestApp, code: &Value) -> crate::common::Resp {
    app.post(
        "/api/auth/tv/token",
        json!({ "deviceCode": code["deviceCode"], "appVersion": "0.1.5" }),
        &Auth::None,
    )
    .await
}

async fn approve(app: &TestApp, code: &str, auth: &Auth) -> crate::common::Resp {
    app.post("/api/auth/tv/approve", json!({ "userCode": code }), auth)
        .await
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn tv_signs_in_with_a_code_confirmed_on_the_phone(db: PgPool) {
    let app = TestApp::new(db).await;
    let (_, me) = app.admin("ann").await;

    let code = new_code(&app, "tvos").await;
    let user_code = code["userCode"].as_str().unwrap().to_string();
    assert_eq!(user_code.len(), 9, "8 знаков и дефис: {user_code}");
    assert!(
        code["verificationUrlComplete"]
            .as_str()
            .unwrap()
            .ends_with(&format!("/tv?code={}", user_code.replace('-', "")))
    );
    assert_eq!(code["intervalSecs"], 2);

    // Пока не подтвердили — ждём
    let res = poll(&app, &code).await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    assert_eq!(res.body["status"], "pending");

    // Подтверждает только вошедший
    assert_eq!(
        app.get(&format!("/api/auth/tv/{user_code}"), &Auth::None)
            .await
            .status,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        approve(&app, &user_code, &Auth::None).await.status,
        StatusCode::UNAUTHORIZED
    );
    // Что подтверждаем: код без дефиса и в нижнем регистре тоже годится
    let typed = user_code.replace('-', " ").to_lowercase();
    let info = app
        .get(&format!("/api/auth/tv/{}", typed.replace(' ', "")), &me)
        .await;
    assert_eq!(info.status, StatusCode::OK, "{}", info.text);
    assert_eq!(
        (
            info.body["platform"].as_str(),
            info.body["deviceName"].as_str()
        ),
        (Some("tvos"), Some("Apple TV"))
    );
    let wrong = approve(&app, "AAAA-AAAA", &me).await;
    assert_eq!(wrong.status, StatusCode::NOT_FOUND);
    assert_eq!(wrong.code(), "tv_code_unknown");
    let res = approve(&app, &typed, &me).await;
    assert_eq!(res.status, StatusCode::NO_CONTENT, "{}", res.text);
    // Второй раз тот же код не подтвердить
    assert_eq!(
        approve(&app, &user_code, &me).await.status,
        StatusCode::NOT_FOUND
    );

    // Телевизор получает своё устройство и токен — один раз
    let res = poll(&app, &code).await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    assert_eq!(res.body["status"], "ready");
    let token = res.body["token"].as_str().unwrap().to_string();
    assert!(token.starts_with("dwd_"));
    assert_eq!(res.body["device"]["platform"], "tvos");
    assert_eq!(res.body["device"]["appVersion"], "0.1.5");
    let again = poll(&app, &code).await;
    assert_eq!(again.status, StatusCode::BAD_REQUEST);
    assert_eq!(again.code(), "invalid_grant");

    let tv = Auth::Bearer(token);
    let me_tv = app.get("/api/auth/me", &tv).await;
    assert_eq!(me_tv.status, StatusCode::OK, "{}", me_tv.text);
    assert_eq!(me_tv.body["device"]["name"], "Apple TV");
    assert!(app.audit_kinds().await.contains(&"auth.sign_in".into()));

    // В «Доступе» — отдельная строка; отзыв выключает телевизор
    let list = app.get("/api/admin/sessions", &me).await;
    let row = list.body["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["device"]["platform"] == "tvos")
        .expect("телевизор в «Доступе»")
        .clone();
    let res = app
        .delete(
            &format!("/api/admin/sessions/{}", row["id"].as_str().unwrap()),
            &me,
        )
        .await;
    assert_eq!(res.status, StatusCode::NO_CONTENT);
    assert_eq!(
        app.get("/api/auth/me", &tv).await.status,
        StatusCode::UNAUTHORIZED
    );
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn tv_codes_expire_and_only_tvs_get_them(db: PgPool) {
    let app = TestApp::new(db).await;
    let me = app.listener("ann").await;

    let code = new_code(&app, "tvos").await;
    sqlx::query("UPDATE tv_logins SET expires_at = now() - interval '1 second'")
        .execute(&app.state.db)
        .await
        .unwrap();
    let res = approve(&app, code["userCode"].as_str().unwrap(), &me).await;
    assert_eq!(res.status, StatusCode::NOT_FOUND, "истёкший код");
    let res = poll(&app, &code).await;
    assert_eq!(res.status, StatusCode::BAD_REQUEST);
    assert_eq!(res.code(), "invalid_grant");

    // Подтверждённый, но не забранный вовремя — тоже не годится
    let code = new_code(&app, "tvos").await;
    let res = approve(&app, code["userCode"].as_str().unwrap(), &me).await;
    assert_eq!(res.status, StatusCode::NO_CONTENT);
    sqlx::query("UPDATE tv_logins SET expires_at = now() - interval '1 second'")
        .execute(&app.state.db)
        .await
        .unwrap();
    assert_eq!(poll(&app, &code).await.status, StatusCode::BAD_REQUEST);

    let res = app
        .post(
            "/api/auth/tv/code",
            json!({ "platform": "ios" }),
            &Auth::None,
        )
        .await;
    assert_eq!(res.status, StatusCode::BAD_REQUEST);
    let res = app
        .post(
            "/api/auth/tv/token",
            json!({ "deviceCode": "nope" }),
            &Auth::None,
        )
        .await;
    assert_eq!(res.status, StatusCode::BAD_REQUEST);
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn tv_browser_gets_a_site_session(db: PgPool) {
    let app = TestApp::new(db).await;
    let me = app.listener("ann").await;
    let code = new_code(&app, "web").await;
    let res = app
        .get(
            &format!("/api/auth/tv/{}", code["userCode"].as_str().unwrap()),
            &me,
        )
        .await;
    assert_eq!(res.body["deviceName"], "Телевизор");
    approve(&app, code["userCode"].as_str().unwrap(), &me).await;
    let res = poll(&app, &code).await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    assert_eq!(res.body["status"], "signed_in");
    assert_eq!(res.body["user"]["email"], "ann@example.com");
    let cookie = res.cookie("dw_session").expect("cookie сайта");
    let tv = Auth::Cookie(cookie);
    assert_eq!(app.get("/api/auth/me", &tv).await.status, StatusCode::OK);
    assert_eq!(poll(&app, &code).await.status, StatusCode::BAD_REQUEST);
    // Выход в браузере телевизора отзывает его сессию
    app.call(Method::POST, "/api/auth/logout", None, &tv).await;
    assert_eq!(
        app.get("/api/auth/me", &tv).await.status,
        StatusCode::UNAUTHORIZED
    );
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn party_settings_open_the_jam_and_tune_the_wave(db: PgPool) {
    let app = TestApp::new(db).await;
    let me = app.listener("ann").await;

    let res = app.get("/api/party", &me).await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    assert_eq!(res.body["active"], false);
    assert_eq!(
        res.body["settings"],
        json!({ "energetic": false, "noArtistRepeat": true, "dj": true })
    );
    assert!(app.get("/api/jam", &me).await.body["jam"].is_null());

    let res = app
        .call(
            Method::PATCH,
            "/api/party",
            Some(json!({ "active": true, "energetic": true })),
            &me,
        )
        .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    assert_eq!(res.body["active"], true);
    assert_eq!(res.body["settings"]["energetic"], true);
    assert!(res.body["until"].is_string());
    // QR для гостей: «Слушать вместе» открылось
    let jam = app.get("/api/jam", &me).await;
    assert!(jam.body["jam"]["url"].as_str().unwrap().contains("/jam/"));

    // Волна на вечеринке — энергичная
    let res = app
        .post(
            "/api/wave/sessions",
            json!({ "settings": { "mood": "calm" } }),
            &me,
        )
        .await;
    assert_eq!(res.status, StatusCode::CREATED, "{}", res.text);
    assert_eq!(res.body["settings"]["mood"], "energetic");
    assert_eq!(res.body["settings"]["activity"], "party");

    // Настройки меняются по одной; выключение их сохраняет, волна снова как задана
    let res = app
        .call(
            Method::PATCH,
            "/api/party",
            Some(json!({ "noArtistRepeat": false })),
            &me,
        )
        .await;
    assert_eq!(
        (
            res.body["active"].as_bool(),
            res.body["settings"]["noArtistRepeat"].as_bool()
        ),
        (Some(true), Some(false))
    );
    let res = app
        .call(
            Method::PATCH,
            "/api/party",
            Some(json!({ "active": false })),
            &me,
        )
        .await;
    assert_eq!(res.body["active"], false);
    assert!(res.body["until"].is_null());
    assert_eq!(res.body["settings"]["energetic"], true);
    let res = app
        .post(
            "/api/wave/sessions",
            json!({ "settings": { "mood": "calm" } }),
            &me,
        )
        .await;
    assert_eq!(res.body["settings"]["mood"], "calm");

    // Чужая вечеринка не видна
    let other = app.listener("bob").await;
    assert_eq!(app.get("/api/party", &other).await.body["active"], false);
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn apple_tv_gets_aac_instead_of_opus(db: PgPool) {
    let app = TestApp::new(db).await;
    let me = app.listener("ann").await;
    let tracks = ep_tracks(&app, &me).await;
    let db = &app.state.db;

    // AAC у записи есть — его и отдаём
    let rec = recording_of(db, tracks[1]).await;
    let opus = add_file(db, rec, "opus", "mp4", false, Some(160)).await;
    let aac = add_file(db, rec, "aac", "mp4", false, Some(256)).await;
    make_primary(db, rec, opus).await;
    let res = app
        .get(&format!("/api/tracks/{}/stream?aac=true", tracks[1]), &me)
        .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    assert_eq!(res.body["fileId"], aac.to_string());
    // Без флага — как раньше
    let res = app
        .get(&format!("/api/tracks/{}/stream", tracks[1]), &me)
        .await;
    assert_eq!(res.body["fileId"], opus.to_string());

    // Только Opus — копия AAC: пока её делают — `preparing`, задача одна
    let track = tracks[0];
    let rec = recording_of(db, track).await;
    let source = add_file(db, rec, "opus", "mp4", false, Some(160)).await;
    make_primary(db, rec, source).await;
    let before = transcode_jobs(db).await;
    let uri = format!("/api/tracks/{track}/stream?aac=true");
    let res = app.get(&uri, &me).await;
    assert_eq!(res.status, StatusCode::ACCEPTED, "{}", res.text);
    assert_eq!(res.body["status"], "preparing");
    app.get(&uri, &me).await;
    assert_eq!(transcode_jobs(db).await, before + 1);

    if !ffmpeg_available() {
        eprintln!("ffmpeg not found: skipping the transcode part");
        return;
    }
    let key: String = sqlx::query_scalar("SELECT storage_key FROM audio_files WHERE id = $1")
        .bind(source)
        .fetch_one(db)
        .await
        .unwrap();
    app.state
        .storage
        .put(&key, opus_bytes(), "audio/mp4", None)
        .await
        .unwrap();
    let (kind, _) = app.run_job(renditions::LANE).await.expect("transcode job");
    assert_eq!(kind, "transcode");
    let res = app.get(&uri, &me).await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    assert_eq!(
        (res.body["codec"].as_str(), res.body["container"].as_str()),
        (Some("aac"), Some("mp4"))
    );
    assert_eq!(res.body["bitrateKbps"], 256);
    assert_eq!(res.body["mimeType"], "audio/mp4; codecs=\"mp4a.40.2\"");
    let bytes = app
        .state
        .storage
        .get_bytes(&renditions::key_of(source, renditions::AAC256))
        .await
        .unwrap();
    assert!(bytes.len() > 1000 && &bytes[4..8] == b"ftyp", "MP4");
}
