//! S3-совместимое хранилище (R2, SeaweedFS) и хранилище в памяти процесса — через `object_store`.

use std::{fmt, ops::Range, path::Path as FsPath, sync::Arc, time::Duration};

use anyhow::Context;
use bytes::Bytes;
use futures_util::{FutureExt, StreamExt, TryStreamExt, future::BoxFuture};
use object_store::{
    Attribute, Attributes, GetOptions, GetRange, ObjectStore, ObjectStoreExt, PutOptions,
    PutPayload,
    aws::{AmazonS3, AmazonS3Builder},
    memory::InMemory,
    path::Path,
    signer::{Method, SignedUrlOptions, Signer},
};
use url::Url;

use super::{FileStore, Meta, Object, Presign, StorageError};
use crate::config::{S3Config, StorageKind};

pub struct ObjectStoreBackend {
    store: Arc<dyn ObjectStore>,
    /// Подписывает ссылки для клиентов: публичный адрес хранилища
    signer: Arc<dyn Signer>,
    /// Ссылки сервису анализа, если он видит хранилище по другому адресу
    ml_signer: Option<Arc<dyn Signer>>,
    /// S3 по внутреннему адресу; нет у хранилища в памяти
    s3: Option<Arc<AmazonS3>>,
    http: reqwest::Client,
    bucket: String,
}

impl fmt::Debug for ObjectStoreBackend {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ObjectStore")
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

impl ObjectStoreBackend {
    pub fn s3(cfg: &S3Config) -> anyhow::Result<Self> {
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
        let ml_signer: Option<Arc<dyn Signer>> = match &cfg.ml_endpoint {
            Some(ml) => Some(Arc::new(
                s3_builder(cfg, ml).build().context("S3 endpoint for ml")?,
            )),
            None => None,
        };
        Ok(Self {
            store: s3.clone(),
            signer,
            ml_signer,
            s3: Some(s3),
            http: reqwest::Client::builder()
                .timeout(Duration::from_secs(15))
                .build()?,
            bucket: cfg.bucket.clone(),
        })
    }

    /// В памяти процесса. Ссылки подписываются как для `http://storage.invalid`, по ним ничего не скачать.
    pub fn memory() -> Self {
        let cfg = S3Config {
            endpoint: "http://storage.invalid".into(),
            public_endpoint: None,
            ml_endpoint: None,
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
            ml_signer: None,
            s3: None,
            http: reqwest::Client::new(),
            bucket: cfg.bucket,
        }
    }

    /// Создаёт бакет, если его нет (SeaweedFS после `make up`). У R2 бакет заводится в панели, а ключ
    /// ограничен им, поэтому создание пробуется только когда бакета действительно нет.
    async fn ensure_bucket(&self) -> anyhow::Result<bool> {
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

    async fn put_file_inner(
        &self,
        key: &str,
        file: &FsPath,
        content_type: &str,
    ) -> Result<u64, StorageError> {
        use tokio::io::AsyncReadExt;
        let path = Path::parse(key)?;
        let mut attributes = Attributes::new();
        attributes.insert(Attribute::ContentType, content_type.to_string().into());
        let opts = object_store::PutMultipartOptions {
            attributes,
            ..Default::default()
        };
        let upload = self.store.put_multipart_opts(&path, opts).await?;
        let mut writer = object_store::WriteMultipart::new(upload);
        let mut f = tokio::fs::File::open(file).await?;
        let mut buf = vec![0u8; 1024 * 1024];
        let mut total = 0u64;
        loop {
            let n = match f.read(&mut buf).await {
                Ok(n) => n,
                Err(e) => {
                    let _ = writer.abort().await;
                    return Err(e.into());
                }
            };
            if n == 0 {
                break;
            }
            writer.wait_for_capacity(4).await?;
            writer.write(&buf[..n]);
            total += n as u64;
        }
        writer.finish().await?;
        Ok(total)
    }

    async fn get_inner(
        &self,
        key: &str,
        range: Option<Range<u64>>,
    ) -> Result<Object, StorageError> {
        let path = Path::parse(key)?;
        let opts = GetOptions {
            range: range.clone().map(GetRange::Bounded),
            ..Default::default()
        };
        let res = match self.store.get_opts(&path, opts).await {
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
            range,
            content_type,
            e_tag: res.meta.e_tag.clone(),
            stream: res.into_stream().map_err(StorageError::from).boxed(),
        })
    }
}

impl FileStore for ObjectStoreBackend {
    fn kind(&self) -> StorageKind {
        if self.s3.is_some() {
            StorageKind::S3
        } else {
            StorageKind::Memory
        }
    }

    fn put<'a>(
        &'a self,
        key: &'a str,
        data: Bytes,
        content_type: &'a str,
        cache_control: Option<&'a str>,
    ) -> BoxFuture<'a, Result<(), StorageError>> {
        async move {
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
                .put_opts(&path, PutPayload::from(data), opts)
                .await?;
            Ok(())
        }
        .boxed()
    }

    fn put_file<'a>(
        &'a self,
        key: &'a str,
        file: &'a FsPath,
        content_type: &'a str,
    ) -> BoxFuture<'a, Result<u64, StorageError>> {
        self.put_file_inner(key, file, content_type).boxed()
    }

    fn get<'a>(
        &'a self,
        key: &'a str,
        range: Option<Range<u64>>,
    ) -> BoxFuture<'a, Result<Object, StorageError>> {
        self.get_inner(key, range).boxed()
    }

    fn head<'a>(&'a self, key: &'a str) -> BoxFuture<'a, Result<Option<Meta>, StorageError>> {
        async move {
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
        .boxed()
    }

    fn delete<'a>(&'a self, key: &'a str) -> BoxFuture<'a, Result<(), StorageError>> {
        async move {
            let path = Path::parse(key)?;
            match self.store.delete(&path).await {
                Ok(()) | Err(object_store::Error::NotFound { .. }) => Ok(()),
                Err(e) => Err(e.into()),
            }
        }
        .boxed()
    }

    fn ensure(&self) -> BoxFuture<'_, anyhow::Result<bool>> {
        self.ensure_bucket().boxed()
    }

    fn presign<'a>(
        &'a self,
        key: &'a str,
        how: Presign<'a>,
        ttl: Duration,
    ) -> BoxFuture<'a, Result<Option<Url>, StorageError>> {
        async move {
            let path = Path::parse(key)?;
            let url = match how {
                Presign::Get => self.signer.signed_url(Method::GET, &path, ttl).await?,
                Presign::Put => self.signer.signed_url(Method::PUT, &path, ttl).await?,
                Presign::GetForMl => {
                    let signer = self.ml_signer.as_ref().unwrap_or(&self.signer);
                    signer.signed_url(Method::GET, &path, ttl).await?
                }
                Presign::Download(file_name) => {
                    let options = SignedUrlOptions::new().with_query([(
                        "response-content-disposition",
                        format!("attachment; filename=\"{file_name}\""),
                    )]);
                    self.signer
                        .signed_url_opts(Method::GET, &path, ttl, &options)
                        .await?
                }
            };
            Ok(Some(url))
        }
        .boxed()
    }
}
