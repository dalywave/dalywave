//! Свои аудиофайлы. API принимает файл потоком, кладёт его в хранилище (`imports/{импорт}/{позиция}`)
//! и ставит `import_file`. Воркер читает теги и формат, сопоставляет файл с каталогом (ISRC из тегов,
//! иначе артист, название, альбом и длительность), сверяет отпечаток с превью Deezer, считает громкость
//! и огибающую и записывает файл записи. Файл лучше основного (lossless лучше любой копии с YouTube)
//! становится основным и уходит на полный анализ. Не нашёлся — ждёт выбора трека человеком.

use std::{path::Path, sync::Arc, time::Duration};

use dalywave_protocol::{
    imports::{ImportFileInfo, ImportFilePayload, ImportItemKind, ImportStats},
    jobs::kind,
    stream::AudioCodec,
};
use serde_json::json;
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use tokio::io::AsyncReadExt;
use uuid::Uuid;

use super::{
    audio::{self, AudioError},
    cache_key,
    jobs::{AlbumWaiting, match_track},
    lists,
    matching::{ACCEPT, Wanted},
};
use crate::{
    acquire::{self, result as acq},
    catalog,
    jobs::{
        self as queue, JobRow, NewJob,
        worker::{Context, Done, JobError, JobResult, Outcome},
    },
    storage::keys,
};

/// Нет ffmpeg или fpcalc на этой машине — задача ждёт, не тратя попыток.
const TOOLS_PAUSE: Duration = Duration::from_secs(600);
/// Без отпечатка файл принимается только при такой уверенности (или по ISRC).
const NO_PRINT_ACCEPT: f32 = 0.9;

/// Расширения, которые принимаем.
pub const EXTENSIONS: [&str; 12] = [
    "flac", "m4a", "mp4", "alac", "mp3", "opus", "ogg", "oga", "webm", "wav", "aif", "aiff",
];

/// Похож ли файл на аудио по первым байтам (а не по расширению).
pub fn sniff(head: &[u8]) -> bool {
    let at = |i: usize, sig: &[u8]| head.len() >= i + sig.len() && &head[i..i + sig.len()] == sig;
    at(0, b"fLaC")
        || at(0, b"ID3")
        || (head.len() >= 2 && head[0] == 0xFF && head[1] & 0xE0 == 0xE0)
        || at(0, b"OggS")
        || (at(0, b"RIFF") && at(8, b"WAVE"))
        || (at(0, b"FORM") && (at(8, b"AIFF") || at(8, b"AIFC")))
        || at(4, b"ftyp")
        || at(0, &[0x1A, 0x45, 0xDF, 0xA3])
}

pub fn extension(name: &str) -> Option<String> {
    let ext = name.rsplit_once('.')?.1.to_ascii_lowercase();
    EXTENSIONS.contains(&ext.as_str()).then_some(ext)
}

pub async fn enqueue(
    conn: &mut sqlx::PgConnection,
    item_id: Uuid,
    track_id: Option<Uuid>,
) -> Result<(), sqlx::Error> {
    let mut job = NewJob::new(
        kind::IMPORT_FILE,
        super::LANE,
        ImportFilePayload { item_id, track_id },
    )
    .max_attempts(4);
    // Выбор человека — отдельная задача: прежняя «не нашли» уже закончилась
    job = match track_id {
        Some(t) => job.dedupe(format!("import_file:{item_id}:{t}")).priority(3),
        None => job.dedupe(format!("import_file:{item_id}")),
    };
    queue::enqueue(conn, job).await?;
    Ok(())
}

struct Item {
    import_id: Uuid,
    import_status: String,
    status: String,
    title: String,
    source_ref: Option<String>,
    file_key: Option<String>,
    file_size: Option<i64>,
}

