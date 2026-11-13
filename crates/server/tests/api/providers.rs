//! Несколько провайдеров OIDC: свои адреса возврата, подписи RS256/ES256/EdDSA, client_secret_post и
//! публичный клиент, claim групп, связывание по почте только у доверенных, приглашения, привязка входа
//! в настройках. Прежний daly.rest (старые переменные и старый адрес возврата) — в `web_login.rs`.

use axum::http::StatusCode;
use serde_json::json;
use sqlx::PgPool;

use crate::{
    accounts::{get_with, post, set_up},
    common::{Auth, PUBLIC_URL, Resp, TestApp},
    mock::{Alg, MockOptions, MockProvider, Person},
};

struct Extra {
    authentik: MockProvider,
    pocket: MockProvider,
}

/// daly.rest (ES256, прежние переменные) + Authentik (RS256, группы, client_secret_post)
/// + Pocket ID (EdDSA, публичный клиент).
async fn three_providers(
    db: PgPool,
    tweak: impl FnOnce(&mut dalywave_server::Config),
) -> (TestApp, Extra) {
    let authentik = MockProvider::start_with(MockOptions {
        alg: Alg::Rs256,
        roles_claim: "groups".into(),
        post_only: true,
        public: false,
    })
    .await;
    let pocket = MockProvider::start_with(MockOptions {
        alg: Alg::EdDsa,
        public: true,
        ..MockOptions::default()
    })
    .await;
    let mut ak = authentik.provider_config("authentik", "Authentik");
    ak.admin_values = vec!["wave-admins".into()];
    let pk = pocket.provider_config("pocket-id", "Pocket ID");
    let app = TestApp::with(db, |c| {
        c.oidc.push(ak);
        c.oidc.push(pk);
        tweak(c);
    })
    .await;
    (app, Extra { authentik, pocket })
}

async fn provider_callback(app: &TestApp, id: &str, code: &str, state: &str, cookie: &str) -> Resp {
    get_with(
        app,
        &format!("/api/auth/oidc/{id}/callback?code={code}&state={state}"),
        &[cookie],
    )
    .await
}

/// Вход через провайдера `id`. Возвращает ответ возврата.
async fn sign_in(
    app: &TestApp,
    mock: &MockProvider,
    id: &str,
    person: &Person,
    extra: &str,
) -> Resp {
    let (url, cookie) = app
        .start(&format!("/api/auth/oidc/{id}/start{extra}"))
        .await;
    let q: std::collections::HashMap<String, String> = url::Url::parse(&url)
        .unwrap()
        .query_pairs()
        .into_owned()
        .collect();
    assert_eq!(
        q["redirect_uri"],
        format!("{PUBLIC_URL}/api/auth/oidc/{id}/callback")
    );
    let (code, state) = mock.approve(&url, person);
    provider_callback(app, id, &code, &state, &cookie).await
}

