use std::collections::HashSet;

use axum::http::StatusCode;
use serde_json::{Value, json};
use sqlx::PgPool;

use crate::{
    common::{Auth, TestApp},
    deezer::{DEVIL_SINGLE, LOWBORN, MODERN_SADNESS},
};

fn ids(page: &Value, key: &str, field: &str) -> HashSet<String> {
    page[key]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r[field].as_str().unwrap().to_string())
        .collect()
}

/// Все страницы с этого курсора; возвращает собранные страницы и последний курсор.
async fn pull(app: &TestApp, auth: &Auth, since: &str, limit: u32) -> (Vec<Value>, String) {
    let mut pages = Vec::new();
    let mut since = since.to_string();
    loop {
        let res = app
            .get(&format!("/api/sync?since={since}&limit={limit}"), auth)
            .await;
        assert_eq!(res.status, StatusCode::OK, "{}", res.text);
        since = res.body["cursor"].as_str().unwrap().to_string();
        let more = res.body["hasMore"] == true;
        pages.push(res.body);
        if !more {
            return (pages, since);
        }
        assert!(pages.len() < 100, "sync does not converge");
    }
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn sync_pulls_library_with_catalog_and_tombstones(db: PgPool) {
    let app = TestApp::new(db).await;
    let me = app.listener("ann").await;
    // Каталог, на который медиатека ещё не ссылается, в синхронизацию не попадает
    let artist = app.resolve_id("artist", LOWBORN, &me).await;
    app.get(&format!("/api/artists/{artist}"), &me).await;
    let ep = app.resolve_id("album", MODERN_SADNESS, &me).await;
    let album = app.get(&format!("/api/albums/{ep}"), &me).await;
    let tracks: Vec<String> = album.body["tracks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["id"].as_str().unwrap().to_string())
        .collect();

    let res = app.get("/api/sync", &me).await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    let empty = &res.body;
    assert_eq!(empty["hasMore"], false);
    for key in ["artists", "albums", "tracks", "likedTracks", "tombstones"] {
        assert!(empty[key].as_array().unwrap().is_empty(), "{key}");
    }
    let cursor = empty["cursor"].as_str().unwrap().to_string();

    // Лайк старого трека приносит сам трек, его альбом, артиста и запись
    app.put(&format!("/api/library/tracks/{}", tracks[3]), &me)
        .await;
    let res = app.get(&format!("/api/sync?since={cursor}"), &me).await;
    let page = &res.body;
    assert_eq!(
        ids(page, "likedTracks", "trackId"),
        HashSet::from([tracks[3].clone()])
    );
    assert!(ids(page, "tracks", "id").contains(&tracks[3]));
    assert!(ids(page, "albums", "id").contains(&ep));
    assert!(ids(page, "artists", "id").contains(&artist));
    let track = page["tracks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["id"] == tracks[3].as_str())
        .unwrap();
    assert_eq!(track["title"], "Devil");
    assert_eq!(track["albumId"], ep.as_str());
    assert_eq!(track["artists"][0]["artistId"], artist.as_str());
    assert_eq!(track["artists"][0]["role"], "main");
    let recording = track["recordingId"].as_str().unwrap();
    assert!(ids(page, "recordings", "id").contains(recording));
    // Лайк сразу ставит запись на скачивание
    assert_eq!(page["recordings"][0]["audioStatus"], "queued");
    let album_row = page["albums"]
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a["id"] == ep.as_str())
        .unwrap();
    assert_eq!(album_row["recordType"], "ep");
    assert!(
        album_row["image"]["small"]
            .as_str()
            .unwrap()
            .contains("/250.jpg?v=")
    );
    let cursor = page["cursor"].as_str().unwrap().to_string();

    // Ничего не изменилось. Строки могут прийти повторно (пока в базе идёт чужая пишущая транзакция —
    // а тесты идут параллельно в том же кластере), но ничего нового не появляется
    let res = app.get(&format!("/api/sync?since={cursor}"), &me).await;
    assert!(
        ids(&res.body, "likedTracks", "trackId").is_subset(&HashSet::from([tracks[3].clone()]))
    );
    assert!(res.body["tombstones"].as_array().unwrap().is_empty());
    let cursor: String = res.body["cursor"].as_str().unwrap().to_string();

    // Изменение каталожной строки из медиатеки (аудио скачалось) приходит само
    sqlx::query("UPDATE recordings SET audio_status = 'ready' WHERE id = $1::uuid")
        .bind(recording)
        .execute(&app.state.db)
        .await
        .unwrap();
    // …а записи не из медиатеки — нет
    sqlx::query(
        "UPDATE recordings SET audio_status = 'queued' WHERE id = (SELECT recording_id FROM tracks WHERE id = $1::uuid)",
    )
    .bind(&tracks[0])
    .execute(&app.state.db)
    .await
    .unwrap();
    let other_recording: String =
        sqlx::query_scalar("SELECT recording_id::text FROM tracks WHERE id = $1::uuid")
            .bind(&tracks[0])
            .fetch_one(&app.state.db)
            .await
            .unwrap();
    let res = app.get(&format!("/api/sync?since={cursor}"), &me).await;
    let recs = res.body["recordings"].as_array().unwrap();
    let ready = recs
        .iter()
        .find(|r| r["id"] == recording)
        .expect("changed recording");
    assert_eq!(ready["audioStatus"], "ready");
    assert!(recs.iter().all(|r| r["id"] != other_recording.as_str()));
    let cursor = res.body["cursor"].as_str().unwrap().to_string();

    // Отмена лайка — tombstone с ключом в camelCase
    app.delete(&format!("/api/library/tracks/{}", tracks[3]), &me)
        .await;
    let res = app.get(&format!("/api/sync?since={cursor}"), &me).await;
    let tombstones = res.body["tombstones"].as_array().unwrap();
    assert_eq!(tombstones.len(), 1);
    assert_eq!(tombstones[0]["entity"], "likedTracks");
    assert_eq!(tombstones[0]["key"], json!({ "trackId": tracks[3] }));
    assert!(tombstones[0]["rev"].as_i64().unwrap() > 0);
    assert!(
        res.body["likedTracks"].as_array().unwrap().is_empty(),
        "лайка больше нет"
    );

    // Чужие изменения не приходят
    let other = app.listener("bob").await;
    app.put(&format!("/api/library/tracks/{}", tracks[1]), &other)
        .await;
    let cursor = res.body["cursor"].as_str().unwrap().to_string();
    let res = app.get(&format!("/api/sync?since={cursor}"), &me).await;
    assert!(res.body["likedTracks"].as_array().unwrap().is_empty());
    assert!(
        res.body["tombstones"]
            .as_array()
            .unwrap()
            .iter()
            .all(|t| t["key"] == json!({ "trackId": tracks[3] }))
    );

    for bad in [
        "/api/sync?since=nope",
        "/api/sync?since=1.0.2",
        "/api/sync?limit=0",
        "/api/sync?limit=2001",
    ] {
        assert_eq!(
            app.get(bad, &me).await.status,
            StatusCode::BAD_REQUEST,
            "{bad}"
        );
    }
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn sync_pages_through_a_big_library(db: PgPool) {
    let app = TestApp::new(db).await;
    let me = app.listener("ann").await;
    let ep = app.resolve_id("album", MODERN_SADNESS, &me).await;
    let single = app.resolve_id("album", DEVIL_SINGLE, &me).await;
    app.put(&format!("/api/library/albums/{ep}"), &me).await;
    app.put(&format!("/api/library/albums/{single}"), &me).await;
    let album = app.get(&format!("/api/albums/{ep}"), &me).await;
    let tracks: Vec<String> = album.body["tracks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["id"].as_str().unwrap().to_string())
        .collect();
    for t in &tracks {
        app.put(&format!("/api/library/tracks/{t}"), &me).await;
    }
    let res = app
        .post(
            "/api/playlists",
            json!({ "title": "Всё", "trackIds": tracks }),
            &me,
        )
        .await;
    let playlist = res.body["playlist"]["id"].as_str().unwrap().to_string();

    // Маленькие страницы: всё приходит, курсор сходится
    let (pages, cursor) = pull(&app, &me, "", 3).await;
    assert!(pages.len() > 3, "{} pages", pages.len());
    let liked: HashSet<String> = pages
        .iter()
        .flat_map(|p| ids(p, "likedTracks", "trackId"))
        .collect();
    assert_eq!(liked, tracks.iter().cloned().collect());
    let saved: HashSet<String> = pages
        .iter()
        .flat_map(|p| ids(p, "savedAlbums", "albumId"))
        .collect();
    assert_eq!(saved, HashSet::from([ep.clone(), single.clone()]));
    let items: HashSet<String> = pages
        .iter()
        .flat_map(|p| ids(p, "playlistItems", "trackId"))
        .collect();
    assert_eq!(items.len(), 5);
    let playlists: HashSet<String> = pages
        .iter()
        .flat_map(|p| ids(p, "playlists", "id"))
        .collect();
    assert_eq!(playlists, HashSet::from([playlist.clone()]));
    // Треки сохранённых альбомов — тоже (7 треков: 5 в EP и 2 в сингле)
    let all_tracks: HashSet<String> = pages.iter().flat_map(|p| ids(p, "tracks", "id")).collect();
    assert_eq!(all_tracks.len(), 7);
    let recordings: HashSet<String> = pages
        .iter()
        .flat_map(|p| ids(p, "recordings", "id"))
        .collect();
    assert_eq!(recordings.len(), 5);
    // Посреди прохода курсор составной, в конце — простой
    assert!(pages[0]["cursor"].as_str().unwrap().contains('.'));
    assert!(!cursor.contains('.'));

    // Один большой запрос даёт то же
    let (big, _) = pull(&app, &me, "", 2000).await;
    assert_eq!(big.len(), 1);
    assert_eq!(ids(&big[0], "likedTracks", "trackId"), liked);
    assert_eq!(ids(&big[0], "tracks", "id"), all_tracks);

    // Удаление плейлиста: tombstone плейлиста; позиции приложение удаляет вместе с ним
    app.delete(&format!("/api/playlists/{playlist}"), &me).await;
    let (pages, _) = pull(&app, &me, &cursor, 500).await;
    let tombstones: Vec<&Value> = pages
        .iter()
        .flat_map(|p| p["tombstones"].as_array().unwrap())
        .collect();
    assert_eq!(tombstones.len(), 1);
    assert_eq!(tombstones[0]["entity"], "playlists");
    assert_eq!(tombstones[0]["key"], json!({ "id": playlist }));
}

/// Номер изменения выдаётся при записи, а видна строка — после коммита. Транзакция, взявшая номер раньше,
/// но закоммиченная позже чужой, не должна потеряться: курсор держится на xmin снимка.
#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn late_commits_are_not_lost(db: PgPool) {
    let app = TestApp::new(db).await;
    let me = app.listener("ann").await;
    let user: uuid::Uuid = app.user_id(&me).await.parse().unwrap();
    let ep = app.resolve_id("album", MODERN_SADNESS, &me).await;
    let album = app.get(&format!("/api/albums/{ep}"), &me).await;
    let tracks: Vec<uuid::Uuid> = album.body["tracks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["id"].as_str().unwrap().parse().unwrap())
        .collect();
    let (_, cursor) = pull(&app, &me, "", 500).await;

    // Медленная транзакция берёт номер первой…
    let mut slow = app.state.db.begin().await.unwrap();
    let slow_rev: i64 = sqlx::query_scalar(
        "INSERT INTO liked_tracks (user_id, track_id) VALUES ($1, $2) RETURNING rev",
    )
    .bind(user)
    .bind(tracks[0])
    .fetch_one(&mut *slow)
    .await
    .unwrap();
    // …быстрая — позже и коммитится сразу
    app.put(&format!("/api/library/tracks/{}", tracks[1]), &me)
        .await;
    let (pages, cursor) = pull(&app, &me, &cursor, 500).await;
    let liked: HashSet<String> = pages
        .iter()
        .flat_map(|p| ids(p, "likedTracks", "trackId"))
        .collect();
    assert_eq!(liked, HashSet::from([tracks[1].to_string()]));
    let fast_rev = pages[0]["likedTracks"][0]["rev"].as_i64().unwrap();
    assert!(
        slow_rev < fast_rev,
        "номер медленной меньше: курсор по номеру её бы пропустил"
    );

    slow.commit().await.unwrap();
    let (pages, _) = pull(&app, &me, &cursor, 500).await;
    let liked: HashSet<String> = pages
        .iter()
        .flat_map(|p| ids(p, "likedTracks", "trackId"))
        .collect();
    assert!(
        liked.contains(&tracks[0].to_string()),
        "поздний коммит пришёл"
    );
}