/// Задача `import_file`.
pub async fn run_file(ctx: Arc<Context>, job: JobRow) -> JobResult {
    let p: ImportFilePayload = job.payload()?;
    let Some(item) = sqlx::query_as!(
        Item,
        "SELECT i.import_id, imp.status AS import_status, i.status, i.title, i.source_ref, i.file_key, i.file_size
         FROM import_items i JOIN imports imp ON imp.id = i.import_id
         WHERE i.id = $1 AND i.kind = 'file'",
        p.item_id
    )
    .fetch_optional(&ctx.db)
    .await?
    else {
        return Ok(json!({ "skipped": "gone" }).into());
    };
    if item.import_status != "running" && p.track_id.is_none() {
        return Ok(json!({ "skipped": "not_running" }).into());
    }
    if item.status == "matched" && p.track_id.is_none() {
        return Ok(json!({ "skipped": "done" }).into());
    }
    let Some(key) = item.file_key.clone() else {
        return Err(JobError::permanent("у позиции нет файла"));
    };
    let dir = tempfile::tempdir().map_err(|e| JobError::retry(format!("временная папка: {e}")))?;
    let ext = item
        .source_ref
        .as_deref()
        .and_then(extension)
        .unwrap_or_else(|| "bin".into());
    let input = dir.path().join(format!("in.{ext}"));
    match ctx.storage.download_to(&key, &input).await {
        Ok(_) => {}
        Err(crate::storage::StorageError::NotFound) => {
            return Err(JobError::permanent("загруженного файла больше нет"));
        }
        Err(e) => return Err(e.into()),
    }
    let name = item.source_ref.clone().unwrap_or(item.title.clone());
    let res = process(&ctx, &p, &item, &name, &input, dir.path()).await;
    match res {
        Ok(v) => {
            if v.get("accepted").and_then(serde_json::Value::as_bool) == Some(true)
                && let Err(e) = ctx.storage.delete(&key).await
            {
                tracing::warn!(%key, error = %e, "could not delete an uploaded file");
            }
            maybe_finish(&ctx.db, item.import_id).await?;
            Ok(v.into())
        }
        Err(Failure::Tools(msg)) => {
            ctx.cooldowns.pause(kind::IMPORT_FILE, TOOLS_PAUSE);
            let worker = job.locked_by.clone().unwrap_or_default();
            queue::postpone(&ctx.db, job.id, &worker, &msg, TOOLS_PAUSE).await?;
            Ok(Done::Settled(Outcome::Retry(msg)))
        }
        Err(Failure::Unreadable(msg)) => {
            let info = ImportFileInfo {
                name: name.clone(),
                size_bytes: item.file_size.unwrap_or(0),
                ..ImportFileInfo::default()
            };
            sqlx::query!(
                "UPDATE import_items SET status = 'skipped', resolved_by = 'error', file_meta = $2,
                        candidates = '[]', updated_at = now()
                 WHERE id = $1",
                p.item_id,
                serde_json::to_value(ImportFileInfo {
                    error: Some(msg.clone()),
                    ..info
                })
                .unwrap_or_else(|_| json!({})),
            )
            .execute(&ctx.db)
            .await?;
            let _ = ctx.storage.delete(&key).await;
            maybe_finish(&ctx.db, item.import_id).await?;
            Ok(json!({ "unreadable": msg }).into())
        }
        Err(Failure::Job(e)) => Err(e),
    }
}

enum Failure {
    Tools(String),
    Unreadable(String),
    Job(JobError),
}

impl From<AudioError> for Failure {
    fn from(e: AudioError) -> Self {
        match e {
            AudioError::Missing(_) => Self::Tools(e.to_string()),
            AudioError::Unreadable(m) => Self::Unreadable(m),
            AudioError::Failed(m) => Self::Job(JobError::retry(m)),
        }
    }
}

macro_rules! job_failure {
    ($($t:ty),+) => {$(
        impl From<$t> for Failure {
            fn from(e: $t) -> Self {
                Self::Job(e.into())
            }
        }
    )+};
}

job_failure!(
    JobError,
    sqlx::Error,
    crate::storage::StorageError,
    crate::deezer::DeezerError,
    crate::catalog::CatalogError
);

