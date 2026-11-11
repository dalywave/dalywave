//! Обработчики задач: копии картинок, обновление артистов, сверка дискографий подписок и скачивание
//! аудио через серверный fetcher (только с `server` в `DALYWAVE_FETCHER_MODE`; иначе задачи `acquire`
//! берёт загрузчик на Mac).

use std::sync::Arc;

use dalywave_protocol::jobs::{
    ImageOwner, MirrorImagePayload, RefreshArtistPayload, SyncDiscographyPayload, kind,
};
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

use super::{
    Enqueued, JobRow, NewJob, lanes,
    worker::{Context, JobError, JobResult, Registry},
};
use crate::{
    acquire,
    catalog::{self, ArtistParts},
    config::FetcherConfig,
    images, jobs,
    storage::{IMMUTABLE, keys},
};

/// Больше Deezer не отдаёт: всё крупнее — не обложка
const MAX_IMAGE_BYTES: usize = 15 * 1024 * 1024;

/// Обработчики воркера. Задачи `acquire` воркер выполняет сам, только если включён серверный fetcher.
pub fn registry(fetcher: &FetcherConfig) -> Registry {
    let mut registry = Registry::default();
    registry
        .register(kind::MIRROR_IMAGE, mirror_image)
        .register(kind::REFRESH_ARTIST, refresh_artist)
        .register(kind::SYNC_DISCOGRAPHY, sync_discography);
    if fetcher.server {
        registry.register(kind::ACQUIRE, acquire::server::run);
    } else {
        registry.elsewhere(kind::ACQUIRE);
    }
    registry
}

/// Ставит копирование картинки в очередь (одна задача на картинку).
pub async fn request_mirror(
    db: &PgPool,
    owner: ImageOwner,
    id: Uuid,
    force: bool,
) -> Result<Enqueued, sqlx::Error> {
    let mut conn = db.acquire().await?;
    jobs::enqueue(
        &mut conn,
        NewJob::new(
            kind::MIRROR_IMAGE,
            lanes::MIRROR,
            MirrorImagePayload { owner, id, force },
        )
        .dedupe(format!("mirror_image:{owner}:{id}")),
    )
    .await
}

/// Скачивает картинку Deezer, кладёт в хранилище три размера JPEG, для обложки — палитру.
pub async fn mirror_image(ctx: Arc<Context>, job: JobRow) -> JobResult {
    let p: MirrorImagePayload = job.payload()?;
    let row = match p.owner {
        ImageOwner::Album => sqlx::query!(
            r#"SELECT cover_source AS source, cover_key AS key FROM albums WHERE id = $1"#,
            p.id
        )
        .fetch_optional(&ctx.db)
        .await?
        .map(|r| (r.source, r.key)),
        ImageOwner::Artist => sqlx::query!(
            r#"SELECT picture_source AS source, picture_key AS key FROM artists WHERE id = $1"#,
            p.id
        )
        .fetch_optional(&ctx.db)
        .await?
        .map(|r| (r.source, r.key)),
    };
    let Some((source, key)) = row else {
        return Err(JobError::permanent("нет такой записи каталога"));
    };
    let Some(source) = source else {
        return Ok(json!({ "skipped": "no_source" }).into());
    };
    if key.is_some() && !p.force {
        return Ok(json!({ "skipped": "already_mirrored" }).into());
    }

    let res = ctx
        .http
        .get(&source)
        .send()
        .await
        .map_err(|e| JobError::retry(format!("картинка не скачалась: {e}")))?;
    let status = res.status();
    if status.as_u16() == 404 || status.as_u16() == 410 {
        return Err(JobError::permanent(format!(
            "картинки нет у источника: HTTP {status}"
        )));
    }
    if !status.is_success() {
        return Err(JobError::retry(format!(
            "картинка не скачалась: HTTP {status}"
        )));
    }
    if res
        .content_length()
        .is_some_and(|n| n as usize > MAX_IMAGE_BYTES)
    {
        return Err(JobError::permanent("картинка слишком большая"));
    }
    let bytes = res
        .bytes()
        .await
        .map_err(|e| JobError::retry(format!("картинка не скачалась: {e}")))?;
    if bytes.len() > MAX_IMAGE_BYTES {
        return Err(JobError::permanent("картинка слишком большая"));
    }
    let with_palette = p.owner == ImageOwner::Album;
    let processed = tokio::task::spawn_blocking(move || images::process(&bytes, with_palette))
        .await
        .map_err(|e| JobError::retry(format!("обработка картинки прервалась: {e}")))?
        .map_err(|e| JobError::permanent(format!("это не картинка: {e:#}")))?;

    let prefix = keys::image_prefix(p.owner, p.id);
    for (size, jpeg) in &processed.variants {
        ctx.storage
            .put(
                &keys::image(&prefix, *size),
                jpeg.clone(),
                "image/jpeg",
                Some(IMMUTABLE),
            )
            .await?;
    }
    let bytes = processed.total_bytes() as i32;
    // Если за это время у Deezer сменилась картинка, копия старой уже не нужна: её переснимет следующая задача
    let updated = match p.owner {
        ImageOwner::Album => {
            sqlx::query!(
                "UPDATE albums SET cover_key = $2, cover_bytes = $3, cover_palette = $4, updated_at = now()
                 WHERE id = $1 AND cover_source = $5",
                p.id,
                prefix,
                bytes,
                json!(processed.palette),
                source,
            )
            .execute(&ctx.db)
            .await?
        }
        ImageOwner::Artist => {
            sqlx::query!(
                "UPDATE artists SET picture_key = $2, picture_bytes = $3, updated_at = now()
                 WHERE id = $1 AND picture_source = $4",
                p.id,
                prefix,
                bytes,
                source,
            )
            .execute(&ctx.db)
            .await?
        }
    };
    Ok(json!({
        "key": prefix,
        "bytes": bytes,
        "palette": processed.palette,
        "applied": updated.rows_affected() == 1,
    })
    .into())
}

/// Обновляет устаревшие части страницы артиста (или все, если `force`).
pub async fn refresh_artist(ctx: Arc<Context>, job: JobRow) -> JobResult {
    let p: RefreshArtistPayload = job.payload()?;
    let Some(freshness) = catalog::artist_freshness(&ctx.db, p.artist_id).await? else {
        return Err(JobError::permanent("нет такого артиста"));
    };
    let parts = if p.force {
        ArtistParts::ALL
    } else {
        freshness.stale()
    };
    let done = catalog::refresh_artist(&ctx.db, &ctx.deezer, p.artist_id, parts).await?;
    Ok(json!({ "refreshed": done }).into())
}

/// Сверка релизов артиста из подписок и постановка новых треков на скачивание.
pub async fn sync_discography(ctx: Arc<Context>, job: JobRow) -> JobResult {
    let p: SyncDiscographyPayload = job.payload()?;
    let report = catalog::sync_discography(&ctx.db, &ctx.deezer, p.artist_id).await?;
    Ok(serde_json::to_value(report)?.into())
}
