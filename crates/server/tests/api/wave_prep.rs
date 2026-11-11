//! Подготовка волны против API: начало подбирается под настройки по всему индексу (а не из скачанного),
//! неготовое качается вне очереди, сессия `preparing → ready` по мере отчётов загрузчика; что не скачалось —
//! заменяется; качать некому или срок вышел — начало из лучшего готового. Заготовки под частые настройки
//! и мгновенный старт с них.

use axum::http::StatusCode;
use dalywave_server::reco::{self, prep, taste};
use serde_json::{Value, json};
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    common::{Auth, TestApp},
    fetcher::{claimed, new_downloader, no_intervals, report, upload, verified},
    synth::{Feat, Synth},
};

/// Каталог, в котором скачанное не подходит «Работаю · Спокойное · Незнакомое»: любимое и громкое
/// уже на сервере, спокойное слышанное — тоже; спокойное незнакомое — только в индексе.
struct Catalog {
    liked: Vec<Uuid>,
    loud_ready: Vec<Uuid>,
    calm_heard: Vec<Uuid>,
    calm_new: Vec<Uuid>,
}

async fn catalog(app: &TestApp, user: Uuid) -> Catalog {
    let s = Synth::new(app.state.db.clone());
    let mut liked = Vec::new();
    for i in 0..4 {
        let a = s.artist(&format!("Fav {i}")).await;
        let t = s
            .track(
                a,
                &format!("fav {i}"),
                Feat {
                    jitter: i,
                    energy: 0.8,
                    status: "ready",
                    ..Default::default()
                },
            )
            .await;
        s.like(user, t, 10).await;
        liked.push(t);
    }
    let mut loud_ready = Vec::new();
    for i in 0..6 {
        let a = s.artist(&format!("Loud {i}")).await;
        loud_ready.push(
            s.track(
                a,
                &format!("loud {i}"),
                Feat {
                    jitter: 10 + i,
                    energy: 0.9,
                    bpm: 128.0,
                    status: "ready",
                    ..Default::default()
                },
            )
            .await,
        );
    }
    let mut calm_heard = Vec::new();
    for i in 0..3 {
        let a = s.artist(&format!("Heard {i}")).await;
        let t = s
            .track(
                a,
                &format!("heard {i}"),
                Feat {
                    jitter: 20 + i,
                    energy: 0.3,
                    bpm: 100.0,
                    status: "ready",
                    ..Default::default()
                },
            )
            .await;
        s.play(user, t, 72, true).await;
        calm_heard.push(t);
    }
    let mut calm_new = Vec::new();
    for i in 0..12 {
        let a = s.artist(&format!("Calm {i}")).await;
        calm_new.push(
            s.track(
                a,
                &format!("calm {i}"),
                Feat {
                    jitter: 30 + i,
                    energy: 0.25 + 0.02 * i as f32,
                    bpm: 96.0,
                    ..Default::default()
                },
            )
            .await,
        );
    }
    let settings = reco::Settings::load(&app.state.db).await.unwrap();
    taste::rebuild(&app.state.db, user, &settings, false)
        .await
        .unwrap();
    Catalog {
        liked,
        loud_ready,
        calm_heard,
        calm_new,
    }
}

const WORK_CALM_NEW: &str = r#"{ "activity": "work", "mood": "calm", "character": "new" }"#;

async fn create(app: &TestApp, auth: &Auth, settings: &str) -> Value {
    let settings: Value = serde_json::from_str(settings).unwrap();
    let res = app
        .post("/api/wave/sessions", json!({ "settings": settings }), auth)
        .await;
    assert_eq!(res.status, StatusCode::CREATED, "{}", res.text);
    res.body
}

async fn get(app: &TestApp, auth: &Auth, id: &str) -> Value {
    let res = app.get(&format!("/api/wave/sessions/{id}"), auth).await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    res.body
}

fn ids(v: &Value) -> Vec<Uuid> {
    v["queue"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["track"]["id"].as_str().unwrap().parse().unwrap())
        .collect()
}

fn statuses(v: &Value) -> Vec<String> {
    v["queue"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["track"]["audioStatus"].as_str().unwrap().to_string())
        .collect()
}

async fn now_lane(app: &TestApp) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM acquisitions WHERE lane = 'now' AND reason = 'wave'")
        .fetch_one(&app.state.db)
        .await
        .unwrap()
}

