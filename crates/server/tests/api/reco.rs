//! Рекомендации против API: волна (кандидаты, отсев, расстановка, причины, докачка, показы), границы
//! настроений и занятий, «отдых» заслушанного, реакции, похожие, поиск по звучанию, анализ звука
//! (клиент ml против заглушки, задачи полного анализа и превью), разведчик с бюджетом и пределом индекса,
//! вкус с миксами на главной и раздел «Рекомендации» админки. Каталог — синтетический (`synth.rs`)
//! или записанные ответы Deezer, сервис анализа — заглушка (`fake_ml.rs`).

use std::{collections::HashSet, time::Duration};

use axum::http::{Method, StatusCode};
use dalywave_server::{
    jobs::{self, worker::Outcome},
    ml::{Ml, MlError, Mode},
    reco::{self, analysis, scout, taste},
};
use serde_json::{Value, json};
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    common::{Auth, TestApp},
    deezer::{CHURCH_ON_SUNDAY, LIGHT_ME_UP, LOWBORN, fixture},
    fake_ml::FakeMl,
    synth::{Feat, Synth},
};

async fn user_of(app: &TestApp, auth: &Auth) -> Uuid {
    app.user_id(auth).await.parse().unwrap()
}

/// Новая сессия — ответ целиком: с подготовленным началом (`queue`).
async fn create(app: &TestApp, auth: &Auth, settings: Value) -> Value {
    let res = app
        .post("/api/wave/sessions", json!({ "settings": settings }), auth)
        .await;
    assert_eq!(res.status, StatusCode::CREATED, "{}", res.text);
    res.body
}

async fn session(app: &TestApp, auth: &Auth, settings: Value) -> String {
    create(app, auth, settings).await["id"]
        .as_str()
        .unwrap()
        .to_string()
}

/// Первые `count` треков сессии так, как их сыграет плеер: подготовленное начало, затем выдача.
async fn opening(app: &TestApp, auth: &Auth, created: &Value, count: usize) -> Vec<Value> {
    let id = created["id"].as_str().unwrap();
    let mut items = created["queue"].as_array().unwrap().clone();
    if items.len() < count {
        items.extend(next(app, auth, id, count - items.len()).await);
    }
    items.truncate(count);
    items
}

async fn next(app: &TestApp, auth: &Auth, id: &str, count: usize) -> Vec<Value> {
    let res = app
        .get(&format!("/api/wave/sessions/{id}/next?count={count}"), auth)
        .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    res.body["items"].as_array().unwrap().clone()
}

fn track_ids(items: &[Value]) -> Vec<String> {
    items
        .iter()
        .map(|i| i["track"]["id"].as_str().unwrap().to_string())
        .collect()
}

fn artist_of(item: &Value) -> String {
    item["track"]["artists"][0]["id"]
        .as_str()
        .unwrap()
        .to_string()
}

async fn rebuild_taste(app: &TestApp, user: Uuid) -> taste::TasteReport {
    let settings = reco::Settings::load(&app.state.db).await.unwrap();
    taste::rebuild(&app.state.db, user, &settings, false)
        .await
        .unwrap()
}

