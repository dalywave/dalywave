//! Один файл: докачка с места обрыва, проверка размера и хеша, переименование готового.

use std::{
    fs::{self, File, OpenOptions},
    io::{self, Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};

use sha2::{Digest, Sha256};

/// Что качать.
pub struct Request<'a> {
    pub url: &'a str,
    /// Итоговый файл; рядом — `<имя>.part`.
    pub dest: &'a Path,
    /// Размер со слов сервера: по нему проверяется скачанное.
    pub size: u64,
    /// SHA-256 в hex, если сервер его знает.
    pub sha256: Option<&'a str>,
    pub user_agent: &'a str,
}

#[derive(Debug, thiserror::Error)]
pub enum FetchError {
    /// Сеть оборвалась: недокачанное остаётся, следующая попытка продолжит с места обрыва.
    #[error("сеть: {0}")]
    Network(String),
    /// Ссылка истекла или её не приняли (400–404, 410): нужна свежая.
    #[error("ссылка не действует: HTTP {0}")]
    Expired(u16),
    #[error("HTTP {0}")]
    Http(u16),
    /// Скачанное не совпало с сервером по размеру или хешу — удалено, качать заново.
    #[error("файл не прошёл проверку: {0}")]
    Corrupt(String),
    #[error("загрузка остановлена")]
    Cancelled,
    #[error("диск: {0}")]
    Io(#[from] io::Error),
}

impl FetchError {
    /// Повтор с той же ссылкой может помочь.
    pub fn retryable(&self) -> bool {
        matches!(
            self,
            Self::Network(_) | Self::Http(500..=599) | Self::Corrupt(_)
        )
    }
}

pub fn part_path(dest: &Path) -> PathBuf {
    let mut name = dest
        .file_name()
        .map(|n| n.to_os_string())
        .unwrap_or_default();
    name.push(".part");
    dest.with_file_name(name)
}

fn agent(user_agent: &str) -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_connect(Some(Duration::from_secs(15)))
        .timeout_recv_response(Some(Duration::from_secs(20)))
        .timeout_recv_body(Some(Duration::from_secs(30)))
        .http_status_as_error(false)
        .user_agent(user_agent)
        .build()
        .into()
}

/// Проверить готовый файл: размер и, если известен, SHA-256.
pub fn verify(path: &Path, size: u64, sha256: Option<&str>) -> Result<(), FetchError> {
    let len = fs::metadata(path)?.len();
    if len != size {
        return Err(FetchError::Corrupt(format!("размер {len} вместо {size}")));
    }
    if let Some(want) = sha256.filter(|s| !s.is_empty()) {
        let mut f = File::open(path)?;
        let mut h = Sha256::new();
        let mut buf = vec![0u8; 256 * 1024];
        loop {
            let n = f.read(&mut buf)?;
            if n == 0 {
                break;
            }
            h.update(&buf[..n]);
        }
        let got = hex::encode(h.finalize());
        if !got.eq_ignore_ascii_case(want) {
            return Err(FetchError::Corrupt("хеш не совпал".into()));
        }
    }
    Ok(())
}

/// Скачать файл. `progress(получено, всего)`; `cancel` останавливает между кусками, оставляя `.part`.
pub fn fetch(
    req: &Request,
    cancel: &AtomicBool,
    mut progress: impl FnMut(u64, u64),
) -> Result<u64, FetchError> {
    if req.size == 0 {
        return Err(FetchError::Corrupt("сервер не сказал размер файла".into()));
    }
    if let Some(dir) = req.dest.parent() {
        fs::create_dir_all(dir)?;
    }
    let part = part_path(req.dest);
    let mut have = fs::metadata(&part).map(|m| m.len()).unwrap_or(0);
    if have > req.size {
        fs::remove_file(&part)?;
        have = 0;
    }
    if have < req.size {
        download(req, &part, have, cancel, &mut progress)?;
    }
    if let Err(e) = verify(&part, req.size, req.sha256) {
        let _ = fs::remove_file(&part);
        return Err(e);
    }
    fs::rename(&part, req.dest)?;
    progress(req.size, req.size);
    Ok(req.size)
}

fn download(
    req: &Request,
    part: &Path,
    have: u64,
    cancel: &AtomicBool,
    progress: &mut impl FnMut(u64, u64),
) -> Result<(), FetchError> {
    let mut call = agent(req.user_agent).get(req.url);
    if have > 0 {
        call = call.header("Range", format!("bytes={have}-"));
    }
    let res = call
        .call()
        .map_err(|e| FetchError::Network(e.to_string()))?;
    let status = res.status().as_u16();
    let (mut file, mut got) = match status {
        206 => (OpenOptions::new().append(true).open(part)?, have),
        // Сервер отдал файл целиком (Range не понял): начать заново
        200 => (File::create(part)?, 0),
        // Запрошено за концом: всё уже есть — проверка решит
        416 => return Ok(()),
        400 | 401 | 403 | 404 | 410 => return Err(FetchError::Expired(status)),
        other => return Err(FetchError::Http(other)),
    };
    let mut body = res.into_body().into_reader();
    let mut buf = vec![0u8; 64 * 1024];
    progress(got, req.size);
    loop {
        if cancel.load(Ordering::Relaxed) {
            file.flush()?;
            return Err(FetchError::Cancelled);
        }
        let n = match body.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => n,
            Err(e) => {
                file.flush()?;
                return Err(FetchError::Network(e.to_string()));
            }
        };
        file.write_all(&buf[..n])?;
        got += n as u64;
        progress(got, req.size);
        if got > req.size {
            break;
        }
    }
    file.flush()?;
    file.sync_all()?;
    if got < req.size {
        return Err(FetchError::Network(format!(
            "соединение закрылось на {got} из {} байт",
            req.size
        )));
    }
    Ok(())
}