async fn process(
    ctx: &Context,
    p: &ImportFilePayload,
    item: &Item,
    name: &str,
    input: &Path,
    dir: &Path,
) -> Result<serde_json::Value, Failure> {
    let probe = audio::probe(input).await?;
    let target = audio::target(&probe)?;
    let (name_artist, name_title) = lists::from_file_name(name);
    let artist = probe
        .tags
        .artist
        .clone()
        .or(probe.tags.album_artist.clone())
        .unwrap_or(name_artist);
    let title = probe.tags.title.clone().unwrap_or(name_title);
    sqlx::query!(
        "UPDATE import_items SET artist = $2, title = $3, album = $4, duration_ms = $5, isrc = $6, updated_at = now()
         WHERE id = $1",
        p.item_id,
        artist,
        title,
        probe.tags.album,
        probe.duration_ms,
        probe.tags.isrc,
    )
    .execute(&ctx.db)
    .await?;
    let mut info = ImportFileInfo {
        name: name.to_string(),
        size_bytes: item.file_size.unwrap_or(0),
        codec: Some(probe.codec.as_str().to_string()),
        lossless: probe.lossless(),
        bitrate_kbps: probe.bitrate_kbps,
        sample_rate: probe.sample_rate,
        bit_depth: probe.bit_depth.map(i32::from),
        ..ImportFileInfo::default()
    };

    // Какой это трек
    let manual = p.track_id.is_some();
    let (track_id, confidence) = match p.track_id {
        Some(t) => (t, 1.0),
        None => {
            let w = Wanted {
                artist: artist.clone(),
                title: title.clone(),
                album: probe.tags.album.clone(),
                duration_ms: Some(probe.duration_ms),
                isrc: probe.tags.isrc.clone(),
            };
            let key = cache_key(ImportItemKind::File, &artist, &title, None);
            let out = match_track(&ctx.db, &ctx.deezer, &w, &key, &mut AlbumWaiting::new()).await?;
            match out.track_id {
                Some(t) => (t, out.confidence.unwrap_or(ACCEPT)),
                None => {
                    unmatched(&ctx.db, p.item_id, &info, &out.candidates, out.confidence).await?;
                    return Ok(json!({ "accepted": false, "reason": "not_found" }));
                }
            }
        }
    };
    let mut track = track_row(&ctx.db, track_id).await?;
    if track.as_ref().is_some_and(|t| t.recording_id.is_none()) {
        catalog::ensure_track(&ctx.db, &ctx.deezer, track_id)
            .await
            .map_err(|e| JobError::retry(e.to_string()))?;
        track = track_row(&ctx.db, track_id).await?;
    }
    let Some(TrackRow {
        recording_id: Some(recording_id),
        deezer_id,
        isrc,
        ..
    }) = track
    else {
        return Err(Failure::Job(JobError::permanent(
            "у трека нет записи в каталоге",
        )));
    };

    // Та ли это запись: отпечаток файла против превью Deezer
    let similarity = match deezer_id {
        Some(id) => verify(ctx, id, input, dir).await?,
        None => None,
    };
    info.verified = similarity.map(|s| s >= audio::VERIFY_THRESHOLD);
    let by_isrc =
        matches!((&probe.tags.isrc, &isrc), (Some(a), Some(b)) if a.eq_ignore_ascii_case(b));
    let accept = manual
        || match similarity {
            Some(s) => s >= audio::VERIFY_THRESHOLD,
            None => by_isrc || confidence >= NO_PRINT_ACCEPT,
        };
    if !accept {
        let candidate = sqlx::query!(
            r#"SELECT t.deezer_id AS "deezer_id!", t.title, t.title_version, t.duration_ms, al.title AS "album?",
                      (SELECT a.name FROM track_artists ta JOIN artists a ON a.id = ta.artist_id
                       WHERE ta.track_id = t.id ORDER BY ta.position LIMIT 1) AS "artist?"
               FROM tracks t LEFT JOIN albums al ON al.id = t.album_id WHERE t.id = $1 AND t.deezer_id IS NOT NULL"#,
            track_id
        )
        .fetch_optional(&ctx.db)
        .await?
        .map(|r| dalywave_protocol::imports::ImportCandidate {
            deezer_id: r.deezer_id,
            title: if r.title_version.is_empty() { r.title } else { format!("{} {}", r.title, r.title_version) },
            artist: r.artist.unwrap_or_default(),
            album: r.album,
            duration_ms: Some(r.duration_ms),
            score: similarity.unwrap_or(confidence),
        });
        unmatched(
            &ctx.db,
            p.item_id,
            &info,
            &candidate.into_iter().collect::<Vec<_>>(),
            Some(confidence.min(similarity.unwrap_or(1.0))),
        )
        .await?;
        return Ok(json!({ "accepted": false, "reason": "fingerprint", "similarity": similarity }));
    }

    // Как хранить: lossless — во FLAC, Opus — в MP4
    let stored = match target.repack {
        Some(how) => {
            let out = dir.join(format!("out.{}", target.ext));
            audio::repack(input, &out, how).await?;
            out
        }
        None => input.to_path_buf(),
    };
    let level = match audio::loudness(&stored).await {
        Ok(l) => Some(l),
        Err(AudioError::Missing(m)) => return Err(Failure::Tools(format!("на сервере нет {m}"))),
        Err(e) => {
            tracing::warn!(error = %e, "loudness analysis failed for an uploaded file");
            None
        }
    };
    let (size, sha256) = hash_file(&stored)
        .await
        .map_err(|e| JobError::retry(format!("файл не прочитался: {e}")))?;
    let bitrate = if probe.lossless() || target.repack.is_some() {
        Some(((size as f64 * 8.0) / f64::from(probe.duration_ms.max(1))).round() as i32)
    } else {
        probe.bitrate_kbps
    };
    info.bitrate_kbps = bitrate;
    info.codec = Some(target.codec.as_str().to_string());
    let file_id = Uuid::now_v7();
    let storage_key = keys::audio(recording_id, file_id, target.ext);
    ctx.storage
        .put_file(&storage_key, &stored, target.content_type)
        .await?;

    let mut tx = ctx.db.begin().await?;
    let lossless = matches!(
        target.codec,
        AudioCodec::Flac | AudioCodec::Alac | AudioCodec::Pcm
    );
    let verified = info.verified == Some(true);
    sqlx::query!(
        "INSERT INTO audio_files (id, recording_id, storage_key, codec, container, lossless, bitrate_kbps,
                                  sample_rate, channels, bit_depth, duration_ms, size_bytes, sha256, source,
                                  source_title, loudness_lufs, true_peak_db, waveform, match_score, verified,
                                  verify_similarity, acquired_by)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, 'upload', $14, $15, $16, $17, $18, $19,
                 $20, 'upload')",
        file_id,
        recording_id,
        storage_key,
        target.codec.as_str(),
        target.container.as_str(),
        lossless,
        bitrate,
        probe.sample_rate,
        probe.channels,
        probe.bit_depth,
        probe.duration_ms,
        size as i64,
        sha256,
        name,
        level.as_ref().and_then(|l| l.lufs),
        level.as_ref().and_then(|l| l.true_peak_db),
        level.as_ref().map(|l| l.waveform.as_slice()),
        confidence,
        verified,
        similarity,
    )
    .execute(&mut *tx)
    .await?;
    let current = sqlx::query!(
        "SELECT f.codec, f.verified, f.bitrate_kbps FROM recordings r JOIN audio_files f ON f.id = r.primary_file_id
         WHERE r.id = $1",
        recording_id,
    )
    .fetch_optional(&mut *tx)
    .await?;
    // Свой файл, выбранный человеком, проверен им самим: среди равных по формату он не хуже
    let better = match current
        .and_then(|c| Some((acq::parse_codec(&c.codec)?, c.verified, c.bitrate_kbps)))
    {
        None => true,
        Some((codec, was_verified, kbps)) => {
            acq::quality(target.codec, verified || manual, bitrate)
                > acq::quality(codec, was_verified, kbps)
        }
    };
    if better {
        sqlx::query!(
            "UPDATE recordings SET primary_file_id = $2 WHERE id = $1",
            recording_id,
            file_id
        )
        .execute(&mut *tx)
        .await?;
        crate::reco::analysis::request_full(&mut tx, recording_id).await?;
    }
    acquire::set_audio_status(
        &mut tx,
        recording_id,
        dalywave_protocol::catalog::AudioStatus::Ready,
    )
    .await?;
    // Ждущее скачивание с YouTube больше не нужно: файл уже на сервере
    let waiting = sqlx::query_scalar!(
        r#"UPDATE acquisitions SET status = 'cancelled', error = 'загружен свой файл', updated_at = now(),
                  finished_at = now()
           WHERE recording_id = $1 AND status = 'queued'
           RETURNING job_id"#,
        recording_id
    )
    .fetch_all(&mut *tx)
    .await?;
    for job_id in waiting.into_iter().flatten() {
        queue::cancel(&mut tx, job_id).await?;
    }
    info.upgraded = better;
    sqlx::query!(
        "UPDATE import_items SET status = 'matched', track_id = $2, confidence = $3, resolved_by = $4,
                file_meta = $5, candidates = '[]', applied = true, updated_at = now()
         WHERE id = $1",
        p.item_id,
        track_id,
        confidence,
        if manual { "manual" } else { "auto" },
        serde_json::to_value(&info).unwrap_or_else(|_| json!({})),
    )
    .execute(&mut *tx)
    .await?;
    super::add_stats(
        &mut tx,
        item.import_id,
        &ImportStats {
            files: 1,
            upgraded: i64::from(better),
            ..ImportStats::default()
        },
    )
    .await?;
    super::refresh_counts(&mut tx, item.import_id).await?;
    acquire::recording_changed(&mut tx, recording_id).await?;
    tx.commit().await?;
    Ok(
        json!({ "accepted": true, "recordingId": recording_id, "fileId": file_id, "primary": better }),
    )
}

