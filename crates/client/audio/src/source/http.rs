//! HTTP(S)-источник: скачивает файл в память по Range-запросам, пока декодер читает.
//!
//! - Основной поток качает последовательно от начала (`bytes=N-`).
//! - Параллельно одним запросом берётся хвост файла (`bytes=-64K`): Ogg ищет в конце длительность,
//!   MP4 без faststart держит там `moov`. Так открытие не ждёт лишний круг запросов.
//! - Если декодер прыгнул далеко вперёд (перемотка), основной поток перезапускается с нужного
//!   места; пропущенные куски докачиваются в конце, файл оказывается в памяти целиком.
//! - Обрыв — повтор с той же позиции с паузой (до 5 попыток подряд).
//! - Ссылка перестала действовать (подписанная ссылка R2 живёт 6 часов, а трек могли поставить
//!   на паузу на ночь): ответ 400/401/403/404/410 — новая ссылка у [`Resolver`] вызывающего и
//!   продолжение с той же позиции. Уже скачанные байты остаются, если длина файла та же.
//!
//! Читатель блокируется на условной переменной — это поток декодера, не аудиоколбэк.

use std::io::{self, Read, Seek, SeekFrom};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use symphonia::core::io::MediaSource;

use super::Refresh;
use crate::AudioError;

const CHUNK: usize = 64 * 1024;
const TAIL: u64 = 64 * 1024;
/// Если читатель ждёт дальше этого от курсора загрузки — перезапускаем загрузку с его позиции.
const JUMP: u64 = 256 * 1024;
const MAX_RETRIES: u32 = 5;
/// Сколько раз подряд просить новую ссылку, если и свежая не открывается.
const MAX_REFRESHES: u32 = 3;

#[derive(Clone, Debug, Default)]
pub struct HttpStats {
    pub len: Option<u64>,
    pub downloaded: u64,
    pub requests: u32,
    pub first_byte_ms: Option<f64>,
    pub done_ms: Option<f64>,
    pub range_supported: bool,
    /// Сколько раз ссылку обновляли.
    pub refreshes: u32,
}

#[derive(Default)]
struct State {
    len: Option<u64>,
    data: Vec<u8>,
    /// Скачанные диапазоны [start, end), отсортированы и не пересекаются.
    have: Vec<(u64, u64)>,
    /// Позиция, на которой ждёт читатель (для решения о перезапуске загрузки).
    want: Option<u64>,
    cursor: u64,
    error: Option<String>,
    done: bool,
    range_supported: bool,
    tail_pending: bool,
    stats: HttpStats,
}

impl State {
    fn insert(&mut self, start: u64, end: u64) {
        if start >= end {
            return;
        }
        let mut s = start;
        let mut e = end;
        let mut out = Vec::with_capacity(self.have.len() + 1);
        let mut placed = false;
        for &(a, b) in &self.have {
            if b < s {
                out.push((a, b));
            } else if a > e {
                if !placed {
                    out.push((s, e));
                    placed = true;
                }
                out.push((a, b));
            } else {
                s = s.min(a);
                e = e.max(b);
            }
        }
        if !placed {
            out.push((s, e));
        }
        self.have = out;
    }

    /// Сколько байтов подряд есть начиная с pos.
    fn available_at(&self, pos: u64) -> u64 {
        for &(a, b) in &self.have {
            if a <= pos && pos < b {
                return b - pos;
            }
        }
        0
    }

    /// Первая дыра начиная с from (или с начала, если после from всё есть).
    fn next_gap(&self, from: u64) -> Option<u64> {
        let len = self.len?;
        let gap_after = |p: u64| -> Option<u64> {
            let mut p = p;
            for &(a, b) in &self.have {
                if a <= p && p < b {
                    p = b;
                }
            }
            (p < len).then_some(p)
        };
        gap_after(from).or_else(|| gap_after(0))
    }

    fn complete(&self) -> bool {
        matches!(self.len, Some(l) if self.have.len() == 1 && self.have[0] == (0, l))
    }
}

struct Shared {
    url: Mutex<String>,
    refresh: Option<Refresh>,
    user_agent: String,
    state: Mutex<State>,
    cond: Condvar,
    stop: AtomicBool,
    started: Instant,
}

/// Общая загрузка одного URL; читателей может быть несколько (перемотка, повторное открытие).
#[derive(Clone)]
pub struct HttpCache {
    shared: Arc<Shared>,
}

