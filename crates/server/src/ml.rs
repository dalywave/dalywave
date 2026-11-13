//! Клиент сервиса анализа звука (services/ml): разбор аудио по ссылке и вектор текстового запроса.
//!
//! Анализ на сервисе строго один за раз: занятый отвечает 409 `busy`, и клиент ждёт с растущей паузой.
//! Внутри процесса анализы тоже идут по одному (семафор): задачи полос «Анализ звука» и «Разведка
//! каталога» не толкаются за сервис. Текстовые запросы у сервиса под своим замком и анализа не ждут.
//! Векторы приходят в base64 от float16 little-endian — ровно то, что хранит `halfvec`.

use std::time::Duration;

use serde::{Deserialize, de::DeserializeOwned};
use serde_json::{Value, json};
use tokio::{sync::Semaphore, time::Instant};

use crate::reco::vectors::{SEMANTIC_DIM, SOUND_DIM, decode_f16_base64};

/// Самая длинная пауза между вопросами к занятому сервису.
const MAX_BUSY_PAUSE: Duration = Duration::from_secs(15);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// 30-секундное превью Deezer: векторы, темп, тональность, настроение.
    Preview,
    /// Полный файл: ещё сетка долей, вступление, концовка, участки с голосом, кривая энергии.
    Full,
}

impl Mode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Preview => "preview",
            Self::Full => "full",
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum MlError {
    /// Сервис не ответил: не запущен, перезапускается, сеть.
    #[error("сервис анализа недоступен: {0}")]
    Unreachable(String),
    /// Занят дольше, чем клиент готов ждать.
    #[error("сервис анализа занят другим анализом")]
    Busy,
    #[error("сервис анализа ответил HTTP {status} ({code}): {message}")]
    Api {
        status: u16,
        code: String,
        message: String,
    },
    #[error("непонятный ответ сервиса анализа: {0}")]
    Decode(String),
}

impl MlError {
    /// Сбой сервиса, а не записи: задачу стоит отложить, не тратя попытку.
    pub fn is_service(&self) -> bool {
        match self {
            Self::Unreachable(_) | Self::Busy => true,
            // models_missing: модели не загрузились
            Self::Api { status, .. } => *status == 503,
            Self::Decode(_) => false,
        }
    }

    /// Повтор позже может помочь: ссылка не скачалась (истекла подпись), сбой внутри сервиса.
    /// Неразбираемый файл, слишком короткий или большой — навсегда.
    pub fn is_temporary(&self) -> bool {
        match self {
            Self::Api { status, .. } => *status >= 500 || *status == 429 || *status == 409,
            Self::Unreachable(_) | Self::Busy | Self::Decode(_) => true,
        }
    }
}

/// Ответ `POST /v1/analyze`: строка `track_analysis` в camelCase. Числа — f64: сервис может прислать
/// целое как дробное; незнакомые поля пропускаются — сервис обновляется отдельно от сервера.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Analysis {
    pub source: String,
    pub models: Value,
    pub duration_ms: Option<f64>,
    pub sound: Option<String>,
    pub semantic: Option<String>,
    pub bpm: Option<f64>,
    pub bpm_confidence: Option<f64>,
    pub key_camelot: Option<String>,
    pub key: Option<String>,
    pub key_confidence: Option<f64>,
    pub energy: Option<f64>,
    pub danceability: Option<f64>,
    pub valence: Option<f64>,
    pub moods: Option<Value>,
    pub voice: Option<f64>,
    pub language: Option<String>,
    pub styles: Option<Value>,
    pub intro_end_ms: Option<f64>,
    pub outro_start_ms: Option<f64>,
    pub beats_ms: Option<Vec<f64>>,
    pub downbeats_ms: Option<Vec<f64>>,
    pub vocal_segments: Option<Value>,
    pub energy_curve: Option<Vec<f64>>,
    pub extra: Option<Value>,
    pub timings_ms: Option<Value>,
}

impl Analysis {
    /// Вектор звучания: 1280 чисел.
    pub fn sound_vector(&self) -> Result<Option<Vec<f32>>, MlError> {
        self.sound
            .as_deref()
            .map(|s| {
                decode_f16_base64(s, SOUND_DIM).map_err(|e| MlError::Decode(format!("sound: {e}")))
            })
            .transpose()
    }

    /// Вектор «смысла» (CLAP): 512 чисел.
    pub fn semantic_vector(&self) -> Result<Option<Vec<f32>>, MlError> {
        self.semantic
            .as_deref()
            .map(|s| {
                decode_f16_base64(s, SEMANTIC_DIM)
                    .map_err(|e| MlError::Decode(format!("semantic: {e}")))
            })
            .transpose()
    }
}

/// Ответ `POST /v1/embed-text`.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct TextVector {
    pub semantic: String,
    pub dim: usize,
    /// Запрос, как его понял сервис (переведённый на английский для CLAP).
    pub query: String,
    pub translated: bool,
    pub unknown_words: Vec<String>,
}

impl TextVector {
    pub fn vector(&self) -> Result<Vec<f32>, MlError> {
        decode_f16_base64(&self.semantic, SEMANTIC_DIM).map_err(MlError::Decode)
    }
}

