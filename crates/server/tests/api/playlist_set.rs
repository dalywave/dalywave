//! «Собрать как сет»: предпросмотр, применение одной операцией с «Отменить», миксы только для
//! чтения, изменения уходят на устройства.

use axum::http::{Method, StatusCode};
use serde_json::{Value, json};
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    common::{Auth, TestApp},
    synth::{Feat, Synth},
};

/// (темп, тональность, энергия, исполнитель); `None` — трек без анализа.
const TRACKS: [Option<(f32, &str, f32, usize)>; 14] = [
    Some((128.0, "8A", 0.9, 0)),
    Some((90.0, "3B", 0.2, 1)),
    Some((126.0, "9A", 0.8, 2)),
    Some((92.0, "4B", 0.3, 0)),
    Some((124.0, "8A", 0.7, 1)),
    Some((94.0, "3B", 0.25, 2)),
    Some((127.0, "10A", 0.85, 3)),
    Some((91.0, "5B", 0.35, 3)),
    Some((125.0, "9A", 0.6, 0)),
    Some((120.0, "8A", 0.5, 1)),
    None,
    Some((93.0, "4B", 0.4, 2)),
    None,
    Some((122.0, "7A", 0.55, 3)),
];

/// Плейлист из треков в «неудобном» порядке; id позиций по порядку.
async fn setup(app: &TestApp, db: &PgPool, me: &Auth) -> (String, Vec<String>) {
    let s = Synth::new(db.clone());
    let mut artists = Vec::new();
    for n in 0..4 {
        artists.push(s.artist(&format!("Артист {n}")).await);
    }
    let mut ids: Vec<Uuid> = Vec::new();
    for (i, t) in TRACKS.iter().enumerate() {
        let feat = match t {
            Some((bpm, key, energy, _)) => Feat {
                bpm: *bpm,
                key,
                energy: *energy,
                ..Feat::default()
            },
            None => Feat {
                analyzed: false,
                ..Feat::default()
            },
        };
        let artist = artists[t.map_or(i % 4, |t| t.3)];
        ids.push(s.track(artist, &format!("Трек {i}"), feat).await);
    }
    let res = app
        .post(
            "/api/playlists",
            json!({ "title": "Вечер", "trackIds": ids }),
            me,
        )
        .await;
    assert_eq!(res.status, StatusCode::CREATED, "{}", res.text);
    let id = res.body["playlist"]["id"].as_str().unwrap().to_string();
    (id, item_ids(&res.body))
}