impl HttpCache {
    /// `with_tail` — параллельно взять последние 64 КБ: нужно Ogg (symphonia при открытии читает
    /// последние ~64 КБ ради длительности и без них не умеет перематывать). MP4 с faststart,
    /// FLAC, MP3 хвост не читают — для них лишний запрос только отнимал бы полосу у начала файла.
    pub(crate) fn start(
        url: String,
        user_agent: String,
        with_tail: bool,
        refresh: Option<Refresh>,
    ) -> Self {
        let shared = Arc::new(Shared {
            url: Mutex::new(url),
            refresh,
            user_agent,
            state: Mutex::new(State {
                tail_pending: with_tail,
                ..Default::default()
            }),
            cond: Condvar::new(),
            stop: AtomicBool::new(false),
            started: Instant::now(),
        });
        let s = shared.clone();
        let _ = std::thread::Builder::new()
            .name("dw-http".into())
            .spawn(move || download_main(s));
        if with_tail {
            let s = shared.clone();
            let _ = std::thread::Builder::new()
                .name("dw-http-tail".into())
                .spawn(move || download_tail(s));
        }
        Self { shared }
    }

    pub fn stop(&self) {
        self.shared.stop.store(true, Ordering::Relaxed);
        self.shared.cond.notify_all();
    }

    pub fn failed(&self) -> bool {
        self.shared
            .state
            .lock()
            .map(|s| s.error.is_some())
            .unwrap_or(true)
    }

    pub fn stats(&self) -> HttpStats {
        let st = self.shared.state.lock().unwrap_or_else(|p| p.into_inner());
        let mut s = st.stats.clone();
        s.len = st.len;
        s.range_supported = st.range_supported;
        s
    }

    /// Ждёт длину файла (первый ответ сервера) и возвращает читателя.
    pub fn reader(&self) -> Result<HttpReader, AudioError> {
        let mut st = self.shared.state.lock().unwrap_or_else(|p| p.into_inner());
        loop {
            if let Some(len) = st.len {
                return Ok(HttpReader {
                    shared: self.shared.clone(),
                    pos: 0,
                    len,
                });
            }
            if let Some(e) = &st.error {
                return Err(AudioError::Network(e.clone()));
            }
            if self.shared.stop.load(Ordering::Relaxed) {
                return Err(AudioError::Network("остановлено".into()));
            }
            st = self
                .shared
                .cond
                .wait_timeout(st, Duration::from_millis(100))
                .map(|r| r.0)
                .unwrap_or_else(|p| p.into_inner().0);
        }
    }
}

fn agent(user_agent: &str) -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_connect(Some(Duration::from_secs(10)))
        .timeout_recv_response(Some(Duration::from_secs(15)))
        .timeout_recv_body(Some(Duration::from_secs(20)))
        .http_status_as_error(false)
        .user_agent(user_agent)
        .build()
        .into()
}

/// Разбирает Content-Range: "bytes a-b/len".
fn parse_content_range(v: &str) -> Option<(u64, u64, Option<u64>)> {
    let v = v.trim().strip_prefix("bytes ")?;
    let (range, total) = v.split_once('/')?;
    let (a, b) = range.split_once('-')?;
    let total = if total == "*" {
        None
    } else {
        total.parse().ok()
    };
    Some((a.parse().ok()?, b.parse().ok()?, total))
}

impl Shared {
    fn url(&self) -> String {
        self.url.lock().unwrap_or_else(|p| p.into_inner()).clone()
    }

    /// Новая ссылка у вызывающего. false — обновить нельзя или он отказал.
    fn refresh_url(&self, status: u16) -> bool {
        let Some(r) = &self.refresh else {
            return false;
        };
        match r.resolver.resolve(&r.track) {
            Ok(resolved) if !resolved.url.is_empty() => {
                log::info!(
                    "link for {} answered HTTP {status}, got a fresh one",
                    r.track.id
                );
                *self.url.lock().unwrap_or_else(|p| p.into_inner()) = resolved.url;
                let mut st = self.state.lock().unwrap_or_else(|p| p.into_inner());
                st.stats.refreshes += 1;
                true
            }
            Ok(_) => false,
            Err(e) => {
                log::warn!("could not refresh the link for {}: {e}", r.track.id);
                false
            }
        }
    }
}

/// Так отвечает хранилище, когда подписанная ссылка истекла или файл заменили.
fn link_expired(status: u16) -> bool {
    matches!(status, 400 | 401 | 403 | 404 | 410)
}

fn set_len(shared: &Shared, st: &mut State, len: u64) {
    if st.len.is_none() {
        st.len = Some(len);
        st.data = vec![0u8; len as usize];
        shared.cond.notify_all();
    }
}

