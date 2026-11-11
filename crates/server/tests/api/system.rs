use std::fs;

use axum::{
    body::Body,
    http::{Method, Request, StatusCode, header},
};
use serde_json::json;
use sqlx::PgPool;

use crate::{
    common::{Auth, PUBLIC_URL, TestApp},
    mock::Person,
};

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn health_and_unknown_routes(db: PgPool) {
    let app = TestApp::new(db).await;
    let res = app.get("/api/healthz", &Auth::None).await;
    assert_eq!(res.status, StatusCode::OK);
    assert_eq!(res.body["status"], "ok");
    assert_eq!(res.body["db"], "ok");
    assert_eq!(res.body["version"], env!("CARGO_PKG_VERSION"));
    assert!(res.body["uptimeS"].is_u64());
    assert!(res.headers.contains_key("x-request-id"));
    assert_eq!(res.headers[header::X_CONTENT_TYPE_OPTIONS], "nosniff");

    let res = app.get("/api/nope", &Auth::None).await;
    assert_eq!(res.status, StatusCode::NOT_FOUND);
    assert_eq!(
        res.body,
        json!({ "error": { "code": "not_found", "message": "не найдено" } })
    );
    let res = app
        .call(Method::DELETE, "/api/healthz", None, &Auth::None)
        .await;
    assert_eq!(res.status, StatusCode::METHOD_NOT_ALLOWED);
    assert_eq!(res.code(), "method_not_allowed");
    // Без собранного интерфейса всё вне /api — тоже 404
    assert_eq!(
        app.get("/library", &Auth::None).await.status,
        StatusCode::NOT_FOUND
    );

    app.state.db.close().await;
    let res = app.get("/api/healthz", &Auth::None).await;
    assert_eq!(res.status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(res.body["db"], "error");
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn openapi_lists_every_endpoint(db: PgPool) {
    let app = TestApp::new(db).await;
    let res = app.get("/api/openapi.json", &Auth::None).await;
    assert_eq!(res.status, StatusCode::OK);
    assert_eq!(res.body["openapi"], "3.1.0");
    let paths: Vec<&str> = res.body["paths"]
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    let expected = [
        "/api/admin/audit",
        "/api/admin/jobs",
        "/api/admin/jobs/{id}/cancel",
        "/api/admin/jobs/{id}/retry",
        "/api/admin/lanes",
        "/api/admin/lanes/{lane}",
        "/api/admin/overview",
        "/api/admin/sessions",
        "/api/admin/sessions/{id}",
        "/api/admin/users",
        "/api/admin/users/{id}",
        "/api/albums/{id}",
        "/api/artists/{id}",
        "/api/auth/app/start",
        "/api/auth/app/token",
        "/api/auth/logout",
        "/api/auth/me",
        "/api/auth/oidc/callback",
        "/api/auth/oidc/start",
        "/api/auth/password",
        "/api/auth/providers",
        "/api/catalog/deezer/{kind}/{deezerId}",
        "/api/events",
        "/api/healthz",
        "/api/home",
        "/api/images/{kind}/{id}/{file}",
        "/api/library",
        "/api/library/albums",
        "/api/library/albums/{id}",
        "/api/library/artists",
        "/api/library/artists/{id}",
        "/api/library/blocked",
        "/api/library/blocked/{kind}/{id}",
        "/api/library/tracks",
        "/api/library/tracks/{id}",
        "/api/me/history",
        "/api/openapi.json",
        "/api/playlists",
        "/api/playlists/{id}",
        "/api/playlists/{id}/items",
        "/api/playlists/{id}/items/{itemId}",
        "/api/playlists/{id}/items/{itemId}/move",
        "/api/playlists/{id}/pin",
        "/api/search",
        "/api/sync",
        "/api/tracks/lookup",
        "/api/tracks/{id}",
        "/api/ws",
    ];
    let mut sorted = paths.clone();
    sorted.sort();
    assert_eq!(sorted, expected);
    let schemas = res.body["components"]["schemas"].as_object().unwrap();
    for name in [
        "Me",
        "AppToken",
        "AdminUser",
        "AuditPage",
        "ErrorBody",
        "Health",
        "SessionList",
        "ArtistDetail",
        "AlbumDetail",
        "Track",
        "SearchResults",
        "CatalogEntity",
        "LibraryOverview",
        "PlaylistDetail",
        "SyncPage",
        "Home",
        "ShelfItem",
        "EventBatch",
        "AdminOverview",
        "JobPage",
        "LaneList",
        "ServerEvent",
        "ClientMessage",
        "AcquirePayload",
    ] {
        assert!(schemas.contains_key(name), "{name}");
    }
    assert_eq!(schemas["Me"]["properties"]["sessionId"]["format"], "uuid");
    assert!(res.body["components"]["securitySchemes"]["session"].is_object());
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn unsafe_requests_need_our_origin_and_json(db: PgPool) {
    let app = TestApp::new(db).await;
    let cookie = app.login(&Person::new("ann")).await;
    let logout = |origin: Option<&str>, content_type: Option<&str>, body: &str| {
        let mut req = Request::builder()
            .method(Method::POST)
            .uri("/api/auth/logout")
            .header(header::COOKIE, &cookie);
        if let Some(o) = origin {
            req = req.header(header::ORIGIN, o);
        }
        if let Some(t) = content_type {
            req = req.header(header::CONTENT_TYPE, t);
        }
        req.body(Body::from(body.to_string())).unwrap()
    };

    let res = app
        .send(logout(Some("https://evil.example"), None, ""))
        .await;
    assert_eq!(res.status, StatusCode::FORBIDDEN);
    assert_eq!(res.code(), "bad_origin");
    let res = app.send(logout(None, None, "")).await;
    assert_eq!(res.code(), "bad_origin", "cookie без Origin");
    let res = app
        .send(logout(Some(PUBLIC_URL), Some("text/plain"), "x=1"))
        .await;
    assert_eq!(res.status, StatusCode::UNSUPPORTED_MEDIA_TYPE);
    // WebView приложения тоже свой
    assert_eq!(
        app.get("/api/auth/me", &Auth::Cookie(cookie.clone()))
            .await
            .status,
        StatusCode::OK
    );
    let res = app.send(logout(Some("tauri://localhost"), None, "")).await;
    assert_eq!(res.status, StatusCode::NO_CONTENT);

    // Сломанный и слишком большой JSON отвечают в общем формате
    let req = Request::builder()
        .method(Method::POST)
        .uri("/api/auth/app/token")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from("{"))
        .unwrap();
    let res = app.send(req).await;
    assert_eq!(res.status, StatusCode::BAD_REQUEST);
    assert_eq!(res.code(), "bad_request");
    let huge = json!({ "code": "x".repeat(2 * 1024 * 1024), "codeVerifier": "y" }).to_string();
    let req = Request::builder()
        .method(Method::POST)
        .uri("/api/auth/app/token")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(huge))
        .unwrap();
    assert_eq!(app.send(req).await.status, StatusCode::PAYLOAD_TOO_LARGE);
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn static_spa_fallback(db: PgPool) {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir_all(dir.path().join("_nuxt")).unwrap();
    fs::write(
        dir.path().join("index.html"),
        "<!doctype html><title>DalyWave</title>",
    )
    .unwrap();
    fs::write(
        dir.path().join("_nuxt/entry.Bk2x9.js"),
        "console.log('wave')".repeat(100),
    )
    .unwrap();
    fs::write(dir.path().join("favicon.ico"), [0u8; 16]).unwrap();
    let path = dir.path().to_path_buf();
    let app = TestApp::with(db, move |c| c.static_dir = Some(path)).await;

    for uri in ["/", "/library/albums", "/artist/123?tab=popular"] {
        let res = app.get(uri, &Auth::None).await;
        assert_eq!(res.status, StatusCode::OK, "{uri}");
        assert!(res.text.contains("<title>DalyWave</title>"), "{uri}");
        assert_eq!(res.headers[header::CACHE_CONTROL], "no-cache", "{uri}");
        assert!(
            res.headers[header::CONTENT_SECURITY_POLICY]
                .to_str()
                .unwrap()
                .contains("frame-ancestors 'none'")
        );
        assert_eq!(res.headers[header::X_FRAME_OPTIONS], "DENY");
    }

    let res = app.get("/_nuxt/entry.Bk2x9.js", &Auth::None).await;
    assert_eq!(res.status, StatusCode::OK);
    assert_eq!(
        res.headers[header::CACHE_CONTROL],
        "public, max-age=31536000, immutable"
    );
    assert!(!res.headers.contains_key(header::CONTENT_SECURITY_POLICY));
    // Несуществующий бандл — 404, а не index.html под видом скрипта
    let res = app.get("/_nuxt/missing.js", &Auth::None).await;
    assert_eq!(res.status, StatusCode::NOT_FOUND);
    assert!(!res.text.contains("DalyWave"));

    let res = app.get("/favicon.ico", &Auth::None).await;
    assert_eq!(res.status, StatusCode::OK);
    assert_eq!(res.headers[header::CACHE_CONTROL], "no-cache");

    // Незнакомый файл — 404, а не страница интерфейса под видом файла
    for uri in [
        "/robots.txt",
        "/apple-touch-icon.png",
        "/deep/path/manifest.webmanifest",
        "/_nuxt/sub/missing.css",
    ] {
        let res = app.get(uri, &Auth::None).await;
        assert_eq!(res.status, StatusCode::NOT_FOUND, "{uri}");
        assert!(!res.text.contains("DalyWave"), "{uri}");
        assert!(
            !res.headers.contains_key(header::CONTENT_SECURITY_POLICY),
            "{uri}"
        );
    }

    // API не подменяется интерфейсом
    let res = app.get("/api/nope", &Auth::None).await;
    assert_eq!(res.status, StatusCode::NOT_FOUND);
    assert_eq!(res.code(), "not_found");

    // Сжатие для тех, кто его понимает
    let req = Request::builder()
        .uri("/_nuxt/entry.Bk2x9.js")
        .header(header::ACCEPT_ENCODING, "br")
        .body(Body::empty())
        .unwrap();
    let res = app.send(req).await;
    assert_eq!(res.headers[header::CONTENT_ENCODING], "br");
}
