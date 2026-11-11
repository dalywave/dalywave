//! Настоящее S3: SeaweedFS из `make up` с настройками `DALYWAVE_S3_*` из `.env`. В CI его нет, поэтому
//! тест запускается вручную: `cargo test -p dalywave-server --test api storage -- --ignored`.

use dalywave_server::{
    Config,
    storage::{AUDIO_GET_TTL, IMMUTABLE, Storage, UPLOAD_PUT_TTL},
};
use futures_util::TryStreamExt;

#[tokio::test]
#[ignore = "нужен SeaweedFS из make up и DALYWAVE_S3_* в .env"]
async fn seaweedfs_roundtrip_and_presigned_urls() {
    let _ = dotenvy::from_path(concat!(env!("CARGO_MANIFEST_DIR"), "/../../.env"));
    let mut cfg = Config::from_env()
        .unwrap()
        .s3
        .expect("DALYWAVE_S3_* are not set");
    cfg.bucket = "dalywave-test".into();
    let storage = Storage::from_config(&cfg).unwrap();
    // Бакет создаётся при первом запуске, дальше — уже есть
    storage.ensure_bucket().await.unwrap();
    assert!(!storage.ensure_bucket().await.unwrap());

    let key = format!("images/test/{}/250.jpg", uuid::Uuid::now_v7());
    storage
        .put(&key, b"jpeg bytes".to_vec(), "image/jpeg", Some(IMMUTABLE))
        .await
        .unwrap();
    assert_eq!(storage.head(&key).await.unwrap().unwrap().size, 10);
    let object = storage.get(&key).await.unwrap();
    assert_eq!(object.content_type.as_deref(), Some("image/jpeg"));
    let body: Vec<bytes::Bytes> = object.stream.try_collect().await.unwrap();
    assert_eq!(body.concat(), b"jpeg bytes");

    // Подписанные ссылки: загрузчик кладёт файл PUT-ом, плеер читает GET-ом — без ключей
    let audio = format!(
        "audio/{}/{}.ogg",
        uuid::Uuid::now_v7(),
        uuid::Uuid::now_v7()
    );
    let http = reqwest::Client::new();
    let put = storage.presign_put(&audio, UPLOAD_PUT_TTL).await.unwrap();
    let res = http
        .put(put)
        .header("content-type", "audio/ogg")
        .body(vec![7u8; 4096])
        .send()
        .await
        .unwrap();
    assert!(res.status().is_success(), "{}", res.status());
    let get = storage.presign_get(&audio, AUDIO_GET_TTL).await.unwrap();
    let res = http.get(get.clone()).send().await.unwrap();
    assert!(res.status().is_success(), "{}", res.status());
    assert_eq!(res.bytes().await.unwrap().len(), 4096);
    // Подпись не подделать: испорченная ссылка не работает
    let broken = get
        .as_str()
        .replace("X-Amz-Signature=", "X-Amz-Signature=00");
    assert!(!http.get(broken).send().await.unwrap().status().is_success());

    storage.delete(&key).await.unwrap();
    storage.delete(&audio).await.unwrap();
    assert!(storage.head(&key).await.unwrap().is_none());
}
