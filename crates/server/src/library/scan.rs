//! Проход по папкам своей музыки (задача `library_scan`). Полный — все папки; по изменениям — только пути,
//! о которых сообщило наблюдение за папками.
//!
//! Файл с теми же размером и временем изменения не читается. Изменившийся — перечитываются теги, трек
//! остаётся тем же. Новый путь с отпечатком пропавшего файла — это переезд: строка, трек, лайки и история
//! остаются, меняется только путь. Пропавший — уходит из каталога вместе со своими треками без других файлов.

use std::{
    collections::{HashMap, HashSet},
    io::Read,
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant, UNIX_EPOCH},
};

use chrono::{DateTime, Utc};
use dalywave_protocol::{
    admin::LibraryScanKind,
    jobs::{LibraryAnalyzePayload, LibraryScanPayload, kind},
};
use serde_json::json;
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use uuid::Uuid;

use super::{
    ANALYZE_LANE,
    catalog::{self, Described},
    tags::{self, FileInfo, ReadError},
};
use crate::{
    jobs::{
        self as queue, JobRow, NewJob,
        worker::{Context, JobError, JobResult},
    },
    storage::{IMMUTABLE, keys},
};

/// Сколько байт начала файла входит в отпечаток.
const PRINT_HEAD: usize = 256 * 1024;
/// Сколько альбомов без обложки перепроверять за полный проход.
const COVER_RETRIES: i64 = 200;
/// Как часто писать прогресс прохода в базу.
const PROGRESS_EVERY: Duration = Duration::from_millis(700);

/// Файл на диске.
#[derive(Debug, Clone)]
pub struct Found {
    pub path: PathBuf,
    pub root: PathBuf,
    pub size: i64,
    pub mtime: DateTime<Utc>,
}

/// Файл из базы.
#[derive(Debug, Clone)]
struct Known {
    id: Uuid,
    path: String,
    size: i64,
    mtime: DateTime<Utc>,
    fingerprint: String,
    status: String,
    track_id: Option<Uuid>,
    audio_file_id: Option<Uuid>,
}

fn hidden(name: &std::ffi::OsStr) -> bool {
    name.to_str().is_some_and(|n| n.starts_with('.'))
}

/// Обход папки: аудиофайлы со скрытыми папками и файлами — мимо. Символьные ссылки на папки не
/// раскрываются (петли), на файлы — да.
pub fn walk(dir: &Path, root: &Path, out: &mut Vec<Found>, errors: &mut Vec<String>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(e) => {
            errors.push(format!("{}: {e}", dir.display()));
            return;
        }
    };
    let mut dirs = Vec::new();
    for entry in entries.flatten() {
        if hidden(&entry.file_name()) {
            continue;
        }
        let path = entry.path();
        let Ok(ft) = entry.file_type() else { continue };
        if ft.is_dir() {
            dirs.push(path);
            continue;
        }
        if !tags::is_audio(&path) {
            continue;
        }
        if let Some(f) = stat(&path, root) {
            out.push(f);
        }
    }
    dirs.sort();
    for d in dirs {
        walk(&d, root, out, errors);
    }
}

/// Размер и время изменения файла; `None` — не файл или его нет.
pub fn stat(path: &Path, root: &Path) -> Option<Found> {
    let meta = std::fs::metadata(path).ok()?;
    if !meta.is_file() {
        return None;
    }
    let mtime = meta
        .modified()
        .ok()
        .and_then(|m| m.duration_since(UNIX_EPOCH).ok())
        .and_then(|d| DateTime::from_timestamp(d.as_secs() as i64, d.subsec_micros() * 1000))
        .unwrap_or_default();
    Some(Found {
        path: path.to_path_buf(),
        root: root.to_path_buf(),
        size: meta.len() as i64,
        mtime,
    })
}

