//! Ссылки dalywave:// для интерфейса (кроме возврата входа): `dalywave://share?url=…` из меню
//! «Поделиться» iPhone и macOS, `dalywave://album/{id}` и подобные. Приложение могли запустить
//! самой ссылкой — тогда событие `deep-link:open` уходит раньше, чем страница подписалась, поэтому
//! последняя ссылка ещё и лежит здесь, пока интерфейс не заберёт её `links_take`.

use std::sync::Mutex;

use tauri::{AppHandle, Manager};

#[derive(Default)]
pub struct Links(Mutex<Option<String>>);

pub fn init(app: &AppHandle) {
    app.manage(Links::default());
}

/// Запомнить ссылку до того, как интерфейс её заберёт.
pub fn remember(app: &AppHandle, url: &str) {
    if let Some(links) = app.try_state::<Links>() {
        *links.0.lock().unwrap_or_else(|e| e.into_inner()) = Some(url.to_string());
    }
}

/// Последняя необработанная ссылка (и забыть её): при запуске и на каждое `deep-link:open`.
#[tauri::command]
pub fn links_take(app: AppHandle) -> Option<String> {
    app.state::<Links>()
        .0
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .take()
}

#[cfg(test)]
mod tests {
    #[test]
    fn share_link_carries_the_url() {
        let link = url::Url::parse(
            "dalywave://share?url=https%3A%2F%2Fopen.spotify.com%2Ftrack%2F4uLU6hMCjMI75M1A2tKUQC",
        )
        .unwrap();
        assert_eq!(link.host_str(), Some("share"));
        let shared = link
            .query_pairs()
            .find(|(k, _)| k == "url")
            .map(|(_, v)| v.into_owned());
        assert_eq!(
            shared.as_deref(),
            Some("https://open.spotify.com/track/4uLU6hMCjMI75M1A2tKUQC")
        );
    }
}
