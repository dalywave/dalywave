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

/// Итоги — событие: в тестах оно открыто за текущий год, как в декабре.
async fn open_event(db: &PgPool) {
    sqlx::query("UPDATE rewind_event SET mode = 'open', year = $1")
        .bind(Utc::now().year())
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
    open_event(&db).await;
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
    // По часам: больше всего — в 2 часа ночи (два полных трека)
    let hours = r["personality"]["hours"].as_array().unwrap();
    assert_eq!(hours.len(), 24);
    assert_eq!(r["personality"]["peakHour"], 2);
    let sum: f64 = hours.iter().map(|h| h.as_f64().unwrap()).sum();
    assert!((sum - 1.0).abs() < 1e-3, "{sum}");
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

/// Полный анализ с кривой энергии: куплет 300, припев 800 с 50-й секунды, доли каждые 2 секунды.
async fn full_analysis(db: &PgPool, track: Uuid) {
    let mut curve = vec![300i16; 180];
    curve[50..70].fill(800);
    curve[110..130].fill(800);
    curve[150..].fill(200);
    let downbeats: Vec<i32> = (0..90).map(|i| i * 2000).collect();
    sqlx::query(
        "UPDATE track_analysis SET source = 'full', energy_curve = $2, downbeats_ms = $3
         WHERE recording_id = (SELECT recording_id FROM tracks WHERE id = $1)",
    )
    .bind(track)
    .bind(&curve)
    .bind(&downbeats)
    .execute(db)
    .await
    .unwrap();
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn soundtrack_more_and_year_wave(db: PgPool) {
    sqlx::query("UPDATE reco_settings SET utc_offset_min = 0")
        .execute(&db)
        .await
        .unwrap();
    open_event(&db).await;
    let app = TestApp::new(db.clone()).await;
    let me = app.listener("ann").await;
    let user: Uuid = app.user_id(&me).await.parse().unwrap();
    let y = Utc::now().year() - 1;
    let s = Synth::new(db.clone());
    let star = s.artist("Star").await;
    let other = s.artist("Other").await;
    let hit = s
        .track(
            star,
            "Хит",
            Feat {
                status: "ready",
                ..Feat::default()
            },
        )
        .await;
    let second = s.track(star, "Второй", Feat::default()).await;
    let third = s
        .track(
            other,
            "Третий",
            Feat {
                axis: 600,
                ..Feat::default()
            },
        )
        .await;
    full_analysis(&db, hit).await;
    for d in 1..=5 {
        play(&db, user, hit, at(y, 2, d, 12), 180_000, "stream").await;
    }
    for d in 1..=3 {
        play(&db, user, second, at(y, 3, d, 12), 180_000, "stream").await;
    }
    for d in 1..=2 {
        play(&db, user, third, at(y, 4, d, 12), 180_000, "stream").await;
    }

    // Музыка историй: трек года — с припева по анализу, остальное — с 30 %
    let res = app.get(&format!("/api/rewind/{y}/soundtrack"), &me).await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    let cues = res.body["cues"].as_array().unwrap().clone();
    let cue = |slide: &str| cues.iter().find(|c| c["slide"] == slide).cloned();
    let toy = cue("toy").expect("трек года");
    assert_eq!(toy["track"]["title"], "Хит");
    assert_eq!(toy["startMs"], 50_000);
    assert_eq!(toy["startFrom"], "analysis");
    let intro = cue("intro").expect("вступление");
    assert_eq!(intro["track"]["title"], "Второй");
    assert_eq!(intro["startMs"], 54_000);
    assert_eq!(intro["startFrom"], "estimate");
    assert_eq!(cue("summary").unwrap()["track"]["title"], "Хит");
    // Слайды без своего трека продолжают прежний: повторов, кроме финала, нет
    let mut ids: Vec<&str> = cues
        .iter()
        .filter(|c| c["slide"] != "summary")
        .map(|c| c["track"]["id"].as_str().unwrap())
        .collect();
    let n = ids.len();
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(ids.len(), n, "{cues:?}");

    // Чего нет на сервере — на скачивание в полосу «сейчас», ниже запуска трека
    let mut queued = Vec::new();
    for _ in 0..50 {
        queued = sqlx::query_as::<_, (Uuid, String, i16)>(
            "SELECT a.track_id, j.lane, j.priority FROM acquisitions a JOIN jobs j ON j.id = a.job_id",
        )
        .fetch_all(&db)
        .await
        .unwrap();
        if queued.len() >= 2 {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    let mut tracks: Vec<Uuid> = queued.iter().map(|q| q.0).collect();
    tracks.sort();
    let mut want = vec![second, third];
    want.sort();
    assert_eq!(tracks, want, "готовый трек не качается: {queued:?}");
    assert!(
        queued
            .iter()
            .all(|q| q.1 == "now" && (1..=5).contains(&q.2))
    );

    // «Ещё послушать»: неслышанное, похожее на любимое; от артиста — не больше двух
    let fresh = s.artist("Fresh").await;
    let mut near = Vec::new();
    for j in 1..=3 {
        near.push(
            s.track(
                fresh,
                &format!("Новое {j}"),
                Feat {
                    jitter: j,
                    ..Feat::default()
                },
            )
            .await,
        );
    }
    let alone = s.artist("Alone").await;
    let single = s
        .track(
            alone,
            "Одно",
            Feat {
                jitter: 9,
                ..Feat::default()
            },
        )
        .await;
    // Та же песня другим релизом (другая запись) — уже знакома
    let rerelease = s
        .track(
            star,
            "хит",
            Feat {
                jitter: 5,
                ..Feat::default()
            },
        )
        .await;
    let res = app.get(&format!("/api/rewind/{y}/more"), &me).await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    let items: Vec<Uuid> = res.body["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["id"].as_str().unwrap().parse().unwrap())
        .collect();
    assert!(items.contains(&single), "{}", res.body);
    assert_eq!(items.iter().filter(|t| near.contains(t)).count(), 2);
    assert!(
        ![hit, second, third, rerelease]
            .iter()
            .any(|t| items.contains(t))
    );
    assert!(res.body["playlistId"].is_null());

    let body = serde_json::json!({ "trackIds": items });
    let res = app
        .post(&format!("/api/rewind/{y}/more"), body.clone(), &me)
        .await;
    assert_eq!(res.status, StatusCode::CREATED, "{}", res.text);
    assert_eq!(res.body["title"], format!("Ещё послушать {y}"));
    assert_eq!(res.body["tracksCount"], items.len());
    let id = res.body["id"].clone();
    let res = app.post(&format!("/api/rewind/{y}/more"), body, &me).await;
    assert_eq!(res.status, StatusCode::OK);
    assert_eq!(res.body["id"], id);
    assert_eq!(
        app.get(&format!("/api/rewind/{y}/more"), &me).await.body["playlistId"],
        id
    );
    let empty = serde_json::json!({ "trackIds": [] });
    let res = app.post(&format!("/api/rewind/{y}/more"), empty, &me).await;
    assert_eq!(res.status, StatusCode::BAD_REQUEST);
    // Лучшие треки года — отдельный плейлист, «Ещё послушать» его не трогает
    assert!(rewind(&app, &me, y).await["playlistId"].is_null());

    // Волна из итогов года
    let res = app
        .post(
            "/api/wave/sessions",
            serde_json::json!({ "settings": { "seed": { "type": "year", "text": y.to_string() } } }),
            &me,
        )
        .await;
    assert_eq!(res.status, StatusCode::CREATED, "{}", res.text);
    assert_eq!(res.body["title"], format!("Волна {y}"));
    let res = app
        .post(
            "/api/wave/sessions",
            serde_json::json!({ "settings": { "seed": { "type": "year", "text": (y - 5).to_string() } } }),
            &me,
        )
        .await;
    assert_eq!(res.status, StatusCode::BAD_REQUEST);

    // Будущий год — 404
    for path in ["soundtrack", "more"] {
        assert_eq!(
            app.get(&format!("/api/rewind/{}/{path}", y + 2), &me)
                .await
                .status,
            StatusCode::NOT_FOUND
        );
    }
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn rewind_is_an_event(db: PgPool) {
    sqlx::query("UPDATE reco_settings SET utc_offset_min = 0")
        .execute(&db)
        .await
        .unwrap();
    let app = TestApp::new(db.clone()).await;
    let me = app.listener("ann").await;
    let (_, admin) = app.admin("boss").await;
    let y = Utc::now().year();
    let user: Uuid = app.user_id(&me).await.parse().unwrap();
    let s = Synth::new(db.clone());
    let artist = s.artist("Star").await;
    let track = s.track(artist, "Хит", Feat::default()).await;
    play(&db, user, track, at(y - 1, 3, 1, 12), 180_000, "stream").await;

    // Закрыто: слушателю итогов не видно вовсе
    let res = app
        .call(
            Method::PATCH,
            "/api/admin/rewind",
            Some(serde_json::json!({ "mode": "closed" })),
            &admin,
        )
        .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    assert!(res.body["openYear"].is_null());
    for path in [
        "/api/rewind".to_string(),
        format!("/api/rewind/{}", y - 1),
        format!("/api/rewind/{}/soundtrack", y - 1),
        format!("/api/rewind/{}/more", y - 1),
    ] {
        let res = app.get(&path, &me).await;
        assert_eq!(res.status, StatusCode::NOT_FOUND, "{path}");
        assert_eq!(res.code(), "not_open", "{path}");
    }
    let res = app
        .call(
            Method::POST,
            &format!("/api/rewind/{}/seen", y - 1),
            None,
            &me,
        )
        .await;
    assert_eq!(res.code(), "not_open");
    // Администратор видит всегда — предпросмотр любого года
    let years = app.get("/api/rewind", &admin).await;
    assert_eq!(years.status, StatusCode::OK, "{}", years.text);
    assert_eq!(years.body["open"], false);
    assert_eq!(years.body["current"], y);
    assert_eq!(
        app.get(&format!("/api/rewind/{}", y - 1), &admin)
            .await
            .status,
        StatusCode::OK
    );
    // Настройки события — только администратору
    assert_eq!(
        app.get("/api/admin/rewind", &me).await.status,
        StatusCode::FORBIDDEN
    );

    // Открыто вручную за прошлый год, стиль «Пластинка»
    let res = app
        .call(
            Method::PATCH,
            "/api/admin/rewind",
            Some(serde_json::json!({ "mode": "open", "year": y - 1, "style": "record" })),
            &admin,
        )
        .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    assert_eq!(res.body["mode"], "open");
    assert_eq!(res.body["openYear"], y - 1);
    assert_eq!(res.body["style"], "record");
    assert!(res.body["opensAt"].is_null());
    assert!(
        res.body["years"]
            .as_array()
            .unwrap()
            .contains(&serde_json::json!(y - 1))
    );
    let years = app.get("/api/rewind", &me).await;
    assert_eq!(years.status, StatusCode::OK, "{}", years.text);
    assert_eq!(years.body["open"], true);
    assert_eq!(years.body["current"], y - 1);
    assert_eq!(years.body["style"], "record");
    assert_eq!(years.body["seen"], false);
    let list: Vec<i64> = years.body["years"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v["year"].as_i64().unwrap())
        .collect();
    assert_eq!(list, vec![i64::from(y - 1)], "год позже открытого не виден");
    assert_eq!(rewind(&app, &me, y - 1).await["plays"], 1);
    assert_eq!(
        app.get(&format!("/api/rewind/{y}"), &me).await.code(),
        "not_open"
    );

    // Посмотрел — карточка «готовы» больше не зовёт
    let res = app
        .call(
            Method::POST,
            &format!("/api/rewind/{}/seen", y - 1),
            None,
            &me,
        )
        .await;
    assert_eq!(res.status, StatusCode::NO_CONTENT, "{}", res.text);
    let res = app
        .call(
            Method::POST,
            &format!("/api/rewind/{}/seen", y - 1),
            None,
            &me,
        )
        .await;
    assert_eq!(res.status, StatusCode::NO_CONTENT);
    assert_eq!(app.get("/api/rewind", &me).await.body["seen"], true);

    // Случайный стиль — свой у слушателя и постоянный
    let res = app
        .call(
            Method::PATCH,
            "/api/admin/rewind",
            Some(serde_json::json!({ "style": "random" })),
            &admin,
        )
        .await;
    assert_eq!(res.body["style"], "random");
    assert_eq!(res.body["mode"], "open", "режим не тронут");
    let a = app.get("/api/rewind", &me).await.body["style"].clone();
    assert!(["wave", "spread", "record"].contains(&a.as_str().unwrap()));
    assert_eq!(app.get("/api/rewind", &me).await.body["style"], a);

    // Автоматически: открыто с 1 декабря по 15 января, иначе — время открытия
    let res = app
        .call(
            Method::PATCH,
            "/api/admin/rewind",
            Some(serde_json::json!({ "mode": "auto" })),
            &admin,
        )
        .await;
    assert_eq!(res.status, StatusCode::OK);
    let today = Utc::now().date_naive();
    let auto_open = today.month() == 12 || (today.month() == 1 && today.day() <= 15);
    assert_eq!(res.body["openYear"].is_null(), !auto_open);
    assert_eq!(res.body["opensAt"].is_null(), auto_open);

    // Год из будущего — 400
    let res = app
        .call(
            Method::PATCH,
            "/api/admin/rewind",
            Some(serde_json::json!({ "mode": "open", "year": y + 3 })),
            &admin,
        )
        .await;
    assert_eq!(res.status, StatusCode::BAD_REQUEST);
    assert!(
        app.audit_kinds()
            .await
            .iter()
            .any(|k| k == "settings.changed")
    );
}