pub struct Ml {
    http: reqwest::Client,
    base: String,
    /// Анализы этого процесса — по одному
    slot: Semaphore,
    /// Первая пауза после «занят»; дальше удваивается
    busy_pause: Duration,
    /// Сколько всего ждать занятый сервис
    busy_wait: Duration,
}

impl Ml {
    pub fn new(base: &str) -> anyhow::Result<Self> {
        let http = reqwest::Client::builder()
            .user_agent(concat!("dalywave/", env!("CARGO_PKG_VERSION")))
            .connect_timeout(Duration::from_secs(5))
            // Полный трек — до ~15 с на сервере, плюс скачивание файла
            .timeout(Duration::from_secs(5 * 60))
            .build()?;
        Ok(Self {
            http,
            base: base.trim_end_matches('/').to_string(),
            slot: Semaphore::new(1),
            busy_pause: Duration::from_secs(1),
            busy_wait: Duration::from_secs(5 * 60),
        })
    }

    /// Короткие паузы при «занят» — для тестов с заглушкой.
    pub fn with_backoff(mut self, pause: Duration, wait: Duration) -> Self {
        self.busy_pause = pause;
        self.busy_wait = wait;
        self
    }

    pub fn url(&self) -> &str {
        &self.base
    }

    /// Разбор аудио по ссылке (подписанная ссылка хранилища или превью Deezer).
    pub async fn analyze(&self, url: &str, mode: Mode) -> Result<Analysis, MlError> {
        let _slot = self
            .slot
            .acquire()
            .await
            .map_err(|_| MlError::Unreachable("клиент остановлен".into()))?;
        let body = json!({ "url": url, "mode": mode.as_str() });
        let started = Instant::now();
        let mut pause = self.busy_pause;
        loop {
            match self.post::<Analysis>("/v1/analyze", &body, None).await {
                Err(MlError::Api { status: 409, .. }) => {
                    if started.elapsed() + pause > self.busy_wait {
                        return Err(MlError::Busy);
                    }
                    tracing::debug!(pause_ms = pause.as_millis() as u64, "ml is busy, waiting");
                    tokio::time::sleep(pause).await;
                    pause = (pause * 2).min(MAX_BUSY_PAUSE);
                }
                other => return other,
            }
        }
    }

    /// Вектор текстового запроса в пространстве CLAP. Русский запрос сервис переводит словарём.
    pub async fn embed_text(&self, text: &str) -> Result<TextVector, MlError> {
        self.post(
            "/v1/embed-text",
            &json!({ "text": text }),
            Some(Duration::from_secs(20)),
        )
        .await
    }

    /// Ответ `/v1/healthz`: загружены ли модели, занят ли сервис.
    pub async fn health(&self) -> Result<Value, MlError> {
        let res = self
            .http
            .get(format!("{}/v1/healthz", self.base))
            .timeout(Duration::from_secs(5))
            .send()
            .await
            .map_err(|e| MlError::Unreachable(e.to_string()))?;
        let status = res.status();
        if !status.is_success() {
            return Err(MlError::Api {
                status: status.as_u16(),
                code: "healthz".into(),
                message: format!("HTTP {status}"),
            });
        }
        res.json().await.map_err(|e| MlError::Decode(e.to_string()))
    }

    async fn post<T: DeserializeOwned>(
        &self,
        path: &str,
        body: &Value,
        timeout: Option<Duration>,
    ) -> Result<T, MlError> {
        let mut req = self.http.post(format!("{}{path}", self.base)).json(body);
        if let Some(t) = timeout {
            req = req.timeout(t);
        }
        let res = req
            .send()
            .await
            .map_err(|e| MlError::Unreachable(e.to_string()))?;
        let status = res.status();
        if status.is_success() {
            return res
                .json::<T>()
                .await
                .map_err(|e| MlError::Decode(e.to_string()));
        }
        let error: Value = res.json().await.unwrap_or(Value::Null);
        let text = |key: &str| {
            error
                .pointer(&format!("/error/{key}"))
                .and_then(Value::as_str)
                .map(str::to_string)
        };
        Err(MlError::Api {
            status: status.as_u16(),
            code: text("code").unwrap_or_else(|| "unknown".into()),
            message: text("message").unwrap_or_else(|| format!("HTTP {status}")),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn errors_are_sorted_by_whose_fault() {
        let api = |status: u16| MlError::Api {
            status,
            code: "x".into(),
            message: "m".into(),
        };
        assert!(MlError::Unreachable("refused".into()).is_service());
        assert!(MlError::Busy.is_service());
        assert!(api(503).is_service(), "модели не загрузились");
        assert!(
            !api(502).is_service() && api(502).is_temporary(),
            "ссылка не скачалась"
        );
        assert!(!api(422).is_temporary(), "файл не разобрать");
        assert!(!api(413).is_temporary());
        assert!(api(500).is_temporary());
    }

    #[test]
    fn analysis_tolerates_missing_and_new_fields() {
        let a: Analysis = serde_json::from_value(json!({
            "source": "preview", "models": {"sound": "x"}, "bpm": 120, "futureField": [1, 2],
            "beatsMs": null, "sound": null
        }))
        .unwrap();
        assert_eq!(a.bpm, Some(120.0));
        assert!(a.sound_vector().unwrap().is_none());
        let bad = Analysis {
            sound: Some("AAAA".into()),
            ..Default::default()
        };
        assert!(matches!(bad.sound_vector(), Err(MlError::Decode(_))));
    }
}
