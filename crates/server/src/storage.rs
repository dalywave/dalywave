//! Хранилище файлов, S3-совместимое: SeaweedFS локально и Cloudflare R2 в проде — один и тот же код.
//!
//! Ключи: `audio/{recordingId}/{fileId}.{ext}`, `images/albums/{id}/{size}.jpg`, `images/artists/{id}/{size}.jpg`.
//! Аудио клиенты качают напрямую по подписанной ссылке; подписывается адрес `S3_PUBLIC_ENDPOINT`, если он задан.

use std::{fmt, sync::Arc, time::Duration};

use anyhow::Context;
use bytes::Bytes;
use dalywave_protocol::jobs::ImageOwner;
use futures_util::{StreamExt, stream::BoxStream};
use object_store::{
    Attribute, Attributes, ObjectStore, ObjectStoreExt, PutOptions, PutPayload,
    aws::{AmazonS3, AmazonS3Builder},
    memory::InMemory,
    path::Path,
    signer::{Method, Signer},
};
use url::Url;
use uuid::Uuid;

use crate::config::S3Config;

/// Картинки по адресу не меняются: смена обложки меняет `?v=` в ссылке.
pub const IMMUTABLE: &str = "public, max-age=31536000, immutable";
/// Ссылка на аудио живёт несколько часов: столько может длиться пауза плеера.
pub const AUDIO_GET_TTL: Duration = Duration::from_secs(6 * 3600);
/// Загрузчику хватает получаса на скачивание и загрузку одного трека.
pub const UPLOAD_PUT_TTL: Duration = Duration::from_secs(30 * 60);

/// Размеры копий картинок.
pub const IMAGE_SIZES: [u32; 3] = [1000, 500, 250];

pub mod keys {
    use super::*;

    pub fn audio(recording_id: Uuid, file_id: Uuid, ext: &str) -> String {
        format!("audio/{recording_id}/{file_id}.{ext}")
    }

    /// Папка копий картинки; сами файлы — `{prefix}/{size}.jpg`.
    pub fn image_prefix(owner: ImageOwner, id: Uuid) -> String {
        let dir = match owner {
            ImageOwner::Album => "albums",
            ImageOwner::Artist => "artists",
        };
        format!("images/{dir}/{id}")
    }

    pub fn image(prefix: &str, size: u32) -> String {
        format!("{prefix}/{size}.jpg")
    }
}

#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("нет такого файла")]
    NotFound,
    #[error(transparent)]
    Store(#[from] object_store::Error),
    #[error("неверный ключ: {0}")]
    Key(#[from] object_store::path::Error),
}

/// Файл, который читается потоком.
pub struct Object {
    pub size: u64,
    pub content_type: Option<String>,
    pub e_tag: Option<String>,
    pub stream: BoxStream<'static, Result<Bytes, object_store::Error>>,
}

pub struct Meta {
    pub size: u64,
    pub e_tag: Option<String>,
}

pub struct Storage {
    store: Arc<dyn ObjectStore>,
    /// Подписывает ссылки для клиентов: публичный адрес хранилища
    signer: Arc<dyn Signer>,
    /// S3 по внутреннему адресу; нет у хранилища в памяти
    s3: Option<Arc<AmazonS3>>,
    http: reqwest::Client,
    bucket: String,
}

impl fmt::Debug for Storage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Storage")
            .field("bucket", &self.bucket)
            .field("memory", &self.s3.is_none())
            .finish()
    }
}

fn s3_builder(cfg: &S3Config, endpoint: &str) -> AmazonS3Builder {
    AmazonS3Builder::new()
        .with_endpoint(endpoint)
        .with_allow_http(endpoint.starts_with("http://"))
        .with_region(&cfg.region)
        .with_bucket_name(&cfg.bucket)
        .with_access_key_id(&cfg.access_key_id)
        .with_secret_access_key(&cfg.secret_access_key)
}

impl Storage {
    pub fn from_config(cfg: &S3Config) -> anyhow::Result<Self> {
        let s3 = Arc::new(
            s3_builder(cfg, &cfg.endpoint)
                .build()
                .context("S3 settings")?,
        );
        let signer: Arc<dyn Signer> = match &cfg.public_endpoint {
            Some(public) => Arc::new(
                s3_builder(cfg, public)
                    .build()
                    .context("S3 public endpoint")?,
            ),
            None => s3.clone(),
        };
        Ok(Self {
            store: s3.clone(),
            signer,
            s3: Some(s3),
            http: reqwest::Client::builder()
                .timeout(Duration::from_secs(15))
                .build()?,
            bucket: cfg.bucket.clone(),
        })
    }

