use axum::{
    body::Body,
    http::{Method, Request, StatusCode, header},
};
use serde_json::json;
use sqlx::PgPool;

use crate::common::{ADMIN_PASSWORD, Auth, PUBLIC_URL, Resp, TestApp};

async fn password(app: &TestApp, value: &str, ip: &str) -> Resp {
    let req = Request::builder()
        .method(Method::POST)
        .uri("/api/auth/password")
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::ORIGIN, PUBLIC_URL)
        .header("x-forwarded-for", ip)
        .body(Body::from(json!({ "password": value }).to_string()))
        .unwrap();
    app.send(req).await
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn password_is_off_by_default(db: PgPool) {
    let app = TestApp::new(db).await;
    let res = password(&app, ADMIN_PASSWORD, "203.0.113.1").await;
    assert_eq!(res.status, StatusCode::FORBIDDEN);
    assert_eq!(res.code(), "password_disabled");
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn password_creates_the_owner(db: PgPool) {
    let app = TestApp::with(db, |c| c.admin_password = Some(ADMIN_PASSWORD.into())).await;
    assert_eq!(
        app.get("/api/auth/providers", &Auth::None).await.body["password"],
        true
    );

    let res = password(
        &app,
        "wrong password, wrong password, wrong!",
        "203.0.113.1",
    )
    .await;
    assert_eq!(res.status, StatusCode::UNAUTHORIZED);
    assert_eq!(res.code(), "wrong_password");

    let res = password(&app, ADMIN_PASSWORD, "203.0.113.1").await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    assert_eq!(res.body["via"], "password");
    assert_eq!(res.body["user"]["name"], "Владелец");
    assert_eq!(res.body["roles"], json!(["admin"]));
    let cookie = Auth::Cookie(res.cookie("dw_session").unwrap());
    let me = app.get("/api/auth/me", &cookie).await;
    assert_eq!(me.body["via"], "password");
    assert_eq!(
        app.get("/api/admin/users", &cookie).await.status,
        StatusCode::OK
    );

    // Второй вход паролем попадает в того же владельца
    let again = password(&app, ADMIN_PASSWORD, "203.0.113.1").await;
    assert_eq!(again.body["user"]["id"], res.body["user"]["id"]);

    let kinds = app.audit_kinds().await;
    assert_eq!(
        kinds,
        [
            "auth.sign_in_failed",
            "user.created",
            "auth.sign_in",
            "auth.sign_in"
        ]
    );
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn password_signs_in_as_the_first_admin(db: PgPool) {
    let app = TestApp::with(db, |c| c.admin_password = Some(ADMIN_PASSWORD.into())).await;
    let (boss, _) = app.admin("boss").await;
    app.admin("deputy").await;
    let res = password(&app, ADMIN_PASSWORD, "203.0.113.1").await;
    assert_eq!(res.body["user"]["id"], boss.as_str());
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM users")
        .fetch_one(&app.state.db)
        .await
        .unwrap();
    assert_eq!(count, 2, "владелец не заводится, если админ уже есть");
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn password_is_rate_limited_per_ip(db: PgPool) {
    let app = TestApp::with(db, |c| c.admin_password = Some(ADMIN_PASSWORD.into())).await;
    for _ in 0..5 {
        assert_eq!(
            password(&app, "nope", "203.0.113.1").await.status,
            StatusCode::UNAUTHORIZED
        );
    }
    let res = password(&app, ADMIN_PASSWORD, "203.0.113.1").await;
    assert_eq!(res.status, StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(res.code(), "too_many_requests");
    // Другой адрес за прокси считается отдельно
    assert_eq!(
        password(&app, ADMIN_PASSWORD, "203.0.113.2").await.status,
        StatusCode::OK
    );
}
