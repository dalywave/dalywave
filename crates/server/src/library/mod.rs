//! Своя музыка из папок на сервере (`DALYWAVE_MUSIC_DIRS`), только чтение: файлы не копируются и не
//! перекодируются — клиенты получают их как есть по подписанной ссылке API, а форматы, которые они не
//! играют, — копией по запросу ([`crate::renditions`], вид `play`).
//!
//! Воркер проходит папки ([`scan`]), следит за изменениями ([`watch`]) и считает громкость с огибающей
//! каждого файла (задача `library_analyze`), после чего запись уходит на обычный анализ звука.

pub mod catalog;
pub mod scan;
pub mod tags;
pub mod watch;

use std::{path::Path, sync::Arc, time::Duration};

use dalywave_protocol::{
    admin::{
        LibraryDir, LibraryFileProblem, LibraryProblem, LibraryScan, LibraryScanKind,
        LibraryScanStatus, LibraryStatus,
    },
    jobs::{LibraryAnalyzePayload, LibraryScanPayload, kind},
};
use serde_json::json;
use sqlx::PgPool;

use crate::{
    import::audio::{self, AudioError},
    jobs::{
        self as queue, Enqueued, JobRow, NewJob,
        worker::{Context, Done, JobError, JobResult, Outcome},
    },
    storage::Storage,
};

/// Полоса проходов по папкам: один за раз.
pub const SCAN_LANE: &str = "local_scan";
/// Полоса громкости и огибающей файлов.
pub const ANALYZE_LANE: &str = "local_analyze";
/// Нет ffmpeg на этой машине — задача ждёт, не тратя попыток.
const TOOLS_PAUSE: Duration = Duration::from_secs(600);
/// Сколько файлов с ошибками показывать в админке.
const PROBLEMS_SHOWN: i64 = 50;

/// Поставить проход: `paths` пусто — все папки целиком (один в очереди), иначе только эти пути.
pub async fn request_scan(db: &PgPool, paths: Vec<String>) -> Result<Enqueued, sqlx::Error> {
    let mut conn = db.acquire().await?;
    let full = paths.is_empty();
    let mut job =
        NewJob::new(kind::LIBRARY_SCAN, SCAN_LANE, LibraryScanPayload { paths }).max_attempts(3);
    if full {
        job = job.dedupe("library_scan:full").priority(1);
    }
    queue::enqueue(&mut conn, job).await
}

/// Задача `library_analyze`: громкость EBU R128, истинный пик и огибающая файла — прямо с диска, затем
/// полный анализ звука записи (основного её файла).
pub async fn run_analyze(ctx: Arc<Context>, job: JobRow) -> JobResult {
    let p: LibraryAnalyzePayload = job.payload()?;
    let Some(row) = sqlx::query!(
        r#"SELECT lf.path, lf.audio_file_id AS "audio_file_id!", f.recording_id,
                  f.id = r.primary_file_id AS "primary!"
           FROM library_files lf
           JOIN audio_files f ON f.id = lf.audio_file_id
           JOIN recordings r ON r.id = f.recording_id
           WHERE lf.id = $1 AND lf.status = 'ok'"#,
        p.file_id
    )
    .fetch_optional(&ctx.db)
    .await?
    else {
        return Ok(json!({ "skipped": "gone" }).into());
    };
    let level = match audio::loudness(Path::new(&row.path)).await {
        Ok(l) => l,
        Err(AudioError::Missing(tool)) => {
            let msg = format!("на сервере нет {tool}");
            ctx.cooldowns.pause(kind::LIBRARY_ANALYZE, TOOLS_PAUSE);
            let worker = job.locked_by.clone().unwrap_or_default();
            queue::postpone(&ctx.db, job.id, &worker, &msg, TOOLS_PAUSE).await?;
            return Ok(Done::Settled(Outcome::Retry(msg)));
        }
        Err(AudioError::Unreadable(e)) => return Err(JobError::permanent(e)),
        Err(AudioError::Failed(e)) => return Err(JobError::retry(e)),
    };
    let mut tx = ctx.db.begin().await?;
    sqlx::query!(
        "UPDATE audio_files SET loudness_lufs = COALESCE($2, loudness_lufs),
                true_peak_db = COALESCE($3, true_peak_db), waveform = $4
         WHERE id = $1",
        row.audio_file_id,
        level.lufs,
        level.true_peak_db,
        &level.waveform,
    )
    .execute(&mut *tx)
    .await?;
    sqlx::query!(
        "UPDATE library_files SET analyzed_at = now() WHERE id = $1",
        p.file_id
    )
    .execute(&mut *tx)
    .await?;
    if row.primary {
        crate::reco::analysis::request_full(&mut tx, row.recording_id).await?;
    }
    crate::acquire::recording_changed(&mut tx, row.recording_id).await?;
    tx.commit().await?;
    Ok(
        json!({ "lufs": level.lufs, "peak": level.true_peak_db, "points": level.waveform.len() })
            .into(),
    )
}

