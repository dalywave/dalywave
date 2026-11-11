use axum::http::{StatusCode, header};
use dalywave_server::jobs::worker::Outcome;
use serde_json::Value;
use sqlx::PgPool;

use crate::{
    common::{Auth, TestApp},
    deezer::{LOWBORN, MODERN_SADNESS},
};

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn covers_are_mirrored_with_palette(db: PgPool) {
    let app = TestApp::new(db).await;
    let (_, admin) = app.admin("boss").await;
    let album = app.resolve("album", MODERN_SADNESS, &admin).await;
    let id = album["id"].as_str().unwrap().to_string();
    assert!(album["palette"].is_null());
    let url = album["image"]["medium"].as_str().unwrap().to_string();
    assert!(
        url.starts_with(&format!("/api/images/albums/{id}/500.jpg?v=")),
        "{url}"
    );

    // Копии ещё нет: временный редирект на Deezer нужного размера, копия — в очередь
    let res = app.get(&url, &Auth::None).await;
    assert_eq!(res.status, StatusCode::FOUND);
    let location = res.location();
    assert!(location.starts_with(&app.deezer.base), "{location}");
    assert!(
        location.ends_with("/500x500-000000-80-0-0.jpg"),
        "{location}"
    );
    assert_eq!(res.headers[header::CACHE_CONTROL], "no-store");
    app.get(&url, &Auth::None).await;
    let queued: i64 = sqlx::query_scalar("SELECT count(*) FROM jobs WHERE kind = 'mirror_image'")
        .fetch_one(&app.state.db)
        .await
        .unwrap();
    assert_eq!(queued, 1, "одна задача на картинку");

    let (kind, outcome) = app.run_job("mirror").await.unwrap();
    assert_eq!(kind, "mirror_image");
    assert_eq!(outcome, Outcome::Done);
    let (key, bytes, palette): (Option<String>, Option<i32>, Option<Value>) = sqlx::query_as(
        "SELECT cover_key, cover_bytes, cover_palette FROM albums WHERE id = $1::uuid",
    )
    .bind(&id)
    .fetch_one(&app.state.db)
    .await
    .unwrap();
    assert_eq!(key.as_deref(), Some(format!("images/albums/{id}").as_str()));
    assert!(bytes.unwrap() > 1000);
    let palette = palette.unwrap();
    let colors = palette.as_array().unwrap();
    assert!((3..=5).contains(&colors.len()), "{palette}");
    for size in [250, 500, 1000] {
        let object = app
            .state
            .storage
            .get_bytes(&format!("images/albums/{id}/{size}.jpg"))
            .await
            .unwrap();
        let img = image::load_from_memory(&object).unwrap();
        assert_eq!((img.width(), img.height()), (size, size));
    }

    // Теперь картинка отдаётся сама, с вечным кешем; адрес в DTO тот же, палитра появилась
    let res = app.get(&url, &Auth::None).await;
    assert_eq!(res.status, StatusCode::OK);
    assert_eq!(res.headers[header::CONTENT_TYPE], "image/jpeg");
    assert_eq!(
        res.headers[header::CACHE_CONTROL],
        "public, max-age=31536000, immutable"
    );
    let img = image::load_from_memory(&res.bytes).unwrap();
    assert_eq!((img.width(), img.height()), (500, 500));
    assert_eq!(
        res.headers[header::CONTENT_LENGTH],
        res.bytes.len().to_string()
    );
    let res = app.get(&format!("/api/albums/{id}"), &admin).await;
    assert_eq!(res.body["album"]["image"]["medium"], url.as_str());
    assert_eq!(res.body["album"]["palette"], palette);

    for bad in [
        format!("/api/images/albums/{id}/300.jpg"),
        format!("/api/images/albums/{id}/500.png"),
        format!("/api/images/covers/{id}/500.jpg"),
        format!("/api/images/albums/{}/500.jpg", uuid::Uuid::now_v7()),
    ] {
        assert_eq!(
            app.get(&bad, &Auth::None).await.status,
            StatusCode::NOT_FOUND,
            "{bad}"
        );
    }

    let overview = app.get("/api/admin/overview", &admin).await;
    assert_eq!(overview.body["storage"]["images"], 1);
    assert_eq!(overview.body["storage"]["imageBytes"], bytes.unwrap());
}

#[sqlx::test(migrator = "dalywave_server::MIGRATOR")]
async fn artist_photos_are_mirrored_without_palette(db: PgPool) {
    let app = TestApp::new(db).await;
    let (_, admin) = app.admin("boss").await;
    let artist = app.resolve("artist", LOWBORN, &admin).await;
    let id = artist["id"].as_str().unwrap().to_string();
    let url = artist["image"]["small"].as_str().unwrap().to_string();
    assert_eq!(app.get(&url, &Auth::None).await.status, StatusCode::FOUND);
    let (kind, outcome) = app.run_job("mirror").await.unwrap();
    assert_eq!((kind.as_str(), outcome), ("mirror_image", Outcome::Done));
    let res = app.get(&url, &Auth::None).await;
    assert_eq!(res.status, StatusCode::OK);

    // Копия пропала из хранилища — снова редирект и новая копия
    app.state
        .storage
        .delete(&format!("images/artists/{id}/250.jpg"))
        .await
        .unwrap();
    assert_eq!(app.get(&url, &Auth::None).await.status, StatusCode::FOUND);
    let (_, outcome) = app.run_job("mirror").await.unwrap();
    assert_eq!(outcome, Outcome::Done);
    assert_eq!(app.get(&url, &Auth::None).await.status, StatusCode::OK);

    // Смена фото у Deezer: копия считается устаревшей, адрес в DTO меняется
    let before =
        app.get(&format!("/api/artists/{id}"), &admin).await.body["artist"]["image"]["small"]
            .clone();
    let mut card = crate::deezer::fixture(&format!("artist_{LOWBORN}"));
    card["picture_xl"] = Value::String(
        "https://cdn-images.dzcdn.net/images/artist/ffffffffffffffffffffffffffffffff/1000x1000-000000-80-0-0.jpg".into(),
    );
    app.deezer.set(&format!("/artist/{LOWBORN}"), card);
    sqlx::query("UPDATE artists SET fetched_at = NULL WHERE id = $1::uuid")
        .bind(&id)
        .execute(&app.state.db)
        .await
        .unwrap();
    let after =
        app.get(&format!("/api/artists/{id}"), &admin).await.body["artist"]["image"]["small"]
            .clone();
    assert_ne!(before, after);
    let key: Option<String> =
        sqlx::query_scalar("SELECT picture_key FROM artists WHERE id = $1::uuid")
            .bind(&id)
            .fetch_one(&app.state.db)
            .await
            .unwrap();
    assert!(key.is_none());
}