/// Отпечаток: SHA-256 размера и первых 256 КБ. Узнаёт файл после переезда, не читая его целиком.
pub fn fingerprint(path: &Path) -> std::io::Result<String> {
    let mut f = std::fs::File::open(path)?;
    let size = f.metadata()?.len();
    let mut buf = vec![0u8; PRINT_HEAD];
    let mut filled = 0;
    while filled < buf.len() {
        let n = f.read(&mut buf[filled..])?;
        if n == 0 {
            break;
        }
        filled += n;
    }
    let mut h = Sha256::new();
    h.update(size.to_le_bytes());
    h.update(&buf[..filled]);
    Ok(hex::encode(h.finalize()))
}

/// Папка музыки, в которой лежит путь (самая глубокая из подходящих).
pub fn root_of<'a>(path: &Path, dirs: &'a [PathBuf]) -> Option<&'a PathBuf> {
    dirs.iter()
        .filter(|d| path.starts_with(d) && path != d.as_path())
        .max_by_key(|d| d.components().count())
}

/// Счётчики прохода.
#[derive(Debug, Default, Clone, Copy)]
pub struct Counts {
    pub total: i32,
    pub done: i32,
    pub added: i32,
    pub updated: i32,
    pub moved: i32,
    pub removed: i32,
    pub failed: i32,
}

struct Progress {
    scan_id: Uuid,
    counts: Counts,
    written: Instant,
}

impl Progress {
    async fn save(&mut self, db: &PgPool, force: bool) -> Result<(), sqlx::Error> {
        if !force && self.written.elapsed() < PROGRESS_EVERY {
            return Ok(());
        }
        self.written = Instant::now();
        let c = self.counts;
        sqlx::query!(
            "UPDATE library_scans SET files_total = $2, files_done = $3, added = $4, updated = $5, moved = $6,
                    removed = $7, failed = $8
             WHERE id = $1",
            self.scan_id,
            c.total,
            c.done,
            c.added,
            c.updated,
            c.moved,
            c.removed,
            c.failed,
        )
        .execute(db)
        .await?;
        Ok(())
    }
}

/// Задача `library_scan`.
pub async fn run(ctx: Arc<Context>, job: JobRow) -> JobResult {
    let p: LibraryScanPayload = job.payload()?;
    let dirs: Vec<PathBuf> = ctx.storage.music_dirs().to_vec();
    if dirs.is_empty() {
        return Ok(json!({ "skipped": "no_music_dirs" }).into());
    }
    let kind = if p.paths.is_empty() {
        LibraryScanKind::Full
    } else {
        LibraryScanKind::Changes
    };
    // Прерванный проход (перезапуск воркера) закрывается: идущий — только один
    sqlx::query!(
        "UPDATE library_scans SET status = 'failed', message = 'проход прервался', finished_at = now()
         WHERE status = 'running'"
    )
    .execute(&ctx.db)
    .await?;
    let scan_id = sqlx::query_scalar!(
        "INSERT INTO library_scans (kind) VALUES ($1) RETURNING id",
        kind.as_str()
    )
    .fetch_one(&ctx.db)
    .await?;
    let mut progress = Progress {
        scan_id,
        counts: Counts::default(),
        written: Instant::now(),
    };
    let res = scan(&ctx, &dirs, &p.paths, &mut progress).await;
    progress.save(&ctx.db, true).await?;
    match res {
        Ok(warnings) => {
            let message = (!warnings.is_empty()).then(|| warnings.join("; "));
            sqlx::query!(
                "UPDATE library_scans SET status = 'done', finished_at = now(), message = $2 WHERE id = $1",
                scan_id,
                message
            )
            .execute(&ctx.db)
            .await?;
            let c = progress.counts;
            Ok(json!({
                "scan": scan_id, "files": c.total, "added": c.added, "updated": c.updated,
                "moved": c.moved, "removed": c.removed, "failed": c.failed,
            })
            .into())
        }
        Err(e) => {
            sqlx::query!(
                "UPDATE library_scans SET status = 'failed', finished_at = now(), message = $2 WHERE id = $1",
                scan_id,
                e.to_string()
            )
            .execute(&ctx.db)
            .await?;
            Err(e)
        }
    }
}

