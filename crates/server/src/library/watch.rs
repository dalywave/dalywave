//! Наблюдение за папками своей музыки (воркер): события файловой системы собираются, пока папки не
//! затихнут на несколько секунд, и уходят одним проходом по изменившимся путям — так переименование
//! (старый и новый путь) попадает в один проход и считается переездом. Раз в несколько часов и при запуске —
//! полный проход: события теряются (сетевые диски, docker на macOS их не шлют).

use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use notify::{Event, EventKind, RecursiveMode, Watcher};
use tokio::{sync::mpsc, task::JoinHandle, time::Instant};

use super::{request_scan, tags};
use crate::jobs::worker::Context;

/// Тишина после последнего события, после которой изменения уходят в проход.
const QUIET: Duration = Duration::from_secs(3);
/// Дольше изменения не копятся, даже если события идут без перерыва (копирование большой папки).
const MAX_WAIT: Duration = Duration::from_secs(30);
/// Полный проход по расписанию.
const RESCAN_EVERY: Duration = Duration::from_secs(6 * 3600);

/// Запустить наблюдение и полные проходы; `None` — папок своей музыки нет.
pub fn spawn(ctx: Arc<Context>) -> Option<JoinHandle<()>> {
    let dirs = ctx.storage.music_dirs().to_vec();
    if dirs.is_empty() {
        return None;
    }
    Some(tokio::spawn(run(ctx, dirs)))
}

/// Стоит ли событие прохода: аудиофайл или папка (у удалённой папки не узнать, что это была папка, —
/// путь без расширения тоже берётся). Скрытое — мимо.
pub fn relevant(path: &Path) -> bool {
    let hidden = path
        .components()
        .any(|c| c.as_os_str().to_str().is_some_and(|s| s.starts_with('.')));
    !hidden && (tags::is_audio(path) || path.extension().is_none() || path.is_dir())
}

/// Пути без вложенных друг в друга: папка уже покрывает свои файлы.
pub fn compact(paths: &BTreeSet<PathBuf>) -> Vec<String> {
    let mut out: Vec<&PathBuf> = Vec::new();
    for p in paths {
        if !out.iter().any(|o| p.starts_with(o)) {
            out.push(p);
        }
    }
    out.into_iter()
        .filter_map(|p| p.to_str().map(str::to_string))
        .collect()
}

async fn run(ctx: Arc<Context>, dirs: Vec<PathBuf>) {
    if let Err(e) = request_scan(&ctx.db, Vec::new()).await {
        tracing::warn!(error = %e, "could not queue a library scan");
    }
    let (tx, mut rx) = mpsc::unbounded_channel::<PathBuf>();
    let watcher = notify::recommended_watcher(move |res: notify::Result<Event>| {
        let Ok(event) = res else { return };
        if matches!(event.kind, EventKind::Access(_)) {
            return;
        }
        for p in event.paths {
            let _ = tx.send(p);
        }
    });
    let _watcher = match watcher {
        Ok(mut w) => {
            for d in &dirs {
                match w.watch(d, RecursiveMode::Recursive) {
                    Ok(()) => tracing::info!(dir = %d.display(), "watching music folder"),
                    Err(e) => {
                        tracing::warn!(dir = %d.display(), error = %e, "music folder is not watched; scheduled scans only")
                    }
                }
            }
            Some(w)
        }
        Err(e) => {
            tracing::warn!(error = %e, "file watching is unavailable; scheduled scans only");
            None
        }
    };
    let mut rescan = tokio::time::interval_at(Instant::now() + RESCAN_EVERY, RESCAN_EVERY);
    let mut pending: BTreeSet<PathBuf> = BTreeSet::new();
    let mut first: Option<Instant> = None;
    let mut deadline = Instant::now() + QUIET;
    loop {
        tokio::select! {
            got = rx.recv() => {
                let Some(path) = got else { break };
                if !relevant(&path) {
                    continue;
                }
                pending.insert(path);
                let now = Instant::now();
                let started = *first.get_or_insert(now);
                deadline = (now + QUIET).min(started + MAX_WAIT);
            }
            _ = tokio::time::sleep_until(deadline), if !pending.is_empty() => {
                let paths = compact(&pending);
                pending.clear();
                first = None;
                tracing::debug!(paths = paths.len(), "music folders changed");
                if let Err(e) = request_scan(&ctx.db, paths).await {
                    tracing::warn!(error = %e, "could not queue a library scan");
                }
            }
            _ = rescan.tick() => {
                if let Err(e) = request_scan(&ctx.db, Vec::new()).await {
                    tracing::warn!(error = %e, "could not queue a library scan");
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn events_are_filtered_and_compacted() {
        assert!(relevant(Path::new("/music/Кино/01.flac")));
        assert!(relevant(Path::new("/music/Кино")));
        assert!(!relevant(Path::new("/music/Кино/cover.jpg")));
        assert!(!relevant(Path::new("/music/.Trash/01.flac")));
        let set: BTreeSet<PathBuf> = [
            "/music/A/01.flac",
            "/music/A",
            "/music/B/02.mp3",
            "/music/Ab/03.mp3",
        ]
        .into_iter()
        .map(PathBuf::from)
        .collect();
        assert_eq!(
            compact(&set),
            vec!["/music/A", "/music/Ab/03.mp3", "/music/B/02.mp3"]
        );
    }
}
