//! Папки плейлистов и «Свой порядок»: создание с плейлистами, перекладка в папку и из неё, перестановка
//! папок, переименование, сворачивание, удаление (плейлисты встают на место папки), чужое недоступно,
//! синхронизация и перенумерация, когда места между соседями нет.

use axum::http::{Method, StatusCode};
use serde_json::{Value, json};
use sqlx::PgPool;

use crate::common::{Auth, TestApp};

async fn playlist(app: &TestApp, auth: &Auth, title: &str) -> String {
    let res = app
        .post("/api/playlists", json!({ "title": title }), auth)
        .await;
    assert_eq!(res.status, StatusCode::CREATED, "{}", res.text);
    res.body["playlist"]["id"].as_str().unwrap().to_string()
}

/// Вне папок по «Своему порядку»: папки и плейлисты одной лентой; у папки — её плейлисты.
fn layout(list: &Value) -> Vec<(String, Vec<String>)> {
    let items = list["items"].as_array().unwrap();
    let folders = list["folders"].as_array().unwrap();
    let mut root: Vec<(f64, String, Vec<String>)> = Vec::new();
    for f in folders {
        let id = f["id"].as_str().unwrap();
        let mut inside: Vec<(f64, String)> = items
            .iter()
            .filter(|p| p["folderId"].as_str() == Some(id))
            .map(|p| {
                (
                    p["position"].as_f64().unwrap(),
                    p["title"].as_str().unwrap().to_string(),
                )
            })
            .collect();
        inside.sort_by(|a, b| a.0.total_cmp(&b.0));
        root.push((
            f["position"].as_f64().unwrap(),
            format!("[{}]", f["title"].as_str().unwrap()),
            inside.into_iter().map(|x| x.1).collect(),
        ));
    }
    for p in items.iter().filter(|p| p["folderId"].is_null()) {
        root.push((
            p["position"].as_f64().unwrap(),
            p["title"].as_str().unwrap().to_string(),
            Vec::new(),
        ));
    }
    root.sort_by(|a, b| a.0.total_cmp(&b.0));
    root.into_iter().map(|(_, t, i)| (t, i)).collect()
}

fn names(l: &[(String, Vec<String>)]) -> Vec<String> {
    l.iter()
        .map(|(t, inside)| {
            if inside.is_empty() {
                t.clone()
            } else {
                format!("{t}:{}", inside.join(","))
            }
        })
        .collect()
}