fn session(res: &Resp) -> Auth {
    assert_eq!(res.status, StatusCode::FOUND);
    assert_eq!(
        res.location(),
        format!("{PUBLIC_URL}/"),
        "{}",
        res.location()
    );
    Auth::Cookie(res.cookie("dw_session").expect("dw_session"))
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn several_providers_sign_in(db: PgPool) {
    let (app, x) = three_providers(db, |_| {}).await;
    let providers = app.get("/api/auth/providers", &Auth::None).await;
    let ids: Vec<&str> = providers.body["providers"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, ["dalyrest", "authentik", "pocket-id"]);
    assert_eq!(
        providers.body["oidc"]["id"], "dalyrest",
        "для прежних приложений"
    );

    // Authentik: RS256, секрет в теле, админ по группе
    let mut boss = Person::new("boss");
    boss.roles = vec!["wave-admins".into(), "staff".into()];
    let res = sign_in(&app, &x.authentik, "authentik", &boss, "").await;
    let cookie = session(&res);
    let me = app.get("/api/auth/me", &cookie).await;
    assert_eq!(me.body["roles"], json!(["admin"]));
    assert_eq!(me.body["via"], "oidc");

    // Pocket ID: EdDSA, публичный клиент; новый человек ждёт доступа
    let res = sign_in(&app, &x.pocket, "pocket-id", &Person::new("ann"), "").await;
    let ann = session(&res);
    let me = app.get("/api/auth/me", &ann).await;
    assert_eq!(me.body["roles"], json!([]));
    assert_eq!(me.body["user"]["name"], "Person ann");

    // daly.rest: прежний вход живёт, роль admin из claim roles
    let cookie_dr = Auth::Cookie(app.login(&Person::admin("owner")).await);
    assert_eq!(
        app.get("/api/auth/me", &cookie_dr).await.body["roles"],
        json!(["admin"])
    );

    // «Доступ»: источник входа каждого
    let users = app.get("/api/admin/users", &cookie).await;
    let mut sources: Vec<String> = users.body["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|u| u["providers"][0].as_str().unwrap().to_string())
        .collect();
    sources.sort();
    assert_eq!(sources, ["Authentik", "Pocket ID", "daly.rest"]);
    let overview = app.get("/api/admin/auth", &cookie).await;
    assert_eq!(overview.body["providers"][1]["users"], 1);
    assert_eq!(
        overview.body["providers"][1]["callbackUrl"],
        format!("{PUBLIC_URL}/api/auth/oidc/authentik/callback")
    );
    assert_eq!(overview.body["providers"][1]["adminClaim"], "groups");
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn callbacks_stay_with_their_provider(db: PgPool) {
    let (app, x) = three_providers(db, |_| {}).await;
    // Вход начат у Authentik, а возврат пришёл на адрес Pocket ID или на прежний адрес daly.rest
    let (url, cookie) = app.start("/api/auth/oidc/authentik/start").await;
    let (code, state) = x.authentik.approve(&url, &Person::new("ann"));
    let res = provider_callback(&app, "pocket-id", &code, &state, &cookie).await;
    assert_eq!(res.location(), format!("{PUBLIC_URL}/login?error=state"));
    let res = app.callback(&code, &state, Some(&cookie)).await;
    assert_eq!(res.location(), format!("{PUBLIC_URL}/login?error=state"));
    // Неизвестный провайдер
    assert_eq!(
        app.get("/api/auth/oidc/nope/start", &Auth::None)
            .await
            .status,
        StatusCode::NOT_FOUND
    );
    // Прежний адрес начала входа ведёт к первому провайдеру — daly.rest
    let (url, _) = app.start("/api/auth/oidc/start").await;
    assert!(url.starts_with(&app.provider.issuer), "{url}");
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn email_links_only_trusted_providers(db: PgPool) {
    let (app, x) = three_providers(db, |_| {}).await;
    // Аня вошла через daly.rest: почта подтверждена доверенным провайдером
    let ann_dr = Auth::Cookie(app.login(&Person::new("ann")).await);
    let ann_id = app.user_id(&ann_dr).await;

    // Pocket ID присылает ту же «подтверждённую» почту, но ему не доверяем: это другой человек
    let mut fake = Person::new("pocket-ann");
    fake.email = Some("ann@example.com".into());
    let res = sign_in(&app, &x.pocket, "pocket-id", &fake, "").await;
    let other = session(&res);
    let me = app.get("/api/auth/me", &other).await;
    assert_ne!(me.body["user"]["id"], ann_id);
    assert!(
        me.body["user"]["email"].is_null(),
        "чужая почта не присваивается"
    );
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn trusted_email_links_accounts(db: PgPool) {
    let (app, x) = three_providers(db, |c| {
        let ak = c.oidc.iter_mut().find(|p| p.id == "authentik").unwrap();
        ak.trust_email = true;
    })
    .await;
    let ann_dr = Auth::Cookie(app.login(&Person::new("ann")).await);
    let ann_id = app.user_id(&ann_dr).await;
    let mut same = Person::new("ak-ann");
    same.email = Some("ann@example.com".into());
    let res = sign_in(&app, &x.authentik, "authentik", &same, "").await;
    let cookie = session(&res);
    assert_eq!(app.user_id(&cookie).await, ann_id);
    let account = app.get("/api/auth/account", &cookie).await;
    let titles: Vec<&str> = account.body["identities"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["title"].as_str().unwrap())
        .collect();
    assert_eq!(titles, ["daly.rest", "Authentik"]);
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn admin_belongs_to_the_granting_provider(db: PgPool) {
    let (app, x) = three_providers(db, |c| {
        let ak = c.oidc.iter_mut().find(|p| p.id == "authentik").unwrap();
        ak.trust_email = true;
    })
    .await;
    // admin от daly.rest
    let cookie = Auth::Cookie(app.login(&Person::admin("zak")).await);
    // Вход через Authentik без админской группы (та же почта — тот же человек) роль не снимает
    let mut zak = Person::new("ak-zak");
    zak.email = Some("zak@example.com".into());
    let res = sign_in(&app, &x.authentik, "authentik", &zak, "").await;
    let ak = session(&res);
    assert_eq!(app.user_id(&ak).await, app.user_id(&cookie).await);
    assert_eq!(
        app.get("/api/auth/me", &ak).await.body["roles"],
        json!(["admin"])
    );
    // daly.rest снял admin — снимается и здесь
    let cookie = Auth::Cookie(app.login(&Person::new("zak")).await);
    assert_eq!(
        app.get("/api/auth/me", &cookie).await.body["roles"],
        json!([])
    );
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn closed_provider_needs_an_invite(db: PgPool) {
    let (app, x) = three_providers(db, |c| {
        c.local_auth = true;
        let ak = c.oidc.iter_mut().find(|p| p.id == "authentik").unwrap();
        ak.auto_register = false;
    })
    .await;
    let admin = set_up(&app).await;
    let res = sign_in(&app, &x.authentik, "authentik", &Person::new("ann"), "").await;
    assert_eq!(
        res.location(),
        format!("{PUBLIC_URL}/login?error=not_registered")
    );

    let res = app
        .post(
            "/api/admin/invites",
            json!({ "roles": ["listener"] }),
            &admin,
        )
        .await;
    let token = res.body["url"]
        .as_str()
        .unwrap()
        .rsplit('/')
        .next()
        .unwrap()
        .to_string();
    let res = sign_in(
        &app,
        &x.authentik,
        "authentik",
        &Person::new("ann"),
        &format!("?invite={token}"),
    )
    .await;
    let ann = session(&res);
    assert_eq!(
        app.get("/api/auth/me", &ann).await.body["roles"],
        json!(["listener"])
    );
    // Приглашение использовано
    assert_eq!(
        app.get(&format!("/api/auth/invites/{token}"), &Auth::None)
            .await
            .status,
        StatusCode::NOT_FOUND
    );
    // Дальше Аня входит и без приглашения
    let res = sign_in(&app, &x.authentik, "authentik", &Person::new("ann"), "").await;
    session(&res);
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn link_and_unlink_in_settings(db: PgPool) {
    let (app, x) = three_providers(db, |c| c.local_auth = true).await;
    let owner = set_up(&app).await;
    let Auth::Cookie(owner_cookie) = &owner else {
        unreachable!()
    };

    let account = app.get("/api/auth/account", &owner).await;
    assert_eq!(account.body["username"], "owner");
    assert_eq!(account.body["hasPassword"], true);
    assert_eq!(account.body["identities"], json!([]));
    assert_eq!(account.body["linkable"].as_array().unwrap().len(), 3);

    // Привязка — только с сессией сайта
    assert_eq!(
        app.get("/api/auth/oidc/pocket-id/start?link=1", &Auth::None)
            .await
            .status,
        StatusCode::UNAUTHORIZED
    );
    let res = get_with(
        &app,
        "/api/auth/oidc/pocket-id/start?link=1",
        &[owner_cookie],
    )
    .await;
    let oidc_cookie = res.cookie("dw_oidc").unwrap();
    let (code, state) = x.pocket.approve(&res.location(), &Person::new("zak"));
    let res = provider_callback(&app, "pocket-id", &code, &state, &oidc_cookie).await;
    assert_eq!(
        res.location(),
        format!("{PUBLIC_URL}/settings?linked=pocket-id")
    );
    assert!(
        res.cookie("dw_session").is_none(),
        "сессия остаётся прежней"
    );

    // Теперь вход через Pocket ID — тот же владелец
    let res = sign_in(&app, &x.pocket, "pocket-id", &Person::new("zak"), "").await;
    let via_pocket = session(&res);
    assert_eq!(app.user_id(&via_pocket).await, app.user_id(&owner).await);

    // Чужой вход не привязать: он уже у другого человека
    let res = sign_in(&app, &x.authentik, "authentik", &Person::new("ann"), "").await;
    session(&res);
    let res = get_with(
        &app,
        "/api/auth/oidc/authentik/start?link=1",
        &[owner_cookie],
    )
    .await;
    let oidc_cookie = res.cookie("dw_oidc").unwrap();
    let (code, state) = x.authentik.approve(&res.location(), &Person::new("ann"));
    let res = provider_callback(&app, "authentik", &code, &state, &oidc_cookie).await;
    assert_eq!(
        res.location(),
        format!("{PUBLIC_URL}/settings?link_error=linked_elsewhere")
    );

    // Отвязать можно, пока остаётся пароль
    let account = app.get("/api/auth/account", &owner).await;
    let id = account.body["identities"][0]["id"]
        .as_str()
        .unwrap()
        .to_string();
    assert_eq!(account.body["identities"][0]["provider"], "pocket-id");
    assert_eq!(
        app.delete(&format!("/api/auth/account/identities/{id}"), &owner)
            .await
            .status,
        StatusCode::NO_CONTENT
    );
    let res = sign_in(&app, &x.pocket, "pocket-id", &Person::new("zak"), "").await;
    let again = session(&res);
    assert_ne!(
        app.user_id(&again).await,
        app.user_id(&owner).await,
        "отвязанный вход больше не ведёт в аккаунт"
    );

    // У человека только один вход через провайдера и нет пароля: отвязать нельзя
    let account = app.get("/api/auth/account", &again).await;
    let id = account.body["identities"][0]["id"]
        .as_str()
        .unwrap()
        .to_string();
    let res = app
        .delete(&format!("/api/auth/account/identities/{id}"), &again)
        .await;
    assert_eq!(res.status, StatusCode::CONFLICT);
    assert_eq!(res.code(), "last_sign_in_method");

    // Задать себе логин и пароль в настройках
    let res = app
        .post(
            "/api/auth/account/password",
            json!({ "username": "zak2", "password": "zak's password!" }),
            &again,
        )
        .await;
    assert_eq!(res.status, StatusCode::NO_CONTENT, "{}", res.text);
    let res = post(
        &app,
        "/api/auth/local/login",
        json!({ "username": "zak2", "password": "zak's password!" }),
        &[],
        "203.0.113.50",
    )
    .await;
    assert_eq!(res.status, StatusCode::OK);
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn in_flight_login_from_the_previous_version(db: PgPool) {
    use axum::response::IntoResponse;
    use axum_extra::extract::cookie::{Cookie, PrivateCookieJar};

    // Вход начат прежней версией: в cookie нет поля provider. После обновления он доигрывается
    let app = TestApp::new(db).await;
    let (url, cookie) = app.start("/api/auth/oidc/start").await;
    let key = app.state.cookie_key.clone();
    let headers =
        axum::http::HeaderMap::from_iter([(axum::http::header::COOKIE, cookie.parse().unwrap())]);
    let jar = PrivateCookieJar::from_headers(&headers, key.clone());
    let mut pending: serde_json::Value =
        serde_json::from_str(jar.get("dw_oidc").unwrap().value()).unwrap();
    for field in ["provider", "link", "invite"] {
        pending.as_object_mut().unwrap().remove(field);
    }
    let old = PrivateCookieJar::new(key).add(Cookie::new("dw_oidc", pending.to_string()));
    let set = old
        .into_response()
        .headers()
        .get(axum::http::header::SET_COOKIE)
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    let old_cookie = set.split(';').next().unwrap().to_string();
    let (code, state) = app.provider.approve(&url, &Person::new("ann"));
    let res = app.callback(&code, &state, Some(&old_cookie)).await;
    assert_eq!(res.location(), format!("{PUBLIC_URL}/"));
    assert!(res.cookie("dw_session").is_some());
}
