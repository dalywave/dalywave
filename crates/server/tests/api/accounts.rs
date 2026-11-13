//! Свои аккаунты: первый запуск, вход логином и паролем, ограничение попыток, приглашения,
//! ссылки сброса, смена пароля и вход приложения через экран входа сайта.

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
    common::{Auth, PUBLIC_URL, Resp, TestApp},
    mock::Person,
};

pub const OWNER_PASSWORD: &str = "the owner's long password";
const VERIFIER: &str = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk-app-verifier";

/// POST как из браузера: JSON, Origin сайта, cookie и адрес клиента.
pub async fn post(app: &TestApp, uri: &str, body: Value, cookies: &[&str], ip: &str) -> Resp {
    let mut req = Request::builder()
        .method(Method::POST)
        .uri(uri)
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::ORIGIN, PUBLIC_URL)
        .header("x-forwarded-for", ip);
    if !cookies.is_empty() {
        req = req.header(header::COOKIE, cookies.join("; "));
    }
    app.send(req.body(Body::from(body.to_string())).unwrap())
        .await
}

pub async fn get_with(app: &TestApp, uri: &str, cookies: &[&str]) -> Resp {
    let mut req = Request::builder().uri(uri);
    if !cookies.is_empty() {
        req = req.header(header::COOKIE, cookies.join("; "));
    }
    app.send(req.body(Body::empty()).unwrap()).await
}

/// Сервер со своими аккаунтами (и daly.rest из прежних переменных, как в остальных тестах).
pub async fn local_app(db: PgPool) -> TestApp {
    TestApp::with(db, |c| c.local_auth = true).await
}

/// Мастер первого запуска: администратор `owner`. Возвращает cookie сессии.
pub async fn set_up(app: &TestApp) -> Auth {
    let res = post(
        app,
        "/api/auth/setup",
        json!({ "name": "Захар", "username": "Owner", "password": OWNER_PASSWORD }),
        &[],
        "203.0.113.1",
    )
    .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    Auth::Cookie(res.cookie("dw_session").unwrap())
}

async fn login(app: &TestApp, username: &str, password: &str, ip: &str) -> Resp {
    post(
        app,
        "/api/auth/local/login",
        json!({ "username": username, "password": password }),
        &[],
        ip,
    )
    .await
}

