//! Предсохранение: анонс на странице артиста, «Скоро выйдет», доставка в день выхода по часовому
//! поясу слушателя, перенос и отмена релиза.

use axum::http::StatusCode;
use chrono::{DateTime, Duration, NaiveDate, TimeZone, Utc};
use dalywave_server::presave::{self, Delivered};
use serde_json::{Value, json};
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    common::{Auth, TestApp},
    deezer::{LOWBORN, fixture},
};

/// EP «melancholy» Lowborn: в записанном ответе Deezer — анонс без треков и обложки.
const MELANCHOLY: i64 = 1052752402;

/// Релизы Lowborn, где «melancholy» выходит `release`.
fn announce(app: &TestApp, release: NaiveDate) {
    let mut albums = fixture(&format!("artist_{LOWBORN}_albums"));
    assert_eq!(albums["data"][0]["id"], MELANCHOLY);
    albums["data"][0]["release_date"] = json!(release.to_string());
    app.deezer.set(&format!("/artist/{LOWBORN}/albums"), albums);
    set_album(app, release, 0);
}

/// Альбом в Deezer: дата выхода и сколько треков уже есть.
fn set_album(app: &TestApp, release: NaiveDate, tracks: usize) {
    app.deezer.set(
        &format!("/album/{MELANCHOLY}"),
        json!({
            "id": MELANCHOLY,
            "title": "melancholy",
            "md5_image": "041a7401ead0c50765000792dc75b14e",
            "release_date": release.to_string(),
            "record_type": "ep",
            "nb_tracks": tracks,
            "artist": { "id": LOWBORN, "name": "Lowborn" },
            "contributors": [{ "id": LOWBORN, "name": "Lowborn" }],
        }),
    );
    let data: Vec<Value> = (0..tracks)
        .map(|i| {
            json!({
                "id": 990_000_000 + i as i64,
                "title": format!("melancholy {}", i + 1),
                "isrc": format!("QZTST26{:05}", i + 1),
                "duration": 180,
                "track_position": i + 1,
                "disk_number": 1,
                "artist": { "id": LOWBORN, "name": "Lowborn" },
            })
        })
        .collect();
    app.deezer.set(
        &format!("/album/{MELANCHOLY}/tracks"),
        json!({ "data": data, "total": tracks }),
    );
}

async fn artist(app: &TestApp, auth: &Auth) -> Value {
    let id = app.resolve_id("artist", LOWBORN, auth).await;
    let res = app.get(&format!("/api/artists/{id}"), auth).await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    res.body
}

async fn deliver(app: &TestApp, now: DateTime<Utc>) -> Delivered {
    presave::deliver(&app.state.db, &app.state.deezer, now)
        .await
        .unwrap()
}

async fn set_offset(db: &PgPool, user: &str, offset: i16) {
    sqlx::query(
        "INSERT INTO notification_settings (user_id, utc_offset_min) VALUES ($1::uuid, $2)",
    )
    .bind(user)
    .bind(offset)
    .execute(db)
    .await
    .unwrap();
}