    /// Хранилище в памяти процесса — для тестов и разработки без S3. Ссылки подписываются как для
    /// `http://storage.invalid`, по ним ничего не скачать.
    pub fn memory() -> Self {
        let cfg = S3Config {
            endpoint: "http://storage.invalid".into(),
            public_endpoint: None,
            region: "auto".into(),
            bucket: "dalywave".into(),
            access_key_id: "memory".into(),
            secret_access_key: "memory".into(),
        };
        let signer = match s3_builder(&cfg, &cfg.endpoint).build() {
            Ok(s) => s,
            // Сборка без сети не падает: все поля заданы выше
            Err(e) => unreachable!("in-memory signer: {e}"),
        };
        Self {
            store: Arc::new(InMemory::new()),
            signer: Arc::new(signer),
            s3: None,
            http: reqwest::Client::new(),
            bucket: cfg.bucket,
        }
    }

    pub fn is_memory(&self) -> bool {
        self.s3.is_none()
    }

    /// Создаёт бакет, если его нет (SeaweedFS после `make up`). У R2 бакет заводится в панели, а ключ
    /// ограничен им, поэтому создание пробуется только когда бакета действительно нет.
    pub async fn ensure_bucket(&self) -> anyhow::Result<bool> {
        let Some(s3) = &self.s3 else {
            return Ok(false);
        };
        match self
            .store
            .list_with_delimiter(Some(&Path::from("images")))
            .await
        {
            Ok(_) => return Ok(false),
            Err(object_store::Error::NotFound { .. }) => {}
            Err(e) if e.to_string().contains("NoSuchBucket") => {}
            Err(e) => return Err(e).context("check the S3 bucket"),
        }
        // PUT на корень бакета — CreateBucket в адресации по пути
        let url = s3
            .signed_url(Method::PUT, &Path::from(""), Duration::from_secs(60))
            .await?;
        let res = self.http.put(url).send().await?;
        let status = res.status();
        if !status.is_success() {
            let body = res.text().await.unwrap_or_default();
            anyhow::bail!("create bucket {}: HTTP {status}: {body}", self.bucket);
        }
        tracing::info!(bucket = %self.bucket, "S3 bucket created");
        Ok(true)
    }

    pub async fn put(
        &self,
        key: &str,
        data: impl Into<Bytes>,
        content_type: &str,
        cache_control: Option<&str>,
    ) -> Result<(), StorageError> {
        let path = Path::parse(key)?;
        let mut attributes = Attributes::new();
        attributes.insert(Attribute::ContentType, content_type.to_string().into());
        if let Some(cc) = cache_control {
            attributes.insert(Attribute::CacheControl, cc.to_string().into());
        }
        let opts = PutOptions {
            attributes,
            ..Default::default()
        };
        self.store
            .put_opts(&path, PutPayload::from(data.into()), opts)
            .await?;
        Ok(())
    }

    /// Файл потоком; `NotFound`, если его нет.
    pub async fn get(&self, key: &str) -> Result<Object, StorageError> {
        let path = Path::parse(key)?;
        let res = match self.store.get(&path).await {
            Ok(res) => res,
            Err(object_store::Error::NotFound { .. }) => return Err(StorageError::NotFound),
            Err(e) => return Err(e.into()),
        };
        let content_type = res
            .attributes
            .get(&Attribute::ContentType)
            .map(|v| v.to_string());
        Ok(Object {
            size: res.meta.size,
            content_type,
            e_tag: res.meta.e_tag.clone(),
            stream: res.into_stream().boxed(),
        })
    }

    pub async fn get_bytes(&self, key: &str) -> Result<Bytes, StorageError> {
        let path = Path::parse(key)?;
        match self.store.get(&path).await {
            Ok(res) => Ok(res.bytes().await?),
            Err(object_store::Error::NotFound { .. }) => Err(StorageError::NotFound),
            Err(e) => Err(e.into()),
        }
    }