fn query(url: &str) -> HashMap<String, String> {
    url::Url::parse(url)
        .unwrap()
        .query_pairs()
        .into_owned()
        .collect()
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn first_run_creates_the_admin(db: PgPool) {
    let app = local_app(db).await;
    let providers = app.get("/api/auth/providers", &Auth::None).await;
    assert_eq!(providers.body["local"], true);
    assert_eq!(providers.body["setup"], true, "людей нет — мастер");

    // Короткий пароль и плохой логин не принимаются
    let res = post(
        &app,
        "/api/auth/setup",
        json!({ "username": "owner", "password": "short" }),
        &[],
        "203.0.113.1",
    )
    .await;
    assert_eq!(res.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(res.code(), "weak_password");
    let res = post(
        &app,
        "/api/auth/setup",
        json!({ "username": "a b", "password": OWNER_PASSWORD }),
        &[],
        "203.0.113.1",
    )
    .await;
    assert_eq!(res.code(), "bad_username");

    let cookie = set_up(&app).await;
    let me = app.get("/api/auth/me", &cookie).await;
    assert_eq!(me.body["user"]["name"], "Захар");
    assert_eq!(me.body["roles"], json!(["admin"]));
    assert_eq!(me.body["via"], "local");
    let hash: String = sqlx::query_scalar("SELECT password_hash FROM users")
        .fetch_one(&app.state.db)
        .await
        .unwrap();
    assert!(hash.starts_with("$argon2id$"), "argon2id: {hash}");

    // Второй раз мастер не открывается
    let res = post(
        &app,
        "/api/auth/setup",
        json!({ "username": "intruder", "password": OWNER_PASSWORD }),
        &[],
        "203.0.113.2",
    )
    .await;
    assert_eq!(res.status, StatusCode::CONFLICT);
    assert_eq!(res.code(), "already_set_up");
    let providers = app.get("/api/auth/providers", &Auth::None).await;
    assert_eq!(providers.body["setup"], false);
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn local_login_and_rate_limit(db: PgPool) {
    let app = local_app(db).await;
    set_up(&app).await;

    let res = login(&app, "OWNER", OWNER_PASSWORD, "203.0.113.5").await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    assert_eq!(res.body["me"]["via"], "local");
    assert!(res.body["redirect"].is_null());
    assert!(res.cookie("dw_session").is_some());

    // Неизвестный логин и чужой пароль отвечают одинаково
    let res = login(&app, "nobody", OWNER_PASSWORD, "203.0.113.5").await;
    assert_eq!(res.status, StatusCode::UNAUTHORIZED);
    assert_eq!(res.code(), "wrong_password");

    // Пять попыток в минуту на пару «адрес + логин»
    for _ in 0..4 {
        let res = login(&app, "owner", "wrong password!!", "203.0.113.9").await;
        assert_eq!(res.status, StatusCode::UNAUTHORIZED);
    }
    let res = login(&app, "owner", OWNER_PASSWORD, "203.0.113.9").await;
    assert_eq!(res.status, StatusCode::OK, "пятая попытка ещё проходит");
    let res = login(&app, "owner", OWNER_PASSWORD, "203.0.113.9").await;
    assert_eq!(res.status, StatusCode::TOO_MANY_REQUESTS);
    // С другого адреса — можно
    let res = login(&app, "owner", OWNER_PASSWORD, "203.0.113.10").await;
    assert_eq!(res.status, StatusCode::OK);

    let kinds = app.audit_kinds().await;
    assert!(kinds.contains(&"auth.sign_in_failed".to_string()));

    // Заблокированный не входит
    let id: uuid::Uuid = sqlx::query_scalar("SELECT id FROM users")
        .fetch_one(&app.state.db)
        .await
        .unwrap();
    sqlx::query("UPDATE users SET disabled = true WHERE id = $1")
        .bind(id)
        .execute(&app.state.db)
        .await
        .unwrap();
    let res = login(&app, "owner", OWNER_PASSWORD, "203.0.113.11").await;
    assert_eq!(res.status, StatusCode::FORBIDDEN);
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn local_accounts_can_be_off(db: PgPool) {
    let app = TestApp::new(db).await;
    let providers = app.get("/api/auth/providers", &Auth::None).await;
    assert_eq!(providers.body["local"], false);
    assert_eq!(
        providers.body["setup"], false,
        "без своих аккаунтов мастера нет"
    );
    let res = post(
        &app,
        "/api/auth/setup",
        json!({ "username": "owner", "password": OWNER_PASSWORD }),
        &[],
        "203.0.113.1",
    )
    .await;
    assert_eq!(res.status, StatusCode::FORBIDDEN);
    assert_eq!(res.code(), "password_disabled");
    let res = login(&app, "owner", OWNER_PASSWORD, "203.0.113.1").await;
    assert_eq!(res.code(), "password_disabled");
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn invites_give_roles(db: PgPool) {
    let app = local_app(db).await;
    let admin = set_up(&app).await;

    let res = app
        .post(
            "/api/admin/invites",
            json!({ "roles": ["listener"], "days": 3, "note": "Для Ани" }),
            &admin,
        )
        .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    let url = res.body["url"].as_str().unwrap().to_string();
    assert!(url.starts_with(&format!("{PUBLIC_URL}/invite/")), "{url}");
    let token = url.rsplit('/').next().unwrap().to_string();
    assert_eq!(res.body["invite"]["status"], "active");

    let info = app
        .get(&format!("/api/auth/invites/{token}"), &Auth::None)
        .await;
    assert_eq!(info.status, StatusCode::OK);
    assert_eq!(info.body["roles"], json!(["listener"]));

    let res = post(
        &app,
        &format!("/api/auth/invites/{token}"),
        json!({ "name": "Аня", "username": "anna", "password": "anna's password" }),
        &[],
        "203.0.113.20",
    )
    .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    assert_eq!(res.body["me"]["roles"], json!(["listener"]));
    assert_eq!(res.body["me"]["user"]["name"], "Аня");

    // Приглашение одноразовое
    let res = post(
        &app,
        &format!("/api/auth/invites/{token}"),
        json!({ "username": "anna2", "password": "anna's password" }),
        &[],
        "203.0.113.20",
    )
    .await;
    assert_eq!(res.status, StatusCode::NOT_FOUND);
    assert_eq!(res.code(), "invite_invalid");

    // Отозванное — тоже не действует; логин уже занят — 409
    let res = app
        .post("/api/admin/invites", json!({ "roles": [] }), &admin)
        .await;
    let second = res.body["url"]
        .as_str()
        .unwrap()
        .rsplit('/')
        .next()
        .unwrap()
        .to_string();
    let id = res.body["invite"]["id"].as_str().unwrap().to_string();
    let res = post(
        &app,
        &format!("/api/auth/invites/{second}"),
        json!({ "username": "Anna", "password": "anna's password" }),
        &[],
        "203.0.113.21",
    )
    .await;
    assert_eq!(res.code(), "username_taken");
    assert_eq!(
        app.delete(&format!("/api/admin/invites/{id}"), &admin)
            .await
            .status,
        StatusCode::NO_CONTENT
    );
    let res = app
        .get(&format!("/api/auth/invites/{second}"), &Auth::None)
        .await;
    assert_eq!(res.status, StatusCode::NOT_FOUND);

    let list = app.get("/api/admin/invites", &admin).await;
    let statuses: Vec<&str> = list.body["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["status"].as_str().unwrap())
        .collect();
    assert_eq!(statuses, ["revoked", "used"]);
    assert_eq!(list.body["items"][1]["usedBy"], "Аня");
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn admin_creates_users_and_reset_links(db: PgPool) {
    let app = local_app(db).await;
    let admin = set_up(&app).await;

    // Без пароля — ссылка, по которой человек задаст его сам
    let res = app
        .post(
            "/api/admin/users",
            json!({ "name": "Боря", "username": "bob", "roles": ["listener"] }),
            &admin,
        )
        .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    assert_eq!(res.body["user"]["username"], "bob");
    assert_eq!(res.body["user"]["hasPassword"], false);
    let link = res.body["link"]["url"].as_str().unwrap().to_string();
    assert!(link.starts_with(&format!("{PUBLIC_URL}/reset/")), "{link}");
    let token = link.rsplit('/').next().unwrap().to_string();
    let user_id = res.body["user"]["id"].as_str().unwrap().to_string();

    let info = app
        .get(&format!("/api/auth/reset/{token}"), &Auth::None)
        .await;
    assert_eq!(info.body["name"], "Боря");
    assert_eq!(info.body["username"], "bob");
    let res = post(
        &app,
        &format!("/api/auth/reset/{token}"),
        json!({ "password": "bob's new password" }),
        &[],
        "203.0.113.30",
    )
    .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    let bob = Auth::Cookie(res.cookie("dw_session").unwrap());
    assert_eq!(res.body["me"]["roles"], json!(["listener"]));
    // Ссылка одноразовая
    let res = post(
        &app,
        &format!("/api/auth/reset/{token}"),
        json!({ "password": "bob's new password" }),
        &[],
        "203.0.113.30",
    )
    .await;
    assert_eq!(res.code(), "reset_invalid");

    // Смена пароля в настройках требует нынешний
    let res = app
        .post(
            "/api/auth/account/password",
            json!({ "current": "not my password", "password": "bob's newer password" }),
            &bob,
        )
        .await;
    assert_eq!(res.status, StatusCode::UNAUTHORIZED);
    let res = app
        .post(
            "/api/auth/account/password",
            json!({ "current": "bob's new password", "password": "bob's newer password" }),
            &bob,
        )
        .await;
    assert_eq!(res.status, StatusCode::NO_CONTENT, "{}", res.text);
    assert_eq!(
        login(&app, "bob", "bob's newer password", "203.0.113.31")
            .await
            .status,
        StatusCode::OK
    );

    // Сброс админом закрывает все сессии человека
    let res = app
        .post(
            &format!("/api/admin/users/{user_id}/reset"),
            json!({}),
            &admin,
        )
        .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    let token = res.body["url"]
        .as_str()
        .unwrap()
        .rsplit('/')
        .next()
        .unwrap()
        .to_string();
    let res = post(
        &app,
        &format!("/api/auth/reset/{token}"),
        json!({ "password": "bob's reset password" }),
        &[],
        "203.0.113.32",
    )
    .await;
    assert_eq!(res.status, StatusCode::OK);
    assert_eq!(
        app.get("/api/auth/me", &bob).await.status,
        StatusCode::UNAUTHORIZED
    );

    // «Доступ»: у кого какой вход
    let users = app.get("/api/admin/users", &admin).await;
    let bob = users.body["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|u| u["username"] == "bob")
        .unwrap()
        .clone();
    assert_eq!(bob["hasPassword"], true);
    assert_eq!(bob["providers"], json!([]));

    let overview = app.get("/api/admin/auth", &admin).await;
    assert_eq!(overview.body["local"], true);
    assert_eq!(overview.body["providers"][0]["id"], "dalyrest");
    assert_eq!(
        overview.body["providers"][0]["callbackUrl"],
        format!("{PUBLIC_URL}/api/auth/oidc/callback")
    );
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn app_signs_in_through_the_login_screen(db: PgPool) {
    let app = local_app(db).await;
    set_up(&app).await;

    let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(VERIFIER.as_bytes()));
    let start = format!(
        "/api/auth/app/start?redirect_uri=dalywave%3A%2F%2Fauth&code_challenge={challenge}\
         &code_challenge_method=S256&state=s1&device_name=iPhone&platform=ios"
    );
    // Способов входа несколько: системный браузер открывает экран входа сайта
    let res = app.get(&start, &Auth::None).await;
    assert_eq!(res.status, StatusCode::FOUND);
    assert_eq!(res.location(), format!("{PUBLIC_URL}/login?app=1"));
    let app_cookie = res.cookie("dw_app").expect("dw_app cookie");

    // Без cookie приложения код не выдаётся
    let res = post(
        &app,
        "/api/auth/local/login",
        json!({ "username": "owner", "password": OWNER_PASSWORD, "app": true }),
        &[],
        "203.0.113.40",
    )
    .await;
    assert_eq!(res.code(), "app_expired");

    let res = post(
        &app,
        "/api/auth/local/login",
        json!({ "username": "owner", "password": OWNER_PASSWORD, "app": true }),
        &[&app_cookie],
        "203.0.113.40",
    )
    .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    assert!(res.cookie("dw_session").is_none(), "сессия сайта не нужна");
    let back = res.body["redirect"].as_str().unwrap().to_string();
    let q = query(&back);
    assert!(back.starts_with("dalywave://auth?code="), "{back}");
    assert_eq!(q["state"], "s1");

    let res = app
        .call(
            Method::POST,
            "/api/auth/app/token",
            Some(json!({ "code": q["code"], "codeVerifier": VERIFIER })),
            &Auth::None,
        )
        .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    let token = res.body["token"].as_str().unwrap().to_string();
    let me = app.get("/api/auth/me", &Auth::Bearer(token)).await;
    assert_eq!(me.body["via"], "device");
    let method: String =
        sqlx::query_scalar("SELECT auth_method FROM sessions WHERE device_id IS NOT NULL")
            .fetch_one(&app.state.db)
            .await
            .unwrap();
    assert_eq!(method, "local");

    // Тот же экран: вход через провайдера тоже отдаёт код приложению
    let res = app.get(&start, &Auth::None).await;
    let app_cookie = res.cookie("dw_app").unwrap();
    let res = get_with(&app, "/api/auth/oidc/dalyrest/start?app=1", &[&app_cookie]).await;
    assert_eq!(res.status, StatusCode::FOUND, "{}", res.text);
    let oidc_cookie = res.cookie("dw_oidc").unwrap();
    let (code, state) = app.provider.approve(&res.location(), &Person::new("ann"));
    let res = app.callback(&code, &state, Some(&oidc_cookie)).await;
    assert!(
        res.location().starts_with("dalywave://auth?code="),
        "{}",
        res.location()
    );
}