struct TrackRow {
    recording_id: Option<Uuid>,
    deezer_id: Option<i64>,
    isrc: Option<String>,
}

async fn track_row(db: &PgPool, track_id: Uuid) -> Result<Option<TrackRow>, sqlx::Error> {
    sqlx::query_as!(
        TrackRow,
        "SELECT recording_id, deezer_id, isrc FROM tracks WHERE id = $1",
        track_id
    )
    .fetch_optional(db)
    .await
}

async fn unmatched(
    db: &PgPool,
    item_id: Uuid,
    info: &ImportFileInfo,
    candidates: &[dalywave_protocol::imports::ImportCandidate],
    confidence: Option<f32>,
) -> Result<(), sqlx::Error> {
    let mut tx = db.begin().await?;
    let import_id = sqlx::query_scalar!(
        "UPDATE import_items SET status = 'unmatched', track_id = NULL, confidence = $2, candidates = $3,
                file_meta = $4, resolved_by = 'auto', updated_at = now()
         WHERE id = $1
         RETURNING import_id",
        item_id,
        confidence,
        serde_json::to_value(candidates).unwrap_or_else(|_| json!([])),
        serde_json::to_value(info).unwrap_or_else(|_| json!({})),
    )
    .fetch_one(&mut *tx)
    .await?;
    super::refresh_counts(&mut tx, import_id).await?;
    tx.commit().await?;
    Ok(())
}

