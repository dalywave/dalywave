//! Клиент публичного API Deezer (ключ не нужен). Один лимит запросов на процесс, повторы при перегрузке
//! и исчерпанной квоте, таймауты. Deezer отвечает ошибкой с HTTP 200: `{"error": {"code": 4, …}}`.

pub mod types;

use std::{collections::VecDeque, time::Duration};

use serde::de::DeserializeOwned;
use serde_json::Value;
use tokio::{sync::Mutex, time::Instant};

use self::types::{Album, ApiError, Artist, Chart, Genre, Page, Track};

/// Окно лимита Deezer.
pub const WINDOW: Duration = Duration::from_secs(5);
const MAX_ATTEMPTS: u32 = 4;
const PAGE_SIZE: u32 = 100;
/// Больше 1000 релизов у артиста — это уже сборники с чужими треками, дальше не читаем
const MAX_PAGES: u32 = 10;
/// Квота исчерпана (`Quota limit exceeded`).
const CODE_QUOTA: i64 = 4;
/// Сервис занят.
const CODE_BUSY: i64 = 700;
/// Нет такого объекта.
const CODE_NO_DATA: i64 = 800;

#[derive(Debug, thiserror::Error)]
pub enum DeezerError {
    #[error("в Deezer этого нет")]
    NotFound,
    #[error("Deezer перегружен: {0}")]
    Busy(String),
    #[error("Deezer ответил ошибкой {code}: {message}")]
    Api { code: i64, message: String },
    #[error("Deezer недоступен: {0}")]
    Network(#[source] reqwest::Error),
    #[error("Deezer прислал непонятный ответ: {0}")]
    Decode(String),
}

impl DeezerError {
    /// Повтор позже может помочь: сеть, перегрузка, квота.
    pub fn is_temporary(&self) -> bool {
        matches!(self, Self::Busy(_) | Self::Network(_))
    }
}

/// Скользящее окно: не больше `max` запросов за [`WINDOW`] на весь процесс.
struct Throttle {
    max: usize,
    sent: Mutex<VecDeque<Instant>>,
}

impl Throttle {
    async fn acquire(&self) {
        loop {
            let wait = {
                let mut sent = self.sent.lock().await;
                let now = Instant::now();
                while sent
                    .front()
                    .is_some_and(|t| now.duration_since(*t) >= WINDOW)
                {
                    sent.pop_front();
                }
                if sent.len() < self.max {
                    sent.push_back(now);
                    return;
                }
                sent.front()
                    .map(|first| WINDOW.saturating_sub(now.duration_since(*first)))
                    .unwrap_or_default()
            };
            tokio::time::sleep(wait.max(Duration::from_millis(5))).await;
        }
    }
}

pub struct Deezer {
    http: reqwest::Client,
    base: String,
    throttle: Throttle,
    /// Пауза после «квота исчерпана»: окно Deezer — 5 секунд
    quota_backoff: Duration,
    /// Первая пауза при перегрузке и сбое сети; дальше удваивается
    retry_backoff: Duration,
}

impl Deezer {
    /// `per_window` — сколько запросов за 5 секунд позволено этому процессу.
    pub fn new(base: &str, per_window: u32) -> anyhow::Result<Self> {
        let http = reqwest::Client::builder()
            .user_agent(concat!("dalywave/", env!("CARGO_PKG_VERSION")))
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(15))
            .build()?;
        Ok(Self {
            http,
            base: base.trim_end_matches('/').to_string(),
            throttle: Throttle {
                max: per_window.max(1) as usize,
                sent: Mutex::new(VecDeque::new()),
            },
            quota_backoff: WINDOW,
            retry_backoff: Duration::from_millis(500),
        })
    }

    /// Короткие паузы между повторами — для тестов с заглушкой.
    pub fn with_backoff(mut self, quota: Duration, retry: Duration) -> Self {
        self.quota_backoff = quota;
        self.retry_backoff = retry;
        self
    }

