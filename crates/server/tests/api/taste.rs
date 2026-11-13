//! «Мой вкус» против API: грани с примерами и долями, статистика месяца, «меньше такого» (доля грани,
//! кандидаты волны, повтор, отмена, чужая грань), сброс вкуса; «Радар новинок» и порядок полок главной.

use axum::http::{Method, StatusCode};
use chrono::{Duration, Timelike, Utc};
use dalywave_server::{
    reco::{self, DayPart, taste},
    routes::home::shelf_order,
};
use serde_json::{Value, json};
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    common::{Auth, TestApp},
    synth::{Feat, Synth},
};

/// Часовой пояс слушателя такой, что у него сейчас девять утра: порядок полок главной — утренний.
pub async fn morning(db: &PgPool) {
    let hour = i32::try_from(Utc::now().hour()).unwrap();
    let mut offset = (9 - hour).rem_euclid(24) * 60;
    if offset > 840 {
        offset -= 1440;
    }
    sqlx::query("UPDATE reco_settings SET utc_offset_min = $1")
        .bind(offset as i16)
        .execute(db)
        .await
        .unwrap();
}

async fn user_of(app: &TestApp, auth: &Auth) -> Uuid {
    app.user_id(auth).await.parse().unwrap()
}

async fn rebuild(app: &TestApp, user: Uuid) -> taste::TasteReport {
    let settings = reco::Settings::load(&app.state.db).await.unwrap();
    taste::rebuild(&app.state.db, user, &settings, false)
        .await
        .unwrap()
}

/// `n` лайкнутых и дослушанных треков на оси `axis` у артиста `name`.
async fn taste_on(s: &Synth, user: Uuid, name: &str, axis: usize, n: usize) -> Vec<Uuid> {
    let a = s.artist(name).await;
    let mut out = Vec::new();
    for i in 0..n {
        let t = s
            .track(
                a,
                &format!("{name} {i}"),
                Feat {
                    axis,
                    jitter: i,
                    ..Default::default()
                },
            )
            .await;
        s.like(user, t, 5).await;
        s.play(user, t, 2, true).await;
        out.push(t);
    }
    out
}

async fn overview(app: &TestApp, auth: &Auth) -> Value {
    let res = app.get("/api/taste", auth).await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    res.body
}

