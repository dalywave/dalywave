//! Публичное демо (`DALYWAVE_DEMO_ACCOUNT`): вход без пароля на сайте, в приложении и на телевизоре,
//! замки общего аккаунта (пароль, логин, входы, удаление, админка) и частота входа.

use std::collections::HashMap;

use axum::{
    body::Body,
    http::{Method, Request, StatusCode, header},
};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sqlx::PgPool;

use crate::{
    accounts::{post, set_up},
    common::{Auth, Resp, TestApp},
};

const DEMO_PASSWORD: &str = "the demo account's password";
const VERIFIER: &str = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk-demo-verifier";

/// Сервер со своими аккаунтами и демо-аккаунтом `demo` в настройках (сам аккаунт ещё не создан).
async fn demo_app(db: PgPool) -> TestApp {
    TestApp::with(db, |c| {
        c.local_auth = true;
        c.demo_account = Some("demo".into());
    })
    .await
}

/// Аккаунт `demo` с ролью listener — как его заводит seed демо-сервера. Возвращает id.
async fn create_demo(app: &TestApp, admin: &Auth) -> String {
    let res = app
        .post(
            "/api/admin/users",
            json!({ "name": "Гость", "username": "demo", "password": DEMO_PASSWORD, "roles": ["listener"] }),
            admin,
        )
        .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    res.body["user"]["id"].as_str().unwrap().to_string()
}

async fn demo_login(app: &TestApp, ip: &str) -> Resp {
    post(app, "/api/auth/demo", json!({}), &[], ip).await
}

/// Вход в демо на сайте: cookie сессии.
async fn demo_cookie(app: &TestApp, ip: &str) -> Auth {
    let res = demo_login(app, ip).await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    Auth::Cookie(res.cookie("dw_session").expect("dw_session"))
}

fn query(url: &str) -> HashMap<String, String> {
    url::Url::parse(url)
        .unwrap()
        .query_pairs()
        .into_owned()
        .collect()
}

fn app_start(state: &str, demo: bool) -> String {
    let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(VERIFIER.as_bytes()));
    let demo = if demo { "&demo=1" } else { "" };
    format!(
        "/api/auth/app/start?redirect_uri=dalywave%3A%2F%2Fauth&code_challenge={challenge}\
         &code_challenge_method=S256&state={state}&device_name=iPhone&platform=ios{demo}"
    )
}

/// Код из адреса возврата приложения → токен устройства.
async fn exchange(app: &TestApp, back: &str) -> String {
    let q = query(back);
    let res = app
        .call(
            Method::POST,
            "/api/auth/app/token",
            Some(json!({ "code": q["code"], "codeVerifier": VERIFIER })),
            &Auth::None,
        )
        .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    res.body["token"].as_str().unwrap().to_string()
}