/// Вкус: `n` лайкнутых треков на оси `axis` у двух артистов.
async fn taste_on(s: &Synth, user: Uuid, axis: usize, n: usize) -> Vec<Uuid> {
    let a = s.artist(&format!("Liked A{axis}")).await;
    let b = s.artist(&format!("Liked B{axis}")).await;
    let mut out = Vec::new();
    for i in 0..n {
        let t = s
            .track(
                if i % 2 == 0 { a } else { b },
                &format!("liked {axis}/{i}"),
                Feat {
                    axis,
                    jitter: i,
                    energy: 0.8,
                    ..Default::default()
                },
            )
            .await;
        s.like(user, t, 10).await;
        out.push(t);
    }
    out
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn wave_follows_taste_and_skips_what_it_must(db: PgPool) {
    let app = TestApp::new(db).await;
    let me = app.listener("ann").await;
    let user = user_of(&app, &me).await;
    let s = Synth::new(app.state.db.clone());
    let liked = taste_on(&s, user, 0, 4).await;
    // Рядом со вкусом — незнакомые артисты; далеко — чужой вкус
    let mut fresh = Vec::new();
    for i in 0..12 {
        let a = s.artist(&format!("New {i}")).await;
        fresh.push(
            s.track(
                a,
                &format!("fresh {i}"),
                Feat {
                    axis: 0,
                    jitter: 20 + i,
                    energy: 0.75,
                    ..Default::default()
                },
            )
            .await,
        );
    }
    for i in 0..5 {
        let a = s.artist(&format!("Far {i}")).await;
        s.track(
            a,
            &format!("far {i}"),
            Feat {
                axis: 600,
                jitter: i,
                ..Default::default()
            },
        )
        .await;
    }
    // Отсев: «не рекомендовать» трек и артиста, «не нравится», нет в источниках, только что играл
    let blocked = fresh[0];
    s.block(user, "track", blocked).await;
    let disliked = fresh[1];
    s.dislike(user, disliked).await;
    let rested = fresh[2];
    s.play(user, rested, 1, true).await;
    let gone_artist = s.artist("Gone").await;
    let gone = s
        .track(
            gone_artist,
            "gone",
            Feat {
                jitter: 50,
                status: "unavailable",
                ..Default::default()
            },
        )
        .await;
    let banned = s.artist("Banned").await;
    let by_banned = s
        .track(
            banned,
            "banned",
            Feat {
                jitter: 51,
                ..Default::default()
            },
        )
        .await;
    s.block(user, "artist", banned).await;

    let report = rebuild_taste(&app, user).await;
    assert_eq!(report.tracks, 5, "четыре лайка и дослушанный");
    assert!(report.facets >= 1);

    let created = create(&app, &me, json!({})).await;
    let id = created["id"].as_str().unwrap().to_string();
    let res = app.get(&format!("/api/wave/sessions/{id}"), &me).await;
    assert_eq!(res.body["title"], "Моя волна");
    assert_eq!(res.body["settings"]["character"], "mix");
    let items = opening(&app, &me, &created, 10).await;
    assert_eq!(items.len(), 10);
    let ids = track_ids(&items);
    for bad in [blocked, disliked, rested, gone, by_banned] {
        assert!(
            !ids.contains(&bad.to_string()),
            "{bad} не должен попасть в волну"
        );
    }
    for w in items.windows(2) {
        assert_ne!(artist_of(&w[0]), artist_of(&w[1]), "артист подряд");
    }
    let reasons: Vec<&str> = items
        .iter()
        .map(|i| i["reason"]["text"].as_str().unwrap())
        .collect();
    assert!(reasons.contains(&"Из любимого"), "{reasons:?}");
    assert!(
        reasons
            .iter()
            .any(|r| r.starts_with("Похоже на Liked") || *r == "Новое для тебя"),
        "{reasons:?}"
    );
    assert!(items.iter().all(|i| i["reason"]["kind"].is_string()));
    assert_eq!(items[0]["keyCamelot"], "8A");
    assert!(items[0]["bpm"].is_number() && items[0]["energy"].is_number());
    // Любимое — в выдаче, но и незнакомое близкое тоже (знакомое и новое)
    assert!(liked.iter().any(|t| ids.contains(&t.to_string())));
    assert!(fresh.iter().any(|t| ids.contains(&t.to_string())));

    // Каждый показ записан с причиной и оценкой
    let shown: Vec<(String, Option<f32>, Value, i32, String)> = sqlx::query_as(
        "SELECT surface, score, reason, position, model_version FROM recommendation_impressions
         WHERE wave_session_id = $1::uuid ORDER BY position",
    )
    .bind(&id)
    .fetch_all(&app.state.db)
    .await
    .unwrap();
    assert_eq!(shown.len(), 10);
    assert_eq!(shown[0].0, "wave");
    assert!(shown[0].1.is_some());
    assert_eq!(shown[0].2["text"], items[0]["reason"]["text"]);
    assert_eq!(
        shown.iter().map(|s| s.3).collect::<Vec<_>>(),
        (0..10).collect::<Vec<_>>()
    );
    assert_eq!(shown[0].4, "rules-1");

    // Докачка: треки, которых нет на сервере, — в полосе «Волна» (качать некому — начало не ждёт):
    // начало (6) и следующая выдача (4)
    let prefetched: Vec<(String, i16)> = sqlx::query_as(
        "SELECT a.reason, j.priority FROM acquisitions a JOIN jobs j ON j.id = a.job_id
         WHERE j.lane = 'wave' ORDER BY j.priority DESC",
    )
    .fetch_all(&app.state.db)
    .await
    .unwrap();
    assert_eq!(prefetched.len(), 10);
    assert!(prefetched.iter().all(|(r, _)| r == "wave"));
    assert_eq!(prefetched[0].1, 8);

    // Следующая выдача не повторяет выданное
    let more = next(&app, &me, &id, 10).await;
    assert!(!more.is_empty());
    let first: HashSet<String> = ids.into_iter().collect();
    assert!(track_ids(&more).iter().all(|t| !first.contains(t)));
    let served: i64 = sqlx::query_scalar(
        "SELECT (state ->> 'step')::bigint FROM wave_sessions WHERE id = $1::uuid",
    )
    .bind(&id)
    .fetch_one(&app.state.db)
    .await
    .unwrap();
    assert_eq!(served as usize, 10 + more.len());

    // Чужая сессия не видна, count проверяется
    let other = app.listener("bob").await;
    let res = app
        .get(&format!("/api/wave/sessions/{id}/next"), &other)
        .await;
    assert_eq!(res.status, StatusCode::NOT_FOUND);
    let res = app
        .get(&format!("/api/wave/sessions/{id}/next?count=31"), &me)
        .await;
    assert_eq!(res.status, StatusCode::BAD_REQUEST);
    let res = app
        .get(&format!("/api/wave/sessions/{id}/next"), &Auth::None)
        .await;
    assert_eq!(res.status, StatusCode::UNAUTHORIZED);
}

/// Сетка признаков: энергия 0,05…0,95, позитивность 0,15 / 0,5 / 0,85, темп 85 / 170 / 100 / 128 / 60 / 140,
/// голос — у нечётных. У каждого трека свой артист.
async fn grid(s: &Synth) -> Vec<Uuid> {
    let bpms = [85.0, 170.0, 100.0, 128.0, 60.0, 140.0];
    let mut out = Vec::new();
    for i in 0..60usize {
        let a = s.artist(&format!("Grid {i}")).await;
        out.push(
            s.track(
                a,
                &format!("grid {i}"),
                Feat {
                    axis: 0,
                    jitter: 30 + i,
                    energy: 0.05 + 0.1 * (i % 10) as f32,
                    valence: 0.15 + 0.35 * ((i / 10) % 3) as f32,
                    bpm: bpms[(i / 10 + i) % 6],
                    voice: if i % 2 == 0 { 0.1 } else { 0.9 },
                    ..Default::default()
                },
            )
            .await,
        );
    }
    out
}

/// Энергия, позитивность, темп, голос.
type Features = (f32, f32, f32, f32);

async fn features(app: &TestApp, items: &[Value]) -> Vec<Features> {
    let mut out = Vec::new();
    for t in track_ids(items) {
        let row: Features = sqlx::query_as(
            "SELECT a.energy, a.valence, a.bpm, a.voice FROM tracks t
             JOIN track_analysis a ON a.recording_id = t.recording_id WHERE t.id = $1::uuid",
        )
        .bind(t)
        .fetch_one(&app.state.db)
        .await
        .unwrap();
        out.push(row);
    }
    out
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn moods_and_activities_bound_the_wave(db: PgPool) {
    let app = TestApp::new(db).await;
    let me = app.listener("ann").await;
    let user = user_of(&app, &me).await;
    let s = Synth::new(app.state.db.clone());
    taste_on(&s, user, 0, 3).await;
    grid(&s).await;
    rebuild_taste(&app, user).await;

    let check = |name: &str, f: &[Features], ok: &dyn Fn(&Features) -> bool| {
        assert!(!f.is_empty(), "{name}: пусто");
        for x in f {
            assert!(ok(x), "{name}: {x:?} вне границ");
        }
    };
    let run = |settings: Value, count: usize| {
        let app = &app;
        let me = &me;
        async move {
            let created = create(app, me, settings).await;
            let items = opening(app, me, &created, count).await;
            features(app, &items).await
        }
    };
    let f = run(json!({ "mood": "calm" }), 8).await;
    check("calm", &f, &|x| x.0 <= 0.5 + 1e-4);
    let f = run(json!({ "mood": "energetic" }), 8).await;
    check("energetic", &f, &|x| x.0 >= 0.6 - 1e-4);
    let f = run(json!({ "mood": "happy" }), 8).await;
    check("happy", &f, &|x| x.1 >= 0.55 - 1e-4);
    let f = run(json!({ "mood": "sad" }), 8).await;
    check("sad", &f, &|x| x.1 <= 0.5 + 1e-4);
    let f = run(json!({ "language": "none" }), 8).await;
    check("без слов", &f, &|x| x.3 <= 0.4);
    // «Тренируюсь» в темп бега 170: темп трека — шаг или полшага
    let f = run(json!({ "activity": "sport", "tempo": 170 }), 4).await;
    check("sport", &f, &|x| {
        x.0 >= 0.6 - 1e-4
            && [85.0f32, 170.0]
                .iter()
                .any(|t| (x.2 / t - 1.0).abs() <= 0.06)
    });
    // «Засыпаю»: спокойно, медленно и всё тише. Цель энергии падает на 0,02 за трек, а у сетки шаг 0,1:
    // на шести треках это один шаг сетки, и случайная добавка оценки с переходами по темпу изредка
    // переворачивали половины. На десяти — почти два шага, ход виден при любом зерне сессии
    let f = run(json!({ "activity": "sleep" }), 10).await;
    check("sleep", &f, &|x| x.0 <= 0.5 + 1e-4 && x.2 <= 115.0);
    let half = f.len() / 2;
    let mean = |s: &[Features]| s.iter().map(|x| x.0).sum::<f32>() / s.len() as f32;
    assert!(
        mean(&f[..half]) >= mean(&f[half..]),
        "энергия снижается: {f:?}"
    );
    // «Просыпаюсь» — наоборот
    let f = run(json!({ "activity": "wake" }), 10).await;
    let half = f.len() / 2;
    assert!(
        mean(&f[..half]) <= mean(&f[half..]),
        "энергия растёт: {f:?}"
    );
    // Неверный темп
    let res = app
        .post(
            "/api/wave/sessions",
            json!({ "settings": { "activity": "sport", "tempo": 400 } }),
            &me,
        )
        .await;
    assert_eq!(res.status, StatusCode::BAD_REQUEST);
    let res = app
        .post(
            "/api/wave/sessions",
            json!({ "settings": { "mood": "furious" } }),
            &me,
        )
        .await;
    assert_eq!(res.status, StatusCode::BAD_REQUEST);
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn overplayed_tracks_rest_and_return(db: PgPool) {
    let app = TestApp::new(db).await;
    let me = app.listener("ann").await;
    let user = user_of(&app, &me).await;
    let s = Synth::new(app.state.db.clone());
    // У каждого трека свой артист: расстановке есть из чего чередовать
    let mut liked = Vec::new();
    for i in 0..6 {
        let a = s.artist(&format!("Fav {i}")).await;
        let t = s
            .track(
                a,
                &format!("fav {i}"),
                Feat {
                    jitter: i,
                    ..Default::default()
                },
            )
            .await;
        s.like(user, t, 10).await;
        liked.push(t);
    }
    // Первый — сыгран раз 7 часов назад: отдохнул (6 часов). Второй — четыре раза за сутки: отдыхает двое суток
    s.play(user, liked[0], 7, true).await;
    for h in [5, 10, 15, 20] {
        s.play(user, liked[1], h, true).await;
    }
    rebuild_taste(&app, user).await;
    let created = create(&app, &me, json!({ "character": "fav" })).await;
    let ids = track_ids(&opening(&app, &me, &created, 10).await);
    assert!(ids.contains(&liked[0].to_string()), "отдохнувший вернулся");
    assert!(!ids.contains(&liked[1].to_string()), "заслушанный отдыхает");
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn reactions_move_the_session(db: PgPool) {
    let app = TestApp::new(db).await;
    let me = app.listener("ann").await;
    let user = user_of(&app, &me).await;
    let s = Synth::new(app.state.db.clone());
    taste_on(&s, user, 0, 3).await;
    for i in 0..10 {
        let a = s.artist(&format!("Near {i}")).await;
        s.track(
            a,
            &format!("near {i}"),
            Feat {
                jitter: 40 + i,
                ..Default::default()
            },
        )
        .await;
        let b = s.artist(&format!("Other {i}")).await;
        s.track(
            b,
            &format!("other {i}"),
            Feat {
                axis: 600,
                jitter: i,
                ..Default::default()
            },
        )
        .await;
    }
    rebuild_taste(&app, user).await;
    let id = session(&app, &me, json!({ "character": "new" })).await;
    let items = next(&app, &me, &id, 6).await;
    let ids = track_ids(&items);
    let feedback = |track: &str, kind: &str, position: i64| {
        let uri = format!("/api/wave/sessions/{id}/feedback");
        let body = json!({ "trackId": track, "kind": kind, "positionMs": position });
        let app = &app;
        let me = &me;
        async move { app.post(&uri, body, me).await }
    };

    // Лайк: в медиатеке, событие с сессией, исход показа, «заходит» в сессии
    let res = feedback(&ids[0], "like", 90_000).await;
    assert_eq!(res.status, StatusCode::NO_CONTENT, "{}", res.text);
    let liked: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM liked_tracks WHERE user_id = $1 AND track_id = $2::uuid)",
    )
    .bind(user)
    .bind(&ids[0])
    .fetch_one(&app.state.db)
    .await
    .unwrap();
    assert!(liked);
    let (kind, context, sid): (String, String, Uuid) = sqlx::query_as(
        "SELECT kind, context_type, wave_session_id FROM feedback_events WHERE track_id = $1::uuid",
    )
    .bind(&ids[0])
    .fetch_one(&app.state.db)
    .await
    .unwrap();
    assert_eq!((kind.as_str(), context.as_str()), ("like", "wave"));
    assert_eq!(sid.to_string(), id);
    let outcome: Option<String> = sqlx::query_scalar(
        "SELECT outcome FROM recommendation_impressions WHERE wave_session_id = $1::uuid AND track_id = $2::uuid",
    )
    .bind(&id)
    .bind(&ids[0])
    .fetch_one(&app.state.db)
    .await
    .unwrap();
    assert_eq!(outcome.as_deref(), Some("like"));

    // Ранний пропуск: «пропускается», серия пропусков, артист отдыхает
    let res = feedback(&ids[1], "skip", 4_000).await;
    assert_eq!(res.status, StatusCode::NO_CONTENT);
    let (hits, misses, streak, rested): (bool, bool, i64, i64) = sqlx::query_as(
        "SELECT hits IS NOT NULL, misses IS NOT NULL, (state ->> 'skipStreak')::bigint,
                jsonb_array_length(state -> 'restedArtists')::bigint
         FROM wave_sessions WHERE id = $1::uuid",
    )
    .bind(&id)
    .fetch_one(&app.state.db)
    .await
    .unwrap();
    assert!(hits && misses);
    assert_eq!(streak, 1);
    assert_eq!(rested, 1);
    // «Не нравится» — и в новой сессии трека нет; «не сейчас» — без штрафа
    let res = feedback(&ids[2], "dislike", 1_000).await;
    assert_eq!(res.status, StatusCode::NO_CONTENT);
    let res = feedback(&ids[3], "not_now", 0).await;
    assert_eq!(res.status, StatusCode::NO_CONTENT);
    let res = feedback(&ids[4], "completed", 180_000).await;
    assert_eq!(res.status, StatusCode::NO_CONTENT);
    let streak: i64 = sqlx::query_scalar(
        "SELECT (state ->> 'skipStreak')::bigint FROM wave_sessions WHERE id = $1::uuid",
    )
    .bind(&id)
    .fetch_one(&app.state.db)
    .await
    .unwrap();
    assert_eq!(streak, 0, "дослушанный трек прерывает серию пропусков");
    let fresh = session(&app, &me, json!({ "character": "new" })).await;
    let again = track_ids(&next(&app, &me, &fresh, 20).await);
    assert!(
        !again.contains(&ids[2]),
        "не нравится — больше не рекомендуется"
    );
    assert!(
        again.contains(&ids[3]) || again.len() < 20,
        "«не сейчас» трек не наказывает"
    );

    // Ошибки
    let res = feedback(&Uuid::now_v7().to_string(), "like", 0).await;
    assert_eq!(res.status, StatusCode::NOT_FOUND);
    let res = feedback(&ids[5], "love", 0).await;
    assert_eq!(res.status, StatusCode::BAD_REQUEST);
    let res = app
        .post(
            &format!("/api/wave/sessions/{}/feedback", Uuid::now_v7()),
            json!({ "trackId": ids[5], "kind": "skip" }),
            &me,
        )
        .await;
    assert_eq!(res.status, StatusCode::NOT_FOUND);
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn radio_similar_and_sound_search(db: PgPool) {
    let fake = FakeMl::start().await;
    let app = TestApp::with(db, |c| c.ml.url = fake.base.clone()).await;
    let me = app.listener("ann").await;
    let user = user_of(&app, &me).await;
    let s = Synth::new(app.state.db.clone());
    let liked = taste_on(&s, user, 0, 3).await;
    let star = s.artist("Star").await;
    let mut calm = Vec::new();
    for i in 0..8 {
        let a = s.artist(&format!("Calm {i}")).await;
        // Ось 7: «спокойное» у заглушки ml попадает сюда по смыслу (CLAP)
        calm.push(
            s.track(
                a,
                &format!("calm {i}"),
                Feat {
                    axis: 7,
                    jitter: i,
                    energy: 0.2,
                    ..Default::default()
                },
            )
            .await,
        );
    }
    let star_tracks: Vec<Uuid> = {
        let mut v = Vec::new();
        for i in 0..3 {
            v.push(
                s.track(
                    star,
                    &format!("star {i}"),
                    Feat {
                        axis: 7,
                        jitter: 20 + i,
                        ..Default::default()
                    },
                )
                .await,
            );
        }
        v
    };
    rebuild_taste(&app, user).await;

    // Радио от трека: соседи затравки, подпись — по артисту затравки
    let res = app
        .post(
            "/api/wave/sessions",
            json!({ "settings": { "seed": { "type": "track", "id": calm[0] } } }),
            &me,
        )
        .await;
    assert_eq!(res.status, StatusCode::CREATED, "{}", res.text);
    assert_eq!(res.body["title"], "Радио: calm 0");
    let radio = res.body["id"].as_str().unwrap().to_string();
    let items = opening(&app, &me, &res.body, 6).await;
    assert!(
        items.iter().all(|i| i["reason"]["kind"] == "seed"),
        "{items:?}"
    );
    assert!(
        items
            .iter()
            .any(|i| i["reason"]["text"] == "Похоже на Calm 0")
    );
    let surface: String = sqlx::query_scalar(
        "SELECT DISTINCT surface FROM recommendation_impressions WHERE wave_session_id = $1::uuid",
    )
    .bind(&radio)
    .fetch_one(&app.state.db)
    .await
    .unwrap();
    assert_eq!(surface, "radio");
    // Радио артиста: его треки — «Ещё Star»
    let res = app
        .post(
            "/api/wave/sessions",
            json!({ "settings": { "seed": { "type": "artist", "id": star } } }),
            &me,
        )
        .await;
    assert_eq!(res.status, StatusCode::CREATED, "{}", res.text);
    let items = opening(&app, &me, &res.body, 6).await;
    assert!(
        items.iter().any(|i| i["reason"]["text"] == "Ещё Star"),
        "{items:?}"
    );
    // По описанию словами: вектор CLAP от ml
    let res = app
        .post(
            "/api/wave/sessions",
            json!({ "settings": { "seed": { "type": "text", "text": "спокойный вечер" } } }),
            &me,
        )
        .await;
    assert_eq!(res.status, StatusCode::CREATED, "{}", res.text);
    assert_eq!(res.body["title"], "По запросу «спокойный вечер»");
    let items = opening(&app, &me, &res.body, 5).await;
    let calm_ids: HashSet<String> = calm
        .iter()
        .chain(&star_tracks)
        .map(|t| t.to_string())
        .collect();
    assert!(
        track_ids(&items).iter().all(|t| calm_ids.contains(t)),
        "{items:?}"
    );
    assert_eq!(items[0]["reason"]["text"], "По запросу «спокойный вечер»");
    // Затравка без id или текста, несуществующая
    for (seed, status) in [
        (json!({ "type": "artist" }), StatusCode::BAD_REQUEST),
        (
            json!({ "type": "text", "text": " " }),
            StatusCode::BAD_REQUEST,
        ),
        (
            json!({ "type": "album", "id": Uuid::now_v7() }),
            StatusCode::NOT_FOUND,
        ),
        (
            json!({ "type": "playlist", "id": Uuid::now_v7() }),
            StatusCode::NOT_FOUND,
        ),
    ] {
        let res = app
            .post(
                "/api/wave/sessions",
                json!({ "settings": { "seed": seed } }),
                &me,
            )
            .await;
        assert_eq!(res.status, status, "{seed}: {}", res.text);
    }

    // Похожие: соседи по звучанию, без самого трека, не больше двух у артиста
    let res = app
        .get(&format!("/api/tracks/{}/similar?limit=5", calm[0]), &me)
        .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    let items = res.body["items"].as_array().unwrap();
    assert_eq!(items.len(), 5);
    assert!(!track_ids(items).contains(&calm[0].to_string()));
    assert!(track_ids(items).iter().all(|t| calm_ids.contains(t)));
    assert_eq!(items[0]["reason"]["text"], "Похоже на «calm 0»");
    let similar: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM recommendation_impressions WHERE surface = 'similar'",
    )
    .fetch_one(&app.state.db)
    .await
    .unwrap();
    assert_eq!(similar, 5);
    // Неразобранный трек: превью — на анализ вне очереди
    let raw = s
        .track(
            star,
            "raw",
            Feat {
                analyzed: false,
                ..Default::default()
            },
        )
        .await;
    let res = app.get(&format!("/api/tracks/{raw}/similar"), &me).await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    let priority: i16 = sqlx::query_scalar(
        "SELECT priority FROM jobs WHERE kind = 'analyze_preview' AND payload ->> 'trackId' = $1",
    )
    .bind(raw.to_string())
    .fetch_one(&app.state.db)
    .await
    .unwrap();
    assert_eq!(priority, analysis::ON_DEMAND_PRIORITY);
    let res = app
        .get(&format!("/api/tracks/{}/similar", Uuid::now_v7()), &me)
        .await;
    assert_eq!(res.status, StatusCode::NOT_FOUND);

    // Поиск словами по звучанию: медиатека (лайки) и каталог — отдельно
    let res = app
        .post(
            "/api/search/sound",
            json!({ "text": "спокойный инструментал", "limit": 4 }),
            &me,
        )
        .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    assert_eq!(res.body["query"], "calm music");
    assert_eq!(res.body["translated"], true);
    let ids_of = |list: &Value| -> Vec<String> {
        list.as_array()
            .unwrap()
            .iter()
            .map(|t| t["id"].as_str().unwrap().to_string())
            .collect()
    };
    let catalog = ids_of(&res.body["catalog"]);
    assert_eq!(catalog.len(), 4);
    assert!(catalog.iter().all(|t| calm_ids.contains(t)));
    let res = app
        .post("/api/search/sound", json!({ "text": "громкий рок" }), &me)
        .await;
    let library = ids_of(&res.body["library"]);
    assert_eq!(library.len(), 3, "лайки — в медиатеке");
    assert!(
        library
            .iter()
            .all(|t| liked.iter().any(|l| l.to_string() == *t))
    );
    assert_eq!(fake.texts().len(), 3);
    let res = app
        .post("/api/search/sound", json!({ "text": "" }), &me)
        .await;
    assert_eq!(res.status, StatusCode::BAD_REQUEST);
    // Сервис анализа лёг — 503 с понятным текстом
    fake.fail(503, "models_missing");
    let res = app
        .post("/api/search/sound", json!({ "text": "дождь" }), &me)
        .await;
    assert_eq!(res.status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(res.code(), "unavailable");
}

#[tokio::test]
async fn ml_client_waits_out_busy_and_sorts_errors() {
    let fake = FakeMl::start().await;
    let ml = Ml::new(&fake.base)
        .unwrap()
        .with_backoff(Duration::from_millis(5), Duration::from_secs(5));
    fake.busy(2);
    let a = ml
        .analyze("https://cdnt-preview.dzcdn.net/x.mp3", Mode::Preview)
        .await
        .unwrap();
    assert_eq!(fake.calls().len(), 3, "два «занят» и ответ");
    assert_eq!(fake.calls()[0]["mode"], "preview");
    assert_eq!(a.source, "preview");
    let sound = a.sound_vector().unwrap().unwrap();
    assert_eq!(sound.len(), 1280);
    assert_eq!(sound.iter().filter(|x| **x == 1.0).count(), 2);
    assert_eq!(a.semantic_vector().unwrap().unwrap().len(), 512);
    assert_eq!(a.key_camelot.as_deref(), Some("8A"));
    // Занят дольше, чем клиент ждёт
    let impatient = Ml::new(&fake.base)
        .unwrap()
        .with_backoff(Duration::from_millis(5), Duration::from_millis(30));
    fake.busy(1000);
    let e = impatient
        .analyze("https://x/y.mp3", Mode::Full)
        .await
        .unwrap_err();
    assert!(matches!(e, MlError::Busy), "{e}");
    assert!(e.is_service());
    fake.busy(0);
    fake.fail(422, "undecodable");
    let e = ml.analyze("https://x/y.mp3", Mode::Full).await.unwrap_err();
    assert!(
        matches!(e, MlError::Api { status: 422, ref code, .. } if code == "undecodable"),
        "{e}"
    );
    assert!(!e.is_temporary() && !e.is_service());
    fake.fail(502, "download_failed");
    let e = ml.analyze("https://x/y.mp3", Mode::Full).await.unwrap_err();
    assert!(e.is_temporary() && !e.is_service());
    fake.heal();
    let t = ml.embed_text("спокойный вечер").await.unwrap();
    assert_eq!(t.vector().unwrap().len(), 512);
    assert_eq!(t.query, "calm music");
    assert!(ml.health().await.unwrap()["ok"].as_bool().unwrap());
    let down = Ml::new("http://127.0.0.1:9").unwrap();
    let e = down
        .analyze("https://x/y.mp3", Mode::Preview)
        .await
        .unwrap_err();
    assert!(matches!(e, MlError::Unreachable(_)) && e.is_service());
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn analysis_jobs_fill_the_index(db: PgPool) {
    let fake = FakeMl::start().await;
    let app = TestApp::with(db, |c| c.ml.url = fake.base.clone()).await;
    let me = app.listener("ann").await;
    // Пауза между задачами полосы «Разведка» в тестах не нужна
    sqlx::query("UPDATE job_lanes SET min_interval_ms = 0")
        .execute(&app.state.db)
        .await
        .unwrap();
    let track: Uuid = app
        .resolve_id("track", LIGHT_ME_UP, &me)
        .await
        .parse()
        .unwrap();
    let mut conn = app.state.db.acquire().await.unwrap();
    let job = analysis::request_preview(&mut conn, track, 0)
        .await
        .unwrap();
    assert!(job.created);
    assert!(
        !analysis::request_preview(&mut conn, track, 0)
            .await
            .unwrap()
            .created
    );
    drop(conn);

    // Превью: свежая ссылка у Deezer, анализ, строка индекса
    let (kind, outcome) = app.run_job("scout").await.unwrap();
    assert_eq!((kind.as_str(), outcome), ("analyze_preview", Outcome::Done));
    let url = fake.calls()[0]["url"].as_str().unwrap().to_string();
    assert!(url.starts_with("https://cdnt-preview.dzcdn.net/"), "{url}");
    type IndexRow = (
        String,
        Option<f32>,
        Option<String>,
        Option<f32>,
        i32,
        Option<Vec<i32>>,
    );
    let row: IndexRow = sqlx::query_as(
        "SELECT a.source, a.bpm, a.key_camelot, a.approachability, vector_dims(a.sound), a.beats_ms
         FROM tracks t JOIN track_analysis a ON a.recording_id = t.recording_id WHERE t.id = $1",
    )
    .bind(track)
    .fetch_one(&app.state.db)
    .await
    .unwrap();
    assert_eq!(row.0, "preview");
    assert_eq!(row.1, Some(128.0));
    assert_eq!(row.2.as_deref(), Some("8A"));
    assert_eq!(row.3, Some(0.6));
    assert_eq!(row.4, 1280);
    assert!(row.5.is_none(), "у превью нет сетки долей");

    // Запись скачана: полный анализ по подписанной ссылке заменяет превью
    let recording: Uuid = sqlx::query_scalar("SELECT recording_id FROM tracks WHERE id = $1")
        .bind(track)
        .fetch_one(&app.state.db)
        .await
        .unwrap();
    let file: Uuid = sqlx::query_scalar(
        "INSERT INTO audio_files (recording_id, storage_key, codec, container, duration_ms, size_bytes, source)
         VALUES ($1, 'audio/x/y.m4a', 'opus', 'mp4', 180000, 1000, 'youtube_music') RETURNING id",
    )
    .bind(recording)
    .fetch_one(&app.state.db)
    .await
    .unwrap();
    sqlx::query("UPDATE recordings SET primary_file_id = $2, audio_status = 'ready' WHERE id = $1")
        .bind(recording)
        .bind(file)
        .execute(&app.state.db)
        .await
        .unwrap();
    assert_eq!(
        analysis::schedule_backfill(&app.state.db, 10)
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        analysis::schedule_backfill(&app.state.db, 10)
            .await
            .unwrap(),
        0,
        "уже стоит"
    );
    let (kind, outcome) = app.run_job("analysis").await.unwrap();
    assert_eq!((kind.as_str(), outcome), ("analyze_full", Outcome::Done));
    let full_url = fake.calls().last().unwrap()["url"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(
        full_url.contains("audio/x/y.m4a") && full_url.contains("X-Amz-Expires=3600"),
        "{full_url}"
    );
    let (source, file_id, beats): (String, Option<Uuid>, Option<Vec<i32>>) = sqlx::query_as(
        "SELECT source, file_id, beats_ms FROM track_analysis WHERE recording_id = $1",
    )
    .bind(recording)
    .fetch_one(&app.state.db)
    .await
    .unwrap();
    assert_eq!(source, "full");
    assert_eq!(file_id, Some(file));
    assert_eq!(beats, Some(vec![20, 490, 960]));
    // Превью полный анализ не перезаписывает; тот же файл повторно не анализируется
    let mut conn = app.state.db.acquire().await.unwrap();
    let stored = analysis::store(
        &mut conn,
        recording,
        analysis::Source::Preview,
        &analysis::AnalysisRow::default(),
        None,
    )
    .await
    .unwrap();
    assert!(!stored);
    analysis::request_full(&mut conn, recording).await.unwrap();
    drop(conn);
    let calls = fake.calls().len();
    let (_, outcome) = app.run_job("analysis").await.unwrap();
    assert_eq!(outcome, Outcome::Done);
    assert_eq!(
        fake.calls().len(),
        calls,
        "свежий анализ того же файла не повторяется"
    );

    // Сервис занят дольше ожидания: задача отложена, попытка не потрачена
    let other: Uuid = app
        .resolve_id("track", CHURCH_ON_SUNDAY, &me)
        .await
        .parse()
        .unwrap();
    let mut conn = app.state.db.acquire().await.unwrap();
    let job = analysis::request_preview(&mut conn, other, 0)
        .await
        .unwrap();
    drop(conn);
    fake.busy(1000);
    let worker = app.worker_with_ml(
        Ml::new(&fake.base)
            .unwrap()
            .with_backoff(Duration::from_millis(5), Duration::from_millis(30)),
    );
    let (_, outcome) = worker.run_once("scout").await.unwrap().unwrap();
    assert!(
        matches!(outcome, Outcome::Retry(ref m) if m.contains("занят")),
        "{outcome:?}"
    );
    let row = jobs::get(&app.state.db, job.id).await.unwrap().unwrap();
    assert_eq!((row.status.as_str(), row.attempts), ("queued", 0));
    assert!(row.run_after > chrono::Utc::now());
    // …а пока сервис лежит, этот воркер анализы не берёт
    sqlx::query("UPDATE jobs SET run_after = now() WHERE id = $1")
        .bind(job.id)
        .execute(&app.state.db)
        .await
        .unwrap();
    assert!(worker.run_once("scout").await.unwrap().is_none());
    // Файл не разобрать — окончательно
    fake.busy(0);
    fake.fail(422, "undecodable");
    let (_, outcome) = app.run_job("scout").await.unwrap();
    assert!(
        matches!(outcome, Outcome::Failed(ref m) if m.contains("undecodable")),
        "{outcome:?}"
    );
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn scout_grows_the_index_within_budget(db: PgPool) {
    let fake = FakeMl::start().await;
    let app = TestApp::with(db, |c| c.ml.url = fake.base.clone()).await;
    let me = app.listener("ann").await;
    sqlx::query("UPDATE job_lanes SET min_interval_ms = 0")
        .execute(&app.state.db)
        .await
        .unwrap();
    sqlx::query("UPDATE reco_settings SET tracks_per_day = 12, max_hops = 1")
        .execute(&app.state.db)
        .await
        .unwrap();
    // Семя — подписка на Lowborn
    let artist = app.resolve_id("artist", LOWBORN, &me).await;
    let res = app
        .call(
            Method::PUT,
            &format!("/api/library/artists/{artist}"),
            None,
            &me,
        )
        .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);

    let settings = reco::Settings::load(&app.state.db).await.unwrap();
    let seeds = scout::seeds(&app.state.db).await.unwrap();
    assert_eq!(seeds, vec![(artist.parse().unwrap(), 1.0)]);

    // Первый проход: окрестность (семя), разведка артиста, чарты; кандидатов ещё нет
    let mut conn = app.state.db.acquire().await.unwrap();
    jobs::enqueue(
        &mut conn,
        jobs::NewJob::new("scout", "scout", json!({})).dedupe("scout"),
    )
    .await
    .unwrap();
    drop(conn);
    let (kind, outcome) = app.run_job("scout").await.unwrap();
    assert_eq!((kind.as_str(), outcome), ("scout", Outcome::Done));
    let result: Value = sqlx::query_scalar("SELECT result FROM jobs WHERE kind = 'scout'")
        .fetch_one(&app.state.db)
        .await
        .unwrap();
    assert_eq!(result["seeds"], 1);
    assert_eq!(result["queued"], 0);
    // Разведка артиста: похожие (шаг 1 разрешён), популярное, релизы — из записанных ответов Deezer
    let (kind, outcome) = app.run_job("scout").await.unwrap();
    assert_eq!(
        (kind.as_str(), outcome),
        ("scout_artist", Outcome::Done),
        "сначала разведка, потом чарты"
    );
    let top: i64 = sqlx::query_scalar("SELECT count(*) FROM scout_tracks WHERE source = 'top'")
        .fetch_one(&app.state.db)
        .await
        .unwrap();
    assert_eq!(top, 10);
    let nodes = scout::neighbourhood(&app.state.db, &settings)
        .await
        .unwrap();
    assert_eq!(nodes.len(), 21, "семя и двадцать похожих на шаг");
    assert!(
        nodes
            .values()
            .filter(|n| n.hops == 1)
            .all(|n| n.weight <= 0.5)
    );

    // Анализ превью — не больше шестой части суточного бюджета за проход
    let queued = scout::enqueue_previews(&app.state.db, &settings)
        .await
        .unwrap();
    assert_eq!(queued, 2);
    let priorities: Vec<i16> = sqlx::query_scalar(
        "SELECT priority FROM jobs WHERE kind = 'analyze_preview' ORDER BY priority DESC",
    )
    .fetch_all(&app.state.db)
    .await
    .unwrap();
    assert_eq!(
        priorities,
        vec![10, 10],
        "популярное семени — самое близкое"
    );
    for _ in 0..5 {
        scout::enqueue_previews(&app.state.db, &settings)
            .await
            .unwrap();
    }
    let total: i64 = sqlx::query_scalar("SELECT count(*) FROM jobs WHERE kind = 'analyze_preview'")
        .fetch_one(&app.state.db)
        .await
        .unwrap();
    assert_eq!(total, 10, "в записанном популярном — 10 треков");
    assert_eq!(
        scout::enqueue_previews(&app.state.db, &settings)
            .await
            .unwrap(),
        0
    );
    // Сутки исчерпаны: даже новые находки ждут
    sqlx::query("UPDATE scout_tracks SET queued_at = NULL")
        .execute(&app.state.db)
        .await
        .unwrap();
    sqlx::query("UPDATE reco_settings SET tracks_per_day = 10")
        .execute(&app.state.db)
        .await
        .unwrap();
    let tight = reco::Settings::load(&app.state.db).await.unwrap();
    assert_eq!(
        scout::enqueue_previews(&app.state.db, &tight)
            .await
            .unwrap(),
        0
    );

    // Анализ превью идёт в индекс; предел индекса вытесняет дальних, а медиатеку не трогает
    // Трек целиком (с ISRC и свежим превью) у заглушки Deezer — из записанного популярного
    for (i, mut t) in fixture("artist_13828465_top")["data"]
        .as_array()
        .unwrap()
        .clone()
        .into_iter()
        .enumerate()
    {
        t["isrc"] = json!(format!("QZTOP{i:07}"));
        app.deezer.set(&format!("/track/{}", t["id"]), t);
    }
    let mut done = 0;
    while let Some((kind, outcome)) = app.run_job("scout").await {
        if kind == "analyze_preview" {
            assert_eq!(outcome, Outcome::Done);
            done += 1;
        }
    }
    assert_eq!(done, 10);
    assert_eq!(fake.calls().len(), 10);
    assert_eq!(scout::index_size(&app.state.db).await.unwrap(), 10);
    let s = Synth::new(app.state.db.clone());
    let a = s.artist("Kept").await;
    let user = user_of(&app, &me).await;
    for i in 0..100 {
        let t = s
            .track(
                a,
                &format!("kept {i}"),
                Feat {
                    jitter: i,
                    ..Default::default()
                },
            )
            .await;
        if i < 3 {
            s.like(user, t, 1).await;
        }
    }
    // Предел 100 при 110 анализах: уходят десять самых далёких — найденные не разведчиком (близость
    // неизвестна), а не популярное семени и не лайки
    let evicted = scout::evict(&app.state.db, 100).await.unwrap();
    assert_eq!(evicted, 10);
    assert_eq!(scout::index_size(&app.state.db).await.unwrap(), 100);
    let kept: (i64, i64) = sqlx::query_as(
        "SELECT (SELECT count(*) FROM liked_tracks l JOIN tracks t ON t.id = l.track_id
                 JOIN track_analysis a ON a.recording_id = t.recording_id),
                (SELECT count(*) FROM scout_tracks st JOIN tracks t ON t.id = st.track_id
                 JOIN track_analysis a ON a.recording_id = t.recording_id)",
    )
    .fetch_one(&app.state.db)
    .await
    .unwrap();
    assert_eq!(kept, (3, 10));
    // Дальше вытесняются и находки разведчика — с пометкой, чтобы их не ставили на анализ снова
    assert_eq!(scout::evict(&app.state.db, 90).await.unwrap(), 10);
    let marks: i64 =
        sqlx::query_scalar("SELECT count(*) FROM scout_tracks WHERE skipped = 'evicted'")
            .fetch_one(&app.state.db)
            .await
            .unwrap();
    assert_eq!(marks, 0, "сначала уходят остальные чужие (их 87)");
    assert_eq!(scout::evict(&app.state.db, 5).await.unwrap(), 85);
    let marks: i64 =
        sqlx::query_scalar("SELECT count(*) FROM scout_tracks WHERE skipped = 'evicted'")
            .fetch_one(&app.state.db)
            .await
            .unwrap();
    assert_eq!(
        marks, 8,
        "из популярного осталось два самых близких, лайки не тронуты"
    );
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn library_previews_come_first(db: PgPool) {
    let fake = FakeMl::start().await;
    let app = TestApp::with(db, |c| c.ml.url = fake.base.clone()).await;
    let me = app.listener("ann").await;
    sqlx::query("UPDATE job_lanes SET min_interval_ms = 0")
        .execute(&app.state.db)
        .await
        .unwrap();
    // Лайк, которого ещё нет на сервере: грани вкуса не ждут загрузчик — превью на анализ раньше находок
    let track = app.resolve_id("track", LIGHT_ME_UP, &me).await;
    let res = app.put(&format!("/api/library/tracks/{track}"), &me).await;
    assert_eq!(res.status, StatusCode::NO_CONTENT);
    assert_eq!(scout::enqueue_library(&app.state.db).await.unwrap(), 1);
    assert_eq!(
        scout::enqueue_library(&app.state.db).await.unwrap(),
        0,
        "уже стоит"
    );
    let priority: i16 = sqlx::query_scalar(
        "SELECT priority FROM jobs WHERE kind = 'analyze_preview' AND payload ->> 'trackId' = $1",
    )
    .bind(&track)
    .fetch_one(&app.state.db)
    .await
    .unwrap();
    assert_eq!(priority, 20);
    let (kind, outcome) = app.run_job("scout").await.unwrap();
    assert_eq!((kind.as_str(), outcome), ("analyze_preview", Outcome::Done));
    // Разобранная запись больше не ставится
    sqlx::query("UPDATE jobs SET finished_at = now() - interval '10 days'")
        .execute(&app.state.db)
        .await
        .unwrap();
    assert_eq!(scout::enqueue_library(&app.state.db).await.unwrap(), 0);
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn taste_makes_mixes_and_home_shows_them(db: PgPool) {
    let app = TestApp::new(db).await;
    let me = app.listener("ann").await;
    let user = user_of(&app, &me).await;
    let s = Synth::new(app.state.db.clone());
    // Два вкуса: трэп (ось 0) и спокойное (ось 600)
    taste_on(&s, user, 0, 6).await;
    taste_on(&s, user, 600, 6).await;
    for i in 0..20 {
        let a = s.artist(&format!("Fresh {i}")).await;
        s.track(
            a,
            &format!("fresh {i}"),
            Feat {
                axis: if i % 2 == 0 { 0 } else { 600 },
                jitter: 30 + i,
                ..Default::default()
            },
        )
        .await;
    }
    // Лайк двухмесячной давности, который с тех пор не играл, — «забытый любимый»
    let old_artist = s.artist("Old").await;
    let old = s
        .track(
            old_artist,
            "old",
            Feat {
                axis: 0,
                jitter: 90,
                ..Default::default()
            },
        )
        .await;
    s.like(user, old, 60).await;

    let report = rebuild_taste(&app, user).await;
    assert_eq!(report.facets, 2);
    assert_eq!(report.mixes, 2);
    assert!(report.discovery.unwrap() >= 5);
    let facets = taste::facets(&app.state.db, user).await.unwrap();
    assert!(
        facets[0].label.starts_with("Трэп · Liked"),
        "{}",
        facets[0].label
    );
    assert!((facets.iter().map(|f| f.weight).sum::<f32>() - 1.0).abs() < 1e-3);
    assert!(facets.iter().all(|f| !f.examples.is_empty()));

    crate::taste::morning(&app.state.db).await;
    let res = app.get("/api/home", &me).await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    let shelves = res.body["shelves"].as_array().unwrap();
    let ids: Vec<&str> = shelves.iter().map(|s| s["id"].as_str().unwrap()).collect();
    assert_eq!(
        ids,
        vec![
            "daily-mixes",
            "discover-weekly",
            "recent-likes",
            "forgotten-favorites"
        ]
    );
    let mixes = shelves[0]["items"].as_array().unwrap();
    assert_eq!(mixes.len(), 2);
    assert_eq!(mixes[0]["type"], "playlist");
    assert_eq!(mixes[0]["kind"], "mix");
    assert_eq!(mixes[0]["title"], "Микс дня 1");
    assert!(
        mixes[0]["description"]
            .as_str()
            .unwrap()
            .ends_with("и другие")
    );
    assert!(mixes[0]["tracksCount"].as_i64().unwrap() >= 5);
    let discovery = shelves[1]["items"].as_array().unwrap();
    assert!(
        discovery
            .iter()
            .all(|t| t["type"] == "track" && t["liked"] == false)
    );
    let forgotten = shelves[3]["items"].as_array().unwrap();
    assert_eq!(forgotten[0]["id"], old.to_string());

    // Пересчёт тех же миксов — те же плейлисты, новые треки
    let before: Vec<Uuid> =
        sqlx::query_scalar("SELECT id FROM playlists WHERE kind = 'mix' ORDER BY id")
            .fetch_all(&app.state.db)
            .await
            .unwrap();
    rebuild_taste(&app, user).await;
    let after: Vec<Uuid> =
        sqlx::query_scalar("SELECT id FROM playlists WHERE kind = 'mix' ORDER BY id")
            .fetch_all(&app.state.db)
            .await
            .unwrap();
    assert_eq!(before, after);

    // Лайк сдвигает ближайшую грань сразу
    let tracks_before: i32 =
        sqlx::query_scalar("SELECT sum(tracks)::int FROM taste_facets WHERE user_id = $1")
            .bind(user)
            .fetch_one(&app.state.db)
            .await
            .unwrap();
    let new_artist = s.artist("Another").await;
    let t = s
        .track(
            new_artist,
            "another",
            Feat {
                axis: 0,
                jitter: 99,
                ..Default::default()
            },
        )
        .await;
    let res = app.put(&format!("/api/library/tracks/{t}"), &me).await;
    assert_eq!(res.status, StatusCode::NO_CONTENT);
    let tracks_after: i32 =
        sqlx::query_scalar("SELECT sum(tracks)::int FROM taste_facets WHERE user_id = $1")
            .bind(user)
            .fetch_one(&app.state.db)
            .await
            .unwrap();
    assert_eq!(tracks_after, tracks_before + 1);
    let examples: Vec<Uuid> = sqlx::query_scalar(
        "SELECT examples[1] FROM taste_facets WHERE user_id = $1 AND $2 = ANY(examples)",
    )
    .bind(user)
    .bind(t)
    .fetch_all(&app.state.db)
    .await
    .unwrap();
    assert_eq!(examples, vec![t]);

    // Плановый пересчёт: уже сделанный сегодня не повторяется
    let settings = reco::Settings::load(&app.state.db).await.unwrap();
    assert_eq!(
        taste::schedule(&app.state.db, &settings).await.unwrap(),
        1,
        "ещё не было задачи"
    );
    assert_eq!(
        taste::schedule(&app.state.db, &settings).await.unwrap(),
        0,
        "уже стоит"
    );
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn admin_sees_the_index_and_tunes_the_scout(db: PgPool) {
    let fake = FakeMl::start().await;
    let app = TestApp::with(db, |c| c.ml.url = fake.base.clone()).await;
    let (_, admin) = app.admin("boss").await;
    let me = app.listener("ann").await;
    let user = user_of(&app, &me).await;
    let s = Synth::new(app.state.db.clone());
    taste_on(&s, user, 0, 4).await;
    for i in 0..6 {
        let a = s.artist(&format!("Idx {i}")).await;
        s.track(
            a,
            &format!("idx {i}"),
            Feat {
                jitter: 20 + i,
                ..Default::default()
            },
        )
        .await;
    }
    rebuild_taste(&app, user).await;
    let queued = create(&app, &me, json!({})).await["queue"]
        .as_array()
        .unwrap()
        .len();
    assert!(queued > 0);

    // По умолчанию — вкус того, кто смотрит: у администратора своего нет; общий индекс тот же
    let res = app.get("/api/admin/reco", &admin).await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    let own = &res.body;
    assert_eq!(own["user"]["name"], "Person boss");
    assert!(own["facets"].as_array().unwrap().is_empty());
    assert!(own["impressions"].as_array().unwrap().is_empty());
    assert!(own["sessions"].as_array().unwrap().is_empty());
    let names: Vec<&str> = own["listeners"]
        .as_array()
        .unwrap()
        .iter()
        .map(|l| l["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["Person boss", "Person ann"], "свой — первым");
    assert_eq!(own["index"]["size"], 10);

    let res = app
        .get(&format!("/api/admin/reco?userId={user}"), &admin)
        .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    let b = &res.body;
    assert_eq!(b["user"]["id"], user.to_string());
    assert!(b["user"]["likes"].as_i64().unwrap() > 0);
    assert_eq!(b["index"], own["index"], "индекс каталога общий");
    assert_eq!(b["index"]["size"], 10);
    assert_eq!(b["index"]["maxIndex"], 200_000);
    assert_eq!(b["index"]["full"], 0);
    assert_eq!(b["settings"]["tracksPerDay"], 3000);
    assert_eq!(b["modelVersion"], "rules-1");
    assert_eq!(b["ml"]["reachable"], true);
    assert_eq!(b["facets"][0]["userName"], "Person ann");
    assert!(!b["facets"][0]["examples"].as_array().unwrap().is_empty());
    let shown = b["impressions"].as_array().unwrap();
    assert_eq!(shown.len(), queued, "начало волны — тоже показы");
    assert!(shown[0]["reason"]["text"].is_string());
    assert_eq!(shown[0]["surface"], "wave");
    assert!(shown.iter().all(|i| i["userName"] == "Person ann"));
    let sessions = b["sessions"].as_array().unwrap();
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0]["title"], "Моя волна");
    assert_eq!(sessions[0]["served"], queued);
    assert_eq!(sessions[0]["settings"]["character"], "mix");
    assert!(
        b["mixes"]
            .as_array()
            .unwrap()
            .iter()
            .all(|m| m["generator"] == "daily_mix" || m["generator"] == "discover_weekly")
    );
    // Заготовки — по сочетаниям настроек, с готовым на сервере
    sqlx::query(
        "INSERT INTO wave_pools (user_id, combo, track_id, settings)
         SELECT $1, 'any:new:any:any', t.id, '{\"character\": \"new\"}'::jsonb FROM tracks t LIMIT 3",
    )
    .bind(user)
    .execute(&app.state.db)
    .await
    .unwrap();
    let res = app
        .get(&format!("/api/admin/reco?userId={user}"), &admin)
        .await;
    let pools = res.body["pools"].as_array().unwrap().clone();
    assert_eq!(pools.len(), 1);
    assert_eq!(pools[0]["combo"], "any:new:any:any");
    assert_eq!(pools[0]["tracks"], 3);
    assert_eq!(pools[0]["ready"], 0);
    assert_eq!(pools[0]["settings"]["character"], "new");
    let res = app
        .get(
            &format!("/api/admin/reco?userId={}", Uuid::now_v7()),
            &admin,
        )
        .await;
    assert_eq!(res.status, StatusCode::NOT_FOUND);

    let res = app
        .call(
            Method::PATCH,
            "/api/admin/reco/settings",
            Some(json!({ "tracksPerDay": 500, "maxIndex": 50000, "maxHops": 3, "hopDecay": 0.6 })),
            &admin,
        )
        .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    assert_eq!(res.body["tracksPerDay"], 500);
    assert_eq!(res.body["maxIndex"], 50000);
    assert_eq!(res.body["maxHops"], 3);
    for body in [
        json!({ "tracksPerDay": -1 }),
        json!({ "maxIndex": 10 }),
        json!({ "maxHops": 4 }),
        json!({ "hopDecay": 1.5 }),
        json!({ "utcOffsetMin": 1000 }),
        json!({ "boldness": 1 }),
    ] {
        let res = app
            .call(
                Method::PATCH,
                "/api/admin/reco/settings",
                Some(body.clone()),
                &admin,
            )
            .await;
        assert_eq!(res.status, StatusCode::BAD_REQUEST, "{body}");
    }
    let res = app
        .post("/api/admin/reco/run", json!({ "task": "scout" }), &admin)
        .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    assert_eq!(res.body["queued"], 1);
    let res = app
        .post("/api/admin/reco/run", json!({ "task": "scout" }), &admin)
        .await;
    assert_eq!(res.body["queued"], 0, "уже стоит");
    // Пересчёт вкуса выбранного слушателя — только его
    sqlx::query("DELETE FROM jobs WHERE kind = 'taste'")
        .execute(&app.state.db)
        .await
        .unwrap();
    let res = app
        .post(
            "/api/admin/reco/run",
            json!({ "task": "taste", "userId": user }),
            &admin,
        )
        .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    assert_eq!(res.body["queued"], 1);
    let for_whom: Vec<String> =
        sqlx::query_scalar("SELECT payload ->> 'userId' FROM jobs WHERE kind = 'taste'")
            .fetch_all(&app.state.db)
            .await
            .unwrap();
    assert_eq!(for_whom, vec![user.to_string()]);
    let res = app
        .post(
            "/api/admin/reco/run",
            json!({ "task": "taste", "userId": Uuid::now_v7() }),
            &admin,
        )
        .await;
    assert_eq!(res.status, StatusCode::NOT_FOUND);
    let res = app
        .post("/api/admin/reco/run", json!({ "task": "taste" }), &admin)
        .await;
    assert_eq!(
        res.body["queued"], 0,
        "у ann уже стоит, у остальных нет данных"
    );
    let kinds = app.audit_kinds().await;
    assert!(kinds.iter().any(|k| k == "reco.settings_changed"));
    assert!(kinds.iter().any(|k| k == "reco.run"));

    // Только администратору
    for (method, uri) in [
        (Method::GET, "/api/admin/reco"),
        (Method::PATCH, "/api/admin/reco/settings"),
        (Method::POST, "/api/admin/reco/run"),
    ] {
        let body = (method != Method::GET).then(|| json!({ "task": "scout" }));
        let res = app.call(method.clone(), uri, body, &me).await;
        assert_eq!(res.status, StatusCode::FORBIDDEN, "{method} {uri}");
    }
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn chart_finds_are_weighted_by_closeness_to_taste(db: PgPool) {
    let app = TestApp::new(db).await;
    let s = Synth::new(app.state.db.clone());
    let unanalyzed = || Feat {
        analyzed: false,
        ..Default::default()
    };
    let near = s.artist("Near").await;
    let far = s.artist("Hop Two").await;
    let stranger = s.artist("Game Composer").await;
    let near_chart = s.track(near, "near chart", unanalyzed()).await;
    let far_top = s.track(far, "far top", unanalyzed()).await;
    let stranger_chart = s.track(stranger, "boss theme", unanalyzed()).await;
    sqlx::query(
        "INSERT INTO scout_artists (artist_id, weight, hops) VALUES ($1, 0.5, 1), ($2, 0.25, 2)",
    )
    .bind(near)
    .bind(far)
    .execute(&app.state.db)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO scout_tracks (track_id, artist_id, source, base) VALUES
            ($1, NULL, 'chart', 0.4), ($2, $4, 'top', 0.8), ($3, NULL, 'chart', 0.45)",
    )
    .bind(near_chart)
    .bind(far_top)
    .bind(stranger_chart)
    .bind(far)
    .execute(&app.state.db)
    .await
    .unwrap();
    let settings = reco::Settings::load(&app.state.db).await.unwrap();
    assert_eq!(
        scout::enqueue_previews(&app.state.db, &settings)
            .await
            .unwrap(),
        3
    );
    let priority = |track: Uuid| {
        let db = app.state.db.clone();
        async move {
            sqlx::query_scalar::<_, i16>(
                "SELECT priority FROM jobs WHERE kind = 'analyze_preview' AND dedupe_key = 'analyze_preview:' || $1",
            )
            .bind(track)
            .fetch_one(&db)
            .await
            .unwrap()
        }
    };
    // Чарт с артистом окрестности — по его весу (0,4 × 0,5), чарт незнакомца — четверть (0,45 × 0,25),
    // а не единица, с которой он обгонял популярное похожих артистов
    assert_eq!(priority(near_chart).await, 2);
    assert_eq!(priority(far_top).await, 2);
    assert_eq!(priority(stranger_chart).await, 1);
}
