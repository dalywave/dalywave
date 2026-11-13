//! Сеть вкуса: набор из прослушиваний с честной отложенной по времени проверкой (метки импорта, лайки
//! из медиатеки, грани без знания отложенного), обучение здесь же (`dalywave-taste`), проверка через
//! onnxruntime и включение только если сеть точнее правил и текущей версии. Обучение на компьютере:
//! задание, веса, проверка сервером (плохие веса не включаются), права — общую сеть присылает только
//! администратор. «Почему этот трек» — по показу волны.

use std::collections::HashMap;

use axum::http::StatusCode;
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use chrono::{DateTime, Duration, Utc};
use dalywave_server::{
    jobs::worker::Outcome,
    reco::{self, model},
};
use dalywave_taste::wire;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    common::{Auth, TestApp},
    mock::Person,
    synth::{Feat, Synth},
};

const VERIFIER: &str = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk-taste";

/// Вход приложения на устройстве `platform` тем же человеком: токен устройства.
async fn app_token(app: &TestApp, person: &Person, platform: &str, name: &str) -> Auth {
    let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(VERIFIER.as_bytes()));
    let (authorize, cookie) = app
        .start(&format!(
            "/api/auth/app/start?redirect_uri=dalywave%3A%2F%2Fauth&code_challenge={challenge}\
             &code_challenge_method=S256&state=s1&device_name={name}&platform={platform}"
        ))
        .await;
    let (code, state) = app.provider.approve(&authorize, person);
    let res = app.callback(&code, &state, Some(&cookie)).await;
    let q: HashMap<String, String> = url::Url::parse(&res.location())
        .unwrap()
        .query_pairs()
        .into_owned()
        .collect();
    let res = app
        .post(
            "/api/auth/app/token",
            json!({ "code": q["code"], "codeVerifier": VERIFIER, "appVersion": "0.1.5" }),
            &Auth::None,
        )
        .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    Auth::Bearer(res.body["token"].as_str().unwrap().to_string())
}

async fn active_version(db: &PgPool) -> String {
    sqlx::query_scalar(
        "SELECT version FROM model_versions WHERE kind = 'taste' AND active AND user_id IS NULL",
    )
    .fetch_one(db)
    .await
    .unwrap()
}

async fn play_at(
    db: &PgPool,
    user: Uuid,
    track: Uuid,
    at: DateTime<Utc>,
    played_ms: i32,
    end: &str,
    source: &str,
) {
    sqlx::query(
        "INSERT INTO play_events (client_event_id, user_id, track_id, started_at, played_ms, duration_ms, end_reason, source)
         VALUES ($1, $2, $3, $4, $5, 180000, $6, $7)",
    )
    .bind(Uuid::now_v7())
    .bind(user)
    .bind(track)
    .bind(at)
    .bind(played_ms)
    .bind(end)
    .bind(source)
    .execute(db)
    .await
    .unwrap();
}

