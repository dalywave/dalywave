//! «Недавно прослушанное» (поиск, переход к дате, календарь, очистка), «На повторе» и «Старые хиты
//! на повторе», «Умное перемешивание» и «Неверная запись».

use axum::http::StatusCode;
use chrono::{DateTime, Duration, Utc};
use dalywave_server::reco::repeat;
use serde_json::{Value, json};
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    common::TestApp,
    synth::{Feat, Synth},
};

async fn play_at(
    db: &PgPool,
    user: Uuid,
    track: Uuid,
    at: DateTime<Utc>,
    context: Option<(&str, String)>,
    source: &str,
) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO play_events (client_event_id, user_id, track_id, started_at, played_ms, duration_ms, end_reason,
                                  context_type, context_id, source)
         VALUES ($1, $2, $3, $4, 180000, 180000, 'completed', $5, $6, $7)
         RETURNING id",
    )
    .bind(Uuid::now_v7())
    .bind(user)
    .bind(track)
    .bind(at)
    .bind(context.as_ref().map(|c| c.0))
    .bind(context.map(|c| c.1))
    .bind(source)
    .fetch_one(db)
    .await
    .unwrap()
}

fn ids(page: &Value) -> Vec<String> {
    page["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["track"]["id"].as_str().unwrap().to_string())
        .collect()
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn history_has_titles_search_dates_and_clearing(db: PgPool) {
    let app = TestApp::new(db).await;
    let me = app.listener("ann").await;
    let user: Uuid = app.user_id(&me).await.parse().unwrap();
    let s = Synth::new(app.state.db.clone());
    let rushex = s.artist("Rushex").await;
    let lowborn = s.artist("Lowborn").await;
    let night = s.track(rushex, "Ночной город", Feat::default()).await;
    let ice = s.track(rushex, "Лёд", Feat::default()).await;
    let rain = s.track(lowborn, "Rain", Feat::default()).await;

    let pl = app
        .post("/api/playlists", json!({ "title": "Микс дня 2" }), &me)
        .await;
    assert_eq!(pl.status, StatusCode::CREATED, "{}", pl.text);
    let pl_id = pl.body["playlist"]["id"].as_str().unwrap().to_string();

    let now = Utc::now();
    let db = &app.state.db;
    // Позавчера — из архива Spotify, вчера — из плейлиста, сегодня — волна и снова плейлист
    let old = play_at(db, user, rain, now - Duration::days(2), None, "import").await;
    play_at(
        db,
        user,
        ice,
        now - Duration::days(1),
        Some(("playlist", pl_id.clone())),
        "stream",
    )
    .await;
    let wave = play_at(
        db,
        user,
        night,
        now - Duration::hours(2),
        Some(("wave", Uuid::now_v7().to_string())),
        "stream",
    )
    .await;
    play_at(
        db,
        user,
        ice,
        now - Duration::hours(1),
        Some(("playlist", pl_id.clone())),
        "stream",
    )
    .await;

    let res = app.get("/api/me/history", &me).await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    assert_eq!(
        ids(&res.body),
        [
            ice.to_string(),
            night.to_string(),
            ice.to_string(),
            rain.to_string()
        ]
    );
    let items = res.body["items"].as_array().unwrap();
    assert_eq!(items[0]["contextTitle"], "Микс дня 2");
    assert_eq!(items[0]["imported"], false);
    assert_eq!(items[1]["contextType"], "wave");
    assert!(items[1]["contextTitle"].is_null());
    assert_eq!(
        items[3]["imported"], true,
        "история из архива Spotify видна"
    );

    // Поиск: по артисту (без регистра) и по названию без «ё»
    let res = app.get("/api/me/history?q=lowBORN", &me).await;
    assert_eq!(ids(&res.body), [rain.to_string()]);
    let res = app.get("/api/me/history?q=%D0%BB%D0%B5%D0%B4", &me).await;
    assert_eq!(ids(&res.body), [ice.to_string(), ice.to_string()]);
    // Подстановочные знаки LIKE — буквально
    let res = app.get("/api/me/history?q=%25", &me).await;
    assert!(ids(&res.body).is_empty());

    // Переход к дате: всё раньше начала вчерашнего дня, курсор — дальше от него
    let before = (now - Duration::hours(30)).to_rfc3339();
    let res = app
        .get(
            &format!("/api/me/history?limit=1&before={}", urlencoding(&before)),
            &me,
        )
        .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    assert_eq!(ids(&res.body), [rain.to_string()]);

    let res = app.get("/api/me/history?limit=2", &me).await;
    let cursor = res.body["nextCursor"].as_str().unwrap().to_string();
    let res = app
        .get(
            &format!(
                "/api/me/history?limit=2&cursor={cursor}&before={}",
                urlencoding(&before)
            ),
            &me,
        )
        .await;
    assert_eq!(
        ids(&res.body),
        [ice.to_string(), rain.to_string()],
        "курсор главнее даты"
    );

    // Календарь: три дня с прослушиваниями, новые первыми
    let res = app.get("/api/me/history/days?offsetMin=180", &me).await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    let days = res.body["days"].as_array().unwrap();
    let total: i64 = days.iter().map(|d| d["plays"].as_i64().unwrap()).sum();
    assert_eq!(total, 4);
    assert!(days.len() >= 2);
    assert!(days[0]["day"].as_str().unwrap() > days[1]["day"].as_str().unwrap());
    let res = app.get("/api/me/history/days?offsetMin=9999", &me).await;
    assert_eq!(res.status, StatusCode::BAD_REQUEST);

    // Убрать одно; чужое и уже убранное — 404
    let other = app.listener("bob").await;
    assert_eq!(
        app.delete(&format!("/api/me/history/{wave}"), &other)
            .await
            .status,
        StatusCode::NOT_FOUND
    );
    let res = app.delete(&format!("/api/me/history/{wave}"), &me).await;
    assert_eq!(res.status, StatusCode::NO_CONTENT, "{}", res.text);
    assert_eq!(
        app.delete(&format!("/api/me/history/{wave}"), &me)
            .await
            .status,
        StatusCode::NOT_FOUND
    );
    let res = app.get("/api/me/history", &me).await;
    assert_eq!(res.body["items"].as_array().unwrap().len(), 3);

    // Очистить: экран пуст, но прослушивания остаются для вкуса и миксов
    let res = app.delete("/api/me/history", &me).await;
    assert_eq!(res.status, StatusCode::NO_CONTENT, "{}", res.text);
    let res = app.get("/api/me/history", &me).await;
    assert!(res.body["items"].as_array().unwrap().is_empty());
    let res = app.get("/api/me/history/days", &me).await;
    assert!(res.body["days"].as_array().unwrap().is_empty());
    let kept: i64 = sqlx::query_scalar("SELECT count(*) FROM play_events WHERE user_id = $1")
        .bind(user)
        .fetch_one(db)
        .await
        .unwrap();
    assert_eq!(kept, 4);
    // Новое прослушивание после очистки — снова в истории
    play_at(db, user, night, Utc::now(), None, "stream").await;
    assert_eq!(
        ids(&app.get("/api/me/history", &me).await.body),
        [night.to_string()]
    );
    let _ = old;
}

fn urlencoding(s: &str) -> String {
    s.replace('+', "%2B").replace(':', "%3A")
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn repeat_playlists_follow_history(db: PgPool) {
    let app = TestApp::new(db).await;
    let me = app.listener("ann").await;
    let user: Uuid = app.user_id(&me).await.parse().unwrap();
    let s = Synth::new(app.state.db.clone());
    let a = s.artist("Rushex").await;
    let b = s.artist("Mzlff").await;

    // «На повторе»: шесть треков, сыгранных 2–7 раз за месяц; один раз — не считается
    let mut hot = Vec::new();
    for i in 0..6 {
        let t = s.track(a, &format!("Сейчас {i}"), Feat::default()).await;
        for k in 0..(7 - i) {
            s.play(user, t, 5 + k * 24, true).await;
        }
        hot.push(t);
    }
    let once = s.track(a, "Один раз", Feat::default()).await;
    s.play(user, once, 3, true).await;
    let skipped = s.track(a, "Пропущенный", Feat::default()).await;
    for k in 0..5 {
        s.play(user, skipped, 2 + k, false).await;
    }
    let blocked = s.track(b, "Не рекомендовать", Feat::default()).await;
    for k in 0..9 {
        s.play(user, blocked, 2 + k, true).await;
    }
    s.block(user, "track", blocked).await;

    // «Старые хиты»: крутились 8–12 месяцев назад, один из них звучал и на прошлой неделе
    let month = 24 * 30;
    let mut old = Vec::new();
    for i in 0..6 {
        let t = s.track(b, &format!("Тогда {i}"), Feat::default()).await;
        for k in 0..(3 + i) {
            s.play(user, t, (8 + k % 4) * month + k * 3, true).await;
        }
        old.push(t);
    }
    let back = s.track(b, "Вернулся", Feat::default()).await;
    for k in 0..6 {
        s.play(user, back, 9 * month + k, true).await;
    }
    s.play(user, back, 24 * 7, true).await;

    let report = repeat::refresh(&app.state.db, user).await.unwrap();
    assert_eq!(report.on_repeat, 6);
    assert_eq!(report.repeat_rewind, 6);
    let on_repeat = repeat::on_repeat(&app.state.db, user).await.unwrap();
    assert_eq!(
        on_repeat, hot,
        "чаще сыгранное — выше; пропуски и «не рекомендовать» не считаются"
    );
    let rewind = repeat::repeat_rewind(&app.state.db, user).await.unwrap();
    let mut want = old.clone();
    want.reverse();
    assert_eq!(rewind, want, "недавно звучавшее — не «старый хит»");

    // На главной — в полке «Собрано для вас» после миксов дня
    let home = app.get("/api/home", &me).await;
    let shelf = home.body["shelves"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["id"] == "daily-mixes")
        .expect("полка миксов")
        .clone();
    assert_eq!(shelf["title"], "Собрано для вас");
    let titles: Vec<&str> = shelf["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["title"].as_str().unwrap())
        .collect();
    assert_eq!(titles, ["На повторе", "Старые хиты на повторе"]);
    let id = shelf["items"][0]["id"].as_str().unwrap();
    let detail = app.get(&format!("/api/playlists/{id}"), &me).await;
    assert_eq!(detail.status, StatusCode::OK, "{}", detail.text);
    assert_eq!(detail.body["playlist"]["kind"], "mix");
    assert_eq!(detail.body["playlist"]["generator"], "on_repeat");
    assert_eq!(detail.body["items"].as_array().unwrap().len(), 6);

    // Пересчёт обновляет тот же плейлист; истории не стало — плейлистов тоже
    repeat::refresh(&app.state.db, user).await.unwrap();
    let count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM playlists WHERE user_id = $1 AND rules ->> 'generator' IN ('on_repeat', 'repeat_rewind')",
    )
    .bind(user)
    .fetch_one(&app.state.db)
    .await
    .unwrap();
    assert_eq!(count, 2);
    sqlx::query("DELETE FROM play_events WHERE user_id = $1")
        .bind(user)
        .execute(&app.state.db)
        .await
        .unwrap();
    let report = repeat::refresh(&app.state.db, user).await.unwrap();
    assert_eq!(report, repeat::RepeatReport::default());
    let count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM playlists WHERE user_id = $1 AND rules ->> 'generator' IN ('on_repeat', 'repeat_rewind')",
    )
    .bind(user)
    .fetch_one(&app.state.db)
    .await
    .unwrap();
    assert_eq!(count, 0);
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn smart_shuffle_suggests_what_fits_the_list(db: PgPool) {
    let app = TestApp::new(db).await;
    let me = app.listener("ann").await;
    let s = Synth::new(app.state.db.clone());
    let list_artist = s.artist("Rushex").await;
    let mut list = Vec::new();
    for i in 0..4 {
        list.push(
            s.track(
                list_artist,
                &format!("Свой {i}"),
                Feat {
                    axis: 0,
                    jitter: i,
                    ..Default::default()
                },
            )
            .await,
        );
    }
    // Похожие: три у одного артиста (возьмутся два), по одному у других; далёкое — мимо
    let near_artist = s.artist("Lowborn").await;
    let mut near = Vec::new();
    for i in 0..3 {
        near.push(
            s.track(
                near_artist,
                &format!("Рядом {i}"),
                Feat {
                    axis: 0,
                    jitter: 5 + i,
                    ..Default::default()
                },
            )
            .await,
        );
    }
    let other = s.artist("Mzlff").await;
    let fits = s
        .track(
            other,
            "Тоже подходит",
            Feat {
                axis: 0,
                jitter: 9,
                ..Default::default()
            },
        )
        .await;
    let far = s
        .track(
            other,
            "Далеко",
            Feat {
                axis: 600,
                ..Default::default()
            },
        )
        .await;

    let res = app
        .post(
            "/api/tracks/suggest",
            json!({ "trackIds": list, "limit": 10 }),
            &me,
        )
        .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    let got: Vec<Uuid> = res.body["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["track"]["id"].as_str().unwrap().parse().unwrap())
        .collect();
    assert!(
        got.iter().all(|t| !list.contains(t)),
        "своё не предлагается"
    );
    assert!(got.contains(&fits));
    assert_eq!(
        got.iter().filter(|t| near.contains(t)).count(),
        2,
        "не больше двух у артиста"
    );
    assert_eq!(res.body["items"][0]["reason"]["text"], "Подобрано");
    let _ = far;

    // Убранное из очереди больше не предлагается
    let res = app
        .post(
            "/api/tracks/suggest",
            json!({ "trackIds": list, "exclude": [fits], "limit": 10 }),
            &me,
        )
        .await;
    let got: Vec<&str> = res.body["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["track"]["id"].as_str().unwrap())
        .collect();
    assert!(!got.contains(&fits.to_string().as_str()));

    let res = app
        .post("/api/tracks/suggest", json!({ "trackIds": [] }), &me)
        .await;
    assert_eq!(res.status, StatusCode::BAD_REQUEST);
}

/// Готовая запись трека с файлом из YouTube Music и удачным скачиванием, которое его принесло.
async fn ready_with_video(db: &PgPool, track: Uuid, video: &str) -> Uuid {
    let recording: Uuid = sqlx::query_scalar("SELECT recording_id FROM tracks WHERE id = $1")
        .bind(track)
        .fetch_one(db)
        .await
        .unwrap();
    let file: Uuid = sqlx::query_scalar(
        "INSERT INTO audio_files (recording_id, storage_key, codec, container, duration_ms, size_bytes, source,
                                  source_ref, bitrate_kbps, verified)
         VALUES ($1, 'audio/' || $2 || '.m4a', 'opus', 'mp4', 180000, 4000000, 'youtube_music', $2, 160, true)
         RETURNING id",
    )
    .bind(recording)
    .bind(video)
    .fetch_one(db)
    .await
    .unwrap();
    sqlx::query("UPDATE recordings SET primary_file_id = $2, audio_status = 'ready' WHERE id = $1")
        .bind(recording)
        .bind(file)
        .execute(db)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO acquisitions (recording_id, track_id, status, reason, lane, chosen, finished_at)
         VALUES ($1, $2, 'ready', 'play', 'now', $3, now())",
    )
    .bind(recording)
    .bind(track)
    .bind(json!({ "candidate": { "videoId": video } }))
    .execute(db)
    .await
    .unwrap();
    recording
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn wrong_recording_report_disputes_and_reacquires(db: PgPool) {
    let app = TestApp::new(db).await;
    let me = app.listener("ann").await;
    let (_, admin) = app.admin("root").await;
    let s = Synth::new(app.state.db.clone());
    let artist = s.artist("Rushex").await;
    let track = s
        .track(
            artist,
            "Ночной город",
            Feat {
                status: "ready",
                ..Default::default()
            },
        )
        .await;
    let recording = ready_with_video(&app.state.db, track, "afkL3uLNeXA").await;

    let res = app
        .post(
            &format!("/api/tracks/{track}/wrong-recording"),
            json!({ "reason": "wrong_version", "comment": "  это лайв  " }),
            &me,
        )
        .await;
    assert_eq!(res.status, StatusCode::ACCEPTED, "{}", res.text);
    let dispute = res.body["disputeId"].as_str().unwrap().to_string();
    let acquisition = res.body["acquisitionId"].as_str().unwrap().to_string();

    // В «Спорных» — жалоба с причиной, комментарием и тем, что было выбрано
    let list = app
        .get("/api/admin/acquisitions?status=disputed", &admin)
        .await;
    assert_eq!(list.status, StatusCode::OK, "{}", list.text);
    let item = list.body["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|i| i["acquisition"]["id"] == dispute.as_str())
        .expect("жалоба в «Спорных»")
        .clone();
    let error = item["acquisition"]["error"].as_str().unwrap();
    assert!(error.contains("другая версия или лайв"), "{error}");
    assert!(error.contains("«это лайв»"), "{error}");
    assert_eq!(item["chosen"]["candidate"]["videoId"], "afkL3uLNeXA");

    // Новое скачивание: без этого видео, с заменой основного файла; запись пока играет прежний
    let payload: Value = sqlx::query_scalar(
        "SELECT j.payload FROM acquisitions a JOIN jobs j ON j.id = a.job_id WHERE a.id = $1::uuid",
    )
    .bind(&acquisition)
    .fetch_one(&app.state.db)
    .await
    .unwrap();
    assert_eq!(payload["excludeVideoIds"], json!(["afkL3uLNeXA"]));
    assert_eq!(payload["replace"], true);
    let status: String = sqlx::query_scalar("SELECT audio_status FROM recordings WHERE id = $1")
        .bind(recording)
        .fetch_one(&app.state.db)
        .await
        .unwrap();
    assert_eq!(status, "ready");
    assert!(
        app.audit_kinds()
            .await
            .contains(&"recording.reported".to_string())
    );

    // Повторная жалоба, пока скачивание идёт: ещё одна спорная, нового скачивания нет
    let res = app
        .post(
            &format!("/api/tracks/{track}/wrong-recording"),
            json!({ "reason": "cut" }),
            &me,
        )
        .await;
    assert_eq!(res.status, StatusCode::ACCEPTED, "{}", res.text);
    assert!(res.body["acquisitionId"].is_null());
    let reports: i64 =
        sqlx::query_scalar("SELECT count(*) FROM recording_reports WHERE recording_id = $1")
            .bind(recording)
            .fetch_one(&app.state.db)
            .await
            .unwrap();
    assert_eq!(reports, 2);

    // Ошибки: длинный комментарий, нет трека, нет файла
    let res = app
        .post(
            &format!("/api/tracks/{track}/wrong-recording"),
            json!({ "reason": "wrong_song", "comment": "а".repeat(501) }),
            &me,
        )
        .await;
    assert_eq!(res.status, StatusCode::BAD_REQUEST);
    let res = app
        .post(
            &format!("/api/tracks/{}/wrong-recording", Uuid::now_v7()),
            json!({ "reason": "wrong_song" }),
            &me,
        )
        .await;
    assert_eq!(res.status, StatusCode::NOT_FOUND);
    let fresh = s.track(artist, "Ещё не скачан", Feat::default()).await;
    let res = app
        .post(
            &format!("/api/tracks/{fresh}/wrong-recording"),
            json!({ "reason": "wrong_song" }),
            &me,
        )
        .await;
    assert_eq!(res.status, StatusCode::CONFLICT);
    assert_eq!(res.body["error"]["code"], "not_ready");
    let res = app
        .post(
            &format!("/api/tracks/{track}/wrong-recording"),
            json!({ "reason": "teleported" }),
            &me,
        )
        .await;
    assert!(res.status.is_client_error());
}
