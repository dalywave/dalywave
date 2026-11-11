//! Источники байтов: HTTP(S) с Range и опережающей загрузкой, локальные файлы.

mod http;

use std::collections::HashMap;
use std::fs::File;
use std::sync::{Arc, Mutex};

use symphonia::core::io::MediaSource;

pub use http::{HttpCache, HttpStats};

use crate::{AudioError, SourceSpec};

/// Кэш открытых HTTP-источников: следующий трек начинает качаться заранее, а перемотка и
/// повторное открытие (seek, next/previous) берут уже скачанные байты.
#[derive(Clone, Default)]
pub struct SourceCache {
    inner: Arc<Mutex<HashMap<String, HttpCache>>>,
    user_agent: String,
}

impl SourceCache {
    pub fn new(user_agent: String) -> Self {
        Self {
            inner: Default::default(),
            user_agent,
        }
    }

    /// Начинает загрузку заранее (не блокирует).
    pub fn prefetch(&self, spec: &SourceSpec) {
        if let SourceSpec::Url(url) = spec {
            let _ = self.http(url);
        }
    }

    fn http(&self, url: &str) -> HttpCache {
        let mut map = self.inner.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(c) = map.get(url)
            && !c.failed()
        {
            return c.clone();
        }
        let with_tail = !matches!(
            extension_of(url).as_deref(),
            Some("mp4" | "m4a" | "flac" | "mp3" | "webm" | "wav")
        );
        let c = HttpCache::start(url.to_string(), self.user_agent.clone(), with_tail);
        map.insert(url.to_string(), c.clone());
        c
    }

    /// Оставляет в кэше только перечисленные адреса (память под аудио не копится).
    pub fn retain(&self, keep: &[&str]) {
        let mut map = self.inner.lock().unwrap_or_else(|p| p.into_inner());
        map.retain(|k, c| {
            let keep_it = keep.contains(&k.as_str());
            if !keep_it {
                c.stop();
            }
            keep_it
        });
    }

    pub fn open(
        &self,
        spec: &SourceSpec,
    ) -> Result<(Box<dyn MediaSource>, Option<String>), AudioError> {
        match spec {
            SourceSpec::File(path) => {
                let f = File::open(path)
                    .map_err(|e| AudioError::Io(format!("{}: {e}", path.display())))?;
                let ext = path
                    .extension()
                    .and_then(|e| e.to_str())
                    .map(str::to_lowercase);
                Ok((Box::new(f), ext))
            }
            SourceSpec::Url(url) => {
                let cache = self.http(url);
                let reader = cache.reader()?;
                Ok((Box::new(reader), extension_of(url)))
            }
        }
    }

    pub fn stats(&self, url: &str) -> Option<HttpStats> {
        let map = self.inner.lock().unwrap_or_else(|p| p.into_inner());
        map.get(url).map(|c| c.stats())
    }
}

fn extension_of(url: &str) -> Option<String> {
    let path = url.split(['?', '#']).next()?;
    let name = path.rsplit('/').next()?;
    let (_, ext) = name.rsplit_once('.')?;
    Some(ext.to_lowercase())
}