async fn known_rows(db: &PgPool, paths: &[String]) -> Result<Vec<Known>, sqlx::Error> {
    if paths.is_empty() {
        return sqlx::query_as!(
            Known,
            "SELECT id, path, size_bytes AS size, mtime, fingerprint, status, track_id, audio_file_id
             FROM library_files"
        )
        .fetch_all(db)
        .await;
    }
    sqlx::query_as!(
        Known,
        "SELECT id, path, size_bytes AS size, mtime, fingerprint, status, track_id, audio_file_id
         FROM library_files
         WHERE EXISTS (SELECT 1 FROM unnest($1::text[]) p WHERE path = p OR starts_with(path, p || '/'))",
        paths
    )
    .fetch_all(db)
    .await
}

/// Что найти на диске: все папки или только изменившиеся пути (внутри папок музыки).
fn collect(dirs: &[PathBuf], paths: &[String]) -> (Vec<Found>, Vec<String>, Vec<String>) {
    let mut found = Vec::new();
    let mut errors = Vec::new();
    let mut scope = Vec::new();
    if paths.is_empty() {
        for d in dirs {
            if !d.is_dir() {
                errors.push(format!("папка {} недоступна", d.display()));
                continue;
            }
            walk(d, d, &mut found, &mut errors);
        }
        return (found, errors, scope);
    }
    for raw in paths {
        let p = PathBuf::from(raw);
        let Some(root) = root_of(&p, dirs) else {
            continue;
        };
        if p.components()
            .any(|c| matches!(c, std::path::Component::ParentDir))
        {
            continue;
        }
        scope.push(raw.trim_end_matches('/').to_string());
        if p.is_dir() {
            walk(&p, root, &mut found, &mut errors);
        } else if tags::is_audio(&p)
            && let Some(f) = stat(&p, root)
        {
            found.push(f);
        }
    }
    (found, errors, scope)
}

