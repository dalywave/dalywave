//! Сведения о музыке из открытых источников без ключей: кто сделал трек (MusicBrainz), об артисте
//! (Wikidata и Википедия), чарт России (открытый чарт Apple Music). Ответы хранятся в базе, сами
//! источники спрашиваются редко и не чаще, чем они просят.

pub mod about;
pub mod apple;
pub mod credits;
pub mod musicbrainz;
pub mod wiki;

use std::time::Duration;

use reqwest::StatusCode;
use serde::de::DeserializeOwned;
use tokio::{sync::Mutex, time::Instant};
use unicode_normalization::UnicodeNormalization;

use crate::config::InfoConfig;

/// Представляемся, как просят MusicBrainz и Wikimedia: название, версия и как связаться.
pub const USER_AGENT: &str = concat!(
    "DalyWave/",
    env!("CARGO_PKG_VERSION"),
    " ( https://wave.daly.rest )"
);
/// Сколько раз повторить запрос, когда источник занят (503, 429).
const ATTEMPTS: u32 = 3;

#[derive(Debug, thiserror::Error)]
pub enum InfoError {
    #[error("источник недоступен: {0}")]
    Network(String),
    #[error("источник занят")]
    Busy,
    #[error("источник ответил HTTP {0}")]
    Status(u16),
    #[error("источник прислал непонятный ответ: {0}")]
    Decode(String),
}

/// HTTP с паузой между запросами процесса и повторами, когда источник занят.
pub struct Paced {
    http: reqwest::Client,
    gap: Duration,
    /// Раньше этого времени следующий запрос не уходит
    next: Mutex<Instant>,
    /// Первая пауза после «занят»; дальше удваивается
    backoff: Duration,
}

impl Paced {
    pub fn new(gap: Duration) -> anyhow::Result<Self> {
        let http = reqwest::Client::builder()
            .user_agent(USER_AGENT)
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(12))
            .build()?;
        Ok(Self {
            http,
            gap,
            next: Mutex::new(Instant::now()),
            backoff: Duration::from_millis(1500),
        })
    }

    /// Короткие паузы — для тестов с заглушкой.
    pub fn with_backoff(mut self, backoff: Duration) -> Self {
        self.backoff = backoff;
        self
    }

    async fn slot(&self) {
        let mut next = self.next.lock().await;
        let now = Instant::now();
        if *next > now {
            tokio::time::sleep_until(*next).await;
        }
        *next = Instant::now() + self.gap;
    }

    /// GET JSON. 404 — `None`.
    pub async fn get<T: DeserializeOwned>(&self, url: url::Url) -> Result<Option<T>, InfoError> {
        let mut attempt = 0;
        loop {
            attempt += 1;
            self.slot().await;
            let res = self
                .http
                .get(url.clone())
                .send()
                .await
                .map_err(|e| InfoError::Network(e.without_url().to_string()));
            let busy = match res {
                Err(e) if attempt >= ATTEMPTS => return Err(e),
                Err(_) => true,
                Ok(res) => match res.status() {
                    StatusCode::NOT_FOUND => return Ok(None),
                    StatusCode::TOO_MANY_REQUESTS | StatusCode::SERVICE_UNAVAILABLE => {
                        if attempt >= ATTEMPTS {
                            return Err(InfoError::Busy);
                        }
                        true
                    }
                    s if s.is_success() => {
                        let bytes = res
                            .bytes()
                            .await
                            .map_err(|e| InfoError::Network(e.without_url().to_string()))?;
                        return serde_json::from_slice(&bytes)
                            .map(Some)
                            .map_err(|e| InfoError::Decode(format!("{}: {e}", url.path())));
                    }
                    s => return Err(InfoError::Status(s.as_u16())),
                },
            };
            if busy {
                tokio::time::sleep(self.backoff * 2u32.pow(attempt - 1)).await;
            }
        }
    }
}

/// Все источники сведений вместе: так их держит состояние API.
pub struct Sources {
    pub musicbrainz: musicbrainz::MusicBrainz,
    pub wiki: wiki::Wiki,
    pub apple: apple::AppleCharts,
}

impl Sources {
    pub fn new(config: &InfoConfig) -> anyhow::Result<Self> {
        Ok(Self {
            musicbrainz: musicbrainz::MusicBrainz::new(&config.musicbrainz_url)?,
            wiki: wiki::Wiki::new(&config.wikidata_url, &config.wikipedia_url)?,
            apple: apple::AppleCharts::new(&config.apple_charts_url)?,
        })
    }

    /// Без пауз между запросами и повторами — для тестов с заглушкой.
    pub fn fast(config: &InfoConfig) -> anyhow::Result<Self> {
        Ok(Self {
            musicbrainz: musicbrainz::MusicBrainz::with_gap(
                &config.musicbrainz_url,
                Duration::ZERO,
                Duration::from_millis(5),
            )?,
            wiki: wiki::Wiki::new(&config.wikidata_url, &config.wikipedia_url)?,
            apple: apple::AppleCharts::new(&config.apple_charts_url)?,
        })
    }
}

/// Без регистра, диакритики и знаков; «ё» = «е». Для сравнения имён артистов.
pub fn loose(s: &str) -> String {
    crate::lyrics::loose(s)
}

/// Русский текст Википедии с ударениями («Би́лли») — без них.
pub fn strip_stress(s: &str) -> String {
    s.nfd()
        .filter(|c| *c != '\u{301}' && *c != '\u{300}')
        .nfc()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stress_marks_are_removed_but_letters_stay() {
        assert_eq!(strip_stress("Би́лли А́йлиш"), "Билли Айлиш");
        // «й» и «ё» — не ударения
        assert_eq!(strip_stress("Йо́шкар-Ола́, ёлка"), "Йошкар-Ола, ёлка");
    }
}
