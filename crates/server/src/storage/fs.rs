//! Хранилище на своём диске: ключ — путь внутри папки `DALYWAVE_STORAGE_PATH`. Запись — во временный файл
//! рядом (`.tmp/`) и переименование, поэтому читатель не увидит половину файла. Тип файла не хранится: он
//! выводится из расширения ключа ([`super::mime_of`]).
//!
//! [`Library`] — папки своей музыки (`DALYWAVE_MUSIC_DIRS`) только на чтение: ключ `library/<абсолютный путь>`
//! принимается, только если путь лежит внутри одной из папок и в нём нет `..`.

use std::{
    io::SeekFrom,
    ops::Range,
    path::{Component, Path, PathBuf},
    time::{Duration, UNIX_EPOCH},
};

use bytes::Bytes;
use futures_util::{FutureExt, StreamExt, future::BoxFuture, stream};
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt};
use url::Url;

use super::{FileStore, Meta, Object, Presign, StorageError, keys, mime_of};
use crate::config::StorageKind;

const CHUNK: usize = 256 * 1024;
const TMP: &str = ".tmp";

/// Проверка ключа: части пути без `.`/`..`, скрытых имён и обратных слэшей.
fn check_key(key: &str) -> Result<(), StorageError> {
    let bad = key.is_empty()
        || key.starts_with('/')
        || key.contains('\\')
        || key.contains('\0')
        || key
            .split('/')
            .any(|part| part.is_empty() || part.starts_with('.'));
    if bad {
        return Err(StorageError::Key(key.to_string()));
    }
    Ok(())
}

/// ETag по размеру и времени изменения: файл заменяется только целиком, переименованием.
fn e_tag(meta: &std::fs::Metadata) -> String {
    let modified = meta
        .modified()
        .ok()
        .and_then(|m| m.duration_since(UNIX_EPOCH).ok())
        .unwrap_or_default();
    format!("\"{:x}-{:x}\"", meta.len(), modified.as_nanos())
}

/// Файл потоком, кусок `range` (уже обрезанный по размеру) или целиком.
async fn open(path: &Path, key: &str, range: Option<Range<u64>>) -> Result<Object, StorageError> {
    let mut file = match tokio::fs::File::open(path).await {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Err(StorageError::NotFound),
        Err(e) => return Err(e.into()),
    };
    let meta = file.metadata().await?;
    if !meta.is_file() {
        return Err(StorageError::NotFound);
    }
    let size = meta.len();
    let range = range.map(|r| r.start.min(size)..r.end.min(size));
    let (start, len) = match &range {
        Some(r) => (r.start, r.end.saturating_sub(r.start)),
        None => (0, size),
    };
    if start > 0 {
        file.seek(SeekFrom::Start(start)).await?;
    }
    let stream = stream::unfold((file, len), |(mut file, left)| async move {
        if left == 0 {
            return None;
        }
        let want = (left as usize).min(CHUNK);
        let mut buf = vec![0u8; want];
        match file.read(&mut buf).await {
            Ok(0) => None,
            Ok(n) => {
                buf.truncate(n);
                Some((Ok(Bytes::from(buf)), (file, left - n as u64)))
            }
            Err(e) => Some((Err(StorageError::Io(e)), (file, 0))),
        }
    })
    .boxed();
    Ok(Object {
        size,
        range,
        content_type: Some(mime_of(key).to_string()),
        e_tag: Some(e_tag(&meta)),
        stream,
    })
}

async fn head(path: &Path) -> Result<Option<Meta>, StorageError> {
    match tokio::fs::metadata(path).await {
        Ok(meta) if meta.is_file() => Ok(Some(Meta {
            size: meta.len(),
            e_tag: Some(e_tag(&meta)),
        })),
        Ok(_) => Ok(None),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.into()),
    }
}

#[derive(Debug)]
pub struct FsStore {
    root: PathBuf,
}

impl FsStore {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    fn path(&self, key: &str) -> Result<PathBuf, StorageError> {
        check_key(key)?;
        Ok(self.root.join(key))
    }

    /// Временный файл внутри хранилища: потом он переименовывается на место.
    fn temp(&self) -> PathBuf {
        self.root
            .join(TMP)
            .join(uuid::Uuid::now_v7().simple().to_string())
    }

    async fn settle(&self, tmp: &Path, path: &Path) -> Result<(), StorageError> {
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        if let Err(e) = tokio::fs::rename(tmp, path).await {
            let _ = tokio::fs::remove_file(tmp).await;
            return Err(e.into());
        }
        Ok(())
    }

    async fn write(&self, key: &str, data: Bytes) -> Result<(), StorageError> {
        let path = self.path(key)?;
        let tmp = self.temp();
        tokio::fs::create_dir_all(self.root.join(TMP)).await?;
        let mut f = tokio::fs::File::create(&tmp).await?;
        let written = async {
            f.write_all(&data).await?;
            f.sync_all().await
        }
        .await;
        drop(f);
        if let Err(e) = written {
            let _ = tokio::fs::remove_file(&tmp).await;
            return Err(e.into());
        }
        self.settle(&tmp, &path).await
    }

