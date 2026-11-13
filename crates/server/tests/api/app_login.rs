use std::collections::HashMap;

use axum::http::{Method, StatusCode};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use serde_json::json;
use sha2::{Digest, Sha256};
use sqlx::PgPool;

use crate::{
    common::{Auth, PUBLIC_URL, Resp, TestApp},
    mock::Person,
};

const VERIFIER: &str = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk-app-verifier";

fn challenge(verifier: &str) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
}

fn query(url: &str) -> HashMap<String, String> {
    url::Url::parse(url)
        .unwrap()
        .query_pairs()
        .into_owned()
        .collect()
}

fn start_uri(redirect_uri: &str) -> String {
    format!(
        "/api/auth/app/start?redirect_uri={}&code_challenge={}&code_challenge_method=S256&state=app-state-1\
         &device_name=MacBook%20%D0%97%D0%B0%D1%85%D0%B0%D1%80%D0%B0&platform=macos",
        url::form_urlencoded::byte_serialize(redirect_uri.as_bytes()).collect::<String>(),
        challenge(VERIFIER),
    )
}

/// Приложение открыло системный браузер, человек вошёл на daly.rest. Возвращает адрес, куда браузер
/// вернулся в приложение.
async fn browser_round_trip(app: &TestApp, redirect_uri: &str, person: &Person) -> String {
    let (authorize, cookie) = app.start(&start_uri(redirect_uri)).await;
    let (code, state) = app.provider.approve(&authorize, person);
    let res = app.callback(&code, &state, Some(&cookie)).await;
    assert_eq!(res.status, StatusCode::FOUND);
    assert!(
        res.cookie("dw_session").is_none(),
        "браузеру сессия сайта не нужна"
    );
    res.location()
}

