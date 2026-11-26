//! Уведомления: центр уведомлений, настройки, подписки устройств и доставка push. APNs и служба
//! Web Push — поддельные HTTP-серверы на этой машине: настоящей сети тесты не трогают.

use std::{
    net::SocketAddr,
    sync::{Arc, Mutex},
};

use axum::{
    Router,
    body::Bytes,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    routing::post,
};
use chrono::Utc;
use dalywave_protocol::notifications::{NotificationItem, NotificationKind};
use dalywave_server::{
    config::{ApnsConfig, VapidConfig},
    crypto::{b64, b64_decode, random_bytes},
    jobs::worker::Outcome,
    notify::{self, Draft, webpush},
};
use serde_json::{Value, json};
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    common::{Auth, TestApp},
    deezer::{DEVIL_SINGLE, LIGHT_ME_UP_ALBUM, LOWBORN, MODERN_SADNESS, fixture},
};

/// Ключ APNs только для тестов.
const TEST_P8: &str = "-----BEGIN PRIVATE KEY-----
MIGHAgEAMBMGByqGSM49AgEGCCqGSM49AwEHBG0wawIBAQQg6SaekJxSZC4o4BWu
a0FcuNrh88dL09FQibl8gPEHg++hRANCAARwCAOsN4ViqE37jC8jSECODRd13ia4
hJQ3qUm5USAhKHOp+Ru9zn9P/xGT/50v4MOZUMS4WiYwXyy2BRKUh+p/
-----END PRIVATE KEY-----";

/// Что пришло на поддельную службу.
#[derive(Debug, Clone)]
struct Hit {
    path: String,
    headers: HeaderMap,
    body: Bytes,
}

#[derive(Clone, Default)]
struct Fake {
    hits: Arc<Mutex<Vec<Hit>>>,
}

impl Fake {
    fn hits(&self) -> Vec<Hit> {
        self.hits.lock().unwrap().clone()
    }
}

async fn serve(router: Router) -> SocketAddr {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    addr
}

/// Служба Web Push: `/ok/*` принимает, `/gone/*` — подписки больше нет (410).
async fn fake_webpush() -> (Fake, SocketAddr) {
    let fake = Fake::default();
    async fn take(
        State(f): State<Fake>,
        Path((kind, _id)): Path<(String, String)>,
        headers: HeaderMap,
        body: Bytes,
    ) -> StatusCode {
        f.hits.lock().unwrap().push(Hit {
            path: kind.clone(),
            headers,
            body,
        });
        if kind == "gone" {
            StatusCode::GONE
        } else {
            StatusCode::CREATED
        }
    }
    let router = Router::new()
        .route("/{kind}/{id}", post(take))
        .with_state(fake.clone());
    (fake.clone(), serve(router).await)
}

/// APNs: боевая среда не знает токенов песочницы (`BadDeviceToken`), токен `dead…` — удалённое
/// приложение (410).
async fn fake_apns() -> (Fake, SocketAddr) {
    let fake = Fake::default();
    async fn take(
        State(f): State<Fake>,
        Path((env, token)): Path<(String, String)>,
        headers: HeaderMap,
        body: Bytes,
    ) -> (StatusCode, String) {
        f.hits.lock().unwrap().push(Hit {
            path: format!("{env}/{token}"),
            headers,
            body,
        });
        if token.starts_with("dead") {
            return (
                StatusCode::GONE,
                json!({ "reason": "Unregistered" }).to_string(),
            );
        }
        if env == "prod" && token.starts_with("5a") {
            return (
                StatusCode::BAD_REQUEST,
                json!({ "reason": "BadDeviceToken" }).to_string(),
            );
        }
        (StatusCode::OK, String::new())
    }
    let router = Router::new()
        .route("/{env}/3/device/{token}", post(take))
        .with_state(fake.clone());
    (fake.clone(), serve(router).await)
}

pub(crate) fn vapid() -> VapidConfig {
    let (public, private) = webpush::generate_keys().unwrap();
    let mut key = [0u8; 32];
    key.copy_from_slice(&b64_decode(&private).unwrap());
    VapidConfig {
        public_key: public,
        private_key: key,
        subject: "mailto:owner@example.com".into(),
    }
}