    /// GET с повторами. `path` — путь с параметрами, например `/artist/1/top?limit=10`.
    async fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T, DeezerError> {
        let url = format!("{}{}", self.base, path);
        let mut attempt = 0;
        loop {
            attempt += 1;
            let backoff = self.retry_backoff * 2u32.pow(attempt - 1);
            self.throttle.acquire().await;
            let started = Instant::now();
            let res = self.http.get(&url).send().await;
            let (retry_after, error) = match res {
                Err(e) if e.is_timeout() || e.is_connect() || e.is_request() => {
                    (backoff, DeezerError::Network(e))
                }
                Err(e) => return Err(DeezerError::Network(e)),
                Ok(res) => {
                    let status = res.status();
                    if status.as_u16() == 429 || status.is_server_error() {
                        (backoff, DeezerError::Busy(format!("HTTP {status}")))
                    } else if status.as_u16() == 404 {
                        return Err(DeezerError::NotFound);
                    } else if !status.is_success() {
                        return Err(DeezerError::Api {
                            code: i64::from(status.as_u16()),
                            message: format!("HTTP {status}"),
                        });
                    } else {
                        let bytes = res.bytes().await.map_err(DeezerError::Network)?;
                        let value: Value = serde_json::from_slice(&bytes)
                            .map_err(|e| DeezerError::Decode(e.to_string()))?;
                        match value.get("error") {
                            None => {
                                tracing::debug!(
                                    path,
                                    ms = started.elapsed().as_millis() as u64,
                                    "deezer"
                                );
                                return serde_json::from_value(value)
                                    .map_err(|e| DeezerError::Decode(format!("{path}: {e}")));
                            }
                            Some(err) => {
                                let err: ApiError = serde_json::from_value(err.clone())
                                    .map_err(|e| DeezerError::Decode(e.to_string()))?;
                                match err.code {
                                    CODE_NO_DATA => return Err(DeezerError::NotFound),
                                    CODE_QUOTA | CODE_BUSY => (
                                        self.quota_backoff.max(backoff),
                                        DeezerError::Busy(err.message),
                                    ),
                                    code => {
                                        return Err(DeezerError::Api {
                                            code,
                                            message: format!("{}: {}", err.kind, err.message),
                                        });
                                    }
                                }
                            }
                        }
                    }
                }
            };
            if attempt >= MAX_ATTEMPTS {
                return Err(error);
            }
            tracing::warn!(path, attempt, error = %error, "deezer request failed, retrying");
            tokio::time::sleep(retry_after).await;
        }
    }

    /// Все страницы списка (`index` растёт, пока Deezer отдаёт `next`).
    async fn all<T: DeserializeOwned>(&self, path: &str) -> Result<Vec<T>, DeezerError> {
        let mut out = Vec::new();
        for page in 0..MAX_PAGES {
            let index = page * PAGE_SIZE;
            let sep = if path.contains('?') { '&' } else { '?' };
            let chunk: Page<T> = self
                .get(&format!("{path}{sep}limit={PAGE_SIZE}&index={index}"))
                .await?;
            let len = chunk.data.len();
            out.extend(chunk.data);
            let total = chunk.total.unwrap_or(0);
            if chunk.next.is_none() || len == 0 || out.len() as i64 >= total {
                break;
            }
        }
        Ok(out)
    }

    pub async fn artist(&self, id: i64) -> Result<Artist, DeezerError> {
        self.get(&format!("/artist/{id}")).await
    }

    /// Популярные треки артиста — с участниками, но без ISRC.
    pub async fn artist_top(&self, id: i64, limit: u32) -> Result<Vec<Track>, DeezerError> {
        let page: Page<Track> = self.get(&format!("/artist/{id}/top?limit={limit}")).await?;
        Ok(page.data)
    }

    /// Дискография: без главного артиста у каждого релиза.
    pub async fn artist_albums(&self, id: i64) -> Result<Vec<Album>, DeezerError> {
        self.all(&format!("/artist/{id}/albums")).await
    }

    pub async fn artist_related(&self, id: i64, limit: u32) -> Result<Vec<Artist>, DeezerError> {
        let page: Page<Artist> = self
            .get(&format!("/artist/{id}/related?limit={limit}"))
            .await?;
        Ok(page.data)
    }