async fn tv_code(app: &TestApp) -> Value {
    let res = app
        .post(
            "/api/auth/tv/code",
            json!({ "platform": "tvos" }),
            &Auth::None,
        )
        .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    res.body
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn no_demo_without_the_setting(db: PgPool) {
    // Как на wave.daly.rest и других установках: аккаунт `demo` может и быть, но демо нет совсем
    let app = TestApp::with(db, |c| c.local_auth = true).await;
    let admin = set_up(&app).await;
    create_demo(&app, &admin).await;

    let providers = app.get("/api/auth/providers", &Auth::None).await;
    assert_eq!(providers.body["demo"], false);
    let res = demo_login(&app, "203.0.113.1").await;
    assert_eq!(res.status, StatusCode::NOT_FOUND, "{}", res.text);
    assert!(res.cookie("dw_session").is_none());
    let code = tv_code(&app).await;
    let res = app
        .post(
            "/api/auth/demo/tv",
            json!({ "deviceCode": code["deviceCode"] }),
            &Auth::None,
        )
        .await;
    assert_eq!(res.status, StatusCode::NOT_FOUND);
    // Приложение с `demo=1` получает ошибку на свой адрес, а не вход
    let res = app.get(&app_start("s1", true), &Auth::None).await;
    assert_eq!(res.status, StatusCode::FOUND);
    let q = query(&res.location());
    assert_eq!(q["error"], "demo_unavailable", "{}", res.location());
    assert_eq!(q["state"], "s1");

    // Тот же логин без настройки — обычный аккаунт: не демо, пароль меняется
    let res = post(
        &app,
        "/api/auth/local/login",
        json!({ "username": "demo", "password": DEMO_PASSWORD }),
        &[],
        "203.0.113.2",
    )
    .await;
    assert_eq!(res.body["me"]["demo"], false);
    let auth = Auth::Cookie(res.cookie("dw_session").unwrap());
    let res = app
        .post(
            "/api/auth/account/password",
            json!({ "current": DEMO_PASSWORD, "password": "a brand new password" }),
            &auth,
        )
        .await;
    assert_eq!(res.status, StatusCode::NO_CONTENT, "{}", res.text);
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn site_signs_in_to_demo_without_a_password(db: PgPool) {
    let app = demo_app(db).await;
    let admin = set_up(&app).await;
    let providers = app.get("/api/auth/providers", &Auth::None).await;
    assert_eq!(providers.body["demo"], true);
    assert_eq!(providers.body["local"], true);

    // Настройка есть, аккаунта ещё нет — входить некуда
    let res = demo_login(&app, "203.0.113.1").await;
    assert_eq!(res.status, StatusCode::NOT_FOUND, "{}", res.text);

    // Админом демо-аккаунт не создаётся
    let res = app
        .post(
            "/api/admin/users",
            json!({ "username": "Demo", "password": DEMO_PASSWORD, "roles": ["admin"] }),
            &admin,
        )
        .await;
    assert_eq!(res.status, StatusCode::FORBIDDEN);
    assert_eq!(res.code(), "demo_account");
    create_demo(&app, &admin).await;

    let res = demo_login(&app, "203.0.113.1").await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    assert!(res.body["redirect"].is_null());
    assert_eq!(res.body["me"]["demo"], true);
    assert_eq!(res.body["me"]["via"], "local");
    assert_eq!(res.body["me"]["roles"], json!(["listener"]));
    assert_eq!(res.body["me"]["user"]["name"], "Гость");
    let cookie = Auth::Cookie(res.cookie("dw_session").unwrap());
    let me = app.get("/api/auth/me", &cookie).await;
    assert_eq!(me.body["demo"], true);
    // Медиатека открыта, как у любого слушателя
    assert_eq!(
        app.get("/api/library/tracks", &cookie).await.status,
        StatusCode::OK
    );
    // Владелец — не демо
    let me = app.get("/api/auth/me", &admin).await;
    assert_eq!(me.body["demo"], false);

    // Origin проверяется, как у обычного входа: с чужого сайта не войти
    let req = Request::builder()
        .method(Method::POST)
        .uri("/api/auth/demo")
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::ORIGIN, "https://evil.example")
        .header("x-forwarded-for", "203.0.113.3")
        .body(Body::from("{}"))
        .unwrap();
    let res = app.send(req).await;
    assert_eq!(res.status, StatusCode::FORBIDDEN);
    assert_eq!(res.code(), "bad_origin");
    // И не простой формой без JSON
    let req = Request::builder()
        .method(Method::POST)
        .uri("/api/auth/demo")
        .header(header::CONTENT_TYPE, "text/plain")
        .header("x-forwarded-for", "203.0.113.3")
        .body(Body::from("{}"))
        .unwrap();
    assert_eq!(
        app.send(req).await.status,
        StatusCode::UNSUPPORTED_MEDIA_TYPE
    );

    // Заблокированный демо-аккаунт не пускает
    sqlx::query("UPDATE users SET disabled = true WHERE username = 'demo'")
        .execute(&app.state.db)
        .await
        .unwrap();
    let res = demo_login(&app, "203.0.113.4").await;
    assert_eq!(res.status, StatusCode::FORBIDDEN);
    assert!(
        app.audit_kinds()
            .await
            .contains(&"auth.sign_in_denied".to_string())
    );
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn demo_account_is_locked(db: PgPool) {
    let app = demo_app(db).await;
    let admin = set_up(&app).await;
    let demo_id = create_demo(&app, &admin).await;
    let demo = demo_cookie(&app, "203.0.113.1").await;

    let locked = |res: Resp, what: &str| {
        assert_eq!(res.status, StatusCode::FORBIDDEN, "{what}: {}", res.text);
        assert_eq!(res.code(), "demo_account", "{what}");
    };
    // Пароль и логин
    locked(
        app.post(
            "/api/auth/account/password",
            json!({ "current": DEMO_PASSWORD, "password": "someone else's password" }),
            &demo,
        )
        .await,
        "пароль",
    );
    // Удалить аккаунт
    locked(app.delete("/api/auth/me", &demo).await, "удаление");
    // Отвязать и привязать вход провайдера (почта приходит только от провайдера)
    locked(
        app.delete(
            "/api/auth/account/identities/00000000-0000-7000-8000-000000000001",
            &demo,
        )
        .await,
        "отвязка",
    );
    locked(
        app.get("/api/auth/oidc/dalyrest/start?link=1", &demo).await,
        "привязка",
    );
    // Приглашения и вся админка
    locked(
        app.post("/api/admin/invites", json!({ "roles": ["admin"] }), &demo)
            .await,
        "приглашение",
    );
    locked(app.get("/api/admin/users", &demo).await, "админка");

    // Админ не может сделать его администратором
    locked(
        app.call(
            Method::PATCH,
            &format!("/api/admin/users/{demo_id}"),
            Some(json!({ "roles": ["admin", "listener"] })),
            &admin,
        )
        .await,
        "роль admin",
    );
    // Даже если роль попала в базу в обход API — гость её не получает
    sqlx::query("UPDATE users SET roles = '{admin,listener}' WHERE username = 'demo'")
        .execute(&app.state.db)
        .await
        .unwrap();
    let me = app.get("/api/auth/me", &demo).await;
    assert_eq!(me.body["roles"], json!(["listener"]));
    assert_eq!(me.body["permissions"], json!(["play", "library"]));
    locked(
        app.get("/api/admin/invites", &demo).await,
        "админка с ролью",
    );

    // Всё осталось как было: пароль прежний, аккаунт на месте, вход работает
    let res = post(
        &app,
        "/api/auth/local/login",
        json!({ "username": "demo", "password": DEMO_PASSWORD }),
        &[],
        "203.0.113.9",
    )
    .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    assert_eq!(res.body["me"]["demo"], true);
    // Обычные действия слушателя доступны
    let res = app
        .post("/api/playlists", json!({ "title": "Гостевой" }), &demo)
        .await;
    assert!(res.status.is_success(), "{}", res.text);
    // У других людей всё по-прежнему
    let res = app
        .post(
            "/api/admin/invites",
            json!({ "roles": ["listener"] }),
            &admin,
        )
        .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn apps_and_tv_sign_in_to_demo(db: PgPool) {
    let app = demo_app(db).await;
    let admin = set_up(&app).await;
    create_demo(&app, &admin).await;

    // Приложение, как после обычного входа: экран входа в системном браузере → код → токен
    let res = app.get(&app_start("s1", false), &Auth::None).await;
    assert_eq!(
        res.location(),
        format!("{}/login?app=1", crate::common::PUBLIC_URL)
    );
    let app_cookie = res.cookie("dw_app").unwrap();
    let res = post(
        &app,
        "/api/auth/demo",
        json!({ "app": true }),
        &[&app_cookie],
        "203.0.113.1",
    )
    .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    assert!(res.cookie("dw_session").is_none(), "сессия сайта не нужна");
    let back = res.body["redirect"].as_str().unwrap().to_string();
    assert!(back.starts_with("dalywave://auth?code="), "{back}");
    assert_eq!(query(&back)["state"], "s1");
    let token = exchange(&app, &back).await;
    let me = app.get("/api/auth/me", &Auth::Bearer(token.clone())).await;
    assert_eq!(me.body["demo"], true);
    assert_eq!(me.body["via"], "device");
    // Замки — и у токена приложения
    let res = app.delete("/api/auth/me", &Auth::Bearer(token)).await;
    assert_eq!(res.code(), "demo_account");

    // «Войти в демо» в самом приложении: экран входа не открывается, код — сразу
    let res = app.get(&app_start("s2", true), &Auth::None).await;
    assert_eq!(res.status, StatusCode::FOUND, "{}", res.text);
    let back = res.location();
    assert!(back.starts_with("dalywave://auth?code="), "{back}");
    assert_eq!(query(&back)["state"], "s2");
    let token = exchange(&app, &back).await;
    let me = app.get("/api/auth/me", &Auth::Bearer(token)).await;
    assert_eq!(me.body["demo"], true);
    assert_eq!(me.body["device"]["platform"], "ios");

    // Телевизор: свой код подтверждает сам и забирает сессию обычным опросом
    let code = tv_code(&app).await;
    let res = app
        .post(
            "/api/auth/demo/tv",
            json!({ "deviceCode": code["deviceCode"] }),
            &Auth::None,
        )
        .await;
    assert_eq!(res.status, StatusCode::NO_CONTENT, "{}", res.text);
    let res = app
        .post(
            "/api/auth/tv/token",
            json!({ "deviceCode": code["deviceCode"], "appVersion": "0.1.19" }),
            &Auth::None,
        )
        .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    assert_eq!(res.body["status"], "ready");
    let tv = Auth::Bearer(res.body["token"].as_str().unwrap().to_string());
    let me = app.get("/api/auth/me", &tv).await;
    assert_eq!(me.body["demo"], true);
    assert_eq!(me.body["device"]["platform"], "tvos");
    // Код одноразовый, чужой не подходит
    let res = app
        .post(
            "/api/auth/demo/tv",
            json!({ "deviceCode": code["deviceCode"] }),
            &Auth::None,
        )
        .await;
    assert_eq!(res.code(), "invalid_grant");
    let res = app
        .post(
            "/api/auth/demo/tv",
            json!({ "deviceCode": "not-a-device-code" }),
            &Auth::None,
        )
        .await;
    assert_eq!(res.code(), "invalid_grant");
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn demo_sign_in_is_rate_limited(db: PgPool) {
    let app = demo_app(db).await;
    let admin = set_up(&app).await;
    create_demo(&app, &admin).await;

    // Пять входов в минуту с адреса, как у входа по логину
    for _ in 0..5 {
        let res = demo_login(&app, "203.0.113.20").await;
        assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    }
    let res = demo_login(&app, "203.0.113.20").await;
    assert_eq!(res.status, StatusCode::TOO_MANY_REQUESTS);
    // Предел общий для сайта, приложения и телевизора
    let req = Request::builder()
        .uri(app_start("s1", true))
        .header("x-forwarded-for", "203.0.113.20")
        .body(Body::empty())
        .unwrap();
    let res = app.send(req).await;
    assert_eq!(query(&res.location())["error"], "too_many_requests");
    // С другого адреса — можно
    let res = demo_login(&app, "203.0.113.21").await;
    assert_eq!(res.status, StatusCode::OK);
}