fn download_tail(shared: Arc<Shared>) {
    let agent = agent(&shared.user_agent);
    let res = agent
        .get(&shared.url())
        .header("Range", format!("bytes=-{TAIL}"))
        .call();
    let mut ok = false;
    if let Ok(mut resp) = res
        && resp.status().as_u16() == 206
    {
        let cr = resp
            .headers()
            .get("content-range")
            .and_then(|v| v.to_str().ok())
            .and_then(parse_content_range);
        if let Some((a, b, Some(total))) = cr {
            let mut buf = vec![0u8; (b + 1 - a) as usize];
            if resp.body_mut().as_reader().read_exact(&mut buf).is_ok() {
                let mut st = shared.state.lock().unwrap_or_else(|p| p.into_inner());
                st.stats.requests += 1;
                st.range_supported = true;
                set_len(&shared, &mut st, total);
                st.data[a as usize..=b as usize].copy_from_slice(&buf);
                st.insert(a, b + 1);
                ok = true;
            }
        }
    }
    let mut st = shared.state.lock().unwrap_or_else(|p| p.into_inner());
    st.tail_pending = false;
    if !ok {
        log::debug!("tail prefetch unavailable");
    }
    shared.cond.notify_all();
}

fn download_main(shared: Arc<Shared>) {
    let agent = agent(&shared.user_agent);
    let mut retries = 0u32;
    let mut refreshes = 0u32;
    loop {
        if shared.stop.load(Ordering::Relaxed) {
            return;
        }
        let start = {
            let st = shared.state.lock().unwrap_or_else(|p| p.into_inner());
            if st.complete() || st.done {
                drop(st);
                let mut st = shared.state.lock().unwrap_or_else(|p| p.into_inner());
                st.done = true;
                if st.stats.done_ms.is_none() {
                    st.stats.done_ms = Some(shared.started.elapsed().as_secs_f64() * 1000.0);
                }
                shared.cond.notify_all();
                return;
            }
            match (st.want, st.len) {
                (Some(w), Some(_)) if st.available_at(w) == 0 => w,
                (_, Some(_)) => st.next_gap(st.cursor).unwrap_or(0),
                (_, None) => 0,
            }
        };
        let resp = agent
            .get(&shared.url())
            .header("Range", format!("bytes={start}-"))
            .call();
        let mut resp = match resp {
            Ok(r) => r,
            Err(e) => {
                if fail_or_retry(&shared, &mut retries, e.to_string()) {
                    return;
                }
                continue;
            }
        };
        let status = resp.status().as_u16();
        let (body_start, total) = match status {
            206 => {
                let cr = resp
                    .headers()
                    .get("content-range")
                    .and_then(|v| v.to_str().ok())
                    .and_then(parse_content_range);
                match cr {
                    Some((a, _, t)) => (a, t),
                    None => (start, None),
                }
            }
            200 => {
                // Сервер без Range: качаем целиком от начала.
                let len = resp
                    .headers()
                    .get("content-length")
                    .and_then(|v| v.to_str().ok())
                    .and_then(|v| v.parse().ok());
                (0, len)
            }
            416 => {
                let mut st = shared.state.lock().unwrap_or_else(|p| p.into_inner());
                st.done = true;
                shared.cond.notify_all();
                return;
            }
            other if link_expired(other) && refreshes < MAX_REFRESHES => {
                refreshes += 1;
                if shared.refresh_url(other) {
                    continue;
                }
                let mut st = shared.state.lock().unwrap_or_else(|p| p.into_inner());
                st.error = Some(format!("HTTP {other}: ссылка больше не действует"));
                shared.cond.notify_all();
                return;
            }
            other => {
                let msg = format!("HTTP {other}");
                let mut st = shared.state.lock().unwrap_or_else(|p| p.into_inner());
                st.error = Some(msg);
                shared.cond.notify_all();
                return;
            }
        };
        {
            let mut st = shared.state.lock().unwrap_or_else(|p| p.into_inner());
            st.stats.requests += 1;
            if status == 206 {
                st.range_supported = true;
            }
            match total {
                Some(t) if st.len.is_some_and(|len| len != t) => {
                    // Новая ссылка ведёт на другой файл (сервер заменил запись) — скачанное не годится
                    st.error = Some("файл на сервере заменён".into());
                    shared.cond.notify_all();
                    return;
                }
                Some(t) => set_len(&shared, &mut st, t),
                None if st.len.is_none() => {
                    st.error = Some("сервер не сообщил длину файла".into());
                    shared.cond.notify_all();
                    return;
                }
                None => {}
            }
            st.cursor = body_start;
            if st.stats.first_byte_ms.is_none() {
                st.stats.first_byte_ms = Some(shared.started.elapsed().as_secs_f64() * 1000.0);
            }
        }
        refreshes = 0;
        let mut reader = resp.body_mut().as_reader();
        let mut buf = vec![0u8; CHUNK];
        let mut pos = body_start;
        let restart = loop {
            if shared.stop.load(Ordering::Relaxed) {
                return;
            }
            let n = match reader.read(&mut buf) {
                Ok(0) => break false,
                Ok(n) => n,
                Err(e) => {
                    if fail_or_retry(&shared, &mut retries, e.to_string()) {
                        return;
                    }
                    break true;
                }
            };
            retries = 0;
            let mut st = shared.state.lock().unwrap_or_else(|p| p.into_inner());
            let len = st.len.unwrap_or(0);
            let end = (pos + n as u64).min(len);
            if end > pos {
                st.data[pos as usize..end as usize].copy_from_slice(&buf[..(end - pos) as usize]);
                st.insert(pos, end);
                st.stats.downloaded += end - pos;
            }
            pos = end;
            st.cursor = pos;
            shared.cond.notify_all();
            // Читатель ждёт далеко от нас — перезапуск с его позиции.
            if let Some(w) = st.want
                && st.available_at(w) == 0
                && (w < pos || w > pos + JUMP)
            {
                break true;
            }
            // Дальше уже скачано (например, хвост) — переходим к следующей дыре.
            if pos < len && st.available_at(pos) > 0 {
                break true;
            }
            if pos >= len {
                break false;
            }
        };
        if !restart {
            let mut st = shared.state.lock().unwrap_or_else(|p| p.into_inner());
            if st.complete() {
                st.done = true;
                st.stats.done_ms = Some(shared.started.elapsed().as_secs_f64() * 1000.0);
                shared.cond.notify_all();
                return;
            }
        }
    }
}

