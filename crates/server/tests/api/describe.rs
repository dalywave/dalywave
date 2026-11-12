//! «Плейлист по описанию» против API: разбор и подбор с фильтрами (язык, «без мата», «без …», жанр по стилям,
//! ориентир «как …»), «Ещё в том же духе», сохранение и обновление раз в неделю, запасной путь без сервиса
//! анализа, лимит. Каталог — синтетический (`synth.rs`), сервис анализа — заглушка (`fake_ml.rs`):
//! «спокойное» у неё попадает в измерение 7, остальное — в 300.

use std::{collections::HashSet, time::Duration};

use axum::http::{Method, StatusCode};
use dalywave_server::reco::{self, describe, taste, wave};
use serde_json::{Value, json};
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    common::{Auth, TestApp},
    fake_ml::FakeMl,
    synth::{Feat, Synth},
};

async fn user_of(app: &TestApp, auth: &Auth) -> Uuid {
    app.user_id(auth).await.parse().unwrap()
}

fn ids(items: &Value) -> Vec<String> {
    items
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["track"]["id"].as_str().unwrap().to_string())
        .collect()
}

fn artists(items: &Value) -> Vec<String> {
    items
        .as_array()
        .unwrap()
        .iter()
        .map(|i| {
            i["track"]["artists"][0]["name"]
                .as_str()
                .unwrap()
                .to_string()
        })
        .collect()
}

async fn preview(app: &TestApp, auth: &Auth, text: &str) -> Value {
    let res = app
        .post("/api/describe", json!({ "text": text }), auth)
        .await;
    assert_eq!(res.status, StatusCode::OK, "{text}: {}", res.text);
    res.body
}

/// Вкус на оси 0 и спокойный каталог на оси 7: половина — русское, у части — explicit.
struct World {
    calm_ru: Vec<Uuid>,
    calm_en: Vec<Uuid>,
    explicit: HashSet<Uuid>,
}

