//! Настройки слушателя и фильтр «Без нецензурного»: значения по умолчанию, правка, синхронизация,
//! explicit-треки не попадают в волну, чистая версия встаёт вместо explicit.

use axum::http::{Method, StatusCode};
use dalywave_server::reco;
use serde_json::{Value, json};
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    common::{Auth, TestApp},
    synth::{Feat, Synth},
};

async fn user_of(app: &TestApp, auth: &Auth) -> Uuid {
    app.user_id(auth).await.parse().unwrap()
}

async fn patch(app: &TestApp, auth: &Auth, body: Value) -> crate::common::Resp {
    app.call(Method::PATCH, "/api/settings", Some(body), auth)
        .await
}

async fn mark_explicit(db: &PgPool, track: Uuid) {
    sqlx::query("UPDATE tracks SET explicit = true WHERE id = $1")
        .bind(track)
        .execute(db)
        .await
        .unwrap();
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn settings_have_defaults_change_and_sync(db: PgPool) {
    let app = TestApp::new(db).await;
    let me = app.listener("ann").await;

    let res = app.get("/api/settings", &me).await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    assert_eq!(
        res.body,
        json!({ "hideExplicit": false, "qualityWifi": "high", "qualityCellular": "normal",
                "qualityDownload": "high", "playlistSort": "recent", "updatedAt": null })
    );
    // Синхронизация без правок настроек не несёт
    let first = app.get("/api/sync", &me).await;
    assert!(first.body["settings"].is_null(), "{}", first.text);
    let cursor = first.body["cursor"].as_str().unwrap().to_string();

    let res = patch(
        &app,
        &me,
        json!({ "hideExplicit": true, "qualityCellular": "saver" }),
    )
    .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    assert_eq!(res.body["hideExplicit"], true);
    assert_eq!(res.body["qualityCellular"], "saver");
    assert_eq!(res.body["qualityWifi"], "high", "остальное как было");
    assert!(res.body["updatedAt"].is_string());
    // Только своё: у другого слушателя всё по умолчанию
    let bob = app.listener("bob").await;
    assert_eq!(
        app.get("/api/settings", &bob).await.body["hideExplicit"],
        false
    );

    let res = app.get(&format!("/api/sync?since={cursor}"), &me).await;
    assert_eq!(res.body["settings"]["hideExplicit"], true, "{}", res.text);
    assert_eq!(res.body["settings"]["qualityCellular"], "saver");
    let rev = res.body["settings"]["rev"].as_i64().unwrap();
    assert!(rev > 0);
    // Повтор возможен (полнота по id транзакции), но той же строкой
    let after = res.body["cursor"].as_str().unwrap().to_string();
    let res = app.get(&format!("/api/sync?since={after}"), &me).await;
    let again = &res.body["settings"];
    assert!(again.is_null() || again["rev"] == rev, "{}", res.text);

    for bad in [
        json!({ "qualityCellular": "lossless" }),
        json!({ "qualityWifi": "saver" }),
        json!({ "bass": 9 }),
        json!({ "playlistSort": "random" }),
    ] {
        assert_eq!(
            patch(&app, &me, bad.clone()).await.status,
            StatusCode::BAD_REQUEST,
            "{bad}"
        );
    }
    assert_eq!(
        app.get("/api/settings", &Auth::None).await.status,
        StatusCode::UNAUTHORIZED
    );
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn explicit_tracks_leave_the_wave_and_clean_versions_replace_them(db: PgPool) {
    let app = TestApp::new(db).await;
    let me = app.listener("ann").await;
    let user = user_of(&app, &me).await;
    let s = Synth::new(app.state.db.clone());
    // Вкус и находки рядом с ним: треть — explicit
    let a = s.artist("Liked").await;
    for i in 0..4 {
        let t = s
            .track(
                a,
                &format!("liked {i}"),
                Feat {
                    jitter: i,
                    energy: 0.8,
                    ..Default::default()
                },
            )
            .await;
        s.like(user, t, 10).await;
    }
    let mut explicit = Vec::new();
    let mut clean = Vec::new();
    for i in 0..12 {
        let artist = s.artist(&format!("New {i}")).await;
        let t = s
            .track(
                artist,
                &format!("fresh {i}"),
                Feat {
                    jitter: 20 + i,
                    energy: 0.75,
                    ..Default::default()
                },
            )
            .await;
        if i % 3 == 0 {
            mark_explicit(&app.state.db, t).await;
            explicit.push(t);
        } else {
            clean.push(t);
        }
    }
    // У первого explicit — чистая версия той же песни того же артиста (далеко от вкуса: сама не пройдёт)
    let artist0: Uuid =
        sqlx::query_scalar("SELECT artist_id FROM track_artists WHERE track_id = $1")
            .bind(explicit[0])
            .fetch_one(&app.state.db)
            .await
            .unwrap();
    let twin = s
        .track(
            artist0,
            "fresh 0",
            Feat {
                axis: 700,
                ..Default::default()
            },
        )
        .await;
    // Ремикс той же песни — не чистая версия
    let remix = s.track(artist0, "fresh 0", Feat::default()).await;
    sqlx::query("UPDATE tracks SET title_version = 'Remix' WHERE id = $1")
        .bind(remix)
        .execute(&app.state.db)
        .await
        .unwrap();

    // Трек каталога знает свою чистую версию
    let res = app.get(&format!("/api/tracks/{}", explicit[0]), &me).await;
    assert_eq!(res.body["explicit"], true, "{}", res.text);
    assert_eq!(res.body["cleanId"], twin.to_string());
    let res = app.get(&format!("/api/tracks/{}", explicit[1]), &me).await;
    assert!(res.body["cleanId"].is_null());
    let res = app.get(&format!("/api/tracks/{twin}"), &me).await;
    assert!(res.body["cleanId"].is_null(), "у чистой нет своей чистой");

    // Фильтр выключен — подмен нет, explicit не исключён
    assert!(
        reco::clean_swaps(&app.state.db, user, &explicit)
            .await
            .unwrap()
            .is_empty()
    );
    let cands = reco::candidates(&app.state.db, &explicit, user, None, None)
        .await
        .unwrap();
    assert!(cands.iter().all(|c| !c.excluded));

    assert_eq!(
        patch(&app, &me, json!({ "hideExplicit": true }))
            .await
            .status,
        StatusCode::OK
    );
    let swaps = reco::clean_swaps(&app.state.db, user, &explicit)
        .await
        .unwrap();
    assert_eq!(swaps.len(), 1);
    assert_eq!(swaps[&explicit[0]], twin);
    assert_eq!(
        reco::apply_swaps(&[explicit[0], twin, clean[0]], &swaps),
        vec![twin, clean[0]],
        "чистая версия не повторяется"
    );
    let cands = reco::candidates(&app.state.db, &explicit, user, None, None)
        .await
        .unwrap();
    assert!(cands.iter().all(|c| c.excluded), "explicit отсеивается");

    // Волна: ни одного explicit, сколько ни проси
    let res = app
        .post("/api/wave/sessions", json!({ "settings": {} }), &me)
        .await;
    assert_eq!(res.status, StatusCode::CREATED, "{}", res.text);
    let id = res.body["id"].as_str().unwrap().to_string();
    let mut items: Vec<Value> = res.body["queue"].as_array().unwrap().clone();
    let more = app
        .get(&format!("/api/wave/sessions/{id}/next?count=20"), &me)
        .await;
    assert_eq!(more.status, StatusCode::OK, "{}", more.text);
    items.extend(more.body["items"].as_array().unwrap().clone());
    assert!(!items.is_empty());
    for item in &items {
        assert_eq!(item["track"]["explicit"], false, "{item}");
        let id: Uuid = item["track"]["id"].as_str().unwrap().parse().unwrap();
        assert!(!explicit.contains(&id));
    }
}
