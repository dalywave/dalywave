//! Сведения о музыке: чарты (Deezer и открытый чарт Apple Music), отрывок с припева, кто сделал трек
//! (MusicBrainz) и «Об артисте» (Wikidata, Википедия). Все чужие сервисы — одна заглушка на локальном
//! порту с записанными ответами из `tests/fixtures/{apple,musicbrainz,wiki}`.

use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::Duration,
};

use axum::{
    Router,
    extract::State,
    http::{HeaderMap, StatusCode, Uri, header},
    response::{IntoResponse, Response},
};
use serde_json::{Value, json};
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    common::{Auth, TestApp},
    deezer::{LIGHT_ME_UP, LOWBORN, fixture},
};

const FINNEAS: &str = "151cd917-1ee2-4702-859f-90899ad897f8";
const FINNEAS_DEEZER: i64 = 10797684;

fn file(dir: &str, name: &str) -> Value {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(dir)
        .join(format!("{name}.json"));
    serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap()
}

#[derive(Default)]
struct Inner {
    /// Путь, параметры, User-Agent
    calls: Vec<(String, HashMap<String, String>, String)>,
    /// Следующие ответы MusicBrainz — 503 «занят»
    busy: usize,
}

#[derive(Clone)]
struct FakeSources {
    base: String,
    inner: Arc<Mutex<Inner>>,
}

async fn serve(State(fake): State<FakeSources>, uri: Uri, headers: HeaderMap) -> Response {
    let path = uri.path().to_string();
    let q: HashMap<String, String> =
        url::form_urlencoded::parse(uri.query().unwrap_or("").as_bytes())
            .into_owned()
            .collect();
    let ua = headers
        .get(header::USER_AGENT)
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default()
        .to_string();
    let busy = {
        let mut inner = fake.inner.lock().unwrap();
        inner.calls.push((path.clone(), q.clone(), ua));
        let busy =
            inner.busy > 0 && (path.starts_with("/recording") || path.starts_with("/artist"));
        if busy {
            inner.busy -= 1;
        }
        busy
    };
    if busy {
        return (StatusCode::SERVICE_UNAVAILABLE, "busy").into_response();
    }
    let not_found = || {
        (
            StatusCode::NOT_FOUND,
            axum::Json(json!({ "error": "Not Found" })),
        )
            .into_response()
    };
    let body = if path.starts_with("/api/v2/ru/music/most-played/") {
        file("apple", "most_played_ru")
    } else if path == "/recording" {
        file("musicbrainz", "search_isrc_bad_guy")
    } else if path.starts_with("/recording/") {
        file("musicbrainz", "recording_bad_guy")
    } else if path == format!("/artist/{FINNEAS}") {
        file("musicbrainz", "artist_finneas")
    } else if path.starts_with("/artist/") {
        return not_found();
    } else if path == "/w/api.php" {
        match q.get("action").map(String::as_str) {
            Some("query")
                if q.get("srsearch").map(String::as_str)
                    == Some(&format!("haswbstatement:P2722={LOWBORN}")) =>
            {
                file("wiki", "search_deezer_billie")
            }
            Some("query") => {
                json!({ "batchcomplete": "", "query": { "searchinfo": { "totalhits": 0 }, "search": [] } })
            }
            Some("wbgetentities") if q.get("props").is_some_and(|p| p.contains("claims")) => {
                file("wiki", "entity_billie")
            }
            Some("wbgetentities") => file("wiki", "labels_billie"),
            _ => return not_found(),
        }
    } else if path.starts_with("/ru/api/rest_v1/page/summary/") {
        file("wiki", "summary_billie_ru")
    } else {
        return not_found();
    };
    axum::Json(body).into_response()
}

impl FakeSources {
    async fn start() -> Self {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let fake = Self {
            base: format!("http://{}", listener.local_addr().unwrap()),
            inner: Arc::new(Mutex::new(Inner::default())),
        };
        let app = Router::new().fallback(serve).with_state(fake.clone());
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        fake
    }

    fn calls(&self, prefix: &str) -> Vec<(String, HashMap<String, String>, String)> {
        self.inner
            .lock()
            .unwrap()
            .calls
            .iter()
            .filter(|c| c.0.starts_with(prefix))
            .cloned()
            .collect()
    }

    fn busy(&self, n: usize) {
        self.inner.lock().unwrap().busy = n;
    }
}

async fn setup(db: PgPool) -> (TestApp, FakeSources, Auth) {
    let fake = FakeSources::start().await;
    let base = fake.base.clone();
    let app = TestApp::with(db, |c| {
        c.info.apple_charts_url = base.clone();
        c.info.musicbrainz_url = base.clone();
        c.info.wikidata_url = base.clone();
        c.info.wikipedia_url = format!("{base}/{{lang}}");
    })
    .await;
    let auth = app.listener("info-listener").await;
    (app, fake, auth)
}

