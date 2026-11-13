//! Выгрузка медиатеки (CSV, M3U8, JSON, ZIP — только свои данные) и встраиваемый плеер: страница
//! `/embed/{kind}/{id}` без входа, «Можно встраивать» у плейлиста, отрывки вместо полного трека.

use std::io::Read;

use axum::http::{Method, StatusCode, header};
use serde_json::{Value, json};
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    common::{Auth, PUBLIC_URL, Resp, TestApp},
    deezer::LIGHT_ME_UP,
    fetcher::ep_tracks,
};

fn header_of(res: &Resp, name: header::HeaderName) -> &str {
    res.headers
        .get(name)
        .map(|v| v.to_str().unwrap())
        .unwrap_or_default()
}

/// Плейлист с треками в папке; название с запятой и кавычками — для экранирования CSV.
async fn playlist(app: &TestApp, me: &Auth, title: &str, tracks: &[Uuid]) -> String {
    let res = app
        .post(
            "/api/playlists",
            json!({ "title": title, "trackIds": tracks }),
            me,
        )
        .await;
    assert_eq!(res.status, StatusCode::CREATED, "{}", res.text);
    res.body["playlist"]["id"].as_str().unwrap().to_string()
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn playlist_and_liked_export_formats(db: PgPool) {
    let app = TestApp::new(db).await;
    let me = app.listener("ann").await;
    let tracks = ep_tracks(&app, &me).await;
    for t in &tracks[..2] {
        let res = app.put(&format!("/api/library/tracks/{t}"), &me).await;
        assert!(res.status.is_success(), "{}", res.text);
    }
    let pl = playlist(&app, &me, "Ночь, \"дорога\"", &tracks).await;

    // CSV: BOM, заголовок, строки по порядку плейлиста, имя файла в UTF-8
    let res = app
        .get(&format!("/api/export/playlists/{pl}?format=csv"), &me)
        .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    assert_eq!(
        header_of(&res, header::CONTENT_TYPE),
        "text/csv; charset=utf-8"
    );
    let disposition = header_of(&res, header::CONTENT_DISPOSITION);
    assert!(disposition.starts_with("attachment; filename=\"playlist.csv\"; filename*=UTF-8''"));
    assert!(
        disposition.contains("%D0%9D%D0%BE%D1%87%D1%8C"),
        "{disposition}"
    );
    assert!(res.bytes.starts_with(b"\xEF\xBB\xBF"), "BOM для Excel");
    let text = String::from_utf8(res.bytes[3..].to_vec()).unwrap();
    let lines: Vec<&str> = text.split("\r\n").filter(|l| !l.is_empty()).collect();
    assert_eq!(
        lines[0],
        "Artist Name(s),Track Name,Album Name,ISRC,Duration (ms),Added At"
    );
    assert_eq!(lines.len(), tracks.len() + 1);
    let first = app
        .get(&format!("/api/tracks/{}", tracks[0]), &me)
        .await
        .body;
    assert!(
        lines[1].contains(first["title"].as_str().unwrap()),
        "{}",
        lines[1]
    );
    if let Some(isrc) = first["isrc"].as_str() {
        assert!(lines[1].contains(isrc));
    }

    // M3U8: ссылки на страницы треков
    let res = app
        .get(&format!("/api/export/playlists/{pl}?format=m3u8"), &me)
        .await;
    assert_eq!(res.status, StatusCode::OK);
    assert!(
        res.text
            .starts_with("#EXTM3U\n#PLAYLIST:Ночь, \"дорога\"\n#EXTINF:")
    );
    assert!(
        res.text
            .contains(&format!("{PUBLIC_URL}/track/{}\n", tracks[0]))
    );
    assert_eq!(res.text.matches("#EXTINF:").count(), tracks.len());

    // JSON
    let res = app
        .get(&format!("/api/export/playlists/{pl}?format=json"), &me)
        .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    assert_eq!(res.body["title"], "Ночь, \"дорога\"");
    assert_eq!(res.body["tracks"].as_array().unwrap().len(), tracks.len());
    assert_eq!(res.body["tracks"][0]["id"], tracks[0].to_string());
    assert!(res.body["tracks"][0]["addedAt"].is_string());

    // Любимые — новые первыми
    let res = app.get("/api/export/liked?format=json", &me).await;
    assert_eq!(res.status, StatusCode::OK);
    let liked = res.body["tracks"].as_array().unwrap();
    assert_eq!(liked.len(), 2);
    assert_eq!(liked[0]["id"], tracks[1].to_string());
    let res = app.get("/api/export/liked", &me).await;
    assert!(header_of(&res, header::CONTENT_DISPOSITION).contains("filename=\"liked.csv\""));

    // Права: чужой плейлист — 404, без входа — 401, архив списка — 400
    let bob = app.listener("bob").await;
    let res = app
        .get(&format!("/api/export/playlists/{pl}?format=csv"), &bob)
        .await;
    assert_eq!(res.status, StatusCode::NOT_FOUND);
    let res = app.get("/api/export/liked", &bob).await;
    assert_eq!(
        String::from_utf8(res.bytes[3..].to_vec())
            .unwrap()
            .lines()
            .count(),
        1,
        "у Боба лайков нет — только заголовок"
    );
    let res = app
        .get(&format!("/api/export/playlists/{pl}"), &Auth::None)
        .await;
    assert_eq!(res.status, StatusCode::UNAUTHORIZED);
    let res = app
        .get(&format!("/api/export/playlists/{pl}?format=zip"), &me)
        .await;
    assert_eq!(res.status, StatusCode::BAD_REQUEST);
    let res = app.get("/api/export/library?format=csv", &me).await;
    assert_eq!(res.status, StatusCode::BAD_REQUEST);
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn whole_library_as_zip_and_json(db: PgPool) {
    let app = TestApp::new(db).await;
    let me = app.listener("ann").await;
    let tracks = ep_tracks(&app, &me).await;
    app.put(&format!("/api/library/tracks/{}", tracks[0]), &me)
        .await;
    let a = playlist(&app, &me, "Бег", &tracks[..1]).await;
    let b = playlist(&app, &me, "Бег", &tracks[1..]).await;
    let c = playlist(&app, &me, "Вне папок / тест", &tracks).await;
    let res = app
        .post(
            "/api/playlist-folders",
            json!({ "title": "Спорт", "playlistIds": [a, b] }),
            &me,
        )
        .await;
    assert_eq!(res.status, StatusCode::CREATED, "{}", res.text);
    let user = app.user_id(&me).await;
    sqlx::query(
        "INSERT INTO play_events (client_event_id, user_id, track_id, started_at, played_ms, duration_ms, end_reason)
         VALUES (gen_random_uuid(), $1, $2, now(), 120000, 180000, 'completed')",
    )
    .bind(Uuid::parse_str(&user).unwrap())
    .bind(tracks[0])
    .execute(&app.state.db)
    .await
    .unwrap();
    // Чужой плейлист в выгрузку не попадает
    let bob = app.listener("bob").await;
    playlist(&app, &bob, "Чужой", &tracks).await;

    let res = app.get("/api/export/library", &me).await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    assert_eq!(header_of(&res, header::CONTENT_TYPE), "application/zip");
    assert!(header_of(&res, header::CONTENT_DISPOSITION).contains(".zip"));
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(res.bytes.clone())).unwrap();
    let mut names: Vec<String> = zip.file_names().map(|n| n.unwrap().to_string()).collect();
    names.sort();
    assert_eq!(
        names,
        [
            "Альбомы.csv",
            "Артисты.csv",
            "Любимые треки.csv",
            "Любимые треки.m3u8",
            "Медиатека.json",
            "Плейлисты/Вне папок _ тест.csv",
            "Плейлисты/Вне папок _ тест.m3u8",
            "Плейлисты/Спорт/Бег (2).csv",
            "Плейлисты/Спорт/Бег (2).m3u8",
            "Плейлисты/Спорт/Бег.csv",
            "Плейлисты/Спорт/Бег.m3u8",
        ]
    );
    let mut json = String::new();
    zip.by_name("Медиатека.json")
        .unwrap()
        .read_to_string(&mut json)
        .unwrap();
    let lib: Value = serde_json::from_str(&json).unwrap();
    assert_eq!(lib["format"], "dalywave-library");
    assert_eq!(lib["likedTracks"].as_array().unwrap().len(), 1);
    assert_eq!(lib["folders"][0]["title"], "Спорт");
    let pls = lib["playlists"].as_array().unwrap();
    assert_eq!(pls.len(), 3);
    let titles: Vec<&str> = pls.iter().map(|p| p["title"].as_str().unwrap()).collect();
    assert!(!titles.contains(&"Чужой"));
    let in_folder = pls.iter().find(|p| p["id"] == a.as_str()).unwrap();
    assert_eq!(in_folder["folder"], "Спорт");
    assert_eq!(in_folder["tracks"].as_array().unwrap().len(), 1);
    let free = pls.iter().find(|p| p["id"] == c.as_str()).unwrap();
    assert!(free["folder"].is_null());
    assert_eq!(free["tracks"].as_array().unwrap().len(), tracks.len());
    assert!(lib.get("history").is_none(), "история — только по просьбе");

    // С историей: отдельная таблица и раздел JSON
    let res = app.get("/api/export/library?history=true", &me).await;
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(res.bytes.clone())).unwrap();
    let mut history = String::new();
    zip.by_name("История.csv")
        .unwrap()
        .read_to_string(&mut history)
        .unwrap();
    assert!(history.starts_with("\u{feff}Played At,"));
    assert_eq!(history.lines().count(), 2);

    // Один JSON
    let res = app
        .get("/api/export/library?format=json&history=true", &me)
        .await;
    assert_eq!(res.status, StatusCode::OK);
    assert_eq!(res.body["history"].as_array().unwrap().len(), 1);
    assert_eq!(res.body["history"][0]["playedMs"], 120000);
    assert_eq!(res.body["playlists"].as_array().unwrap().len(), 3);
}

