//! Докачка с места обрыва и проверка скачанного — против маленького HTTP-сервера на 127.0.0.1.

use std::{
    io::{BufRead, BufReader, Write},
    net::TcpListener,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    thread,
};

use dalywave_downloads::{FetchError, Request, fetch};
use sha2::{Digest, Sha256};

struct Server {
    url: String,
    /// Заголовки Range каждого запроса.
    ranges: Arc<Mutex<Vec<Option<String>>>>,
}

/// Отдаёт `body` с поддержкой Range. Первые `cut_first` запросов обрываются на половине.
fn serve(body: Vec<u8>, cut_first: usize) -> Server {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/f.m4a", listener.local_addr().unwrap());
    let ranges = Arc::new(Mutex::new(Vec::new()));
    let seen = ranges.clone();
    let count = Arc::new(AtomicUsize::new(0));
    thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut range = None;
            loop {
                let mut line = String::new();
                if reader.read_line(&mut line).unwrap_or(0) == 0 {
                    break;
                }
                let l = line.trim_end();
                if l.is_empty() {
                    break;
                }
                if let Some(v) = l.to_ascii_lowercase().strip_prefix("range: bytes=") {
                    range = Some(v.trim_end_matches('-').to_string());
                }
            }
            seen.lock().unwrap().push(range.clone());
            let n = count.fetch_add(1, Ordering::SeqCst);
            let start: usize = range.as_deref().and_then(|r| r.parse().ok()).unwrap_or(0);
            let slice = &body[start..];
            let head = if range.is_some() {
                format!(
                    "HTTP/1.1 206 Partial Content\r\nContent-Length: {}\r\nContent-Range: bytes {}-{}/{}\r\nConnection: close\r\n\r\n",
                    slice.len(),
                    start,
                    body.len() - 1,
                    body.len()
                )
            } else {
                format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    slice.len()
                )
            };
            let _ = stream.write_all(head.as_bytes());
            if n < cut_first {
                // Обрыв посреди тела: отдали половину и закрыли соединение
                let _ = stream.write_all(&slice[..slice.len() / 2]);
                let _ = stream.flush();
                drop(stream);
                continue;
            }
            let _ = stream.write_all(slice);
        }
    });
    Server { url, ranges }
}

fn body() -> Vec<u8> {
    (0..300_000u32).map(|i| (i % 251) as u8).collect()
}

#[test]
fn resumes_after_a_dropped_connection_and_verifies() {
    let data = body();
    let sha = hex::encode(Sha256::digest(&data));
    let server = serve(data.clone(), 1);
    let dir = tempfile::tempdir().unwrap();
    let dest = dir.path().join("t/track.m4a");
    let req = Request {
        url: &server.url,
        dest: &dest,
        size: data.len() as u64,
        sha256: Some(&sha),
        user_agent: "test",
    };
    let cancel = AtomicBool::new(false);
    let first = fetch(&req, &cancel, |_, _| {});
    assert!(matches!(first, Err(FetchError::Network(_))), "{first:?}");
    let part = dir.path().join("t/track.m4a.part");
    let half = std::fs::metadata(&part).unwrap().len();
    assert_eq!(half, data.len() as u64 / 2, "недокачанное сохранено");
    assert!(!dest.exists());

    let mut last = (0, 0);
    let n = fetch(&req, &cancel, |a, b| last = (a, b)).unwrap();
    assert_eq!(n, data.len() as u64);
    assert_eq!(last, (data.len() as u64, data.len() as u64));
    assert_eq!(std::fs::read(&dest).unwrap(), data);
    assert!(!part.exists());
    let ranges = server.ranges.lock().unwrap().clone();
    assert_eq!(
        ranges,
        vec![None, Some(half.to_string())],
        "второй запрос — с места обрыва"
    );
}

#[test]
fn wrong_hash_is_rejected_and_part_removed() {
    let data = body();
    let server = serve(data.clone(), 0);
    let dir = tempfile::tempdir().unwrap();
    let dest = dir.path().join("track.m4a");
    let req = Request {
        url: &server.url,
        dest: &dest,
        size: data.len() as u64,
        sha256: Some("00"),
        user_agent: "test",
    };
    let r = fetch(&req, &AtomicBool::new(false), |_, _| {});
    assert!(matches!(r, Err(FetchError::Corrupt(_))), "{r:?}");
    assert!(!dest.exists());
    assert!(!dir.path().join("track.m4a.part").exists());
    // Неверный размер — тоже
    let req = Request {
        size: data.len() as u64 + 10,
        sha256: None,
        ..req
    };
    let r = fetch(&req, &AtomicBool::new(false), |_, _| {});
    assert!(r.is_err());
    assert!(!dest.exists());
}

#[test]
fn cancel_keeps_the_part_for_later() {
    let data = body();
    let server = serve(data.clone(), 0);
    let dir = tempfile::tempdir().unwrap();
    let dest = dir.path().join("track.m4a");
    let req = Request {
        url: &server.url,
        dest: &dest,
        size: data.len() as u64,
        sha256: None,
        user_agent: "test",
    };
    let cancel = AtomicBool::new(false);
    let r = fetch(&req, &cancel, |got, _| {
        if got > 100_000 {
            cancel.store(true, Ordering::Relaxed);
        }
    });
    assert!(matches!(r, Err(FetchError::Cancelled)));
    let part = dir.path().join("track.m4a.part");
    assert!(std::fs::metadata(&part).unwrap().len() > 100_000);
    cancel.store(false, Ordering::Relaxed);
    fetch(&req, &cancel, |_, _| {}).unwrap();
    assert_eq!(std::fs::read(&dest).unwrap(), data);
}
