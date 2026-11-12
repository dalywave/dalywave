//! Перенос из «Яндекс Музыки» и YouTube Music: заглушки API Яндекса и сервиса fetcher с записанными
//! ответами (tests/fixtures/yandex, tests/fixtures/ytmusic), сопоставление — с заглушкой Deezer.

use std::{
    io::{Cursor, Write},
    path::PathBuf,
    sync::{Arc, Mutex},
};

use axum::{
    Router,
    body::{Body, Bytes},
    extract::State,
    http::{HeaderMap, Method, Request, StatusCode, Uri, header},
    response::{IntoResponse, Response},
};
use dalywave_server::jobs::worker::Outcome;
use serde_json::{Value, json};
use sqlx::PgPool;

use crate::{
    common::{Auth, PUBLIC_URL, Resp, TestApp},
    deezer::fixture as deezer_fixture,
};

const TOKEN: &str = "y0_AgAAAAATestTokenNotRealAbcdefghijklmnop";

fn fixture(dir: &str, name: &str) -> Value {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(dir)
        .join(format!("{name}.json"));
    let text =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("fixture {dir}/{name}: {e}"));
    serde_json::from_str(&text).unwrap()
}

/// Что спросили у заглушки.
#[derive(Debug, Clone)]
struct Hit {
    path: String,
    auth: Option<String>,
    body: String,
}

/// Заглушка внешнего сервиса: ответ — функция от пути и тела.
#[derive(Clone)]
struct Stub {
    base: String,
    hits: Arc<Mutex<Vec<Hit>>>,
}

type Answer = Arc<dyn Fn(&str, &str, bool) -> (StatusCode, Value) + Send + Sync>;

impl Stub {
    async fn start(answer: Answer) -> Self {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let stub = Self {
            base: format!("http://{}", listener.local_addr().unwrap()),
            hits: Arc::default(),
        };
        let app = Router::new()
            .fallback(serve)
            .with_state((stub.clone(), answer));
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        stub
    }

    fn hits(&self) -> Vec<Hit> {
        self.hits.lock().unwrap().clone()
    }
}

async fn serve(
    State((stub, answer)): State<(Stub, Answer)>,
    uri: Uri,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let auth = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .map(String::from);
    let body = String::from_utf8_lossy(&body).into_owned();
    let path = uri.path().to_string();
    stub.hits.lock().unwrap().push(Hit {
        path: path.clone(),
        auth: auth.clone(),
        body: body.clone(),
    });
    let (status, json) = answer(&path, &body, auth.is_some());
    (status, axum::Json(json)).into_response()
}

