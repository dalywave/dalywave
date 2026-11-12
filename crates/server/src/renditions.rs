//! Какой файл записи играть: по качеству из настроек слушателя и сети устройства.
//!
//! «Высокое» — основной файл (загруженный lossless вытесняет копию с YouTube, поэтому он и есть основной),
//! «Обычное» — лучший файл с потерями (Opus 130–160 кбит/с), а если у записи только lossless — он.
//! «Экономия» — облегчённая копия Opus 64 кбит/с: делается воркером при первом запросе
//! (пока её нет — отдаётся обычный файл), лежит в хранилище вторым файлом и вычищается по давности
//! использования, когда копий больше предела.

use std::{sync::Arc, time::Duration};

use dalywave_protocol::{
    jobs::{TranscodePayload, kind},
    settings::{CellularQuality, UserSettings, WifiQuality},
    stream::{AudioCodec, AudioContainer, NetworkKind, StreamQuality},
};
use serde_json::json;
use sqlx::{PgConnection, PgPool};
use uuid::Uuid;

use crate::{
    acquire::PlayableFile,
    import::audio::{self, AudioError},
    jobs::{
        self as queue, JobRow, NewJob,
        worker::{Context, Done, JobError, JobResult, Outcome},
    },
    storage::Storage,
};

pub const LANE: &str = "transcode";
/// Вид облегчённой копии.
pub const OPUS64: &str = "opus64";
const OPUS64_KBPS: i32 = 64;
/// Предел копий в хранилище; сверх него уходят давно не игравшие. `DALYWAVE_RENDITIONS_MAX_GB`.
const DEFAULT_LIMIT_GB: u64 = 20;
/// `last_used_at` обновляется не чаще: каждое воспроизведение не пишет в базу.
const TOUCH_EVERY: &str = "1 hour";
/// Нет ffmpeg на этой машине — задача ждёт, не тратя попыток.
const TOOLS_PAUSE: Duration = Duration::from_secs(600);

/// Качество для запроса: явное, иначе по сети и настройкам слушателя. Сайт без `network` — Wi-Fi.
pub fn quality_for(
    explicit: Option<StreamQuality>,
    network: Option<NetworkKind>,
    settings: &UserSettings,
) -> StreamQuality {
    if let Some(q) = explicit {
        return q;
    }
    match network.unwrap_or(NetworkKind::Wifi) {
        NetworkKind::Wifi => match settings.quality_wifi {
            WifiQuality::High => StreamQuality::High,
            WifiQuality::Normal => StreamQuality::Normal,
        },
        NetworkKind::Cellular => match settings.quality_cellular {
            CellularQuality::Normal => StreamQuality::Normal,
            CellularQuality::Saver => StreamQuality::Saver,
        },
    }
}

/// Файл для воспроизведения и что в нём на самом деле.
#[derive(Debug, Clone)]
pub struct Picked {
    pub file: PlayableFile,
    pub quality: StreamQuality,
    pub lossless: bool,
}

struct FileRow {
    id: Uuid,
    storage_key: String,
    codec: String,
    container: String,
    lossless: bool,
    bitrate_kbps: Option<i32>,
    duration_ms: i32,
    loudness_lufs: Option<f32>,
    true_peak_db: Option<f32>,
    waveform: Option<Vec<i16>>,
    primary: bool,
}

impl FileRow {
    fn playable(&self) -> Option<PlayableFile> {
        Some(PlayableFile {
            id: self.id,
            storage_key: self.storage_key.clone(),
            codec: self.codec.parse().ok()?,
            container: self.container.parse().ok()?,
            bitrate_kbps: self.bitrate_kbps,
            duration_ms: self.duration_ms,
            loudness_lufs: self.loudness_lufs,
            true_peak_db: self.true_peak_db,
            waveform: self.waveform.clone(),
        })
    }
}