/// Превью Deezer против файла. `None` — сравнить не с чем (нет превью, файл слишком короткий).
async fn verify(
    ctx: &Context,
    deezer_id: i64,
    input: &Path,
    dir: &Path,
) -> Result<Option<f32>, Failure> {
    let preview = match ctx.deezer.track(deezer_id).await {
        Ok(t) => t.preview,
        Err(crate::deezer::DeezerError::NotFound) => None,
        Err(e) => return Err(e.into()),
    };
    let Some(url) = preview else {
        return Ok(None);
    };
    let res = ctx
        .http
        .get(&url)
        .send()
        .await
        .map_err(|e| JobError::retry(format!("превью не скачалось: {e}")))?;
    if !res.status().is_success() {
        return Ok(None);
    }
    let bytes = res
        .bytes()
        .await
        .map_err(|e| JobError::retry(format!("превью не скачалось: {e}")))?;
    let path = dir.join("preview.mp3");
    tokio::fs::write(&path, &bytes)
        .await
        .map_err(|e| JobError::retry(format!("временный файл: {e}")))?;
    let (preview, track) =
        match tokio::try_join!(audio::fingerprint(&path), audio::fingerprint(input)) {
            Ok(v) => v,
            Err(AudioError::Missing(m)) => {
                return Err(Failure::Tools(format!("на сервере нет {m}")));
            }
            Err(e) => {
                tracing::warn!(error = %e, "fingerprint failed for an uploaded file");
                return Ok(None);
            }
        };
    Ok(audio::similarity(&preview, &track))
}

async fn hash_file(path: &Path) -> std::io::Result<(u64, String)> {
    let mut f = tokio::fs::File::open(path).await?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 1024 * 1024];
    let mut size = 0u64;
    loop {
        let n = f.read(&mut buf).await?;
        if n == 0 {
            break;
        }
        size += n as u64;
        hasher.update(&buf[..n]);
    }
    Ok((size, hex::encode(hasher.finalize())))
}

/// Файлы догружены и все разобраны — импорт закончен.
pub async fn maybe_finish(db: &PgPool, import_id: Uuid) -> Result<(), sqlx::Error> {
    let open = sqlx::query_scalar!(
        r#"SELECT (phase = 'uploading' OR EXISTS (SELECT 1 FROM import_items WHERE import_id = $1 AND status = 'pending'))
                  AS "open!"
           FROM imports WHERE id = $1"#,
        import_id
    )
    .fetch_optional(db)
    .await?
    .unwrap_or(true);
    if !open {
        super::finish(db, import_id).await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sniffing() {
        assert!(sniff(b"fLaC\0\0\0\x22"));
        assert!(sniff(b"ID3\x04\0\0"));
        assert!(sniff(b"RIFF\x24\0\0\0WAVEfmt "));
        assert!(sniff(b"\0\0\0\x20ftypM4A "));
        assert!(sniff(b"OggS\0\x02"));
        assert!(!sniff(b"<html><body>"));
        assert!(!sniff(b"PK\x03\x04"));
        assert_eq!(extension("Song.FLAC").as_deref(), Some("flac"));
        assert_eq!(extension("notes.txt"), None);
    }
}
