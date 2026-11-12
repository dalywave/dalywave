use std::time::Duration;

use axum::http::{Method, StatusCode};
use dalywave_server::jobs::{self, Claim, NewJob, worker::Outcome};
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

use crate::common::{Auth, TestApp};

async fn enqueue(db: &PgPool, job: NewJob) -> jobs::Enqueued {
    let mut conn = db.acquire().await.unwrap();
    jobs::enqueue(&mut conn, job).await.unwrap()
}

async fn claim(db: &PgPool, lane: &str, worker: &str) -> Claim {
    jobs::claim(db, lane, worker).await.unwrap()
}

fn claimed(c: Claim) -> jobs::JobRow {
    match c {
        Claim::Job(job) => *job,
        other => panic!("expected a job, got {other:?}"),
    }
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn claiming_respects_priority_dedupe_and_lanes(db: PgPool) {
    let first = enqueue(
        &db,
        NewJob::new("probe", "mirror", json!({ "n": 1 })).dedupe("probe:1"),
    )
    .await;
    assert!(first.created);
    let again = enqueue(
        &db,
        NewJob::new("probe", "mirror", json!({ "n": 1 })).dedupe("probe:1"),
    )
    .await;
    assert_eq!(
        again,
        jobs::Enqueued {
            id: first.id,
            created: false
        }
    );
    let urgent = enqueue(
        &db,
        NewJob::new("probe", "mirror", json!({ "n": 2 })).priority(10),
    )
    .await;

    // Больший приоритет — раньше
    let job = claimed(claim(&db, "mirror", "w1").await);
    assert_eq!(job.id, urgent.id);
    assert_eq!(job.status, "running");
    assert_eq!(job.attempts, 1);
    assert_eq!(job.locked_by.as_deref(), Some("w1"));
    let job2 = claimed(claim(&db, "mirror", "w2").await);
    assert_eq!(job2.id, first.id);
    assert!(matches!(
        claim(&db, "mirror", "w1").await,
        Claim::Empty(None)
    ));
    // Пока задача в работе, тот же ключ не ставится повторно
    let dup = enqueue(
        &db,
        NewJob::new("probe", "mirror", json!({})).dedupe("probe:1"),
    )
    .await;
    assert!(!dup.created);
    assert!(
        jobs::complete(&db, job2.id, "w2", json!({ "ok": true }))
            .await
            .unwrap()
            .is_some()
    );
    // Закончилась — можно снова
    assert!(
        enqueue(
            &db,
            NewJob::new("probe", "mirror", json!({})).dedupe("probe:1")
        )
        .await
        .created
    );
    // Чужой исполнитель не может закрыть задачу
    assert!(
        jobs::complete(&db, job.id, "intruder", json!({}))
            .await
            .unwrap()
            .is_none()
    );
    assert!(matches!(
        claim(&db, "nonexistent", "w1").await,
        Claim::Paused
    ));
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn lanes_limit_concurrency_interval_and_pause(db: PgPool) {
    // discography: параллельность 1, пауза 8 с между стартами
    for n in 0..3 {
        enqueue(&db, NewJob::new("probe", "discography", json!({ "n": n }))).await;
    }
    let first = claimed(claim(&db, "discography", "w1").await);
    assert!(matches!(claim(&db, "discography", "w2").await, Claim::Full));
    jobs::complete(&db, first.id, "w1", json!({}))
        .await
        .unwrap();
    match claim(&db, "discography", "w2").await {
        Claim::Wait(d) => assert!(
            d > Duration::from_secs(6) && d <= Duration::from_secs(8),
            "{d:?}"
        ),
        other => panic!("expected Wait, got {other:?}"),
    }
    sqlx::query("UPDATE job_lanes SET last_claimed_at = now() - interval '9 seconds' WHERE lane = 'discography'")
        .execute(&db)
        .await
        .unwrap();
    let second = claimed(claim(&db, "discography", "w2").await);

    // Пауза полосы: задачи стоят
    sqlx::query("UPDATE job_lanes SET paused = true, min_interval_ms = 0, concurrency = 2 WHERE lane = 'discography'")
        .execute(&db)
        .await
        .unwrap();
    assert!(matches!(
        claim(&db, "discography", "w3").await,
        Claim::Paused
    ));
    sqlx::query("UPDATE job_lanes SET paused = false WHERE lane = 'discography'")
        .execute(&db)
        .await
        .unwrap();
    // Параллельность 2: вторая задача берётся, пока первая идёт
    let third = claimed(claim(&db, "discography", "w3").await);
    assert_ne!(third.id, second.id);
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn failures_back_off_and_stale_jobs_are_reaped(db: PgPool) {
    let job = enqueue(
        &db,
        NewJob::new("probe", "mirror", json!({})).max_attempts(2),
    )
    .await;
    let running = claimed(claim(&db, "mirror", "w1").await);
    let failed = jobs::fail(&db, running.id, "w1", "сеть", true)
        .await
        .unwrap()
        .unwrap();
    // Первая неудача — обратно в очередь, но не сразу
    assert_eq!(failed.status, "queued");
    assert_eq!(failed.last_error.as_deref(), Some("сеть"));
    let delay = failed.run_after - chrono::Utc::now();
    assert!(
        delay > chrono::Duration::seconds(20) && delay < chrono::Duration::seconds(40),
        "{delay}"
    );
    match claim(&db, "mirror", "w1").await {
        Claim::Empty(Some(d)) => assert!(d > Duration::from_secs(20)),
        other => panic!("expected Empty with a delay, got {other:?}"),
    }
    sqlx::query("UPDATE jobs SET run_after = now() WHERE id = $1")
        .bind(job.id)
        .execute(&db)
        .await
        .unwrap();
    let running = claimed(claim(&db, "mirror", "w1").await);
    assert_eq!(running.attempts, 2);
    // Попытки кончились — failed
    let failed = jobs::fail(&db, running.id, "w1", "снова сеть", true)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(failed.status, "failed");
    assert!(failed.finished_at.is_some());

    // Окончательная ошибка — без повторов
    enqueue(&db, NewJob::new("probe", "mirror", json!({}))).await;
    let running = claimed(claim(&db, "mirror", "w1").await);
    let failed = jobs::fail(&db, running.id, "w1", "нет такого", false)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(failed.status, "failed");
    assert_eq!(failed.attempts, 1);

    // Исполнитель пропал: без пульса задача возвращается в очередь
    let lost = enqueue(&db, NewJob::new("probe", "mirror", json!({}))).await;
    claimed(claim(&db, "mirror", "dead").await);
    assert!(jobs::heartbeat(&db, lost.id, "dead").await.unwrap());
    assert!(
        jobs::reap(&db, Some(Duration::from_secs(120)))
            .await
            .unwrap()
            .is_empty()
    );
    sqlx::query("UPDATE jobs SET heartbeat_at = now() - interval '5 minutes' WHERE id = $1")
        .bind(lost.id)
        .execute(&db)
        .await
        .unwrap();
    let reaped = jobs::reap(&db, Some(Duration::from_secs(120)))
        .await
        .unwrap();
    assert_eq!(reaped.len(), 1);
    assert_eq!(reaped[0].status, "queued");
    assert!(
        !jobs::heartbeat(&db, lost.id, "dead").await.unwrap(),
        "задача больше не его"
    );
    // Остановка воркера возвращает недоделанное без траты попытки
    let again = claimed(claim(&db, "mirror", "w2").await);
    assert_eq!(again.attempts, 2);
    assert_eq!(jobs::release(&db, "w2").await.unwrap().len(), 1);
    let row = jobs::get(&db, again.id).await.unwrap().unwrap();
    assert_eq!((row.status.as_str(), row.attempts), ("queued", 1));
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn worker_runs_handlers_and_closes_unknown_kinds(db: PgPool) {
    let app = TestApp::new(db).await;
    // Вид без обработчика закрывается сразу и без повторов
    let job = enqueue(
        &app.state.db,
        NewJob::new("teleport", "maintenance", json!({})),
    )
    .await;
    let (kind, outcome) = app.run_job("maintenance").await.unwrap();
    assert_eq!(kind, "teleport");
    assert_eq!(
        outcome,
        Outcome::Failed("обработчик задач «teleport» ещё не реализован".into())
    );
    let row = jobs::get(&app.state.db, job.id).await.unwrap().unwrap();
    assert_eq!(row.status, "failed");
    // Неверные данные задачи — окончательная ошибка
    let bad = enqueue(
        &app.state.db,
        NewJob::new("refresh_artist", "maintenance", json!({ "x": 1 })),
    )
    .await;
    let (_, outcome) = app.run_job("maintenance").await.unwrap();
    assert!(
        matches!(outcome, Outcome::Failed(ref m) if m.contains("неверные данные")),
        "{outcome:?}"
    );
    assert_eq!(
        jobs::get(&app.state.db, bad.id)
            .await
            .unwrap()
            .unwrap()
            .attempts,
        1
    );
    assert!(app.run_job("maintenance").await.is_none());
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn admin_manages_jobs_and_lanes(db: PgPool) {
    let app = TestApp::new(db).await;
    let (_, admin) = app.admin("boss").await;
    let a = enqueue(
        &app.state.db,
        NewJob::new("probe", "mirror", json!({ "n": 1 })),
    )
    .await;
    let b = enqueue(
        &app.state.db,
        NewJob::new("other", "maintenance", json!({ "n": 2 })),
    )
    .await;
    let running = claimed(claim(&app.state.db, "mirror", "w1").await);
    jobs::fail(&app.state.db, running.id, "w1", "сломалось", false)
        .await
        .unwrap();

    let res = app.get("/api/admin/jobs", &admin).await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    assert_eq!(res.body["items"].as_array().unwrap().len(), 2);
    assert_eq!(
        res.body["items"][0]["id"],
        b.id.to_string(),
        "новые первыми"
    );
    let res = app
        .get("/api/admin/jobs?status=failed&lane=mirror", &admin)
        .await;
    let items = res.body["items"].as_array().unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["lastError"], "сломалось");
    assert_eq!(items[0]["payload"], json!({ "n": 1 }));
    let res = app.get("/api/admin/jobs?kind=other", &admin).await;
    assert_eq!(res.body["items"][0]["id"], b.id.to_string());
    let page = app.get("/api/admin/jobs?limit=1", &admin).await;
    let cursor = page.body["nextCursor"].as_str().unwrap();
    let next = app
        .get(&format!("/api/admin/jobs?limit=1&cursor={cursor}"), &admin)
        .await;
    assert_eq!(next.body["items"][0]["id"], a.id.to_string());
    assert!(next.body["nextCursor"].is_null());

    // Повтор неудачной: в очередь с нуля
    let res = app
        .call(
            Method::POST,
            &format!("/api/admin/jobs/{}/retry", a.id),
            None,
            &admin,
        )
        .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    assert_eq!(res.body["status"], "queued");
    assert_eq!(res.body["attempts"], 0);
    let res = app
        .call(
            Method::POST,
            &format!("/api/admin/jobs/{}/retry", a.id),
            None,
            &admin,
        )
        .await;
    assert_eq!(res.status, StatusCode::CONFLICT);
    assert_eq!(res.code(), "job_active");
    // Отмена из очереди; закончившуюся отменить нельзя
    let res = app
        .call(
            Method::POST,
            &format!("/api/admin/jobs/{}/cancel", b.id),
            None,
            &admin,
        )
        .await;
    assert_eq!(res.status, StatusCode::OK);
    assert_eq!(res.body["status"], "cancelled");
    let res = app
        .call(
            Method::POST,
            &format!("/api/admin/jobs/{}/cancel", b.id),
            None,
            &admin,
        )
        .await;
    assert_eq!(res.code(), "job_finished");
    let missing = Uuid::now_v7();
    for action in ["retry", "cancel"] {
        let res = app
            .call(
                Method::POST,
                &format!("/api/admin/jobs/{missing}/{action}"),
                None,
                &admin,
            )
            .await;
        assert_eq!(res.status, StatusCode::NOT_FOUND, "{action}");
    }
    // Повтор, когда такая же задача уже в очереди, — конфликт
    let d1 = enqueue(
        &app.state.db,
        NewJob::new("probe", "library", json!({})).dedupe("same"),
    )
    .await;
    app.call(
        Method::POST,
        &format!("/api/admin/jobs/{}/cancel", d1.id),
        None,
        &admin,
    )
    .await;
    enqueue(
        &app.state.db,
        NewJob::new("probe", "library", json!({})).dedupe("same"),
    )
    .await;
    let res = app
        .call(
            Method::POST,
            &format!("/api/admin/jobs/{}/retry", d1.id),
            None,
            &admin,
        )
        .await;
    assert_eq!(res.code(), "job_duplicate");

    // Идущую задачу отмена прерывает: пульс исполнителя видит, что она больше не его
    let running = claimed(claim(&app.state.db, "mirror", "w9").await);
    app.call(
        Method::POST,
        &format!("/api/admin/jobs/{}/cancel", running.id),
        None,
        &admin,
    )
    .await;
    assert!(
        !jobs::heartbeat(&app.state.db, running.id, "w9")
            .await
            .unwrap()
    );

    // Полосы
    let res = app.get("/api/admin/lanes", &admin).await;
    assert_eq!(res.status, StatusCode::OK);
    let lanes = res.body["items"].as_array().unwrap();
    assert_eq!(
        lanes.len(),
        12,
        "девять полос очереди, «Импорт», «Тексты» и «Сжатие для сотовой»"
    );
    assert_eq!(lanes[0]["lane"], "now");
    assert_eq!(lanes[0]["title"], "Сейчас");
    let res = app
        .call(
            Method::PATCH,
            "/api/admin/lanes/discography",
            Some(json!({ "paused": true, "concurrency": 2, "minIntervalMs": 15000 })),
            &admin,
        )
        .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    assert_eq!(res.body["paused"], true);
    assert_eq!(res.body["concurrency"], 2);
    assert_eq!(res.body["minIntervalMs"], 15000);
    for (lane, body) in [
        ("discography", json!({ "concurrency": 0 })),
        ("discography", json!({ "minIntervalMs": -1 })),
        ("discography", json!({ "speed": 1 })),
    ] {
        let res = app
            .call(
                Method::PATCH,
                &format!("/api/admin/lanes/{lane}"),
                Some(body.clone()),
                &admin,
            )
            .await;
        assert_eq!(res.status, StatusCode::BAD_REQUEST, "{body}");
    }
    let res = app
        .call(
            Method::PATCH,
            "/api/admin/lanes/nope",
            Some(json!({ "paused": true })),
            &admin,
        )
        .await;
    assert_eq!(res.status, StatusCode::NOT_FOUND);
    let kinds = app.audit_kinds().await;
    for kind in ["job.retried", "job.cancelled", "lane.changed"] {
        assert!(kinds.iter().any(|k| k == kind), "{kind}");
    }

    enqueue(&app.state.db, NewJob::new("probe", "scout", json!({}))).await;
    let broken = claimed(claim(&app.state.db, "scout", "w1").await);
    jobs::fail(&app.state.db, broken.id, "w1", "насовсем", false)
        .await
        .unwrap();
    let res = app.get("/api/admin/overview", &admin).await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    assert_eq!(res.body["recordingsByStatus"].as_array().unwrap().len(), 7);
    assert_eq!(res.body["lanes"].as_array().unwrap().len(), 12);
    assert!(res.body["last24h"]["jobsFailed"].as_i64().unwrap() >= 1);
    assert_eq!(res.body["storage"]["audioBytes"], 0);
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn queue_admin_is_guarded(db: PgPool) {
    let app = TestApp::new(db).await;
    let listener = app.listener("lis").await;
    let id = Uuid::now_v7();
    let requests = [
        (Method::GET, "/api/admin/overview".to_string()),
        (Method::GET, "/api/admin/jobs".to_string()),
        (Method::POST, format!("/api/admin/jobs/{id}/retry")),
        (Method::POST, format!("/api/admin/jobs/{id}/cancel")),
        (Method::GET, "/api/admin/lanes".to_string()),
        (Method::PATCH, "/api/admin/lanes/now".to_string()),
    ];
    for (method, uri) in &requests {
        let body = (method == Method::PATCH).then(|| json!({ "paused": true }));
        let res = app
            .call(method.clone(), uri, body.clone(), &Auth::None)
            .await;
        assert_eq!(res.status, StatusCode::UNAUTHORIZED, "{method} {uri}");
        let res = app.call(method.clone(), uri, body, &listener).await;
        assert_eq!(res.status, StatusCode::FORBIDDEN, "{method} {uri}");
        assert_eq!(res.code(), "forbidden");
    }
}