/// Из файлов записи (основной первым): «Высокое» — основной, «Обычное» — основной, если он с потерями,
/// иначе лучший файл с потерями, а нет такого — всё-таки основной.
fn choose(files: &[FileRow], quality: StreamQuality) -> Option<&FileRow> {
    let primary = files.iter().find(|f| f.primary).or(files.first())?;
    if quality == StreamQuality::High || !primary.lossless {
        return Some(primary);
    }
    files.iter().find(|f| !f.lossless).or(Some(primary))
}

/// Файл записи под качество. `None` — запись не готова. Для «Экономии» без готовой копии ставит её в
/// очередь и отдаёт обычный файл.
pub async fn pick(
    db: &PgPool,
    recording: Uuid,
    quality: StreamQuality,
) -> Result<Option<Picked>, sqlx::Error> {
    let files = sqlx::query_as!(
        FileRow,
        r#"SELECT f.id, f.storage_key, f.codec, f.container, f.lossless, f.bitrate_kbps, f.duration_ms,
                  f.loudness_lufs, f.true_peak_db, f.waveform,
                  f.id = COALESCE(r.primary_file_id,
                      (SELECT id FROM audio_files WHERE recording_id = r.id ORDER BY created_at DESC LIMIT 1))
                    AS "primary!"
           FROM recordings r JOIN audio_files f ON f.recording_id = r.id
           WHERE r.id = $1 AND r.audio_status = 'ready'
           ORDER BY 11 DESC, f.verified DESC, f.bitrate_kbps DESC NULLS LAST, f.created_at DESC"#,
        recording,
    )
    .fetch_all(db)
    .await?;
    let Some(chosen) = choose(&files, quality) else {
        return Ok(None);
    };
    if quality == StreamQuality::Saver
        && let Some(primary) = files.iter().find(|f| f.primary)
    {
        if let Some(copy) = rendition(db, primary).await? {
            return Ok(Some(Picked {
                file: copy,
                quality: StreamQuality::Saver,
                lossless: false,
            }));
        }
        let mut conn = db.acquire().await?;
        request(&mut conn, primary.id).await?;
    }
    let Some(file) = chosen.playable() else {
        return Ok(None);
    };
    Ok(Some(Picked {
        file,
        quality: if quality == StreamQuality::High {
            StreamQuality::High
        } else {
            StreamQuality::Normal
        },
        lossless: chosen.lossless,
    }))
}

/// Готовая копия основного файла; отмечает, что её снова играли.
async fn rendition(db: &PgPool, source: &FileRow) -> Result<Option<PlayableFile>, sqlx::Error> {
    let row = sqlx::query!(
        "UPDATE audio_renditions SET last_used_at = CASE
             WHEN last_used_at < now() - $3::text::interval THEN now() ELSE last_used_at END
         WHERE source_file_id = $1 AND kind = $2
         RETURNING id, storage_key, codec, container, bitrate_kbps, duration_ms",
        source.id,
        OPUS64,
        TOUCH_EVERY,
    )
    .fetch_optional(db)
    .await?;
    Ok(row.and_then(|r| {
        Some(PlayableFile {
            id: r.id,
            storage_key: r.storage_key,
            codec: r.codec.parse().ok()?,
            container: r.container.parse().ok()?,
            bitrate_kbps: Some(r.bitrate_kbps),
            duration_ms: r.duration_ms,
            // Громкость и огибающая — у источника: копия звучит так же
            loudness_lufs: source.loudness_lufs,
            true_peak_db: source.true_peak_db,
            waveform: source.waveform.clone(),
        })
    }))
}

/// Поставить облегчённую копию файла в очередь (одна задача на файл).
pub async fn request(conn: &mut PgConnection, file_id: Uuid) -> Result<(), sqlx::Error> {
    queue::enqueue(
        conn,
        NewJob::new(kind::TRANSCODE, LANE, TranscodePayload { file_id })
            .dedupe(format!("transcode:{file_id}"))
            .max_attempts(3),
    )
    .await?;
    Ok(())
}

pub fn key(file_id: Uuid) -> String {
    format!("renditions/{file_id}/{OPUS64}.m4a")
}

