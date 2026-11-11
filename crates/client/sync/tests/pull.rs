//! Проход синхронизации против заглушки сервера: страницы по курсору, закреплённое без сети уходит на сервер.

use std::{
    io::{BufRead, BufReader, Read, Write},
    net::TcpListener,
    sync::{Arc, Mutex},
    thread,
};

use dalywave_store::Store;
use dalywave_sync::Client;
use serde_json::{Value, json};

/// Запросы, которые дошли до заглушки: метод, путь с query, тело.
type Log = Arc<Mutex<Vec<(String, String, String)>>>;

fn serve(respond: impl Fn(&str, &str) -> (u16, Value) + Send + 'static) -> (String, Log) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let log: Log = Arc::default();
    let seen = log.clone();
    thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut first = String::new();
            reader.read_line(&mut first).unwrap();
            let mut parts = first.split_whitespace();
            let method = parts.next().unwrap_or("").to_string();
            let path = parts.next().unwrap_or("").to_string();
            let mut len = 0usize;
            let mut auth = false;
            loop {
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                let l = line.trim_end().to_ascii_lowercase();
                if l.is_empty() {
                    break;
                }
                if let Some(v) = l.strip_prefix("content-length: ") {
                    len = v.parse().unwrap_or(0);
                }
                if l == "authorization: bearer dwd_test" {
                    auth = true;
                }
            }
            let mut body = vec![0u8; len];
            reader.read_exact(&mut body).unwrap();
            seen.lock().unwrap().push((
                method.clone(),
                path.clone(),
                String::from_utf8_lossy(&body).into(),
            ));
            let (status, v) = if auth {
                respond(&method, &path)
            } else {
                (401, json!({}))
            };
            let text = v.to_string();
            let head = format!(
                "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                text.len()
            );
            let _ = stream.write_all(head.as_bytes());
            let _ = stream.write_all(text.as_bytes());
        }
    });
    (url, log)
}

fn page(cursor: &str, more: bool, liked: &[&str]) -> Value {
    json!({
        "cursor": cursor, "hasMore": more,
        "artists": [], "albums": [], "tracks": [], "recordings": [],
        "likedTracks": liked.iter().enumerate().map(|(i, id)| json!({
            "trackId": id, "rev": i + 1, "likedAt": "2026-11-11T10:00:00Z"
        })).collect::<Vec<_>>(),
        "savedAlbums": [], "followedArtists": [], "blockedItems": [], "playlists": [],
        "playlistItems": [], "deviceDownloads": [], "tombstones": []
    })
}

const T1: &str = "00000000-0000-0000-0000-000000000001";
const T2: &str = "00000000-0000-0000-0000-000000000002";

#[tokio::test]
async fn pulls_pages_by_cursor() {
    let (url, log) = serve(|_, path| {
        if path.contains("since=5.1.9") {
            (200, page("9", false, &[T2]))
        } else {
            (200, page("5.1.9", true, &[T1]))
        }
    });
    let store = Mutex::new(Store::memory().unwrap());
    let client = Client::new(reqwest::Client::new(), &url, "dwd_test");
    let p = client.pull(&store).await.unwrap();
    assert_eq!(p.pages, 2);
    assert!(p.changes.library);
    let s = store.lock().unwrap();
    assert_eq!(s.cursor().unwrap().as_deref(), Some("9"));
    assert!(s.synced().unwrap());
    let paths: Vec<String> = log.lock().unwrap().iter().map(|r| r.1.clone()).collect();
    assert!(paths[0].starts_with("/api/sync?limit="));
    assert!(!paths[0].contains("since"), "первый раз — без курсора");
    assert!(paths[1].contains("since=5.1.9"));
}

#[tokio::test]
async fn unauthorized_is_reported() {
    let (url, _) = serve(|_, _| (200, page("1", false, &[])));
    let store = Mutex::new(Store::memory().unwrap());
    let client = Client::new(reqwest::Client::new(), &url, "dwd_wrong");
    let e = client.pull(&store).await.unwrap_err();
    assert!(matches!(e, dalywave_sync::SyncError::Unauthorized));
    assert!(store.lock().unwrap().cursor().unwrap().is_none());
}

#[tokio::test]
async fn pins_made_offline_reach_the_server() {
    let pin_id = "00000000-0000-0000-0000-0000000000aa";
    let (url, log) = serve(move |method, path| match (method, path) {
        ("POST", "/api/devices/current/downloads") => (
            200,
            json!({ "id": pin_id, "kind": "liked", "refId": null, "createdAt": "2026-11-11T10:00:00Z",
                    "tracksReady": 0, "tracksTotal": 0 }),
        ),
        ("DELETE", _) => (204, json!(null)),
        _ => (404, json!({})),
    });
    let store = Mutex::new(Store::memory().unwrap());
    store.lock().unwrap().add_pin("liked", None).unwrap();
    let client = Client::new(reqwest::Client::new(), &url, "dwd_test");
    assert_eq!(client.push_pins(&store).await.unwrap(), 1);
    {
        let s = store.lock().unwrap();
        assert!(s.pending_pins().unwrap().is_empty());
        assert_eq!(s.pins().unwrap()[0].server_id.unwrap().to_string(), pin_id);
        s.remove_pin("liked", None).unwrap();
    }
    assert_eq!(client.push_pins(&store).await.unwrap(), 1);
    assert!(store.lock().unwrap().pending_pins().unwrap().is_empty());
    let calls: Vec<(String, String)> = log
        .lock()
        .unwrap()
        .iter()
        .map(|r| (r.0.clone(), r.1.clone()))
        .collect();
    assert_eq!(
        calls[0],
        ("POST".into(), "/api/devices/current/downloads".into())
    );
    assert_eq!(
        calls[1],
        (
            "DELETE".into(),
            format!("/api/devices/current/downloads/{pin_id}")
        )
    );
    assert!(log.lock().unwrap()[0].2.contains("\"kind\":\"liked\""));
}