/// Раздел админки «Библиотека».
pub async fn status(db: &PgPool, storage: &Storage) -> Result<LibraryStatus, sqlx::Error> {
    let per_root = sqlx::query!(
        r#"SELECT root, count(*) AS "files!", COALESCE(sum(size_bytes), 0)::bigint AS "bytes!"
           FROM library_files WHERE status = 'ok' GROUP BY root"#
    )
    .fetch_all(db)
    .await?;
    let dirs = storage
        .music_dirs()
        .iter()
        .map(|d| {
            let path = d.to_string_lossy().to_string();
            let row = per_root.iter().find(|r| r.root == path);
            LibraryDir {
                available: std::fs::read_dir(d).is_ok(),
                files: row.map(|r| r.files).unwrap_or(0),
                size_bytes: row.map(|r| r.bytes).unwrap_or(0),
                path,
            }
        })
        .collect();
    let scan = sqlx::query!(
        "SELECT id, kind, status, files_total, files_done, added, updated, moved, removed, failed, message,
                started_at, finished_at
         FROM library_scans ORDER BY (status = 'running') DESC, started_at DESC LIMIT 1"
    )
    .fetch_optional(db)
    .await?
    .map(|r| LibraryScan {
        id: r.id,
        kind: r.kind.parse().unwrap_or(LibraryScanKind::Full),
        status: r.status.parse().unwrap_or(LibraryScanStatus::Done),
        files_total: r.files_total,
        files_done: r.files_done,
        added: r.added,
        updated: r.updated,
        moved: r.moved,
        removed: r.removed,
        failed: r.failed,
        message: r.message,
        started_at: r.started_at,
        finished_at: r.finished_at,
    });
    let counts = sqlx::query!(
        r#"SELECT count(DISTINCT lf.track_id) FILTER (WHERE lf.status = 'ok') AS "tracks!",
                  count(*) FILTER (WHERE lf.status = 'ok' AND lf.analyzed_at IS NULL) AS "analyzing!",
                  count(*) FILTER (WHERE lf.status = 'ok' AND NOT f.direct) AS "transcoded!",
                  count(*) FILTER (WHERE lf.status = 'error') AS "errors!",
                  count(*) FILTER (WHERE lf.status = 'unsupported') AS "unsupported!"
           FROM library_files lf LEFT JOIN audio_files f ON f.id = lf.audio_file_id"#
    )
    .fetch_one(db)
    .await?;
    let problems = sqlx::query!(
        "SELECT path, status, error, updated_at FROM library_files WHERE status <> 'ok'
         ORDER BY updated_at DESC LIMIT $1",
        PROBLEMS_SHOWN
    )
    .fetch_all(db)
    .await?
    .into_iter()
    .map(|r| LibraryProblem {
        path: r.path,
        problem: r.status.parse().unwrap_or(LibraryFileProblem::Error),
        message: r.error,
        updated_at: r.updated_at,
    })
    .collect();
    Ok(LibraryStatus {
        dirs,
        scan,
        tracks: counts.tracks,
        analyzing: counts.analyzing,
        transcoded: counts.transcoded,
        errors: counts.errors,
        unsupported: counts.unsupported,
        problems,
    })
}