async fn scan(
    ctx: &Context,
    dirs: &[PathBuf],
    paths: &[String],
    progress: &mut Progress,
) -> Result<Vec<String>, JobError> {
    let (dirs_owned, paths_owned) = (dirs.to_vec(), paths.to_vec());
    let (found, mut warnings, scope) =
        tokio::task::spawn_blocking(move || collect(&dirs_owned, &paths_owned))
            .await
            .map_err(|e| JobError::retry(format!("обход папок прервался: {e}")))?;
    // Недоступная папка (не смонтирован диск) — не повод стереть всю её музыку из каталога
    let unavailable: Vec<&PathBuf> = dirs.iter().filter(|d| !d.is_dir()).collect();
    if !paths.is_empty() && scope.is_empty() {
        return Ok(warnings);
    }
    progress.counts.total = found.len() as i32;
    progress.save(&ctx.db, true).await?;

    let known = known_rows(&ctx.db, &scope).await?;
    let mut by_path: HashMap<String, Known> =
        known.into_iter().map(|k| (k.path.clone(), k)).collect();
    let mut unchanged = Vec::new();
    let mut fresh: Vec<Found> = Vec::new();
    let mut changed: Vec<(Found, Known)> = Vec::new();
    for f in found {
        let Some(path) = f.path.to_str().map(str::to_string) else {
            warnings.push(format!("имя файла не в UTF-8: {}", f.path.display()));
            continue;
        };
        match by_path.remove(&path) {
            Some(k) if k.size == f.size && k.mtime == f.mtime => unchanged.push(k.id),
            Some(k) => changed.push((f, k)),
            None => fresh.push(f),
        }
    }
    // Всё, что осталось в by_path, на диске не нашлось. Файлы недоступной папки не трогаем
    let mut missing: Vec<Known> = by_path
        .into_values()
        .filter(|k| {
            !unavailable
                .iter()
                .any(|d| Path::new(&k.path).starts_with(d))
        })
        .collect();
    sqlx::query!(
        "UPDATE library_files SET seen_at = now() WHERE id = ANY($1)",
        &unchanged
    )
    .execute(&ctx.db)
    .await?;
    progress.counts.done += unchanged.len() as i32;
    progress.save(&ctx.db, false).await?;

    let mut covers_tried: HashSet<Uuid> = HashSet::new();
    for (f, k) in changed {
        update(ctx, &f, &k, &mut covers_tried, progress).await?;
        progress.counts.done += 1;
        progress.save(&ctx.db, false).await?;
    }
    // Отпечатки пропавших: новый путь с тем же отпечатком — переезд
    let mut by_print: HashMap<String, usize> = HashMap::new();
    for (i, k) in missing.iter().enumerate() {
        by_print.entry(k.fingerprint.clone()).or_insert(i);
    }
    let mut moved_ids: HashSet<Uuid> = HashSet::new();
    for f in fresh {
        let path = f.path.clone();
        let print = tokio::task::spawn_blocking(move || fingerprint(&path))
            .await
            .map_err(|e| JobError::retry(format!("чтение файла прервалось: {e}")))?;
        let print = match print {
            Ok(p) => p,
            Err(e) => {
                record_failure(&ctx.db, &f, "", &format!("файл не читается: {e}"), "error").await?;
                progress.counts.failed += 1;
                progress.counts.done += 1;
                continue;
            }
        };
        if let Some(i) = by_print.remove(&print) {
            let k = &missing[i];
            relocate(&ctx.db, k, &f).await?;
            moved_ids.insert(k.id);
            progress.counts.moved += 1;
        } else {
            add(ctx, &f, &print, &mut covers_tried, progress).await?;
        }
        progress.counts.done += 1;
        progress.save(&ctx.db, false).await?;
    }
    missing.retain(|k| !moved_ids.contains(&k.id));
    let mut conn = ctx.db.acquire().await?;
    for k in &missing {
        let mut tx = ctx.db.begin().await?;
        if let Some(file) = k.audio_file_id {
            catalog::remove_file(&mut tx, file).await?;
        }
        sqlx::query!("DELETE FROM library_files WHERE id = $1", k.id)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        progress.counts.removed += 1;
    }
    if progress.counts.removed > 0 || progress.counts.updated > 0 {
        catalog::sweep(&mut conn).await?;
    }
    drop(conn);
    if paths.is_empty() {
        fill_covers(ctx, &covers_tried).await?;
    }
    Ok(warnings)
}

/// Свои альбомы без обложки (обложку положили в папку позже, прошлая попытка сорвалась): ещё раз из файла
/// или папки. За проход — не больше [`COVER_RETRIES`].
async fn fill_covers(ctx: &Context, tried: &HashSet<Uuid>) -> Result<(), JobError> {
    let rows = sqlx::query!(
        r#"SELECT DISTINCT ON (t.album_id) t.album_id AS "album_id!", lf.path
           FROM library_files lf JOIN tracks t ON t.id = lf.track_id
           JOIN albums al ON al.id = t.album_id
           WHERE lf.status = 'ok' AND al.source = 'local' AND al.cover_key IS NULL
           ORDER BY t.album_id, lf.path
           LIMIT $1"#,
        COVER_RETRIES
    )
    .fetch_all(&ctx.db)
    .await?;
    for r in rows {
        if tried.contains(&r.album_id) {
            continue;
        }
        let path = PathBuf::from(&r.path);
        let embedded = read_info(&path).await?.ok().and_then(|i| i.picture);
        set_cover(ctx, r.album_id, embedded, &path).await;
    }
    Ok(())
}