/// Ждёт, пока фоновая работа не выставит `flag` в false.
async fn settle(app: &TestApp, uri: &str, flag: &str, auth: &Auth) -> Value {
    for _ in 0..100 {
        let res = app.get(uri, auth).await;
        assert_eq!(res.status, StatusCode::OK, "{}", res.text);
        if res.body[flag] == false {
            return res.body;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!("{uri}: {flag} не закончилось");
}

// ───────────── Чарты ─────────────

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn world_chart_is_gathered_once_and_numbered(db: PgPool) {
    let (app, _fake, auth) = setup(db.clone()).await;

    let res = app.get("/api/charts/world", &auth).await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    let titles: Vec<&str> = res.body["tracks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["title"].as_str().unwrap())
        .collect();
    let recorded = fixture("playlist_3155776842_tracks");
    let expected: Vec<&str> = recorded["data"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["title_short"].as_str().or(t["title"].as_str()).unwrap())
        .collect();
    assert_eq!(
        titles, expected,
        "места — в порядке плейлиста «Top Worldwide»"
    );
    assert_eq!(res.body["chart"]["kind"], "world");
    assert!(res.body["chart"]["updatedAt"].is_string());
    assert!(res.body["chart"]["image"]["medium"].is_string());
    assert_eq!(res.body["updating"], false);
    // Артисты — по местам их треков: у Ella Langley два трека (3-е и 5-е места), она выше первого
    // места с одним треком; без повторов
    let artists = res.body["artists"].as_array().unwrap();
    let names: Vec<&str> = artists
        .iter()
        .map(|a| a["name"].as_str().unwrap())
        .collect();
    assert_eq!(&names[..2], &["Ella Langley", "STELLA LEFTY"]);
    assert_eq!(names.len(), 5);

    // Второй раз — из базы, Deezer не спрашивается
    let hits = app.deezer.hits("/playlist/3155776842/tracks");
    assert_eq!(hits, 1);
    let again = app.get("/api/charts/world", &auth).await;
    assert_eq!(again.body["tracks"], res.body["tracks"]);
    assert_eq!(app.deezer.hits("/playlist/3155776842/tracks"), 1);

    // Устарел — показывается прежний, а свежий собирается в фоне
    sqlx::query("UPDATE charts SET fetched_at = now() - interval '5 hours' WHERE id = 'world'")
        .execute(&db)
        .await
        .unwrap();
    let stale = app.get("/api/charts/world", &auth).await;
    assert_eq!(stale.body["tracks"], res.body["tracks"]);
    settle(&app, "/api/charts/world", "updating", &auth).await;
    assert_eq!(app.deezer.hits("/playlist/3155776842/tracks"), 2);

    assert_eq!(
        app.get("/api/charts/nope", &auth).await.status,
        StatusCode::NOT_FOUND
    );
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn genre_chart_and_the_list(db: PgPool) {
    let (app, _fake, auth) = setup(db).await;
    let res = app.get("/api/charts/genre-116", &auth).await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    assert_eq!(res.body["tracks"].as_array().unwrap().len(), 5);
    assert_eq!(res.body["chart"]["title"], "Хип-хоп");
    assert_eq!(app.deezer.hits("/chart/116/tracks"), 1);

    let list = app.get("/api/charts", &auth).await;
    assert_eq!(list.status, StatusCode::OK, "{}", list.text);
    let items = list.body["items"].as_array().unwrap();
    assert_eq!(items[0]["id"], "ru");
    assert_eq!(items[1]["id"], "world");
    let rap = items.iter().find(|c| c["id"] == "genre-116").unwrap();
    assert!(rap["image"]["small"].is_string(), "обложка первого места");
    let rock = items.iter().find(|c| c["id"] == "genre-152").unwrap();
    assert!(rock["updatedAt"].is_null() && rock["image"].is_null());
    // Список жанры не собирает: только Россию и мир, их видно на главной поиска
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert_eq!(app.deezer.hits("/chart/152/tracks"), 0);
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn russia_comes_from_apple_and_is_matched_on_deezer(db: PgPool) {
    let (app, fake, auth) = setup(db.clone()).await;
    let lowborn = fixture("search_track_lowborn");
    app.deezer
        .set_search("track", "Lowborn Devil", lowborn.clone());
    app.deezer.set_search("track", "Lowborn Crazy", lowborn);

    let first = app.get("/api/charts/ru", &auth).await;
    assert_eq!(first.status, StatusCode::OK, "{}", first.text);
    assert_eq!(first.body["chart"]["kind"], "country");
    let body = settle(&app, "/api/charts/ru", "updating", &auth).await;
    let tracks: Vec<(&str, &str)> = body["tracks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| {
            (
                t["title"].as_str().unwrap(),
                t["artists"][0]["name"].as_str().unwrap(),
            )
        })
        .collect();
    assert_eq!(
        tracks,
        vec![("Devil", "Lowborn"), ("Crazy", "Lowborn")],
        "третьей песни в Deezer нет — её место пропускается"
    );
    assert_eq!(fake.calls("/api/v2/ru/").len(), 1);
    let matched: Vec<(String, Option<Uuid>)> =
        sqlx::query_as("SELECT external_id, track_id FROM chart_matches ORDER BY external_id")
            .fetch_all(&db)
            .await
            .unwrap();
    assert_eq!(matched.len(), 3);
    assert!(matched[2].1.is_none(), "«не нашли» запомнено");

    // Следующий сбор не ищет уже найденное и недавно не найденное
    let searches = app.deezer.hits("/search/track");
    sqlx::query("UPDATE charts SET fetched_at = now() - interval '5 hours' WHERE id = 'ru'")
        .execute(&db)
        .await
        .unwrap();
    app.get("/api/charts/ru", &auth).await;
    let again = settle(&app, "/api/charts/ru", "updating", &auth).await;
    assert_eq!(again["tracks"], body["tracks"]);
    assert_eq!(fake.calls("/api/v2/ru/").len(), 2);
    assert_eq!(app.deezer.hits("/search/track"), searches);
}

// ───────────── Отрывок с припева ─────────────

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn hook_is_a_deezer_preview_until_the_track_is_on_the_server(db: PgPool) {
    let (app, _fake, auth) = setup(db.clone()).await;
    let id = app.resolve_id("track", LIGHT_ME_UP, &auth).await;

    let res = app.get(&format!("/api/tracks/{id}/hook"), &auth).await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    assert_eq!(res.body["ready"], false);
    assert_eq!(res.body["from"], "preview");
    assert_eq!(res.body["startMs"], 0);
    assert_eq!(res.body["lengthMs"], 15_000);
    assert!(res.body["previewUrl"].as_str().unwrap().contains(".mp3"));
    assert_eq!(res.headers[header::CACHE_CONTROL], "no-store");
    // Отрывок трек на скачивание не ставит
    let acquisitions: i64 = sqlx::query_scalar("SELECT count(*) FROM acquisitions")
        .fetch_one(&db)
        .await
        .unwrap();
    assert_eq!(acquisitions, 0);

    // Звук на сервере, анализа нет — 30 % трека (193 с), а не превью
    sqlx::query(
        "UPDATE recordings SET audio_status = 'ready'
         WHERE id = (SELECT recording_id FROM tracks WHERE id = $1)",
    )
    .bind(Uuid::parse_str(&id).unwrap())
    .execute(&db)
    .await
    .unwrap();
    let res = app.get(&format!("/api/tracks/{id}/hook"), &auth).await;
    assert_eq!(res.body["ready"], true, "{}", res.text);
    assert_eq!(res.body["from"], "estimate");
    assert_eq!(res.body["startMs"], 57_900);
    assert!(res.body["previewUrl"].is_null());

    let missing = app
        .get(&format!("/api/tracks/{}/hook", Uuid::nil()), &auth)
        .await;
    assert_eq!(missing.status, StatusCode::NOT_FOUND);
}

// ───────────── Кто сделал трек ─────────────

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn credits_come_from_musicbrainz_and_people_get_links(db: PgPool) {
    let (app, fake, auth) = setup(db.clone()).await;
    let id = app.resolve_id("track", LIGHT_ME_UP, &auth).await;
    // Длительность — как у записи MusicBrainz из заглушки
    sqlx::query("UPDATE tracks SET duration_ms = 194088 WHERE id = $1")
        .bind(Uuid::parse_str(&id).unwrap())
        .execute(&db)
        .await
        .unwrap();
    let mut finneas: Value = fixture("artist_13828465");
    finneas["id"] = json!(FINNEAS_DEEZER);
    finneas["name"] = json!("FINNEAS");
    app.deezer
        .set(&format!("/artist/{FINNEAS_DEEZER}"), finneas);

    let res = app.get(&format!("/api/tracks/{id}/credits"), &auth).await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    let names = |list: &Value| -> Vec<String> {
        list.as_array()
            .unwrap()
            .iter()
            .map(|p| {
                format!(
                    "{}:{}",
                    p["name"].as_str().unwrap(),
                    p["roles"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|r| r.as_str().unwrap())
                        .collect::<Vec<_>>()
                        .join("+")
                )
            })
            .collect()
    };
    assert_eq!(
        names(&res.body["writing"]),
        vec!["Billie Eilish:writer", "FINNEAS:writer"]
    );
    assert_eq!(
        names(&res.body["production"]),
        vec![
            "Billie Eilish:producer",
            "FINNEAS:producer",
            "Rob Kinelski:mix"
        ]
    );
    assert_eq!(res.body["year"], 2020, "год — из альбома каталога");
    let searches = fake.calls("/recording");
    let search = &searches[0];
    assert_eq!(search.1["query"], "isrc:GBWUL2035890");
    assert!(
        search.2.starts_with("DalyWave/") && search.2.contains("wave.daly.rest"),
        "User-Agent с контактом: {}",
        search.2
    );

    // Ссылки ищутся в фоне: у FINNEAS есть страница в Deezer, у остальных — нет
    let body = settle(
        &app,
        &format!("/api/tracks/{id}/credits"),
        "resolving",
        &auth,
    )
    .await;
    let finneas = body["writing"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["name"] == "FINNEAS")
        .unwrap();
    let artist = finneas["artistId"]
        .as_str()
        .expect("FINNEAS ведёт к артисту");
    let page = app.get(&format!("/api/artists/{artist}"), &auth).await;
    assert_eq!(page.body["artist"]["name"], "FINNEAS");
    let billie = body["writing"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["name"] == "Billie Eilish")
        .unwrap();
    assert!(
        billie["artistId"].is_null(),
        "без ссылки на Deezer — просто имя"
    );

    // Второй раз — из базы: MusicBrainz больше не спрашивается
    let before = fake.calls("/").len();
    let again = app.get(&format!("/api/tracks/{id}/credits"), &auth).await;
    assert_eq!(again.body, body);
    assert_eq!(fake.calls("/").len(), before);
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn busy_musicbrainz_still_shows_label_and_year(db: PgPool) {
    let (app, fake, auth) = setup(db.clone()).await;
    let id = app.resolve_id("track", LIGHT_ME_UP, &auth).await;
    fake.busy(10);
    let res = app.get(&format!("/api/tracks/{id}/credits"), &auth).await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    assert_eq!(res.body["writing"], json!([]));
    assert_eq!(res.body["year"], 2020);
    // Сбой не запоминается: в следующий раз MusicBrainz спросят снова
    let stored: i64 = sqlx::query_scalar("SELECT count(*) FROM recording_credits")
        .fetch_one(&db)
        .await
        .unwrap();
    assert_eq!(stored, 0);
}

// ───────────── Об артисте ─────────────

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn about_comes_from_wikipedia_by_the_deezer_id(db: PgPool) {
    let (app, fake, auth) = setup(db.clone()).await;
    let artist = app.resolve_id("artist", LOWBORN, &auth).await;

    let res = app
        .get(&format!("/api/artists/{artist}/about"), &auth)
        .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    let about = &res.body["about"];
    assert_eq!(about["lang"], "ru");
    assert!(
        about["summary"]
            .as_str()
            .unwrap()
            .starts_with("Билли Айлиш")
    );
    assert_eq!(about["active"], json!({ "from": 2015, "to": null }));
    assert_eq!(about["origin"], "Лос-Анджелес");
    assert_eq!(
        about["genres"],
        json!(["электропоп", "инди-поп", "альт-поп"])
    );
    assert_eq!(about["group"], false);
    assert!(
        about["url"]
            .as_str()
            .unwrap()
            .starts_with("https://ru.wikipedia.org/wiki/")
    );
    let searches = fake.calls("/w/api.php");
    assert_eq!(
        searches[0].1["srsearch"],
        format!("haswbstatement:P2722={LOWBORN}")
    );

    // Второй раз — из базы
    let before = fake.calls("/").len();
    let again = app
        .get(&format!("/api/artists/{artist}/about"), &auth)
        .await;
    assert_eq!(again.body, res.body);
    assert_eq!(fake.calls("/").len(), before);
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn no_exact_link_means_no_about_block(db: PgPool) {
    let (app, fake, auth) = setup(db.clone()).await;
    // Артист без id Deezer в Wikidata и без записи с его именем в MusicBrainz: по одному имени не ищем
    let id: Uuid = sqlx::query_scalar(
        "INSERT INTO artists (name, deezer_id) VALUES ('Однофамилец', 777) RETURNING id",
    )
    .fetch_one(&db)
    .await
    .unwrap();
    let res = app.get(&format!("/api/artists/{id}/about"), &auth).await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    assert!(res.body["about"].is_null());
    assert!(fake.calls("/ru/").is_empty(), "статью не открывали");
    let stored: Option<Value> =
        sqlx::query_scalar("SELECT about FROM artist_about WHERE artist_id = $1")
            .bind(id)
            .fetch_one(&db)
            .await
            .unwrap();
    assert!(stored.is_none(), "«статьи нет» запомнено");

    let missing = app
        .get(&format!("/api/artists/{}/about", Uuid::nil()), &auth)
        .await;
    assert_eq!(missing.status, StatusCode::NOT_FOUND);
}
