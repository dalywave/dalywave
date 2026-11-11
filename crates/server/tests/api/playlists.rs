use axum::http::{Method, StatusCode};
use serde_json::{Value, json};
use sqlx::PgPool;

use crate::{
    common::{Auth, TestApp},
    deezer::MODERN_SADNESS,
};

async fn tracks(app: &TestApp, auth: &Auth) -> Vec<String> {
    let ep = app.resolve_id("album", MODERN_SADNESS, auth).await;
    app.get(&format!("/api/albums/{ep}"), auth).await.body["tracks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["id"].as_str().unwrap().to_string())
        .collect()
}

/// Порядок треков плейлиста и id позиций.
async fn order(app: &TestApp, auth: &Auth, id: &str) -> (Vec<String>, Vec<String>) {
    let res = app.get(&format!("/api/playlists/{id}"), auth).await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    let items = res.body["items"].as_array().unwrap();
    let positions: Vec<f64> = items
        .iter()
        .map(|i| i["position"].as_f64().unwrap())
        .collect();
    assert!(positions.windows(2).all(|w| w[0] < w[1]), "{positions:?}");
    (
        items
            .iter()
            .map(|i| i["track"]["id"].as_str().unwrap().to_string())
            .collect(),
        items
            .iter()
            .map(|i| i["id"].as_str().unwrap().to_string())
            .collect(),
    )
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn playlist_crud_and_ordering(db: PgPool) {
    let app = TestApp::new(db).await;
    let me = app.listener("ann").await;
    let t = tracks(&app, &me).await;

    let res = app
        .post(
            "/api/playlists",
            json!({ "title": "  Ночь  ", "description": "для дороги", "trackIds": [t[0], t[1]] }),
            &me,
        )
        .await;
    assert_eq!(res.status, StatusCode::CREATED, "{}", res.text);
    let id = res.body["playlist"]["id"].as_str().unwrap().to_string();
    assert_eq!(res.body["playlist"]["title"], "Ночь");
    assert_eq!(res.body["playlist"]["kind"], "user");
    assert_eq!(res.body["playlist"]["tracksCount"], 2);
    assert_eq!(
        res.body["playlist"]["covers"].as_array().unwrap().len(),
        1,
        "один альбом"
    );
    assert_eq!(res.body["items"][0]["position"], 1024.0);

    // В конец, в начало, после позиции
    let add = |body: Value| {
        let uri = format!("/api/playlists/{id}/items");
        let app = &app;
        let me = &me;
        async move { app.post(&uri, body, me).await }
    };
    let res = add(json!({ "trackIds": [t[2]] })).await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    assert_eq!(res.body["rebalanced"], false);
    add(json!({ "trackIds": [t[3]], "atStart": true })).await;
    let (_, items) = order(&app, &me, &id).await;
    add(json!({ "trackIds": [t[4], t[4]], "afterItemId": items[1] })).await;
    let (now, items) = order(&app, &me, &id).await;
    assert_eq!(
        now,
        vec![
            t[3].clone(),
            t[0].clone(),
            t[4].clone(),
            t[4].clone(),
            t[1].clone(),
            t[2].clone()
        ]
    );

    // Перестановка: последний — в начало, первый — после третьего
    let res = app
        .post(
            &format!("/api/playlists/{id}/items/{}/move", items[5]),
            json!({ "afterItemId": null }),
            &me,
        )
        .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    let res = app
        .post(
            &format!("/api/playlists/{id}/items/{}/move", items[0]),
            json!({ "afterItemId": items[2] }),
            &me,
        )
        .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    let (now, items) = order(&app, &me, &id).await;
    assert_eq!(
        now,
        vec![
            t[2].clone(),
            t[0].clone(),
            t[4].clone(),
            t[3].clone(),
            t[4].clone(),
            t[1].clone()
        ]
    );
    // На то же место — ничего не меняется
    let res = app
        .post(
            &format!("/api/playlists/{id}/items/{}/move", items[1]),
            json!({ "afterItemId": items[1] }),
            &me,
        )
        .await;
    assert_eq!(res.status, StatusCode::OK);

    assert_eq!(
        app.delete(&format!("/api/playlists/{id}/items/{}", items[0]), &me)
            .await
            .status,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        app.delete(&format!("/api/playlists/{id}/items/{}", items[0]), &me)
            .await
            .status,
        StatusCode::NOT_FOUND
    );
    assert_eq!(order(&app, &me, &id).await.0.len(), 5);

    let res = app
        .call(
            Method::PATCH,
            &format!("/api/playlists/{id}"),
            Some(json!({ "title": "Ночная дорога" })),
            &me,
        )
        .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    assert_eq!(res.body["title"], "Ночная дорога");
    assert_eq!(res.body["description"], "для дороги");
    assert_eq!(
        app.put(&format!("/api/playlists/{id}/pin"), &me)
            .await
            .status,
        StatusCode::NO_CONTENT
    );
    let library = app.get("/api/library", &me).await;
    assert_eq!(library.body["pinnedPlaylists"][0]["id"], id.as_str());
    assert_eq!(library.body["counts"]["playlists"], 1);
    assert_eq!(
        app.delete(&format!("/api/playlists/{id}/pin"), &me)
            .await
            .status,
        StatusCode::NO_CONTENT
    );
    assert!(
        app.get("/api/library", &me).await.body["pinnedPlaylists"]
            .as_array()
            .unwrap()
            .is_empty()
    );

    for (body, why) in [
        (json!({ "title": "" }), "пустое название"),
        (
            json!({ "title": "x", "trackIds": [uuid::Uuid::now_v7()] }),
            "неизвестный трек",
        ),
        (json!({ "title": "x", "color": "red" }), "незнакомое поле"),
    ] {
        assert_eq!(
            app.post("/api/playlists", body, &me).await.status,
            StatusCode::BAD_REQUEST,
            "{why}"
        );
    }
    let res = add(json!({ "trackIds": [t[0]], "afterItemId": uuid::Uuid::now_v7() })).await;
    assert_eq!(res.status, StatusCode::BAD_REQUEST);

    // Чужой плейлист не виден и не меняется
    let other = app.listener("bob").await;
    assert_eq!(
        app.get(&format!("/api/playlists/{id}"), &other)
            .await
            .status,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        app.post(
            &format!("/api/playlists/{id}/items"),
            json!({ "trackIds": [t[0]] }),
            &other
        )
        .await
        .status,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        app.delete(&format!("/api/playlists/{id}"), &other)
            .await
            .status,
        StatusCode::NOT_FOUND
    );
    assert!(
        app.get("/api/playlists", &other).await.body["items"]
            .as_array()
            .unwrap()
            .is_empty()
    );

    assert_eq!(
        app.delete(&format!("/api/playlists/{id}"), &me)
            .await
            .status,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        app.get(&format!("/api/playlists/{id}"), &me).await.status,
        StatusCode::NOT_FOUND
    );
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn positions_are_rebalanced_when_gaps_run_out(db: PgPool) {
    let app = TestApp::new(db).await;
    let me = app.listener("ann").await;
    let t = tracks(&app, &me).await;
    let res = app
        .post(
            "/api/playlists",
            json!({ "title": "Тесно", "trackIds": [t[0], t[1]] }),
            &me,
        )
        .await;
    let id = res.body["playlist"]["id"].as_str().unwrap().to_string();
    let first = res.body["items"][0]["id"].as_str().unwrap().to_string();

    // Вставка всё время сразу после первой позиции делит промежуток пополам
    let mut rebalanced = false;
    for _ in 0..40 {
        let res = app
            .post(
                &format!("/api/playlists/{id}/items"),
                json!({ "trackIds": [t[2]], "afterItemId": first }),
                &me,
            )
            .await;
        assert_eq!(res.status, StatusCode::OK, "{}", res.text);
        rebalanced |= res.body["rebalanced"] == true;
    }
    assert!(rebalanced, "промежуток кончился — позиции перенумерованы");
    let (now, items) = order(&app, &me, &id).await;
    assert_eq!(now.len(), 42);
    assert_eq!(now[0], t[0]);
    assert_eq!(now[41], t[1]);
    // Перестановка в тесном месте тоже перенумеровывает
    sqlx::query("UPDATE playlist_items SET position = 1.0 + 1e-9 WHERE id = $1")
        .bind(uuid::Uuid::parse_str(&items[1]).unwrap())
        .execute(&app.state.db)
        .await
        .unwrap();
    sqlx::query("UPDATE playlist_items SET position = 1.0 WHERE id = $1")
        .bind(uuid::Uuid::parse_str(&items[0]).unwrap())
        .execute(&app.state.db)
        .await
        .unwrap();
    let res = app
        .post(
            &format!("/api/playlists/{id}/items/{}/move", items[41]),
            json!({ "afterItemId": items[0] }),
            &me,
        )
        .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    assert_eq!(res.body["rebalanced"], true);
    let (now, _) = order(&app, &me, &id).await;
    assert_eq!(now[1], t[1]);
}