/// Файл не прочитался: строка со статусом, чтобы админка показала его в списке.
async fn record_failure(
    db: &PgPool,
    f: &Found,
    print: &str,
    message: &str,
    status: &str,
) -> Result<(), sqlx::Error> {
    let path = f.path.to_string_lossy().to_string();
    sqlx::query!(
        "INSERT INTO library_files (path, root, size_bytes, mtime, fingerprint, status, error)
         VALUES ($1, $2, $3, $4, $5, $6, $7)
         ON CONFLICT (path) DO UPDATE SET size_bytes = EXCLUDED.size_bytes, mtime = EXCLUDED.mtime,
            fingerprint = EXCLUDED.fingerprint, status = EXCLUDED.status, error = EXCLUDED.error,
            track_id = NULL, audio_file_id = NULL, seen_at = now(), updated_at = now()",
        path,
        f.root.to_string_lossy().to_string(),
        f.size,
        f.mtime,
        print,
        status,
        message,
    )
    .execute(db)
    .await?;
    Ok(())
}

async fn read_info(path: &Path) -> Result<Result<FileInfo, ReadError>, JobError> {
    let path = path.to_path_buf();
    tokio::task::spawn_blocking(move || tags::read(&path))
        .await
        .map_err(|e| JobError::retry(format!("чтение тегов прервалось: {e}")))
}

fn failure(e: &ReadError) -> (&'static str, String) {
    match e {
        ReadError::Unsupported(f) => ("unsupported", format!("формат {f} не поддерживается")),
        ReadError::Broken(m) => ("error", m.clone()),
    }
}

async fn add(
    ctx: &Context,
    f: &Found,
    print: &str,
    covers: &mut HashSet<Uuid>,
    progress: &mut Progress,
) -> Result<(), JobError> {
    let info = match read_info(&f.path).await? {
        Ok(i) => i,
        Err(e) => {
            let (status, msg) = failure(&e);
            record_failure(&ctx.db, f, print, &msg, status).await?;
            progress.counts.failed += 1;
            return Ok(());
        }
    };
    let Some(key) = keys::library(&f.path) else {
        record_failure(&ctx.db, f, print, "путь не подходит", "error").await?;
        progress.counts.failed += 1;
        return Ok(());
    };
    let d = catalog::describe(&info.tags, &f.path, &f.root);
    let matched = match catalog::match_deezer(&ctx.db, &ctx.deezer, &d, &info).await {
        Ok(m) => m,
        Err(e) if e.is_temporary() => return Err(e.into()),
        Err(e) => {
            tracing::debug!(path = %f.path.display(), error = %e, "deezer match failed");
            None
        }
    };
    let name = f
        .path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    let mut tx = ctx.db.begin().await?;
    let placed = catalog::place(&mut tx, &d, &info, matched, &key, f.size, print, &name).await?;
    let file_id = sqlx::query_scalar!(
        "INSERT INTO library_files (path, root, size_bytes, mtime, fingerprint, status, error, format, track_id,
                                    audio_file_id, tags)
         VALUES ($1, $2, $3, $4, $5, 'ok', NULL, $6, $7, $8, $9)
         ON CONFLICT (path) DO UPDATE SET size_bytes = EXCLUDED.size_bytes, mtime = EXCLUDED.mtime,
            fingerprint = EXCLUDED.fingerprint, status = 'ok', error = NULL, format = EXCLUDED.format,
            track_id = EXCLUDED.track_id, audio_file_id = EXCLUDED.audio_file_id, tags = EXCLUDED.tags,
            analyzed_at = NULL, seen_at = now(), updated_at = now()
         RETURNING id",
        f.path.to_string_lossy().to_string(),
        f.root.to_string_lossy().to_string(),
        f.size,
        f.mtime,
        print,
        info.label,
        placed.track_id,
        placed.audio_file_id,
        serde_json::to_value(&info.tags).unwrap_or_else(|_| json!({})),
    )
    .fetch_one(&mut *tx)
    .await?;
    request_analyze(&mut tx, file_id).await?;
    tx.commit().await?;
    progress.counts.added += 1;
    if let Some(album) = placed.needs_cover
        && covers.insert(album)
    {
        set_cover(ctx, album, info.picture, &f.path).await;
    }
    Ok(())
}