fn item_ids(detail: &Value) -> Vec<String> {
    detail["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["id"].as_str().unwrap().to_string())
        .collect()
}

fn entry_ids(set: &Value) -> Vec<String> {
    set["entries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["itemId"].as_str().unwrap().to_string())
        .collect()
}

async fn order(app: &TestApp, me: &Auth, id: &str) -> Vec<String> {
    let res = app.get(&format!("/api/playlists/{id}"), me).await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    item_ids(&res.body)
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn preview_does_not_change_the_playlist(db: PgPool) {
    let app = TestApp::new(db.clone()).await;
    let me = app.listener("ann").await;
    let (id, items) = setup(&app, &db, &me).await;

    let res = app
        .post(
            &format!("/api/playlists/{id}/set/preview"),
            json!({ "curve": "rise" }),
            &me,
        )
        .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    let set = res.body;
    assert_eq!(set["curve"], "rise");
    assert_eq!(set["changed"], true);
    assert_eq!(set["readOnly"], false);
    assert_eq!(set["unplaced"], 2);
    let entries = set["entries"].as_array().unwrap();
    assert_eq!(entries.len(), items.len());
    let mut got = entry_ids(&set);
    // Без анализа — в конце, в прежнем порядке, без подсказки
    assert_eq!(&got[12..], &[items[10].clone(), items[12].clone()]);
    assert!(entries[..12].iter().all(|e| e["placed"] == true));
    assert!(
        entries[12..]
            .iter()
            .all(|e| e["placed"] == false && e["fit"].is_null() && e["energy"].is_null())
    );
    assert!(entries[0]["fit"].is_null());
    assert!(entries[1..12].iter().all(|e| e["fit"].is_string()));
    // Переходы стали плавнее, энергия растёт
    assert!(
        set["smoothAfter"].as_i64().unwrap() > set["smoothBefore"].as_i64().unwrap(),
        "{set}"
    );
    let energy: Vec<f64> = entries[..12]
        .iter()
        .map(|e| e["energy"].as_f64().unwrap())
        .collect();
    let mean = |s: &[f64]| s.iter().sum::<f64>() / s.len() as f64;
    assert!(mean(&energy[..4]) < mean(&energy[4..8]) && mean(&energy[4..8]) < mean(&energy[8..]));
    got.sort();
    let mut all = items.clone();
    all.sort();
    assert_eq!(got, all);

    // Детерминированно; плейлист не тронут
    let again = app
        .post(
            &format!("/api/playlists/{id}/set/preview"),
            json!({ "curve": "rise" }),
            &me,
        )
        .await;
    assert_eq!(again.body, set);
    assert_eq!(order(&app, &me, &id).await, items);

    // Пустое тело — нарастание; волна и ровно — другие порядки
    let plain = app
        .call(
            Method::POST,
            &format!("/api/playlists/{id}/set/preview"),
            None,
            &me,
        )
        .await;
    assert_eq!(plain.status, StatusCode::OK, "{}", plain.text);
    assert_eq!(plain.body, set);
    for curve in ["wave", "flat"] {
        let other = app
            .post(
                &format!("/api/playlists/{id}/set/preview"),
                json!({ "curve": curve }),
                &me,
            )
            .await;
        assert_eq!(other.status, StatusCode::OK, "{}", other.text);
        assert_eq!(other.body["curve"], curve);
        assert_ne!(entry_ids(&other.body), entry_ids(&set), "{curve}");
    }

    // Первый трек — по выбору
    let res = app
        .post(
            &format!("/api/playlists/{id}/set/preview"),
            json!({ "curve": "wave", "firstItemId": items[0] }),
            &me,
        )
        .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    assert_eq!(entry_ids(&res.body)[0], items[0]);
    let res = app
        .post(
            &format!("/api/playlists/{id}/set/preview"),
            json!({ "firstItemId": Uuid::now_v7() }),
            &me,
        )
        .await;
    assert_eq!(res.status, StatusCode::BAD_REQUEST, "{}", res.text);
    let res = app
        .post(
            &format!("/api/playlists/{id}/set/preview"),
            json!({ "curve": "zigzag" }),
            &me,
        )
        .await;
    assert_eq!(res.status, StatusCode::BAD_REQUEST, "{}", res.text);

    // Чужой плейлист — как несуществующий
    let bob = app.listener("bob").await;
    let res = app
        .post(&format!("/api/playlists/{id}/set/preview"), json!({}), &bob)
        .await;
    assert_eq!(res.status, StatusCode::NOT_FOUND, "{}", res.text);
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn apply_and_undo(db: PgPool) {
    let app = TestApp::new(db.clone()).await;
    let me = app.listener("ann").await;
    let (id, items) = setup(&app, &db, &me).await;

    let preview = app
        .post(
            &format!("/api/playlists/{id}/set/preview"),
            json!({ "curve": "wave" }),
            &me,
        )
        .await
        .body;
    let cursor = app.get("/api/sync", &me).await.body["cursor"]
        .as_str()
        .unwrap()
        .to_string();

    let res = app
        .post(
            &format!("/api/playlists/{id}/set"),
            json!({ "curve": "wave" }),
            &me,
        )
        .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    assert_eq!(res.body["set"], preview);
    let previous: Vec<String> = res.body["previousItemIds"]
        .as_array()
        .unwrap()
        .iter()
        .map(|x| x.as_str().unwrap().to_string())
        .collect();
    assert_eq!(previous, items);
    let applied = entry_ids(&preview);
    assert_eq!(order(&app, &me, &id).await, applied);

    // Новые позиции уходят на устройства тем же путём, что и другие правки порядка
    let res = app.get(&format!("/api/sync?since={cursor}"), &me).await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    let synced: Vec<&str> = res.body["playlistItems"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["id"].as_str().unwrap())
        .collect();
    assert!(!synced.is_empty());
    assert!(synced.iter().all(|s| items.iter().any(|i| i == s)));

    // Собрать ещё раз — уже собрано
    let res = app
        .post(
            &format!("/api/playlists/{id}/set/preview"),
            json!({ "curve": "wave" }),
            &me,
        )
        .await;
    assert_eq!(res.body["changed"], false, "{}", res.text);
    assert_eq!(entry_ids(&res.body), applied);

    // «Отменить» — прежний порядок
    let res = app
        .call(
            Method::PUT,
            &format!("/api/playlists/{id}/order"),
            Some(json!({ "itemIds": previous })),
            &me,
        )
        .await;
    assert_eq!(res.status, StatusCode::NO_CONTENT, "{}", res.text);
    assert_eq!(order(&app, &me, &id).await, items);

    // Порядок не целиком: названные — первыми, остальные — за ними; чужие и повторы пропущены
    let res = app
        .call(
            Method::PUT,
            &format!("/api/playlists/{id}/order"),
            Some(json!({ "itemIds": [items[5], Uuid::now_v7(), items[2], items[5]] })),
            &me,
        )
        .await;
    assert_eq!(res.status, StatusCode::NO_CONTENT, "{}", res.text);
    let mut expected = vec![items[5].clone(), items[2].clone()];
    expected.extend(
        items
            .iter()
            .filter(|i| **i != items[5] && **i != items[2])
            .cloned(),
    );
    assert_eq!(order(&app, &me, &id).await, expected);
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn mixes_are_read_only(db: PgPool) {
    let app = TestApp::new(db.clone()).await;
    let me = app.listener("ann").await;
    let (id, items) = setup(&app, &db, &me).await;
    sqlx::query("UPDATE playlists SET kind = 'mix' WHERE id = $1::uuid")
        .bind(&id)
        .execute(&db)
        .await
        .unwrap();

    // Предпросмотр есть, применить — только к своей копии
    let res = app
        .post(&format!("/api/playlists/{id}/set/preview"), json!({}), &me)
        .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    assert_eq!(res.body["readOnly"], true);
    let res = app
        .post(&format!("/api/playlists/{id}/set"), json!({}), &me)
        .await;
    assert_eq!(res.status, StatusCode::CONFLICT, "{}", res.text);
    assert_eq!(res.code(), "read_only");
    let res = app
        .call(
            Method::PUT,
            &format!("/api/playlists/{id}/order"),
            Some(json!({ "itemIds": [items[3]] })),
            &me,
        )
        .await;
    assert_eq!(res.status, StatusCode::CONFLICT, "{}", res.text);
    assert_eq!(order(&app, &me, &id).await, items);

    let copy = app
        .post(&format!("/api/playlists/{id}/duplicate"), json!({}), &me)
        .await;
    assert_eq!(copy.status, StatusCode::CREATED, "{}", copy.text);
    let copy = copy.body["id"].as_str().unwrap().to_string();
    let res = app
        .post(&format!("/api/playlists/{copy}/set"), json!({}), &me)
        .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    assert_eq!(res.body["set"]["changed"], true);
}
