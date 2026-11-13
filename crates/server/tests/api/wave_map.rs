//! «Как подобран» против API: след подбора пишется вместе с показами (фоном, небольшой), карта звучания
//! считается сервисом анализа или запасным PCA и выравнивается к прошлой, новые треки ложатся по
//! соседям; `GET /api/wave/explain` отдаёт карту только своего показа.

use std::time::Duration;

use axum::{Router, body::Bytes, extract::Query, http::StatusCode, routing::post};
use dalywave_server::{
    ml::Ml,
    reco::{self, map, taste},
};
use serde_json::{Value, json};
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    common::TestApp,
    synth::{Feat, Synth},
};

/// Любимое на двух «осях» звучания (две грани вкуса) и незнакомое рядом с ними — всё на сервере.
async fn catalog(app: &TestApp, s: &Synth, user: Uuid) -> Vec<Uuid> {
    let mut all = Vec::new();
    for (axis, name) in [(0usize, "Ось"), (400, "Даль")] {
        for i in 0..5 {
            let a = s.artist(&format!("{name} {i}")).await;
            let t = s
                .track(
                    a,
                    &format!("{name} любимое {i}"),
                    Feat {
                        axis,
                        jitter: i,
                        status: "ready",
                        ..Default::default()
                    },
                )
                .await;
            s.like(user, t, 5).await;
            all.push(t);
        }
        for i in 0..10 {
            let a = s.artist(&format!("{name} новое {i}")).await;
            all.push(
                s.track(
                    a,
                    &format!("{name} новое {i}"),
                    Feat {
                        axis,
                        jitter: 10 + i,
                        status: "ready",
                        ..Default::default()
                    },
                )
                .await,
            );
        }
    }
    let settings = reco::Settings::load(&app.state.db).await.unwrap();
    taste::rebuild(&app.state.db, user, &settings, false)
        .await
        .unwrap();
    all
}

/// Сервис анализа, которого нет: карта — запасным PCA.
fn no_ml() -> Ml {
    Ml::new("http://127.0.0.1:9").unwrap()
}