// ───────────── Встраиваемый плеер ─────────────

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn embed_pages_are_public_and_frameable(db: PgPool) {
    let app = TestApp::new(db).await;
    let me = app.listener("ann").await;
    let tracks = ep_tracks(&app, &me).await;
    let album = app
        .get(&format!("/api/tracks/{}", tracks[0]), &me)
        .await
        .body["album"]["id"]
        .as_str()
        .unwrap()
        .to_string();

    // Трек и альбом каталога — всегда, без входа
    let res = app
        .get(
            &format!("/embed/track/{}?theme=light", tracks[0]),
            &Auth::None,
        )
        .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    assert!(header_of(&res, header::CONTENT_TYPE).starts_with("text/html"));
    let csp = header_of(&res, header::CONTENT_SECURITY_POLICY);
    assert!(csp.contains("frame-ancestors *"), "{csp}");
    assert!(res.headers.get(header::X_FRAME_OPTIONS).is_none());
    assert!(res.text.contains("card light single"));
    assert!(res.text.contains("Открыть в DalyWave"));
    let res = app
        .get(&format!("/embed/album/{album}?lang=en"), &Auth::None)
        .await;
    assert_eq!(res.status, StatusCode::OK);
    assert!(res.text.contains("card dark"));
    assert!(res.text.contains("Open in DalyWave"));
    assert_eq!(res.text.matches("class=\"row\"").count(), tracks.len());
    // У остального сайта встраивание по-прежнему запрещено (страница превью — DENY)
    let res = app
        .get(&format!("/embed/artist/{}", Uuid::nil()), &Auth::None)
        .await;
    assert_eq!(res.status, StatusCode::NOT_FOUND);
    let res = app.get("/embed/nonsense/123", &Auth::None).await;
    assert_eq!(res.status, StatusCode::NOT_FOUND);

    // Плейлист — только с разрешения владельца
    let pl = playlist(&app, &me, "Ночь", &tracks).await;
    let page = format!("/embed/playlist/{pl}");
    assert_eq!(
        app.get(&page, &Auth::None).await.status,
        StatusCode::NOT_FOUND
    );
    let res = app.get(&format!("/api/playlists/{pl}/embed"), &me).await;
    assert_eq!(res.body, json!({ "embeddable": false }));
    let bob = app.listener("bob").await;
    let res = app
        .call(
            Method::PUT,
            &format!("/api/playlists/{pl}/embed"),
            Some(json!({ "embeddable": true })),
            &bob,
        )
        .await;
    assert_eq!(res.status, StatusCode::NOT_FOUND, "чужой плейлист");
    let res = app
        .call(
            Method::PUT,
            &format!("/api/playlists/{pl}/embed"),
            Some(json!({ "embeddable": true })),
            &me,
        )
        .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    let res = app.get(&page, &Auth::None).await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    assert!(res.text.contains("Ночь"));
    assert!(res.text.contains("Плейлист · "), "{}", res.text);
    assert_eq!(res.text.matches("class=\"row\"").count(), tracks.len());
    // Снова закрыли — снова 404
    app.call(
        Method::PUT,
        &format!("/api/playlists/{pl}/embed"),
        Some(json!({ "embeddable": false })),
        &me,
    )
    .await;
    assert_eq!(
        app.get(&page, &Auth::None).await.status,
        StatusCode::NOT_FOUND
    );
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn embed_clip_is_a_preview_never_the_full_track(db: PgPool) {
    let app = TestApp::new(db).await;
    let me = app.listener("ann").await;
    let id = app.resolve_id("track", LIGHT_ME_UP, &me).await;
    let id = Uuid::parse_str(&id).unwrap();

    // Звука на сервере нет — превью Deezer, без входа; трек на скачивание не ставится
    let res = app
        .get(&format!("/api/embed/tracks/{id}/clip"), &Auth::None)
        .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    assert_eq!(res.body["source"], "preview");
    assert_eq!(res.body["lengthMs"], 30_000);
    assert!(res.body["url"].as_str().unwrap().contains("dzcdn.net"));
    assert_eq!(header_of(&res, header::CACHE_CONTROL), "no-store");
    let jobs = |kind: &'static str| {
        let db = app.state.db.clone();
        async move {
            sqlx::query_scalar::<_, i64>("SELECT count(*) FROM jobs WHERE kind = $1")
                .bind(kind)
                .fetch_one(&db)
                .await
                .unwrap()
        }
    };
    assert_eq!(jobs("acquire").await, 0);
    assert_eq!(jobs("embed_clip").await, 0);
    let res = app
        .get(
            &format!("/api/embed/tracks/{}/clip", Uuid::nil()),
            &Auth::None,
        )
        .await;
    assert_eq!(res.status, StatusCode::NOT_FOUND);

    // Звук на сервере — нарезка встаёт в очередь (одна на трек), пока — превью
    let rec: Uuid = sqlx::query_scalar("SELECT recording_id FROM tracks WHERE id = $1")
        .bind(id)
        .fetch_one(&app.state.db)
        .await
        .unwrap();
    let file: Uuid = sqlx::query_scalar(
        "INSERT INTO audio_files (recording_id, storage_key, codec, container, lossless, bitrate_kbps, duration_ms,
                                  size_bytes, source, sha256)
         VALUES ($1, 'audio/' || $1 || '/full.m4a', 'opus', 'mp4', false, 160, 40000, 4096, 'youtube', $2)
         RETURNING id",
    )
    .bind(rec)
    .bind("ab".repeat(32))
    .fetch_one(&app.state.db)
    .await
    .unwrap();
    sqlx::query("UPDATE recordings SET audio_status = 'ready', primary_file_id = $2 WHERE id = $1")
        .bind(rec)
        .bind(file)
        .execute(&app.state.db)
        .await
        .unwrap();
    for _ in 0..2 {
        let res = app
            .get(&format!("/api/embed/tracks/{id}/clip"), &Auth::None)
            .await;
        assert_eq!(res.body["source"], "preview", "{}", res.text);
    }
    assert_eq!(jobs("embed_clip").await, 1);

    if !crate::quality::ffmpeg_available() {
        eprintln!("ffmpeg not found: skipping the clip part");
        return;
    }
    let key = format!("audio/{rec}/full.m4a");
    app.state
        .storage
        .put(&key, crate::quality::opus_seconds(40), "audio/mp4", None)
        .await
        .unwrap();
    let (kind, outcome) = app
        .run_job(dalywave_server::renditions::LANE)
        .await
        .expect("clip job");
    assert_eq!(kind, "embed_clip");
    assert!(
        matches!(outcome, dalywave_server::jobs::worker::Outcome::Done),
        "{outcome:?}"
    );
    let res = app
        .get(&format!("/api/embed/tracks/{id}/clip"), &Auth::None)
        .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    assert_eq!(res.body["source"], "chorus");
    let (start, length): (i32, i32) =
        sqlx::query_as("SELECT start_ms, length_ms FROM embed_clips WHERE track_id = $1")
            .bind(id)
            .fetch_one(&app.state.db)
            .await
            .unwrap();
    // Место припева (30 % трека) не дальше, чем позволяет файл
    assert_eq!((start, length), (10_000, 30_000));
    let clip = app
        .state
        .storage
        .get_bytes(&dalywave_server::embed::clip_key(id, 10_000))
        .await
        .unwrap();
    assert!(clip.len() > 1000 && &clip[4..8] == b"ftyp", "MP4");
}