fn facet_by_artist<'a>(o: &'a Value, artist: &str) -> &'a Value {
    o["facets"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["artists"][0]["name"] == artist)
        .unwrap_or_else(|| panic!("нет грани {artist}: {o}"))
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn taste_shows_facets_stats_and_what_it_knows(db: PgPool) {
    let app = TestApp::new(db.clone()).await;
    let me = app.listener("ann").await;
    let user = user_of(&app, &me).await;

    // До прослушиваний — пустой, но целый ответ
    let empty = overview(&app, &me).await;
    assert!(empty["facets"].as_array().unwrap().is_empty());
    assert_eq!(empty["stats"]["minutes"], 0);
    assert_eq!(empty["knows"]["likes"], 0);
    assert!(empty["knows"]["updatedAt"].is_null());
    assert_eq!(empty["stats"]["dayParts"].as_array().unwrap().len(), 4);

    let s = Synth::new(db.clone());
    taste_on(&s, user, "Rushex", 0, 8).await;
    taste_on(&s, user, "Molchat Doma", 400, 4).await;
    let report = rebuild(&app, user).await;
    assert_eq!(report.facets, 2, "{report:?}");

    let o = overview(&app, &me).await;
    let facets = o["facets"].as_array().unwrap();
    assert_eq!(facets.len(), 2);
    let shares: f64 = facets.iter().map(|f| f["share"].as_f64().unwrap()).sum();
    assert!((shares - 1.0).abs() < 1e-3, "{shares}");
    let big = facet_by_artist(&o, "Rushex");
    assert!(big["share"].as_f64().unwrap() > 0.5);
    assert!(!big["examples"].as_array().unwrap().is_empty());
    assert!(big["examples"].as_array().unwrap().len() <= 6);
    assert_eq!(big["damped"], 0.0);
    assert!(big["damperId"].is_null());

    // 12 дослушиваний по 3 минуты — 36 минут в этом месяце; Rushex — первым
    assert_eq!(o["stats"]["plays"], 12);
    assert_eq!(o["stats"]["minutes"], 36);
    assert_eq!(o["stats"]["tracks"], 12);
    assert_eq!(o["stats"]["artists"], 2);
    assert_eq!(o["stats"]["topArtists"][0]["artist"]["name"], "Rushex");
    assert_eq!(o["stats"]["topArtists"][0]["plays"], 8);
    let parts: f64 = o["stats"]["dayParts"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["share"].as_f64().unwrap())
        .sum();
    assert!((parts - 1.0).abs() < 1e-3);

    let k = &o["knows"];
    assert_eq!(k["likes"], 12);
    assert_eq!(k["listened"], 12);
    assert_eq!(k["analyzed"], 12);
    assert!(k["updatedAt"].is_string());
    assert!(k["resetAt"].is_null());

    // Чужой вкус не виден
    let bob = app.listener("bob").await;
    assert!(
        overview(&app, &bob).await["facets"]
            .as_array()
            .unwrap()
            .is_empty()
    );
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn less_like_this_dampens_a_facet_and_the_wave(db: PgPool) {
    let app = TestApp::new(db.clone()).await;
    let me = app.listener("ann").await;
    let user = user_of(&app, &me).await;
    let s = Synth::new(db.clone());
    taste_on(&s, user, "Rushex", 0, 6).await;
    taste_on(&s, user, "Molchat Doma", 400, 6).await;
    rebuild(&app, user).await;

    // Новые треки обеих осей — кандидаты волны
    let other = s.artist("Fresh").await;
    let near = s
        .track(
            other,
            "near Rushex",
            Feat {
                axis: 0,
                jitter: 30,
                ..Default::default()
            },
        )
        .await;
    let far = s
        .track(
            other,
            "near Molchat",
            Feat {
                axis: 400,
                jitter: 30,
                ..Default::default()
            },
        )
        .await;
    let damped =
        |cands: &[reco::Cand], id: Uuid| cands.iter().find(|c| c.track_id == id).unwrap().damped;
    let before = reco::candidates(&db, &[near, far], user, None, None)
        .await
        .unwrap();
    assert_eq!(damped(&before, near), 0.0);

    let o = overview(&app, &me).await;
    let facet = facet_by_artist(&o, "Rushex").clone();
    let share_before = facet["share"].as_f64().unwrap();
    let id = facet["id"].as_str().unwrap();

    let res = app
        .call(
            Method::POST,
            &format!("/api/taste/facets/{id}/less"),
            None,
            &me,
        )
        .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    assert_eq!(res.body["strength"], 0.5);
    assert!(!res.body["examples"].as_array().unwrap().is_empty());
    let damper = res.body["id"].as_str().unwrap().to_string();

    let o = overview(&app, &me).await;
    let facet = facet_by_artist(&o, "Rushex");
    assert!(facet["damped"].as_f64().unwrap() > 0.4, "{facet}");
    assert_eq!(facet["damperId"], damper.as_str());
    assert!(facet["share"].as_f64().unwrap() < share_before - 0.1);
    assert_eq!(facet_by_artist(&o, "Molchat Doma")["damped"], 0.0);
    assert_eq!(o["dampers"].as_array().unwrap().len(), 1);

    let after = reco::candidates(&db, &[near, far], user, None, None)
        .await
        .unwrap();
    assert!(damped(&after, near) > 0.4);
    assert_eq!(damped(&after, far), 0.0);
    // Волна грани ниже по весу: её вес в гранях волны снижен
    let facets = taste::facets(&db, user).await.unwrap();
    let rushex = facets
        .iter()
        .find(|f| f.artists.first().is_some_and(|a| a.1 == "Rushex"))
        .unwrap();
    assert!(rushex.weight < share_before as f32 * 0.7);

    // Повтор — сильнее, а не второе приглушение
    let res = app
        .call(
            Method::POST,
            &format!("/api/taste/facets/{id}/less"),
            None,
            &me,
        )
        .await;
    assert_eq!(res.body["id"], damper.as_str());
    assert_eq!(res.body["strength"], 0.75);
    assert_eq!(
        overview(&app, &me).await["dampers"]
            .as_array()
            .unwrap()
            .len(),
        1
    );

    // Приглушение переживает ночной пересчёт граней
    rebuild(&app, user).await;
    let o = overview(&app, &me).await;
    assert!(facet_by_artist(&o, "Rushex")["damped"].as_f64().unwrap() > 0.6);

    // Чужую грань не приглушить
    let bob = app.listener("bob").await;
    let res = app
        .call(
            Method::POST,
            &format!("/api/taste/facets/{id}/less"),
            None,
            &bob,
        )
        .await;
    assert_eq!(res.status, StatusCode::NOT_FOUND);

    // Отмена; повтор безопасен
    for _ in 0..2 {
        let res = app
            .delete(&format!("/api/taste/dampers/{damper}"), &me)
            .await;
        assert_eq!(res.status, StatusCode::NO_CONTENT);
    }
    let o = overview(&app, &me).await;
    assert!(o["dampers"].as_array().unwrap().is_empty());
    assert_eq!(facet_by_artist(&o, "Rushex")["damped"], 0.0);
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn reset_starts_taste_over(db: PgPool) {
    let app = TestApp::new(db.clone()).await;
    let me = app.listener("ann").await;
    let user = user_of(&app, &me).await;
    let s = Synth::new(db.clone());
    let liked = taste_on(&s, user, "Rushex", 0, 8).await;
    let report = rebuild(&app, user).await;
    assert!(report.facets > 0, "{report:?}");
    // Микс дня, собранный по старому вкусу
    sqlx::query(
        "INSERT INTO playlists (user_id, title, kind, rules) VALUES ($1, 'Микс дня 1', 'mix', '{\"generator\": \"daily_mix\", \"slot\": 1}')",
    )
    .bind(user)
    .execute(&db)
    .await
    .unwrap();
    let id = overview(&app, &me).await["facets"][0]["id"]
        .as_str()
        .unwrap()
        .to_string();
    app.call(
        Method::POST,
        &format!("/api/taste/facets/{id}/less"),
        None,
        &me,
    )
    .await;
    s.block(user, "track", liked[0]).await;

    let res = app
        .post("/api/taste/reset", json!({ "confirm": false }), &me)
        .await;
    assert_eq!(res.status, StatusCode::BAD_REQUEST, "{}", res.text);

    let res = app
        .post("/api/taste/reset", json!({ "confirm": true }), &me)
        .await;
    assert_eq!(res.status, StatusCode::NO_CONTENT, "{}", res.text);
    let o = overview(&app, &me).await;
    assert!(o["facets"].as_array().unwrap().is_empty());
    assert!(o["dampers"].as_array().unwrap().is_empty());
    assert!(o["knows"]["resetAt"].is_string());
    // Медиатека и «не рекомендовать» на месте, миксы — нет
    assert_eq!(o["knows"]["likes"], 8);
    assert_eq!(o["knows"]["blocked"], 1);
    let mixes: i64 =
        sqlx::query_scalar("SELECT count(*) FROM playlists WHERE user_id = $1 AND kind = 'mix'")
            .bind(user)
            .fetch_one(&db)
            .await
            .unwrap();
    assert_eq!(mixes, 0);
    assert!(app.audit_kinds().await.contains(&"taste.reset".to_string()));

    // Старое вкус не строит…
    let report = rebuild(&app, user).await;
    assert_eq!(report.tracks, 0, "{report:?}");
    assert!(
        overview(&app, &me).await["facets"]
            .as_array()
            .unwrap()
            .is_empty()
    );

    // …а новое — строит
    let a = s.artist("Molchat Doma").await;
    for i in 0..4 {
        let t = s
            .track(
                a,
                &format!("after {i}"),
                Feat {
                    axis: 400,
                    jitter: i,
                    ..Default::default()
                },
            )
            .await;
        s.play(user, t, 0, true).await;
    }
    let report = rebuild(&app, user).await;
    assert_eq!(report.tracks, 4, "{report:?}");
    let o = overview(&app, &me).await;
    assert_eq!(o["facets"][0]["artists"][0]["name"], "Molchat Doma");
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn home_has_release_radar_in_time_of_day_order(db: PgPool) {
    let app = TestApp::new(db.clone()).await;
    let me = app.listener("ann").await;
    let user = user_of(&app, &me).await;
    let s = Synth::new(db.clone());
    taste_on(&s, user, "Rushex", 0, 4).await;

    // Свежий релиз артиста из подписок, старый релиз и релиз заблокированного артиста
    let followed = s.artist("Lowborn").await;
    let blocked = s.artist("Blocked").await;
    sqlx::query("INSERT INTO followed_artists (user_id, artist_id) VALUES ($1, $2), ($1, $3)")
        .bind(user)
        .bind(followed)
        .bind(blocked)
        .execute(&db)
        .await
        .unwrap();
    s.block(user, "artist", blocked).await;
    let release = |artist: Uuid, title: &'static str, days_ago: i64| {
        let db = db.clone();
        async move {
            let album: Uuid = sqlx::query_scalar(
                "INSERT INTO albums (title, artist_id, record_type, release_date) VALUES ($1, $2, 'single', $3)
                 RETURNING id",
            )
            .bind(title)
            .bind(artist)
            .bind((Utc::now() - Duration::days(days_ago)).date_naive())
            .fetch_one(&db)
            .await
            .unwrap();
            sqlx::query("INSERT INTO artist_releases (artist_id, album_id) VALUES ($1, $2)")
                .bind(artist)
                .bind(album)
                .execute(&db)
                .await
                .unwrap();
            album
        }
    };
    let fresh = release(followed, "Fresh", 3).await;
    let newer = release(followed, "Newer", 1).await;
    let old = release(followed, "Old", 90).await;
    let hidden = release(blocked, "Hidden", 2).await;
    let disliked = release(followed, "Disliked", 2).await;
    let mut gone = Vec::new();
    for (album, n) in [(fresh, 4), (newer, 1), (old, 1), (hidden, 1), (disliked, 2)] {
        for i in 0..n {
            let t = s
                .track(
                    if album == hidden { blocked } else { followed },
                    &format!("radar {i}"),
                    Feat {
                        axis: 800,
                        rank: 1000 - i,
                        ..Default::default()
                    },
                )
                .await;
            sqlx::query("UPDATE tracks SET album_id = $2, track_position = $3 WHERE id = $1")
                .bind(t)
                .bind(album)
                .bind(i + 1)
                .execute(&db)
                .await
                .unwrap();
            if album == disliked {
                gone.push(t);
            }
        }
    }
    // Из релиза не осталось ничего, что можно рекомендовать: трек «не рекомендовать», трек «не нравится»
    s.block(user, "track", gone[0]).await;
    s.dislike(user, gone[1]).await;
    let res = app.get("/api/home", &me).await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    let shelves = res.body["shelves"].as_array().unwrap();
    let radar = shelves
        .iter()
        .find(|s| s["id"] == "release-radar")
        .unwrap_or_else(|| panic!("нет радара: {}", res.text));
    assert_eq!(radar["kind"], "release_radar");
    let ids: Vec<String> = radar["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| {
            assert_eq!(i["type"], "album");
            i["id"].as_str().unwrap().to_string()
        })
        .collect();
    // По карточке на релиз, свежие первыми; старое, заблокированное и целиком отвергнутое — мимо
    assert_eq!(ids, vec![newer.to_string(), fresh.to_string()]);
    assert_eq!(radar["items"][0]["recordType"], "single");
    // В «Новинках подписок» те же релизы не повторяются, заблокированного артиста там тоже нет
    let fresh: Vec<&str> = shelves
        .iter()
        .find(|s| s["id"] == "new-releases")
        .map(|s| {
            s["items"]
                .as_array()
                .unwrap()
                .iter()
                .map(|i| i["title"].as_str().unwrap())
                .collect()
        })
        .unwrap_or_default();
    assert_eq!(fresh, vec!["Disliked"], "{}", res.text);

    // Порядок полок — по времени суток слушателя
    let settings = reco::Settings::load(&db).await.unwrap();
    let order = shelf_order(DayPart::of_hour(settings.local_hour(Utc::now())));
    let got: Vec<usize> = shelves
        .iter()
        .map(|s| {
            order
                .iter()
                .position(|id| s["id"] == *id)
                .unwrap_or_else(|| panic!("полки {} нет в порядке", s["id"]))
        })
        .collect();
    assert!(got.windows(2).all(|w| w[0] < w[1]), "{got:?}");

    // Утром «Миксы дня» выше «Забытых любимых», ночью миксы — первыми
    let morning = shelf_order(DayPart::Morning);
    let pos = |o: &[&str], id: &str| o.iter().position(|x| *x == id).unwrap();
    assert!(pos(&morning, "release-radar") < pos(&morning, "forgotten-favorites"));
    assert_eq!(shelf_order(DayPart::Night)[0], "daily-mixes");
}

/// Превью (30 с вместо трека) «дослушивается» за полминуты — вкус по нему не собирается. Грань,
/// собранная раньше из одних превью, при пересчёте уходит, а не остаётся «всем вкусом» с долей 1.
#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn previews_do_not_make_a_taste(db: PgPool) {
    let app = TestApp::new(db.clone()).await;
    let me = app.listener("ann").await;
    let user = user_of(&app, &me).await;
    let s = Synth::new(db.clone());
    let a = s.artist("Booker").await;
    for i in 0..6 {
        let t = s
            .track(
                a,
                &format!("Превью {i}"),
                Feat {
                    axis: 0,
                    jitter: i,
                    ..Default::default()
                },
            )
            .await;
        sqlx::query(
            "INSERT INTO play_events (client_event_id, user_id, track_id, started_at, played_ms, duration_ms, end_reason, source)
             VALUES ($1, $2, $3, now() - interval '1 hour', 29000, 30000, 'completed', 'stream')",
        )
        .bind(Uuid::now_v7())
        .bind(user)
        .bind(t)
        .execute(&db)
        .await
        .unwrap();
    }
    // Старая грань из превью (как собиралась до исправления)
    sqlx::query(
        "INSERT INTO taste_facets (user_id, position, label, centroid, weight)
         VALUES ($1, 0, 'Booker', array_fill(0.03::real, ARRAY[1280])::halfvec(1280), 1.0)",
    )
    .bind(user)
    .execute(&db)
    .await
    .unwrap();
    let report = rebuild(&app, user).await;
    assert_eq!(report.tracks, 0, "{report:?}");
    assert!(
        overview(&app, &me).await["facets"]
            .as_array()
            .unwrap()
            .is_empty()
    );
}