    async fn copy_in(&self, key: &str, file: &Path) -> Result<u64, StorageError> {
        let path = self.path(key)?;
        let tmp = self.temp();
        tokio::fs::create_dir_all(self.root.join(TMP)).await?;
        let size = match tokio::fs::copy(file, &tmp).await {
            Ok(n) => n,
            Err(e) => {
                let _ = tokio::fs::remove_file(&tmp).await;
                return Err(e.into());
            }
        };
        self.settle(&tmp, &path).await?;
        Ok(size)
    }

    async fn remove(&self, key: &str) -> Result<(), StorageError> {
        let path = self.path(key)?;
        match tokio::fs::remove_file(&path).await {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(e) => return Err(e.into()),
        }
        // Пустые папки ключа — следом, до корня хранилища
        let mut dir = path.parent();
        while let Some(d) = dir {
            if d == self.root || tokio::fs::remove_dir(d).await.is_err() {
                break;
            }
            dir = d.parent();
        }
        Ok(())
    }

    async fn prepare(&self) -> anyhow::Result<bool> {
        let created = !tokio::fs::try_exists(&self.root).await.unwrap_or(false);
        tokio::fs::create_dir_all(self.root.join(TMP)).await?;
        // Остатки прерванных записей. Свежие не трогаем: API и воркер пишут в одну папку, и запуск
        // одного не должен оборвать запись другого
        if let Ok(mut dir) = tokio::fs::read_dir(self.root.join(TMP)).await {
            while let Ok(Some(entry)) = dir.next_entry().await {
                let stale = entry
                    .metadata()
                    .await
                    .ok()
                    .and_then(|m| m.modified().ok())
                    .and_then(|m| m.elapsed().ok())
                    .is_some_and(|age| age > Duration::from_secs(3600));
                if stale {
                    let _ = tokio::fs::remove_file(entry.path()).await;
                }
            }
        }
        // Папка должна быть доступна на запись: иначе каждая загрузка упадёт позже
        let probe = self.temp();
        tokio::fs::write(&probe, b"").await.map_err(|e| {
            anyhow::anyhow!(
                "storage folder {} is not writable: {e}",
                self.root.display()
            )
        })?;
        let _ = tokio::fs::remove_file(&probe).await;
        if created {
            tracing::info!(path = %self.root.display(), "storage folder created");
        }
        Ok(created)
    }
}

impl FileStore for FsStore {
    fn kind(&self) -> StorageKind {
        StorageKind::Fs
    }

    fn put<'a>(
        &'a self,
        key: &'a str,
        data: Bytes,
        _content_type: &'a str,
        _cache_control: Option<&'a str>,
    ) -> BoxFuture<'a, Result<(), StorageError>> {
        self.write(key, data).boxed()
    }

    fn put_file<'a>(
        &'a self,
        key: &'a str,
        file: &'a Path,
        _content_type: &'a str,
    ) -> BoxFuture<'a, Result<u64, StorageError>> {
        self.copy_in(key, file).boxed()
    }

    fn get<'a>(
        &'a self,
        key: &'a str,
        range: Option<Range<u64>>,
    ) -> BoxFuture<'a, Result<Object, StorageError>> {
        async move { open(&self.path(key)?, key, range).await }.boxed()
    }

    fn head<'a>(&'a self, key: &'a str) -> BoxFuture<'a, Result<Option<Meta>, StorageError>> {
        async move { head(&self.path(key)?).await }.boxed()
    }

    fn delete<'a>(&'a self, key: &'a str) -> BoxFuture<'a, Result<(), StorageError>> {
        self.remove(key).boxed()
    }

    fn ensure(&self) -> BoxFuture<'_, anyhow::Result<bool>> {
        self.prepare().boxed()
    }

    fn presign<'a>(
        &'a self,
        _key: &'a str,
        _how: Presign<'a>,
        _ttl: Duration,
    ) -> BoxFuture<'a, Result<Option<Url>, StorageError>> {
        // Своих ссылок у диска нет: файлы отдаёт API
        async { Ok(None) }.boxed()
    }
}

/// Папки своей музыки, только чтение.
#[derive(Debug, Clone)]
pub struct Library {
    dirs: Vec<PathBuf>,
}

impl Library {
    pub fn new(dirs: Vec<PathBuf>) -> Self {
        Self { dirs }
    }

    pub fn dirs(&self) -> &[PathBuf] {
        &self.dirs
    }

    /// Путь файла по ключу `library/…`, если он внутри одной из папок.
    pub fn path(&self, key: &str) -> Result<PathBuf, StorageError> {
        let rest = key
            .strip_prefix(keys::LIBRARY)
            .ok_or_else(|| StorageError::Key(key.to_string()))?;
        let path = PathBuf::from(format!("/{rest}"));
        let clean = path
            .components()
            .all(|c| matches!(c, Component::RootDir | Component::Normal(_)));
        if !clean || key.contains('\0') {
            return Err(StorageError::Key(key.to_string()));
        }
        if !self.dirs.iter().any(|d| path.starts_with(d) && path != *d) {
            return Err(StorageError::NotFound);
        }
        Ok(path)
    }
}

