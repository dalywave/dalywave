use axum::http::StatusCode;
use serde_json::json;
use sqlx::PgPool;

use crate::{
    common::{Auth, TestApp},
    deezer::{
        CHURCH_ON_SUNDAY, DEVIL_SINGLE, LIGHT_ME_UP, LONG_ISLAND, LOWBORN, MODERN_SADNESS, fixture,
    },
};

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn artist_page_loads_once_and_stays_fresh(db: PgPool) {
    let app = TestApp::new(db).await;
    let (_, admin) = app.admin("boss").await;
    let resolved = app.resolve("artist", LOWBORN, &admin).await;
    assert_eq!(resolved["type"], "artist");
    assert_eq!(resolved["name"], "Lowborn");
    let id = resolved["id"].as_str().unwrap().to_string();

    let res = app.get(&format!("/api/artists/{id}"), &admin).await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    let page = &res.body;
    assert_eq!(page["artist"]["name"], "Lowborn");
    assert_eq!(page["artist"]["fans"], 611);
    assert_eq!(page["artist"]["followed"], false);
    assert!(
        page["artist"]["image"]["medium"]
            .as_str()
            .unwrap()
            .starts_with(&format!("/api/images/artists/{id}/500.jpg?v="))
    );
    assert_eq!(page["topTracks"].as_array().unwrap().len(), 10);
    assert_eq!(page["topTracks"][0]["title"], "I Want Out");
    assert_eq!(page["topTracks"][0]["artists"][0]["name"], "Lowborn");
    // Популярное приходит без ISRC: записи у таких треков пока нет
    assert!(page["topTracks"][0]["recordingId"].is_null());
    assert_eq!(page["topTracks"][0]["audioStatus"], "none");
    assert_eq!(page["releases"]["eps"].as_array().unwrap().len(), 4);
    assert_eq!(page["releases"]["singles"].as_array().unwrap().len(), 50);
    assert!(page["releases"]["albums"].as_array().unwrap().is_empty());
    // Новые релизы первыми
    assert_eq!(page["releases"]["eps"][0]["title"], "melancholy");
    assert_eq!(page["related"].as_array().unwrap().len(), 20);
    assert_eq!(page["related"][0]["name"], "Bohnes");
    assert!(page["follow"].is_null());
    assert_eq!(page["tracksOnServer"], 0);
    assert!(page["tracksTotal"].as_i64().unwrap() > 0);

    for path in [
        format!("/artist/{LOWBORN}"),
        format!("/artist/{LOWBORN}/top"),
        format!("/artist/{LOWBORN}/related"),
        format!("/artist/{LOWBORN}/albums"),
    ] {
        assert_eq!(app.deezer.hits(&path), 1, "{path}");
    }
    // Всё свежее: второй заход в Deezer не ходит
    let before = app.deezer.total_hits();
    let again = app.get(&format!("/api/artists/{id}"), &admin).await;
    assert_eq!(again.status, StatusCode::OK);
    assert_eq!(app.deezer.total_hits(), before);
    assert_eq!(again.body["artist"], page["artist"]);
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn stale_artist_is_shown_and_refreshed_in_background(db: PgPool) {
    let app = TestApp::new(db).await;
    let (_, admin) = app.admin("boss").await;
    let id = app.resolve_id("artist", LOWBORN, &admin).await;
    assert_eq!(
        app.get(&format!("/api/artists/{id}"), &admin).await.status,
        StatusCode::OK
    );

    // Карточке 8 дней, популярному — двое суток
    sqlx::query(
        "UPDATE artists SET fetched_at = now() - interval '8 days', top_fetched_at = now() - interval '2 days'
         WHERE deezer_id = $1",
    )
    .bind(LOWBORN)
    .execute(&app.state.db)
    .await
    .unwrap();
    let mut renamed = fixture(&format!("artist_{LOWBORN}"));
    renamed["name"] = json!("Lowborn (renamed)");
    app.deezer.set(&format!("/artist/{LOWBORN}"), renamed);
    let hits = app.deezer.total_hits();

    let res = app.get(&format!("/api/artists/{id}"), &admin).await;
    assert_eq!(res.status, StatusCode::OK);
    // Устаревшее показывается сразу, без похода в Deezer
    assert_eq!(res.body["artist"]["name"], "Lowborn");
    assert_eq!(app.deezer.total_hits(), hits);
    // Повторный заход не ставит вторую такую же задачу
    app.get(&format!("/api/artists/{id}"), &admin).await;
    let queued: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM jobs WHERE kind = 'refresh_artist' AND status = 'queued'",
    )
    .fetch_one(&app.state.db)
    .await
    .unwrap();
    assert_eq!(queued, 1);

    let (kind, outcome) = app.run_job("maintenance").await.unwrap();
    assert_eq!(kind, "refresh_artist");
    assert_eq!(outcome, dalywave_server::jobs::worker::Outcome::Done);
    assert_eq!(app.deezer.hits(&format!("/artist/{LOWBORN}")), 2);
    assert_eq!(app.deezer.hits(&format!("/artist/{LOWBORN}/top")), 2);
    // Свежие части не трогались
    assert_eq!(app.deezer.hits(&format!("/artist/{LOWBORN}/related")), 1);
    let res = app.get(&format!("/api/artists/{id}"), &admin).await;
    assert_eq!(res.body["artist"]["name"], "Lowborn (renamed)");
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn album_tracks_share_recordings_by_isrc(db: PgPool) {
    let app = TestApp::new(db).await;
    let (_, admin) = app.admin("boss").await;
    let ep = app.resolve("album", MODERN_SADNESS, &admin).await;
    assert_eq!(ep["type"], "album");
    assert_eq!(ep["recordType"], "ep");
    let ep_id = ep["id"].as_str().unwrap();

    let res = app.get(&format!("/api/albums/{ep_id}"), &admin).await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    let album = &res.body;
    assert_eq!(album["album"]["title"], "Modern Sadness");
    assert_eq!(album["album"]["artist"]["name"], "Lowborn");
    assert_eq!(album["album"]["releaseDate"], "2021-05-21");
    assert_eq!(album["label"], "Nettwerk Music Group");
    assert_eq!(album["genres"], json!(["Alternative"]));
    assert_eq!(album["totalDurationMs"], 856_000);
    assert_eq!(album["readyTracks"], 0);
    let titles: Vec<&str> = album["tracks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["title"].as_str().unwrap())
        .collect();
    assert_eq!(
        titles,
        vec![
            "Church On Sunday",
            "sad. sad.",
            "TWSS",
            "Devil",
            "GOOD TIME"
        ]
    );
    let devil = &album["tracks"][3];
    assert_eq!(devil["isrc"], "QZ5FN2194847");
    assert_eq!(devil["trackPosition"], 4);
    assert_eq!(devil["discNumber"], 1);
    assert_eq!(devil["album"]["title"], "Modern Sadness");
    assert!(devil["recordingId"].is_string());

    // Сингл «Devil»: те же ISRC — те же записи, но свои треки
    let single = app.resolve_id("album", DEVIL_SINGLE, &admin).await;
    let res = app.get(&format!("/api/albums/{single}"), &admin).await;
    let single_tracks = res.body["tracks"].as_array().unwrap().clone();
    assert_eq!(single_tracks.len(), 2);
    assert_eq!(single_tracks[0]["title"], "Devil");
    assert_ne!(single_tracks[0]["id"], devil["id"]);
    assert_eq!(single_tracks[0]["recordingId"], devil["recordingId"]);
    assert_eq!(
        single_tracks[1]["recordingId"],
        album["tracks"][1]["recordingId"]
    );
    let recordings: i64 = sqlx::query_scalar("SELECT count(*) FROM recordings")
        .fetch_one(&app.state.db)
        .await
        .unwrap();
    assert_eq!(recordings, 5, "7 треков, 5 разных ISRC");

    // Свежий альбом не перезагружается
    let hits = app.deezer.total_hits();
    app.get(&format!("/api/albums/{ep_id}"), &admin).await;
    assert_eq!(app.deezer.total_hits(), hits);
    // А устаревший (30 дней) — загружается заново
    sqlx::query("UPDATE albums SET fetched_at = now() - interval '31 days' WHERE deezer_id = $1")
        .bind(MODERN_SADNESS)
        .execute(&app.state.db)
        .await
        .unwrap();
    app.get(&format!("/api/albums/{ep_id}"), &admin).await;
    assert_eq!(
        app.deezer.hits(&format!("/album/{MODERN_SADNESS}/tracks")),
        2
    );
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn tracks_without_isrc_get_their_own_recording(db: PgPool) {
    let app = TestApp::new(db).await;
    let (_, admin) = app.admin("boss").await;
    let mut tracks = fixture(&format!("album_{DEVIL_SINGLE}_tracks"));
    for t in tracks["data"].as_array_mut().unwrap() {
        t["isrc"] = json!("");
    }
    app.deezer
        .set(&format!("/album/{DEVIL_SINGLE}/tracks"), tracks);
    let id = app.resolve_id("album", DEVIL_SINGLE, &admin).await;
    let res = app.get(&format!("/api/albums/{id}"), &admin).await;
    let t = res.body["tracks"].as_array().unwrap();
    assert!(t[0]["isrc"].is_null());
    assert!(t[0]["recordingId"].is_string());
    assert_ne!(t[0]["recordingId"], t[1]["recordingId"]);

    // Позже у Deezer появился ISRC: трек переезжает на общую запись, своя пустая удаляется
    app.deezer.set(
        &format!("/album/{DEVIL_SINGLE}/tracks"),
        fixture(&format!("album_{DEVIL_SINGLE}_tracks")),
    );
    let ep = app.resolve_id("album", MODERN_SADNESS, &admin).await;
    sqlx::query("UPDATE albums SET fetched_at = NULL WHERE deezer_id = $1")
        .bind(DEVIL_SINGLE)
        .execute(&app.state.db)
        .await
        .unwrap();
    let res = app.get(&format!("/api/albums/{id}"), &admin).await;
    let ep_res = app.get(&format!("/api/albums/{ep}"), &admin).await;
    assert_eq!(
        res.body["tracks"][0]["recordingId"],
        ep_res.body["tracks"][3]["recordingId"]
    );
    let recordings: i64 = sqlx::query_scalar("SELECT count(*) FROM recordings")
        .fetch_one(&app.state.db)
        .await
        .unwrap();
    assert_eq!(recordings, 5);
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn track_artists_come_from_contributors(db: PgPool) {
    let app = TestApp::new(db).await;
    let (_, admin) = app.admin("boss").await;
    let track = app.resolve("track", LIGHT_ME_UP, &admin).await;
    assert_eq!(track["type"], "track");
    assert_eq!(track["isrc"], "GBWUL2035890");
    assert_eq!(
        track["artists"]
            .as_array()
            .unwrap()
            .iter()
            .map(|a| (a["name"].as_str().unwrap(), a["role"].as_str().unwrap()))
            .collect::<Vec<_>>(),
        vec![("Lowborn", "main"), ("vista", "featured")]
    );
    // Альбом трека: облегчённая строка, а при открытии — полная, участники трека не затираются
    let album_id = track["album"]["id"].as_str().unwrap();
    let album = app.get(&format!("/api/albums/{album_id}"), &admin).await;
    assert_eq!(
        album.body["tracks"][0]["artists"].as_array().unwrap().len(),
        2
    );

    let collab = app.resolve("track", LONG_ISLAND, &admin).await;
    let names: Vec<&str> = collab["artists"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| a["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, vec!["MellSher", "5opka"]);
    assert!(
        collab["artists"]
            .as_array()
            .unwrap()
            .iter()
            .all(|a| a["role"] == "main")
    );

    // Трек каталога целиком загружается при открытии
    let id = app.resolve_id("track", CHURCH_ON_SUNDAY, &admin).await;
    let res = app.get(&format!("/api/tracks/{id}"), &admin).await;
    assert_eq!(res.status, StatusCode::OK);
    assert_eq!(res.body["title"], "Church On Sunday");
    assert_eq!(res.body["liked"], false);
    let unknown = uuid::Uuid::now_v7();
    assert_eq!(
        app.get(&format!("/api/tracks/{unknown}"), &admin)
            .await
            .status,
        StatusCode::NOT_FOUND
    );
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn catalog_search_maps_deezer_to_our_entities(db: PgPool) {
    let app = TestApp::new(db).await;
    let (_, admin) = app.admin("boss").await;
    let res = app.get("/api/search?q=Lowborn", &admin).await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    assert_eq!(res.body["scope"], "catalog");
    let tracks = res.body["tracks"].as_array().unwrap();
    let albums = res.body["albums"].as_array().unwrap();
    let artists = res.body["artists"].as_array().unwrap();
    assert_eq!(tracks.len(), 10);
    assert_eq!(albums.len(), 10);
    assert_eq!(artists.len(), 10);
    assert_eq!(tracks[0]["title"], "Doctor Doctor");
    assert_eq!(tracks[0]["isrc"], "QZWDD2320959");
    assert!(tracks[0]["recordingId"].is_string());
    assert_eq!(tracks[0]["album"]["title"], "Doctor Doctor");
    assert_eq!(artists[0]["name"], "Lowborn");
    assert_eq!(artists[0]["fans"], 611);
    // Альбом из поиска альбомов знает своего главного артиста
    assert_eq!(albums[0]["title"], "Lowborn");
    assert_eq!(albums[0]["artist"]["name"], "Wolves At The Gate");

    // Повторный поиск — те же наши id, без дублей
    let again = app
        .get("/api/search?q=Lowborn&types=track,artist&limit=5", &admin)
        .await;
    assert_eq!(again.body["tracks"][0]["id"], tracks[0]["id"]);
    assert_eq!(again.body["artists"][0]["id"], artists[0]["id"]);
    assert!(again.body["albums"].as_array().unwrap().is_empty());
    let lowborns: i64 = sqlx::query_scalar("SELECT count(*) FROM artists WHERE deezer_id = $1")
        .bind(LOWBORN)
        .fetch_one(&app.state.db)
        .await
        .unwrap();
    assert_eq!(lowborns, 1);

    for bad in [
        "/api/search?q=",
        "/api/search?q=%20%20",
        "/api/search?q=x&types=playlist",
        "/api/search?q=x&limit=0",
        "/api/search?q=x&limit=51",
        "/api/search?q=x&scope=everything",
    ] {
        assert_eq!(
            app.get(bad, &admin).await.status,
            StatusCode::BAD_REQUEST,
            "{bad}"
        );
    }
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn library_search_ignores_case_and_diacritics(db: PgPool) {
    let app = TestApp::new(db).await;
    let (_, admin) = app.admin("boss").await;
    let ep = app.resolve_id("album", MODERN_SADNESS, &admin).await;
    let album = app.get(&format!("/api/albums/{ep}"), &admin).await;
    let devil = album.body["tracks"][3]["id"].as_str().unwrap().to_string();
    let church = album.body["tracks"][0]["id"].as_str().unwrap().to_string();
    assert_eq!(
        app.put(&format!("/api/library/tracks/{devil}"), &admin)
            .await
            .status,
        StatusCode::NO_CONTENT
    );
    // Трек «на сервере» тоже в медиатеке, даже без лайка
    sqlx::query(
        "UPDATE recordings SET audio_status = 'ready' WHERE id = (SELECT recording_id FROM tracks WHERE id = $1)",
    )
    .bind(uuid::Uuid::parse_str(&church).unwrap())
    .execute(&app.state.db)
    .await
    .unwrap();
    sqlx::query("UPDATE tracks SET title = 'Chürch On Sünday' WHERE id = $1")
        .bind(uuid::Uuid::parse_str(&church).unwrap())
        .execute(&app.state.db)
        .await
        .unwrap();

    let found = |body: &serde_json::Value| -> Vec<String> {
        body["tracks"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["id"].as_str().unwrap().to_string())
            .collect()
    };
    let res = app.get("/api/search?q=DEVIL&scope=library", &admin).await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    assert_eq!(res.body["scope"], "library");
    assert_eq!(found(&res.body), vec![devil.clone()]);
    // По артисту и по куску слова
    let res = app
        .get("/api/search?q=lowborn%20dev&scope=library", &admin)
        .await;
    assert_eq!(found(&res.body), vec![devil.clone()]);
    // Диакритика не мешает
    let res = app
        .get("/api/search?q=church&scope=library&types=track", &admin)
        .await;
    assert_eq!(found(&res.body), vec![church]);
    // Не в медиатеке — не находится, хотя есть в каталоге
    let res = app.get("/api/search?q=twss&scope=library", &admin).await;
    assert!(found(&res.body).is_empty());
    let res = app
        .get("/api/search?q=lowborn&scope=library&types=artist", &admin)
        .await;
    assert_eq!(res.body["artists"][0]["name"], "Lowborn");
    // В Deezer библиотечный поиск не ходит
    assert_eq!(app.deezer.hits("/search/track"), 0);
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn deezer_failures_and_lookup(db: PgPool) {
    let app = TestApp::new(db).await;
    let (_, admin) = app.admin("boss").await;
    // «Квота исчерпана» приходит с HTTP 200: клиент ждёт и повторяет
    app.deezer.quota_errors(2);
    let id = app.resolve_id("track", CHURCH_ON_SUNDAY, &admin).await;
    assert_eq!(app.deezer.hits(&format!("/track/{CHURCH_ON_SUNDAY}")), 3);
    // Нет у Deezer — 404, а не 500
    let res = app.get("/api/catalog/deezer/album/42", &admin).await;
    assert_eq!(res.status, StatusCode::NOT_FOUND);
    assert_eq!(res.code(), "not_found");
    assert_eq!(
        app.get("/api/catalog/deezer/playlist/42", &admin)
            .await
            .status,
        StatusCode::BAD_REQUEST
    );
    // Квота не кончается — 503 с понятным текстом
    app.deezer.quota_errors(100);
    let res = app.get("/api/catalog/deezer/artist/1", &admin).await;
    assert_eq!(res.status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(res.code(), "unavailable");
    app.deezer.quota_errors(0);

    let unknown = uuid::Uuid::now_v7();
    let res = app
        .post(
            "/api/tracks/lookup",
            json!({ "ids": [unknown, id, id] }),
            &admin,
        )
        .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    let items = res.body["items"].as_array().unwrap();
    assert_eq!(
        items.len(),
        2,
        "неизвестный пропущен, порядок и повторы сохранены"
    );
    assert_eq!(items[0]["id"], id.as_str());
    let many: Vec<uuid::Uuid> = (0..501).map(|_| uuid::Uuid::now_v7()).collect();
    let res = app
        .post("/api/tracks/lookup", json!({ "ids": many }), &admin)
        .await;
    assert_eq!(res.status, StatusCode::BAD_REQUEST);
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn catalog_needs_a_role(db: PgPool) {
    let app = TestApp::new(db).await;
    let waiting = Auth::Cookie(app.login(&crate::mock::Person::new("ann")).await);
    for uri in [
        "/api/search?q=x",
        "/api/catalog/deezer/artist/1",
        "/api/home",
        "/api/sync",
    ] {
        assert_eq!(
            app.get(uri, &Auth::None).await.status,
            StatusCode::UNAUTHORIZED,
            "{uri}"
        );
        let res = app.get(uri, &waiting).await;
        assert_eq!(res.status, StatusCode::FORBIDDEN, "{uri}");
        assert_eq!(res.code(), "no_access");
    }
    // Слушателю каталог и медиатека открыты, админка — нет
    let listener = app.listener("lis").await;
    assert_eq!(app.get("/api/sync", &listener).await.status, StatusCode::OK);
    assert_eq!(
        app.get("/api/admin/jobs", &listener).await.status,
        StatusCode::FORBIDDEN
    );
}

#[tokio::test]
async fn deezer_chart_and_lists_parse() {
    let mock = crate::deezer::DeezerMock::start().await;
    let deezer = crate::common::fast_deezer(&mock.base);
    let chart = deezer.chart(5).await.unwrap();
    assert!(!chart.tracks.data.is_empty());
    assert!(!chart.albums.data.is_empty());
    assert!(!chart.artists.data.is_empty());
    // В чарте у треков нет ISRC, зато есть альбом и артист
    let track = &chart.tracks.data[0];
    assert!(track.isrc.is_none());
    assert!(track.album.is_some() && track.artist.is_some());
    let albums = deezer.artist_albums(LOWBORN).await.unwrap();
    assert_eq!(albums.len(), 54);
    assert!(
        albums.iter().all(|a| a.artist.is_none()),
        "в списке релизов главного артиста нет"
    );
    let top = deezer.artist_top(LOWBORN, 10).await.unwrap();
    assert!(top.iter().all(|t| !t.contributors.is_empty()));
    assert!(matches!(
        deezer.track(1).await,
        Err(dalywave_server::deezer::DeezerError::NotFound)
    ));
}
