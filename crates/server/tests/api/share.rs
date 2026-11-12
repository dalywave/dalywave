//! «Поделиться»: карточка ссылки, превью `/{kind}/{id}` для мессенджеров и разбор чужих ссылок.
//! Spotify, Apple Music и oEmbed YouTube — заглушка на локальном порту (`Links::stubbed`).

use std::sync::Arc;

use axum::{
    Router,
    extract::Query,
    http::{StatusCode, header},
    response::{Html, IntoResponse},
    routing::get,
};
use dalywave_server::links::Links;
use serde_json::{Value, json};
use sqlx::PgPool;

use crate::{
    common::{Auth, TestApp},
    deezer::{LIGHT_ME_UP, fixture},
};

/// Чужие сервисы: страница трека Spotify, альбома Apple Music, oEmbed YouTube, короткая ссылка Deezer.
async fn external() -> String {
    let spotify_track = || async {
        Html(
            r#"<!doctype html><html><head>
            <meta property="og:title" content="Devil"/>
            <meta property="og:description" content="Lowborn · Devil · Song · 2024"/>
            </head><body></body></html>"#,
        )
    };
    let app = Router::new()
        .route("/intl-de/track/{id}", get(spotify_track))
        .route(
            "/track/{id}",
            get(|| async {
                Html(
                    r#"<!doctype html><html><head>
                    <meta property="og:title" content="Devil"/>
                    <meta property="og:description" content="Lowborn · Devil · Song · 2024"/>
                    </head><body></body></html>"#,
                )
            }),
        )
        .route(
            "/track/nothing",
            get(|| async {
                Html(r#"<meta property="og:title" content="Совсем другая песня"><meta property="og:description" content="Никто · Song · 2020">"#)
            }),
        )
        .route(
            "/us/album/devil/1700000000",
            get(|| async {
                Html(r#"<meta property="og:title" content="&#x200e;Devil by Lowborn on Apple Music">"#)
            }),
        )
        .route(
            "/oembed",
            get(|Query(q): Query<Vec<(String, String)>>| async move {
                let url = q.iter().find(|(k, _)| k == "url").map(|(_, v)| v.clone());
                match url.as_deref() {
                    Some("https://www.youtube.com/watch?v=dQw4w9WgXcQ") => axum::Json(json!({
                        "title": "Lowborn - Doctor Doctor (Official Video)",
                        "author_name": "LowbornVEVO",
                    }))
                    .into_response(),
                    _ => StatusCode::NOT_FOUND.into_response(),
                }
            }),
        )
        .route(
            "/s/30abc",
            get(|| async {
                (
                    StatusCode::FOUND,
                    [(
                        header::LOCATION,
                        format!("https://www.deezer.com/ru/track/{LIGHT_ME_UP}?utm_source=x"),
                    )],
                )
            }),
        );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    base
}

async fn app_with_links(db: PgPool) -> TestApp {
    let mut app = TestApp::new(db).await;
    app.state.links = Arc::new(Links::stubbed(&external().await).unwrap());
    app.router = dalywave_server::app(app.state.clone());
    app
}

async fn resolve(app: &TestApp, auth: &Auth, url: &str) -> (StatusCode, Value) {
    let res = app
        .post("/api/resolve-link", json!({ "url": url }), auth)
        .await;
    (res.status, res.body)
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn deezer_and_own_links(db: PgPool) {
    let app = app_with_links(db).await;
    let me = app.listener("ann").await;

    let (status, body) = resolve(
        &app,
        &me,
        &format!("Послушай: https://www.deezer.com/en/track/{LIGHT_ME_UP}?utm=1 — огонь"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["source"], "deezer");
    assert_eq!(body["kind"], "track");
    assert_eq!(body["track"]["title"], "Light Me Up");
    assert!(body["query"].is_null() && body["album"].is_null());
    let id = body["track"]["id"].as_str().unwrap().to_string();

    // Короткая ссылка Deezer — через редирект
    let (status, body) = resolve(&app, &me, "https://link.deezer.com/s/30abc").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["track"]["id"], id.as_str());

    // Свои ссылки: и с адреса сервера, и боевые
    for url in [
        format!("http://localhost:3400/track/{id}"),
        format!("https://wave.daly.rest/track/{id}"),
    ] {
        let (status, body) = resolve(&app, &me, &url).await;
        assert_eq!(status, StatusCode::OK, "{url}: {body}");
        assert_eq!(body["source"], "dalywave");
        assert_eq!(body["track"]["id"], id.as_str());
    }
    let album = app.get(&format!("/api/tracks/{id}"), &me).await.body["album"]["id"]
        .as_str()
        .unwrap()
        .to_string();
    let (status, body) = resolve(&app, &me, &format!("https://wave.daly.rest/album/{album}")).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["kind"], "album");
    assert_eq!(body["album"]["title"], "Light Me Up");

    // Чужой плейлист не находится, свой — да
    let pl = app
        .post(
            "/api/playlists",
            json!({ "title": "Ночь", "trackIds": [id] }),
            &me,
        )
        .await;
    assert_eq!(pl.status, StatusCode::CREATED, "{}", pl.text);
    let pl = pl.body["playlist"]["id"].as_str().unwrap().to_string();
    let (status, body) = resolve(&app, &me, &format!("https://wave.daly.rest/playlist/{pl}")).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["playlistId"], pl.as_str());
    let other = app.listener("bob").await;
    let (status, _) = resolve(
        &app,
        &other,
        &format!("https://wave.daly.rest/playlist/{pl}"),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    for junk in [
        "просто текст без ссылки",
        "https://example.com/track/1",
        "https://open.spotify.com/playlist/37i9dQZF1DXcBWIGoYBM5M",
        "https://wave.daly.rest/library",
    ] {
        let res = app
            .post("/api/resolve-link", json!({ "url": junk }), &me)
            .await;
        assert_eq!(
            res.status,
            StatusCode::UNPROCESSABLE_ENTITY,
            "{junk}: {}",
            res.text
        );
        assert_eq!(res.code(), "unsupported_link");
    }
    let res = app
        .post("/api/resolve-link", json!({ "url": "x" }), &Auth::None)
        .await;
    assert_eq!(res.status, StatusCode::UNAUTHORIZED);
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn spotify_apple_and_youtube_go_through_deezer_search(db: PgPool) {
    let app = app_with_links(db).await;
    let me = app.listener("ann").await;
    app.deezer
        .set("/search/track", fixture("search_track_lowborn"));
    app.deezer
        .set("/search/album", fixture("search_album_lowborn"));

    let (status, body) = resolve(
        &app,
        &me,
        "https://open.spotify.com/intl-de/track/4uLU6hMCjMI75M1A2tKUQC?si=abc",
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["source"], "spotify");
    assert_eq!(body["kind"], "track");
    assert_eq!(body["query"], "Lowborn Devil");
    assert_eq!(body["track"]["title"], "Devil");
    assert_eq!(body["track"]["artists"][0]["name"], "Lowborn");
    assert_eq!(body["track"]["deezerId"], 3657134502_i64);

    let (status, body) = resolve(
        &app,
        &me,
        "https://music.apple.com/us/album/devil/1700000000",
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["source"], "apple");
    assert_eq!(body["kind"], "album");
    assert_eq!(body["album"]["title"], "Devil");
    assert_eq!(body["album"]["artist"]["name"], "Lowborn");

    let (status, body) = resolve(&app, &me, "https://youtu.be/dQw4w9WgXcQ?si=share").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["source"], "youtube");
    assert_eq!(body["query"], "Lowborn Doctor Doctor");
    assert_eq!(body["track"]["title"], "Doctor Doctor");

    // Названия нет в найденном — не подставляем первое попавшееся
    let (status, _) = resolve(&app, &me, "https://open.spotify.com/track/nothing").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    // Страницы нет у сервиса
    let (status, _) = resolve(&app, &me, "https://youtu.be/missing0000").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn share_card_and_link_preview(db: PgPool) {
    let dir = tempfile::tempdir().unwrap();
    let index = "<!doctype html><html><head><title>DalyWave</title></head><body><div id=\"__nuxt\"></div></body></html>";
    std::fs::write(dir.path().join("index.html"), index).unwrap();
    let static_dir = dir.path().to_path_buf();
    let app = TestApp::with(db, move |c| c.static_dir = Some(static_dir)).await;
    let me = app.listener("ann").await;
    let id = app.resolve_id("track", LIGHT_ME_UP, &me).await;

    let res = app.get(&format!("/api/share/track/{id}"), &me).await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    assert_eq!(res.body["kind"], "track");
    assert_eq!(res.body["title"], "Light Me Up");
    assert_eq!(res.body["subtitle"], "Lowborn, vista");
    assert_eq!(res.body["url"], format!("http://localhost:3400/track/{id}"));
    // Адрес Deezer (в тестах CDN подменён заглушкой), а не наш /api/images
    let image = res.body["image"].as_str().unwrap().to_string();
    assert!(image.starts_with(&app.deezer.base), "{image}");
    assert_eq!(
        app.get(&format!("/api/share/track/{id}"), &Auth::None)
            .await
            .status,
        StatusCode::UNAUTHORIZED
    );
    let missing = uuid::Uuid::now_v7();
    assert_eq!(
        app.get(&format!("/api/share/album/{missing}"), &me)
            .await
            .status,
        StatusCode::NOT_FOUND
    );

    // Превью — без входа, теги в <head>, значения экранированы
    sqlx::query("UPDATE tracks SET title = 'Light <Me> & \"Up\"' WHERE id = $1::uuid")
        .bind(&id)
        .execute(&app.state.db)
        .await
        .unwrap();
    let page = app.get(&format!("/track/{id}"), &Auth::None).await;
    assert_eq!(page.status, StatusCode::OK);
    assert!(
        page.headers[header::CONTENT_TYPE]
            .to_str()
            .unwrap()
            .starts_with("text/html")
    );
    assert_eq!(page.headers[header::CACHE_CONTROL], "no-cache");
    assert_eq!(page.headers[header::X_FRAME_OPTIONS], "DENY");
    assert!(
        page.text.contains(
            r#"<meta property="og:title" content="Light &lt;Me&gt; &amp; &quot;Up&quot;">"#
        ),
        "{}",
        page.text
    );
    assert!(
        page.text
            .contains(r#"<meta property="og:type" content="music.song">"#)
    );
    assert!(
        page.text
            .contains(r#"<meta property="og:description" content="Lowborn, vista">"#)
    );
    assert!(
        page.text
            .contains(&format!(r#"<meta property="og:image" content="{image}">"#))
    );
    assert!(page.text.find("og:title").unwrap() < page.text.find("</head>").unwrap());
    assert!(page.text.contains("<div id=\"__nuxt\"></div>"));

    // Неизвестное — интерфейс как есть: «не найдено» покажет он сам
    for path in [
        format!("/album/{missing}"),
        "/track/not-a-uuid".to_string(),
        "/library".to_string(),
    ] {
        let res = app.get(&path, &Auth::None).await;
        assert_eq!(res.status, StatusCode::OK, "{path}");
        assert_eq!(res.text, index, "{path}");
    }
    assert_eq!(
        app.get("/api/healthz", &Auth::None).await.status,
        StatusCode::OK
    );
}