/// Изменившийся на месте файл: свойства, теги и, если надо, обложка.
async fn update(
    ctx: &Context,
    f: &Found,
    k: &Known,
    covers: &mut HashSet<Uuid>,
    progress: &mut Progress,
) -> Result<(), JobError> {
    let path = f.path.clone();
    let print = tokio::task::spawn_blocking(move || fingerprint(&path))
        .await
        .map_err(|e| JobError::retry(format!("чтение файла прервалось: {e}")))?
        .unwrap_or_default();
    let info = match read_info(&f.path).await? {
        Ok(i) => i,
        Err(e) => {
            let (status, msg) = failure(&e);
            let mut tx = ctx.db.begin().await?;
            if let Some(file) = k.audio_file_id {
                catalog::remove_file(&mut tx, file).await?;
            }
            tx.commit().await?;
            record_failure(&ctx.db, f, &print, &msg, status).await?;
            progress.counts.failed += 1;
            return Ok(());
        }
    };
    // Раньше не читался — теперь как новый
    let (Some(audio_file_id), Some(track_id), "ok") =
        (k.audio_file_id, k.track_id, k.status.as_str())
    else {
        return add(ctx, f, &print, covers, progress).await;
    };
    let d: Described = catalog::describe(&info.tags, &f.path, &f.root);
    let mut tx = ctx.db.begin().await?;
    catalog::update_file(&mut tx, audio_file_id, &info, f.size, &print).await?;
    let needs_cover = catalog::retag(&mut tx, track_id, &d, &info).await?;
    sqlx::query!(
        "UPDATE library_files SET size_bytes = $2, mtime = $3, fingerprint = $4, format = $5, tags = $6,
                status = 'ok', error = NULL, analyzed_at = NULL, seen_at = now(), updated_at = now()
         WHERE id = $1",
        k.id,
        f.size,
        f.mtime,
        print,
        info.label,
        serde_json::to_value(&info.tags).unwrap_or_else(|_| json!({})),
    )
    .execute(&mut *tx)
    .await?;
    request_analyze(&mut tx, k.id).await?;
    tx.commit().await?;
    progress.counts.updated += 1;
    if let Some(album) = needs_cover
        && covers.insert(album)
    {
        set_cover(ctx, album, info.picture, &f.path).await;
    }
    Ok(())
}