/// Задача `transcode`: скачать источник, сжать в Opus 64 кбит/с, положить рядом.
pub async fn run(ctx: Arc<Context>, job: JobRow) -> JobResult {
    let p: TranscodePayload = job.payload()?;
    let Some(source) = sqlx::query!(
        "SELECT f.storage_key, f.duration_ms FROM audio_files f WHERE f.id = $1",
        p.file_id
    )
    .fetch_optional(&ctx.db)
    .await?
    else {
        return Ok(json!({ "skipped": "gone" }).into());
    };
    let done = sqlx::query_scalar!(
        "SELECT id FROM audio_renditions WHERE source_file_id = $1 AND kind = $2",
        p.file_id,
        OPUS64
    )
    .fetch_optional(&ctx.db)
    .await?;
    if done.is_some() {
        return Ok(json!({ "skipped": "exists" }).into());
    }
    let dir = tempfile::tempdir().map_err(|e| JobError::retry(format!("временная папка: {e}")))?;
    let input = dir.path().join("source");
    let output = dir.path().join("out.m4a");
    match ctx.storage.download_to(&source.storage_key, &input).await {
        Ok(_) => {}
        Err(crate::storage::StorageError::NotFound) => {
            return Err(JobError::permanent("файла-источника нет в хранилище"));
        }
        Err(e) => return Err(e.into()),
    }
    match audio::transcode_opus(&input, &output, OPUS64_KBPS as u32).await {
        Ok(()) => {}
        Err(AudioError::Missing(tool)) => {
            let msg = format!("на сервере нет {tool}");
            ctx.cooldowns.pause(kind::TRANSCODE, TOOLS_PAUSE);
            let worker = job.locked_by.clone().unwrap_or_default();
            queue::postpone(&ctx.db, job.id, &worker, &msg, TOOLS_PAUSE).await?;
            return Ok(Done::Settled(Outcome::Retry(msg)));
        }
        Err(AudioError::Unreadable(e)) => return Err(JobError::permanent(e)),
        Err(AudioError::Failed(e)) => return Err(JobError::retry(e)),
    }
    let key = key(p.file_id);
    let size = ctx
        .storage
        .put_file(&key, &output, &AudioContainer::Mp4.mime(AudioCodec::Opus))
        .await? as i64;
    let inserted = sqlx::query_scalar!(
        "INSERT INTO audio_renditions (source_file_id, kind, storage_key, codec, container, bitrate_kbps,
                                       duration_ms, size_bytes)
         VALUES ($1, $2, $3, 'opus', 'mp4', $4, $5, $6)
         ON CONFLICT (source_file_id, kind) DO NOTHING
         RETURNING id",
        p.file_id,
        OPUS64,
        key,
        OPUS64_KBPS,
        source.duration_ms,
        size,
    )
    .fetch_optional(&ctx.db)
    .await;
    match inserted {
        Ok(id) => Ok(json!({ "rendition": id, "bytes": size }).into()),
        // Источник удалили, пока шло сжатие: копия никому не нужна
        Err(sqlx::Error::Database(e)) if e.is_foreign_key_violation() => {
            let _ = ctx.storage.delete(&key).await;
            Ok(json!({ "skipped": "source_gone" }).into())
        }
        Err(e) => Err(e.into()),
    }
}

/// Предел копий в байтах.
pub fn limit_bytes() -> i64 {
    let gb = std::env::var("DALYWAVE_RENDITIONS_MAX_GB")
        .ok()
        .and_then(|v| v.trim().parse::<u64>().ok())
        .unwrap_or(DEFAULT_LIMIT_GB);
    (gb.saturating_mul(1024 * 1024 * 1024)).min(i64::MAX as u64) as i64
}

/// Итог чистки копий.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Trimmed {
    /// Копии старых основных файлов.
    pub stale: u64,
    /// Давно не игравшие сверх предела.
    pub evicted: u64,
    /// Объекты, стёртые из хранилища.
    pub deleted: u64,
}