async fn list(app: &TestApp, auth: &Auth) -> Vec<String> {
    let res = app.get("/api/playlists", auth).await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    names(&layout(&res.body))
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn folders_hold_playlists_and_keep_custom_order(db: PgPool) {
    let app = TestApp::new(db).await;
    let me = app.listener("ann").await;
    let a = playlist(&app, &me, "A").await;
    let b = playlist(&app, &me, "B").await;
    let c = playlist(&app, &me, "C").await;
    // Новые — первыми
    assert_eq!(list(&app, &me).await, ["C", "B", "A"]);

    // Папка с плейлистом сразу; встаёт первой
    let res = app
        .post(
            "/api/playlist-folders",
            json!({ "title": "  Для   бега ", "playlistIds": [a] }),
            &me,
        )
        .await;
    assert_eq!(res.status, StatusCode::CREATED, "{}", res.text);
    let folder = res.body["folders"][0]["id"].as_str().unwrap().to_string();
    assert_eq!(res.body["folders"][0]["title"], "Для бега");
    assert_eq!(res.body["folders"][0]["collapsed"], false);
    assert_eq!(names(&layout(&res.body)), ["[Для бега]:A", "C", "B"]);

    // В папку после A, из папки — после самой папки
    let res = app
        .post(
            &format!("/api/playlists/{b}/move"),
            json!({ "folderId": folder, "afterId": a }),
            &me,
        )
        .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    assert_eq!(names(&layout(&res.body)), ["[Для бега]:A,B", "C"]);
    let res = app
        .post(
            &format!("/api/playlists/{b}/move"),
            json!({ "folderId": folder, "atStart": true }),
            &me,
        )
        .await;
    assert_eq!(names(&layout(&res.body)), ["[Для бега]:B,A", "C"]);
    let res = app
        .post(
            &format!("/api/playlists/{c}/move"),
            json!({ "folderId": null, "atStart": true }),
            &me,
        )
        .await;
    assert_eq!(names(&layout(&res.body)), ["C", "[Для бега]:B,A"]);
    // Папка в конец
    let res = app
        .post(
            &format!("/api/playlist-folders/{folder}/move"),
            json!({}),
            &me,
        )
        .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    assert_eq!(names(&layout(&res.body)), ["C", "[Для бега]:B,A"]);
    let res = app
        .post(
            &format!("/api/playlist-folders/{folder}/move"),
            json!({ "atStart": true }),
            &me,
        )
        .await;
    assert_eq!(names(&layout(&res.body)), ["[Для бега]:B,A", "C"]);

    // Перекладка не меняет «Недавние»: updated_at плейлиста прежний
    let before: Value = app.get(&format!("/api/playlists/{a}"), &me).await.body;

    // Переименовать и свернуть
    let res = app
        .call(
            Method::PATCH,
            &format!("/api/playlist-folders/{folder}"),
            Some(json!({ "title": "Бег", "collapsed": true })),
            &me,
        )
        .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    assert_eq!(res.body["title"], "Бег");
    assert_eq!(res.body["collapsed"], true);
    let res = app
        .call(
            Method::PATCH,
            &format!("/api/playlist-folders/{folder}"),
            Some(json!({ "title": "  " })),
            &me,
        )
        .await;
    assert_eq!(res.status, StatusCode::BAD_REQUEST);

    // Сосед не с этого уровня, чужая папка, чужой плейлист
    let res = app
        .post(
            &format!("/api/playlists/{c}/move"),
            json!({ "folderId": null, "afterId": a }),
            &me,
        )
        .await;
    assert_eq!(res.status, StatusCode::BAD_REQUEST, "A — в папке");
    let bob = app.listener("bob").await;
    let theirs = playlist(&app, &bob, "Bob's").await;
    let res = app
        .post(
            &format!("/api/playlists/{theirs}/move"),
            json!({ "folderId": folder }),
            &bob,
        )
        .await;
    assert_eq!(res.status, StatusCode::NOT_FOUND, "чужая папка");
    let res = app
        .post(
            &format!("/api/playlists/{a}/move"),
            json!({ "folderId": null }),
            &bob,
        )
        .await;
    assert_eq!(res.status, StatusCode::NOT_FOUND, "чужой плейлист");
    assert_eq!(list(&app, &bob).await, ["Bob's"]);

    // Удалили папку — её плейлисты встали на её место в её порядке
    let res = app
        .delete(&format!("/api/playlist-folders/{folder}"), &me)
        .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    assert_eq!(names(&layout(&res.body)), ["B", "A", "C"]);
    assert!(res.body["folders"].as_array().unwrap().is_empty());
    let after: Value = app.get(&format!("/api/playlists/{a}"), &me).await.body;
    assert_eq!(
        before["playlist"]["updatedAt"],
        after["playlist"]["updatedAt"]
    );
    assert_eq!(
        app.delete(&format!("/api/playlist-folders/{folder}"), &me)
            .await
            .status,
        StatusCode::NOT_FOUND
    );
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn folders_sync_and_rebalance_when_crowded(db: PgPool) {
    let app = TestApp::new(db).await;
    let me = app.listener("ann").await;
    let a = playlist(&app, &me, "A").await;
    let b = playlist(&app, &me, "B").await;
    let first = app.get("/api/sync", &me).await;
    let cursor = first.body["cursor"].as_str().unwrap().to_string();
    let positions: Vec<f64> = first.body["playlists"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["position"].as_f64().unwrap())
        .collect();
    assert_eq!(positions.len(), 2);

    let res = app
        .post(
            "/api/playlist-folders",
            json!({ "title": "Вечер", "playlistIds": [b] }),
            &me,
        )
        .await;
    let folder = res.body["folders"][0]["id"].as_str().unwrap().to_string();
    let res = app.get(&format!("/api/sync?since={cursor}"), &me).await;
    let folders = res.body["playlistFolders"].as_array().unwrap();
    assert_eq!(folders.len(), 1, "{}", res.text);
    assert_eq!(folders[0]["title"], "Вечер");
    let moved = res.body["playlists"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["id"] == b.as_str())
        .unwrap()
        .clone();
    assert_eq!(moved["folderId"], folder.as_str());
    let cursor = res.body["cursor"].as_str().unwrap().to_string();

    // По очереди A и C встают сразу после папки: зазор каждый раз вдвое меньше, пока уровень не
    // перенумеруется — порядок при этом верный всегда
    let c = playlist(&app, &me, "C").await;
    app.post(
        &format!("/api/playlist-folders/{folder}/move"),
        json!({ "atStart": true }),
        &me,
    )
    .await;
    for i in 0..70 {
        let (who, title) = if i % 2 == 0 { (&a, "A") } else { (&c, "C") };
        let res = app
            .post(
                &format!("/api/playlists/{who}/move"),
                json!({ "folderId": null, "afterId": folder }),
                &me,
            )
            .await;
        assert_eq!(res.status, StatusCode::OK, "{}", res.text);
        let l = names(&layout(&res.body));
        assert_eq!(l[0], "[Вечер]:B");
        assert_eq!(l[1], title, "{l:?}");
    }
    let spread: f64 = sqlx::query_scalar(
        "SELECT min(position) FILTER (WHERE title = 'C') - min(position) FILTER (WHERE title = 'A')
         FROM playlists",
    )
    .fetch_one(&app.state.db)
    .await
    .unwrap();
    assert!(spread.abs() > 1e-6, "уровень перенумерован: {spread}");

    // Удаление папки — tombstone, плейлист выходит из неё
    app.delete(&format!("/api/playlist-folders/{folder}"), &me)
        .await;
    let res = app.get(&format!("/api/sync?since={cursor}"), &me).await;
    let gone = res.body["tombstones"]
        .as_array()
        .unwrap()
        .iter()
        .any(|t| t["entity"] == "playlistFolders" && t["key"]["id"] == folder.as_str());
    assert!(gone, "{}", res.text);
    let b_row = res.body["playlists"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["id"] == b.as_str())
        .unwrap()
        .clone();
    assert!(b_row["folderId"].is_null());

    // Порядок плейлистов — настройка слушателя
    let res = app
        .call(
            Method::PATCH,
            "/api/settings",
            Some(json!({ "playlistSort": "custom" })),
            &me,
        )
        .await;
    assert_eq!(res.body["playlistSort"], "custom");
}