impl FileStore for Library {
    fn kind(&self) -> StorageKind {
        StorageKind::Fs
    }

    fn put<'a>(
        &'a self,
        _key: &'a str,
        _data: Bytes,
        _content_type: &'a str,
        _cache_control: Option<&'a str>,
    ) -> BoxFuture<'a, Result<(), StorageError>> {
        async { Err(StorageError::ReadOnly) }.boxed()
    }

    fn put_file<'a>(
        &'a self,
        _key: &'a str,
        _file: &'a Path,
        _content_type: &'a str,
    ) -> BoxFuture<'a, Result<u64, StorageError>> {
        async { Err(StorageError::ReadOnly) }.boxed()
    }

    fn get<'a>(
        &'a self,
        key: &'a str,
        range: Option<Range<u64>>,
    ) -> BoxFuture<'a, Result<Object, StorageError>> {
        async move { open(&self.path(key)?, key, range).await }.boxed()
    }

    fn head<'a>(&'a self, key: &'a str) -> BoxFuture<'a, Result<Option<Meta>, StorageError>> {
        async move {
            match self.path(key) {
                Ok(p) => head(&p).await,
                Err(StorageError::NotFound) => Ok(None),
                Err(e) => Err(e),
            }
        }
        .boxed()
    }

    fn delete<'a>(&'a self, _key: &'a str) -> BoxFuture<'a, Result<(), StorageError>> {
        async { Ok(()) }.boxed()
    }

    fn ensure(&self) -> BoxFuture<'_, anyhow::Result<bool>> {
        async { Ok(false) }.boxed()
    }

    fn presign<'a>(
        &'a self,
        _key: &'a str,
        _how: Presign<'a>,
        _ttl: Duration,
    ) -> BoxFuture<'a, Result<Option<Url>, StorageError>> {
        async { Ok(None) }.boxed()
    }
}

#[cfg(test)]
mod tests {
    use futures_util::TryStreamExt;

    use super::*;

    #[test]
    fn keys_stay_inside() {
        assert!(check_key("audio/r/f.flac").is_ok());
        for bad in [
            "",
            "/etc/passwd",
            "audio/../x",
            "a//b",
            ".tmp/x",
            "a/.hidden",
            "a\\b",
        ] {
            assert!(check_key(bad).is_err(), "{bad}");
        }
        let lib = Library::new(vec![PathBuf::from("/music"), PathBuf::from("/mnt/Музыка")]);
        assert_eq!(
            lib.path("library/music/Кино/a.flac").unwrap(),
            PathBuf::from("/music/Кино/a.flac")
        );
        assert!(lib.path("library/mnt/Музыка/b.mp3").is_ok());
        assert!(matches!(
            lib.path("library/etc/passwd"),
            Err(StorageError::NotFound)
        ));
        assert!(lib.path("library/music/../etc/passwd").is_err());
        assert!(lib.path("library/musicx/a.mp3").is_err());
        assert!(lib.path("library/music").is_err());
        assert!(lib.path("audio/a.mp3").is_err());
    }

    #[tokio::test]
    async fn disk_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let s = FsStore::new(dir.path().join("store"));
        assert!(s.prepare().await.unwrap());
        assert!(!s.prepare().await.unwrap());
        s.write("images/a/250.jpg", Bytes::from_static(b"abcdef"))
            .await
            .unwrap();
        let obj = open(
            &s.path("images/a/250.jpg").unwrap(),
            "images/a/250.jpg",
            None,
        )
        .await
        .unwrap();
        assert_eq!(obj.size, 6);
        assert_eq!(obj.content_type.as_deref(), Some("image/jpeg"));
        let all: Vec<Bytes> = obj.stream.try_collect().await.unwrap();
        assert_eq!(all.concat(), b"abcdef");
        let part = s.get("images/a/250.jpg", Some(2..100)).await.unwrap();
        assert_eq!(part.range, Some(2..6));
        let bytes: Vec<Bytes> = part.stream.try_collect().await.unwrap();
        assert_eq!(bytes.concat(), b"cdef");
        assert_eq!(s.head("images/a/250.jpg").await.unwrap().unwrap().size, 6);
        let src = dir.path().join("in.flac");
        tokio::fs::write(&src, vec![7u8; 300_000]).await.unwrap();
        assert_eq!(s.copy_in("audio/r/f.flac", &src).await.unwrap(), 300_000);
        assert!(src.exists(), "исходный файл остаётся на месте");
        let read: Vec<Bytes> = s
            .get("audio/r/f.flac", None)
            .await
            .unwrap()
            .stream
            .try_collect()
            .await
            .unwrap();
        assert_eq!(read.concat().len(), 300_000);
        s.remove("audio/r/f.flac").await.unwrap();
        s.remove("audio/r/f.flac").await.unwrap();
        assert!(
            !dir.path().join("store/audio").exists(),
            "пустые папки убраны"
        );
        assert!(matches!(
            s.get("audio/r/f.flac", None).await,
            Err(StorageError::NotFound)
        ));
        assert!(s.head("audio/r/f.flac").await.unwrap().is_none());
        assert!(s.get("../in.flac", None).await.is_err());
    }
}