/// «Яндекс Музыка» как из-за границы: без ключа — 451, с чужим ключом — 401.
fn yandex_answer() -> Answer {
    Arc::new(|path, body, authed| {
        if !authed {
            return (
                StatusCode::UNAVAILABLE_FOR_LEGAL_REASONS,
                json!({"error": {"name": "Unavailable For Legal Reasons", "message": ""}}),
            );
        }
        match path {
            "/users/daly/playlists/1001" | "/users/42/playlists/1001" => {
                (StatusCode::OK, fixture("yandex", "playlist_daly_1001"))
            }
            "/users/42/likes/tracks" => (StatusCode::OK, fixture("yandex", "likes_42")),
            "/users/42/playlists/list" => (StatusCode::OK, fixture("yandex", "playlists_42")),
            "/account/status" => (StatusCode::OK, fixture("yandex", "account_status")),
            "/tracks" => {
                // Только спрошенные треки, в порядке запроса
                let ids: Vec<String> = url::form_urlencoded::parse(body.as_bytes())
                    .find(|(k, _)| k == "track-ids")
                    .map(|(_, v)| {
                        v.split(',')
                            .map(|id| id.split(':').next().unwrap_or("").to_string())
                            .collect()
                    })
                    .unwrap_or_default();
                let all = fixture("yandex", "tracks");
                let found: Vec<Value> = ids
                    .iter()
                    .filter_map(|id| {
                        all["result"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .find(|t| t["id"] == json!(id))
                            .cloned()
                    })
                    .collect();
                (StatusCode::OK, json!({ "result": found }))
            }
            _ => (
                StatusCode::NOT_FOUND,
                json!({"error": {"name": "playlist-not-found"}}),
            ),
        }
    })
}

/// Сервис fetcher: плейлист и имена видео по id.
fn fetcher_answer() -> Answer {
    Arc::new(|path, body, _| match path {
        "/v1/ytmusic/playlist" => {
            let req: Value = serde_json::from_str(body).unwrap();
            if req["playlistId"] == "PLntVvZRJe0d-6aEQPLPW4C3VjzjP2XbG8" {
                (StatusCode::OK, fixture("ytmusic", "playlist"))
            } else {
                (
                    StatusCode::NOT_FOUND,
                    json!({"error": {"code": "playlist_not_found"}}),
                )
            }
        }
        "/v1/ytmusic/songs" => {
            let req: Value = serde_json::from_str(body).unwrap();
            let all = fixture("ytmusic", "songs");
            let songs: Vec<Value> = req["videoIds"]
                .as_array()
                .unwrap()
                .iter()
                .map(|id| {
                    all["songs"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .find(|s| &s["videoId"] == id)
                        .cloned()
                        .unwrap_or_else(|| json!({"videoId": id, "found": false}))
                })
                .collect();
            (StatusCode::OK, json!({ "songs": songs }))
        }
        _ => (StatusCode::NOT_FOUND, json!({})),
    })
}

fn deezer_knows_lowborn(app: &TestApp) {
    app.deezer
        .set("/search/track", deezer_fixture("search_track_lowborn"));
}

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

async fn playlist_titles(app: &TestApp, auth: &Auth, title: &str) -> Vec<String> {
    let lists = app.get("/api/playlists", auth).await;
    let found = lists.body["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["title"] == title)
        .unwrap_or_else(|| panic!("playlist «{title}»: {}", lists.text))
        .clone();
    let detail = app
        .get(
            &format!("/api/playlists/{}", found["id"].as_str().unwrap()),
            auth,
        )
        .await;
    detail.body["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["track"]["title"].as_str().unwrap().to_string())
        .collect()
}

async fn liked_titles(app: &TestApp, auth: &Auth) -> Vec<String> {
    let liked = app.get("/api/library/tracks", auth).await;
    let mut titles: Vec<String> = liked.body["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["track"]["title"].as_str().unwrap().to_string())
        .collect();
    titles.sort();
    titles
}

/// Ключ входа не должен остаться нигде в базе.
async fn token_is_nowhere(app: &TestApp) {
    let leaked: i64 = sqlx::query_scalar(
        "SELECT (SELECT count(*) FROM imports WHERE imports::text LIKE '%' || $1 || '%')
              + (SELECT count(*) FROM import_items WHERE import_items::text LIKE '%' || $1 || '%')
              + (SELECT count(*) FROM jobs WHERE jobs::text LIKE '%' || $1 || '%')",
    )
    .bind(TOKEN)
    .fetch_one(&app.state.db)
    .await
    .unwrap();
    assert_eq!(leaked, 0, "the Yandex token is stored somewhere");
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn yandex_playlist_and_account(db: PgPool) {
    let yandex = Stub::start(yandex_answer()).await;
    let base = yandex.base.clone();
    let app = TestApp::with(db, |c| c.yandex_music.url = base).await;
    deezer_knows_lowborn(&app);
    let me = app.listener("ann").await;
    let link = "https://music.yandex.ru/users/daly/playlists/1001?utm_source=desktop";

    // Без ключа Яндекс не отвечает серверу из Германии — интерфейс просит ключ
    let res = app
        .post("/api/imports/yandex", json!({ "url": link }), &me)
        .await;
    assert_eq!(res.status, StatusCode::UNPROCESSABLE_ENTITY, "{}", res.text);
    assert_eq!(res.body["error"]["code"], "yandex_login_required");

    // Не ссылка Яндекса и не ключ
    let res = app
        .post(
            "/api/imports/yandex",
            json!({ "url": "https://example.com/users/daly/playlists/1001" }),
            &me,
        )
        .await;
    assert_eq!(res.body["error"]["code"], "yandex_link", "{}", res.text);
    let res = app
        .post(
            "/api/imports/yandex",
            json!({ "url": link, "token": "not a token" }),
            &me,
        )
        .await;
    assert_eq!(res.body["error"]["code"], "yandex_token", "{}", res.text);

    // С ключом: плейлист по порядку, подкаст пропущен, недостающий трек дозапрошен
    let res = app
        .post(
            "/api/imports/yandex",
            json!({ "url": link, "token": format!("https://music.yandex.ru/#access_token={TOKEN}&token_type=bearer") }),
            &me,
        )
        .await;
    assert_eq!(res.status, StatusCode::ACCEPTED, "{}", res.text);
    assert_eq!(res.body["kind"], "yandex");
    assert_eq!(res.body["title"], "Ночь");
    assert_eq!(res.body["total"], 3);
    let id = res.body["id"].as_str().unwrap().to_string();
    let hits = yandex.hits();
    assert!(
        hits.iter()
            .filter(|h| h.auth.is_some())
            .all(|h| h.auth.as_deref() == Some(&*format!("OAuth {TOKEN}"))),
        "{hits:?}"
    );
    let tracks_call = hits.iter().find(|h| h.path == "/tracks").expect("/tracks");
    assert!(
        tracks_call.body.contains("track-ids=103%3A1003"),
        "{tracks_call:?}"
    );

    assert_eq!(drain(&app).await, ["import_match", "import_apply"]);
    let s = summary(&app, &id, &me).await;
    assert_eq!(s["status"], "done", "{s}");
    assert_eq!(s["matched"], 3);
    assert_eq!(s["stats"]["playlists"], 1);
    assert!(s["playlistId"].is_string(), "{s}");
    assert_eq!(
        playlist_titles(&app, &me, "Ночь").await,
        ["Hurt", "Devil", "Doctor Doctor"]
    );

    // Ключ без ссылки — весь аккаунт: «Мне нравится» и плейлисты; «Ночь» дополняется, а не дублируется
    let res = app
        .post("/api/imports/yandex", json!({ "token": TOKEN }), &me)
        .await;
    assert_eq!(res.status, StatusCode::ACCEPTED, "{}", res.text);
    assert_eq!(res.body["title"], "Яндекс Музыка");
    // Devil (лайк и плейлист), неизвестная песня (лайк), Hurt, Doctor Doctor
    assert_eq!(res.body["total"], 4);
    let second = res.body["id"].as_str().unwrap().to_string();
    drain(&app).await;
    let s = summary(&app, &second, &me).await;
    assert_eq!(s["status"], "done", "{s}");
    assert_eq!(s["unmatched"], 1);
    assert_eq!(s["stats"]["likes"], 1);
    assert_eq!(s["stats"]["playlists"], 0);
    assert_eq!(liked_titles(&app, &me).await, ["Devil"]);
    let lists = app.get("/api/playlists", &me).await;
    let nights = lists.body["items"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|p| p["title"] == "Ночь")
        .count();
    assert_eq!(nights, 1, "{}", lists.text);
    // Пустой плейлист аккаунта не запрашивался
    assert!(
        !yandex
            .hits()
            .iter()
            .any(|h| h.path == "/users/42/playlists/1002")
    );
    let review = app
        .get(
            &format!("/api/imports/{second}/items?status=unmatched"),
            &me,
        )
        .await;
    assert_eq!(review.body["items"][0]["title"], "Совсем неизвестная песня");
    assert_eq!(review.body["items"][0]["liked"], true);

    token_is_nowhere(&app).await;
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn yandex_refuses_a_bad_token(db: PgPool) {
    let yandex = Stub::start(Arc::new(|_, _, _| {
        (
            StatusCode::UNAUTHORIZED,
            json!({"error": {"name": "session-expired"}}),
        )
    }))
    .await;
    let base = yandex.base.clone();
    let app = TestApp::with(db, |c| c.yandex_music.url = base).await;
    let me = app.listener("bob").await;
    let res = app
        .post("/api/imports/yandex", json!({ "token": TOKEN }), &me)
        .await;
    assert_eq!(res.status, StatusCode::UNPROCESSABLE_ENTITY, "{}", res.text);
    assert_eq!(res.body["error"]["code"], "yandex_token_rejected");
    // Ни ссылки, ни ключа
    let res = app.post("/api/imports/yandex", json!({}), &me).await;
    assert_eq!(res.status, StatusCode::BAD_REQUEST, "{}", res.text);
    // Без входа — нет
    let res = app
        .post(
            "/api/imports/yandex",
            json!({ "token": TOKEN }),
            &Auth::None,
        )
        .await;
    assert_eq!(res.status, StatusCode::UNAUTHORIZED, "{}", res.text);
    token_is_nowhere(&app).await;
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn youtube_playlist_by_link(db: PgPool) {
    let fetcher = Stub::start(fetcher_answer()).await;
    let base = fetcher.base.clone();
    let app = TestApp::with(db, |c| c.fetcher.url = base).await;
    deezer_knows_lowborn(&app);
    let me = app.listener("cid").await;

    let res = app
        .post(
            "/api/imports/youtube",
            json!({ "url": "https://www.youtube.com/watch?v=QoKpz_95OIM" }),
            &me,
        )
        .await;
    assert_eq!(res.body["error"]["code"], "youtube_link", "{}", res.text);
    let res = app
        .post(
            "/api/imports/youtube",
            json!({ "url": "https://music.youtube.com/playlist?list=PLclosedclosedclosed" }),
            &me,
        )
        .await;
    assert_eq!(
        res.body["error"]["code"], "youtube_not_found",
        "{}",
        res.text
    );

    let res = app
        .post(
            "/api/imports/youtube",
            json!({ "url": "https://music.youtube.com/playlist?list=PLntVvZRJe0d-6aEQPLPW4C3VjzjP2XbG8&si=x" }),
            &me,
        )
        .await;
    assert_eq!(res.status, StatusCode::ACCEPTED, "{}", res.text);
    assert_eq!(res.body["kind"], "youtube_playlist");
    assert_eq!(res.body["title"], "Ночная дорога");
    assert_eq!(res.body["total"], 3);
    let id = res.body["id"].as_str().unwrap().to_string();
    drain(&app).await;
    let s = summary(&app, &id, &me).await;
    assert_eq!(s["status"], "done", "{s}");
    assert_eq!(s["matched"], 2);
    assert_eq!(s["unmatched"], 1);
    // Клип «Lowborn - Devil (Official Video) | Премьера 2023» с канала VEVO нашёлся как Devil
    assert_eq!(
        playlist_titles(&app, &me, "Ночная дорога").await,
        ["Hurt", "Devil"]
    );
    // Тот же плейлист ещё раз — дополняет прежний, кеш по id видео без новых поисков
    let searches = app.deezer.hits("/search/track");
    let res = app
        .post(
            "/api/imports/youtube",
            json!({ "url": "PLntVvZRJe0d-6aEQPLPW4C3VjzjP2XbG8" }),
            &me,
        )
        .await;
    assert_eq!(res.status, StatusCode::ACCEPTED, "{}", res.text);
    drain(&app).await;
    assert_eq!(app.deezer.hits("/search/track"), searches);
    let lists = app.get("/api/playlists", &me).await;
    assert_eq!(
        lists.body["items"].as_array().unwrap().len(),
        1,
        "{}",
        lists.text
    );
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn youtube_playlist_without_fetcher(db: PgPool) {
    let app = TestApp::with(db, |c| c.fetcher.url = "http://127.0.0.1:9".into()).await;
    let me = app.listener("dan").await;
    let res = app
        .post(
            "/api/imports/youtube",
            json!({ "url": "https://music.youtube.com/playlist?list=PLntVvZRJe0d-6aEQPLPW4C3VjzjP2XbG8" }),
            &me,
        )
        .await;
    assert_eq!(res.status, StatusCode::SERVICE_UNAVAILABLE, "{}", res.text);
}

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

async fn upload(app: &TestApp, uri: &str, auth: &Auth, body: Vec<u8>) -> Resp {
    let mut req = Request::builder()
        .method(Method::POST)
        .uri(uri)
        .header(header::CONTENT_TYPE, "application/zip")
        .header(header::CONTENT_LENGTH, body.len());
    if let Auth::Cookie(c) = auth {
        req = req
            .header(header::COOKIE, c)
            .header(header::ORIGIN, PUBLIC_URL);
    }
    app.send(req.body(Body::from(body)).unwrap()).await
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn youtube_takeout_archive(db: PgPool) {
    let fetcher = Stub::start(fetcher_answer()).await;
    let base = fetcher.base.clone();
    let app = TestApp::with(db, |c| c.fetcher.url = base).await;
    deezer_knows_lowborn(&app);
    let me = app.listener("eve").await;
    let archive = zip_of(&[
        (
            "Takeout/YouTube и YouTube Music/музыка (библиотека и загрузки)/music library songs.csv",
            "Video ID,Song Title,Album Title,Artist Name 1\nDevil000001,Devil,Devil,Lowborn\n",
        ),
        (
            "Takeout/YouTube и YouTube Music/плейлисты/Понравившиеся-видео.csv",
            "Video ID,Playlist Video Creation Timestamp\nCrazy000001,2024-01-01T00:00:00+00:00\njNQXAC9IVRw,2024-01-02T00:00:00+00:00\n",
        ),
        (
            "Takeout/YouTube и YouTube Music/плейлисты/Дорога-видео.csv",
            "Video ID,Playlist Video Creation Timestamp\nHurt0000001,2024-01-01T00:00:00+00:00\nGone0000001,2024-01-01T00:00:00+00:00\nDevil000001,2024-01-01T00:00:00+00:00\n",
        ),
        (
            "Takeout/YouTube и YouTube Music/плейлисты/Смотреть позже-видео.csv",
            "Video ID,Playlist Video Creation Timestamp\nWatch000001,2024-01-01T00:00:00+00:00\n",
        ),
        ("Takeout/archive_browser.html", "<html></html>"),
    ]);
    let res = upload(
        &app,
        "/api/imports/youtube/takeout?name=takeout-20261112.zip",
        &me,
        archive,
    )
    .await;
    assert_eq!(res.status, StatusCode::ACCEPTED, "{}", res.text);
    assert_eq!(res.body["kind"], "youtube_takeout");
    assert_eq!(res.body["total"], 5);
    let id = res.body["id"].as_str().unwrap().to_string();
    assert_eq!(drain(&app).await, ["import_match", "import_apply"]);
    let s = summary(&app, &id, &me).await;
    assert_eq!(s["status"], "done", "{s}");
    // Влог из «Понравившихся» и удалённое видео пропущены, не просят проверки
    assert_eq!(s["matched"], 3, "{s}");
    assert_eq!(s["unmatched"], 0, "{s}");
    assert_eq!(s["processed"], 5);
    assert_eq!(liked_titles(&app, &me).await, ["Crazy", "Devil"]);
    assert_eq!(
        playlist_titles(&app, &me, "Дорога").await,
        ["Hurt", "Devil"]
    );
    // Имена узнаны пачкой у fetcher, без «Смотреть позже»
    let asked: Vec<String> = fetcher
        .hits()
        .iter()
        .filter(|h| h.path == "/v1/ytmusic/songs")
        .map(|h| h.body.clone())
        .collect();
    assert_eq!(asked.len(), 1, "{asked:?}");
    assert!(!asked[0].contains("Watch000001") && !asked[0].contains("Devil000001"));

    // Не архив Takeout
    let res = upload(
        &app,
        "/api/imports/youtube/takeout",
        &me,
        zip_of(&[("notes.txt", "hi")]),
    )
    .await;
    assert_eq!(res.status, StatusCode::BAD_REQUEST, "{}", res.text);
    assert!(
        res.body["error"]["message"]
            .as_str()
            .unwrap()
            .contains("Takeout")
    );
}
