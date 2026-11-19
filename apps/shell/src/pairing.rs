//! Apple TV рядом (iPhone, iPad).
//!
//! 1. Список серверов — в iCloud (NSUbiquitousKeyValueStore, ключ `servers`): Apple TV на том же
//!    Apple ID предлагает «wave.daly.rest — подключиться?» без ввода адреса.
//! 2. «Подключить с iPhone»: Apple TV объявляет в локальной сети `_dalywave-pair._tcp`, iPhone
//!    спрашивает «Apple TV „Гостиная“ хочет подключиться» и по «Разрешить» передаёт адрес сервера,
//!    а код входа телевизора подтверждает сам (`POST /api/auth/tv/approve` — тот же вход по коду,
//!    что с экрана телевизора).
//!
//! Bonjour, iCloud и окно — в Swift (`gen/apple/Sources/dalywave-shell/TVPairing.swift`). Мост —
//! C ABI, как у часов: Swift зовёт `dalywave_pair_info` и `dalywave_pair_approve`; обратно —
//! уведомление `DalyWaveServersChanged`. Токен iPhone не покидает Rust.

use std::{sync::OnceLock, time::Duration};

use serde::Serialize;
use serde_json::json;
use tauri::{AppHandle, Manager};

use crate::settings::{RecentServer, Settings};

static APP: OnceLock<AppHandle> = OnceLock::new();

const SERVERS_CHANGED: &str = "DalyWaveServersChanged";

/// При запуске: Swift может спросить сразу.
pub fn install(app: &AppHandle) {
    let _ = APP.set(app.clone());
}

/// Сменился сервер или вход: Swift обновит список в iCloud и поиск Apple TV рядом.
pub fn servers_changed(_app: &AppHandle) {
    use objc2::runtime::AnyObject;
    use objc2::{class, msg_send};
    use objc2_foundation::NSString;

    let name = NSString::from_str(SERVERS_CHANGED);
    // SAFETY: NSNotificationCenter потокобезопасен; имя живо до конца вызова, object — nil
    unsafe {
        let center: *mut AnyObject = msg_send![class!(NSNotificationCenter), defaultCenter];
        let _: () =
            msg_send![center, postNotificationName: &*name, object: std::ptr::null::<AnyObject>()];
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct Info {
    /// Текущий сервер
    server: String,
    /// Вход на нём есть: только тогда iPhone ищет Apple TV рядом
    signed_in: bool,
    /// Недавние серверы — для iCloud (без токенов)
    servers: Vec<RecentServer>,
}

fn app() -> Option<AppHandle> {
    for _ in 0..100 {
        if let Some(app) = APP.get()
            && app.try_state::<Settings>().is_some()
        {
            return Some(app.clone());
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    None
}

fn token(server: &str) -> Option<String> {
    crate::auth::load_token(server).ok().flatten()
}

fn info(app: &AppHandle) -> Info {
    let settings = app.state::<Settings>();
    let server = settings.server_url();
    let mut servers = settings.recent_servers();
    // Сервер по умолчанию (из сборки) в недавние не попадает, пока его не выбрали явно
    if settings.server_chosen() && !servers.iter().any(|s| s.url == server) {
        servers.insert(
            0,
            RecentServer {
                url: server.clone(),
                name: None,
                used_at: 0,
            },
        );
    }
    Info {
        signed_in: token(&server).is_some(),
        server,
        servers,
    }
}

/// Подтвердить вход Apple TV по коду с его экрана от имени этого iPhone.
fn approve(app: &AppHandle, user_code: &str) -> serde_json::Value {
    let server = app.state::<Settings>().server_url();
    let Some(token) = token(&server) else {
        return json!({ "error": "signedOut" });
    };
    let http = app.state::<crate::auth::Auth>().http.clone();
    let res = tauri::async_runtime::block_on(async move {
        http.post(format!("{server}/api/auth/tv/approve"))
            .bearer_auth(token)
            .json(&json!({ "userCode": user_code }))
            .timeout(Duration::from_secs(15))
            .send()
            .await
    });
    match res {
        Ok(r) if r.status().is_success() => {
            log::info!("pairing: Apple TV sign-in approved");
            json!({ "ok": true })
        }
        Ok(r) if r.status() == reqwest::StatusCode::UNAUTHORIZED => json!({ "error": "signedOut" }),
        Ok(r) => {
            log::warn!("pairing: server refused the TV code: {}", r.status());
            json!({ "error": "failed" })
        }
        Err(e) => {
            log::warn!("pairing: server unreachable: {e}");
            json!({ "error": "offline" })
        }
    }
}

fn c_string(v: &impl Serialize) -> *mut std::ffi::c_char {
    let s = serde_json::to_string(v).unwrap_or_else(|_| "{}".into());
    std::ffi::CString::new(s).map_or(std::ptr::null_mut(), std::ffi::CString::into_raw)
}

/// Текущий сервер, есть ли вход и недавние серверы: JSON; освободить — `dalywave_pair_free`.
/// Читает связку ключей — Swift зовёт из фоновой очереди.
#[unsafe(no_mangle)]
pub extern "C" fn dalywave_pair_info() -> *mut std::ffi::c_char {
    match app() {
        Some(app) => c_string(&info(&app)),
        None => std::ptr::null_mut(),
    }
}

/// Подтвердить код Apple TV: `{"ok":true}` или `{"error":"signedOut"|"offline"|"failed"}`.
/// Блокирует (сеть) — Swift зовёт из фоновой очереди.
///
/// # Safety
/// `code` — строка C (UTF-8), живая до конца вызова.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dalywave_pair_approve(
    code: *const std::ffi::c_char,
) -> *mut std::ffi::c_char {
    if code.is_null() {
        return c_string(&json!({ "error": "failed" }));
    }
    // SAFETY: Swift передаёт строку C, живую до конца вызова
    let code = unsafe { std::ffi::CStr::from_ptr(code) }
        .to_string_lossy()
        .into_owned();
    match app() {
        Some(app) => c_string(&approve(&app, &code)),
        None => c_string(&json!({ "error": "failed" })),
    }
}

/// Освободить строку из `dalywave_pair_*`.
///
/// # Safety
/// `ptr` — строка из `dalywave_pair_info`/`dalywave_pair_approve` (или NULL), освобождается один раз.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dalywave_pair_free(ptr: *mut std::ffi::c_char) {
    if !ptr.is_null() {
        // SAFETY: строка создана CString::into_raw
        drop(unsafe { std::ffi::CString::from_raw(ptr) });
    }
}