/// Загрузчик на связи — обращался только что.
async fn downloader_online(app: &TestApp) -> String {
    let (_, admin) = app.admin("boss").await;
    let (token, _) = new_downloader(app, &admin, "Mac").await;
    sqlx::query("UPDATE downloaders SET last_seen_at = now()")
        .execute(&app.state.db)
        .await
        .unwrap();
    token
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn wave_is_prepared_for_its_settings_before_it_starts(db: PgPool) {
    let app = TestApp::new(db).await;
    no_intervals(&app.state.db).await;
    let token = downloader_online(&app).await;
    let me = app.listener("ann").await;
    let user: Uuid = app.user_id(&me).await.parse().unwrap();
    let c = catalog(&app, user).await;

    // Начало — спокойное незнакомое из индекса, а не скачанное любимое, громкое или слышанное
    let body = create(&app, &me, WORK_CALM_NEW).await;
    assert_eq!(body["state"], "preparing", "{body}");
    assert_eq!(body["need"], 3);
    assert_eq!(body["ready"], 0);
    let queue = ids(&body);
    assert_eq!(queue.len(), prep::QUEUE);
    assert!(queue.iter().all(|t| c.calm_new.contains(t)), "{queue:?}");
    for off in c.liked.iter().chain(&c.loud_ready).chain(&c.calm_heard) {
        assert!(!queue.contains(off));
    }
    for item in body["queue"].as_array().unwrap() {
        assert!(item["energy"].as_f64().unwrap() <= 0.5);
    }
    // Лучшее неготовое — вне очереди (с запасом в один трек), остальное — в полосе «Волна»
    assert_eq!(now_lane(&app).await, 4);
    let id = body["id"].as_str().unwrap().to_string();
    assert_eq!(get(&app, &me, &id).await["state"], "preparing");

    // Первое не нашлось в источниках — его место занимает следующее подходящее, тоже вне очереди
    let job = claimed(&app, &token).await;
    let res = report(
        &app,
        &token,
        &job,
        json!({ "status": "not_found", "deezerTrackId": job["deezerTrackId"], "candidates": [] }),
    )
    .await;
    assert_eq!(res.status, StatusCode::NO_CONTENT, "{}", res.text);
    let body = get(&app, &me, &id).await;
    assert_eq!(body["state"], "preparing");
    let queue = ids(&body);
    assert_eq!(queue.len(), prep::QUEUE, "замена вместо потерянного");
    assert!(queue.iter().all(|t| c.calm_new.contains(t)));
    assert!(!statuses(&body).contains(&"unavailable".to_string()));
    assert_eq!(now_lane(&app).await, 5);

    // Загрузчик скачивает: готово k из 3, на третьем — можно играть, готовое — первым
    for k in 1..=3u64 {
        let job = claimed(&app, &token).await;
        let url = job["upload"]["url"].as_str().unwrap().to_string();
        upload(&app.state.storage, &url, &[7u8; 4000]).await;
        let res = report(
            &app,
            &token,
            &job,
            verified(&job["deezerTrackId"], 4000, &format!("vid{k}aaaaaa")),
        )
        .await;
        assert_eq!(res.status, StatusCode::NO_CONTENT, "{}", res.text);
        let body = get(&app, &me, &id).await;
        assert_eq!(body["ready"], k);
        assert_eq!(
            body["state"],
            if k < 3 { "preparing" } else { "ready" },
            "{k}"
        );
    }
    let body = get(&app, &me, &id).await;
    assert_eq!(&statuses(&body)[..3], &["ready", "ready", "ready"]);
    // Начало выдано: следующая выдача его не повторяет
    let res = app
        .get(&format!("/api/wave/sessions/{id}/next?count=4"), &me)
        .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    let first = ids(&body);
    for item in res.body["items"].as_array().unwrap() {
        let t: Uuid = item["track"]["id"].as_str().unwrap().parse().unwrap();
        assert!(!first.contains(&t));
    }
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn without_downloader_or_in_time_the_wave_starts_with_the_best_ready(db: PgPool) {
    let app = TestApp::new(db).await;
    let me = app.listener("ann").await;
    let user: Uuid = app.user_id(&me).await.parse().unwrap();
    let c = catalog(&app, user).await;
    // Два спокойных незнакомых уже на сервере
    sqlx::query(
        "UPDATE recordings SET audio_status = 'ready'
         WHERE id IN (SELECT recording_id FROM tracks WHERE id = ANY($1))",
    )
    .bind(&c.calm_new[10..])
    .execute(&app.state.db)
    .await
    .unwrap();

    // Качать некому — ждать нечего: сразу готово, начало — из того, что на сервере, лучшее по настройкам
    let body = create(&app, &me, WORK_CALM_NEW).await;
    assert_eq!(body["state"], "ready", "{body}");
    let queue = ids(&body);
    assert!(!queue.is_empty());
    assert!(statuses(&body).iter().all(|s| s == "ready"));
    assert!(c.calm_new[10..].contains(&queue[0]), "{queue:?}");
    assert!(c.calm_new[10..].contains(&queue[1]), "{queue:?}");
    assert_eq!(now_lane(&app).await, 0);

    // Загрузчик на связи, но за полторы минуты ничего не скачал: начало — из готового
    let _token = downloader_online(&app).await;
    sqlx::query("UPDATE recordings SET audio_status = 'none' WHERE audio_status = 'ready' AND id IN (SELECT recording_id FROM tracks WHERE id = ANY($1))")
        .bind(&c.calm_new[10..])
        .execute(&app.state.db)
        .await
        .unwrap();
    let body = create(&app, &me, WORK_CALM_NEW).await;
    assert_eq!(body["state"], "preparing");
    let id = body["id"].as_str().unwrap().to_string();
    let first = ids(&body);
    sqlx::query(
        "UPDATE wave_sessions SET state = jsonb_set(state, '{prep,deadline}', to_jsonb(now() - interval '1 second'))
         WHERE id = $1::uuid",
    )
    .bind(&id)
    .execute(&app.state.db)
    .await
    .unwrap();
    let body = get(&app, &me, &id).await;
    assert_eq!(body["state"], "ready");
    let queue = ids(&body);
    let st = statuses(&body);
    let ready: Vec<&Uuid> = queue
        .iter()
        .zip(&st)
        .filter(|(_, s)| *s == "ready")
        .map(|(t, _)| t)
        .collect();
    assert!(!ready.is_empty() && st[0] == "ready", "{st:?}");
    // Готовое добрано из лучшего на сервере; подобранное под настройки — следом, докачается
    assert!(first.iter().all(|t| queue.contains(t)));
    assert!(ready.iter().all(|t| !first.contains(t)));
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn pools_make_the_next_start_instant(db: PgPool) {
    let app = TestApp::new(db).await;
    let token = downloader_online(&app).await;
    let me = app.listener("ann").await;
    let user: Uuid = app.user_id(&me).await.parse().unwrap();
    let c = catalog(&app, user).await;
    // Человек слушал «Работаю · Спокойное · Незнакомое»
    let body = create(&app, &me, WORK_CALM_NEW).await;
    let served = ids(&body);
    let s = Synth::new(app.state.db.clone());
    for t in &served {
        s.play(user, *t, 1, true).await;
    }
    sqlx::query("UPDATE acquisitions SET status = 'cancelled', finished_at = now()")
        .execute(&app.state.db)
        .await
        .unwrap();
    sqlx::query("UPDATE jobs SET status = 'cancelled' WHERE kind = 'acquire'")
        .execute(&app.state.db)
        .await
        .unwrap();
    sqlx::query("UPDATE recordings SET audio_status = 'none' WHERE audio_status = 'queued'")
        .execute(&app.state.db)
        .await
        .unwrap();

    let report_json = prep::refill_pools(&app.state.db, &app.state.deezer, false)
        .await
        .unwrap();
    assert_eq!(report_json["users"], 1);
    let rows: Vec<(String, Uuid)> =
        sqlx::query_as("SELECT combo, track_id FROM wave_pools ORDER BY combo, picked_at")
            .fetch_all(&app.state.db)
            .await
            .unwrap();
    let combo = prep::combo_key(&serde_json::from_str(WORK_CALM_NEW).unwrap());
    let mine: Vec<Uuid> = rows
        .iter()
        .filter(|(k, _)| *k == combo)
        .map(|(_, t)| *t)
        .collect();
    assert!(
        !mine.is_empty() && mine.len() <= prep::POOL_SIZE,
        "{rows:?}"
    );
    // Заготовка — под настройки: незнакомое спокойное, не слышанное
    assert!(mine.iter().all(|t| c.calm_new.contains(t)), "{mine:?}");
    assert!(mine.iter().all(|t| !served.contains(t)));
    assert!(
        rows.iter().any(|(k, _)| k == "any:mix:any:any"),
        "«Моя волна»"
    );
    // Неготовое — в полосе «Открытия», в пределах суток
    let queued: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM acquisitions WHERE lane = 'discovery' AND reason = 'wave' AND status = 'queued'",
    )
    .fetch_one(&app.state.db)
    .await
    .unwrap();
    assert!(queued > 0 && queued <= prep::POOL_DAILY_CAP);
    assert_eq!(report_json["queued"], queued);

    // Загрузчик скачал заготовку — следующий старт сразу готов, с ней в начале
    sqlx::query(
        "UPDATE recordings SET audio_status = 'ready'
         WHERE id IN (SELECT recording_id FROM tracks WHERE id = ANY($1))",
    )
    .bind(&mine)
    .execute(&app.state.db)
    .await
    .unwrap();
    let body = create(&app, &me, WORK_CALM_NEW).await;
    assert_eq!(body["state"], "ready", "{body}");
    let queue = ids(&body);
    let pooled = queue.iter().filter(|t| mine.contains(t)).count();
    assert!(pooled >= mine.len().min(3), "{queue:?} vs {mine:?}");
    // Взятое ушло из заготовок; услышанное и устаревшее уходит при пересчёте
    let left: i64 = sqlx::query_scalar("SELECT count(*) FROM wave_pools WHERE track_id = ANY($1)")
        .bind(&queue)
        .fetch_one(&app.state.db)
        .await
        .unwrap();
    assert_eq!(left, 0);
    let rest: Vec<Uuid> =
        sqlx::query_scalar("SELECT track_id FROM wave_pools WHERE combo = 'any:mix:any:any'")
            .fetch_all(&app.state.db)
            .await
            .unwrap();
    if let Some(t) = rest.first() {
        s.play(user, *t, 0, true).await;
        prep::refill_pools(&app.state.db, &app.state.deezer, false)
            .await
            .unwrap();
        let again: i64 = sqlx::query_scalar("SELECT count(*) FROM wave_pools WHERE track_id = $1")
            .bind(t)
            .fetch_one(&app.state.db)
            .await
            .unwrap();
        assert_eq!(again, 0, "услышанное не заготавливается снова");
    }
    drop(token);
}
