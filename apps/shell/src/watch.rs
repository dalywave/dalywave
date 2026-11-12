//! Apple Watch (iPhone): часы — пульт со своей сессией устройства. Своего входа у них нет: этот
//! iPhone просит у сервера дочернюю сессию (`POST /api/auth/app/pair`, «Apple Watch» в разделе
//! «Доступ», отзывается вместе с iPhone) и передаёт её часам через WatchConnectivity.
//!
//! WatchConnectivity доступен только из Swift: `gen/apple/Sources/dalywave-shell/WatchLink.swift`
//! (WCSession поднимает main.mm до запуска приложения — часы могут разбудить iPhone в фоне).
//! Мост — C ABI, как у Siri: Swift зовёт `dalywave_watch_pair` (ключ для часов, JSON). Обратно —
//! уведомления NSNotificationCenter (вход и выход: часы узнают сразу): прямой вызов Swift из Rust
//! не слинковался бы в динамическую библиотеку оболочки.
//! Токен iPhone не покидает Rust, токен часов не пишется в журнал.
#![cfg_attr(not(target_os = "ios"), allow(dead_code))]

use std::{sync::OnceLock, time::Duration};

use serde::{Deserialize, Serialize};
use serde_json::json;
use tauri::{AppHandle, Manager};

use crate::settings::Settings;

static APP: OnceLock<AppHandle> = OnceLock::new();

/// Имена уведомлений для WatchLink.swift.
const SIGNED_IN: &str = "DalyWaveWatchSignedIn";
const SIGNED_OUT: &str = "DalyWaveWatchSignedOut";

/// При запуске, до `App::run`: просьба часов о ключе может прийти сразу после запуска.
pub fn install(app: &AppHandle) {
    let _ = APP.set(app.clone());
}

/// Вход или выход на iPhone: часы получают новый ключ или узнают, что вход закрыт.
pub fn account_changed(signed_in: bool) {
    let name = if signed_in { SIGNED_IN } else { SIGNED_OUT };
    #[cfg(target_os = "ios")]
    post(name);
    #[cfg(not(target_os = "ios"))]
    let _ = name;
}

#[cfg(target_os = "ios")]
fn post(name: &str) {
    use objc2::runtime::AnyObject;
    use objc2::{class, msg_send};
    use objc2_foundation::NSString;

    let name = NSString::from_str(name);
    // SAFETY: NSNotificationCenter потокобезопасен; имя живо до конца вызова, object — nil
    unsafe {
        let center: *mut AnyObject = msg_send![class!(NSNotificationCenter), defaultCenter];
        let _: () =
            msg_send![center, postNotificationName: &*name, object: std::ptr::null::<AnyObject>()];
    }
}

/// Что получают часы. `error`: `signedOut` — на iPhone нет входа, `offline` — нет сети или сервер
/// не ответил, `failed` — сервер отказал.
#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
struct Paired {
    #[serde(skip_serializing_if = "Option::is_none")]
    server: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    user_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<&'static str>,
}

impl Paired {
    fn error(code: &'static str) -> Self {
        Paired {
            error: Some(code),
            ..Paired::default()
        }
    }
}

#[derive(Deserialize)]
struct PairResponse {
    token: String,
    user: PairUser,
}

#[derive(Deserialize)]
struct PairUser {
    name: String,
}

/// Приложение, когда Swift зовёт нас: при запуске самими часами — подождать, пока поднимется.
fn ready_app() -> Option<AppHandle> {
    for _ in 0..200 {
        if let Some(app) = APP.get()
            && app.try_state::<Settings>().is_some()
        {
            return Some(app.clone());
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    None
}

async fn pair(app: &AppHandle) -> Paired {
    let server = app.state::<Settings>().server_url();
    let key_server = server.clone();
    let token = tauri::async_runtime::spawn_blocking(move || crate::auth::load_token(&key_server))
        .await
        .ok()
        .and_then(Result::ok)
        .flatten();
    let Some(token) = token else {
        return Paired::error("signedOut");
    };
    let version = app.package_info().version.to_string();
    let http = match reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(8))
        .timeout(Duration::from_secs(15))
        .build()
    {
        Ok(http) => http,
        Err(e) => {
            log::warn!("watch: http client: {e}");
            return Paired::error("failed");
        }
    };
    let res = http
        .post(format!("{server}/api/auth/app/pair"))
        .bearer_auth(token)
        .json(&json!({ "platform": "watchos", "appVersion": version }))
        .send()
        .await;
    let res = match res {
        Ok(res) => res,
        Err(e) => {
            log::warn!("watch: server unreachable: {e}");
            return Paired::error("offline");
        }
    };
    let status = res.status();
    if status == reqwest::StatusCode::UNAUTHORIZED {
        return Paired::error("signedOut");
    }
    if !status.is_success() {
        log::warn!("watch: server refused to pair: {status}");
        return Paired::error("failed");
    }
    match res.json::<PairResponse>().await {
        Ok(p) => {
            log::info!("watch: session issued for the paired Apple Watch");
            Paired {
                server: Some(server),
                token: Some(p.token),
                user_name: Some(p.user.name),
                error: None,
            }
        }
        Err(e) => {
            log::warn!("watch: unexpected pairing answer: {e}");
            Paired::error("failed")
        }
    }
}

fn c_string(v: &impl Serialize) -> *mut std::ffi::c_char {
    let s = serde_json::to_string(v).unwrap_or_else(|_| "{}".into());
    std::ffi::CString::new(s).map_or(std::ptr::null_mut(), std::ffi::CString::into_raw)
}

/// Ключ для часов: JSON [`Paired`]; освободить — `dalywave_watch_free`. Блокирует (сеть) —
/// Swift зовёт из фоновой очереди.
#[cfg(target_os = "ios")]
#[unsafe(no_mangle)]
pub extern "C" fn dalywave_watch_pair() -> *mut std::ffi::c_char {
    let Some(app) = ready_app() else {
        return c_string(&Paired::error("failed"));
    };
    // Без проверки «нет сети» оболочки: разбуженное часами в фоне приложение ещё не проверило связь,
    // а запрос сам скажет, что сервера нет
    c_string(&tauri::async_runtime::block_on(pair(&app)))
}

/// Освободить строку из `dalywave_watch_pair`.
///
/// # Safety
/// `ptr` — строка из `dalywave_watch_pair` (или NULL), освобождается один раз.
#[cfg(target_os = "ios")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dalywave_watch_free(ptr: *mut std::ffi::c_char) {
    if !ptr.is_null() {
        // SAFETY: строка создана CString::into_raw
        drop(unsafe { std::ffi::CString::from_raw(ptr) });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn errors_carry_no_secrets() {
        let json = serde_json::to_value(Paired::error("signedOut")).unwrap();
        assert_eq!(json, json!({ "error": "signedOut" }));
        let ok = Paired {
            server: Some("https://wave.daly.rest".into()),
            token: Some("dwd_x".into()),
            user_name: Some("Захар".into()),
            error: None,
        };
        let json = serde_json::to_value(ok).unwrap();
        assert_eq!(json["userName"], "Захар");
        assert!(json.get("error").is_none());
    }
}
