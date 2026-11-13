//! Поиск независимо от записи: как слышится, не в той раскладке, с опечаткой.

use axum::http::StatusCode;
use dalywave_textmatch as tm;
use serde_json::Value;
use sqlx::PgPool;

use crate::{
    common::{Auth, TestApp},
    deezer::MODERN_SADNESS,
};

/// Имена, на которых SQL и Rust легко разойтись: кириллица всех видов, диакритика, лигатуры, знаки.
const TRICKY: &[&str] = &[
    "Capsize",
    "капсайз",
    "Скриптонит",
    "Oxxxymiron",
    "MORGENSHTERN",
    "Моргенштерн",
    "Jah Khalib",
    "Джах Халиб",
    "Beyoncé",
    "Sigur Rós",
    "Mötley Crüe",
    "Straße",
    "Ørsted",
    "Łona",
    "Æther",
    "Œuvre",
    "Ðorđe",
    "Jesse®",
    "агентство насилия™",
    "BȺRBIE",
    "AC/DC",
    "50 Cent",
    "Їжак",
    "Ґанок",
    "Єва",
    "Ўладзімір",
    "Йоу",
    "Ёлка",
    "Pyrokinesis",
    "Пирокинезис",
    "Mzlff",
    "Queen",
    "The Weeknd",
    "Imagine Dragons",
    "Schwarz",
    "Tchaikovsky",
    "Rushex",
    "Рашекс",
    "XXXTENTACION",
    "Ghostemane",
    "Phonk",
    "Bôa",
    "ﬁnal",
    "宇多田ヒカル",
    "BTS 방탄소년단",
    "Sido & Bushido",
    "Мот feat. Бьянка",
    "  пробелы   и\tтабы ",
];

/// Ключи в базе (колонки и индексы) и в Rust (приложения, оценка выдачи) должны совпадать.
#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn sql_keys_match_rust(db: PgPool) {
    let words = |s: &str| {
        s.split(|c: char| !c.is_alphanumeric())
            .filter(|w| !w.is_empty())
            .collect::<Vec<_>>()
            .join(" ")
    };
    for name in TRICKY {
        let (latin, sound): (String, String) =
            sqlx::query_as("SELECT search_latin($1), sound_key($1)")
                .bind(name)
                .fetch_one(&db)
                .await
                .unwrap();
        assert_eq!(sound, tm::sound_key(name), "{name}");
        assert_eq!(words(&latin), words(&tm::latin(name)), "{name}");
    }
}

fn search_uri(q: &str, extra: &str) -> String {
    let q: String = url::form_urlencoded::byte_serialize(q.as_bytes()).collect();
    format!("/api/search?q={q}{extra}")
}

async fn search(app: &TestApp, auth: &Auth, q: &str, extra: &str) -> Value {
    let res = app.get(&search_uri(q, extra), auth).await;
    assert_eq!(res.status, StatusCode::OK, "{q}: {}", res.text);
    res.body
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn russian_spelling_finds_the_artist_in_deezer(db: PgPool) {
    let app = TestApp::new(db).await;
    let (_, admin) = app.admin("boss").await;

    // В Deezer «капсайз» не находится: сервер спрашивает «английское» написание — capsiz
    let body = search(&app, &admin, "капсайз", "").await;
    let artists = body["artists"].as_array().unwrap();
    assert_eq!(artists[0]["name"], "Capsize");
    assert_eq!(artists[0]["fans"], 3862, "из двух Capsize — популярный");
    assert_eq!(body["top"]["kind"], "artist");
    assert_eq!(body["top"]["id"], artists[0]["id"]);
    assert!(body.get("suggestion").is_none());
    let tracks = body["tracks"].as_array().unwrap();
    assert_eq!(tracks[0]["title"], "Capsize");
    // Чужие похожие — ниже
    let names: Vec<&str> = artists
        .iter()
        .map(|a| a["name"].as_str().unwrap())
        .collect();
    assert!(
        names.iter().position(|n| *n == "Cardi B").unwrap() > 1,
        "{names:?}"
    );
    // Запрос и один вариант, по каждому виду
    for kind in ["artist", "track", "album"] {
        assert_eq!(app.deezer.hits(&format!("/search/{kind}")), 2, "{kind}");
    }

    // Повтор — из памяти, а известный теперь Capsize ищется по имени (Deezer ничего не знает)
    let again = search(&app, &admin, "капсайз", "").await;
    assert_eq!(again["top"]["id"], body["top"]["id"]);
    assert_eq!(app.deezer.hits("/search/artist"), 3);

    // Не та раскладка: сервер уже знает Capsize
    let layout = search(&app, &admin, "сфзышяу", "").await;
    assert_eq!(layout["top"]["id"], body["top"]["id"]);
    assert_eq!(layout["artists"][0]["name"], "Capsize");
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn exact_query_goes_to_deezer_once(db: PgPool) {
    let app = TestApp::new(db).await;
    let (_, admin) = app.admin("boss").await;
    let body = search(&app, &admin, "Lowborn", "").await;
    assert_eq!(body["top"]["kind"], "artist");
    assert_eq!(body["top"]["id"], body["artists"][0]["id"]);
    // Нашлось как есть — без вариантов, порядок Deezer
    assert_eq!(app.deezer.hits("/search/artist"), 1);
    assert_eq!(body["tracks"][0]["title"], "Doctor Doctor");
    // Повтор — из памяти
    search(&app, &admin, "Lowborn", "").await;
    assert_eq!(app.deezer.hits("/search/track"), 1);
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn library_search_by_sound_layout_and_typos(db: PgPool) {
    let app = TestApp::new(db).await;
    let (_, admin) = app.admin("boss").await;
    let ep = app.resolve_id("album", MODERN_SADNESS, &admin).await;
    let album = app.get(&format!("/api/albums/{ep}"), &admin).await;
    let devil = album.body["tracks"][3]["id"].as_str().unwrap().to_string();
    assert_eq!(
        app.put(&format!("/api/library/tracks/{devil}"), &admin)
            .await
            .status,
        StatusCode::NO_CONTENT
    );
    let lib = "&scope=library";

    for q in ["лоуборн", "дщцищкт", "Lowborn"] {
        let body = search(&app, &admin, q, lib).await;
        assert_eq!(body["artists"][0]["name"], "Lowborn", "{q}");
        assert_eq!(body["top"]["id"], body["artists"][0]["id"], "{q}");
        assert_eq!(body["tracks"][0]["id"], devil, "{q}");
    }
    // Русской записью и названия: «девил лоуборн»
    let body = search(&app, &admin, "девил лоуборн", "&scope=library&types=track").await;
    assert_eq!(body["tracks"][0]["id"], devil);
    // Опечатка: уверенности нет — подсказка, а не лучший результат
    let body = search(&app, &admin, "lowbron", lib).await;
    assert_eq!(body["artists"][0]["name"], "Lowborn");
    assert!(body.get("top").is_none());
    assert_eq!(body["suggestion"], "Lowborn");
    // Короткое и непохожее — пусто
    for q in ["qz", "кап", "metallica"] {
        let body = search(&app, &admin, q, lib).await;
        for kind in ["tracks", "albums", "artists"] {
            assert!(body[kind].as_array().unwrap().is_empty(), "{q}: {kind}");
        }
    }
    // В Deezer поиск по медиатеке не ходит
    assert_eq!(app.deezer.hits("/search/artist"), 0);
}
