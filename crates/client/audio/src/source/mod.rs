//! Источники байтов: HTTP(S) с Range и опережающей загрузкой, локальные файлы.

mod http;

use std::collections::HashMap;
use std::fs::File;
use std::sync::{Arc, Mutex};

use symphonia::core::io::MediaSource;

pub use http::{HttpCache, HttpStats};

use crate::{AudioError, Resolver, SourceSpec, TrackSpec};

/// Кто обновит ссылку, когда она перестанет действовать: [`Resolver`] вызывающего и трек,
/// по `stream_id` которого он найдёт файл.
#[derive(Clone, Debug)]
pub(crate) struct Refresh {
    pub resolver: Resolver,
    pub track: TrackSpec,
}

/// Кэш открытых HTTP-источников: следующий трек начинает качаться заранее, а перемотка и
/// повторное открытие (seek, next/previous) берут уже скачанные байты.
#[derive(Clone, Default)]
pub struct SourceCache {
    inner: Arc<Mutex<HashMap<String, HttpCache>>>,
    user_agent: String,
    resolver: Option<Resolver>,
}

impl SourceCache {
    pub fn new(user_agent: String) -> Self {
        Self {
            inner: Default::default(),
            user_agent,
            resolver: None,
        }
    }

    pub fn with_resolver(user_agent: String, resolver: Option<Resolver>) -> Self {
        Self {
            resolver,
            ..Self::new(user_agent)
        }
    }

    /// Начинает загрузку заранее (не блокирует).
    pub fn prefetch(&self, track: &TrackSpec) {
        if let SourceSpec::Url(url) = &track.source {
            let _ = self.http(url, Some(track));
        }
    }

    fn http(&self, url: &str, track: Option<&TrackSpec>) -> HttpCache {
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
        let refresh = match (&self.resolver, track) {
            (Some(resolver), Some(track)) if track.stream_id.is_some() => Some(Refresh {
                resolver: resolver.clone(),
                track: track.clone(),
            }),
            _ => None,
        };
        let c = HttpCache::start(url.to_string(), self.user_agent.clone(), with_tail, refresh);
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

    /// Открыть источник без обновления ссылок (проверки, эталоны).
    pub fn open(
        &self,
        spec: &SourceSpec,
    ) -> Result<(Box<dyn MediaSource>, Option<String>), AudioError> {
        self.open_inner(spec, None)
    }

    /// Открыть трек очереди: истёкшую ссылку обновит [`Resolver`] по `stream_id`.
    pub fn open_track(
        &self,
        track: &TrackSpec,
    ) -> Result<(Box<dyn MediaSource>, Option<String>), AudioError> {
        self.open_inner(&track.source, Some(track))
    }

    fn open_inner(
        &self,
        spec: &SourceSpec,
        track: Option<&TrackSpec>,
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
                let cache = self.http(url, track);
                let reader = cache.reader()?;
                Ok((Box::new(reader), extension_of(url)))
            }
            SourceSpec::Deferred => Err(AudioError::Network("нет ссылки на файл".into())),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extension_ignores_query() {
        assert_eq!(
            extension_of(
                "https://x.r2.cloudflarestorage.com/audio/r/f.mp4?X-Amz-Expires=21600&a=b.c"
            )
            .as_deref(),
            Some("mp4")
        );
        assert_eq!(extension_of("http://h/a/b").as_deref(), None);
    }
}