/// Полночь `day` по UTC плюс `minutes`.
fn at(day: NaiveDate, minutes: i64) -> DateTime<Utc> {
    Utc.from_utc_datetime(&day.and_hms_opt(0, 0, 0).unwrap()) + Duration::minutes(minutes)
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn presaved_release_lands_in_the_library_on_release_day(db: PgPool) {
    let app = TestApp::new(db.clone()).await;
    let moscow = app.listener("ann").await;
    let new_york = app.listener("bob").await;
    set_offset(&db, &app.user_id(&moscow).await, 180).await;
    set_offset(&db, &app.user_id(&new_york).await, -300).await;
    let release = Utc::now().date_naive() + Duration::days(30);
    announce(&app, release);

    // Страница артиста: анонс — в «Скоро», а не среди вышедших EP
    let page = artist(&app, &moscow).await;
    let upcoming = page["upcoming"].as_array().unwrap();
    assert_eq!(upcoming.len(), 1, "{page}");
    assert_eq!(upcoming[0]["album"]["title"], "melancholy");
    assert_eq!(upcoming[0]["album"]["releaseDate"], release.to_string());
    assert_eq!(upcoming[0]["presaved"], false);
    assert!(
        page["releases"]["eps"]
            .as_array()
            .unwrap()
            .iter()
            .all(|a| a["title"] != "melancholy")
    );
    let album = upcoming[0]["album"]["id"].as_str().unwrap().to_string();

    for who in [&moscow, &new_york] {
        let res = app
            .put(&format!("/api/library/presaves/{album}"), who)
            .await;
        assert_eq!(res.status, StatusCode::OK, "{}", res.text);
        assert_eq!(res.body["savedNow"], false);
    }
    // Повтор безопасен
    let again = app
        .put(&format!("/api/library/presaves/{album}"), &moscow)
        .await;
    assert_eq!(again.status, StatusCode::OK);
    let page = artist(&app, &moscow).await;
    assert_eq!(page["upcoming"][0]["presaved"], true);
    let list = app.get("/api/library/upcoming", &moscow).await;
    assert_eq!(list.status, StatusCode::OK, "{}", list.text);
    assert_eq!(list.body["items"].as_array().unwrap().len(), 1);
    assert_eq!(list.body["items"][0]["presaved"], true);
    assert_eq!(list.body["items"][0]["followed"], false);

    // За день до выхода — ничего
    assert_eq!(
        deliver(&app, at(release, -24 * 60)).await,
        Delivered::default()
    );

    // 12-го в 21:30 UTC: в Москве уже день выхода, но треков в Deezer ещё нет — ждём
    let moscow_midnight = at(release, -150);
    let r = deliver(&app, moscow_midnight).await;
    assert_eq!(r.waiting, 1, "{r:?}");
    assert_eq!(r.delivered, 0);

    // Треки появились: москвичу — в медиатеку, в Нью-Йорке ещё вчера
    set_album(&app, release, 2);
    let r = deliver(&app, moscow_midnight + Duration::minutes(60)).await;
    assert_eq!(r.delivered, 1, "{r:?}");
    let saved = app.get("/api/library/albums", &moscow).await;
    assert_eq!(
        saved.body["items"][0]["album"]["id"],
        album.as_str(),
        "{}",
        saved.text
    );
    assert_eq!(
        app.get("/api/library/albums", &new_york).await.body["items"]
            .as_array()
            .unwrap()
            .len(),
        0
    );
    let (title, body): (String, String) = sqlx::query_as(
        "SELECT title, body FROM notifications WHERE user_id = $1::uuid AND kind = 'release'",
    )
    .bind(app.user_id(&moscow).await)
    .fetch_one(&db)
    .await
    .unwrap();
    assert_eq!(title, "Вышел EP «melancholy»");
    assert_eq!(body, "Lowborn · уже в медиатеке");
    // Треки — на скачивание, как у сохранённого альбома
    let queued: i64 = sqlx::query_scalar("SELECT count(*) FROM acquisitions")
        .fetch_one(&db)
        .await
        .unwrap();
    assert_eq!(queued, 2);
    // Доставленное из «Скоро выйдет» уходит
    let list = app.get("/api/library/upcoming", &moscow).await;
    assert!(
        list.body["items"].as_array().unwrap().is_empty(),
        "{}",
        list.text
    );

    // Нью-Йорк (UTC−5): полночь там — в 05:00 UTC дня выхода
    assert_eq!(
        deliver(&app, at(release, 4 * 60 + 59)).await,
        Delivered::default()
    );
    let r = deliver(&app, at(release, 5 * 60)).await;
    assert_eq!(r.delivered, 1, "{r:?}");
    // Повторный проход ничего не дублирует
    assert_eq!(
        deliver(&app, at(release, 6 * 60)).await,
        Delivered::default()
    );
    let notes: i64 =
        sqlx::query_scalar("SELECT count(*) FROM notifications WHERE kind = 'release'")
            .fetch_one(&db)
            .await
            .unwrap();
    assert_eq!(notes, 2);
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn postponed_and_cancelled_releases(db: PgPool) {
    let app = TestApp::new(db.clone()).await;
    let me = app.listener("ann").await;
    set_offset(&db, &app.user_id(&me).await, 0).await;
    let release = Utc::now().date_naive() + Duration::days(10);
    announce(&app, release);
    let page = artist(&app, &me).await;
    let album = page["upcoming"][0]["album"]["id"]
        .as_str()
        .unwrap()
        .to_string();
    let res = app
        .put(&format!("/api/library/presaves/{album}"), &me)
        .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);

    // Перенос на неделю: в прежний день альбом ждёт новую дату
    let later = release + Duration::days(7);
    set_album(&app, later, 3);
    let r = deliver(&app, at(release, 60)).await;
    assert_eq!(r.postponed, 1, "{r:?}");
    let list = app.get("/api/library/upcoming", &me).await;
    assert_eq!(
        list.body["items"][0]["album"]["releaseDate"],
        later.to_string()
    );
    assert_eq!(list.body["items"][0]["presaved"], true);
    // Пока новая дата не настала — Deezer не спрашиваем
    let hits = app.deezer.hits(&format!("/album/{MELANCHOLY}"));
    assert_eq!(deliver(&app, at(release, 120)).await, Delivered::default());
    assert_eq!(app.deezer.hits(&format!("/album/{MELANCHOLY}")), hits);

    // Отмена: альбом пропал из Deezer — предсохранение гаснет
    app.deezer.set(
        &format!("/album/{MELANCHOLY}"),
        json!({ "error": { "type": "DataException", "message": "no data", "code": 800 } }),
    );
    let r = deliver(&app, at(later, 60)).await;
    assert_eq!(r.cancelled, 1, "{r:?}");
    let list = app.get("/api/library/upcoming", &me).await;
    assert!(
        list.body["items"].as_array().unwrap().is_empty(),
        "{}",
        list.text
    );
    assert_eq!(deliver(&app, at(later, 120)).await, Delivered::default());

    // Снова «Сохранить заранее» после отмены — снова ждёт; «Убрать» — снимает
    let res = app
        .put(&format!("/api/library/presaves/{album}"), &me)
        .await;
    assert_eq!(res.body["savedNow"], false);
    let res = app
        .delete(&format!("/api/library/presaves/{album}"), &me)
        .await;
    assert_eq!(res.status, StatusCode::NO_CONTENT);
    let pending: i64 = sqlx::query_scalar("SELECT count(*) FROM album_presaves")
        .fetch_one(&db)
        .await
        .unwrap();
    assert_eq!(pending, 0);

    // Уже вышедший альбом «сохранить заранее» нельзя — он сразу в медиатеке
    sqlx::query("UPDATE albums SET release_date = current_date - 3 WHERE id = $1::uuid")
        .bind(&album)
        .execute(&db)
        .await
        .unwrap();
    set_album(&app, Utc::now().date_naive() - Duration::days(3), 1);
    let res = app
        .put(&format!("/api/library/presaves/{album}"), &me)
        .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    assert_eq!(res.body["savedNow"], true);
    let saved = app.get("/api/library/albums", &me).await;
    assert_eq!(saved.body["items"][0]["album"]["id"], album.as_str());

    let missing = app
        .put(&format!("/api/library/presaves/{}", Uuid::nil()), &me)
        .await;
    assert_eq!(missing.status, StatusCode::NOT_FOUND);
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn followed_artists_announcements_are_listed(db: PgPool) {
    let app = TestApp::new(db.clone()).await;
    let me = app.listener("ann").await;
    let release = Utc::now().date_naive() + Duration::days(45);
    announce(&app, release);
    let page = artist(&app, &me).await;
    let id = page["artist"]["id"].as_str().unwrap().to_string();
    assert!(
        app.get("/api/library/upcoming", &me).await.body["items"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    let res = app.put(&format!("/api/library/artists/{id}"), &me).await;
    assert!(res.status.is_success(), "{}", res.text);
    let list = app.get("/api/library/upcoming", &me).await;
    let items = list.body["items"].as_array().unwrap();
    assert_eq!(items.len(), 1, "{}", list.text);
    assert_eq!(items[0]["followed"], true);
    assert_eq!(items[0]["presaved"], false);
}
