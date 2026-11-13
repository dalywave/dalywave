//! Хранилище файлов. Два бэкенда за одним трейтом [`FileStore`]: S3-совместимое ([`object`]: Cloudflare R2
//! в проде, SeaweedFS локально, память процесса в тестах) и своя папка на диске ([`fs`]). Код сервера видит
//! только [`Storage`].
//!
//! Ключи: `audio/{recordingId}/{fileId}.{ext}`, `images/albums/{id}/{size}.jpg`, `images/artists/{id}/{size}.jpg`.
//! Файлы своей музыки из папок на сервере — ключи `library/<абсолютный путь>`: их сервер только читает
//! ([`fs::Library`]), запись и удаление по ним ничего не делают.
//!
//! Аудио клиенты качают по подписанной ссылке: у S3 — ссылка самого хранилища (адрес `S3_PUBLIC_ENDPOINT`,
//! если задан), у диска и своей музыки — короткая ссылка на этот же API ([`links`], `/api/files/…`).

pub mod fs;
pub mod links;
pub mod object;

use std::{
    fmt,
    ops::Range,
    path::{Path as FsPath, PathBuf},
    sync::Arc,
    time::Duration,
};

use bytes::Bytes;
use dalywave_protocol::jobs::ImageOwner;
use futures_util::{StreamExt, future::BoxFuture, stream::BoxStream};
use url::Url;
use uuid::Uuid;

use crate::config::{Config, S3Config, StorageKind};

pub use links::{Links, Signed};

/// Картинки по адресу не меняются: смена обложки меняет `?v=` в ссылке.
pub const IMMUTABLE: &str = "public, max-age=31536000, immutable";
/// Ссылка на аудио живёт несколько часов: столько может длиться пауза плеера.
pub const AUDIO_GET_TTL: Duration = Duration::from_secs(6 * 3600);
/// Загрузчику хватает получаса на скачивание и загрузку одного трека.
pub const UPLOAD_PUT_TTL: Duration = Duration::from_secs(30 * 60);
/// Сервис анализа качает файл сразу, но может ждать своей очереди (анализ один за раз).
pub const ML_GET_TTL: Duration = Duration::from_secs(60 * 60);

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

    /// Своя обложка плейлиста: у каждой загрузки своя папка, поэтому ссылка `?v=` меняется сама.
    pub fn playlist_cover(playlist_id: Uuid, upload_id: Uuid) -> String {
        format!("images/playlists/{playlist_id}/{upload_id}")
    }

    pub fn image(prefix: &str, size: u32) -> String {
        format!("{prefix}/{size}.jpg")
    }

    /// Префикс ключей своей музыки.
    pub const LIBRARY: &str = "library/";

    /// Ключ файла своей музыки по его абсолютному пути.
    pub fn library(path: &std::path::Path) -> Option<String> {
        let rest = path.to_str()?.strip_prefix('/')?;
        (!rest.is_empty()).then(|| format!("{LIBRARY}{rest}"))
    }

    pub fn is_library(key: &str) -> bool {
        key.starts_with(LIBRARY)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("нет такого файла")]
    NotFound,
    #[error(transparent)]
    Store(#[from] object_store::Error),
    #[error("неверный ключ: {0}")]
    Key(String),
    #[error("файл на диске: {0}")]
    Io(#[from] std::io::Error),
    #[error("этот файл только для чтения")]
    ReadOnly,
}

impl From<object_store::path::Error> for StorageError {
    fn from(e: object_store::path::Error) -> Self {
        Self::Key(e.to_string())
    }
}

/// Файл, который читается потоком (целиком или кусок `range`).
pub struct Object {
    /// Размер всего файла.
    pub size: u64,
    /// Какой кусок в потоке; `None` — весь файл.
    pub range: Option<Range<u64>>,
    pub content_type: Option<String>,
    pub e_tag: Option<String>,
    pub stream: BoxStream<'static, Result<Bytes, StorageError>>,
}

pub struct Meta {
    pub size: u64,
    pub e_tag: Option<String>,
}

/// Что подписать ссылкой самого хранилища.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Presign<'a> {
    Get,
    /// Скачать под этим именем (`Content-Disposition: attachment`).
    Download(&'a str),
    Put,
    /// Для сервиса анализа: по его адресу хранилища.
    GetForMl,
}

/// Бэкенд хранилища. Методы возвращают `BoxFuture`, чтобы трейт годился для `dyn`.
pub trait FileStore: Send + Sync + fmt::Debug {
    fn kind(&self) -> StorageKind;

    fn put<'a>(
        &'a self,
        key: &'a str,
        data: Bytes,
        content_type: &'a str,
        cache_control: Option<&'a str>,
    ) -> BoxFuture<'a, Result<(), StorageError>>;

    /// Загрузка с диска без чтения целиком в память; возвращает размер.
    fn put_file<'a>(
        &'a self,
        key: &'a str,
        file: &'a FsPath,
        content_type: &'a str,
    ) -> BoxFuture<'a, Result<u64, StorageError>>;

    /// Файл потоком (кусок `range`, если задан); `NotFound`, если его нет.
    fn get<'a>(
        &'a self,
        key: &'a str,
        range: Option<Range<u64>>,
    ) -> BoxFuture<'a, Result<Object, StorageError>>;

    fn head<'a>(&'a self, key: &'a str) -> BoxFuture<'a, Result<Option<Meta>, StorageError>>;

    /// Удаление; отсутствующий файл — не ошибка.
    fn delete<'a>(&'a self, key: &'a str) -> BoxFuture<'a, Result<(), StorageError>>;

    /// Подготовка при запуске (бакет, папка); `true` — что-то создано.
    fn ensure(&self) -> BoxFuture<'_, anyhow::Result<bool>>;

    /// Ссылка самого хранилища. `None` — своих ссылок у хранилища нет, файлы отдаёт API ([`links`]).
    fn presign<'a>(
        &'a self,
        key: &'a str,
        how: Presign<'a>,
        ttl: Duration,
    ) -> BoxFuture<'a, Result<Option<Url>, StorageError>>;
}

