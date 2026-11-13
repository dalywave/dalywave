//! Заставка DalyWave для Mac (`apps/shell/saver`, ScreenSaverView + Metal): живая волна, а если играет
//! DalyWave — цвет из обложки и название с исполнителем мелко в углу.
//!
//! - Установка: `make app` кладёт подписанную `DalyWave.saver` в `Contents/Resources`, кнопка
//!   «Установить» в настройках копирует её в `~/Library/Screen Savers` и открывает настройки заставки.
//! - Что играет: заставка живёт в песочнице системы (`legacyScreenSaver`) и к файлам приложения доступа
//!   не имеет, а в сеть ходить может — приложение отвечает на `GET http://127.0.0.1:47815/now-playing`
//!   (только этот адрес и только на этом компьютере). Приложение закрыто — заставка показывает спокойную волну.
#![cfg_attr(not(target_os = "macos"), allow(dead_code))]

use std::sync::Mutex;

use serde::Serialize;
use tauri::{AppHandle, Manager};

use crate::{error::ShellResult, media::NowPlaying};

/// Порт ответа заставке (тот же — в `saver/DalyWaveSaver.swift`).
pub const PORT: u16 = 47815;
const SAVER: &str = "DalyWave.saver";

/// Что играет — JSON для заставки.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NowJson {
    playing: bool,
    title: String,
    artist: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    artwork_url: Option<String>,
}

#[derive(Default)]
pub struct Saver {
    now: Mutex<NowJson>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SaverState {
    /// Заставка есть в приложении (сборка `make app`)
    available: bool,
    /// Уже в `~/Library/Screen Savers`
    installed: bool,
}

pub fn init(app: &AppHandle) {
    app.manage(Saver::default());
    #[cfg(target_os = "macos")]
    {
        let app = app.clone();
        tauri::async_runtime::spawn(async move { serve(app).await });
    }
}

/// Событие «что играет» (media.rs): смена трека, пауза, остановка.
pub fn now_playing(app: &AppHandle, now: Option<&NowPlaying>) {
    let Some(saver) = app.try_state::<Saver>() else {
        return;
    };
    let mut cur = saver.now.lock().unwrap_or_else(|e| e.into_inner());
    *cur = match now {
        Some(n) => NowJson {
            playing: n.playing,
            title: n.title.clone(),
            artist: n.artist.clone(),
            artwork_url: n.artwork_url.clone(),
        },
        None => NowJson::default(),
    };
}

/// Ответ на запрос заставки: только `GET /now-playing`.
fn respond(app: &AppHandle, request_line: &str) -> String {
    let path = request_line.split_whitespace().nth(1).unwrap_or("");
    let (status, body) = if request_line.starts_with("GET ") && path == "/now-playing" {
        let now = app
            .try_state::<Saver>()
            .map(|s| s.now.lock().unwrap_or_else(|e| e.into_inner()).clone())
            .unwrap_or_default();
        ("200 OK", serde_json::to_string(&now).unwrap_or_default())
    } else {
        ("404 Not Found", "{}".to_string())
    };
    format!(
        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nCache-Control: no-store\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
}

#[cfg(target_os = "macos")]
async fn serve(app: AppHandle) {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let listener = match tokio::net::TcpListener::bind(("127.0.0.1", PORT)).await {
        Ok(l) => l,
        Err(e) => {
            log::warn!(
                "screensaver: port {PORT} is busy, the screensaver will not see what plays: {e}"
            );
            return;
        }
    };
    loop {
        let Ok((mut stream, _)) = listener.accept().await else {
            continue;
        };
        let app = app.clone();
        tauri::async_runtime::spawn(async move {
            let mut buf = [0u8; 1024];
            let read =
                tokio::time::timeout(std::time::Duration::from_secs(2), stream.read(&mut buf))
                    .await;
            let Ok(Ok(n)) = read else { return };
            let head = String::from_utf8_lossy(&buf[..n]);
            let line = head.lines().next().unwrap_or("");
            let _ = stream.write_all(respond(&app, line).as_bytes()).await;
            let _ = stream.shutdown().await;
        });
    }
}

#[cfg(target_os = "macos")]
fn paths(app: &AppHandle) -> Option<(std::path::PathBuf, std::path::PathBuf)> {
    let bundled = app.path().resource_dir().ok()?.join(SAVER);
    let installed = app
        .path()
        .home_dir()
        .ok()?
        .join("Library/Screen Savers")
        .join(SAVER);
    Some((bundled, installed))
}

fn state(app: &AppHandle) -> SaverState {
    #[cfg(target_os = "macos")]
    if let Some((bundled, installed)) = paths(app) {
        return SaverState {
            available: bundled.join("Contents/Info.plist").is_file(),
            installed: installed.join("Contents/Info.plist").is_file(),
        };
    }
    let _ = app;
    SaverState {
        available: false,
        installed: false,
    }
}

#[tauri::command]
pub fn screensaver_state(app: AppHandle) -> SaverState {
    state(&app)
}

/// Скопировать заставку в `~/Library/Screen Savers` (поверх прежней) и открыть настройки заставки.
#[tauri::command]
pub fn screensaver_install(app: AppHandle) -> ShellResult<SaverState> {
    #[cfg(target_os = "macos")]
    {
        use crate::error::ShellError;
        let Some((bundled, installed)) = paths(&app) else {
            return Err(ShellError::Invalid("нет папки приложения".into()));
        };
        if !bundled.join("Contents/Info.plist").is_file() {
            return Err(ShellError::Invalid("в этой сборке нет заставки".into()));
        }
        if let Some(dir) = installed.parent() {
            std::fs::create_dir_all(dir).map_err(ShellError::internal)?;
        }
        if installed.exists() {
            std::fs::remove_dir_all(&installed).map_err(ShellError::internal)?;
        }
        // ditto сохраняет подпись и атрибуты пакета
        let status = std::process::Command::new("/usr/bin/ditto")
            .arg(&bundled)
            .arg(&installed)
            .status()
            .map_err(ShellError::internal)?;
        if !status.success() {
            return Err(ShellError::Invalid(
                "не удалось скопировать заставку".into(),
            ));
        }
        log::info!("screensaver installed to {}", installed.display());
        let _ = std::process::Command::new("/usr/bin/open")
            .arg("x-apple.systempreferences:com.apple.ScreenSaver-Settings.extension")
            .status();
    }
    Ok(state(&app))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn now_json_is_shared_with_swift() {
        let n = NowJson {
            playing: true,
            title: "LOVESONGS".into(),
            artist: "дора".into(),
            artwork_url: Some("https://wave.daly.rest/api/images/albums/1/500.jpg".into()),
        };
        assert_eq!(
            serde_json::to_value(&n).unwrap(),
            serde_json::json!({
                "playing": true,
                "title": "LOVESONGS",
                "artist": "дора",
                "artworkUrl": "https://wave.daly.rest/api/images/albums/1/500.jpg",
            })
        );
        assert_eq!(
            serde_json::to_value(NowJson::default()).unwrap(),
            serde_json::json!({ "playing": false, "title": "", "artist": "" })
        );
    }
}
