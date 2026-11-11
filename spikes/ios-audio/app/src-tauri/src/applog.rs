//! Журнал спайка: файл в каталоге данных приложения, stdout и последние строки в памяти
//! (их показывает страница). На iOS файл достаётся через `xcrun simctl get_app_container`.

use std::collections::VecDeque;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

struct Logger {
    file: Mutex<Option<File>>,
    ring: Mutex<VecDeque<String>>,
    start: Instant,
}

static LOGGER: OnceLock<Logger> = OnceLock::new();
static DIR: OnceLock<Option<PathBuf>> = OnceLock::new();

fn wall() -> String {
    let t = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let s = t.as_secs();
    format!(
        "{:02}:{:02}:{:02}.{:03}",
        (s / 3600) % 24,
        (s / 60) % 60,
        s % 60,
        t.subsec_millis()
    )
}

impl log::Log for Logger {
    fn enabled(&self, m: &log::Metadata) -> bool {
        m.level() <= log::Level::Info || m.target().starts_with("dw")
    }

    fn log(&self, r: &log::Record) {
        if !self.enabled(r.metadata()) || r.target().starts_with("symphonia") {
            return;
        }
        let line = format!(
            "{} +{:8.3}s {:5} {}",
            wall(),
            self.start.elapsed().as_secs_f64(),
            r.level(),
            r.args()
        );
        println!("{line}");
        if let Ok(mut f) = self.file.lock()
            && let Some(f) = f.as_mut()
        {
            let _ = writeln!(f, "{line}");
        }
        if let Ok(mut ring) = self.ring.lock() {
            if ring.len() >= 500 {
                ring.pop_front();
            }
            ring.push_back(line);
        }
    }

    fn flush(&self) {
        if let Ok(mut f) = self.file.lock()
            && let Some(f) = f.as_mut()
        {
            let _ = f.flush();
        }
    }
}

pub fn init(dir: Option<PathBuf>) {
    if let Some(d) = &dir {
        let _ = std::fs::create_dir_all(d);
    }
    let _ = DIR.set(dir.clone());
    let file = dir.and_then(|d| {
        OpenOptions::new()
            .create(true)
            .append(true)
            .open(d.join("dw-spike.log"))
            .ok()
    });
    let logger = LOGGER.get_or_init(|| Logger {
        file: Mutex::new(file),
        ring: Mutex::new(VecDeque::new()),
        start: Instant::now(),
    });
    if log::set_logger(logger).is_ok() {
        log::set_max_level(log::LevelFilter::Info);
    }
}

pub fn path() -> Option<PathBuf> {
    DIR.get().cloned().flatten().map(|d| d.join("dw-spike.log"))
}

pub fn uptime() -> f64 {
    LOGGER
        .get()
        .map(|l| l.start.elapsed().as_secs_f64())
        .unwrap_or(0.0)
}

pub fn tail(n: usize) -> Vec<String> {
    LOGGER
        .get()
        .and_then(|l| {
            l.ring
                .lock()
                .ok()
                .map(|r| r.iter().rev().take(n).rev().cloned().collect())
        })
        .unwrap_or_default()
}

fn settings_path(key: &str) -> Option<PathBuf> {
    DIR.get()
        .cloned()
        .flatten()
        .map(|d| d.join(format!("setting-{key}.txt")))
}

pub fn save_setting(key: &str, value: &str) {
    if let Some(p) = settings_path(key) {
        let _ = std::fs::write(p, value);
    }
}

pub fn load_setting(key: &str) -> Option<String> {
    settings_path(key)
        .and_then(|p| std::fs::read_to_string(p).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}