/// Все задачи полосы уведомлений, включая отложенные (окно сбора, тихие часы).
async fn drain(app: &TestApp) -> Vec<(String, Outcome)> {
    let mut done = Vec::new();
    for _ in 0..20 {
        sqlx::query(
            "UPDATE jobs SET run_after = now() WHERE lane = 'notify' AND status = 'queued'",
        )
        .execute(&app.state.db)
        .await
        .unwrap();
        match app.run_job("notify").await {
            Some(r) => done.push(r),
            None => break,
        }
    }
    done
}

async fn quiet_off(app: &TestApp, auth: &Auth) {
    let res = app
        .call(
            axum::http::Method::PATCH,
            "/api/notifications/settings",
            Some(json!({ "quiet": { "enabled": false, "from": 1380, "to": 480 } })),
            auth,
        )
        .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn web_push_end_to_end(db: PgPool) {
    let (fake, addr) = fake_webpush().await;
    let config = vapid();
    let public = config.public_key.clone();
    let app = TestApp::with(db, move |c| c.push.vapid = Some(config)).await;
    let me = app.listener("ann").await;

    let info = app.get("/api/notifications/push", &me).await;
    assert_eq!(info.status, StatusCode::OK, "{}", info.text);
    assert_eq!(info.body["webPushKey"], public);
    assert_eq!(info.body["apns"], false);
    assert_eq!(info.body["devices"], json!([]));

    // Браузер: свои ключи шифрования
    let (ua_public, ua_private) = webpush::generate_keys().unwrap();
    let auth_secret = b64(random_bytes::<16>().unwrap());
    let subscribe = |kind: &str| {
        json!({
            "channel": "webpush",
            "subscription": {
                "endpoint": format!("http://{addr}/{kind}/{}", Uuid::now_v7()),
                "keys": { "p256dh": ua_public, "auth": auth_secret },
            },
            "utcOffsetMin": 180,
        })
    };
    let res = app
        .post("/api/notifications/push", subscribe("ok"), &me)
        .await;
    assert_eq!(res.status, StatusCode::CREATED, "{}", res.text);
    let bad = app
        .post(
            "/api/notifications/push",
            json!({ "channel": "webpush", "subscription": { "endpoint": "ftp://x", "keys": { "p256dh": ua_public, "auth": auth_secret } } }),
            &me,
        )
        .await;
    assert_eq!(bad.status, StatusCode::BAD_REQUEST);
    let apns = app
        .post(
            "/api/notifications/push",
            json!({ "channel": "apns", "token": "ab" }),
            &me,
        )
        .await;
    assert_eq!(
        apns.status,
        StatusCode::BAD_REQUEST,
        "APNs на сервере выключен"
    );

    let info = app.get("/api/notifications/push", &me).await;
    assert_eq!(info.body["devices"][0]["current"], true);
    assert_eq!(info.body["devices"][0]["channel"], "webpush");
    let settings = app.get("/api/notifications/settings", &me).await;
    assert_eq!(settings.body["utcOffsetMin"], 180);
    assert!(
        settings.body["kinds"]
            .as_array()
            .unwrap()
            .iter()
            .all(|k| k["kind"] != "review" && k["kind"] != "test"),
        "слушателю — без админских: {}",
        settings.body
    );

    // Пробное — и в тихие часы
    let test = app.post("/api/notifications/test", json!({}), &me).await;
    assert_eq!(test.status, StatusCode::OK, "{}", test.text);
    assert_eq!(test.body["devices"], 1);
    let ran = drain(&app).await;
    assert_eq!(
        ran.iter().map(|(k, _)| k.as_str()).collect::<Vec<_>>(),
        ["notify_send", "notify_push"]
    );
    let hits = fake.hits();
    assert_eq!(hits.len(), 1);
    let hit = &hits[0];
    assert_eq!(hit.headers["content-encoding"], "aes128gcm");
    let authz = hit.headers["authorization"].to_str().unwrap();
    assert!(authz.starts_with("vapid t=") && authz.ends_with(&format!("k={public}")));
    let plain = webpush::decrypt(
        &b64_decode(&ua_private).unwrap(),
        &b64_decode(&auth_secret).unwrap(),
        &hit.body,
    )
    .unwrap();
    let payload: Value = serde_json::from_slice(&plain).unwrap();
    assert_eq!(payload["title"], "Уведомления работают");
    assert_eq!(payload["link"], "/notifications");
    assert_eq!(payload["unread"], 1);

    // Центр уведомлений
    let page = app.get("/api/notifications", &me).await;
    assert_eq!(page.status, StatusCode::OK, "{}", page.text);
    assert_eq!(page.body["unread"], 1);
    assert_eq!(page.body["items"][0]["kind"], "test");
    assert!(page.body["items"][0]["readAt"].is_null());
    let read = app.post("/api/notifications/read", json!({}), &me).await;
    assert_eq!(read.body["all"], true);
    assert_eq!(read.body["unread"], 0);

    // Браузер сменил подписку: прежняя выключена, новая отвечает «подписки нет» — выключается и она
    let res = app
        .post("/api/notifications/push", subscribe("gone"), &me)
        .await;
    assert_eq!(res.status, StatusCode::CREATED);
    let info = app.get("/api/notifications/push", &me).await;
    assert_eq!(info.body["devices"].as_array().unwrap().len(), 1);
    app.post("/api/notifications/test", json!({}), &me).await;
    let ran = drain(&app).await;
    assert_eq!(ran.len(), 2);
    assert_eq!(ran[1].1, Outcome::Done, "«подписки нет» — не сбой задачи");
    let disabled: Option<String> = sqlx::query_scalar(
        "SELECT disabled_reason FROM push_subscriptions WHERE token LIKE '%/gone/%'",
    )
    .fetch_one(&app.state.db)
    .await
    .unwrap();
    assert!(disabled.unwrap().contains("410"));
    let info = app.get("/api/notifications/push", &me).await;
    assert_eq!(info.body["devices"], json!([]));

    // Выход: подписка этой сессии больше не получает push
    let res = app
        .post("/api/notifications/push", subscribe("ok"), &me)
        .await;
    assert_eq!(res.status, StatusCode::CREATED);
    app.post("/api/auth/logout", json!({}), &me).await;
    let user: Uuid = sqlx::query_scalar("SELECT user_id FROM push_subscriptions LIMIT 1")
        .fetch_one(&app.state.db)
        .await
        .unwrap();
    notify::emit(
        &app.state.db,
        user,
        Draft::new(NotificationKind::Test, "после выхода", ""),
    )
    .await
    .unwrap();
    let before = fake.hits().len();
    let ran = drain(&app).await;
    assert_eq!(ran.len(), 1, "push не ставится: {ran:?}");
    assert_eq!(fake.hits().len(), before);
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn apns_delivery(db: PgPool) {
    let (fake, addr) = fake_apns().await;
    let dir = tempfile::tempdir().unwrap();
    let key_path = dir.path().join("AuthKey_ABC123DEFG.p8");
    std::fs::write(&key_path, TEST_P8).unwrap();
    let apns = ApnsConfig {
        key_path,
        key_id: "ABC123DEFG".into(),
        team_id: "Y5KR5JPCRG".into(),
        topic: "rest.daly.wave".into(),
        production_url: format!("http://{addr}/prod"),
        sandbox_url: format!("http://{addr}/sandbox"),
    };
    let app = TestApp::with(db, move |c| c.push.apns = Some(apns)).await;
    let me = app.listener("ann").await;
    quiet_off(&app, &me).await;
    assert_eq!(
        app.get("/api/notifications/push", &me).await.body["apns"],
        true
    );

    // Сборка TestFlight; токен в верхнем регистре сохраняется как есть, но в нижнем
    let token = "AB".repeat(32);
    let res = app
        .post(
            "/api/notifications/push",
            json!({ "channel": "apns", "token": token, "environment": "production", "name": "iPhone" }),
            &me,
        )
        .await;
    assert_eq!(res.status, StatusCode::CREATED, "{}", res.text);
    app.post("/api/notifications/test", json!({}), &me).await;
    drain(&app).await;
    let hits = fake.hits();
    assert_eq!(hits.len(), 1);
    let hit = &hits[0];
    assert_eq!(hit.path, format!("prod/{}", "ab".repeat(32)));
    assert_eq!(hit.headers["apns-topic"], "rest.daly.wave");
    assert_eq!(hit.headers["apns-push-type"], "alert");
    let bearer = hit.headers["authorization"].to_str().unwrap();
    assert!(bearer.starts_with("bearer ey"));
    let payload: Value = serde_json::from_slice(&hit.body).unwrap();
    assert_eq!(payload["aps"]["alert"]["title"], "Уведомления работают");
    assert_eq!(payload["aps"]["badge"], 1);
    assert_eq!(payload["aps"]["thread-id"], "test");
    assert_eq!(payload["link"], "dalywave://notifications");

    // Отладочная сборка назвалась боевой: APNs отвечает «чужой токен» — пробуем песочницу и запоминаем
    let sandbox = format!("5a{}", "0".repeat(62));
    app.post(
        "/api/notifications/push",
        json!({ "channel": "apns", "token": sandbox }),
        &me,
    )
    .await;
    // Удалённое приложение
    let dead = format!("dead{}", "0".repeat(60));
    app.post(
        "/api/notifications/push",
        json!({ "channel": "apns", "token": dead }),
        &me,
    )
    .await;
    // Прежние подписки этой сессии заменены новыми — снова включим их, как три разных телефона
    sqlx::query("UPDATE push_subscriptions SET disabled_at = NULL, disabled_reason = NULL")
        .execute(&app.state.db)
        .await
        .unwrap();
    app.post("/api/notifications/test", json!({}), &me).await;
    let ran = drain(&app).await;
    assert_eq!(ran.len(), 4, "{ran:?}");
    assert!(ran.iter().all(|(_, o)| *o == Outcome::Done), "{ran:?}");
    let env: String =
        sqlx::query_scalar("SELECT environment FROM push_subscriptions WHERE token = $1")
            .bind(&sandbox)
            .fetch_one(&app.state.db)
            .await
            .unwrap();
    assert_eq!(env, "sandbox");
    let gone: Option<String> =
        sqlx::query_scalar("SELECT disabled_reason FROM push_subscriptions WHERE token = $1")
            .bind(&dead)
            .fetch_one(&app.state.db)
            .await
            .unwrap();
    assert!(gone.unwrap().contains("Unregistered"));
    let paths: Vec<String> = fake.hits().iter().map(|h| h.path.clone()).collect();
    assert!(paths.contains(&format!("sandbox/{sandbox}")), "{paths:?}");

    // Прочитали в приложении, пока push ждал повтора или утра, — push не уходит
    let before = fake.hits().len();
    app.post("/api/notifications/test", json!({}), &me).await;
    sqlx::query("UPDATE jobs SET run_after = now() WHERE lane = 'notify'")
        .execute(&app.state.db)
        .await
        .unwrap();
    app.run_job("notify").await.unwrap();
    app.post("/api/notifications/read", json!({}), &me).await;
    drain(&app).await;
    assert_eq!(fake.hits().len(), before);
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn grouping_marks_and_quiet_hours(db: PgPool) {
    let (fake, addr) = fake_webpush().await;
    let config = vapid();
    let app = TestApp::with(db, move |c| c.push.vapid = Some(config)).await;
    let me = app.listener("ann").await;
    let user: Uuid = app.user_id(&me).await.parse().unwrap();
    let (ua_public, _) = webpush::generate_keys().unwrap();
    let res = app
        .post(
            "/api/notifications/push",
            json!({ "channel": "webpush", "subscription": {
                "endpoint": format!("http://{addr}/ok/1"),
                "keys": { "p256dh": ua_public, "auth": b64(random_bytes::<16>().unwrap()) } } }),
            &me,
        )
        .await;
    assert_eq!(res.status, StatusCode::CREATED, "{}", res.text);

    let release = |n: u32, artist: &str| {
        Draft::new(
            NotificationKind::Release,
            format!("{artist} — Альбом {n}"),
            "Новый альбом",
        )
        .link(format!("/album/{n}"))
        .item(NotificationItem {
            title: format!("Альбом {n}"),
            subtitle: artist.into(),
            link: Some(format!("/album/{n}")),
            image: None,
        })
        .group("release")
        .once(format!("release:{n}"))
    };
    let db = &app.state.db;
    let first = notify::emit(db, user, release(1, "Lowborn")).await.unwrap();
    let second = notify::emit(db, user, release(2, "Saluki")).await.unwrap();
    let third = notify::emit(db, user, release(3, "Lowborn")).await.unwrap();
    assert!(first.is_some());
    assert_eq!(first, second, "события собираются в одно уведомление");
    assert_eq!(first, third);
    assert!(
        notify::emit(db, user, release(1, "Lowborn"))
            .await
            .unwrap()
            .is_none(),
        "о релизе — один раз"
    );
    // Окно сбора ещё идёт: рассылки нет
    assert!(app.run_job("notify").await.is_none());
    let page = app.get("/api/notifications", &me).await;
    assert_eq!(page.body["items"].as_array().unwrap().len(), 1);
    let n = &page.body["items"][0];
    assert_eq!(n["title"], "3 новых релиза");
    assert_eq!(n["body"], "Lowborn и Saluki");
    assert_eq!(n["count"], 3);
    assert_eq!(n["items"].as_array().unwrap().len(), 3);

    // Тихие часы прямо сейчас: окно приложения узнаёт сразу, push ждёт утра
    let now_min = {
        let local = Utc::now() + chrono::Duration::minutes(180);
        use chrono::Timelike;
        (local.hour() * 60 + local.minute()) as i64
    };
    let res = app
        .call(
            axum::http::Method::PATCH,
            "/api/notifications/settings",
            Some(json!({
                "quiet": { "enabled": true, "from": (now_min + 1440 - 60) % 1440, "to": (now_min + 120) % 1440 },
                "utcOffsetMin": 180,
            })),
            &me,
        )
        .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    sqlx::query("UPDATE jobs SET run_after = now() WHERE kind = 'notify_send'")
        .execute(db)
        .await
        .unwrap();
    let (kind, outcome) = app.run_job("notify").await.unwrap();
    assert_eq!((kind.as_str(), outcome), ("notify_send", Outcome::Done));
    let wait: f64 = sqlx::query_scalar(
        "SELECT EXTRACT(EPOCH FROM run_after - now())::float8 FROM jobs WHERE kind = 'notify_push'",
    )
    .fetch_one(db)
    .await
    .unwrap();
    assert!((60.0 * 100.0..=60.0 * 121.0).contains(&wait), "{wait}");
    assert!(app.run_job("notify").await.is_none());
    assert!(fake.hits().is_empty());

    // Вид выключен — уведомления нет вовсе
    let res = app
        .call(
            axum::http::Method::PATCH,
            "/api/notifications/settings",
            Some(json!({ "kinds": [{ "kind": "release", "enabled": false }] })),
            &me,
        )
        .await;
    assert_eq!(res.status, StatusCode::OK);
    assert!(
        res.body["kinds"]
            .as_array()
            .unwrap()
            .iter()
            .any(|k| k["kind"] == "release" && k["enabled"] == false)
    );
    assert!(
        notify::emit(db, user, release(4, "Lowborn"))
            .await
            .unwrap()
            .is_none()
    );
    let bad = app
        .call(
            axum::http::Method::PATCH,
            "/api/notifications/settings",
            Some(json!({ "quiet": { "enabled": true, "from": 2000, "to": 1 } })),
            &me,
        )
        .await;
    assert_eq!(bad.status, StatusCode::BAD_REQUEST);
}

/// Список релизов Lowborn: EP вышел сегодня, остальное — давно.
fn discography_with_new_ep() -> Value {
    let today = Utc::now().date_naive().to_string();
    let real = fixture(&format!("artist_{LOWBORN}_albums"));
    let data: Vec<Value> = real["data"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|a| {
            [MODERN_SADNESS, DEVIL_SINGLE, LIGHT_ME_UP_ALBUM].contains(&a["id"].as_i64().unwrap())
        })
        .cloned()
        .map(|mut a| {
            if a["id"] == MODERN_SADNESS {
                a["release_date"] = json!(today);
            }
            a
        })
        .collect();
    json!({ "data": data, "total": data.len() })
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn new_release_of_a_followed_artist(db: PgPool) {
    let app = TestApp::new(db).await;
    let me = app.listener("ann").await;
    let today = Utc::now().date_naive().to_string();
    app.deezer.set(
        &format!("/artist/{LOWBORN}/albums"),
        discography_with_new_ep(),
    );
    let mut album = fixture(&format!("album_{MODERN_SADNESS}"));
    album["release_date"] = json!(today);
    app.deezer.set(&format!("/album/{MODERN_SADNESS}"), album);
    let artist = app.resolve_id("artist", LOWBORN, &me).await;
    let res = app
        .call(
            axum::http::Method::PUT,
            &format!("/api/library/artists/{artist}"),
            None,
            &me,
        )
        .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    let (kind, outcome) = app.run_job("maintenance").await.unwrap();
    assert_eq!(
        (kind.as_str(), outcome),
        ("sync_discography", Outcome::Done)
    );
    let result: Value =
        sqlx::query_scalar("SELECT result FROM jobs WHERE kind = 'sync_discography'")
            .fetch_one(&app.state.db)
            .await
            .unwrap();
    assert_eq!(result["notified"], 1, "{result}");

    let page = app.get("/api/notifications", &me).await;
    let n = &page.body["items"][0];
    assert_eq!(n["kind"], "release");
    assert!(
        n["title"].as_str().unwrap().starts_with("Lowborn — "),
        "{n}"
    );
    assert_eq!(n["body"], "Новый EP · скоро можно слушать");
    assert!(n["link"].as_str().unwrap().starts_with("/album/"));
    assert!(
        n["image"]
            .as_str()
            .unwrap()
            .starts_with("/api/images/albums/")
    );

    // Повторная сверка не повторяет уведомление
    let mut conn = app.state.db.acquire().await.unwrap();
    dalywave_server::jobs::enqueue(
        &mut conn,
        dalywave_server::jobs::NewJob::new(
            "sync_discography",
            "maintenance",
            json!({ "artistId": artist }),
        ),
    )
    .await
    .unwrap();
    drop(conn);
    app.run_job("maintenance").await.unwrap();
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM notifications")
        .fetch_one(&app.state.db)
        .await
        .unwrap();
    assert_eq!(count, 1);
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn admin_kinds_reach_only_admins(db: PgPool) {
    let app = TestApp::new(db).await;
    let (_, admin) = app.admin("boss").await;
    let listener = app.listener("ann").await;
    let settings = app.get("/api/notifications/settings", &admin).await;
    let kinds: Vec<&str> = settings.body["kinds"]
        .as_array()
        .unwrap()
        .iter()
        .map(|k| k["kind"].as_str().unwrap())
        .collect();
    assert!(
        kinds.contains(&"review") && kinds.contains(&"sources"),
        "{kinds:?}"
    );

    // Три остановки за полчаса «YouTube заподозрил бота» — администраторам
    notify::events::source_blocked(&app.state.db, "bot_check")
        .await
        .unwrap();
    let none: i64 = sqlx::query_scalar("SELECT count(*) FROM notifications")
        .fetch_one(&app.state.db)
        .await
        .unwrap();
    assert_eq!(none, 0, "одной остановки мало");
    // Спорные ждут решения: администраторам, к рассылке — по тому, что ждёт (в пустой базе нечего)
    notify::events::disputed(&app.state.db).await.unwrap();
    let rows: Vec<(String, Uuid)> =
        sqlx::query_as("SELECT kind, user_id FROM notifications ORDER BY created_at")
            .fetch_all(&app.state.db)
            .await
            .unwrap();
    assert_eq!(
        rows.len(),
        2,
        "оба администратора: boss и тот, кто выдал роль ann"
    );
    assert!(rows.iter().all(|(k, _)| k == "review"));
    let listener_id = app.user_id(&listener).await;
    assert!(rows.iter().all(|(_, u)| u.to_string() != listener_id));
    sqlx::query("UPDATE jobs SET run_after = now() WHERE kind = 'notify_send'")
        .execute(&app.state.db)
        .await
        .unwrap();
    drain(&app).await;
    let left: i64 = sqlx::query_scalar("SELECT count(*) FROM notifications")
        .fetch_one(&app.state.db)
        .await
        .unwrap();
    assert_eq!(left, 0, "решать нечего — уведомление снято");
}
