//! Серверный fetcher — второй исполнитель задач `acquire` (с `server` в `DALYWAVE_FETCHER_MODE`).
//!
//! Воркер отдаёт задание сервису fetcher по HTTP вместе с подписанной ссылкой на загрузку и разбирает
//! ответ тем же кодом, что и отчёт загрузчика ([`result::settle`]). YouTube заподозрил бота — серверный
//! fetcher отдыхает 15 минут, а задачи за это время берёт загрузчик на Mac.

use std::{sync::Arc, time::Duration};

use dalywave_protocol::jobs::{AcquirePayload, AttemptOutcome, kind};
use reqwest::StatusCode;
use serde_json::{Value, json};

use super::{Start, candidate_attempts, result, start_attempt};
use crate::{
    jobs::{
        JobRow,
        worker::{Context, Done, JobResult, Outcome},
    },
    storage::UPLOAD_PUT_TTL,
};

/// Исполнитель в `acquisitions.worker` и `audio_files.acquired_by`.
pub const EXECUTOR: &str = "server";
/// Пауза серверного fetcher после бот-проверки и похожих сбоев YouTube.
pub const PAUSE_AFTER_BLOCK: Duration = Duration::from_secs(15 * 60);
const UNREACHABLE_PAUSE: Duration = Duration::from_secs(60);
const BUSY_DELAY: Duration = Duration::from_secs(30);

fn error_result(code: &str, message: String) -> Value {
    json!({ "status": "error", "error": { "code": code, "message": message } })
}

/// Обработчик задачи `acquire` у воркера.
pub async fn run(ctx: Arc<Context>, job: JobRow) -> JobResult {
    let payload: AcquirePayload = job.payload()?;
    // Задачу держит воркер под своим именем, а в данных о файле исполнитель — «server»
    let lock = job.locked_by.clone().unwrap_or_default();
    let upload_key = match start_attempt(&ctx.db, &payload, EXECUTOR).await? {
        Start::Started { upload_key } => upload_key,
        Start::Skip(reason) => return Ok(json!({ "skipped": reason }).into()),
    };
    let url = match ctx.storage.presign_put(&upload_key, UPLOAD_PUT_TTL).await {
        Ok(url) => url,
        Err(e) => {
            let message = format!("хранилище не подписало ссылку: {e}");
            return give_back(&ctx, &job, &payload, &lock, message, BUSY_DELAY).await;
        }
    };
    let body = json!({
        "deezerTrackId": payload.deezer_track_id,
        "upload": { "url": url.as_str(), "method": "PUT", "headers": {} },
        "maxAttempts": candidate_attempts(&payload),
        "preferVideoIds": payload.prefer_video_ids,
        "excludeVideoIds": payload.exclude_video_ids,
        "directUrl": payload.direct_url,
    });
    let res = match ctx
        .fetcher
        .post(format!("{}/v1/acquire", ctx.config.fetcher.url))
        .json(&body)
        .send()
        .await
    {
        Ok(res) => res,
        Err(e) => {
            ctx.cooldowns.pause(kind::ACQUIRE, UNREACHABLE_PAUSE);
            let message = format!("серверный fetcher не ответил: {e}");
            return give_back(&ctx, &job, &payload, &lock, message, UNREACHABLE_PAUSE).await;
        }
    };
    let status = res.status();
    if status == StatusCode::CONFLICT {
        let message = "серверный fetcher занят другим заданием".to_string();
        return give_back(&ctx, &job, &payload, &lock, message, BUSY_DELAY).await;
    }
    let raw = if status.is_success() {
        match res.json::<Value>().await {
            Ok(v) => v,
            Err(e) => error_result("fetcher_response", format!("непонятный ответ fetcher: {e}")),
        }
    } else {
        let text: String = res
            .text()
            .await
            .unwrap_or_default()
            .chars()
            .take(300)
            .collect();
        error_result(
            "fetcher_http",
            format!("fetcher ответил HTTP {status}: {text}"),
        )
    };
    let settled = result::settle(
        &ctx.db,
        &ctx.storage,
        &job,
        &lock,
        EXECUTOR,
        &raw,
        PAUSE_AFTER_BLOCK,
    )
    .await?;
    let Some(settled) = settled else {
        return Ok(Done::Settled(Outcome::Lost));
    };
    if settled.outcome == AttemptOutcome::Postponed {
        tracing::warn!(
            pause_s = PAUSE_AFTER_BLOCK.as_secs(),
            "YouTube refused the server fetcher, pausing it"
        );
        ctx.cooldowns.pause(kind::ACQUIRE, PAUSE_AFTER_BLOCK);
    }
    let message = settled.message.unwrap_or_default();
    Ok(Done::Settled(match settled.outcome {
        AttemptOutcome::Ready | AttemptOutcome::Disputed | AttemptOutcome::Unavailable => {
            Outcome::Done
        }
        AttemptOutcome::Retry | AttemptOutcome::Postponed => Outcome::Retry(message),
        AttemptOutcome::Failed => Outcome::Failed(message),
    }))
}

/// Начать не вышло (хранилище, сервис недоступен или занят): задача ждёт без траты попытки.
async fn give_back(
    ctx: &Context,
    job: &JobRow,
    payload: &AcquirePayload,
    lock: &str,
    message: String,
    delay: Duration,
) -> JobResult {
    tracing::warn!(%message, "acquire job given back");
    let given = result::give_back(&ctx.db, job, payload, lock, &message, delay).await?;
    Ok(Done::Settled(if given {
        Outcome::Retry(message)
    } else {
        Outcome::Lost
    }))
}

/// Ответ `/v1/healthz` серверного fetcher — для раздела «Источники».
pub async fn health(http: &reqwest::Client, url: &str) -> Result<Value, String> {
    let res = http
        .get(format!("{url}/v1/healthz"))
        .timeout(Duration::from_secs(8))
        .send()
        .await
        .map_err(|e| format!("не ответил: {e}"))?;
    let status = res.status();
    if !status.is_success() {
        return Err(format!("ответил HTTP {status}"));
    }
    res.json::<Value>()
        .await
        .map_err(|e| format!("непонятный ответ: {e}"))
}