pub struct Storage {
    backend: Arc<dyn FileStore>,
    /// Своя музыка из папок на сервере (только чтение)
    library: Option<fs::Library>,
    /// Ссылки на файлы через API: для диска и своей музыки
    links: Links,
}

impl fmt::Debug for Storage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Storage")
            .field("backend", &self.backend)
            .field("library", &self.library)
            .finish()
    }
}

impl Storage {
    /// Хранилище по настройкам: S3, диск или память; своя музыка — из `DALYWAVE_MUSIC_DIRS`.
    pub fn new(config: &Config) -> anyhow::Result<Self> {
        let backend: Arc<dyn FileStore> = match (config.storage.kind, &config.s3) {
            (StorageKind::S3, Some(s3)) => Arc::new(object::ObjectStoreBackend::s3(s3)?),
            (StorageKind::S3, None) => anyhow::bail!("S3 storage needs DALYWAVE_S3_* settings"),
            (StorageKind::Fs, _) => Arc::new(fs::FsStore::new(config.storage.path.clone())),
            (StorageKind::Memory, _) => Arc::new(object::ObjectStoreBackend::memory()),
        };
        let links = Links::new(
            &config.secret_key,
            &config.public_url,
            config.storage.ml_url.as_deref(),
        )?;
        Ok(Self {
            backend,
            library: (!config.library.dirs.is_empty())
                .then(|| fs::Library::new(config.library.dirs.clone())),
            links,
        })
    }

    pub fn from_config(cfg: &S3Config) -> anyhow::Result<Self> {
        Ok(Self::with_backend(
            Arc::new(object::ObjectStoreBackend::s3(cfg)?),
            Links::local(),
        ))
    }

    /// Хранилище в памяти процесса — для тестов и разработки без S3. Ссылки подписываются как для
    /// `http://storage.invalid`, по ним ничего не скачать.
    pub fn memory() -> Self {
        Self::with_backend(
            Arc::new(object::ObjectStoreBackend::memory()),
            Links::local(),
        )
    }

    /// Своя папка на диске; ссылки — на API по адресу из `links`.
    pub fn fs(root: impl Into<PathBuf>, links: Links) -> Self {
        Self::with_backend(Arc::new(fs::FsStore::new(root.into())), links)
    }

    pub fn with_backend(backend: Arc<dyn FileStore>, links: Links) -> Self {
        Self {
            backend,
            library: None,
            links,
        }
    }

    /// Подключить папки своей музыки (ключи `library/…`).
    pub fn with_library(mut self, dirs: Vec<PathBuf>) -> Self {
        self.library = (!dirs.is_empty()).then(|| fs::Library::new(dirs));
        self
    }

    pub fn kind(&self) -> StorageKind {
        self.backend.kind()
    }

    pub fn is_memory(&self) -> bool {
        self.kind() == StorageKind::Memory
    }

    /// Папки своей музыки.
    pub fn music_dirs(&self) -> &[PathBuf] {
        self.library.as_ref().map(|l| l.dirs()).unwrap_or_default()
    }

    /// Путь на диске к файлу своей музыки по ключу `library/…` (если он внутри папок музыки).
    pub fn library_path(&self, key: &str) -> Option<PathBuf> {
        self.library.as_ref()?.path(key).ok()
    }