async fn world(app: &TestApp, user: Uuid) -> World {
    let s = Synth::new(app.state.db.clone());
    for i in 0..4 {
        let a = s.artist(&format!("Liked {i}")).await;
        let t = s
            .track(
                a,
                &format!("liked {i}"),
                Feat {
                    axis: 0,
                    jitter: i,
                    energy: 0.8,
                    ..Default::default()
                },
            )
            .await;
        s.like(user, t, 10).await;
    }
    let (mut calm_ru, mut calm_en) = (Vec::new(), Vec::new());
    let mut explicit = HashSet::new();
    for i in 0..12 {
        let ru = i % 2 == 0;
        let a = s
            .artist(&if ru {
                format!("Тихий {i}")
            } else {
                format!("Quiet{i}")
            })
            .await;
        for k in 0..3 {
            let t = s
                .track(
                    a,
                    &if ru {
                        format!("дождь {i}/{k}")
                    } else {
                        format!("rain {i}/{k}")
                    },
                    Feat {
                        axis: 7,
                        jitter: i * 3 + k,
                        energy: 0.25,
                        valence: 0.3,
                        bpm: 90.0,
                        ..Default::default()
                    },
                )
                .await;
            if k == 2 {
                sqlx::query("UPDATE tracks SET explicit = true WHERE id = $1")
                    .bind(t)
                    .execute(&app.state.db)
                    .await
                    .unwrap();
                explicit.insert(t);
            }
            if ru {
                calm_ru.push(t);
            } else {
                calm_en.push(t);
            }
        }
    }
    let settings = reco::Settings::load(&app.state.db).await.unwrap();
    taste::rebuild(&app.state.db, user, &settings, false)
        .await
        .unwrap();
    World {
        calm_ru,
        calm_en,
        explicit,
    }
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn describe_builds_filters_more_and_saves(db: PgPool) {
    let fake = FakeMl::start().await;
    let app = TestApp::with(db, |c| c.ml.url = fake.base.clone()).await;
    let me = app.listener("ann").await;
    let user = user_of(&app, &me).await;
    let w = world(&app, user).await;

    // Спокойное русское без мата: только русские треки оси 7, без explicit, от артиста — не больше двух
    let body = preview(
        &app,
        &me,
        "спокойное под дождь, только русское, без мата, 6 треков",
    )
    .await;
    assert_eq!(
        body["title"], "Спокойная русская музыка под дождь",
        "{body}"
    );
    assert_eq!(
        body["description"], "Без нецензурного",
        "число треков — в строке под названием"
    );
    assert_eq!(body["brief"]["language"], "ru");
    assert_eq!(body["brief"]["mood"], "calm");
    assert_eq!(body["brief"]["noExplicit"], true);
    assert!(fake.texts().iter().any(|t| t.contains("спокойное")));
    let items = &body["items"];
    let got = ids(items);
    assert_eq!(got.len(), 6, "{items}");
    let ru: HashSet<String> = w.calm_ru.iter().map(Uuid::to_string).collect();
    assert!(
        got.iter().all(|t| ru.contains(t)),
        "только русское: {got:?}"
    );
    let explicit: HashSet<String> = w.explicit.iter().map(Uuid::to_string).collect();
    assert!(
        got.iter().all(|t| !explicit.contains(t)),
        "без мата: {got:?}"
    );
    assert!(body["durationMs"].as_i64().unwrap() >= 6 * 180_000);
    // Начало, которого нет на сервере, уже качается
    let queued: i64 = sqlx::query_scalar("SELECT count(*) FROM acquisitions WHERE reason = 'wave'")
        .fetch_one(&app.state.db)
        .await
        .unwrap();
    assert!(queued > 0);

    // «Ещё в том же духе»: без показанного
    let res = app
        .post(
            "/api/describe/more",
            json!({ "brief": body["brief"], "exclude": got, "afterTrackId": got.last(), "count": 4 }),
            &me,
        )
        .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    let more = ids(&res.body["items"]);
    assert!(!more.is_empty());
    assert!(
        more.iter().all(|t| !got.contains(t) && ru.contains(t)),
        "{more:?}"
    );

    // «Без …»: артист по имени из каталога (в косвенном падеже)
    let body = preview(&app, &me, "спокойное на английском без Quiet1, 6 треков").await;
    assert_eq!(body["brief"]["without"][0]["name"], "Quiet1");
    let en: HashSet<String> = w.calm_en.iter().map(Uuid::to_string).collect();
    let got_en = ids(&body["items"]);
    assert!(!got_en.is_empty());
    assert!(
        got_en.iter().all(|t| en.contains(t)),
        "{:?} {}",
        artists(&body["items"]),
        body["brief"]
    );
    assert!(
        artists(&body["items"]).iter().all(|a| a != "Quiet1"),
        "{}",
        body["items"]
    );

    // Сохранить: обычный плейлист с понятым
    let res = app
        .post(
            "/api/describe/save",
            json!({ "brief": body["brief"], "title": body["title"], "description": body["description"],
                    "trackIds": got_en, "weekly": false }),
            &me,
        )
        .await;
    assert_eq!(res.status, StatusCode::CREATED, "{}", res.text);
    assert_eq!(res.body["generator"], "describe");
    assert_eq!(res.body["kind"], "user");
    assert_eq!(res.body["tracksCount"], got_en.len());
    let pl = res.body["id"].as_str().unwrap().to_string();
    let res = app.get(&format!("/api/playlists/{pl}/describe"), &me).await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    assert_eq!(res.body["weekly"], false);
    assert_eq!(res.body["brief"]["language"], "foreign");
    let res = app
        .call(
            Method::PATCH,
            &format!("/api/playlists/{pl}/describe"),
            Some(json!({ "weekly": true })),
            &me,
        )
        .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    assert_eq!(res.body["weekly"], true);
    // «Ещё в том же духе» для сохранённого — в конец, без повторов
    let res = app
        .post(
            &format!("/api/playlists/{pl}/describe/more"),
            json!({ "count": 2 }),
            &me,
        )
        .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    let added: Vec<String> = res.body["trackIds"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .collect();
    assert!(!added.is_empty() && added.len() <= 2);
    let detail = app.get(&format!("/api/playlists/{pl}"), &me).await;
    let order: Vec<String> = detail.body["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["track"]["id"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(order.len(), got_en.len() + added.len());
    assert_eq!(&order[got_en.len()..], &added[..]);
    // Обычный плейлист — «не по описанию»
    let plain = app
        .post("/api/playlists", json!({ "title": "Своё" }), &me)
        .await;
    let plain = plain.body["playlist"]["id"].as_str().unwrap().to_string();
    let res = app
        .get(&format!("/api/playlists/{plain}/describe"), &me)
        .await;
    assert_eq!(res.status, StatusCode::NOT_FOUND);
    assert_eq!(res.body["error"]["code"], "not_described");
    // Чужой — нет такого
    let other = app.listener("bob").await;
    let res = app
        .get(&format!("/api/playlists/{pl}/describe"), &other)
        .await;
    assert_eq!(res.status, StatusCode::NOT_FOUND);

    // Неверные запросы
    for (text, status) in [
        ("", StatusCode::BAD_REQUEST),
        ("а", StatusCode::BAD_REQUEST),
    ] {
        let res = app
            .post("/api/describe", json!({ "text": text }), &me)
            .await;
        assert_eq!(res.status, status, "{text}");
    }
    let long = "спокойное ".repeat(40);
    let res = app
        .post("/api/describe", json!({ "text": long }), &me)
        .await;
    assert_eq!(res.status, StatusCode::BAD_REQUEST);
    let res = app
        .post(
            "/api/describe/more",
            json!({ "brief": { "text": "x", "minutes": 100000 } }),
            &me,
        )
        .await;
    assert_eq!(res.status, StatusCode::BAD_REQUEST);
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn genres_references_and_the_fallback(db: PgPool) {
    let fake = FakeMl::start().await;
    let app = TestApp::with(db, |c| c.ml.url = fake.base.clone()).await;
    let me = app.listener("ann").await;
    let user = user_of(&app, &me).await;
    let s = Synth::new(app.state.db.clone());
    // Вкус — на оси 0
    for i in 0..4 {
        let a = s.artist(&format!("Liked {i}")).await;
        let t = s
            .track(
                a,
                &format!("liked {i}"),
                Feat {
                    axis: 0,
                    jitter: i,
                    ..Default::default()
                },
            )
            .await;
        s.like(user, t, 10).await;
    }
    // Инди по стилям Discogs (ось 40) и трэп (ось 80): жанр решают стили, а не звук
    let mut indie = Vec::new();
    for i in 0..10 {
        let a = s.artist(&format!("Indie {i}")).await;
        let t = s
            .track(
                a,
                &format!("indie {i}"),
                Feat {
                    axis: 40,
                    jitter: i,
                    ..Default::default()
                },
            )
            .await;
        indie.push(t);
    }
    sqlx::query(
        "UPDATE track_analysis SET styles = '[{\"name\": \"Rock---Indie Rock\", \"p\": 0.6}]'
         WHERE recording_id IN (SELECT recording_id FROM tracks WHERE id = ANY($1))",
    )
    .bind(&indie)
    .execute(&app.state.db)
    .await
    .unwrap();
    for i in 0..10 {
        let a = s.artist(&format!("Trap {i}")).await;
        s.track(
            a,
            &format!("trap {i}"),
            Feat {
                axis: 80,
                jitter: i,
                ..Default::default()
            },
        )
        .await;
    }
    // Ориентир «Star» на оси 120 и его соседи по звучанию
    let star = s.artist("Star").await;
    for i in 0..3 {
        s.track(
            star,
            &format!("star {i}"),
            Feat {
                axis: 120,
                jitter: i,
                ..Default::default()
            },
        )
        .await;
    }
    let mut near_star = HashSet::new();
    for i in 0..8 {
        let a = s.artist(&format!("Near {i}")).await;
        let t = s
            .track(
                a,
                &format!("near {i}"),
                Feat {
                    axis: 120,
                    jitter: 5 + i,
                    ..Default::default()
                },
            )
            .await;
        near_star.insert(t.to_string());
    }
    let settings = reco::Settings::load(&app.state.db).await.unwrap();
    taste::rebuild(&app.state.db, user, &settings, false)
        .await
        .unwrap();

    let indie_ids: HashSet<String> = indie.iter().map(Uuid::to_string).collect();
    let body = preview(&app, &me, "20 минут инди").await;
    assert_eq!(body["title"], "Инди · 20 мин");
    let got = ids(&body["items"]);
    assert!(got.len() >= 5, "{body}");
    assert!(
        got.iter().all(|t| indie_ids.contains(t)),
        "только инди: {got:?}"
    );
    let total = body["durationMs"].as_i64().unwrap();
    assert!((15 * 60_000..=24 * 60_000).contains(&total), "{total}");

    // «Без инди» — ни одного трека с такими стилями
    let body = preview(&app, &me, "что-нибудь без инди").await;
    assert!(ids(&body["items"]).iter().all(|t| !indie_ids.contains(t)));

    // Ориентир: артист из каталога, рядом с ним по звучанию
    let body = preview(&app, &me, "как Star, 8 треков").await;
    assert_eq!(body["title"], "Как Star");
    assert_eq!(body["description"], "");
    assert_eq!(body["brief"]["like"][0]["id"], star.to_string());
    let got = ids(&body["items"]);
    let close = got.iter().filter(|t| near_star.contains(*t)).count();
    assert!(
        close * 2 >= got.len(),
        "больше половины — соседи Star: {got:?}"
    );
    assert!(
        artists(&body["items"]).iter().all(|a| a != "Star"),
        "сам ориентир не нужен"
    );

    // Сервис анализа лежит — подбор идёт без поиска по звучанию
    fake.fail(503, "down");
    let body = preview(&app, &me, "спокойное инди").await;
    assert!(!ids(&body["items"]).is_empty());
    fake.heal();

    // Пересобрать: прошлые треки не повторяются, пока хватает другого
    let first = preview(&app, &me, "инди").await;
    let first_ids = ids(&first["items"]);
    let res = app
        .post(
            "/api/describe",
            json!({ "text": "инди", "exclude": first_ids }),
            &me,
        )
        .await;
    assert_eq!(res.status, StatusCode::OK);

    // Лимит: не больше 30 сборок за 10 минут на человека
    for _ in 0..40 {
        app.state
            .limiter
            .check_id("describe", user, 30, Duration::from_secs(600));
    }
    let res = app
        .post("/api/describe", json!({ "text": "инди" }), &me)
        .await;
    assert_eq!(res.status, StatusCode::TOO_MANY_REQUESTS);
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn weekly_refresh_swaps_what_was_heard(db: PgPool) {
    let fake = FakeMl::start().await;
    let app = TestApp::with(db, |c| c.ml.url = fake.base.clone()).await;
    let me = app.listener("ann").await;
    let user = user_of(&app, &me).await;
    let w = world(&app, user).await;
    let body = preview(&app, &me, "спокойное, 5 треков").await;
    let first = ids(&body["items"]);
    let res = app
        .post(
            "/api/describe/save",
            json!({ "brief": body["brief"], "title": "Спокойное", "trackIds": first, "weekly": true }),
            &me,
        )
        .await;
    assert_eq!(res.status, StatusCode::CREATED, "{}", res.text);
    let pl: Uuid = res.body["id"].as_str().unwrap().parse().unwrap();
    // Срок не вышел — ничего не меняется
    let env = wave::Env::new(&app.state.db, &app.state.deezer, false)
        .await
        .unwrap();
    let report = describe::refresh_weekly(&env, &app.state.ml).await.unwrap();
    assert_eq!(report["playlists"], 0);
    // Неделя прошла, два трека прослушаны
    sqlx::query(
        "UPDATE playlists SET created_at = now() - interval '8 days',
                rules = jsonb_set(rules, '{refreshedAt}', to_jsonb(now() - interval '8 days'))
         WHERE id = $1",
    )
    .bind(pl)
    .execute(&app.state.db)
    .await
    .unwrap();
    sqlx::query(
        "UPDATE playlist_items SET added_at = now() - interval '8 days' WHERE playlist_id = $1",
    )
    .bind(pl)
    .execute(&app.state.db)
    .await
    .unwrap();
    let s = Synth::new(app.state.db.clone());
    let heard: Vec<Uuid> = first.iter().take(2).map(|t| t.parse().unwrap()).collect();
    for t in &heard {
        s.play(user, *t, 1, true).await;
    }
    let report = describe::refresh_weekly(&env, &app.state.ml).await.unwrap();
    assert_eq!(report["playlists"], 1, "{report}");
    assert_eq!(report["replaced"], 2, "{report}");
    let now: Vec<Uuid> = sqlx::query_scalar(
        "SELECT track_id FROM playlist_items WHERE playlist_id = $1 ORDER BY position",
    )
    .bind(pl)
    .fetch_all(&app.state.db)
    .await
    .unwrap();
    assert_eq!(now.len(), first.len());
    assert!(heard.iter().all(|t| !now.contains(t)), "прослушанное ушло");
    let calm: HashSet<Uuid> = w.calm_ru.iter().chain(&w.calm_en).copied().collect();
    let fresh: Vec<&Uuid> = now
        .iter()
        .filter(|t| !first.contains(&t.to_string()))
        .collect();
    assert_eq!(fresh.len(), 2);
    assert!(
        fresh.iter().all(|t| calm.contains(t)),
        "новое — в том же духе: {now:?}"
    );
    // Сразу второй раз — срок снова неделя
    let report = describe::refresh_weekly(&env, &app.state.ml).await.unwrap();
    assert_eq!(report["playlists"], 0);
}
