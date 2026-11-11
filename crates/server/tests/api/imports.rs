//! Импорт: архив Spotify (загрузка → сопоставление в фоне с заглушкой Deezer → медиатека и история),
//! списки, свои файлы, исправление несопоставленного, повторный импорт без дублей.

use std::io::{Cursor, Write};

use axum::{
    body::Body,
    http::{Method, Request, StatusCode, header},
};
use dalywave_server::jobs::worker::Outcome;
use serde_json::{Value, json};
use sqlx::PgPool;

use crate::{
    common::{Auth, PUBLIC_URL, Resp, TestApp},
    deezer::fixture,
};

const LIBRARY: &str = r#"{
  "tracks": [
    {"artist": "Lowborn", "album": "Devil", "track": "Devil", "uri": "spotify:track:devil"},
    {"artist": "Lowborn", "album": "Crazy", "track": "Crazy", "uri": "spotify:track:crazy"},
    {"artist": "Nobody At All", "album": "Void", "track": "Совсем неизвестная песня", "uri": "spotify:track:void"}
  ],
  "albums": [{"artist": "Lowborn", "album": "Devil", "uri": "spotify:album:devil"}],
  "artists": [{"name": "Lowborn", "uri": "spotify:artist:lowborn"}]
}"#;

const PLAYLISTS: &str = r#"{"playlists": [{
  "name": "Ночь", "description": "для дороги",
  "items": [
    {"track": {"trackName": "Hurt", "artistName": "Lowborn", "albumName": "Hurt", "trackUri": "spotify:track:hurt"}},
    {"track": {"trackName": "Devil", "artistName": "Lowborn", "albumName": "Devil", "trackUri": "spotify:track:devil"}},
    {"track": {"trackName": "Doctor Doctor", "artistName": "Lowborn", "albumName": "Doctor Doctor", "trackUri": "spotify:track:doc"}}
  ]}]}"#;

const EXTENDED: &str = r#"[
  {"ts": "2024-05-01T10:00:05Z", "platform": "iOS 17.4 (iPhone15,2)", "ms_played": 162000,
   "master_metadata_track_name": "Devil", "master_metadata_album_artist_name": "Lowborn",
   "master_metadata_album_album_name": "Devil", "spotify_track_uri": "spotify:track:devil",
   "reason_end": "trackdone", "shuffle": false, "skipped": null, "incognito_mode": false},
  {"ts": "2024-05-01T10:01:00Z", "platform": "OS X 14.0 [arm 2]", "ms_played": 9000,
   "master_metadata_track_name": "Crazy", "master_metadata_album_artist_name": "Lowborn",
   "master_metadata_album_album_name": "Crazy", "spotify_track_uri": "spotify:track:crazy",
   "reason_end": "fwdbtn", "shuffle": true, "skipped": true, "incognito_mode": false},
  {"ts": "2024-05-01T10:05:00Z", "platform": "Android OS 14", "ms_played": 100000,
   "master_metadata_track_name": "Совсем неизвестная песня", "master_metadata_album_artist_name": "Nobody At All",
   "master_metadata_album_album_name": "Void", "spotify_track_uri": "spotify:track:void",
   "reason_end": "endplay", "incognito_mode": false}
]"#;

fn zip_of(files: &[(&str, &str)]) -> Vec<u8> {
    let mut buf = Cursor::new(Vec::new());
    {
        let mut w = zip::ZipWriter::new(&mut buf);
        let opts = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);
        for (name, body) in files {
            w.start_file(*name, opts).unwrap();
            w.write_all(body.as_bytes()).unwrap();
        }
        w.finish().unwrap();
    }
    buf.into_inner()
}

/// Загрузка тела как есть (архив, аудиофайл) — как из интерфейса, с Origin сайта.
async fn upload(
    app: &TestApp,
    method: Method,
    uri: &str,
    auth: &Auth,
    ct: &str,
    body: Vec<u8>,
) -> Resp {
    let mut req = Request::builder()
        .method(method)
        .uri(uri)
        .header(header::CONTENT_TYPE, ct)
        .header(header::CONTENT_LENGTH, body.len());
    if let Auth::Cookie(c) = auth {
        req = req
            .header(header::COOKIE, c)
            .header(header::ORIGIN, PUBLIC_URL);
    }
    app.send(req.body(Body::from(body)).unwrap()).await
}