    pub fn links(&self) -> &Links {
        &self.links
    }

    /// Создаёт бакет или папку, если их нет. `true` — что-то создано.
    pub async fn ensure_bucket(&self) -> anyhow::Result<bool> {
        self.backend.ensure().await
    }

    fn reader(&self, key: &str) -> Result<&dyn FileStore, StorageError> {
        if keys::is_library(key) {
            return match &self.library {
                Some(l) => Ok(l),
                None => Err(StorageError::NotFound),
            };
        }
        Ok(&*self.backend)
    }

    pub async fn put(
        &self,
        key: &str,
        data: impl Into<Bytes>,
        content_type: &str,
        cache_control: Option<&str>,
    ) -> Result<(), StorageError> {
        if keys::is_library(key) {
            return Err(StorageError::ReadOnly);
        }
        self.backend
            .put(key, data.into(), content_type, cache_control)
            .await
    }

    /// Загрузка с диска частями (большие файлы — импорт, свои аудиофайлы) без чтения целиком в память.
    pub async fn put_file(
        &self,
        key: &str,
        file: &FsPath,
        content_type: &str,
    ) -> Result<u64, StorageError> {
        if keys::is_library(key) {
            return Err(StorageError::ReadOnly);
        }
        self.backend.put_file(key, file, content_type).await
    }

    /// Файл из хранилища на диск; возвращает размер.
    pub async fn download_to(&self, key: &str, file: &FsPath) -> Result<u64, StorageError> {
        use tokio::io::AsyncWriteExt;
        let mut obj = self.get(key).await?;
        let mut f = tokio::fs::File::create(file).await?;
        let mut total = 0u64;
        while let Some(chunk) = obj.stream.next().await {
            let chunk = chunk?;
            total += chunk.len() as u64;
            f.write_all(&chunk).await?;
        }
        f.flush().await?;
        Ok(total)
    }

    /// Файл потоком; `NotFound`, если его нет.
    pub async fn get(&self, key: &str) -> Result<Object, StorageError> {
        self.reader(key)?.get(key, None).await
    }

    /// Кусок файла потоком (запрос с Range).
    pub async fn get_range(
        &self,
        key: &str,
        range: Option<Range<u64>>,
    ) -> Result<Object, StorageError> {
        self.reader(key)?.get(key, range).await
    }

    pub async fn get_bytes(&self, key: &str) -> Result<Bytes, StorageError> {
        let mut obj = self.get(key).await?;
        let mut out = Vec::with_capacity(obj.size as usize);
        while let Some(chunk) = obj.stream.next().await {
            out.extend_from_slice(&chunk?);
        }
        Ok(out.into())
    }

