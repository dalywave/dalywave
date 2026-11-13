//! Раздел «Люди» админки: права, список, «Сейчас» с приватной сессией, блокировка, имя, удаление.

use axum::http::{Method, StatusCode};
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    common::{Auth, TestApp},
    mock::Person,
};

/// Слушатель без лишних администраторов (`TestApp::listener` заводит своего): роль выдаёт `admin`
async fn listener(app: &TestApp, admin: &Auth, sub: &str) -> Auth {
    let auth = Auth::Cookie(app.login(&Person::new(sub)).await);
    let id = app.user_id(&auth).await;
    let res = app
        .call(
            Method::PATCH,
            &format!("/api/admin/users/{id}"),
            Some(json!({ "roles": ["listener"] })),
            admin,
        )
        .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    auth
}

async fn online(app: &TestApp, user: &str, device: &str, playing: bool) {
    let user: Uuid = user.parse().unwrap();
    sqlx::query(
        "INSERT INTO connect_devices (user_id, id, name, kind, platform, connection, last_seen_at)
         VALUES ($1, $2, 'Mac', 'laptop', 'macos', gen_random_uuid(), now())",
    )
    .bind(user)
    .bind(device)
    .execute(&app.state.db)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO playback_state (user_id, active_connect_id, state) VALUES ($1, $2, $3)
         ON CONFLICT (user_id) DO UPDATE SET active_connect_id = $2, state = $3",
    )
    .bind(user)
    .bind(device)
    .bind(crate::connect::state(5000, playing))
    .execute(&app.state.db)
    .await
    .unwrap();
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn people_section_needs_admin(db: PgPool) {
    let app = TestApp::new(db).await;
    let (boss_id, admin) = app.admin("boss").await;
    let ann = listener(&app, &admin, "ann").await;
    for uri in [
        "/api/admin/users/table".to_string(),
        "/api/admin/live".to_string(),
        format!("/api/admin/users/{boss_id}"),
        format!("/api/admin/users/{boss_id}/history"),
    ] {
        let res = app.get(&uri, &ann).await;
        assert_eq!(res.status, StatusCode::FORBIDDEN, "{uri}");
    }
    let res = app
        .call(
            Method::DELETE,
            &format!("/api/admin/users/{boss_id}"),
            Some(json!({ "confirmName": "Person boss" })),
            &ann,
        )
        .await;
    assert_eq!(res.status, StatusCode::FORBIDDEN);
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn table_and_detail_show_activity(db: PgPool) {
    let app = TestApp::new(db).await;
    let (boss_id, admin) = app.admin("boss").await;
    let ann = listener(&app, &admin, "ann").await;
    let ann_id = app.user_id(&ann).await;
    online(&app, &ann_id, "mac", true).await;

    let table = app.get("/api/admin/users/table", &admin).await;
    assert_eq!(table.status, StatusCode::OK, "{}", table.text);
    let row = table.body["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["user"]["id"] == ann_id.as_str())
        .unwrap()
        .clone();
    assert_eq!(row["online"], true);
    assert_eq!(row["listenedMs30d"], 0);
    assert!(row["lastSeenAt"].is_string());

    let d = app.get(&format!("/api/admin/users/{ann_id}"), &admin).await;
    assert_eq!(d.status, StatusCode::OK, "{}", d.text);
    assert_eq!(d.body["summary"]["user"]["roles"], json!(["listener"]));
    assert_eq!(d.body["identities"].as_array().unwrap().len(), 1);
    assert_eq!(d.body["live"]["now"]["track"]["title"], "Первый");
    assert_eq!(d.body["lastAdmin"], false);
    assert_eq!(d.body["stats"]["days"].as_array().unwrap().len(), 30);
    // Единственный администратор помечен: его нельзя ни заблокировать, ни удалить
    let me = app
        .get(&format!("/api/admin/users/{boss_id}"), &admin)
        .await;
    assert_eq!(me.body["lastAdmin"], true);

    let missing = app
        .get(&format!("/api/admin/users/{}", Uuid::now_v7()), &admin)
        .await;
    assert_eq!(missing.status, StatusCode::NOT_FOUND);
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn live_hides_private_session(db: PgPool) {
    let app = TestApp::new(db).await;
    let (_, admin) = app.admin("boss").await;
    let ann = listener(&app, &admin, "ann").await;
    let bob = listener(&app, &admin, "bob").await;
    let ann_id = app.user_id(&ann).await;
    let bob_id = app.user_id(&bob).await;
    online(&app, &ann_id, "mac", true).await;
    online(&app, &bob_id, "phone", false).await;

    let live = app.get("/api/admin/live", &admin).await;
    assert_eq!(live.status, StatusCode::OK, "{}", live.text);
    assert_eq!(live.body["summary"]["online"], 2);
    assert_eq!(live.body["summary"]["listening"], 1);
    // Слушающие — первыми; на паузе — следом, с треком
    let people = live.body["people"].as_array().unwrap();
    assert_eq!(people[0]["user"]["id"], ann_id.as_str());
    assert_eq!(people[0]["now"]["playing"], true);
    assert_eq!(people[0]["now"]["context"]["type"], "album");
    assert_eq!(people[0]["devices"][0]["active"], true);
    assert_eq!(people[1]["now"]["playing"], false);
    assert_eq!(people[1]["now"]["track"]["title"], "Первый");

    // Приватная сессия: виден только сам факт, без трека и источника
    let p = app
        .call(
            Method::PATCH,
            "/api/friends/privacy",
            Some(json!({ "privateSession": "hours6" })),
            &ann,
        )
        .await;
    assert_eq!(p.status, StatusCode::OK, "{}", p.text);
    let live = app.get("/api/admin/live", &admin).await;
    let ann_now = live.body["people"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["user"]["id"] == ann_id.as_str())
        .unwrap()["now"]
        .clone();
    assert_eq!(ann_now["private"], true);
    assert!(ann_now["track"].is_null(), "{ann_now}");
    assert!(ann_now["context"].is_null());
    let d = app.get(&format!("/api/admin/users/{ann_id}"), &admin).await;
    assert!(d.body["live"]["now"]["track"].is_null());

    // Устройство пропало — человека нет в «Сейчас»
    sqlx::query("UPDATE connect_devices SET last_seen_at = now() - interval '1 minute'")
        .execute(&app.state.db)
        .await
        .unwrap();
    let live = app.get("/api/admin/live", &admin).await;
    assert_eq!(live.body["summary"]["online"], 0);
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn block_rename_and_self_protection(db: PgPool) {
    let app = TestApp::new(db).await;
    let (boss_id, admin) = app.admin("boss").await;
    let ann = listener(&app, &admin, "ann").await;
    let ann_id = app.user_id(&ann).await;
    let uri = format!("/api/admin/users/{ann_id}");

    // Блокировка: вход закрыт, сессии отозваны, время записано
    let res = app
        .call(
            Method::PATCH,
            &uri,
            Some(json!({ "disabled": true })),
            &admin,
        )
        .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    assert_eq!(
        app.get("/api/auth/me", &ann).await.status,
        StatusCode::UNAUTHORIZED
    );
    let d = app.get(&uri, &admin).await;
    assert!(d.body["disabledAt"].is_string());
    let res = app
        .call(
            Method::PATCH,
            &uri,
            Some(json!({ "disabled": false })),
            &admin,
        )
        .await;
    assert_eq!(res.status, StatusCode::OK);
    assert!(app.get(&uri, &admin).await.body["disabledAt"].is_null());

    // Имя от админа переживает вход через провайдера
    let res = app
        .call(
            Method::PATCH,
            &uri,
            Some(json!({ "name": "  Анна  " })),
            &admin,
        )
        .await;
    assert_eq!(res.body["name"], "Анна");
    let ann = Auth::Cookie(app.login(&Person::new("ann")).await);
    assert_eq!(
        app.get("/api/auth/me", &ann).await.body["user"]["name"],
        "Анна"
    );
    assert_eq!(app.get(&uri, &admin).await.body["nameLocked"], true);
    let res = app
        .call(Method::PATCH, &uri, Some(json!({ "name": " " })), &admin)
        .await;
    assert_eq!(res.status, StatusCode::BAD_REQUEST);

    // «Выйти везде»
    let res = app.delete(&format!("{uri}/sessions"), &admin).await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    assert!(res.body["revoked"].as_u64().unwrap() >= 1);
    assert_eq!(
        app.get("/api/auth/me", &ann).await.status,
        StatusCode::UNAUTHORIZED
    );

    // Себя: не заблокировать, не лишить admin, не удалить, свои сессии так не закрыть
    let me = format!("/api/admin/users/{boss_id}");
    for (body, code) in [
        (json!({ "disabled": true }), "cannot_disable_self"),
        (json!({ "roles": ["listener"] }), "cannot_demote_self"),
    ] {
        let res = app.call(Method::PATCH, &me, Some(body), &admin).await;
        assert_eq!(res.status, StatusCode::CONFLICT);
        assert_eq!(res.code(), code);
    }
    let res = app
        .call(
            Method::DELETE,
            &me,
            Some(json!({ "confirmName": "Person boss" })),
            &admin,
        )
        .await;
    assert_eq!(res.code(), "cannot_delete_self");
    let res = app.delete(&format!("{me}/sessions"), &admin).await;
    assert_eq!(res.code(), "cannot_revoke_self");

    let kinds = app.audit_kinds().await;
    for k in [
        "user.disabled",
        "user.enabled",
        "user.renamed",
        "user.sessions_revoked",
    ] {
        assert!(kinds.iter().any(|x| x == k), "{k}: {kinds:?}");
    }
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn last_admin_flag_follows_admins(db: PgPool) {
    let app = TestApp::new(db).await;
    let (boss_id, admin) = app.admin("boss").await;
    let ann = listener(&app, &admin, "ann").await;
    let ann_id = app.user_id(&ann).await;
    let flag = |id: String| {
        let app = &app;
        let admin = &admin;
        async move { app.get(&format!("/api/admin/users/{id}"), admin).await.body["lastAdmin"].clone() }
    };
    assert_eq!(flag(boss_id.clone()).await, true);
    // Второй администратор: теперь ни один не последний, и второго можно снять
    let uri = format!("/api/admin/users/{ann_id}");
    let res = app
        .call(
            Method::PATCH,
            &uri,
            Some(json!({ "roles": ["admin", "listener"] })),
            &admin,
        )
        .await;
    assert_eq!(res.status, StatusCode::OK);
    assert_eq!(flag(boss_id.clone()).await, false);
    assert_eq!(flag(ann_id.clone()).await, false);
    // Заблокированный админ не считается: boss снова единственный
    let res = app
        .call(
            Method::PATCH,
            &uri,
            Some(json!({ "disabled": true })),
            &admin,
        )
        .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    assert_eq!(flag(boss_id.clone()).await, true);
    assert_eq!(flag(ann_id).await, false);
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn delete_removes_everything(db: PgPool) {
    let app = TestApp::new(db).await;
    let (_, admin) = app.admin("boss").await;
    let ann = listener(&app, &admin, "ann").await;
    let ann_id = app.user_id(&ann).await;
    let res = app
        .post("/api/playlists", json!({ "title": "Ночь" }), &ann)
        .await;
    assert!(res.status.is_success(), "{}", res.text);
    online(&app, &ann_id, "mac", true).await;
    let uri = format!("/api/admin/users/{ann_id}");

    // Имя для подтверждения должно совпасть
    let res = app
        .call(
            Method::DELETE,
            &uri,
            Some(json!({ "confirmName": "кто-то" })),
            &admin,
        )
        .await;
    assert_eq!(res.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(res.code(), "confirm_mismatch");

    let res = app
        .call(
            Method::DELETE,
            &uri,
            Some(json!({ "confirmName": "Person ann" })),
            &admin,
        )
        .await;
    assert_eq!(res.status, StatusCode::NO_CONTENT, "{}", res.text);
    let id: Uuid = ann_id.parse().unwrap();
    for table in [
        "users WHERE id",
        "playlists WHERE user_id",
        "sessions WHERE user_id",
        "user_identities WHERE user_id",
        "connect_devices WHERE user_id",
        "playback_state WHERE user_id",
        "tombstones WHERE user_id",
    ] {
        let n: i64 = sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
            "SELECT count(*) FROM {table} = $1"
        )))
        .bind(id)
        .fetch_one(&app.state.db)
        .await
        .unwrap();
        assert_eq!(n, 0, "{table}");
    }
    assert_eq!(
        app.get("/api/auth/me", &ann).await.status,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(app.get(&uri, &admin).await.status, StatusCode::NOT_FOUND);
    assert!(app.audit_kinds().await.iter().any(|k| k == "user.deleted"));
}
