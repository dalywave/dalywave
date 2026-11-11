//! Домашний загрузчик и источники аудио.
//!
//! `fetcher worker` на домашнем Mac берёт задания `acquire` у сервера (`POST /api/fetcher/jobs/claim`),
//! качает со своего адреса, кладёт файл по подписанной ссылке в хранилище и отчитывается
//! (`POST /api/fetcher/jobs/{id}/result`). Вход — токен `dwk_…` с единственным правом `downloader`.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use utoipa::ToSchema;
use uuid::Uuid;

/// Что умеет загрузчик.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", default)]
pub struct Capabilities {
    /// Виды заданий; пусто — любые, которые знает сервер.
    pub kinds: Vec<String>,
    /// Есть сервис PO-токенов.
    pub pot_provider: bool,
    /// Есть cookies аккаунта Google.
    pub cookies: bool,
    /// Клиенты плеера YouTube; пусто — выбор yt-dlp.
    pub player_clients: Vec<String>,
    /// Версия yt-dlp.
    #[schema(required = true)]
    pub yt_dlp: Option<String>,
}

/// Запрос задания. Пока заданий нет, сервер держит запрос до ~25 секунд, затем отвечает 204
/// с `Retry-After` — через сколько секунд спросить снова.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ClaimRequest {
    /// Имя машины, например `dalynkaa-mac`.
    pub worker_id: String,
    /// Версия fetcher.
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub capabilities: Capabilities,
}

/// Куда положить файл: подписанная ссылка хранилища. Content-Type в подпись не входит —
/// контейнер известен только после скачивания.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UploadTarget {
    pub url: String,
    /// Всегда `PUT`.
    pub method: String,
    pub headers: BTreeMap<String, String>,
    /// Ссылка живёт дольше аренды задания.
    pub expires_at: DateTime<Utc>,
}

/// Задание загрузчику.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct FetcherJob {
    /// id задачи очереди: с ним отчёт и продление аренды.
    pub id: Uuid,
    /// Всегда `acquire`.
    pub kind: String,
    pub deezer_track_id: i64,
    /// Сколько кандидатов пробовать скачать.
    pub max_attempts: i16,
    /// Видео, выбранные вручную в «Спорных»: пробуются первыми и принимаются без совпадения отпечатка.
    pub prefer_video_ids: Vec<String>,
    pub exclude_video_ids: Vec<String>,
    /// Прямая ссылка из «Спорных» (SoundCloud, Bandcamp, …) вместо поиска.
    #[schema(required = true)]
    pub direct_url: Option<String>,
    pub upload: UploadTarget,
    /// Не пришёл результат к этому сроку — задание вернётся в очередь. Продлевается `…/lease`.
    pub lease_expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ClaimResponse {
    pub job: FetcherJob,
}

/// Отчёт о задании: `result` — тот же объект, что отдаёт `POST /v1/acquire` сервиса fetcher
/// (статус, выбранный кандидат, файл с громкостью и формой волны, попытки, кандидаты, ошибка).
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ResultReport {
    pub worker_id: String,
    #[schema(value_type = Object)]
    pub result: Value,
}

/// Продление аренды: загрузчик ещё работает над заданием.
#[derive(Debug, Clone, Default, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct LeaseRequest {
    #[serde(default)]
    pub worker_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Lease {
    pub lease_expires_at: DateTime<Utc>,
}

/// Загрузчик бросает задание (приложение закрывают посреди скачивания): задание сразу возвращается
/// в очередь, попытка не тратится.
#[derive(Debug, Clone, Default, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseRequest {
    #[serde(default)]
    pub worker_id: Option<String>,
    /// Почему: `quit`, `pause`, `control` — для журнала задачи.
    #[serde(default)]
    pub reason: Option<String>,
}

// ───────────── Админка: загрузчики и источники ─────────────

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NewDownloader {
    /// Например «Mac Захара».
    pub name: String,
}

/// Домашний загрузчик в разделе «Источники».
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Downloader {
    pub id: Uuid,
    pub name: String,
    pub created_at: DateTime<Utc>,
    #[schema(required = true)]
    pub revoked_at: Option<DateTime<Utc>>,
    /// Последнее обращение: запрос задания, продление аренды, отчёт.
    #[schema(required = true)]
    pub last_seen_at: Option<DateTime<Utc>>,
    #[schema(required = true)]
    pub last_claim_at: Option<DateTime<Utc>>,
    /// Обращался в последние две минуты.
    pub online: bool,
    /// Имя машины из последнего запроса.
    #[schema(required = true)]
    pub worker_id: Option<String>,
    /// Версия fetcher.
    #[schema(required = true)]
    pub version: Option<String>,
    #[schema(required = true)]
    pub yt_dlp_version: Option<String>,
    pub capabilities: Capabilities,
    /// Задания в работе прямо сейчас.
    pub running: i64,
    /// Принятые файлы за сутки.
    pub ok_24h: i64,
    /// Сбои за сутки: повторы и окончательные.
    pub failed_24h: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DownloaderList {
    pub items: Vec<Downloader>,
}

/// Новый загрузчик. Токен показывается один раз: на сервере остаётся только его хеш.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DownloaderToken {
    /// `dwk_…`: `fetcher worker --token` или файл `~/.config/dalywave/downloader-token`.
    pub token: String,
    pub downloader: Downloader,
}

/// Серверный fetcher — второй исполнитель (`DALYWAVE_FETCHER_MODE` с `server`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ServerFetcher {
    pub enabled: bool,
    #[schema(required = true)]
    pub url: Option<String>,
    /// Ответил ли `/v1/healthz`; `null`, если серверный fetcher выключен.
    #[schema(required = true)]
    pub reachable: Option<bool>,
    /// Ответ `/v1/healthz`: версии yt-dlp, Deno, ffmpeg, сервис PO-токенов, срок cookies.
    #[schema(value_type = Option<Object>, required = true)]
    pub health: Option<Value>,
    #[schema(required = true)]
    pub error: Option<String>,
    pub running: i64,
    pub ok_24h: i64,
    pub failed_24h: i64,
}

/// Раздел «Источники»: кто качает аудио.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Sources {
    pub downloaders: Vec<Downloader>,
    pub server: ServerFetcher,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn claim_accepts_the_fetcher_hello() {
        // Ровно то, что шлёт `fetcher worker`; лишние поля не мешают
        let req: ClaimRequest = serde_json::from_value(serde_json::json!({
            "workerId": "dalynkaa-mac",
            "version": "0.1.0",
            "capabilities": {"kinds": ["acquire"], "potProvider": true, "cookies": false,
                             "playerClients": [], "ytDlp": "2026.10.26", "future": 1}
        }))
        .unwrap();
        assert_eq!(req.capabilities.kinds, vec!["acquire"]);
        assert_eq!(req.capabilities.yt_dlp.as_deref(), Some("2026.10.26"));
        let bare: ClaimRequest =
            serde_json::from_value(serde_json::json!({"workerId": "x"})).unwrap();
        assert!(bare.capabilities.kinds.is_empty());
    }
}
