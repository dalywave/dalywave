//! Своя музыка из папки и хранилище на диске: сервер без Deezer и без загрузчика. Файлы делает ffmpeg
//! (синус с тегами); без ffmpeg тесты папки пропускаются.

use std::path::{Path, PathBuf};

use axum::{
    body::Body,
    http::{Method, Request, StatusCode, header},
};
use dalywave_server::{
    config::StorageKind,
    storage::{AUDIO_GET_TTL, UPLOAD_PUT_TTL},
};
use serde_json::Value;
use sqlx::PgPool;

use crate::{
    common::{Auth, TestApp},
    quality::ffmpeg_available,
};

fn ffmpeg() -> String {
    std::env::var("DALYWAVE_FFMPEG").unwrap_or_else(|_| "ffmpeg".into())
}

/// Синус с тегами; `codec` — аргументы кодека ffmpeg.
fn tone(path: &Path, seconds: u32, freq: u32, codec: &[&str], tags: &[(&str, &str)]) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let sine = format!("sine=frequency={freq}:duration={seconds}");
    let mut cmd = std::process::Command::new(ffmpeg());
    cmd.args(["-v", "error", "-y", "-f", "lavfi", "-i", &sine, "-ac", "2"]);
    for (k, v) in tags {
        cmd.arg("-metadata").arg(format!("{k}={v}"));
    }
    cmd.args(codec).arg(path);
    let out = cmd.output().unwrap();
    assert!(
        out.status.success(),
        "ffmpeg: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

fn cover(path: &Path) {
    let out = std::process::Command::new(ffmpeg())
        .args([
            "-v",
            "error",
            "-y",
            "-f",
            "lavfi",
            "-i",
            "color=c=0x3366aa:s=64x64",
            "-frames:v",
            "1",
        ])
        .arg(path)
        .output()
        .unwrap();
    assert!(out.status.success());
}

/// Папка как у человека: альбом с тегами и обложкой, папка без тегов, AIFF, битый файл, WMA.
fn music(root: &Path) {
    let album = root.join("Кино/Группа крови");
    tone(
        &album.join("01 - Группа крови.flac"),
        3,
        440,
        &["-c:a", "flac"],
        &[
            ("title", "Группа крови"),
            ("artist", "Кино"),
            ("album", "Группа крови"),
            ("track", "1/2"),
            ("date", "1988"),
            ("genre", "Рок"),
        ],
    );
    tone(
        &album.join("02 - Закрой за мной дверь.mp3"),
        3,
        550,
        &["-c:a", "libmp3lame", "-b:a", "192k"],
        &[
            ("title", "Закрой за мной дверь"),
            ("artist", "Кино"),
            ("album", "Группа крови"),
            ("track", "2/2"),
            ("date", "1988"),
        ],
    );
    cover(&album.join("cover.jpg"));
    tone(
        &root.join("Сборник/03.m4a"),
        3,
        660,
        &["-c:a", "aac", "-b:a", "128k"],
        &[
            ("title", "Песня"),
            ("artist", "Аквариум feat. Гость"),
            ("album_artist", "Various Artists"),
            ("album", "Хиты"),
            ("track", "3"),
        ],
    );
    tone(
        &root.join("Folder Artist/Folder Album/04 Untagged.wav"),
        2,
        330,
        &["-c:a", "pcm_s16le"],
        &[],
    );
    tone(
        &root.join("Lossless/05 Aiff.aiff"),
        2,
        770,
        &["-c:a", "pcm_s16be", "-write_id3v2", "1"],
        &[("title", "AIFF трек"), ("artist", "Студия")],
    );
    std::fs::write(root.join("Lossless/broken.mp3"), b"not an mp3 at all").unwrap();
    std::fs::write(
        root.join("Lossless/old.wma"),
        b"0&\xb2u\x8ef\xcf\x11 garbage",
    )
    .unwrap();
}

async fn local_app(db: PgPool, music: &Path, store: &Path) -> TestApp {
    let (music, store) = (music.to_path_buf(), store.to_path_buf());
    TestApp::with(db, move |c| {
        c.features.deezer = false;
        c.features.downloader = false;
        c.library.dirs = vec![music];
        c.storage.kind = StorageKind::Fs;
        c.storage.path = store;
    })
    .await
}

async fn scan(app: &TestApp, paths: Vec<String>) -> Value {
    dalywave_server::library::request_scan(&app.state.db, paths)
        .await
        .unwrap();
    let (kind, _) = app.run_job("local_scan").await.expect("scan job");
    assert_eq!(kind, "library_scan");
    let (_, auth) = app.admin("lib-admin").await;
    app.get("/api/admin/library", &auth).await.body
}

/// Путь и query подписанной ссылки на наш API.
fn local(url: &str) -> String {
    let u = url::Url::parse(url).unwrap();
    assert_eq!(u.path().split('/').nth(2), Some("files"), "{url}");
    format!("{}?{}", u.path(), u.query().unwrap())
}

async fn track_by_title(db: &PgPool, title: &str) -> String {
    sqlx::query_scalar::<_, uuid::Uuid>("SELECT id FROM tracks WHERE title = $1")
        .bind(title)
        .fetch_one(db)
        .await
        .unwrap()
        .to_string()
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn local_folder_plays_without_deezer(db: PgPool) {
    if !ffmpeg_available() {
        eprintln!("ffmpeg not found: skipping");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("music");
    let store = dir.path().join("data");
    music(&root);
    let app = local_app(db.clone(), &root, &store).await;
    let auth = app.listener("lib-listener").await;

    let config = app.get("/api/config", &Auth::None).await;
    assert_eq!(config.status, StatusCode::OK);
    assert_eq!(config.body["deezer"], false);
    assert_eq!(config.body["downloader"], false);
    assert_eq!(config.body["library"], true);
    assert_eq!(config.body["storage"], "fs");

    let status = scan(&app, Vec::new()).await;
    assert_eq!(status["scan"]["status"], "done", "{status}");
    assert_eq!(status["scan"]["added"], 5, "{status}");
    assert_eq!(status["tracks"], 5);
    assert_eq!(status["errors"], 1, "{status}");
    assert_eq!(status["unsupported"], 1, "{status}");
    assert_eq!(status["transcoded"], 1, "AIFF играется копией");
    assert_eq!(status["dirs"][0]["available"], true);
    assert_eq!(status["dirs"][0]["files"], 5);

    // Каталог из тегов: альбом с обложкой из папки, исполнитель, сборник
    let kino = track_by_title(&db, "Группа крови").await;
    let t = app.get(&format!("/api/tracks/{kino}"), &auth).await;
    assert_eq!(t.status, StatusCode::OK, "{}", t.text);
    let album_id = t.body["track"]["album"]["id"]
        .as_str()
        .or(t.body["album"]["id"].as_str())
        .map(str::to_string);
    let album_id = match album_id {
        Some(a) => a,
        None => {
            sqlx::query_scalar::<_, uuid::Uuid>("SELECT album_id FROM tracks WHERE id = $1::uuid")
                .bind(&kino)
                .fetch_one(&db)
                .await
                .unwrap()
                .to_string()
        }
    };
    let album = app.get(&format!("/api/albums/{album_id}"), &auth).await;
    assert_eq!(album.status, StatusCode::OK, "{}", album.text);
    assert_eq!(album.body["album"]["title"], "Группа крови");
    assert_eq!(album.body["tracks"].as_array().unwrap().len(), 2);
    let cover: Option<String> =
        sqlx::query_scalar("SELECT cover_key FROM albums WHERE id = $1::uuid")
            .bind(&album_id)
            .fetch_one(&db)
            .await
            .unwrap();
    assert!(cover.is_some(), "обложка из cover.jpg");
    let various: String = sqlx::query_scalar(
        "SELECT a.name FROM albums al JOIN artists a ON a.id = al.artist_id WHERE al.title = 'Хиты'",
    )
    .fetch_one(&db)
    .await
    .unwrap();
    assert_eq!(various, "Various Artists");
    let featured: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM track_artists ta JOIN artists a ON a.id = ta.artist_id
         WHERE a.name = 'Гость' AND ta.role = 'featured'",
    )
    .fetch_one(&db)
    .await
    .unwrap();
    assert_eq!(featured, 1);
    let folder: String = sqlx::query_scalar(
        "SELECT al.title FROM tracks t JOIN albums al ON al.id = t.album_id WHERE t.title = 'Untagged'",
    )
    .fetch_one(&db)
    .await
    .unwrap();
    assert_eq!(folder, "Folder Album", "без тегов — альбом по папке");

    // Поиск по каталогу без Deezer ищет в своей музыке
    let found = app
        .get("/api/search?q=%D0%BA%D0%B8%D0%BD%D0%BE", &auth)
        .await;
    assert_eq!(found.status, StatusCode::OK, "{}", found.text);
    assert!(
        !found.body["tracks"].as_array().unwrap().is_empty()
            || !found.body["artists"].as_array().unwrap().is_empty(),
        "{}",
        found.text
    );

    // Воспроизведение: файл как есть по короткой ссылке на API, с Range
    let stream = app.get(&format!("/api/tracks/{kino}/stream"), &auth).await;
    assert_eq!(stream.status, StatusCode::OK, "{}", stream.text);
    assert_eq!(stream.body["codec"], "flac");
    let url = local(stream.body["url"].as_str().unwrap());
    let size = std::fs::metadata(root.join("Кино/Группа крови/01 - Группа крови.flac"))
        .unwrap()
        .len();
    let whole = app.get(&url, &Auth::None).await;
    assert_eq!(whole.status, StatusCode::OK);
    assert_eq!(whole.bytes.len() as u64, size);
    assert_eq!(whole.headers[header::CONTENT_TYPE], "audio/flac");
    let part = app
        .send(
            Request::builder()
                .uri(&url)
                .header(header::RANGE, "bytes=4-99")
                .body(Body::empty())
                .unwrap(),
        )
        .await;
    assert_eq!(part.status, StatusCode::PARTIAL_CONTENT);
    assert_eq!(part.bytes, whole.bytes[4..100].to_vec());
    assert_eq!(
        part.headers[header::CONTENT_RANGE],
        format!("bytes 4-99/{size}").as_str()
    );
    // Подпись не подделать: другой путь с той же подписью — 403
    let forged = url.replace("01%20-%20", "02%20-%20");
    assert_eq!(
        app.get(&forged, &Auth::None).await.status,
        StatusCode::FORBIDDEN
    );

    // AIFF клиенты не играют: копия FLAC по запросу
    let aiff = track_by_title(&db, "AIFF трек").await;
    let first = app.get(&format!("/api/tracks/{aiff}/stream"), &auth).await;
    assert_eq!(first.status, StatusCode::ACCEPTED, "{}", first.text);
    assert_eq!(first.body["status"], "preparing");
    let (kind, _) = app.run_job("transcode").await.expect("transcode job");
    assert_eq!(kind, "transcode");
    let ready = app.get(&format!("/api/tracks/{aiff}/stream"), &auth).await;
    assert_eq!(ready.status, StatusCode::OK, "{}", ready.text);
    assert_eq!(ready.body["codec"], "flac");
    assert_eq!(ready.body["lossless"], true);
    let copy = app
        .get(&local(ready.body["url"].as_str().unwrap()), &Auth::None)
        .await;
    assert_eq!(copy.status, StatusCode::OK);
    assert!(copy.bytes.starts_with(b"fLaC"));

    // Громкость и огибающая — прямо с диска, затем анализ звука
    for _ in 0..5 {
        app.run_job("local_analyze").await;
    }
    let analyzed: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM audio_files WHERE source = 'local' AND waveform IS NOT NULL",
    )
    .fetch_one(&db)
    .await
    .unwrap();
    assert_eq!(analyzed, 5);
    let jobs: i64 = sqlx::query_scalar("SELECT count(*) FROM jobs WHERE kind = 'analyze_full'")
        .fetch_one(&db)
        .await
        .unwrap();
    assert_eq!(jobs, 5, "каждая запись уходит на анализ звука");
    // Без загрузчика скачиваний нет вовсе
    let acquisitions: i64 = sqlx::query_scalar("SELECT count(*) FROM acquisitions")
        .fetch_one(&db)
        .await
        .unwrap();
    assert_eq!(acquisitions, 0);

    // Лайк переживает переезд файла
    let liked = app.put(&format!("/api/library/tracks/{kino}"), &auth).await;
    assert!(liked.status.is_success(), "{}", liked.text);
    let old = root.join("Кино/Группа крови/01 - Группа крови.flac");
    let new = root.join("Кино/1988 — Группа крови/01 Группа крови.flac");
    std::fs::create_dir_all(new.parent().unwrap()).unwrap();
    std::fs::rename(&old, &new).unwrap();
    let status = scan(
        &app,
        vec![
            old.to_string_lossy().to_string(),
            new.parent().unwrap().to_string_lossy().to_string(),
        ],
    )
    .await;
    assert_eq!(status["scan"]["kind"], "changes");
    assert_eq!(status["scan"]["moved"], 1, "{status}");
    assert_eq!(status["scan"]["removed"], 0, "{status}");
    assert_eq!(track_by_title(&db, "Группа крови").await, kino);
    let likes = app.get("/api/library/tracks", &auth).await;
    assert!(likes.text.contains(&kino), "{}", likes.text);
    let stream = app.get(&format!("/api/tracks/{kino}/stream"), &auth).await;
    assert_eq!(stream.status, StatusCode::OK);
    let moved = app
        .get(&local(stream.body["url"].as_str().unwrap()), &Auth::None)
        .await;
    assert_eq!(moved.status, StatusCode::OK);

    // Новые теги на месте: тот же трек, новое название
    let mp3 = root.join("Кино/Группа крови/02 - Закрой за мной дверь.mp3");
    let door = track_by_title(&db, "Закрой за мной дверь").await;
    tone(
        &mp3,
        4,
        550,
        &["-c:a", "libmp3lame", "-b:a", "192k"],
        &[
            ("title", "Закрой за мной дверь (я ухожу)"),
            ("artist", "Кино"),
            ("album", "Группа крови"),
            ("track", "2/2"),
        ],
    );
    let status = scan(&app, Vec::new()).await;
    assert_eq!(status["scan"]["updated"], 1, "{status}");
    assert_eq!(
        track_by_title(&db, "Закрой за мной дверь (я ухожу)").await,
        door
    );

    // Удалённый файл уходит из каталога; пустой альбом и артист — следом
    std::fs::remove_file(root.join("Folder Artist/Folder Album/04 Untagged.wav")).unwrap();
    let status = scan(&app, Vec::new()).await;
    assert_eq!(status["scan"]["removed"], 1, "{status}");
    assert_eq!(status["tracks"], 4);
    let left: i64 = sqlx::query_scalar("SELECT count(*) FROM artists WHERE name = 'Folder Artist'")
        .fetch_one(&db)
        .await
        .unwrap();
    assert_eq!(left, 0);
    let gone: i64 = sqlx::query_scalar("SELECT count(*) FROM tracks WHERE title = 'Untagged'")
        .fetch_one(&db)
        .await
        .unwrap();
    assert_eq!(gone, 0);
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn disk_storage_serves_and_accepts_signed_links(db: PgPool) {
    let dir = tempfile::tempdir().unwrap();
    let store: PathBuf = dir.path().join("data");
    let s = store.clone();
    let app = TestApp::with(db, move |c| {
        c.storage.kind = StorageKind::Fs;
        c.storage.path = s;
    })
    .await;
    let storage = &app.state.storage;
    assert!(storage.ensure_bucket().await.unwrap());

    // Загрузка по подписанной ссылке (так загрузчик кладёт файлы, когда хранилище — диск)
    let key = "audio/r/f.m4a";
    let put = storage.presign_put(key, UPLOAD_PUT_TTL).await.unwrap();
    let put = local(put.as_str());
    let body = vec![9u8; 70_000];
    let res = app
        .send(
            Request::builder()
                .method(Method::PUT)
                .uri(&put)
                .header(header::CONTENT_TYPE, "audio/mp4")
                .body(Body::from(body.clone()))
                .unwrap(),
        )
        .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    assert_eq!(std::fs::read(store.join(key)).unwrap(), body);
    // Ссылка на чтение не годится для записи
    let get = local(
        storage
            .presign_get(key, AUDIO_GET_TTL)
            .await
            .unwrap()
            .as_str(),
    );
    let res = app
        .send(
            Request::builder()
                .method(Method::PUT)
                .uri(&get)
                .header(header::CONTENT_TYPE, "audio/mp4")
                .body(Body::from("x"))
                .unwrap(),
        )
        .await;
    assert_eq!(res.status, StatusCode::FORBIDDEN);

    // Сервис анализа просит gzip: звук отдаётся как есть, иначе Content-Length не сойдётся с телом
    let res = app
        .send(
            Request::builder()
                .uri(&get)
                .header(header::ACCEPT_ENCODING, "gzip, deflate")
                .body(Body::empty())
                .unwrap(),
        )
        .await;
    assert_eq!(res.status, StatusCode::OK);
    assert!(res.headers.get(header::CONTENT_ENCODING).is_none());
    assert_eq!(res.bytes, body);
    assert_eq!(res.headers[header::ACCEPT_RANGES], "bytes");
    let res = app
        .send(
            Request::builder()
                .uri(&get)
                .header(header::RANGE, "bytes=-10")
                .body(Body::empty())
                .unwrap(),
        )
        .await;
    assert_eq!(res.status, StatusCode::PARTIAL_CONTENT);
    assert_eq!(res.bytes.len(), 10);
    let res = app
        .send(
            Request::builder()
                .uri(&get)
                .header(header::RANGE, "bytes=80000-")
                .body(Body::empty())
                .unwrap(),
        )
        .await;
    assert_eq!(res.status, StatusCode::RANGE_NOT_SATISFIABLE);

    // Скачивание под своим именем
    let dl = storage
        .presign_download(key, AUDIO_GET_TTL, "Кино — Группа крови.m4a")
        .await
        .unwrap();
    let res = app.get(&local(dl.as_str()), &Auth::None).await;
    assert_eq!(res.status, StatusCode::OK);
    assert!(
        res.headers[header::CONTENT_DISPOSITION]
            .to_str()
            .unwrap()
            .starts_with("attachment;")
    );

    // Картинки и удаление — как у S3
    storage
        .put(
            "images/albums/x/250.jpg",
            vec![1u8, 2, 3],
            "image/jpeg",
            None,
        )
        .await
        .unwrap();
    assert_eq!(
        storage
            .head("images/albums/x/250.jpg")
            .await
            .unwrap()
            .unwrap()
            .size,
        3
    );
    storage.delete(key).await.unwrap();
    assert!(!store.join(key).exists());
    assert_eq!(
        app.get(&get, &Auth::None).await.status,
        StatusCode::NOT_FOUND
    );
    // Без подписи — нельзя
    assert_eq!(
        app.get(
            "/api/files/images/albums/x/250.jpg?e=99999999999&s=AAAA",
            &Auth::None
        )
        .await
        .status,
        StatusCode::FORBIDDEN
    );
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn features_switch_off_downloading(db: PgPool) {
    let on = TestApp::new(db.clone()).await;
    let cfg = on.get("/api/config", &Auth::None).await;
    assert_eq!(cfg.body["deezer"], true);
    assert_eq!(cfg.body["downloader"], true);
    assert_eq!(cfg.body["library"], false);
    assert_eq!(cfg.body["storage"], "memory");
    drop(on);

    // Трек каталога без файла: без загрузчика его не скачать — 409, скачиваний не появляется
    let app = TestApp::with(db.clone(), |c| c.features.downloader = false).await;
    let auth = app.listener("no-downloader").await;
    let track = crate::fetcher::ep_tracks(&app, &auth).await[0];
    let res = app.get(&format!("/api/tracks/{track}/stream"), &auth).await;
    assert_eq!(res.status, StatusCode::CONFLICT, "{}", res.text);
    assert_eq!(res.code(), "audio_unavailable");
    let acquisitions: i64 = sqlx::query_scalar("SELECT count(*) FROM acquisitions")
        .fetch_one(&db)
        .await
        .unwrap();
    assert_eq!(acquisitions, 0);
}