/// Переезд: тот же файл по новому пути. Трек и всё, что с ним связано, остаются.
async fn relocate(db: &PgPool, k: &Known, f: &Found) -> Result<(), JobError> {
    let path = f.path.to_string_lossy().to_string();
    let mut tx = db.begin().await?;
    sqlx::query!(
        "UPDATE library_files SET path = $2, root = $3, mtime = $4, size_bytes = $5, seen_at = now(),
                updated_at = now()
         WHERE id = $1",
        k.id,
        path,
        f.root.to_string_lossy().to_string(),
        f.mtime,
        f.size,
    )
    .execute(&mut *tx)
    .await?;
    if let (Some(file), Some(key)) = (k.audio_file_id, keys::library(&f.path)) {
        let name = f.path.file_name().map(|n| n.to_string_lossy().to_string());
        sqlx::query!(
            "UPDATE audio_files SET storage_key = $2, source_title = COALESCE($3, source_title) WHERE id = $1",
            file,
            key,
            name
        )
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    Ok(())
}

pub async fn request_analyze(
    conn: &mut sqlx::PgConnection,
    file_id: Uuid,
) -> Result<(), sqlx::Error> {
    queue::enqueue(
        conn,
        NewJob::new(
            kind::LIBRARY_ANALYZE,
            ANALYZE_LANE,
            LibraryAnalyzePayload { file_id },
        )
        .dedupe(format!("library_analyze:{file_id}"))
        .max_attempts(3),
    )
    .await?;
    Ok(())
}

/// Обложка своего альбома: встроенная в файл, иначе cover.jpg/folder.jpg из папки. Сбой — не беда:
/// альбом останется без обложки до следующего файла.
async fn set_cover(ctx: &Context, album_id: Uuid, embedded: Option<Vec<u8>>, path: &Path) {
    let dir = path.parent().map(Path::to_path_buf);
    let bytes = match embedded {
        Some(b) => Some(b),
        None => match dir {
            Some(d) => tokio::task::spawn_blocking(move || tags::folder_cover(&d))
                .await
                .ok()
                .flatten(),
            None => None,
        },
    };
    let Some(bytes) = bytes else { return };
    let processed =
        match tokio::task::spawn_blocking(move || crate::images::process(&bytes, true)).await {
            Ok(Ok(p)) => p,
            Ok(Err(e)) => {
                tracing::debug!(%album_id, error = %e, "cover is not an image");
                return;
            }
            Err(_) => return,
        };
    let prefix = keys::image_prefix(dalywave_protocol::jobs::ImageOwner::Album, album_id);
    for (size, jpeg) in &processed.variants {
        if let Err(e) = ctx
            .storage
            .put(
                &keys::image(&prefix, *size),
                jpeg.clone(),
                "image/jpeg",
                Some(IMMUTABLE),
            )
            .await
        {
            tracing::warn!(%album_id, error = %e, "could not store an album cover");
            return;
        }
    }
    let res = sqlx::query!(
        "UPDATE albums SET cover_key = $2, cover_bytes = $3, cover_palette = $4, updated_at = now()
         WHERE id = $1 AND cover_key IS NULL",
        album_id,
        prefix,
        processed.total_bytes() as i32,
        json!(processed.palette),
    )
    .execute(&ctx.db)
    .await;
    if let Err(e) = res {
        tracing::warn!(%album_id, error = %e, "could not save an album cover");
        return;
    }
    // Фото у своего артиста нет — его лицом становится обложка его альбома
    let res = sqlx::query!(
        "UPDATE artists a SET picture_key = $2, picture_bytes = 0, updated_at = now()
         FROM albums al
         WHERE al.id = $1 AND a.id = al.artist_id AND a.source = 'local' AND a.picture_key IS NULL
           AND a.name <> $3",
        album_id,
        prefix,
        catalog::VARIOUS_ARTISTS,
    )
    .execute(&ctx.db)
    .await;
    if let Err(e) = res {
        tracing::warn!(%album_id, error = %e, "could not set an artist picture");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roots_and_prints() {
        let dirs = vec![PathBuf::from("/music"), PathBuf::from("/music/more")];
        assert_eq!(
            root_of(Path::new("/music/more/a.flac"), &dirs),
            Some(&PathBuf::from("/music/more"))
        );
        assert_eq!(root_of(Path::new("/music/a.flac"), &dirs), Some(&dirs[0]));
        assert_eq!(root_of(Path::new("/other/a.flac"), &dirs), None);
        assert_eq!(root_of(Path::new("/music"), &dirs), None);

        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a.mp3");
        std::fs::write(&a, b"same bytes").unwrap();
        let b = dir.path().join("Кино").join("b.mp3");
        std::fs::create_dir_all(b.parent().unwrap()).unwrap();
        std::fs::write(&b, b"same bytes").unwrap();
        assert_eq!(fingerprint(&a).unwrap(), fingerprint(&b).unwrap());
        std::fs::write(&b, b"other bytes").unwrap();
        assert_ne!(fingerprint(&a).unwrap(), fingerprint(&b).unwrap());
        std::fs::write(dir.path().join("cover.jpg"), b"x").unwrap();
        std::fs::create_dir_all(dir.path().join(".hidden")).unwrap();
        std::fs::write(dir.path().join(".hidden/c.mp3"), b"x").unwrap();
        let mut found = Vec::new();
        let mut errors = Vec::new();
        walk(dir.path(), dir.path(), &mut found, &mut errors);
        let names: Vec<_> = found
            .iter()
            .map(|f| {
                f.path
                    .strip_prefix(dir.path())
                    .unwrap()
                    .to_string_lossy()
                    .to_string()
            })
            .collect();
        assert_eq!(names, vec!["a.mp3", "Кино/b.mp3"]);
        assert!(errors.is_empty());
    }
}
