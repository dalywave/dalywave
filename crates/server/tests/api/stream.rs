//! Поток трека во всех состояниях, постановка из медиатеки и решения в «Спорных».

use axum::http::{Method, StatusCode};
use dalywave_protocol::jobs::AcquireReason;
use dalywave_server::acquire::{self, Request};
use serde_json::{Value, json};
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    common::{Auth, TestApp},
    deezer::{LIGHT_ME_UP, MODERN_SADNESS},
    fetcher::{claimed, ep_tracks, new_downloader, no_intervals, report, upload, verified},
};

async fn job_of(db: &PgPool, acquisition: &str) -> (String, i16, Value) {
    sqlx::query_as(
        "SELECT j.lane, j.priority, j.payload FROM acquisitions a JOIN jobs j ON j.id = a.job_id
         WHERE a.id = $1::uuid",
    )
    .bind(acquisition)
    .fetch_one(db)
    .await
    .unwrap()
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn play_jumps_the_queue_and_reports_its_position(db: PgPool) {
    let app = TestApp::new(db).await;
    let me = app.listener("ann").await;
    let tracks = ep_tracks(&app, &me).await;

    // Запись уже стоит в очереди дискографии — запуск переносит её задачу в «Сейчас»
    let queued = acquire::enqueue(
        &app.state.db,
        Request::new(tracks[0], AcquireReason::Subscription, "discography"),
    )
    .await
    .unwrap();
    let acquire::Enqueue::Queued { acquisition_id, .. } = queued else {
        panic!("{queued:?}");
    };
    sqlx::query("UPDATE jobs SET run_after = now() + interval '1 hour' WHERE kind = 'acquire'")
        .execute(&app.state.db)
        .await
        .unwrap();
    let res = app
        .get(&format!("/api/tracks/{}/stream", tracks[0]), &me)
        .await;
    assert_eq!(res.status, StatusCode::ACCEPTED, "{}", res.text);
    assert_eq!(res.body["status"], "waiting_for_downloader");
    assert_eq!(res.body["acquisitionId"], acquisition_id.to_string());
    let (lane, priority, _) = job_of(&app.state.db, &acquisition_id.to_string()).await;
    assert_eq!((lane.as_str(), priority), ("now", 10));
    let waiting: bool =
        sqlx::query_scalar("SELECT run_after <= now() FROM jobs WHERE kind = 'acquire'")
            .fetch_one(&app.state.db)
            .await
            .unwrap();
    assert!(waiting, "запуск не ждёт отложенного повтора");

    // Второй запущенный трек — за первым
    let res = app
        .get(&format!("/api/tracks/{}/stream", tracks[1]), &me)
        .await;
    assert_eq!(res.body["queuePosition"], 1);
    let second = res.body["acquisitionId"].as_str().unwrap().to_string();
    let info = app.get(&format!("/api/acquisitions/{second}"), &me).await;
    assert_eq!(info.status, StatusCode::OK);
    assert_eq!(info.body["queuePosition"], 1);
    assert_eq!(info.body["waitingForDownloader"], true);
    assert_eq!(info.body["status"], "queued");
    assert_eq!(info.body["audioStatus"], "queued");
    // Дискография после «Сейчас» и не обгоняет
    let later = acquire::enqueue(
        &app.state.db,
        Request::new(tracks[2], AcquireReason::Subscription, "discography"),
    )
    .await
    .unwrap();
    let acquire::Enqueue::Queued { acquisition_id, .. } = later else {
        panic!("{later:?}");
    };
    let info = app
        .get(&format!("/api/acquisitions/{acquisition_id}"), &me)
        .await;
    assert_eq!(info.body["queuePosition"], 2);

    for missing in ["/api/tracks/{id}/stream", "/api/acquisitions/{id}"] {
        let uri = missing.replace("{id}", &Uuid::now_v7().to_string());
        assert_eq!(
            app.get(&uri, &me).await.status,
            StatusCode::NOT_FOUND,
            "{uri}"
        );
    }
    // Без права play — нет и потока
    let waiting = Auth::Cookie(app.login(&crate::mock::Person::new("nobody")).await);
    let res = app
        .get(&format!("/api/tracks/{}/stream", tracks[0]), &waiting)
        .await;
    assert_eq!(res.status, StatusCode::FORBIDDEN);
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn stream_states_follow_the_recording(db: PgPool) {
    let app = TestApp::new(db).await;
    let (_, admin) = app.admin("boss").await;
    let (token, _) = new_downloader(&app, &admin, "mac").await;
    let me = app.listener("ann").await;
    let track = app.resolve_id("track", LIGHT_ME_UP, &me).await;
    let stream = format!("/api/tracks/{track}/stream");

    // none → загрузчик на связи: «скачивается»
    assert_eq!(
        app.post(
            "/api/fetcher/jobs/claim",
            json!({ "workerId": "mac" }),
            &Auth::Bearer(token.clone())
        )
        .await
        .status,
        StatusCode::NO_CONTENT
    );
    let res = app.get(&stream, &me).await;
    assert_eq!(res.status, StatusCode::ACCEPTED);
    assert_eq!(res.body["status"], "acquiring");
    assert_eq!(res.body["downloadersOnline"], 1);
    // Загрузчик пропал больше двух минут назад — снова «ждём загрузчик»
    sqlx::query("UPDATE downloaders SET last_seen_at = now() - interval '3 minutes'")
        .execute(&app.state.db)
        .await
        .unwrap();
    let res = app.get(&stream, &me).await;
    assert_eq!(res.body["status"], "waiting_for_downloader");
    assert_eq!(res.body["downloadersOnline"], 0);

    // Готова — ссылка; повторный запрос даёт новую ссылку, задач не ставит
    let job = claimed(&app, &token).await;
    upload(
        &app.state.storage,
        job["upload"]["url"].as_str().unwrap(),
        &[9u8; 2048],
    )
    .await;
    let res = report(
        &app,
        &token,
        &job,
        verified(&job["deezerTrackId"], 2048, "afkL3uLNeXA"),
    )
    .await;
    assert_eq!(res.status, StatusCode::NO_CONTENT);
    let res = app.get(&stream, &me).await;
    assert_eq!(res.status, StatusCode::OK);
    assert_eq!(res.body["status"], "ready");
    assert!(res.body["expiresAt"].is_string());
    assert!(res.body["fileId"].is_string());
    let jobs: i64 = sqlx::query_scalar("SELECT count(*) FROM jobs WHERE kind = 'acquire'")
        .fetch_one(&app.state.db)
        .await
        .unwrap();
    assert_eq!(jobs, 1);
    // Тот же трек в медиатеке — новых скачиваний нет
    assert_eq!(
        app.put(&format!("/api/library/tracks/{track}"), &me)
            .await
            .status,
        StatusCode::NO_CONTENT
    );
    let acquisitions: i64 = sqlx::query_scalar("SELECT count(*) FROM acquisitions")
        .fetch_one(&app.state.db)
        .await
        .unwrap();
    assert_eq!(acquisitions, 1);
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn library_actions_queue_downloads_once_per_recording(db: PgPool) {
    let app = TestApp::new(db).await;
    let me = app.listener("ann").await;
    let tracks = ep_tracks(&app, &me).await;

    // Лайк — полоса «Медиатека»
    assert_eq!(
        app.put(&format!("/api/library/tracks/{}", tracks[3]), &me)
            .await
            .status,
        StatusCode::NO_CONTENT
    );
    let rows: Vec<(String, String, String)> = sqlx::query_as(
        "SELECT a.reason, a.lane, r.audio_status FROM acquisitions a JOIN recordings r ON r.id = a.recording_id",
    )
    .fetch_all(&app.state.db)
    .await
    .unwrap();
    assert_eq!(
        rows,
        vec![("like".into(), "library".into(), "queued".into())]
    );
    // Повторный лайк ничего не добавляет
    app.put(&format!("/api/library/tracks/{}", tracks[3]), &me)
        .await;

    // Сохранённый альбом — все его записи, каждая один раз
    let ep = app.resolve_id("album", MODERN_SADNESS, &me).await;
    assert_eq!(
        app.put(&format!("/api/library/albums/{ep}"), &me)
            .await
            .status,
        StatusCode::NO_CONTENT
    );
    let reasons: Vec<(String, i64)> =
        sqlx::query_as("SELECT reason, count(*) FROM acquisitions GROUP BY reason ORDER BY reason")
            .fetch_all(&app.state.db)
            .await
            .unwrap();
    assert_eq!(reasons, vec![("album".into(), 4), ("like".into(), 1)]);
    let lanes: Vec<String> =
        sqlx::query_scalar("SELECT DISTINCT lane FROM jobs WHERE kind = 'acquire'")
            .fetch_all(&app.state.db)
            .await
            .unwrap();
    assert_eq!(lanes, vec!["library"]);
    let dupes: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM (SELECT recording_id FROM acquisitions GROUP BY recording_id HAVING count(*) > 1) d",
    )
    .fetch_one(&app.state.db)
    .await
    .unwrap();
    assert_eq!(dupes, 0);

    // Трек без известной записи (популярное артиста) сначала загружается целиком
    let light = app.resolve_id("track", LIGHT_ME_UP, &me).await;
    sqlx::query("UPDATE tracks SET recording_id = NULL, fetched_at = NULL WHERE id = $1::uuid")
        .bind(&light)
        .execute(&app.state.db)
        .await
        .unwrap();
    assert_eq!(
        app.put(&format!("/api/library/tracks/{light}"), &me)
            .await
            .status,
        StatusCode::NO_CONTENT
    );
    let status: String = sqlx::query_scalar(
        "SELECT r.audio_status FROM tracks t JOIN recordings r ON r.id = t.recording_id WHERE t.id = $1::uuid",
    )
    .bind(&light)
    .fetch_one(&app.state.db)
    .await
    .unwrap();
    assert_eq!(status, "queued");

    // Воркер без серверного fetcher такие задачи не трогает: их ждёт загрузчик
    assert!(app.run_job("library").await.is_none());
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn admin_resolves_disputed_records(db: PgPool) {
    let app = TestApp::new(db).await;
    let (_, admin) = app.admin("boss").await;
    let (token, _) = new_downloader(&app, &admin, "mac").await;
    let track = app.resolve_id("track", LIGHT_ME_UP, &admin).await;
    no_intervals(&app.state.db).await;
    let resolve = |id: String, body: Value| {
        let app = &app;
        let admin = &admin;
        async move {
            app.post(
                &format!("/api/admin/acquisitions/{id}/resolve"),
                body,
                admin,
            )
            .await
        }
    };

    // Спорная запись
    acquire::enqueue(
        &app.state.db,
        Request::new(track.parse().unwrap(), AcquireReason::Like, "library"),
    )
    .await
    .unwrap();
    let job = claimed(&app, &token).await;
    let mut disputed = verified(&job["deezerTrackId"], 0, "afkL3uLNeXA");
    disputed["status"] = json!("disputed");
    disputed["chosen"] = Value::Null;
    disputed["upload"] = Value::Null;
    report(&app, &token, &job, disputed).await;
    let list = app
        .get("/api/admin/acquisitions?status=disputed", &admin)
        .await;
    let first = list.body["items"][0]["acquisition"]["id"]
        .as_str()
        .unwrap()
        .to_string();

    for body in [
        json!({ "action": "choose" }),
        json!({ "action": "choose", "videoId": "short" }),
        json!({ "action": "url" }),
        json!({ "action": "url", "url": "https://www.youtube.com/watch?v=bad" }),
        json!({ "action": "url", "url": "file:///etc/passwd" }),
        json!({ "action": "teleport" }),
    ] {
        let res = resolve(first.clone(), body.clone()).await;
        assert_eq!(res.status, StatusCode::BAD_REQUEST, "{body}");
    }

    // Выбрано видео: новое скачивание вне очереди, пробуется только оно
    let res = resolve(
        first.clone(),
        json!({ "action": "choose", "videoId": "manualVid01" }),
    )
    .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    let chosen = res.body["acquisition"]["id"].as_str().unwrap().to_string();
    assert_ne!(chosen, first);
    assert_eq!(res.body["acquisition"]["reason"], "manual");
    assert_eq!(res.body["acquisition"]["audioStatus"], "queued");
    assert_eq!(res.body["preferVideoIds"], json!(["manualVid01"]));
    let (lane, _, _) = job_of(&app.state.db, &chosen).await;
    assert_eq!(lane, "now");
    // Пока оно идёт, второе решение не принимается
    let res = resolve(chosen.clone(), json!({ "action": "retry" })).await;
    assert_eq!(res.status, StatusCode::CONFLICT);
    assert_eq!(res.code(), "acquisition_active");
    let job = claimed(&app, &token).await;
    assert_eq!(job["preferVideoIds"], json!(["manualVid01"]));
    assert_eq!(job["maxAttempts"], 1);
    // Выбор человека принимается и без совпадения отпечатка
    upload(
        &app.state.storage,
        job["upload"]["url"].as_str().unwrap(),
        &[5u8; 99],
    )
    .await;
    let mut manual = verified(&job["deezerTrackId"], 99, "manualVid01");
    manual["status"] = json!("unverified");
    manual["chosen"]["candidate"]["source"] = json!("manual");
    manual["chosen"]["candidate"]["score"] = json!(0.0);
    manual["chosen"]["verify"] = json!({ "verified": false, "similarity": 0.52, "offsetS": 0.0 });
    assert_eq!(
        report(&app, &token, &job, manual).await.status,
        StatusCode::NO_CONTENT
    );
    let ready: (String, String, bool) = sqlx::query_as(
        "SELECT r.audio_status, f.source_ref, f.verified FROM recordings r JOIN audio_files f ON f.id = r.primary_file_id
         JOIN tracks t ON t.recording_id = r.id WHERE t.id = $1::uuid",
    )
    .bind(&track)
    .fetch_one(&app.state.db)
    .await
    .unwrap();
    assert_eq!(ready, ("ready".into(), "manualVid01".into(), false));

    // Ссылка не с YouTube: fetcher качает её саму; ручной выбор заменяет основной файл
    let res = resolve(
        chosen.clone(),
        json!({ "action": "url", "url": "https://soundcloud.com/lowborn/light-me-up" }),
    )
    .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    assert_eq!(
        res.body["directUrl"],
        "https://soundcloud.com/lowborn/light-me-up"
    );
    assert_eq!(
        res.body["acquisition"]["audioStatus"], "ready",
        "старый файл играет, пока качается новый"
    );
    let job = claimed(&app, &token).await;
    assert_eq!(
        job["directUrl"],
        "https://soundcloud.com/lowborn/light-me-up"
    );
    assert_eq!(job["maxAttempts"], 1);
    upload(
        &app.state.storage,
        job["upload"]["url"].as_str().unwrap(),
        &[6u8; 77],
    )
    .await;
    let mut direct = verified(
        &job["deezerTrackId"],
        77,
        "https://soundcloud.com/lowborn/light-me-up",
    );
    direct["chosen"]["candidate"]["source"] = json!("manual");
    report(&app, &token, &job, direct).await;
    let primary: (String, String) = sqlx::query_as(
        "SELECT f.source, f.source_ref FROM recordings r JOIN audio_files f ON f.id = r.primary_file_id
         JOIN tracks t ON t.recording_id = r.id WHERE t.id = $1::uuid",
    )
    .bind(&track)
    .fetch_one(&app.state.db)
    .await
    .unwrap();
    assert_eq!(
        primary,
        (
            "soundcloud".into(),
            "https://soundcloud.com/lowborn/light-me-up".into()
        )
    );

    // Другая спорная запись: «нет в источниках», затем повтор
    let other = ep_tracks(&app, &admin).await[0];
    acquire::enqueue(
        &app.state.db,
        Request::new(other, AcquireReason::Like, "library"),
    )
    .await
    .unwrap();
    let job = claimed(&app, &token).await;
    report(
        &app,
        &token,
        &job,
        json!({ "status": "disputed", "deezerTrackId": job["deezerTrackId"] }),
    )
    .await;
    let list = app
        .get("/api/admin/acquisitions?status=disputed", &admin)
        .await;
    let item = list.body["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|i| i["track"]["id"] == other.to_string())
        .unwrap()
        .clone();
    let id = item["acquisition"]["id"].as_str().unwrap().to_string();
    let res = resolve(id.clone(), json!({ "action": "unavailable" })).await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    assert_eq!(res.body["acquisition"]["status"], "unavailable");
    assert_eq!(res.body["acquisition"]["audioStatus"], "unavailable");
    let res = resolve(id.clone(), json!({ "action": "retry" })).await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    assert_eq!(res.body["acquisition"]["status"], "queued");
    assert_eq!(res.body["acquisition"]["audioStatus"], "queued");
    assert_eq!(res.body["preferVideoIds"], json!([]));
    // Прежнее спорное скачивание закрыто решением и из «Спорных» ушло
    let list = app
        .get("/api/admin/acquisitions?status=disputed", &admin)
        .await;
    assert!(
        list.body["items"].as_array().unwrap().is_empty(),
        "{}",
        list.text
    );
    let closed: (String, String) =
        sqlx::query_as("SELECT status, error FROM acquisitions WHERE id = $1::uuid")
            .bind(&first)
            .fetch_one(&app.state.db)
            .await
            .unwrap();
    assert_eq!(
        closed,
        ("cancelled".into(), "в админке выбрано другое видео".into())
    );
    let missing = Uuid::now_v7();
    let res = resolve(missing.to_string(), json!({ "action": "retry" })).await;
    assert_eq!(res.status, StatusCode::NOT_FOUND);

    // Админка видит всё и с постраничным курсором
    let page = app.get("/api/admin/acquisitions?limit=2", &admin).await;
    assert_eq!(page.body["items"].as_array().unwrap().len(), 2);
    let cursor = page.body["nextCursor"].as_str().unwrap();
    let next = app
        .get(
            &format!("/api/admin/acquisitions?limit=2&cursor={cursor}"),
            &admin,
        )
        .await;
    assert_eq!(next.status, StatusCode::OK);
    assert!(!next.body["items"].as_array().unwrap().is_empty());
    let kinds = app.audit_kinds().await;
    assert!(kinds.iter().any(|k| k == "acquisition.resolved"));
    let res = app
        .call(
            Method::GET,
            "/api/admin/acquisitions?status=sleeping",
            None,
            &admin,
        )
        .await;
    assert_eq!(res.status, StatusCode::BAD_REQUEST);
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn acquisition_without_a_live_job_is_replaced(db: PgPool) {
    let app = TestApp::new(db).await;
    let me = app.listener("ann").await;
    let track = ep_tracks(&app, &me).await[0];
    let first = acquire::enqueue(
        &app.state.db,
        Request::new(track, AcquireReason::Like, "library"),
    )
    .await
    .unwrap();
    let acquire::Enqueue::Queued {
        acquisition_id,
        job_id,
    } = first
    else {
        panic!("{first:?}");
    };
    // Задачу удалили мимо скачивания (чистка истории, ручная правка): запись не должна застрять
    sqlx::query("DELETE FROM jobs WHERE id = $1")
        .bind(job_id)
        .execute(&app.state.db)
        .await
        .unwrap();
    let res = app.get(&format!("/api/tracks/{track}/stream"), &me).await;
    assert_eq!(res.status, StatusCode::ACCEPTED, "{}", res.text);
    let fresh = res.body["acquisitionId"].as_str().unwrap();
    assert_ne!(fresh, acquisition_id.to_string());
    let old: (String, String) =
        sqlx::query_as("SELECT status, error FROM acquisitions WHERE id = $1")
            .bind(acquisition_id)
            .fetch_one(&app.state.db)
            .await
            .unwrap();
    assert_eq!(old, ("failed".into(), "задача скачивания пропала".into()));
    let (lane, priority, _) = job_of(&app.state.db, fresh).await;
    assert_eq!((lane.as_str(), priority), ("now", 10));
}
