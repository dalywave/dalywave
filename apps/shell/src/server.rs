//! Адрес сервера: выбор при первом запуске, проверка «это DalyWave?», недавние серверы, смена в
//! настройках и ссылка `dalywave://server?url=…` (QR-код в админке и на странице входа сайта).
//!
//! Ссылка адрес не меняет сама: интерфейс открывает экран «Адрес сервера» с подставленным адресом,
//! человек нажимает «Подключиться». Иначе чужая ссылка могла бы незаметно увести приложение на
//! другой сервер.

use std::{sync::Mutex, time::Duration};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};
use url::Url;

use crate::{
    EVENT_SERVER, MAIN,
    auth::Auth,
    error::{ShellError, ShellResult},
    settings::{DEMO_SERVER, RecentServer, Settings, normalize_server_url},
};

/// Пришла ссылка `dalywave://server?url=…`: интерфейс забирает адрес `server_link_take`.
pub const EVENT_LINK: &str = "server:link";

#[derive(Default)]
pub struct ServerLink(Mutex<Option<String>>);

pub fn init(app: &AppHandle) {
    app.manage(ServerLink::default());
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerState {
    pub url: String,
    /// Сервер выбран; нет — интерфейс начинает с экрана «Адрес сервера»
    pub chosen: bool,
    pub recent: Vec<RecentServer>,
    /// Демо-сервер для кнопки «Попробовать демо»
    pub demo: &'static str,
}

/// Что ответил сервер по адресу: название и версия для экрана выбора.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ServerProbe {
    pub url: String,
    pub name: String,
    pub version: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Changed {
    server_url: String,
}

#[tauri::command]
pub fn server_url_get(app: AppHandle) -> String {
    app.state::<Settings>().server_url()
}

#[tauri::command]
pub fn server_state(app: AppHandle) -> ServerState {
    let settings = app.state::<Settings>();
    ServerState {
        url: settings.server_url(),
        chosen: settings.server_chosen(),
        recent: settings.recent_servers(),
        demo: DEMO_SERVER,
    }
}

/// `null` — вернуть адрес по умолчанию. Вход на каждом сервере свой: токен прежнего остаётся в
/// связке ключей, пока на нём не выйдут. `name` — название из `/api/config` для недавних.
#[tauri::command]
pub fn server_url_set(
    app: AppHandle,
    url: Option<String>,
    name: Option<String>,
) -> ShellResult<String> {
    let server_url = app
        .state::<Settings>()
        .set_server_url(url.as_deref(), name.as_deref())?;
    app.state::<crate::player::Player>().forget_token();
    // Загрузчик переходит на новый сервер со своим токеном
    #[cfg(desktop)]
    crate::downloader::changed(&app);
    #[cfg(desktop)]
    crate::trainer::changed(&app);
    // iPhone и iPad: список серверов — в iCloud для Apple TV
    #[cfg(target_os = "ios")]
    crate::pairing::servers_changed(&app);
    let _ = app.emit_to(
        MAIN,
        EVENT_SERVER,
        Changed {
            server_url: server_url.clone(),
        },
    );
    log::info!("server changed to {server_url}");
    Ok(server_url)
}

/// Убрать сервер из недавних.
#[tauri::command]
pub fn server_forget(app: AppHandle, url: String) -> ShellResult<()> {
    app.state::<Settings>().forget_server(&url)?;
    #[cfg(target_os = "ios")]
    crate::pairing::servers_changed(&app);
    Ok(())
}

/// Последняя ссылка `dalywave://server?url=…` (и забыть её): адрес из неё, уже приведённый к виду
/// `https://host`, или `null`.
#[tauri::command]
pub fn server_link_take(app: AppHandle) -> Option<String> {
    app.state::<ServerLink>()
        .0
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .take()
}

/// Ссылка `dalywave://server?url=…`. `true` — это она (обработана здесь).
pub fn handle_link(app: &AppHandle, link: &Url) -> bool {
    if link.host_str() != Some("server") {
        return false;
    }
    let Some(url) = link_target(link) else {
        log::warn!("server link without a valid url: {link}");
        return true;
    };
    log::info!("server link {url}");
    if let Some(slot) = app.try_state::<ServerLink>() {
        *slot.0.lock().unwrap_or_else(|e| e.into_inner()) = Some(url.clone());
    }
    crate::window::show_main(app);
    let _ = app.emit_to(MAIN, EVENT_LINK, url);
    true
}

/// Адрес из ссылки `dalywave://server?url=…`.
fn link_target(link: &Url) -> Option<String> {
    let raw = link
        .query_pairs()
        .find(|(k, _)| k == "url")
        .map(|(_, v)| v.into_owned())?;
    normalize_server_url(&raw).ok()
}

/// Проверить адрес: отвечает ли там DalyWave. Запрос из Rust, а не из страницы — чужой сервер мог
/// не разрешить запросы с адреса приложения (CORS), а ответ «это не DalyWave» нужен и тогда.
///
/// Коды ошибок: `invalid` — не адрес; `unreachable` — сервер не ответил (сеть, DNS, сертификат);
/// `not_dalywave` — ответил, но это не DalyWave; `down` — DalyWave, но его база недоступна.
#[tauri::command]
pub async fn server_check(app: AppHandle, url: String) -> ShellResult<ServerProbe> {
    let url = normalize_server_url(&url)?;
    let http = &app.state::<Auth>().http;
    let unreachable = |e: reqwest::Error| {
        log::info!("server check {url}: {e}");
        denied("unreachable", format!("сервер не отвечает: {e}"))
    };
    let res = http
        .get(format!("{url}/api/config"))
        .timeout(Duration::from_secs(10))
        .send()
        .await
        .map_err(unreachable)?;
    let status = res.status();
    let body = res.bytes().await.map_err(unreachable)?;
    let probe = parse_config(&url, status.as_u16(), &body)?;
    // База: /api/healthz отвечает 503, если она недоступна
    let health = http
        .get(format!("{url}/api/healthz"))
        .timeout(Duration::from_secs(10))
        .send()
        .await
        .map_err(unreachable)?;
    if health.status().as_u16() == 503 {
        return Err(denied(
            "down",
            "сервер DalyWave работает, но его база недоступна",
        ));
    }
    Ok(probe)
}

fn denied(code: &str, message: impl Into<String>) -> ShellError {
    ShellError::Denied {
        code: code.into(),
        message: message.into(),
    }
}

#[derive(Deserialize)]
struct ConfigReply {
    app: Option<String>,
    name: Option<String>,
    version: Option<String>,
    deezer: Option<bool>,
}

/// Ответ `/api/config`: у DalyWave — `app: "dalywave"` (у прежних серверов этого поля нет,
/// их узнаём по `version` и флагам возможностей).
fn parse_config(url: &str, status: u16, body: &[u8]) -> ShellResult<ServerProbe> {
    let not_dalywave = || denied("not_dalywave", format!("по адресу {url} не DalyWave"));
    if !(200..300).contains(&status) {
        return Err(not_dalywave());
    }
    let reply: ConfigReply = serde_json::from_slice(body).map_err(|_| not_dalywave())?;
    let ours = match reply.app.as_deref() {
        Some(app) => app == "dalywave",
        None => reply.version.is_some() && reply.deezer.is_some(),
    };
    if !ours {
        return Err(not_dalywave());
    }
    let host = Url::parse(url)
        .ok()
        .and_then(|u| u.host_str().map(str::to_string))
        .unwrap_or_default();
    Ok(ServerProbe {
        url: url.to_string(),
        name: reply
            .name
            .map(|n| n.trim().to_string())
            .filter(|n| !n.is_empty())
            .unwrap_or(host),
        version: reply.version.unwrap_or_default(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn server_link_carries_a_normalized_address() {
        let link =
            Url::parse("dalywave://server?url=https%3A%2F%2Fdemo.wave.daly.rest%2F").unwrap();
        assert_eq!(
            link_target(&link).as_deref(),
            Some("https://demo.wave.daly.rest")
        );
        let bare = Url::parse("dalywave://server?url=music.example.com").unwrap();
        assert_eq!(
            link_target(&bare).as_deref(),
            Some("https://music.example.com")
        );
        assert!(link_target(&Url::parse("dalywave://server").unwrap()).is_none());
        assert!(
            link_target(&Url::parse("dalywave://server?url=http%3A%2F%2Fevil.example").unwrap())
                .is_none(),
            "простой http — только для своей машины"
        );
    }

    #[test]
    fn config_reply_identifies_dalywave() {
        let ok = parse_config(
            "https://demo.wave.daly.rest",
            200,
            br#"{"app":"dalywave","name":"Demo","version":"0.1.17","deezer":false}"#,
        )
        .unwrap();
        assert_eq!(ok.name, "Demo");
        assert_eq!(ok.version, "0.1.17");
        // Старый сервер: без `app` и `name` — название по адресу
        let old = parse_config(
            "https://wave.daly.rest",
            200,
            br#"{"version":"0.1.16","deezer":true,"downloader":true}"#,
        )
        .unwrap();
        assert_eq!(old.name, "wave.daly.rest");
        let code = |r: ShellResult<ServerProbe>| r.unwrap_err().code().to_string();
        assert_eq!(
            code(parse_config("https://x", 200, b"<html>")),
            "not_dalywave"
        );
        assert_eq!(code(parse_config("https://x", 404, b"{}")), "not_dalywave");
        assert_eq!(
            code(parse_config("https://x", 200, br#"{"app":"jellyfin"}"#)),
            "not_dalywave"
        );
    }
}