async fn wait_traces(db: &PgPool, session: Uuid, n: i64) {
    for _ in 0..100 {
        let have: i64 =
            sqlx::query_scalar("SELECT count(*) FROM wave_traces WHERE wave_session_id = $1")
                .bind(session)
                .fetch_one(db)
                .await
                .unwrap();
        if have >= n {
            return;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    panic!("след подбора не записался");
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn wave_explains_how_a_track_was_picked(db: PgPool) {
    let app = TestApp::new(db.clone()).await;
    let (user, auth) = app.admin("map-owner").await;
    let user: Uuid = user.parse().unwrap();
    catalog(&app, &Synth::new(db.clone()), user).await;

    // Карта: сервиса анализа нет — PCA; первая карта — в квадрате 0..1, грани — на ней
    let report = map::layout(&db, &no_ml()).await.unwrap();
    assert_eq!(report.method, "pca");
    assert_eq!(report.points, 30);
    assert_eq!(report.aligned, 0);
    assert!(report.facets >= 2, "{report:?}");
    let (lo, hi): (f32, f32) = sqlx::query_as(
        "SELECT min(least(map_x, map_y)), max(greatest(map_x, map_y)) FROM track_analysis",
    )
    .fetch_one(&db)
    .await
    .unwrap();
    assert!(lo > -0.2 && hi < 1.2, "{lo}..{hi}");

    // Волна: начало и ещё два трека — у каждого подбора свой след, у показов — ссылка на него
    let created = app
        .post("/api/wave/sessions", json!({ "settings": {} }), &auth)
        .await;
    assert_eq!(created.status, StatusCode::CREATED, "{}", created.text);
    let session: Uuid = created.body["id"].as_str().unwrap().parse().unwrap();
    let queue: Vec<Uuid> = created.body["queue"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["track"]["id"].as_str().unwrap().parse().unwrap())
        .collect();
    assert!(queue.len() >= 2);
    let first = queue[0];
    let res = app
        .post(
            &format!("/api/wave/sessions/{session}/feedback"),
            json!({ "trackId": first, "kind": "completed", "positionMs": 179000 }),
            &auth,
        )
        .await;
    assert_eq!(res.status, StatusCode::NO_CONTENT, "{}", res.text);
    let next = app
        .get(&format!("/api/wave/sessions/{session}/next?count=2"), &auth)
        .await;
    assert_eq!(next.status, StatusCode::OK, "{}", next.text);
    let picked: Uuid = next.body["items"][0]["track"]["id"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap();
    wait_traces(&db, session, 2).await;
    let unlinked: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM recommendation_impressions WHERE wave_session_id = $1 AND trace_id IS NULL",
    )
    .bind(session)
    .fetch_one(&db)
    .await
    .unwrap();
    assert_eq!(unlinked, 0);
    let (size, data): (i32, Value) = sqlx::query_as(
        "SELECT pg_column_size(t.data), t.data FROM wave_traces t
         JOIN recommendation_impressions i ON i.trace_id = t.id
         WHERE i.wave_session_id = $1 AND i.track_id = $2",
    )
    .bind(session)
    .bind(picked)
    .fetch_one(&db)
    .await
    .unwrap();
    assert!(size < 8 * 1024, "след {size} байт");
    assert_eq!(data["v"], 1);
    assert!(data["considered"].as_u64().unwrap() >= 20, "{data}");
    // Уже выданное — отсеяно с причиной; выбранный — в вариантах с местом и слагаемыми
    assert!(data["drops"]["served"].as_u64().unwrap() >= 1, "{data}");
    assert!(
        data["variants"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v[0] == json!(picked) && v[5] == json!(0))
    );
    assert!(data["picks"][picked.to_string()]["parts"]["sim"].is_number());

    // Карта для выбранного: грани, сессия до него, варианты, причины коротко
    let res = app
        .get(
            &format!("/api/wave/explain?trackId={picked}&sessionId={session}"),
            &auth,
        )
        .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    let b = &res.body;
    assert_eq!(b["track"]["id"], json!(picked));
    assert!(b["x"].is_number() && b["y"].is_number());
    let facets = b["facets"].as_array().unwrap();
    assert!(facets.len() >= 2, "{b}");
    assert!(facets.iter().all(|f| f["r"].as_f64().unwrap() > 0.0));
    let steps = b["session"].as_array().unwrap();
    assert!(
        steps
            .iter()
            .any(|s| s["trackId"] == json!(first) && s["outcome"] == "completed"),
        "{b}"
    );
    let cands = b["candidates"].as_array().unwrap();
    assert!(!cands.is_empty());
    assert!(cands.iter().all(|c| c["trackId"] != json!(picked)));
    assert!(cands.iter().any(|c| c["stage"] == "dropped"), "{b}");
    assert!(b["considered"].as_u64().unwrap() >= 20);
    assert_eq!(b["dust"].as_array().unwrap().len() % 2, 0);
    let reasons = b["reasons"].as_array().unwrap();
    assert!(!reasons.is_empty() && reasons.len() <= 4);
    for r in reasons {
        let text = r["text"].as_str().unwrap();
        assert!(!text.contains(" — "), "коротко: {text}");
    }

    // Чужой показ — как не найденный; трек, который волна не предлагала, — тоже
    let other = app.listener("map-stranger").await;
    let res = app
        .get(
            &format!("/api/wave/explain?trackId={picked}&sessionId={session}"),
            &other,
        )
        .await;
    assert_eq!(res.status, StatusCode::NOT_FOUND);
    let res = app
        .get(
            &format!("/api/wave/explain?trackId={}", Uuid::now_v7()),
            &auth,
        )
        .await;
    assert_eq!(res.status, StatusCode::NOT_FOUND);

    // Следы старше 30 дней убирает плановая чистка
    sqlx::query("UPDATE wave_traces SET created_at = now() - interval '31 days'")
        .execute(&db)
        .await
        .unwrap();
    assert_eq!(reco::trace::cleanup(&db).await.unwrap(), 2);
}

/// Заглушка `POST /v1/layout`: карта — первые две координаты вектора, повёрнутые на 90° и растянутые
/// вчетверо (как будто UMAP положил карту иначе).
async fn fake_layout() -> String {
    async fn layout(Query(q): Query<Vec<(String, usize)>>, body: Bytes) -> axum::Json<Value> {
        let get = |k: &str| q.iter().find(|(n, _)| n == k).unwrap().1;
        let (rows, dim) = (get("rows"), get("dim"));
        assert!(body.len() == rows * dim * 2 || body.len() == rows * dim * 2 + rows * 8);
        let points: Vec<[f32; 2]> = (0..rows)
            .map(|r| {
                let at = |c: usize| {
                    let o = (r * dim + c) * 2;
                    f32::from(u8::from(u16::from_le_bytes([body[o], body[o + 1]]) != 0))
                };
                // Ось трека — по первым двум «единицам» вектора (оси 0 и 400)
                let (a, b) = (at(0), at(400));
                [-4.0 * (b + r as f32 * 0.001), 4.0 * (a + r as f32 * 0.002)]
            })
            .collect();
        axum::Json(json!({ "points": points, "tookMs": 1 }))
    }
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(async move {
        axum::serve(listener, Router::new().route("/v1/layout", post(layout)))
            .await
            .unwrap();
    });
    base
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn map_stays_in_place_between_rebuilds(db: PgPool) {
    let app = TestApp::new(db.clone()).await;
    let (user, _) = app.admin("map-keeper").await;
    let user: Uuid = user.parse().unwrap();
    let s = Synth::new(db.clone());
    catalog(&app, &s, user).await;
    let base = fake_layout().await;
    let ml = Ml::new(&base).unwrap();

    let first = map::layout(&db, &ml).await.unwrap();
    assert_eq!(first.method, "umap");
    let before: Vec<(Uuid, f32, f32)> = sqlx::query_as(
        "SELECT recording_id, map_x, map_y FROM track_analysis ORDER BY recording_id",
    )
    .fetch_all(&db)
    .await
    .unwrap();
    // Тот же UMAP, иначе повёрнутый, — после выравнивания на тех же местах
    let second = map::layout(&db, &ml).await.unwrap();
    assert_eq!(second.aligned, 30);
    assert!(second.shift.unwrap() < 0.01, "{second:?}");
    let after: Vec<(Uuid, f32, f32)> = sqlx::query_as(
        "SELECT recording_id, map_x, map_y FROM track_analysis ORDER BY recording_id",
    )
    .fetch_all(&db)
    .await
    .unwrap();
    for (a, b) in before.iter().zip(&after) {
        assert!(
            (a.1 - b.1).abs() < 0.01 && (a.2 - b.2).abs() < 0.01,
            "{a:?} → {b:?}"
        );
    }

    // Новый трек между пересчётами — среднее соседей: рядом со своей осью
    let artist = s.artist("Новичок").await;
    let fresh = s
        .track(
            artist,
            "новичок",
            Feat {
                axis: 0,
                jitter: 40,
                ..Default::default()
            },
        )
        .await;
    assert_eq!(map::fill(&db, None, 100).await.unwrap(), 1);
    let (x, y): (f32, f32) = sqlx::query_as(
        "SELECT a.map_x, a.map_y FROM tracks t JOIN track_analysis a ON a.recording_id = t.recording_id WHERE t.id = $1",
    )
    .bind(fresh)
    .fetch_one(&db)
    .await
    .unwrap();
    let (cx, cy): (f64, f64) = sqlx::query_as(
        "SELECT avg(a.map_x)::float8, avg(a.map_y)::float8 FROM tracks t
         JOIN track_analysis a ON a.recording_id = t.recording_id
         JOIN track_artists ta ON ta.track_id = t.id JOIN artists ar ON ar.id = ta.artist_id
         WHERE ar.name LIKE 'Ось%'",
    )
    .fetch_one(&db)
    .await
    .unwrap();
    assert!(
        ((x as f64 - cx).powi(2) + (y as f64 - cy).powi(2)).sqrt() < 0.2,
        "{x},{y} vs {cx},{cy}"
    );
}