    pub async fn head(&self, key: &str) -> Result<Option<Meta>, StorageError> {
        match self.reader(key) {
            Ok(r) => r.head(key).await,
            Err(StorageError::NotFound) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Удаление; отсутствующий файл — не ошибка. Файлы своей музыки не удаляются никогда.
    pub async fn delete(&self, key: &str) -> Result<(), StorageError> {
        if keys::is_library(key) {
            tracing::debug!(%key, "library files are read-only: not deleting");
            return Ok(());
        }
        self.backend.delete(key).await
    }

    async fn sign(&self, key: &str, how: Presign<'_>, ttl: Duration) -> Result<Url, StorageError> {
        if !keys::is_library(key)
            && let Some(url) = self.backend.presign(key, how, ttl).await?
        {
            return Ok(url);
        }
        if keys::is_library(key) && matches!(how, Presign::Put) {
            return Err(StorageError::ReadOnly);
        }
        self.links.sign(key, how, ttl)
    }

    /// Подписанная ссылка на скачивание (аудио — на [`AUDIO_GET_TTL`]).
    pub async fn presign_get(&self, key: &str, ttl: Duration) -> Result<Url, StorageError> {
        self.sign(key, Presign::Get, ttl).await
    }

    /// Подписанная ссылка на скачивание файла под именем `file_name`: хранилище отдаст его с
    /// `Content-Disposition: attachment` (параметр входит в подпись, подменить имя нельзя).
    pub async fn presign_download(
        &self,
        key: &str,
        ttl: Duration,
        file_name: &str,
    ) -> Result<Url, StorageError> {
        self.sign(key, Presign::Download(file_name), ttl).await
    }

    /// Подписанная ссылка на скачивание для сервиса анализа: по его адресу хранилища, если он задан.
    pub async fn presign_get_for_ml(&self, key: &str, ttl: Duration) -> Result<Url, StorageError> {
        self.sign(key, Presign::GetForMl, ttl).await
    }

    /// Подписанная ссылка на загрузку (загрузчику — на [`UPLOAD_PUT_TTL`]). Content-Type в подпись не входит.
    pub async fn presign_put(&self, key: &str, ttl: Duration) -> Result<Url, StorageError> {
        self.sign(key, Presign::Put, ttl).await
    }
}

/// MIME по расширению ключа — для диска и своей музыки, где тип не хранится рядом с файлом.
pub fn mime_of(key: &str) -> &'static str {
    let ext = key
        .rsplit_once('.')
        .map(|(_, e)| e.to_ascii_lowercase())
        .unwrap_or_default();
    match ext.as_str() {
        "jpg" | "jpeg" => "image/jpeg",
        "png" => "image/png",
        "webp" => "image/webp",
        "m4a" | "alac" => "audio/mp4",
        "mp4" | "m4v" => "video/mp4",
        "mp3" => "audio/mpeg",
        "flac" => "audio/flac",
        "ogg" | "oga" | "opus" => "audio/ogg",
        "webm" => "audio/webm",
        "wav" => "audio/wav",
        "aif" | "aiff" => "audio/aiff",
        "json" => "application/json",
        "dmg" => "application/x-apple-diskimage",
        "zip" => "application/zip",
        _ => "application/octet-stream",
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
        assert_eq!(
            keys::library(std::path::Path::new("/music/Кино/01 Группа крови.flac")).as_deref(),
            Some("library/music/Кино/01 Группа крови.flac")
        );
        assert!(keys::library(std::path::Path::new("music/a.mp3")).is_none());
        assert!(keys::is_library("library/music/a.mp3"));
        assert!(!keys::is_library("audio/x.ogg"));
        assert_eq!(mime_of("library/music/a.FLAC"), "audio/flac");
        assert_eq!(mime_of("renditions/x/opus64.m4a"), "audio/mp4");
        assert_eq!(mime_of("imports/x/1"), "application/octet-stream");
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
        let part = s.get_range("images/a/250.jpg", Some(1..3)).await.unwrap();
        assert_eq!(part.range, Some(1..3));
        let chunks: Vec<Bytes> = part.stream.try_collect().await.unwrap();
        assert_eq!(chunks.concat(), vec![2, 3]);
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
        // Своей музыки без папок нет; удалять её нельзя и не нужно
        assert!(s.head("library/music/a.mp3").await.unwrap().is_none());
        assert!(matches!(
            s.put("library/music/a.mp3", vec![1u8], "audio/mpeg", None)
                .await,
            Err(StorageError::ReadOnly)
        ));
        s.delete("library/music/a.mp3").await.unwrap();
    }

    #[tokio::test]
    async fn public_endpoint_signs_links() {
        let cfg = S3Config {
            endpoint: "http://seaweedfs:8333".into(),
            public_endpoint: Some("https://files.example.com".into()),
            ml_endpoint: None,
            region: "auto".into(),
            bucket: "dalywave".into(),
            access_key_id: "k".into(),
            secret_access_key: "s".into(),
        };
        let s = Storage::from_config(&cfg).unwrap();
        let url = s.presign_get("audio/x.ogg", AUDIO_GET_TTL).await.unwrap();
        assert_eq!(url.host_str(), Some("files.example.com"));
        assert_eq!(url.path(), "/dalywave/audio/x.ogg");
        let dl = s
            .presign_download("releases/x.dmg", AUDIO_GET_TTL, "DalyWave-0.2.0.dmg")
            .await
            .unwrap();
        assert_eq!(dl.host_str(), Some("files.example.com"));
        assert!(
            dl.as_str().contains(
                "response-content-disposition=attachment%3B%20filename%3D%22DalyWave-0.2.0.dmg%22"
            ),
            "{dl}"
        );
        // Без своего адреса сервис анализа получает ту же ссылку, что и клиенты
        let ml = s
            .presign_get_for_ml("audio/x.ogg", ML_GET_TTL)
            .await
            .unwrap();
        assert_eq!(ml.host_str(), Some("files.example.com"));
        let s = Storage::from_config(&S3Config {
            ml_endpoint: Some("http://host.docker.internal:8401".into()),
            ..cfg
        })
        .unwrap();
        let ml = s
            .presign_get_for_ml("audio/x.ogg", ML_GET_TTL)
            .await
            .unwrap();
        assert_eq!(ml.host_str(), Some("host.docker.internal"));
        assert_eq!(ml.port(), Some(8401));
        assert!(ml.as_str().contains("X-Amz-Expires=3600"), "{ml}");
    }
}
