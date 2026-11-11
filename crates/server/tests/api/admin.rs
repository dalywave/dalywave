use axum::{
    Router,
    body::Body,
    http::{Method, Request, StatusCode, header},
    middleware::from_fn_with_state,
    routing::get,
};
use dalywave_server::auth::{Principal, require_access};
use http_body_util::BodyExt;
use serde_json::json;
use sqlx::PgPool;
use tower::ServiceExt;

use crate::{
    common::{Auth, TestApp},
    mock::Person,
};

/// Слой, которым закрываются будущие разделы приложения: вход и хотя бы одна роль.
#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn app_sections_need_a_role(db: PgPool) {
    let app = TestApp::new(db).await;
    let (_, admin) = app.admin("boss").await;
    let waiting = app.login(&Person::new("ann")).await;
    let probe = Router::new()
        .route("/probe", get(|p: Principal| async move { p.user.name }))
        .route_layer(from_fn_with_state(app.state.clone(), require_access))
        .with_state(app.state.clone());
    let call = |cookie: Option<String>| {
        let probe = probe.clone();
        async move {
            let mut req = Request::builder().uri("/probe");
            if let Some(c) = cookie {
                req = req.header(header::COOKIE, c);
            }
            let res = probe
                .oneshot(req.body(Body::empty()).unwrap())
                .await
                .unwrap();
            let status = res.status();
            let body = res.into_body().collect().await.unwrap().to_bytes();
            (status, String::from_utf8_lossy(&body).to_string())
        }
    };
    assert_eq!(call(None).await.0, StatusCode::UNAUTHORIZED);
    let (status, body) = call(Some(waiting)).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert!(body.contains("no_access"), "{body}");
    let Auth::Cookie(admin) = admin else {
        unreachable!()
    };
    assert_eq!(
        call(Some(admin)).await,
        (StatusCode::OK, "Person boss".to_string())
    );
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn roles_are_granted_by_admin(db: PgPool) {
    let app = TestApp::new(db).await;
    let (_, admin) = app.admin("boss").await;
    let ann = Auth::Cookie(app.login(&Person::new("ann")).await);
    let ann_id = app.user_id(&ann).await;

    // Ждущие доступа — первыми в списке
    let users = app.get("/api/admin/users", &admin).await;
    assert_eq!(users.status, StatusCode::OK);
    assert_eq!(users.body["items"][0]["id"], ann_id.as_str());
    assert_eq!(users.body["items"][0]["roles"], json!([]));
    assert!(users.body["nextCursor"].is_null());

    let res = app
        .call(
            Method::PATCH,
            &format!("/api/admin/users/{ann_id}"),
            Some(json!({ "roles": ["listener"] })),
            &admin,
        )
        .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    assert_eq!(res.body["roles"], json!(["listener"]));
    // Роль действует сразу, без нового входа
    let me = app.get("/api/auth/me", &ann).await;
    assert_eq!(me.body["permissions"], json!(["play", "library"]));
    // Слушателю админка закрыта
    let res = app.get("/api/admin/users", &ann).await;
    assert_eq!(res.status, StatusCode::FORBIDDEN);
    assert_eq!(res.code(), "forbidden");

    // Ручной admin переживает вход, где daly.rest admin не присылает
    let res = app
        .call(
            Method::PATCH,
            &format!("/api/admin/users/{ann_id}"),
            Some(json!({ "roles": ["admin", "listener"] })),
            &admin,
        )
        .await;
    assert_eq!(res.body["idpAdmin"], false);
    let ann = Auth::Cookie(app.login(&Person::new("ann")).await);
    assert_eq!(
        app.get("/api/auth/me", &ann).await.body["roles"],
        json!(["admin", "listener"])
    );

    let res = app
        .call(
            Method::PATCH,
            &format!("/api/admin/users/{ann_id}"),
            Some(json!({ "roles": ["root"] })),
            &admin,
        )
        .await;
    assert_eq!(res.status, StatusCode::BAD_REQUEST);
    let res = app
        .call(
            Method::PATCH,
            &format!("/api/admin/users/{ann_id}"),
            Some(json!({ "name": "x" })),
            &admin,
        )
        .await;
    assert_eq!(
        res.status,
        StatusCode::BAD_REQUEST,
        "незнакомые поля не принимаются"
    );
    let missing = uuid::Uuid::now_v7();
    let res = app
        .call(
            Method::PATCH,
            &format!("/api/admin/users/{missing}"),
            Some(json!({ "roles": [] })),
            &admin,
        )
        .await;
    assert_eq!(res.status, StatusCode::NOT_FOUND);
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn admin_cannot_lock_themselves_out(db: PgPool) {
    let app = TestApp::new(db).await;
    let (me, admin) = app.admin("boss").await;
    let res = app
        .call(
            Method::PATCH,
            &format!("/api/admin/users/{me}"),
            Some(json!({ "roles": ["listener"] })),
            &admin,
        )
        .await;
    assert_eq!(res.status, StatusCode::CONFLICT);
    assert_eq!(res.code(), "cannot_demote_self");
    let res = app
        .call(
            Method::PATCH,
            &format!("/api/admin/users/{me}"),
            Some(json!({ "disabled": true })),
            &admin,
        )
        .await;
    assert_eq!(res.status, StatusCode::CONFLICT);
    assert_eq!(res.code(), "cannot_disable_self");
    // Добавить себе роль можно
    let res = app
        .call(
            Method::PATCH,
            &format!("/api/admin/users/{me}"),
            Some(json!({ "roles": ["admin", "listener"] })),
            &admin,
        )
        .await;
    assert_eq!(res.status, StatusCode::OK);
    assert_eq!(res.body["roles"], json!(["admin", "listener"]));
    assert_eq!(
        app.get("/api/auth/me", &admin).await.body["roles"],
        json!(["admin", "listener"])
    );
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn sessions_can_be_revoked(db: PgPool) {
    let app = TestApp::new(db).await;
    let (_, admin) = app.admin("boss").await;
    let ann = Auth::Cookie(app.login(&Person::new("ann")).await);
    let ann_id = app.user_id(&ann).await;

    let all = app.get("/api/admin/sessions", &admin).await;
    assert_eq!(all.status, StatusCode::OK, "{}", all.text);
    assert_eq!(all.body["items"].as_array().unwrap().len(), 2);
    assert_eq!(
        all.body["items"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|s| s["current"] == true)
            .count(),
        1
    );

    let theirs = app
        .get(&format!("/api/admin/sessions?userId={ann_id}"), &admin)
        .await;
    let items = theirs.body["items"].as_array().unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["kind"], "web");
    assert_eq!(items[0]["via"], "oidc");
    assert_eq!(items[0]["userName"], "Person ann");
    assert_eq!(items[0]["ip"], "127.0.0.1");
    let session_id = items[0]["id"].as_str().unwrap().to_string();

    let res = app
        .call(
            Method::DELETE,
            &format!("/api/admin/sessions/{session_id}"),
            None,
            &admin,
        )
        .await;
    assert_eq!(res.status, StatusCode::NO_CONTENT);
    assert_eq!(
        app.get("/api/auth/me", &ann).await.status,
        StatusCode::UNAUTHORIZED
    );
    let res = app
        .call(
            Method::DELETE,
            &format!("/api/admin/sessions/{session_id}"),
            None,
            &admin,
        )
        .await;
    assert_eq!(res.status, StatusCode::NOT_FOUND);

    let audit = app.get("/api/admin/audit?limit=1", &admin).await;
    assert_eq!(audit.status, StatusCode::OK);
    let event = &audit.body["items"][0];
    assert_eq!(event["kind"], "session.revoked");
    assert_eq!(event["subject"], session_id.as_str());
    assert_eq!(event["actor"]["name"], "Person boss");
    assert_eq!(event["payload"]["userId"], ann_id.as_str());
    let cursor = audit.body["nextCursor"].as_str().unwrap();

    // Журнал листается курсором, новые события первыми
    let next = app
        .get(
            &format!("/api/admin/audit?limit=200&cursor={cursor}"),
            &admin,
        )
        .await;
    let kinds: Vec<&str> = next.body["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["kind"].as_str().unwrap())
        .collect();
    assert_eq!(kinds.first(), Some(&"auth.sign_in"));
    assert_eq!(kinds.last(), Some(&"user.created"));
    assert!(next.body["nextCursor"].is_null());
    assert_eq!(
        app.get("/api/admin/audit?limit=0", &admin).await.status,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        app.get("/api/admin/audit?cursor=nope", &admin).await.status,
        StatusCode::BAD_REQUEST
    );

    // Кривые параметры адреса отвечают в общем формате ошибок, а не простым текстом
    for (method, uri) in [
        (Method::GET, "/api/admin/audit?limit=many"),
        (Method::GET, "/api/admin/sessions?userId=nope"),
        (Method::DELETE, "/api/admin/sessions/not-a-uuid"),
    ] {
        let res = app.call(method, uri, None, &admin).await;
        assert_eq!(res.status, StatusCode::BAD_REQUEST, "{uri}");
        assert_eq!(res.code(), "bad_request", "{uri}: {}", res.text);
    }
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn admin_endpoints_are_guarded(db: PgPool) {
    let app = TestApp::new(db).await;
    let waiting = Auth::Cookie(app.login(&Person::new("ann")).await);
    let id = uuid::Uuid::now_v7();
    let requests = [
        (Method::GET, "/api/admin/users".to_string()),
        (Method::PATCH, format!("/api/admin/users/{id}")),
        (Method::GET, "/api/admin/sessions".to_string()),
        (Method::DELETE, format!("/api/admin/sessions/{id}")),
        (Method::GET, "/api/admin/audit".to_string()),
    ];
    for (method, uri) in &requests {
        let body = (method == Method::PATCH).then(|| json!({ "roles": [] }));
        let res = app
            .call(method.clone(), uri, body.clone(), &Auth::None)
            .await;
        assert_eq!(res.status, StatusCode::UNAUTHORIZED, "{method} {uri}");
        let res = app.call(method.clone(), uri, body, &waiting).await;
        assert_eq!(res.status, StatusCode::FORBIDDEN, "{method} {uri}");
        assert_eq!(res.code(), "no_access");
    }
}