    pub async fn head(&self, key: &str) -> Result<Option<Meta>, StorageError> {
        let path = Path::parse(key)?;
        match self.store.head(&path).await {
            Ok(meta) => Ok(Some(Meta {
                size: meta.size,
                e_tag: meta.e_tag,
            })),
            Err(object_store::Error::NotFound { .. }) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Удаление; отсутствующий файл — не ошибка.
    pub async fn delete(&self, key: &str) -> Result<(), StorageError> {
        let path = Path::parse(key)?;
        match self.store.delete(&path).await {
            Ok(()) | Err(object_store::Error::NotFound { .. }) => Ok(()),
            Err(e) => Err(e.into()),
        }
    }

    /// Подписанная ссылка на скачивание (аудио — на [`AUDIO_GET_TTL`]).
    pub async fn presign_get(&self, key: &str, ttl: Duration) -> Result<Url, StorageError> {
        let path = Path::parse(key)?;
        Ok(self.signer.signed_url(Method::GET, &path, ttl).await?)
    }

    /// Подписанная ссылка на загрузку (загрузчику — на [`UPLOAD_PUT_TTL`]). Content-Type в подпись не входит.
    pub async fn presign_put(&self, key: &str, ttl: Duration) -> Result<Url, StorageError> {
        let path = Path::parse(key)?;
        Ok(self.signer.signed_url(Method::PUT, &path, ttl).await?)
    }
}

#[cfg(test)]
mod tests {
    use futures_util::TryStreamExt;

    use super::*;

    #[test]
    fn key_layout() {
        let id = Uuid::nil();
        assert_eq!(keys::audio(id, id, "ogg"), format!("audio/{id}/{id}.ogg"));
        let prefix = keys::image_prefix(ImageOwner::Album, id);
        assert_eq!(prefix, format!("images/albums/{id}"));
        assert_eq!(
            keys::image(&prefix, 500),
            format!("images/albums/{id}/500.jpg")
        );
        assert_eq!(
            keys::image_prefix(ImageOwner::Artist, id),
            format!("images/artists/{id}")
        );
    }

    #[tokio::test]
    async fn memory_roundtrip_and_signing() {
        let s = Storage::memory();
        assert!(!s.ensure_bucket().await.unwrap());
        s.put(
            "images/a/250.jpg",
            vec![1u8, 2, 3],
            "image/jpeg",
            Some(IMMUTABLE),
        )
        .await
        .unwrap();
        let obj = s.get("images/a/250.jpg").await.unwrap();
        assert_eq!(obj.size, 3);
        assert_eq!(obj.content_type.as_deref(), Some("image/jpeg"));
        let chunks: Vec<Bytes> = obj.stream.try_collect().await.unwrap();
        assert_eq!(chunks.concat(), vec![1, 2, 3]);
        assert_eq!(s.head("images/a/250.jpg").await.unwrap().unwrap().size, 3);
        assert!(s.head("images/a/500.jpg").await.unwrap().is_none());
        assert!(matches!(
            s.get("images/a/500.jpg").await,
            Err(StorageError::NotFound)
        ));
        s.delete("images/a/250.jpg").await.unwrap();
        s.delete("images/a/250.jpg").await.unwrap();

        let url = s
            .presign_get("audio/r/f.ogg", AUDIO_GET_TTL)
            .await
            .unwrap()
            .to_string();
        assert!(
            url.starts_with("http://storage.invalid/dalywave/audio/r/f.ogg?"),
            "{url}"
        );
        assert!(url.contains("X-Amz-Expires=21600"), "{url}");
        let put = s
            .presign_put("audio/r/f.ogg", UPLOAD_PUT_TTL)
            .await
            .unwrap()
            .to_string();
        assert!(put.contains("X-Amz-Expires=1800"), "{put}");
    }

    #[tokio::test]
    async fn public_endpoint_signs_links() {
        let cfg = S3Config {
            endpoint: "http://seaweedfs:8333".into(),
            public_endpoint: Some("https://files.example.com".into()),
            region: "auto".into(),
            bucket: "dalywave".into(),
            access_key_id: "k".into(),
            secret_access_key: "s".into(),
        };
        let s = Storage::from_config(&cfg).unwrap();
        let url = s.presign_get("audio/x.ogg", AUDIO_GET_TTL).await.unwrap();
        assert_eq!(url.host_str(), Some("files.example.com"));
        assert_eq!(url.path(), "/dalywave/audio/x.ogg");
    }
}
