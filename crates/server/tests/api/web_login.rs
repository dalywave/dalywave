use axum::http::{Method, StatusCode};
use axum::response::IntoResponse;
use axum_extra::extract::cookie::{Cookie, PrivateCookieJar};
use dalywave_server::routes::auth::{OIDC_COOKIE, Pending};
use serde_json::json;
use sqlx::PgPool;

use crate::{
    common::{Auth, PUBLIC_URL, TestApp},
    mock::Person,
};

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn full_web_flow(db: PgPool) {
    let app = TestApp::new(db).await;

    let providers = app.get("/api/auth/providers", &Auth::None).await;
    let dalyrest = json!({ "id": "dalyrest", "title": "daly.rest", "icon": null });
    assert_eq!(
        providers.body,
        json!({ "oidc": dalyrest, "providers": [dalyrest], "password": false, "local": false, "setup": false })
    );

    let res = app
        .get(
            "/api/auth/oidc/start?next=/library%3Ftab%3Dalbums",
            &Auth::None,
        )
        .await;
    assert_eq!(res.status, StatusCode::FOUND);
    let authorize = res.location();
    let url = url::Url::parse(&authorize).unwrap();
    let q: std::collections::HashMap<String, String> = url.query_pairs().into_owned().collect();
    assert_eq!(
        q["redirect_uri"],
        format!("{PUBLIC_URL}/api/auth/oidc/callback")
    );
    assert_eq!(q["code_challenge"].len(), 43);
    assert!(!q["state"].is_empty() && !q["nonce"].is_empty());
    let oidc_cookie = res.set_cookie("dw_oidc").unwrap();
    for attr in ["HttpOnly", "SameSite=Lax", "Path=/api/auth", "Max-Age=600"] {
        assert!(oidc_cookie.contains(attr), "{oidc_cookie}");
    }
    assert!(
        !oidc_cookie.contains("Secure"),
        "по http Secure не ставится"
    );
    // state и verifier зашифрованы: в cookie их не прочесть
    assert!(!oidc_cookie.contains(&q["state"]));

    let (code, state) = app.provider.approve(&authorize, &Person::new("ann"));
    let res = app
        .callback(&code, &state, res.cookie("dw_oidc").as_deref())
        .await;
    assert_eq!(res.status, StatusCode::FOUND);
    assert_eq!(res.location(), format!("{PUBLIC_URL}/library?tab=albums"));
    let session = res.set_cookie("dw_session").unwrap();
    for attr in ["HttpOnly", "SameSite=Lax", "Path=/"] {
        assert!(session.contains(attr), "{session}");
    }
    assert!(
        res.set_cookie("dw_oidc").unwrap().contains("Max-Age=0"),
        "cookie входа стирается"
    );
    let cookie = Auth::Cookie(res.cookie("dw_session").unwrap());

    let me = app.get("/api/auth/me", &cookie).await;
    assert_eq!(me.status, StatusCode::OK, "{}", me.text);
    assert_eq!(me.body["user"]["name"], "Person ann");
    assert_eq!(me.body["user"]["email"], "ann@example.com");
    assert_eq!(
        me.body["user"]["avatarUrl"],
        "https://cdn.example.com/ann.png"
    );
    assert_eq!(me.body["roles"], json!([]));
    assert_eq!(me.body["permissions"], json!([]));
    assert_eq!(me.body["via"], "oidc");
    assert!(me.body["device"].is_null());

    // Без ролей — «ждём доступа»: всё, кроме /api/auth, отвечает no_access
    let res = app.get("/api/admin/users", &cookie).await;
    assert_eq!(res.status, StatusCode::FORBIDDEN);
    assert_eq!(res.code(), "no_access");
    assert_eq!(
        app.get("/api/admin/users", &Auth::None).await.status,
        StatusCode::UNAUTHORIZED
    );

    // Код одноразовый: повтор того же возврата не открывает вторую сессию
    let again = app.callback(&code, &state, None).await;
    assert_eq!(
        again.location(),
        format!("{PUBLIC_URL}/login?error=expired")
    );

    let kinds = app.audit_kinds().await;
    assert_eq!(kinds, ["user.created", "auth.sign_in"]);
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn next_must_be_local(db: PgPool) {
    let app = TestApp::new(db).await;
    for next in [
        "//evil.example",
        "https://evil.example/x",
        "/%5Cevil.example",
        "library",
    ] {
        let res = app
            .get(&format!("/api/auth/oidc/start?next={next}"), &Auth::None)
            .await;
        assert_eq!(res.status, StatusCode::BAD_REQUEST, "{next}");
        assert_eq!(res.code(), "bad_request");
    }
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn state_and_nonce_must_match(db: PgPool) {
    let app = TestApp::new(db).await;

    // Возврат в другом браузере: state из адреса не совпадает с cookie
    let (url_a, _cookie_a) = app.start("/api/auth/oidc/start").await;
    let (_url_b, cookie_b) = app.start("/api/auth/oidc/start").await;
    let (code, state) = app.provider.approve(&url_a, &Person::new("ann"));
    let res = app.callback(&code, &state, Some(&cookie_b)).await;
    assert_eq!(res.location(), format!("{PUBLIC_URL}/login?error=state"));
    assert!(res.cookie("dw_session").is_none());

    // id_token с чужим nonce
    let (url, cookie) = app.start("/api/auth/oidc/start").await;
    app.provider.override_nonce("someone-elses-nonce");
    let (code, state) = app.provider.approve(&url, &Person::new("ann"));
    let res = app.callback(&code, &state, Some(&cookie)).await;
    assert_eq!(res.location(), format!("{PUBLIC_URL}/login?error=state"));
    assert!(res.cookie("dw_session").is_none());

    // Отмена на daly.rest
    let res = app
        .get(
            "/api/auth/oidc/callback?error=access_denied&state=x",
            &Auth::None,
        )
        .await;
    assert_eq!(
        res.location(),
        format!("{PUBLIC_URL}/login?error=cancelled")
    );

    let users: i64 = sqlx::query_scalar("SELECT count(*) FROM users")
        .fetch_one(&app.state.db)
        .await
        .unwrap();
    assert_eq!(users, 0);
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn expired_state(db: PgPool) {
    let app = TestApp::new(db).await;
    let (url, cookie) = app.start("/api/auth/oidc/start?next=/search").await;
    let (code, state) = app.provider.approve(&url, &Person::new("ann"));

    // Cookie живёт 10 минут: подделываем её возраст тем же ключом, что у сервера
    let key = app.state.cookie_key.clone();
    let headers =
        axum::http::HeaderMap::from_iter([(axum::http::header::COOKIE, cookie.parse().unwrap())]);
    let jar = PrivateCookieJar::from_headers(&headers, key.clone());
    let mut pending: Pending = serde_json::from_str(jar.get(OIDC_COOKIE).unwrap().value()).unwrap();
    pending.exp = chrono::Utc::now().timestamp() - 1;
    let old = PrivateCookieJar::new(key).add(Cookie::new(
        OIDC_COOKIE,
        serde_json::to_string(&pending).unwrap(),
    ));
    let set = old
        .into_response()
        .headers()
        .get(axum::http::header::SET_COOKIE)
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    let expired_cookie = set.split(';').next().unwrap().to_string();

    let res = app.callback(&code, &state, Some(&expired_cookie)).await;
    assert_eq!(
        res.location(),
        format!("{PUBLIC_URL}/login?error=expired&next=%2Fsearch")
    );

    // Без cookie вовсе: тоже просим начать заново
    let res = app.callback(&code, &state, None).await;
    assert_eq!(res.location(), format!("{PUBLIC_URL}/login?error=expired"));
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn upstream_admin_comes_and_goes(db: PgPool) {
    let app = TestApp::new(db).await;
    let cookie = Auth::Cookie(app.login(&Person::admin("boss")).await);
    let me = app.get("/api/auth/me", &cookie).await;
    assert_eq!(me.body["roles"], json!(["admin"]));
    assert_eq!(me.body["permissions"], json!(["play", "library", "admin"]));
    let users = app.get("/api/admin/users", &cookie).await;
    assert_eq!(users.status, StatusCode::OK);
    assert_eq!(users.body["items"][0]["idpAdmin"], true);

    // На daly.rest роль сняли: при следующем входе она снимается и здесь, остальные роли остаются
    let id = app.user_id(&cookie).await;
    sqlx::query("UPDATE users SET roles = '{admin,listener}' WHERE id = $1::uuid")
        .bind(&id)
        .execute(&app.state.db)
        .await
        .unwrap();
    let cookie = Auth::Cookie(app.login(&Person::new("boss")).await);
    let me = app.get("/api/auth/me", &cookie).await;
    assert_eq!(me.body["roles"], json!(["listener"]));
    assert_eq!(
        app.get("/api/admin/users", &cookie).await.code(),
        "forbidden"
    );

    // Выданный вручную admin провайдер не трогает
    sqlx::query("UPDATE users SET roles = '{admin}', idp_admin = false WHERE id = $1::uuid")
        .bind(&id)
        .execute(&app.state.db)
        .await
        .unwrap();
    let cookie = Auth::Cookie(app.login(&Person::new("boss")).await);
    assert_eq!(
        app.get("/api/auth/me", &cookie).await.body["roles"],
        json!(["admin"])
    );

    let changes: Vec<serde_json::Value> = sqlx::query_scalar(
        "SELECT payload FROM audit_events WHERE kind = 'user.roles_changed' ORDER BY id",
    )
    .fetch_all(&app.state.db)
    .await
    .unwrap();
    assert_eq!(
        changes[0],
        json!({ "from": [], "to": ["admin"], "reason": "provider" })
    );
    assert_eq!(
        changes[1],
        json!({ "from": ["admin", "listener"], "to": ["listener"], "reason": "provider" })
    );
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn roles_from_userinfo_when_id_token_has_none(db: PgPool) {
    let app = TestApp::new(db).await;
    app.provider.roles_in_id_token(false);
    let cookie = Auth::Cookie(app.login(&Person::admin("boss")).await);
    assert_eq!(
        app.get("/api/auth/me", &cookie).await.body["roles"],
        json!(["admin"])
    );
    assert_eq!(app.provider.userinfo_calls(), 1);

    // Роли в id_token есть — userinfo не нужен
    app.provider.roles_in_id_token(true);
    app.login(&Person::admin("boss")).await;
    assert_eq!(app.provider.userinfo_calls(), 1);
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn linking_by_verified_email_only(db: PgPool) {
    let app = TestApp::with(db, |c| {
        c.admin_emails = vec!["ann@example.com".into(), "eve@example.com".into()]
    })
    .await;
    let first = Auth::Cookie(app.login(&Person::new("ann")).await);
    let ann = app.user_id(&first).await;
    // Почта из ADMIN_EMAILS, подтверждённая провайдером, даёт admin
    assert_eq!(
        app.get("/api/auth/me", &first).await.body["roles"],
        json!(["admin"])
    );

    // Тот же человек с другим sub, но с той же подтверждённой почтой — тот же пользователь
    let mut moved = Person::new("ann-2");
    moved.email = Some("ANN@example.com".into());
    let second = Auth::Cookie(app.login(&moved).await);
    assert_eq!(app.user_id(&second).await, ann);

    // Неподтверждённая почта никого не связывает и админом не делает
    let mut impostor = Person::new("ann-3");
    impostor.email = Some("ann@example.com".into());
    impostor.email_verified = false;
    let third = Auth::Cookie(app.login(&impostor).await);
    let me = app.get("/api/auth/me", &third).await;
    assert_ne!(me.body["user"]["id"], ann.as_str());
    assert!(
        me.body["user"]["email"].is_null(),
        "чужая почта не достаётся"
    );
    assert_eq!(me.body["roles"], json!([]));

    let mut eve = Person::new("eve");
    eve.email_verified = false;
    let eve = Auth::Cookie(app.login(&eve).await);
    let me = app.get("/api/auth/me", &eve).await;
    assert_eq!(me.body["user"]["email"], "eve@example.com");
    assert_eq!(
        me.body["roles"],
        json!([]),
        "неподтверждённая почта из ADMIN_EMAILS не даёт admin"
    );

    // Подтверждённый вход забирает почту у неподтверждённого
    let mut real_eve = Person::new("eve-real");
    real_eve.email = Some("eve@example.com".into());
    let real = Auth::Cookie(app.login(&real_eve).await);
    let me = app.get("/api/auth/me", &real).await;
    assert_eq!(me.body["user"]["email"], "eve@example.com");
    assert_eq!(me.body["roles"], json!(["admin"]));
    let left: Option<String> = sqlx::query_scalar("SELECT email FROM users WHERE subject = 'eve'")
        .fetch_one(&app.state.db)
        .await
        .unwrap();
    assert!(left.is_none());

    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM users")
        .fetch_one(&app.state.db)
        .await
        .unwrap();
    assert_eq!(count, 4);
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn disabled_user_cannot_sign_in(db: PgPool) {
    let app = TestApp::new(db).await;
    let (_, admin) = app.admin("boss").await;
    let user = Auth::Cookie(app.login(&Person::new("ann")).await);
    let id = app.user_id(&user).await;

    let res = app
        .call(
            Method::PATCH,
            &format!("/api/admin/users/{id}"),
            Some(json!({ "disabled": true })),
            &admin,
        )
        .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    assert_eq!(res.body["disabled"], true);
    // Блокировка действует сразу
    assert_eq!(
        app.get("/api/auth/me", &user).await.status,
        StatusCode::UNAUTHORIZED
    );

    let (url, cookie) = app.start("/api/auth/oidc/start").await;
    let (code, state) = app.provider.approve(&url, &Person::new("ann"));
    let res = app.callback(&code, &state, Some(&cookie)).await;
    assert_eq!(res.location(), format!("{PUBLIC_URL}/login?error=disabled"));
    assert!(res.cookie("dw_session").is_none());
    assert!(
        app.audit_kinds()
            .await
            .contains(&"auth.sign_in_denied".to_string())
    );

    let res = app
        .call(
            Method::PATCH,
            &format!("/api/admin/users/{id}"),
            Some(json!({ "disabled": false })),
            &admin,
        )
        .await;
    assert_eq!(res.body["disabled"], false);
    app.login(&Person::new("ann")).await;
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn logout_revokes_the_session(db: PgPool) {
    let app = TestApp::new(db).await;
    let cookie = Auth::Cookie(app.login(&Person::new("ann")).await);
    let res = app
        .call(Method::POST, "/api/auth/logout", None, &cookie)
        .await;
    assert_eq!(res.status, StatusCode::NO_CONTENT);
    assert!(res.set_cookie("dw_session").unwrap().contains("Max-Age=0"));
    assert_eq!(
        app.get("/api/auth/me", &cookie).await.status,
        StatusCode::UNAUTHORIZED
    );

    // Новый вход в том же браузере закрывает прежнюю сессию
    let first = app.login(&Person::new("ann")).await;
    let (url, oidc) = app.start("/api/auth/oidc/start").await;
    let (code, state) = app.provider.approve(&url, &Person::new("ann"));
    let res = app
        .callback(&code, &state, Some(&format!("{oidc}; {first}")))
        .await;
    assert!(res.cookie("dw_session").is_some());
    assert_eq!(
        app.get("/api/auth/me", &Auth::Cookie(first)).await.status,
        StatusCode::UNAUTHORIZED
    );

    // Подделанная cookie — просто нет сессии
    let forged = Auth::Cookie("dw_session=dws_forged".into());
    assert_eq!(
        app.get("/api/auth/me", &forged).await.code(),
        "unauthorized"
    );
}