/// Чистка копий: устаревшие (основной файл записи сменился) и давно не игравшие сверх `limit` уходят из
/// базы, их объекты — из хранилища (через `storage_trash`: если хранилище не ответило, повтор в следующий раз).
pub async fn trim(db: &PgPool, storage: &Storage, limit: i64) -> Result<Trimmed, sqlx::Error> {
    let stale = sqlx::query!(
        "DELETE FROM audio_renditions x USING audio_files f JOIN recordings r ON r.id = f.recording_id
         WHERE x.source_file_id = f.id AND r.primary_file_id IS DISTINCT FROM f.id",
    )
    .execute(db)
    .await?
    .rows_affected();
    let mut out = Trimmed {
        stale,
        ..Trimmed::default()
    };
    out.evicted = sqlx::query!(
        "DELETE FROM audio_renditions WHERE id IN (
             SELECT id FROM (
                 SELECT id, (sum(size_bytes) OVER (ORDER BY last_used_at DESC, id DESC))::bigint AS kept
                 FROM audio_renditions) x
             WHERE x.kept > $1)",
        limit,
    )
    .execute(db)
    .await?
    .rows_affected();
    let trash =
        sqlx::query_scalar!("SELECT storage_key FROM storage_trash ORDER BY created_at LIMIT 1000")
            .fetch_all(db)
            .await?;
    for key in trash {
        match storage.delete(&key).await {
            Ok(()) | Err(crate::storage::StorageError::NotFound) => {
                sqlx::query!("DELETE FROM storage_trash WHERE storage_key = $1", key)
                    .execute(db)
                    .await?;
                out.deleted += 1;
            }
            Err(e) => {
                tracing::warn!(%key, error = %e, "could not delete a rendition");
                break;
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use dalywave_protocol::settings::DownloadQuality;

    fn file(n: u128, lossless: bool, primary: bool) -> FileRow {
        FileRow {
            id: Uuid::from_u128(n),
            storage_key: format!("k{n}"),
            codec: if lossless { "flac" } else { "opus" }.into(),
            container: "mp4".into(),
            lossless,
            bitrate_kbps: None,
            duration_ms: 1000,
            loudness_lufs: None,
            true_peak_db: None,
            waveform: None,
            primary,
        }
    }

    #[test]
    fn quality_follows_network_and_settings() {
        let mut s = UserSettings::default();
        assert_eq!(quality_for(None, None, &s), StreamQuality::High);
        assert_eq!(
            quality_for(None, Some(NetworkKind::Cellular), &s),
            StreamQuality::Normal
        );
        s.quality_cellular = CellularQuality::Saver;
        s.quality_wifi = WifiQuality::Normal;
        s.quality_download = DownloadQuality::High;
        assert_eq!(
            quality_for(None, Some(NetworkKind::Cellular), &s),
            StreamQuality::Saver
        );
        assert_eq!(
            quality_for(None, Some(NetworkKind::Wifi), &s),
            StreamQuality::Normal
        );
        assert_eq!(
            quality_for(Some(StreamQuality::High), Some(NetworkKind::Cellular), &s),
            StreamQuality::High
        );
    }

    #[test]
    fn lossless_only_when_high() {
        let files = [file(1, true, true), file(2, false, false)];
        assert_eq!(
            choose(&files, StreamQuality::High).unwrap().id,
            Uuid::from_u128(1)
        );
        assert_eq!(
            choose(&files, StreamQuality::Normal).unwrap().id,
            Uuid::from_u128(2)
        );
        assert_eq!(
            choose(&files, StreamQuality::Saver).unwrap().id,
            Uuid::from_u128(2)
        );
        // Только lossless — его и играем
        let only = [file(1, true, true)];
        assert_eq!(
            choose(&only, StreamQuality::Normal).unwrap().id,
            Uuid::from_u128(1)
        );
        // Основной с потерями — он для всех
        let lossy = [file(3, false, true), file(4, true, false)];
        assert_eq!(
            choose(&lossy, StreamQuality::High).unwrap().id,
            Uuid::from_u128(3)
        );
        assert!(choose(&[], StreamQuality::High).is_none());
    }
}