/// Поиск Deezer отвечает треками, альбомами и артистами Lowborn — на любой запрос.
fn deezer_knows_lowborn(app: &TestApp) {
    app.deezer
        .set("/search/track", fixture("search_track_lowborn"));
    app.deezer
        .set("/search/album", fixture("search_album_lowborn"));
    app.deezer
        .set("/search/artist", fixture("search_artist_lowborn"));
}

/// Выполняет задачи полосы «Импорт», пока они есть.
async fn drain(app: &TestApp) -> Vec<String> {
    let mut kinds = Vec::new();
    while let Some((kind, outcome)) = app.run_job("import").await {
        assert_eq!(outcome, Outcome::Done, "{kind}");
        kinds.push(kind);
        assert!(kinds.len() < 50, "import jobs never end");
    }
    kinds
}

async fn summary(app: &TestApp, id: &str, auth: &Auth) -> Value {
    let res = app.get(&format!("/api/imports/{id}"), auth).await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    res.body
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn spotify_archive_lands_in_the_library(db: PgPool) {
    let app = TestApp::new(db).await;
    deezer_knows_lowborn(&app);
    let me = app.listener("ann").await;
    let archive = zip_of(&[
        ("Spotify Account Data/YourLibrary.json", LIBRARY),
        ("Spotify Account Data/Playlist1.json", PLAYLISTS),
        (
            "Spotify Extended Streaming History/Streaming_History_Audio_2024_0.json",
            EXTENDED,
        ),
        (
            "Spotify Extended Streaming History/ReadMeFirst_ExtendedStreamingHistory.pdf",
            "%PDF-1.4",
        ),
    ]);
    let res = upload(
        &app,
        Method::POST,
        "/api/imports/spotify?name=my_spotify_data.zip",
        &me,
        "application/zip",
        archive.clone(),
    )
    .await;
    assert_eq!(res.status, StatusCode::ACCEPTED, "{}", res.text);
    let id = res.body["id"].as_str().unwrap().to_string();
    assert_eq!(res.body["kind"], "spotify_archive");
    assert_eq!(res.body["status"], "running");
    assert_eq!(res.body["phase"], "matching");
    // 5 треков, альбом, артист
    assert_eq!(res.body["total"], 7);

    // Второй архив, пока идёт первый, — нельзя
    let again = upload(
        &app,
        Method::POST,
        "/api/imports/spotify",
        &me,
        "application/zip",
        archive.clone(),
    )
    .await;
    assert_eq!(again.status, StatusCode::CONFLICT, "{}", again.text);

    assert_eq!(drain(&app).await, ["import_match", "import_apply"]);
    let s = summary(&app, &id, &me).await;
    assert_eq!(s["status"], "done", "{s}");
    assert_eq!(s["phase"], "done");
    assert_eq!(s["processed"], 7);
    assert_eq!(s["matched"], 6);
    assert_eq!(s["unmatched"], 1);
    let stats = &s["stats"];
    assert_eq!(stats["likes"], 2, "{stats}");
    assert_eq!(stats["playlists"], 1);
    assert_eq!(stats["playlistTracks"], 3);
    assert_eq!(stats["albums"], 1);
    assert_eq!(stats["artists"], 1);
    assert_eq!(stats["plays"], 2);

    // Лайки в медиатеке и поставлены на скачивание
    let liked = app.get("/api/library/tracks", &me).await;
    let titles: Vec<&str> = liked.body["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["track"]["title"].as_str().unwrap())
        .collect();
    assert_eq!(titles.len(), 2);
    assert!(
        titles.contains(&"Devil") && titles.contains(&"Crazy"),
        "{titles:?}"
    );
    let queued: i64 =
        sqlx::query_scalar("SELECT count(*) FROM acquisitions WHERE reason = 'import'")
            .fetch_one(&app.state.db)
            .await
            .unwrap();
    assert!(queued >= 2, "{queued}");

    // Плейлист — то же название и порядок
    let lists = app.get("/api/playlists", &me).await;
    let night = lists.body["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["title"] == "Ночь")
        .expect("playlist «Ночь»")
        .clone();
    let detail = app
        .get(
            &format!("/api/playlists/{}", night["id"].as_str().unwrap()),
            &me,
        )
        .await;
    let order: Vec<&str> = detail.body["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["track"]["title"].as_str().unwrap())
        .collect();
    assert_eq!(order, ["Hurt", "Devil", "Doctor Doctor"]);
    assert_eq!(detail.body["playlist"]["description"], "для дороги");

    // Альбом и подписка
    let albums = app.get("/api/library/albums", &me).await;
    assert_eq!(
        albums.body["items"][0]["album"]["title"], "Devil",
        "{}",
        albums.text
    );
    let artists = app.get("/api/library/artists", &me).await;
    assert_eq!(
        artists.body["items"][0]["artist"]["name"], "Lowborn",
        "{}",
        artists.text
    );

    // История — с исходным временем, пропуском и платформой; «Продолжить» её не показывает
    let plays: Vec<(String, String, i32, Option<String>, String)> = sqlx::query_as(
        "SELECT to_char(started_at AT TIME ZONE 'UTC', 'YYYY-MM-DD HH24:MI:SS'), end_reason, played_ms, platform, source
         FROM play_events ORDER BY started_at",
    )
    .fetch_all(&app.state.db)
    .await
    .unwrap();
    assert_eq!(
        plays,
        [
            (
                "2024-05-01 09:57:23".into(),
                "completed".into(),
                162_000,
                Some("ios".into()),
                "import".into()
            ),
            (
                "2024-05-01 10:00:51".into(),
                "skipped".into(),
                9_000,
                Some("macos".into()),
                "import".into()
            ),
        ]
    );
    let home = app.get("/api/home", &me).await;
    assert_eq!(home.status, StatusCode::OK, "{}", home.text);
    let shelves: Vec<&str> = home.body["shelves"]
        .as_array()
        .map(|s| s.iter().filter_map(|x| x["id"].as_str()).collect())
        .unwrap_or_default();
    assert!(!shelves.contains(&"continue"), "{shelves:?}");

    // Несопоставленное — на проверку; исправление сразу ложится в лайки и историю
    let review = app
        .get(&format!("/api/imports/{id}/items?status=unmatched"), &me)
        .await;
    assert_eq!(review.status, StatusCode::OK, "{}", review.text);
    let items = review.body["items"].as_array().unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["title"], "Совсем неизвестная песня");
    assert_eq!(items[0]["plays"], 1);
    let item_id = items[0]["id"].as_str().unwrap().to_string();
    let hurt = sqlx::query_scalar::<_, uuid::Uuid>("SELECT id FROM tracks WHERE title = 'Hurt'")
        .fetch_one(&app.state.db)
        .await
        .unwrap();
    let fixed = app
        .call(
            Method::PUT,
            &format!("/api/imports/{id}/items/{item_id}"),
            Some(json!({ "trackId": hurt })),
            &me,
        )
        .await;
    assert_eq!(fixed.status, StatusCode::OK, "{}", fixed.text);
    assert_eq!(fixed.body["status"], "matched");
    assert_eq!(fixed.body["track"]["title"], "Hurt");
    let s = summary(&app, &id, &me).await;
    assert_eq!(s["unmatched"], 0);
    assert_eq!(s["stats"]["likes"], 3);
    assert_eq!(s["stats"]["plays"], 3);

    // Тот же архив ещё раз: всё из кеша, ничего не дублируется
    let searches = app.deezer.hits("/search/track");
    let res = upload(
        &app,
        Method::POST,
        "/api/imports/spotify",
        &me,
        "application/zip",
        archive,
    )
    .await;
    assert_eq!(res.status, StatusCode::ACCEPTED, "{}", res.text);
    let second = res.body["id"].as_str().unwrap().to_string();
    drain(&app).await;
    let s = summary(&app, &second, &me).await;
    assert_eq!(s["status"], "done");
    assert_eq!(s["unmatched"], 0, "manual fix is remembered: {s}");
    assert_eq!(s["stats"]["likes"], 0);
    assert_eq!(s["stats"]["plays"], 0);
    assert_eq!(app.deezer.hits("/search/track"), searches);
    let events: i64 = sqlx::query_scalar("SELECT count(*) FROM play_events")
        .fetch_one(&app.state.db)
        .await
        .unwrap();
    assert_eq!(events, 3);

    // История импортов и админка
    let mine = app.get("/api/imports", &me).await;
    assert_eq!(mine.body["items"].as_array().unwrap().len(), 2);
    let (_, admin) = app.admin("boss").await;
    let all = app.get("/api/admin/imports", &admin).await;
    assert_eq!(all.status, StatusCode::OK, "{}", all.text);
    assert_eq!(all.body["items"][0]["phase"], "done");
    assert_eq!(all.body["items"][1]["unmatched"], 0);
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn junk_uploads_are_refused(db: PgPool) {
    let app = TestApp::new(db).await;
    let me = app.listener("bob").await;
    let res = upload(
        &app,
        Method::POST,
        "/api/imports/spotify",
        &me,
        "application/zip",
        b"hello".to_vec(),
    )
    .await;
    assert_eq!(res.status, StatusCode::BAD_REQUEST, "{}", res.text);
    let empty = zip_of(&[("notes.txt", "nothing here")]);
    let res = upload(
        &app,
        Method::POST,
        "/api/imports/spotify",
        &me,
        "application/zip",
        empty,
    )
    .await;
    assert_eq!(res.status, StatusCode::BAD_REQUEST, "{}", res.text);
    assert!(
        res.body["error"]["message"]
            .as_str()
            .unwrap()
            .contains("Spotify")
    );
    // Форма с простым типом тела — нет
    let res = upload(
        &app,
        Method::POST,
        "/api/imports/spotify",
        &me,
        "text/plain",
        b"{}".to_vec(),
    )
    .await;
    assert_eq!(
        res.status,
        StatusCode::UNSUPPORTED_MEDIA_TYPE,
        "{}",
        res.text
    );
    // Без входа — нет
    let res = upload(
        &app,
        Method::POST,
        "/api/imports/spotify",
        &Auth::None,
        "application/zip",
        b"PK".to_vec(),
    )
    .await;
    assert_eq!(res.status, StatusCode::UNAUTHORIZED, "{}", res.text);
    let res = app
        .post(
            "/api/imports/list",
            json!({ "text": "\n\n# ничего\n" }),
            &me,
        )
        .await;
    assert_eq!(res.status, StatusCode::BAD_REQUEST, "{}", res.text);
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn list_becomes_a_playlist(db: PgPool) {
    let app = TestApp::new(db).await;
    deezer_knows_lowborn(&app);
    let me = app.listener("cid").await;
    let res = app
        .post(
            "/api/imports/list",
            json!({
                "text": "Lowborn — Crazy\n2. Lowborn - Devil\nНикто — Ничего\n",
                "target": "playlist",
                "playlistTitle": "Из заметок"
            }),
            &me,
        )
        .await;
    assert_eq!(res.status, StatusCode::ACCEPTED, "{}", res.text);
    assert_eq!(res.body["kind"], "text");
    assert_eq!(res.body["title"], "Из заметок");
    let id = res.body["id"].as_str().unwrap().to_string();
    drain(&app).await;
    let s = summary(&app, &id, &me).await;
    assert_eq!(s["status"], "done", "{s}");
    assert_eq!(s["matched"], 2);
    assert_eq!(s["unmatched"], 1);
    assert_eq!(s["stats"]["likes"], 0);
    let playlist = s["playlistId"].as_str().expect("playlist id");
    let detail = app.get(&format!("/api/playlists/{playlist}"), &me).await;
    assert_eq!(detail.body["playlist"]["title"], "Из заметок");
    let order: Vec<&str> = detail.body["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["track"]["title"].as_str().unwrap())
        .collect();
    assert_eq!(order, ["Crazy", "Devil"]);

    // Пропущенная позиция больше не ждёт проверки
    let items = app
        .get(&format!("/api/imports/{id}/items?status=unmatched"), &me)
        .await;
    let item = items.body["items"][0]["id"].as_str().unwrap().to_string();
    let res = app
        .call(
            Method::PUT,
            &format!("/api/imports/{id}/items/{item}"),
            Some(json!({ "skip": true })),
            &me,
        )
        .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    assert_eq!(res.body["status"], "skipped");
    assert_eq!(summary(&app, &id, &me).await["unmatched"], 0);

    // В «Мне нравится» — CSV с заголовком
    let res = app
        .post(
            "/api/imports/list",
            json!({ "text": "artist,title\nLowborn,Doctor Doctor\n", "target": "likes" }),
            &me,
        )
        .await;
    assert_eq!(res.body["kind"], "csv", "{}", res.text);
    let id = res.body["id"].as_str().unwrap().to_string();
    drain(&app).await;
    assert_eq!(summary(&app, &id, &me).await["stats"]["likes"], 1);
}

fn ffmpeg_available() -> bool {
    std::process::Command::new(std::env::var("DALYWAVE_FFMPEG").unwrap_or_else(|_| "ffmpeg".into()))
        .arg("-version")
        .output()
        .is_ok_and(|o| o.status.success())
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn own_lossless_file_becomes_the_primary(db: PgPool) {
    if !ffmpeg_available() {
        eprintln!("ffmpeg not found: skipping the file import test");
        return;
    }
    let app = TestApp::new(db).await;
    let me = app.listener("dee").await;
    // Трек с этим ISRC у Deezer — «Devil»; превью нет, поэтому решает совпадение ISRC
    let devil = fixture("search_track_lowborn")["data"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["title"] == "Devil")
        .unwrap()
        .clone();
    app.deezer.set("/track/isrc:QZ5FN2194847", devil);

    let dir = tempfile::tempdir().unwrap();
    let wav = dir.path().join("Lowborn - Devil.wav");
    let out = std::process::Command::new(
        std::env::var("DALYWAVE_FFMPEG").unwrap_or_else(|_| "ffmpeg".into()),
    )
    .args([
        "-v",
        "error",
        "-f",
        "lavfi",
        "-i",
        "sine=frequency=440:duration=6",
        "-ac",
        "2",
    ])
    .args([
        "-metadata",
        "title=Devil",
        "-metadata",
        "artist=Lowborn",
        "-metadata",
        "ISRC=QZ5FN2194847",
    ])
    .arg(&wav)
    .output()
    .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let bytes = std::fs::read(&wav).unwrap();

    let res = app.post("/api/imports/files", json!({}), &me).await;
    assert_eq!(res.status, StatusCode::CREATED, "{}", res.text);
    assert_eq!(res.body["phase"], "uploading");
    let id = res.body["id"].as_str().unwrap().to_string();

    let bad = upload(
        &app,
        Method::PUT,
        &format!("/api/imports/{id}/files?name=notes.txt"),
        &me,
        "text/x",
        b"hi".to_vec(),
    )
    .await;
    assert_eq!(bad.status, StatusCode::BAD_REQUEST, "{}", bad.text);
    let fake = upload(
        &app,
        Method::PUT,
        &format!("/api/imports/{id}/files?name=x.flac"),
        &me,
        "audio/flac",
        b"<html>".to_vec(),
    )
    .await;
    assert_eq!(fake.status, StatusCode::BAD_REQUEST, "{}", fake.text);

    let res = upload(
        &app,
        Method::PUT,
        &format!("/api/imports/{id}/files?name=Lowborn%20-%20Devil.wav"),
        &me,
        "audio/wav",
        bytes,
    )
    .await;
    assert_eq!(res.status, StatusCode::CREATED, "{}", res.text);
    assert_eq!(res.body["kind"], "file");
    assert_eq!(res.body["status"], "pending");
    assert_eq!(res.body["title"], "Devil");

    let res = app
        .post(&format!("/api/imports/{id}/finish"), json!({}), &me)
        .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    assert_eq!(res.body["status"], "running");
    assert_eq!(drain(&app).await, ["import_file"]);

    let s = summary(&app, &id, &me).await;
    assert_eq!(s["status"], "done", "{s}");
    assert_eq!(s["stats"]["files"], 1);
    assert_eq!(s["stats"]["upgraded"], 1);
    let items = app.get(&format!("/api/imports/{id}/items"), &me).await;
    let item = &items.body["items"][0];
    assert_eq!(item["status"], "matched", "{item}");
    assert_eq!(item["track"]["title"], "Devil");
    assert_eq!(item["file"]["codec"], "flac");
    assert_eq!(item["file"]["lossless"], true);
    assert_eq!(item["file"]["upgraded"], true);

    // WAV стал FLAC, громкость и огибающая посчитаны, запись готова и встала на полный анализ
    let (codec, container, source, lufs, points, status): (String, String, String, Option<f32>, i32, String) =
        sqlx::query_as(
            "SELECT f.codec, f.container, f.source, f.loudness_lufs, cardinality(f.waveform), r.audio_status
             FROM audio_files f JOIN recordings r ON r.primary_file_id = f.id",
        )
        .fetch_one(&app.state.db)
        .await
        .unwrap();
    assert_eq!(
        (codec.as_str(), container.as_str(), source.as_str()),
        ("flac", "flac", "upload")
    );
    assert!(lufs.is_some_and(|l| l < 0.0), "{lufs:?}");
    assert_eq!(points, 512);
    assert_eq!(status, "ready");
    let analysis: i64 = sqlx::query_scalar("SELECT count(*) FROM jobs WHERE kind = 'analyze_full'")
        .fetch_one(&app.state.db)
        .await
        .unwrap();
    assert_eq!(analysis, 1);
}