/// Вкус зависит от времени суток, а не от звучания: утром дослушивают, вечером пропускают те же
/// треки. Правила (близость к граням) этого не видят, сеть — видит. По 10 треков утром и вечером
/// каждый день, `days` дней.
async fn history_by_hour(s: &Synth, user: Uuid, days: i64) -> Vec<Uuid> {
    let artist = s.artist("Day and Night").await;
    let mut tracks = Vec::new();
    for i in 0..20 {
        let feat = Feat {
            axis: (i % 4) * 100,
            jitter: i,
            ..Default::default()
        };
        let name = format!("track {}", char::from(b'a' + i as u8));
        tracks.push(s.track(artist, &name, feat).await);
    }
    let today = Utc::now()
        .date_naive()
        .and_hms_opt(0, 0, 0)
        .unwrap()
        .and_utc();
    for d in 0..days {
        let day = today - Duration::days(days - d);
        for k in 0..10 {
            let t = tracks[((d * 3 + k) % 20) as usize];
            // 09:00 и 21:00 по Москве (UTC+3 — часовой пояс по умолчанию)
            let morning = day + Duration::hours(6) + Duration::minutes(4 * k);
            let evening = day + Duration::hours(18) + Duration::minutes(4 * k);
            play_at(&s.db, user, t, morning, 180_000, "completed", "stream").await;
            play_at(&s.db, user, t, evening, 5_000, "skipped", "stream").await;
        }
    }
    tracks
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn little_data_stays_on_rules(db: PgPool) {
    let app = TestApp::new(db).await;
    let (id, admin) = app.admin("root").await;
    let user: Uuid = id.parse().unwrap();
    let s = Synth::new(app.state.db.clone());
    history_by_hour(&s, user, 2).await;

    let data = model::dataset(&app.state.db, &model::Recipe::default(), None, None)
        .await
        .unwrap();
    assert_eq!(data.rows(), 40);
    assert!(!data.enough());
    // Ночью задача не ставится вовсе: прослушиваний меньше порога
    let settings = reco::Settings::load(&app.state.db).await.unwrap();
    assert!(!model::schedule(&app.state.db, &settings).await.unwrap());

    assert!(model::request(&app.state.db).await.unwrap());
    let (kind, outcome) = app.run_job("maintenance").await.unwrap();
    assert_eq!(kind, "taste_model");
    assert!(matches!(outcome, Outcome::Done), "{outcome:?}");
    assert_eq!(active_version(&app.state.db).await, reco::MODEL_VERSION);

    let res = app.get("/api/admin/reco", &admin).await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
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
    assert_eq!(models[0]["origin"], "rules");

    // Устройство не получает задание, когда данных мало
    let mac = app_token(&app, &Person::admin("root"), "macos", "MacBook").await;
    let res = app.post("/api/taste/training/claim", json!({}), &mac).await;
    assert_eq!(res.status, StatusCode::NO_CONTENT, "{}", res.text);

    // Типы интерфейса строятся из OpenAPI: версии, задания и «почему» описаны там
    let spec = app.get("/api/openapi.json", &admin).await.body;
    let schemas = &spec["components"]["schemas"];
    for name in [
        "ModelVersionView",
        "ModelTraining",
        "HeadMetrics",
        "TrainTaskView",
        "TrainClaim",
        "TrainOutcome",
        "WhyTrack",
        "WhyKind",
    ] {
        assert!(schemas[name].is_object(), "{name}");
    }
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn dataset_labels_imports_and_keeps_the_holdout_honest(db: PgPool) {
    let app = TestApp::new(db).await;
    let (id, _admin) = app.admin("root").await;
    let user: Uuid = id.parse().unwrap();
    let s = Synth::new(app.state.db.clone());
    let artist = s.artist("Imported").await;
    let a = s.track(artist, "a", Feat::default()).await;
    let b = s
        .track(
            artist,
            "b",
            Feat {
                axis: 300,
                ..Default::default()
            },
        )
        .await;
    let now = Utc::now();
    // Год импорта и чуть-чуть своего: «остановлено» в начале у импорта — пропуск (Spotify endplay)
    for d in 0..300 {
        let at = now - Duration::days(400 - d);
        play_at(&app.state.db, user, a, at, 175_000, "completed", "import").await;
        play_at(
            &app.state.db,
            user,
            b,
            at + Duration::minutes(5),
            8_000,
            "stopped",
            "import",
        )
        .await;
    }
    // Своё «остановлено» посреди трека — без метки; импорт старше года тоже в наборе
    play_at(
        &app.state.db,
        user,
        b,
        now - Duration::hours(2),
        60_000,
        "stopped",
        "stream",
    )
    .await;
    // Лайк из импорта — дата импорта, позже всех прослушиваний: метка «лайк», но не признак «лайкнут»
    s.like(user, a, 0).await;

    let data = model::dataset(&app.state.db, &model::Recipe::default(), None, None)
        .await
        .unwrap();
    assert_eq!(
        data.rows(),
        600,
        "вся история, а не только год; своё «остановлено» — без метки"
    );
    let label = |i: usize| &data.set.labels[i * 3..i * 3 + 3];
    assert_eq!(label(0), [0.0, 1.0, 1.0], "дослушан и в лайках");
    assert_eq!(
        label(1),
        [1.0, 0.0, 0.0],
        "endplay в начале трека — пропуск"
    );
    let col = |i: usize, name: &str| {
        let k = model::SCALARS.iter().position(|x| *x == name).unwrap();
        data.set.scalars[i * model::SCALARS.len() + k]
    };
    assert_eq!(col(0, "liked"), 0.0, "лайк поставлен позже");
    assert!(col(3, "track_skips") > 0.0, "трек уже пропускали");
    assert_eq!(col(1, "has_hits"), 1.0, "в сессии уже зашёл трек");
    assert!(col(1, "hit_sim") < 0.5, "и он не похож на этот");
    assert!(col(2, "days_since_play") > 0.0);

    // Отложенная часть: самые поздние строки, не меньше 20 %, всё в ней позже обучающей
    let h = data.holdout_from;
    assert!(data.rows() - h >= 120 && data.rows() - h <= 240, "{h}");
    assert!(data.times[..h].iter().all(|t| *t < data.times[h]));
    let train = data.train_part();
    assert_eq!(train.rows(), h, "обучающая часть — без отложенной");
    // Свежее весит больше старого
    assert!(data.set.weights[0] < data.set.weights[data.rows() - 1]);

    // Набор одного слушателя и граница задания, выданного раньше
    let cut = data.times[h];
    let mine = model::dataset(
        &app.state.db,
        &model::Recipe::default(),
        Some(user),
        Some(cut),
    )
    .await
    .unwrap();
    assert_eq!(mine.holdout_from, h);
    let other = model::dataset(
        &app.state.db,
        &model::Recipe::default(),
        Some(Uuid::now_v7()),
        None,
    )
    .await
    .unwrap();
    assert_eq!(other.rows(), 0);
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn server_trains_checks_and_switches_on_only_when_better(db: PgPool) {
    let app = TestApp::new(db).await;
    let (id, admin) = app.admin("root").await;
    let user: Uuid = id.parse().unwrap();
    let s = Synth::new(app.state.db.clone());
    history_by_hour(&s, user, 25).await;
    // Новое рядом со вкусом: заслушанное «отдыхает», волне нужно, что предложить
    for i in 0..12u8 {
        let artist = s.artist(&format!("Fresh {}", char::from(b'a' + i))).await;
        let feat = Feat {
            axis: 0,
            jitter: 30 + usize::from(i),
            ..Default::default()
        };
        s.track(artist, &format!("fresh {}", char::from(b'a' + i)), feat)
            .await;
    }
    let settings = reco::Settings::load(&app.state.db).await.unwrap();
    reco::taste::rebuild(&app.state.db, user, &settings, false)
        .await
        .unwrap();

    let data = model::dataset(&app.state.db, &model::Recipe::default(), None, None)
        .await
        .unwrap();
    assert_eq!(data.rows(), 500);
    assert!(data.enough());

    assert!(model::request(&app.state.db).await.unwrap());
    let (_, outcome) = app.run_job("maintenance").await.unwrap();
    assert!(matches!(outcome, Outcome::Done), "{outcome:?}");
    let version = active_version(&app.state.db).await;
    assert!(
        version.starts_with("nn-"),
        "сеть видит время суток и точнее правил: {version}"
    );
    let row: (String, Value) =
        sqlx::query_as("SELECT origin, metrics FROM model_versions WHERE version = $1")
            .bind(&version)
            .fetch_one(&app.state.db)
            .await
            .unwrap();
    assert_eq!(row.0, "server");
    let m = row.1;
    assert!(m["auc"].as_f64().unwrap() > m["baselineAuc"].as_f64().unwrap() + 0.1);
    assert!(m["pBetter"].as_f64().unwrap() >= 0.9);
    // Утром всё дослушано, вечером всё пропущено: внутри сессии сравнивать нечего — `null`, но поле есть
    assert!(m.get("sessionAuc").is_some() && m.get("ndcg").is_some());
    assert!(m["note"].as_str().unwrap().starts_with("Включена"));

    // Волна теперь собирается с сетью: показы пишутся с её версией, в показе — что повлияло
    let created = app
        .post(
            "/api/wave/sessions",
            json!({ "settings": { "mood": "any", "activity": "any", "character": "mix", "language": "any" } }),
            &admin,
        )
        .await;
    assert_eq!(created.status, StatusCode::CREATED, "{}", created.text);
    let shown: Vec<(String, Value, Uuid)> = sqlx::query_as(
        "SELECT model_version, reason, track_id FROM recommendation_impressions WHERE user_id = $1",
    )
    .bind(user)
    .fetch_all(&app.state.db)
    .await
    .unwrap();
    assert!(!shown.is_empty());
    assert!(shown.iter().all(|(v, _, _)| *v == version));
    let (_, reason, track) = &shown[0];
    assert!(reason["why"]["origin"].is_string(), "{reason}");
    assert!(reason["why"]["model"].is_number(), "{reason}");

    // «Почему этот трек»: понятными словами; подробности — только администратору
    let session = created.body["id"].as_str().unwrap();
    let res = app
        .get(
            &format!("/api/wave/why?trackId={track}&sessionId={session}"),
            &admin,
        )
        .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    let lines = res.body["lines"].as_array().unwrap();
    assert!(!lines.is_empty() && lines.len() <= 4);
    for l in lines {
        let text = l["text"].as_str().unwrap();
        assert!(
            !text.chars().any(|c| c.is_ascii_digit()),
            "без чисел: {text}"
        );
    }
    assert!(res.body["details"]["why"].is_object());
    let res = app
        .get(&format!("/api/wave/why?trackId={}", Uuid::now_v7()), &admin)
        .await;
    assert_eq!(res.status, StatusCode::NOT_FOUND);

    let res = app.get("/api/admin/reco", &admin).await;
    let models = res.body["models"].as_array().unwrap();
    assert_eq!(models[0]["version"], version.as_str());
    assert_eq!(models[0]["origin"], "server");
    assert_eq!(models[0]["active"], true);
    assert_eq!(models[0]["heads"].as_array().unwrap().len(), 3);
    assert!(models[0].get("sessionAuc").is_some());

    // Сломанная версия не роняет волну: подбор остаётся на правилах
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
    assert!(
        model::active(&app.state.db, Some(user))
            .await
            .unwrap()
            .is_none()
    );
    let created = app
        .post(
            "/api/wave/sessions",
            json!({ "settings": { "mood": "calm", "activity": "any", "character": "mix", "language": "any" } }),
            &admin,
        )
        .await;
    assert_eq!(created.status, StatusCode::CREATED, "{}", created.text);
}

/// Задание с компьютера: набор → обучение → веса.
fn train_package(claim: &Value) -> (dalywave_taste::Params, dalywave_taste::TrainReport) {
    let pkg: wire::Package = serde_json::from_value(claim["package"].clone()).unwrap();
    let (spec, set) = pkg.decode().unwrap();
    assert_eq!(pkg.feature_names, model::SCALARS.map(String::from).to_vec());
    dalywave_taste::train(&set, &spec).unwrap()
}

async fn submit(
    app: &TestApp,
    auth: &Auth,
    claim: &Value,
    params: &dalywave_taste::Params,
) -> crate::common::Resp {
    app.post(
        &format!(
            "/api/taste/training/{}/result",
            claim["taskId"].as_str().unwrap()
        ),
        json!({ "params": wire::WireParams::new(params), "report": {} }),
        auth,
    )
    .await
}

/// Задание снова «пора»: прошлое — давно.
async fn age_tasks(db: &PgPool) {
    sqlx::query(
        "UPDATE taste_train_tasks SET leased_at = leased_at - interval '2 days',
                finished_at = finished_at - interval '2 days'",
    )
    .execute(db)
    .await
    .unwrap();
    sqlx::query("UPDATE model_versions SET created_at = created_at - interval '2 days'")
        .execute(db)
        .await
        .unwrap();
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn computers_train_and_the_server_decides(db: PgPool) {
    let app = TestApp::new(db).await;
    let (id, admin) = app.admin("root").await;
    let user: Uuid = id.parse().unwrap();
    let s = Synth::new(app.state.db.clone());
    history_by_hour(&s, user, 25).await;
    let mac = app_token(&app, &Person::admin("root"), "macos", "MacBook").await;

    // Администратор по умолчанию учит общую сеть: в наборе нет отложенной части
    let res = app.post("/api/taste/training/claim", json!({}), &mac).await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    let claim = res.body;
    assert_eq!(claim["scope"], "global");
    let data = model::dataset(&app.state.db, &model::Recipe::default(), None, None)
        .await
        .unwrap();
    assert_eq!(claim["rows"], data.holdout_from as i64);
    assert_eq!(claim["package"]["rows"], data.holdout_from as i64);
    // Пока задание у компьютера, второго не дают
    let res = app.post("/api/taste/training/claim", json!({}), &mac).await;
    assert_eq!(res.status, StatusCode::NO_CONTENT);

    let (params, _) = train_package(&claim);
    // Вывод присланных весов на сервере — тот же, что на компьютере
    let onnx = dalywave_taste::onnx::export(&params);
    let loaded = model::Loaded::build(Uuid::nil(), "probe", &onnx).unwrap();
    let rows = data.set.features(0..8);
    let ours = params.forward(&rows, 8);
    let theirs = loaded.predict(rows, 8).unwrap();
    for (a, b) in ours.iter().zip(&theirs) {
        for k in 0..3 {
            assert!((a[k] - b[k]).abs() < 1e-5, "{a:?} {b:?}");
        }
    }

    let res = submit(&app, &mac, &claim, &params).await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    assert_eq!(res.body["active"], true, "{}", res.body);
    let good = res.body["version"].as_str().unwrap().to_string();
    assert_eq!(active_version(&app.state.db).await, good);
    let origin: (String, Option<String>) =
        sqlx::query_as("SELECT origin, device_name FROM model_versions WHERE version = $1")
            .bind(&good)
            .fetch_one(&app.state.db)
            .await
            .unwrap();
    assert_eq!(origin, ("device".into(), Some("MacBook".into())));
    // Дважды одно задание не проверяется
    let res = submit(&app, &mac, &claim, &params).await;
    assert_eq!(res.status, StatusCode::CONFLICT);

    // Плохие веса (сеть наоборот) — записаны, но не включены: включённая остаётся
    age_tasks(&app.state.db).await;
    let res = app
        .post(
            "/api/taste/training/claim",
            json!({ "scope": "global" }),
            &mac,
        )
        .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    let claim = res.body;
    let (mut bad, _) = train_package(&claim);
    let last = bad.layers.last_mut().unwrap();
    last.w.iter_mut().for_each(|w| *w = -*w);
    last.b.iter_mut().for_each(|b| *b = -*b);
    let res = submit(&app, &mac, &claim, &bad).await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    assert_eq!(res.body["active"], false);
    assert!(
        res.body["note"]
            .as_str()
            .unwrap()
            .starts_with("Не включена")
    );
    assert_eq!(active_version(&app.state.db).await, good);

    // Мусор вместо весов — 422, задание закрыто
    age_tasks(&app.state.db).await;
    let claim = app
        .post("/api/taste/training/claim", json!({}), &mac)
        .await
        .body;
    let mut junk = serde_json::to_value(wire::WireParams::new(&params)).unwrap();
    junk["layers"][0]["w"] = json!("AAAA");
    let res = app
        .post(
            &format!(
                "/api/taste/training/{}/result",
                claim["taskId"].as_str().unwrap()
            ),
            json!({ "params": junk, "report": {} }),
            &mac,
        )
        .await;
    assert_eq!(res.status, StatusCode::UNPROCESSABLE_ENTITY, "{}", res.text);

    // Слушатель: общую сеть не учит и не присылает; свою — если попросит
    let listener_web = app.listener("anna").await;
    let _ = listener_web;
    let anna = app_token(&app, &Person::new("anna"), "windows", "DESKTOP").await;
    let res = app
        .post(
            "/api/taste/training/claim",
            json!({ "scope": "global" }),
            &anna,
        )
        .await;
    assert_eq!(res.status, StatusCode::FORBIDDEN, "{}", res.text);
    let res = app
        .post("/api/taste/training/claim", json!({}), &anna)
        .await;
    assert_eq!(
        res.status,
        StatusCode::NO_CONTENT,
        "по умолчанию слушатели не учат"
    );
    // Задание администратора — не её
    age_tasks(&app.state.db).await;
    let admin_claim = app
        .post("/api/taste/training/claim", json!({}), &mac)
        .await
        .body;
    let res = submit(&app, &anna, &admin_claim, &params).await;
    assert_eq!(res.status, StatusCode::NOT_FOUND);
    // Телефон не учит
    let phone = app_token(&app, &Person::admin("root"), "ios", "iPhone").await;
    let res = app
        .post("/api/taste/training/claim", json!({}), &phone)
        .await;
    assert_eq!(res.status, StatusCode::FORBIDDEN);
    // Сайт тоже
    let res = app
        .post("/api/taste/training/claim", json!({}), &admin)
        .await;
    assert!(res.status.is_client_error(), "{}", res.status);

    // Админка: версии с компьютера и задания
    let res = app.get("/api/admin/reco", &admin).await;
    let models = res.body["models"].as_array().unwrap();
    assert!(
        models
            .iter()
            .any(|m| m["origin"] == "device" && m["deviceName"] == "MacBook")
    );
    let tasks = res.body["training"]["deviceTasks"].as_array().unwrap();
    assert!(tasks.len() >= 3);
    assert!(tasks.iter().any(|t| t["status"] == "failed"));
}