    /// Альбом с жанрами и участниками. Треки — отдельно через [`Self::album_tracks`]: только там ISRC и позиции.
    pub async fn album(&self, id: i64) -> Result<Album, DeezerError> {
        self.get(&format!("/album/{id}")).await
    }

    pub async fn album_tracks(&self, id: i64) -> Result<Vec<Track>, DeezerError> {
        self.all(&format!("/album/{id}/tracks")).await
    }

    /// Трек целиком: ISRC, позиция и все участники.
    pub async fn track(&self, id: i64) -> Result<Track, DeezerError> {
        self.get(&format!("/track/{id}")).await
    }

    /// Трек по ISRC (`track/isrc:…`); у кода может быть несколько релизов — Deezer отдаёт один.
    pub async fn track_by_isrc(&self, isrc: &str) -> Result<Track, DeezerError> {
        let code: String = isrc.chars().filter(char::is_ascii_alphanumeric).collect();
        if code.is_empty() {
            return Err(DeezerError::NotFound);
        }
        self.get(&format!("/track/isrc:{}", code.to_ascii_uppercase()))
            .await
    }

    pub async fn search_tracks(&self, q: &str, limit: u32) -> Result<Vec<Track>, DeezerError> {
        let page: Page<Track> = self.get(&search_path("track", q, limit)).await?;
        Ok(page.data)
    }

    pub async fn search_albums(&self, q: &str, limit: u32) -> Result<Vec<Album>, DeezerError> {
        let page: Page<Album> = self.get(&search_path("album", q, limit)).await?;
        Ok(page.data)
    }

    pub async fn search_artists(&self, q: &str, limit: u32) -> Result<Vec<Artist>, DeezerError> {
        let page: Page<Artist> = self.get(&search_path("artist", q, limit)).await?;
        Ok(page.data)
    }

    /// Общий чарт: треки, альбомы и артисты.
    pub async fn chart(&self, limit: u32) -> Result<Chart, DeezerError> {
        self.get(&format!("/chart/0?limit={limit}")).await
    }

    /// Радио артиста: поток похожих треков (без ISRC).
    pub async fn artist_radio(&self, id: i64, limit: u32) -> Result<Vec<Track>, DeezerError> {
        let page: Page<Track> = self
            .get(&format!("/artist/{id}/radio?limit={limit}"))
            .await?;
        Ok(page.data)
    }

    /// Жанры Deezer с id: по ним — чарты и подборки новых релизов.
    pub async fn genres(&self) -> Result<Vec<Genre>, DeezerError> {
        let page: Page<Genre> = self.get("/genre").await?;
        Ok(page.data)
    }

    /// Чарт жанра (без ISRC).
    pub async fn genre_chart(&self, genre_id: i64, limit: u32) -> Result<Vec<Track>, DeezerError> {
        let page: Page<Track> = self
            .get(&format!("/chart/{genre_id}/tracks?limit={limit}"))
            .await?;
        Ok(page.data)
    }

    /// Новые релизы жанра от редакции Deezer.
    pub async fn editorial_releases(
        &self,
        genre_id: i64,
        limit: u32,
    ) -> Result<Vec<Album>, DeezerError> {
        let page: Page<Album> = self
            .get(&format!("/editorial/{genre_id}/releases?limit={limit}"))
            .await?;
        Ok(page.data)
    }
}

fn search_path(kind: &str, q: &str, limit: u32) -> String {
    let q: String = url::form_urlencoded::byte_serialize(q.as_bytes()).collect();
    format!("/search/{kind}?q={q}&limit={limit}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn throttle_keeps_the_window() {
        tokio::time::pause();
        let t = Throttle {
            max: 3,
            sent: Mutex::new(VecDeque::new()),
        };
        let start = Instant::now();
        for _ in 0..3 {
            t.acquire().await;
        }
        assert!(start.elapsed() < Duration::from_millis(10));
        // Четвёртый ждёт, пока первый выйдет из окна
        t.acquire().await;
        assert!(start.elapsed() >= WINDOW);
    }

    #[test]
    fn search_query_is_encoded() {
        assert_eq!(
            search_path("track", "Lowborn & Mzlff ё", 5),
            "/search/track?q=Lowborn+%26+Mzlff+%D1%91&limit=5"
        );
    }
}