/// true — сдаёмся (ошибка записана), false — повторяем.
fn fail_or_retry(shared: &Shared, retries: &mut u32, err: String) -> bool {
    *retries += 1;
    if *retries > MAX_RETRIES {
        let mut st = shared.state.lock().unwrap_or_else(|p| p.into_inner());
        st.error = Some(err);
        shared.cond.notify_all();
        return true;
    }
    log::warn!("http retry {retries}: {err}");
    std::thread::sleep(Duration::from_millis(250 * (1 << (*retries).min(4)) as u64));
    false
}

/// Читатель поверх общей загрузки (Read + Seek для symphonia).
pub struct HttpReader {
    shared: Arc<Shared>,
    pos: u64,
    len: u64,
}

impl Read for HttpReader {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        if self.pos >= self.len || out.is_empty() {
            return Ok(0);
        }
        let mut st = self.shared.state.lock().unwrap_or_else(|p| p.into_inner());
        loop {
            let avail = st.available_at(self.pos);
            if avail > 0 {
                let n = (avail as usize).min(out.len());
                let p = self.pos as usize;
                out[..n].copy_from_slice(&st.data[p..p + n]);
                self.pos += n as u64;
                if st.want == Some(p as u64) {
                    st.want = None;
                }
                return Ok(n);
            }
            if let Some(e) = &st.error {
                return Err(io::Error::other(e.clone()));
            }
            if self.shared.stop.load(Ordering::Relaxed) {
                return Err(io::Error::other("остановлено"));
            }
            // Хвост ещё летит — подождём его, а не будем дёргать основную загрузку.
            let in_tail = st.tail_pending && self.pos + TAIL >= self.len;
            if !in_tail {
                st.want = Some(self.pos);
            }
            self.shared.cond.notify_all();
            st = self
                .shared
                .cond
                .wait_timeout(st, Duration::from_millis(200))
                .map(|r| r.0)
                .unwrap_or_else(|p| p.into_inner().0);
        }
    }
}

impl Seek for HttpReader {
    fn seek(&mut self, from: SeekFrom) -> io::Result<u64> {
        let p = match from {
            SeekFrom::Start(p) => p as i128,
            SeekFrom::End(d) => self.len as i128 + d as i128,
            SeekFrom::Current(d) => self.pos as i128 + d as i128,
        };
        if p < 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "seek before start",
            ));
        }
        self.pos = p as u64;
        Ok(self.pos)
    }
}

impl MediaSource for HttpReader {
    fn is_seekable(&self) -> bool {
        true
    }
    fn byte_len(&self) -> Option<u64> {
        Some(self.len)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ranges_merge() {
        let mut s = State::default();
        s.insert(10, 20);
        s.insert(30, 40);
        s.insert(15, 32);
        assert_eq!(s.have, vec![(10, 40)]);
        s.insert(0, 5);
        assert_eq!(s.have, vec![(0, 5), (10, 40)]);
        s.len = Some(50);
        assert_eq!(s.next_gap(12), Some(40));
        assert_eq!(s.next_gap(45), Some(45));
        assert_eq!(s.available_at(12), 28);
    }

    #[test]
    fn content_range() {
        assert_eq!(
            parse_content_range("bytes 0-99/1000"),
            Some((0, 99, Some(1000)))
        );
        assert_eq!(parse_content_range("bytes 5-9/*"), Some((5, 9, None)));
    }
}
