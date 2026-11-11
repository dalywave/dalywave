//! Сеть вкуса: набор из прослушиваний с отложенной по времени проверкой, обучение в ml (заглушка
//! отдаёт крошечную настоящую ONNX из `tests/fixtures/taste`), включение только если сеть точнее правил,
//! применение в волне через onnxruntime и раздел «Рекомендации» админки. Мало данных — правила, ml не зовут.

use std::sync::{Arc, Mutex};

use axum::{Json, Router, extract::State, routing::post};
use base64::{Engine, engine::general_purpose::STANDARD};
use dalywave_server::{
    jobs::worker::Outcome,
    ml::Ml,
    reco::{self, model, taste},
};
use serde_json::{Value, json};
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    common::TestApp,
    synth::{Feat, Synth},
};

#[derive(Clone, Default)]
struct Stub {
    calls: Arc<Mutex<Vec<Value>>>,
    /// Метрики следующего ответа
    metrics: Arc<Mutex<Value>>,
}

async fn train(State(stub): State<Stub>, Json(body): Json<Value>) -> Json<Value> {
    stub.calls.lock().unwrap().push(body);
    let onnx = std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/taste/tiny.onnx"),
    )
    .unwrap();
    Json(json!({
        "onnx": STANDARD.encode(onnx),
        "inputDim": model::FEATURE_DIM,
        "heads": ["skip", "complete", "like"],
        "params": 172,
        "epochs": 3,
        "reduced": { "sound": 2, "semantic": 2 },
        "metrics": stub.metrics.lock().unwrap().clone(),
    }))
}

impl Stub {
    async fn start() -> (Self, String) {
        let stub = Self::default();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let app = Router::new()
            .route("/v1/taste/train", post(train))
            .with_state(stub.clone());
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        (stub, base)
    }

    fn answer(&self, auc: f64, base_auc: f64, loss: f64, base_loss: f64) {
        let head = |p: i64| json!({ "positives": p, "auc": auc, "logloss": loss, "baselineAuc": base_auc, "baselineLogloss": base_loss });
        *self.metrics.lock().unwrap() = json!({
            "train": 320, "holdout": 80,
            "heads": { "skip": head(40), "complete": head(40), "like": { "positives": 0, "auc": null,
                       "logloss": 0.01, "baselineAuc": null, "baselineLogloss": 0.02 } },
            "auc": auc, "logloss": loss, "baselineAuc": base_auc, "baselineLogloss": base_loss,
        });
    }
}

async fn active_version(db: &PgPool) -> String {
    sqlx::query_scalar("SELECT version FROM model_versions WHERE kind = 'taste' AND active")
        .fetch_one(db)
        .await
        .unwrap()
}

async fn run_training(app: &TestApp, base: &str) -> Outcome {
    assert!(model::request(&app.state.db).await.unwrap());
    let (job, outcome) = app
        .worker_with_ml(Ml::new(base).unwrap())
        .run_once("maintenance")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(job.kind, "taste_model");
    outcome
}

