//! Клиент LRCLIB (https://lrclib.net/docs): `GET /api/get` по артисту, названию, альбому и длительности,
//! `GET /api/search` — запасной путь. Ключ не нужен; просят представляться в User-Agent и не частить,
//! поэтому запросы процесса идут не чаще одного в `gap`, а после 429 — пауза из `Retry-After`.

use std::time::Duration;

use reqwest::StatusCode;
use serde::Deserialize;
use tokio::{sync::Mutex, time::Instant};

const USER_AGENT: &str = "DalyWave (wave.daly.rest)";
/// Без `Retry-After` после 429 ждём столько.
const DEFAULT_BACKOFF: Duration = Duration::from_secs(30);
const MAX_BACKOFF: Duration = Duration::from_secs(10 * 60);
/// Паузу короче этой запрос переждёт сам (и повторит после 429), а не вернёт «подождите».
const SHORT_WAIT: Duration = Duration::from_secs(3);

#[derive(Debug, thiserror::Error)]
pub enum LrclibError {
    #[error("LRCLIB просит подождать {0:?}")]
    RateLimited(Duration),
    #[error("LRCLIB недоступен: {0}")]
    Network(String),
    #[error("LRCLIB ответил HTTP {0}")]
    Status(u16),
    #[error("LRCLIB прислал непонятный ответ: {0}")]
    Decode(String),
}

/// Запись LRCLIB. Длительность — в секундах.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Record {
    pub id: i64,
    #[serde(default)]
    pub track_name: String,
    #[serde(default)]
    pub artist_name: String,
    #[serde(default)]
    pub album_name: Option<String>,
    #[serde(default)]
    pub duration: Option<f64>,
    #[serde(default)]
    pub instrumental: bool,
    #[serde(default)]
    pub plain_lyrics: Option<String>,
    #[serde(default)]
    pub synced_lyrics: Option<String>,
}

impl Record {
    pub fn synced(&self) -> Option<&str> {
        self.synced_lyrics
            .as_deref()
            .filter(|s| !s.trim().is_empty())
    }

    pub fn plain(&self) -> Option<&str> {
        self.plain_lyrics
            .as_deref()
            .filter(|s| !s.trim().is_empty())
    }
}

/// Что ищем.
#[derive(Debug, Clone)]
pub struct Wanted {
    pub title: String,
    /// Основные артисты по порядку; в запросе — первый.
    pub artists: Vec<String>,
    pub album: Option<String>,
    pub duration_ms: i32,
}

pub struct Lrclib {
    http: reqwest::Client,
    base: String,
    gap: Duration,
    /// Раньше этого времени следующий запрос не уходит
    next: Mutex<Instant>,
}

impl Lrclib {
    /// `gap` — наименьший промежуток между запросами процесса.
    pub fn new(base: &str, gap: Duration) -> anyhow::Result<Self> {
        let http = reqwest::Client::builder()
            .user_agent(USER_AGENT)
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(10))
            .build()?;
        Ok(Self {
            http,
            base: base.trim_end_matches('/').to_string(),
            gap,
            next: Mutex::new(Instant::now()),
        })
    }

    /// Ждёт своей очереди; после долгой паузы из 429 сразу отвечает «подождите», не держа запрос.
    async fn slot(&self) -> Result<(), LrclibError> {
        let mut next = self.next.lock().await;
        let now = Instant::now();
        if *next > now + self.gap.max(SHORT_WAIT) {
            return Err(LrclibError::RateLimited(*next - now));
        }
        if *next > now {
            tokio::time::sleep_until(*next).await;
        }
        *next = Instant::now() + self.gap;
        Ok(())
    }

    async fn back_off(&self, wait: Duration) {
        let mut next = self.next.lock().await;
        *next = (*next).max(Instant::now() + wait);
    }

    async fn get_json<T: for<'de> Deserialize<'de>>(
        &self,
        path: &str,
        query: &[(&str, String)],
    ) -> Result<Option<T>, LrclibError> {
        let mut url = url::Url::parse(&format!("{}{path}", self.base))
            .map_err(|e| LrclibError::Network(e.to_string()))?;
        url.query_pairs_mut()
            .extend_pairs(query.iter().map(|(k, v)| (*k, v.as_str())));
        let mut retried = false;
        loop {
            match self.request::<T>(url.clone()).await {
                Err(LrclibError::RateLimited(wait)) if !retried && wait <= SHORT_WAIT => {
                    retried = true;
                }
                other => return other,
            }
        }
    }

    async fn request<T: for<'de> Deserialize<'de>>(
        &self,
        url: url::Url,
    ) -> Result<Option<T>, LrclibError> {
        self.slot().await?;
        let res = self
            .http
            .get(url)
            .send()
            .await
            .map_err(|e| LrclibError::Network(e.without_url().to_string()))?;
        match res.status() {
            StatusCode::NOT_FOUND => Ok(None),
            StatusCode::TOO_MANY_REQUESTS => {
                let wait = res
                    .headers()
                    .get(reqwest::header::RETRY_AFTER)
                    .and_then(|v| v.to_str().ok())
                    .and_then(|v| v.trim().parse::<u64>().ok())
                    .map(Duration::from_secs)
                    .unwrap_or(DEFAULT_BACKOFF)
                    .min(MAX_BACKOFF);
                self.back_off(wait).await;
                Err(LrclibError::RateLimited(wait))
            }
            s if s.is_success() => res
                .json::<T>()
                .await
                .map(Some)
                .map_err(|e| LrclibError::Decode(e.without_url().to_string())),
            s => Err(LrclibError::Status(s.as_u16())),
        }
    }

    /// Точное совпадение: LRCLIB сам сверяет длительность (±2 с).
    pub async fn get(&self, w: &Wanted) -> Result<Option<Record>, LrclibError> {
        let mut query = vec![
            ("track_name", w.title.clone()),
            (
                "artist_name",
                w.artists.first().cloned().unwrap_or_default(),
            ),
            ("duration", ((w.duration_ms + 500) / 1000).to_string()),
        ];
        if let Some(album) = w.album.as_ref().filter(|a| !a.is_empty()) {
            query.push(("album_name", album.clone()));
        }
        self.get_json("/api/get", &query).await
    }

    pub async fn search(&self, w: &Wanted) -> Result<Vec<Record>, LrclibError> {
        let query = [
            ("track_name", w.title.clone()),
            (
                "artist_name",
                w.artists.first().cloned().unwrap_or_default(),
            ),
        ];
        Ok(self
            .get_json::<Vec<Record>>("/api/search", &query)
            .await?
            .unwrap_or_default())
    }
}