async fn exchange(app: &TestApp, code: &str, verifier: &str) -> Resp {
    let body = json!({ "code": code, "codeVerifier": verifier, "appVersion": "0.1.0" });
    app.call(Method::POST, "/api/auth/app/token", Some(body), &Auth::None)
        .await
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn app_gets_a_device_token(db: PgPool) {
    let app = TestApp::new(db).await;
    let back = browser_round_trip(&app, "dalywave://auth", &Person::new("ann")).await;
    assert!(back.starts_with("dalywave://auth?code="), "{back}");
    let q = query(&back);
    assert_eq!(q["state"], "app-state-1");

    let res = exchange(&app, &q["code"], VERIFIER).await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    let token = res.body["token"].as_str().unwrap().to_string();
    assert!(token.starts_with("dwd_"));
    assert_eq!(res.body["device"]["name"], "MacBook Захара");
    assert_eq!(res.body["device"]["platform"], "macos");
    assert_eq!(res.body["device"]["appVersion"], "0.1.0");
    assert_eq!(res.body["user"]["email"], "ann@example.com");

    let bearer = Auth::Bearer(token.clone());
    let me = app.get("/api/auth/me", &bearer).await;
    assert_eq!(me.status, StatusCode::OK, "{}", me.text);
    assert_eq!(me.body["via"], "device");
    assert_eq!(me.body["device"]["id"], res.body["device"]["id"]);
    // Токен приложения не годится как cookie сайта и наоборот
    assert_eq!(
        app.get("/api/auth/me", &Auth::Cookie(format!("dw_session={token}")))
            .await
            .status,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        app.get("/api/auth/me", &Auth::Bearer("dws_whatever".into()))
            .await
            .status,
        StatusCode::UNAUTHORIZED
    );

    // Изменяющие запросы с токеном не требуют Origin
    let res = app
        .call(Method::POST, "/api/auth/logout", None, &bearer)
        .await;
    assert_eq!(res.status, StatusCode::NO_CONTENT);
    assert_eq!(
        app.get("/api/auth/me", &bearer).await.status,
        StatusCode::UNAUTHORIZED
    );
    let revoked: bool = sqlx::query_scalar("SELECT revoked_at IS NOT NULL FROM devices")
        .fetch_one(&app.state.db)
        .await
        .unwrap();
    assert!(revoked, "выход приложения отзывает и устройство");
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn loopback_redirect_and_bad_requests(db: PgPool) {
    let app = TestApp::new(db).await;
    let back =
        browser_round_trip(&app, "http://127.0.0.1:53682/callback", &Person::new("ann")).await;
    assert!(
        back.starts_with("http://127.0.0.1:53682/callback?code="),
        "{back}"
    );

    // Чужой адрес возврата не получает даже ошибку
    for bad in [
        "https://evil.example/callback",
        "http://127.0.0.1/callback",
        "dalywave://other",
    ] {
        let res = app.get(&start_uri(bad), &Auth::None).await;
        assert_eq!(res.status, StatusCode::BAD_REQUEST, "{bad}");
    }
    // Остальные ошибки уходят приложению
    let res = app
        .get(
            &start_uri("dalywave://auth").replace("S256", "plain"),
            &Auth::None,
        )
        .await;
    assert_eq!(res.status, StatusCode::FOUND);
    let q = query(&res.location());
    assert_eq!(q["error"], "invalid_request");
    assert_eq!(q["state"], "app-state-1");
    let res = app
        .get(
            &start_uri("dalywave://auth").replace("&platform=macos", ""),
            &Auth::None,
        )
        .await;
    assert_eq!(query(&res.location())["error"], "invalid_request");
    let res = app
        .get(
            &start_uri("dalywave://auth").replace("platform=macos", "platform=playstation"),
            &Auth::None,
        )
        .await;
    assert_eq!(res.status, StatusCode::BAD_REQUEST);
    assert_eq!(res.code(), "bad_request");
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn wrong_pkce_burns_the_code(db: PgPool) {
    let app = TestApp::new(db).await;
    let back = browser_round_trip(&app, "dalywave://auth", &Person::new("ann")).await;
    let code = query(&back)["code"].clone();

    let res = exchange(
        &app,
        &code,
        "another-verifier-another-verifier-another-verifier",
    )
    .await;
    assert_eq!(res.status, StatusCode::BAD_REQUEST);
    assert_eq!(res.code(), "invalid_grant");
    // После неудачной попытки код погашен: перебирать verifier бесполезно
    let res = exchange(&app, &code, VERIFIER).await;
    assert_eq!(res.status, StatusCode::BAD_REQUEST);
    assert_eq!(res.code(), "invalid_grant");

    let res = exchange(&app, "never-issued", VERIFIER).await;
    assert_eq!(res.code(), "invalid_grant");
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn reused_code_revokes_the_token(db: PgPool) {
    let app = TestApp::new(db).await;
    let back = browser_round_trip(&app, "dalywave://auth", &Person::new("ann")).await;
    let code = query(&back)["code"].clone();
    let token = exchange(&app, &code, VERIFIER).await.body["token"]
        .as_str()
        .unwrap()
        .to_string();
    let bearer = Auth::Bearer(token);
    assert_eq!(
        app.get("/api/auth/me", &bearer).await.status,
        StatusCode::OK
    );

    // Код предъявили второй раз: кто-то его перехватил, выданный токен больше не действует
    let res = exchange(&app, &code, VERIFIER).await;
    assert_eq!(res.status, StatusCode::BAD_REQUEST);
    assert_eq!(
        app.get("/api/auth/me", &bearer).await.status,
        StatusCode::UNAUTHORIZED
    );
    assert!(
        app.audit_kinds()
            .await
            .contains(&"auth.app_code_reused".to_string())
    );
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn app_errors_go_back_to_the_app(db: PgPool) {
    let app = TestApp::new(db).await;
    let (authorize, cookie) = app.start(&start_uri("dalywave://auth")).await;
    let (code, _) = app.provider.approve(&authorize, &Person::new("ann"));
    let res = app.callback(&code, "forged-state", Some(&cookie)).await;
    let q = query(&res.location());
    assert!(res.location().starts_with("dalywave://auth?"));
    assert_eq!(q["error"], "state");
    assert_eq!(q["state"], "app-state-1");

    // Заблокированный человек получает отказ в приложении
    let ann = Auth::Cookie(app.login(&Person::new("ann")).await);
    let id = app.user_id(&ann).await;
    sqlx::query("UPDATE users SET disabled = true WHERE id = $1::uuid")
        .bind(&id)
        .execute(&app.state.db)
        .await
        .unwrap();
    let back = browser_round_trip(&app, "dalywave://auth", &Person::new("ann")).await;
    assert_eq!(query(&back)["error"], "disabled");

    // Сайт после этого ведёт себя как обычно
    let res = app
        .get("/api/auth/oidc/callback?error=access_denied", &Auth::None)
        .await;
    assert_eq!(
        res.location(),
        format!("{PUBLIC_URL}/login?error=cancelled")
    );
}

/// Плагин Stream Deck входит как приложение: браузер, daly.rest, код на свой порт (loopback) и обмен
/// с PKCE. Дальше он — пульт в «Где играть» с платформой streamdeck.
#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn stream_deck_signs_in_through_the_browser(db: PgPool) {
    let app = TestApp::new(db).await;
    // Слушатель с ролью: пульту нужен доступ к «Где играть»
    app.listener("ann").await;
    let redirect = "http://127.0.0.1:53682/callback";
    let start = format!(
        "/api/auth/app/start?redirect_uri={}&code_challenge={}&code_challenge_method=S256&state=deck-1\
         &device_name=Stream%20Deck%20%C2%B7%20MacBook&platform=streamdeck",
        url::form_urlencoded::byte_serialize(redirect.as_bytes()).collect::<String>(),
        challenge(VERIFIER),
    );
    let (authorize, cookie) = app.start(&start).await;
    let (code, state) = app.provider.approve(&authorize, &Person::new("ann"));
    let back = app.callback(&code, &state, Some(&cookie)).await.location();
    assert!(back.starts_with(&format!("{redirect}?code=")), "{back}");
    let q = query(&back);
    assert_eq!(q["state"], "deck-1");

    let res = exchange(&app, &q["code"], VERIFIER).await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    assert_eq!(res.body["device"]["platform"], "streamdeck");
    assert_eq!(res.body["device"]["name"], "Stream Deck · MacBook");
    let deck = Auth::Bearer(res.body["token"].as_str().unwrap().to_string());

    // Пульт: в «Где играть» — с платформой streamdeck, только как пульт, имя — из входа
    let res = app
        .post(
            "/api/devices/remote",
            json!({
                "deviceId": "streamdeck-1",
                "name": "Stream Deck",
                "kind": "desktop",
                "platform": "streamdeck",
                "capabilities": { "remote": true },
            }),
            &deck,
        )
        .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    let row = res.body["devices"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["id"] == "streamdeck-1")
        .expect("Stream Deck в списке")
        .clone();
    assert_eq!(row["platform"], "streamdeck");
    assert_eq!(row["name"], "Stream Deck · MacBook");
    assert_eq!(row["capabilities"]["remote"], true);

    // «Отключить» в плагине закрывает его сессию
    let out = app.post("/api/auth/logout", json!({}), &deck).await;
    assert!(out.status.is_success(), "{}", out.text);
    assert_eq!(
        app.get("/api/devices", &deck).await.status,
        StatusCode::UNAUTHORIZED
    );
}