/// Любимое — ось 0 (артист A, дослушивают), чужое — ось 100 (артист B, пропускают с первых секунд).
async fn history(s: &Synth, user: Uuid, plays: usize) -> Vec<Uuid> {
    let a = s.artist("Loved").await;
    let b = s.artist("Skipped").await;
    let mut loved = Vec::new();
    let mut skipped = Vec::new();
    for i in 0..20 {
        let feat = |axis| Feat {
            axis,
            jitter: i,
            ..Default::default()
        };
        loved.push(s.track(a, &format!("loved {i}"), feat(0)).await);
        skipped.push(s.track(b, &format!("skipped {i}"), feat(100)).await);
    }
    // Каждые 4 минуты — одна долгая сессия
    for i in 0..plays {
        let (track, completed) = if i % 2 == 0 {
            (loved[(i / 2) % 20], true)
        } else {
            (skipped[(i / 2) % 20], false)
        };
        sqlx::query(
            "INSERT INTO play_events (client_event_id, user_id, track_id, started_at, played_ms, duration_ms, end_reason)
             VALUES ($1, $2, $3, now() - make_interval(mins => $4), $5, 180000, $6)",
        )
        .bind(Uuid::now_v7())
        .bind(user)
        .bind(track)
        .bind(((plays - i) * 4) as i32)
        .bind(if completed { 180_000 } else { 5_000 })
        .bind(if completed { "completed" } else { "skipped" })
        .execute(&s.db)
        .await
        .unwrap();
    }
    loved
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn little_data_stays_on_rules(db: PgPool) {
    let app = TestApp::new(db).await;
    let (id, admin) = app.admin("root").await;
    let user: Uuid = id.parse().unwrap();
    let s = Synth::new(app.state.db.clone());
    history(&s, user, 40).await;
    let (stub, base) = Stub::start().await;

    let data = model::dataset(&app.state.db).await.unwrap();
    assert_eq!(data.rows, 40);
    assert!(!data.enough());
    // Ночью задача не ставится вовсе: прослушиваний меньше порога
    let settings = reco::Settings::load(&app.state.db).await.unwrap();
    assert!(!model::schedule(&app.state.db, &settings).await.unwrap());

    let outcome = run_training(&app, &base).await;
    assert!(matches!(outcome, Outcome::Done), "{outcome:?}");
    assert!(
        stub.calls.lock().unwrap().is_empty(),
        "с малыми данными ml не зовут"
    );
    assert_eq!(active_version(&app.state.db).await, reco::MODEL_VERSION);

    let res = app.get("/api/admin/reco", &admin).await;
    assert_eq!(res.status, axum::http::StatusCode::OK, "{}", res.text);
    let training = &res.body["training"];
    assert_eq!(training["outcome"], "too_little_data");
    assert_eq!(training["minRows"], model::MIN_ROWS as i64);
    assert!(
        training["message"]
            .as_str()
            .unwrap()
            .contains("Мало данных")
    );
    let models = res.body["models"].as_array().unwrap();
    assert_eq!(models.len(), 1);
    assert_eq!(models[0]["version"], "rules-1");
    assert_eq!(models[0]["active"], true);
    assert_eq!(models[0]["learned"], false);

    // Типы интерфейса строятся из OpenAPI: версии и обучение описаны там
    let spec = app.get("/api/openapi.json", &admin).await.body;
    let schemas = &spec["components"]["schemas"];
    for name in ["ModelVersionView", "ModelTraining", "HeadMetrics"] {
        assert!(schemas[name].is_object(), "{name}");
    }
    assert!(
        spec.to_string().contains("\"model\""),
        "RecoTask знает model"
    );
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn network_switches_on_only_when_it_beats_rules(db: PgPool) {
    let app = TestApp::new(db).await;
    let (id, admin) = app.admin("root").await;
    let user: Uuid = id.parse().unwrap();
    let s = Synth::new(app.state.db.clone());
    let loved = history(&s, user, 400).await;
    for t in loved.iter().take(6) {
        s.like(user, *t, 30).await;
    }
    // Новое рядом со вкусом: заслушанное «отдыхает», волне нужно, что предложить
    for i in 0..12 {
        let artist = s.artist(&format!("Fresh {i}")).await;
        let feat = Feat {
            axis: 0,
            jitter: 30 + i,
            ..Default::default()
        };
        s.track(artist, &format!("fresh {i}"), feat).await;
    }
    let settings = reco::Settings::load(&app.state.db).await.unwrap();
    taste::rebuild(&app.state.db, user, &settings, false)
        .await
        .unwrap();
    let (stub, base) = Stub::start().await;

    // Набор: строки по времени, метки, последние 20 % — проверка
    let data = model::dataset(&app.state.db).await.unwrap();
    assert_eq!(data.rows, 400);
    assert_eq!(data.holdout_from, 320);
    assert_eq!(data.vector_rows, 40, "вектор трека — один раз");
    assert_eq!(data.scalars.len(), 400 * model::SCALARS.len());
    let label = |i: usize| &data.labels[i * 3..i * 3 + 3];
    assert_eq!(label(0), [0.0, 1.0, 0.0], "любимое дослушано");
    assert_eq!(label(1), [1.0, 0.0, 0.0], "чужое пропущено");
    let col = |i: usize, name: &str| {
        let k = model::SCALARS.iter().position(|x| *x == name).unwrap();
        data.scalars[i * model::SCALARS.len() + k]
    };
    assert_eq!(
        col(0, "completed"),
        0.0,
        "в первый раз трек ещё не дослушан"
    );
    assert!(col(398, "completed") > 0.0, "потом — уже знаком");
    assert_eq!(col(2, "artist_familiar"), 1.0);
    assert_eq!(col(1, "skip_streak"), 0.0);
    assert_eq!(col(2, "skip_streak"), 0.2, "после пропуска");
    assert!(col(3, "session_pos") > col(1, "session_pos"));
    // Любимое ближе к грани вкуса, и правила это знают
    assert!(col(0, "taste_sim") > col(1, "taste_sim"));
    assert!(data.baseline[0] > data.baseline[1]);

    // Правила точнее — версия записана, но не включена
    stub.answer(0.62, 0.66, 0.55, 0.50);
    let outcome = run_training(&app, &base).await;
    assert!(matches!(outcome, Outcome::Done), "{outcome:?}");
    {
        let calls = stub.calls.lock().unwrap();
        assert_eq!(calls.len(), 1);
        let body = &calls[0];
        assert_eq!(body["rows"], 400);
        assert_eq!(body["holdoutFrom"], 320);
        assert_eq!(body["scalarDim"], model::SCALARS.len());
        assert_eq!(body["soundDim"], 1280);
        assert_eq!(body["featureNames"][0], "bpm");
    }
    assert_eq!(active_version(&app.state.db).await, reco::MODEL_VERSION);
    let stored: Option<String> = sqlx::query_scalar(
        "SELECT artifact_key FROM model_versions WHERE version LIKE 'nn-%' ORDER BY created_at LIMIT 1",
    )
    .fetch_one(&app.state.db)
    .await
    .unwrap();
    let key = stored.expect("ONNX в хранилище");
    assert!(key.starts_with("models/taste/nn-"));
    assert!(app.state.storage.head(&key).await.unwrap().is_some());

    // Сеть точнее — включается сама
    stub.answer(0.74, 0.66, 0.45, 0.50);
    run_training(&app, &base).await;
    let version = active_version(&app.state.db).await;
    assert!(version.starts_with("nn-"), "{version}");

    // Волна теперь собирается с сетью: показы пишутся с её версией
    let created = app
        .post(
            "/api/wave/sessions",
            json!({ "settings": { "mood": "any", "activity": "any", "character": "mix", "language": "any" } }),
            &admin,
        )
        .await;
    assert_eq!(
        created.status,
        axum::http::StatusCode::CREATED,
        "{}",
        created.text
    );
    let shown: Vec<String> = sqlx::query_scalar(
        "SELECT DISTINCT model_version FROM recommendation_impressions WHERE user_id = $1",
    )
    .bind(user)
    .fetch_all(&app.state.db)
    .await
    .unwrap();
    assert_eq!(shown, vec![version.clone()]);

    let res = app.get("/api/admin/reco", &admin).await;
    assert_eq!(res.body["modelVersion"], version.as_str());
    assert_eq!(res.body["training"]["outcome"], "trained");
    let models = res.body["models"].as_array().unwrap();
    assert_eq!(models.len(), 3);
    assert_eq!(models[0]["version"], version.as_str());
    assert_eq!(models[0]["active"], true);
    assert_eq!(models[0]["beatsRules"], true);
    assert_eq!(models[0]["heads"].as_array().unwrap().len(), 3);
    assert_eq!(models[1]["beatsRules"], false);
    assert!(
        models[1]["note"]
            .as_str()
            .unwrap()
            .starts_with("Правила точнее")
    );
    assert_eq!(models[2]["version"], "rules-1");
    assert_eq!(models[2]["active"], false);

    // Ручной запуск из админки ставит задачу
    let res = app
        .post("/api/admin/reco/run", json!({ "task": "model" }), &admin)
        .await;
    assert_eq!(res.body["queued"], 1, "{}", res.text);

    // Сломанная версия не роняет волну: подбор остаётся на правилах
    sqlx::query("UPDATE model_versions SET artifact = 'junk' WHERE version = $1")
        .bind(&version)
        .execute(&app.state.db)
        .await
        .unwrap();
    sqlx::query("UPDATE model_versions SET active = false WHERE kind = 'taste'")
        .execute(&app.state.db)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO model_versions (kind, version, metrics, artifact, active)
         VALUES ('taste', 'nn-broken', '{}', 'junk', true)",
    )
    .execute(&app.state.db)
    .await
    .unwrap();
    assert!(model::active(&app.state.db).await.unwrap().is_none());
    let created = app
        .post(
            "/api/wave/sessions",
            json!({ "settings": { "mood": "calm", "activity": "any", "character": "mix", "language": "any" } }),
            &admin,
        )
        .await;
    assert_eq!(
        created.status,
        axum::http::StatusCode::CREATED,
        "{}",
        created.text
    );
}
