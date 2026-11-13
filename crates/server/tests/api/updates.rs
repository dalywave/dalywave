use axum::http::StatusCode;
use serde_json::json;
use sqlx::PgPool;

use crate::{
    common::{Auth, TestApp},
    mock::Person,
};

const TOKEN: &str = "release-token-0123456789abcdef0123456789abcdef";
const SIG: &str = "dW50cnVzdGVkIGNvbW1lbnQ6IHNpZ25hdHVyZQo=";

async fn app(db: PgPool) -> TestApp {
    TestApp::with(db, |c| c.release_token = Some(TOKEN.into())).await
}

fn release(version: &str, file: &str) -> serde_json::Value {
    json!({
        "target": "darwin",
        "arch": "aarch64",
        "version": version,
        "fileName": file,
        "signature": SIG,
        "notes": format!("Что нового в {version}"),
    })
}

/// Выпуск как в конвейере: ссылка на загрузку, файл в хранилище, регистрация.
async fn publish(app: &TestApp, version: &str, file: &str) {
    let auth = Auth::Bearer(TOKEN.into());
    let res = app
        .post(
            "/api/releases/uploads",
            json!({ "target": "darwin", "arch": "aarch64", "version": version, "fileName": file }),
            &auth,
        )
        .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    let key = res.body["key"].as_str().unwrap().to_string();
    // Конвейер кладёт файл по подписанной ссылке; в тесте — прямо в хранилище в памяти
    app.state
        .storage
        .put(&key, vec![1u8; 64], "application/gzip", None)
        .await
        .unwrap();
    let res = app
        .post("/api/releases", release(version, file), &auth)
        .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn releases_need_the_release_token(db: PgPool) {
    // Токена в настройках нет — выпускать нельзя никому
    let off = TestApp::new(db.clone()).await;
    let body = release("0.2.0", "DalyWave.app.tar.gz");
    for auth in [Auth::None, Auth::Bearer(TOKEN.into())] {
        let res = off.post("/api/releases", body.clone(), &auth).await;
        assert_eq!(res.status, StatusCode::UNAUTHORIZED, "{}", res.text);
    }

    let app = app(db).await;
    let res = app
        .post("/api/releases", body.clone(), &Auth::Bearer("wrong".into()))
        .await;
    assert_eq!(res.status, StatusCode::UNAUTHORIZED);
    assert_eq!(res.code(), "unauthorized");
    // Сессия админа сюда тоже не подходит
    let (_, admin) = app.admin("boss").await;
    let res = app.post("/api/releases/uploads", body, &admin).await;
    assert_eq!(res.status, StatusCode::UNAUTHORIZED);
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn upload_link_and_validation(db: PgPool) {
    let app = app(db).await;
    let auth = Auth::Bearer(TOKEN.into());
    let res = app
        .post(
            "/api/releases/uploads",
            json!({ "target": "windows", "arch": "x86_64", "version": "v0.2.0", "fileName": "DalyWave_0.2.0_x64-setup.exe" }),
            &auth,
        )
        .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    assert_eq!(
        res.body["key"],
        "releases/windows/x86_64/0.2.0/DalyWave_0.2.0_x64-setup.exe"
    );
    let url = res.body["url"].as_str().unwrap();
    assert!(
        url.contains("/dalywave/releases/windows/x86_64/0.2.0/DalyWave_0.2.0_x64-setup.exe?"),
        "{url}"
    );
    assert!(url.contains("X-Amz-Expires=3600"), "{url}");
    assert!(res.body["expiresAt"].is_string());

    for (patch, what) in [
        (json!({ "target": "ios" }), "target"),
        (json!({ "arch": "arm64" }), "arch"),
        (json!({ "version": "0.2" }), "version"),
        (json!({ "fileName": "../escape.exe" }), "file"),
        (json!({ "fileName": "a/b.exe" }), "file"),
    ] {
        let mut body = json!({ "target": "darwin", "arch": "aarch64", "version": "0.2.0", "fileName": "DalyWave.app.tar.gz" });
        for (k, v) in patch.as_object().unwrap() {
            body[k] = v.clone();
        }
        let res = app.post("/api/releases/uploads", body, &auth).await;
        assert_eq!(res.status, StatusCode::BAD_REQUEST, "{what}: {}", res.text);
        assert_eq!(res.code(), "bad_request");
    }

    // Регистрация без загруженного файла и с неверной подписью
    let res = app
        .post(
            "/api/releases",
            release("0.2.0", "DalyWave.app.tar.gz"),
            &auth,
        )
        .await;
    assert_eq!(res.status, StatusCode::CONFLICT);
    assert_eq!(res.code(), "not_uploaded");
    let mut body = release("0.2.0", "DalyWave.app.tar.gz");
    body["signature"] = json!("not a signature!");
    let res = app.post("/api/releases", body, &auth).await;
    assert_eq!(res.status, StatusCode::BAD_REQUEST);
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn check_returns_the_newest_release(db: PgPool) {
    let app = app(db).await;
    let check = |v: &str| format!("/api/updates/darwin/aarch64/{v}");
    // Выпусков нет — 204, как ждёт плагин Tauri
    assert_eq!(
        app.get(&check("0.1.0"), &Auth::None).await.status,
        StatusCode::NO_CONTENT
    );

    publish(&app, "0.2.0", "DalyWave.app.tar.gz").await;
    // 0.10.0 новее 0.9.0: порядок SemVer, а не строк
    publish(&app, "0.9.0", "DalyWave.app.tar.gz").await;
    publish(&app, "0.10.0", "DalyWave.app.tar.gz").await;

    let res = app.get(&check("0.1.0"), &Auth::None).await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    assert_eq!(res.headers["cache-control"], "no-store");
    assert_eq!(res.body["version"], "0.10.0");
    assert_eq!(res.body["notes"], "Что нового в 0.10.0");
    assert_eq!(res.body["signature"], SIG);
    assert!(res.body["pub_date"].is_string(), "{}", res.text);
    let url = res.body["url"].as_str().unwrap();
    assert!(
        url.contains("/releases/darwin/aarch64/0.10.0/DalyWave.app.tar.gz?"),
        "{url}"
    );
    assert!(url.contains("X-Amz-Expires=21600"), "{url}");

    for current in ["0.10.0", "0.11.0", "1.0.0"] {
        assert_eq!(
            app.get(&check(current), &Auth::None).await.status,
            StatusCode::NO_CONTENT,
            "{current}"
        );
    }
    // Другая платформа выпусков не видит
    assert_eq!(
        app.get("/api/updates/windows/x86_64/0.1.0", &Auth::None)
            .await
            .status,
        StatusCode::NO_CONTENT
    );
    let res = app.get(&check("latest"), &Auth::None).await;
    assert_eq!(res.status, StatusCode::BAD_REQUEST);

    // Повторная сборка той же версии заменяет выпуск
    let auth = Auth::Bearer(TOKEN.into());
    let mut body = release("0.10.0", "DalyWave.app.tar.gz");
    body["notes"] = json!("Исправлено");
    let res = app.post("/api/releases", body, &auth).await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    assert_eq!(res.body["sizeBytes"], 64);
    let res = app.get(&check("0.9.0"), &Auth::None).await;
    assert_eq!(res.body["notes"], "Исправлено");
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM app_releases")
        .fetch_one(&app.state.db)
        .await
        .unwrap();
    assert_eq!(count, 3);
}

/// Файл в хранилище по ссылке от сервера — как его кладёт конвейер.
async fn upload_file(app: &TestApp, target: &str, arch: &str, version: &str, file: &str) {
    let res = app
        .post(
            "/api/releases/uploads",
            json!({ "target": target, "arch": arch, "version": version, "fileName": file }),
            &Auth::Bearer(TOKEN.into()),
        )
        .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    let key = res.body["key"].as_str().unwrap().to_string();
    app.state
        .storage
        .put(&key, vec![7u8; 128], "application/octet-stream", None)
        .await
        .unwrap();
}

/// Выпуск macOS с образом диска: архив обновления и `.dmg` рядом.
async fn publish_mac(app: &TestApp, version: &str) {
    let dmg = format!("DalyWave_{version}_aarch64.dmg");
    upload_file(app, "darwin", "aarch64", version, "DalyWave.app.tar.gz").await;
    upload_file(app, "darwin", "aarch64", version, &dmg).await;
    let mut body = release(version, "DalyWave.app.tar.gz");
    body["installerFileName"] = json!(dmg);
    let res = app
        .post("/api/releases", body, &Auth::Bearer(TOKEN.into()))
        .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    assert_eq!(res.body["installerSizeBytes"], 128);
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn installers_are_registered_with_the_release(db: PgPool) {
    let app = app(db).await;
    let auth = Auth::Bearer(TOKEN.into());
    // Установщик не загружен — выпуск не регистрируется
    upload_file(&app, "darwin", "aarch64", "0.2.0", "DalyWave.app.tar.gz").await;
    let mut body = release("0.2.0", "DalyWave.app.tar.gz");
    body["installerFileName"] = json!("DalyWave_0.2.0_aarch64.dmg");
    let res = app.post("/api/releases", body.clone(), &auth).await;
    assert_eq!(res.status, StatusCode::CONFLICT, "{}", res.text);
    assert_eq!(res.code(), "not_uploaded");
    // Архив обновления — не установщик
    body["installerFileName"] = json!("DalyWave.app.tar.gz");
    let res = app.post("/api/releases", body, &auth).await;
    assert_eq!(res.status, StatusCode::BAD_REQUEST, "{}", res.text);

    // Windows: установщик NSIS и есть файл обновления
    let exe = "DalyWave_0.2.0_x64-setup.exe";
    upload_file(&app, "windows", "x86_64", "0.2.0", exe).await;
    let res = app
        .post(
            "/api/releases",
            json!({
                "target": "windows", "arch": "x86_64", "version": "0.2.0",
                "fileName": exe, "installerFileName": exe, "signature": SIG,
            }),
            &auth,
        )
        .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    assert_eq!(res.body["sizeBytes"], 128);
    assert_eq!(res.body["installerSizeBytes"], 128);

    // Автообновление установщика не замечает: тот же ответ, что и раньше
    publish_mac(&app, "0.2.0").await;
    let res = app
        .get("/api/updates/darwin/aarch64/0.1.0", &Auth::None)
        .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    let url = res.body["url"].as_str().unwrap();
    assert!(url.contains("/0.2.0/DalyWave.app.tar.gz?"), "{url}");
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn downloads_for_people_with_a_role(db: PgPool) {
    let app = app(db).await;
    // Без входа и без ролей — нельзя, как и весь каталог
    let res = app.get("/api/downloads", &Auth::None).await;
    assert_eq!(res.status, StatusCode::UNAUTHORIZED);
    let res = app.get("/api/downloads/mac/latest", &Auth::None).await;
    assert_eq!(res.status, StatusCode::UNAUTHORIZED);
    let stranger = Auth::Cookie(app.login(&Person::new("stranger")).await);
    let res = app.get("/api/downloads", &stranger).await;
    assert_eq!(res.status, StatusCode::FORBIDDEN);
    assert_eq!(res.code(), "no_access");

    let me = app.listener("fan").await;
    // Выпусков ещё нет — пусто, скачать нечего
    let res = app.get("/api/downloads", &me).await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    assert_eq!(res.body["apps"], json!([]));
    let res = app.get("/api/downloads/mac/latest", &me).await;
    assert_eq!(res.status, StatusCode::NOT_FOUND);

    // Выпуск без установщика (прежний конвейер) скачать нельзя, а с установщиком — можно,
    // и самый новый по SemVer
    publish(&app, "0.30.0", "DalyWave.app.tar.gz").await;
    publish_mac(&app, "0.2.0").await;
    publish_mac(&app, "0.10.0").await;
    publish_mac(&app, "0.9.0").await;
    let res = app.get("/api/downloads", &me).await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    let apps = res.body["apps"].as_array().unwrap();
    assert_eq!(apps.len(), 1, "{}", res.text);
    let mac = &apps[0];
    assert_eq!(mac["platform"], "mac");
    assert_eq!(mac["os"], "macos");
    assert_eq!(mac["arch"], "aarch64");
    assert_eq!(mac["version"], "0.10.0");
    assert_eq!(mac["sizeBytes"], 128);
    assert_eq!(mac["fileName"], "DalyWave-0.10.0.dmg");
    assert_eq!(mac["minOsVersion"], "13");
    assert_eq!(mac["url"], "/api/downloads/mac/latest");
    assert!(mac["publishedAt"].is_string());

    let res = app.get("/api/downloads/mac/latest", &me).await;
    assert_eq!(res.status, StatusCode::FOUND, "{}", res.text);
    assert_eq!(res.headers["cache-control"], "no-store");
    let url = res.location();
    assert!(
        url.contains("/releases/darwin/aarch64/0.10.0/DalyWave_0.10.0_aarch64.dmg?"),
        "{url}"
    );
    assert!(url.contains("X-Amz-Expires=900"), "{url}");
    assert!(
        url.contains("filename%3D%22DalyWave-0.10.0.dmg%22"),
        "{url}"
    );
    // Windows ещё не выпущен; платформы linux на сайте нет
    for path in [
        "/api/downloads/windows/latest",
        "/api/downloads/linux/latest",
    ] {
        let res = app.get(path, &me).await;
        assert_eq!(res.status, StatusCode::NOT_FOUND, "{path}");
    }
}
