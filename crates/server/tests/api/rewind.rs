//! «Итоги года» против API: время, дни и серия, топы, импортированная история в своём году, открытия,
//! жанры по анализу, кеш с отпечатком, плейлист «Твои лучшие треки», чужие итоги и будущий год.

use axum::http::{Method, StatusCode};
use chrono::{DateTime, Datelike, TimeZone, Utc};
use serde_json::Value;
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    common::{Auth, TestApp},
    synth::{Feat, Synth},
};

async fn play(db: &PgPool, user: Uuid, track: Uuid, at: DateTime<Utc>, ms: i32, source: &str) {
    sqlx::query(
        "INSERT INTO play_events (client_event_id, user_id, track_id, started_at, played_ms, duration_ms, end_reason, source)
         VALUES ($1, $2, $3, $4, $5, 180000, $6, $7)",
    )
    .bind(Uuid::now_v7())
    .bind(user)
    .bind(track)
    .bind(at)
    .bind(ms)
    .bind(if ms >= 180_000 { "completed" } else { "skipped" })
    .bind(source)
    .execute(db)
    .await
    .unwrap();
}

fn at(y: i32, m: u32, d: u32, h: u32) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(y, m, d, h, 0, 0).unwrap()
}

async fn rewind(app: &TestApp, auth: &Auth, year: i32) -> Value {
    let res = app.get(&format!("/api/rewind/{year}"), auth).await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    res.body
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn rewind_sums_up_a_year(db: PgPool) {
    sqlx::query("UPDATE reco_settings SET utc_offset_min = 0")
        .execute(&db)
        .await
        .unwrap();
    let app = TestApp::new(db.clone()).await;
    let me = app.listener("ann").await;
    let user: Uuid = app.user_id(&me).await.parse().unwrap();
    let y = Utc::now().year() - 1;
    let s = Synth::new(db.clone());
    let old = s.artist("Molchat Doma").await;
    let fresh = s.artist("Rushex").await;
    let old_track = s.track(old, "Судно", Feat::default()).await;
    let hit = s.track(fresh, "Хит", Feat::default()).await;
    let other = s.track(fresh, "Второй", Feat::default()).await;

    // Позапрошлый год: Molchat Doma уже знакомы
    play(&db, user, old_track, at(y - 1, 6, 1, 12), 180_000, "stream").await;
    // Год: 1–3 января подряд, 10 января; ночью; импорт из Spotify считается в своём году
    play(&db, user, hit, at(y, 1, 1, 1), 180_000, "import").await;
    play(&db, user, hit, at(y, 1, 2, 2), 180_000, "import").await;
    play(&db, user, hit, at(y, 1, 3, 2), 180_000, "stream").await;
    play(&db, user, hit, at(y, 1, 3, 3), 180_000, "stream").await;
    play(&db, user, hit, at(y, 3, 10, 23), 180_000, "stream").await;
    play(&db, user, other, at(y, 3, 10, 22), 60_000, "stream").await;
    play(&db, user, old_track, at(y, 1, 10, 12), 180_000, "stream").await;
    // Пропуск через 5 секунд — время идёт, прослушиванием не считается
    play(&db, user, other, at(y, 1, 10, 13), 5_000, "stream").await;
    // Следующий год в итоги прошлого не попадает
    play(&db, user, old_track, at(y + 1, 1, 1, 0), 180_000, "stream").await;
    sqlx::query("INSERT INTO liked_tracks (user_id, track_id, created_at) VALUES ($1, $2, $3)")
        .bind(user)
        .bind(hit)
        .bind(at(y, 2, 1, 12))
        .execute(&db)
        .await
        .unwrap();

    let r = rewind(&app, &me, y).await;
    assert_eq!(r["year"], y);
    assert_eq!(r["complete"], true);
    assert_eq!(r["season"], true);
    assert_eq!(r["plays"], 7, "{r}");
    // 6 × 3 минуты + 1 минута + 5 секунд
    assert_eq!(r["minutes"], 19);
    assert_eq!(r["tracks"], 3);
    assert_eq!(r["artists"], 2);
    assert_eq!(r["newArtists"], 1);
    assert_eq!(r["days"], 5);
    assert_eq!(r["likes"], 1);
    assert_eq!(r["topTracks"][0]["track"]["title"], "Хит");
    assert_eq!(r["topTracks"][0]["plays"], 5);
    assert_eq!(r["topArtists"][0]["artist"]["name"], "Rushex");
    assert_eq!(r["topArtists"][0]["plays"], 6);
    assert_eq!(r["trackOfYear"]["track"]["title"], "Хит");
    assert_eq!(r["trackOfYear"]["peakMonth"], 1);
    assert_eq!(r["trackOfYear"]["firstPlayed"], format!("{y}-01-01"));
    assert_eq!(r["firstTrack"]["track"]["title"], "Хит");
    assert_eq!(r["streak"]["days"], 3);
    assert_eq!(r["streak"]["from"], format!("{y}-01-01"));
    assert_eq!(r["streak"]["to"], format!("{y}-01-03"));
    assert_eq!(r["topDay"]["date"], format!("{y}-01-03"));
    assert_eq!(r["topDay"]["track"]["title"], "Хит");
    assert_eq!(r["topDay"]["trackPlays"], 2);
    let months = r["months"].as_array().unwrap();
    assert_eq!(months.len(), 12);
    assert_eq!(months[0]["minutes"], 15);
    assert_eq!(months[2]["minutes"], 4);
    // Rushex впервые в этом году и лайкнут — открытие; Molchat Doma — нет
    let disc = r["discoveries"].as_array().unwrap();
    assert_eq!(disc.len(), 1, "{r}");
    assert_eq!(disc[0]["artist"]["name"], "Rushex");
    assert_eq!(disc[0]["firstHeard"], format!("{y}-01-01"));
    assert_eq!(disc[0]["likes"], 1);
    // Жанр — стиль Discogs из анализа, по-русски
    assert_eq!(r["genres"][0]["name"], "Трэп");
    assert!((r["genres"][0]["share"].as_f64().unwrap() - 1.0).abs() < 1e-3);
    assert_eq!(r["moods"].as_array().unwrap().len(), 5);
    assert_eq!(r["moods"][0]["id"], "party");
    // 6 из 7 прослушиваний ночью — «ночной слушатель»
    assert_eq!(r["personality"]["id"], "night_owl", "{r}");
    assert_eq!(r["personality"]["peakPart"], "night");
    assert!(r["playlistId"].is_null());

    // Кеш: те же данные — та же строка; новое прослушивание — пересчёт
    let again = rewind(&app, &me, y).await;
    assert_eq!(again["computedAt"], r["computedAt"]);
    play(&db, user, other, at(y, 5, 5, 12), 180_000, "stream").await;
    let fresh_r = rewind(&app, &me, y).await;
    assert_ne!(fresh_r["computedAt"], r["computedAt"]);
    assert_eq!(fresh_r["plays"], 8);

    // Плейлист «Твои лучшие треки»: создаётся, повтор обновляет тот же
    let res = app
        .call(
            Method::POST,
            &format!("/api/rewind/{y}/playlist"),
            None,
            &me,
        )
        .await;
    assert_eq!(res.status, StatusCode::CREATED, "{}", res.text);
    assert_eq!(res.body["title"], format!("Твои лучшие треки {y}"));
    assert_eq!(res.body["tracksCount"], 3);
    let id = res.body["id"].clone();
    let res = app
        .call(
            Method::POST,
            &format!("/api/rewind/{y}/playlist"),
            None,
            &me,
        )
        .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    assert_eq!(res.body["id"], id);
    assert_eq!(rewind(&app, &me, y).await["playlistId"], id);
    let detail = app
        .get(&format!("/api/playlists/{}", id.as_str().unwrap()), &me)
        .await;
    assert_eq!(detail.body["items"][0]["track"]["title"], "Хит");

    // Годы: с прослушиваниями и текущий
    let years = app.get("/api/rewind", &me).await;
    assert_eq!(years.status, StatusCode::OK);
    let list: Vec<i64> = years.body["years"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v["year"].as_i64().unwrap())
        .collect();
    assert_eq!(list, vec![i64::from(y + 1), i64::from(y), i64::from(y - 1)]);
    assert_eq!(years.body["current"], y + 1);

    // Пустой год — целый ответ с нулями; будущий — 404; плейлист из пустоты — 400
    let empty = rewind(&app, &me, y - 5).await;
    assert_eq!(empty["plays"], 0);
    assert!(empty["topTracks"].as_array().unwrap().is_empty());
    assert!(empty["streak"].is_null());
    assert_eq!(empty["personality"]["id"], "balanced");
    assert_eq!(
        app.get(&format!("/api/rewind/{}", y + 2), &me).await.status,
        StatusCode::NOT_FOUND
    );
    let res = app
        .call(
            Method::POST,
            &format!("/api/rewind/{}/playlist", y - 5),
            None,
            &me,
        )
        .await;
    assert_eq!(res.status, StatusCode::BAD_REQUEST);

    // Чужие итоги не видны
    let bob = app.listener("bob").await;
    let theirs = rewind(&app, &bob, y).await;
    assert_eq!(theirs["plays"], 0);
    assert!(theirs["playlistId"].is_null());
}
