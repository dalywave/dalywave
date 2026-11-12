//! Push-уведомления iPhone и iPad (APNs): разрешение, токен устройства, число на значке, нажатие.
//!
//! Токен сервер получает от интерфейса: интерфейс зовёт `push_register` (после входа — мягким экраном
//! или из настроек «Уведомления»; при первом запуске разрешение не спрашивается) и отправляет ответ
//! в `POST /api/notifications/push`. Токен APNs стоит брать при каждом запуске: система может его
//! сменить, а `push_register(prompt: false)` при выданном разрешении отвечает без окна.
//!
//! Токен приходит делегату приложения: `application:didRegisterForRemoteNotificationsWithDeviceToken:`
//! добавляется делегату tao здесь (как быстрое действие в recognize.rs). Нажатие на уведомление ловит
//! Swift (`gen/apple/Sources/dalywave-shell/PushLink.swift` — делегат UNUserNotificationCenter, его
//! назначает main.mm до запуска) и зовёт `dalywave_push_open` со ссылкой `dalywave://…` из
//! полезной нагрузки: дальше она идёт, как любая ссылка dalywave:// (`auth::handle_urls`).
//!
//! Среда APNs: отладочная сборка (профиль разработчика, `aps-environment = development`) и симулятор
//! получают токены песочницы; TestFlight и App Store — боевые. Сервер шлёт по среде подписки.
//! На компьютерах push нет: `supported: false`, токена нет.
#![cfg_attr(not(target_os = "ios"), allow(dead_code))]

use serde::Serialize;
use tauri::AppHandle;

use crate::error::ShellResult;

/// Ответ системы на просьбу о разрешении.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Permission {
    /// Ещё не спрашивали.
    Default,
    Granted,
    Denied,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PushState {
    /// Есть ли на этой платформе push с сервера.
    pub supported: bool,
    pub permission: Permission,
}

/// Среда APNs, в которой выдан токен.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Environment {
    Sandbox,
    Production,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PushToken {
    /// Токен устройства, hex в нижнем регистре.
    pub token: String,
    pub environment: Environment,
}

/// `UNAuthorizationStatus` → разрешение для интерфейса.
fn permission(status: isize) -> Permission {
    match status {
        // notDetermined
        0 => Permission::Default,
        // denied
        1 => Permission::Denied,
        // authorized, provisional, ephemeral
        _ => Permission::Granted,
    }
}

/// Токен устройства как его ждёт APNs: hex в нижнем регистре.
pub fn token_hex(bytes: &[u8]) -> String {
    use std::fmt::Write;
    bytes
        .iter()
        .fold(String::with_capacity(bytes.len() * 2), |mut s, b| {
            let _ = write!(s, "{b:02x}");
            s
        })
}

/// Среда по профилю подготовки из бандла (`embedded.mobileprovision`): у сборки из Xcode профиль
/// разработчика с `aps-environment = development` — песочница; у TestFlight профиль App Store с
/// `production`, а у приложения из App Store профиля нет вовсе — боевая среда. Профиль подписан (CMS),
/// но plist внутри лежит открытым текстом.
pub fn environment_from_profile(profile: Option<&[u8]>) -> Environment {
    let Some(bytes) = profile else {
        return Environment::Production;
    };
    let text = String::from_utf8_lossy(bytes);
    let Some(at) = text.find("<key>aps-environment</key>") else {
        return Environment::Production;
    };
    let rest = &text[at..];
    let value = rest
        .find("<string>")
        .and_then(|s| {
            let v = &rest[s + "<string>".len()..];
            v.find("</string>").map(|e| v[..e].trim())
        })
        .unwrap_or_default();
    if value == "development" {
        Environment::Sandbox
    } else {
        Environment::Production
    }
}

/// Разрешение и есть ли push на этой платформе.
#[tauri::command]
pub async fn push_state() -> ShellResult<PushState> {
    #[cfg(target_os = "ios")]
    {
        Ok(PushState {
            supported: true,
            permission: permission(ios::status().await),
        })
    }
    #[cfg(not(target_os = "ios"))]
    {
        Ok(PushState {
            supported: false,
            permission: Permission::Granted,
        })
    }
}

/// Токен APNs. `prompt` — можно спросить разрешение (интерфейс сам выбирает момент); без него,
/// пока не спрашивали, — `null` без окна. Запрещено, ошибка или нет ответа 10 с — `null`.
#[tauri::command]
pub async fn push_register(app: AppHandle, prompt: bool) -> ShellResult<Option<PushToken>> {
    #[cfg(target_os = "ios")]
    {
        Ok(ios::register(&app, prompt).await)
    }
    #[cfg(not(target_os = "ios"))]
    {
        let _ = (app, prompt);
        Ok(None)
    }
}

/// Число непрочитанных на значке приложения.
#[tauri::command]
pub fn push_set_badge(app: AppHandle, count: u32) -> ShellResult<()> {
    #[cfg(target_os = "ios")]
    ios::set_badge(&app, count);
    #[cfg(not(target_os = "ios"))]
    let _ = (app, count);
    Ok(())
}

static APP: std::sync::OnceLock<AppHandle> = std::sync::OnceLock::new();

/// При запуске, до `App::run`: делегату tao — методы токена APNs.
pub fn install(app: &AppHandle) {
    let _ = APP.set(app.clone());
    #[cfg(target_os = "ios")]
    ios::install();
}

/// Нажатие на уведомление: ссылка `dalywave://…` — как любая ссылка dalywave://. Приложение могли
/// запустить самим нажатием: ссылку отдаём, когда оно поднимется (главный поток не держим).
pub fn open(link: &str) {
    let Ok(url) = url::Url::parse(link) else {
        log::warn!("push: not a link: {link:?}");
        return;
    };
    if url.scheme() != "dalywave" {
        log::warn!("push: unexpected link scheme {}", url.scheme());
        return;
    }
    std::thread::spawn(move || {
        for _ in 0..200 {
            if let Some(app) = APP.get()
                && tauri::Manager::try_state::<crate::links::Links>(app).is_some()
            {
                crate::auth::handle_urls(app, vec![url]);
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        log::warn!("push: app is not ready, link dropped");
    });
}

/// Swift (PushLink.swift): нажали на уведомление со ссылкой `link`.
///
/// # Safety
/// `link` — NUL-строка UTF-8 или NULL.
#[cfg(target_os = "ios")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dalywave_push_open(link: *const std::ffi::c_char) {
    if link.is_null() {
        return;
    }
    // SAFETY: договор вызова — см. выше
    let link = unsafe { std::ffi::CStr::from_ptr(link) };
    if let Ok(link) = link.to_str() {
        open(link);
    }
}

#[cfg(target_os = "ios")]
mod ios {
    use std::ffi::{CStr, c_char, c_void};
    use std::sync::Mutex;
    use std::time::Duration;

    use block2::RcBlock;
    use objc2::rc::Retained;
    use objc2::runtime::{AnyClass, AnyObject, Bool, Imp, NSObject, Sel};
    use objc2::{ffi, msg_send, sel};
    use objc2_foundation::NSString;
    use tauri::AppHandle;
    use tokio::sync::oneshot;

    use super::{Environment, PushToken, environment_from_profile, token_hex};

    #[link(name = "UserNotifications", kind = "framework")]
    unsafe extern "C" {}

    /// UNAuthorizationOptionBadge | Sound | Alert
    const OPTIONS: usize = (1 << 0) | (1 << 1) | (1 << 2);
    /// Столько ждём токен от системы.
    const TOKEN_WAIT: Duration = Duration::from_secs(10);

    /// Ждущие токена: ответ приходит делегату приложения.
    static WAITERS: Mutex<Vec<oneshot::Sender<Option<String>>>> = Mutex::new(Vec::new());
    /// Последний полученный токен: если система задержит ответ, отдаём его.
    static LAST: Mutex<Option<String>> = Mutex::new(None);

    fn center() -> Option<Retained<NSObject>> {
        let cls = AnyClass::get(c"UNUserNotificationCenter")?;
        // SAFETY: классовый метод, любой поток; общий центр приложения
        unsafe { msg_send![cls, currentNotificationCenter] }
    }

    /// `UNAuthorizationStatus` приложения.
    pub async fn status() -> isize {
        let (tx, rx) = oneshot::channel::<isize>();
        // Объекты Objective-C не переживают await: future команды должна быть Send
        if !ask_status(tx) {
            return 0;
        }
        rx.await.unwrap_or(0)
    }

    fn ask_status(tx: oneshot::Sender<isize>) -> bool {
        let Some(center) = center() else {
            return false;
        };
        let tx = Mutex::new(Some(tx));
        let handler = RcBlock::new(move |settings: *mut AnyObject| {
            let status: isize = if settings.is_null() {
                0
            } else {
                // SAFETY: UNNotificationSettings.authorizationStatus
                unsafe { msg_send![&*settings, authorizationStatus] }
            };
            if let Some(tx) = tx.lock().unwrap_or_else(|e| e.into_inner()).take() {
                let _ = tx.send(status);
            }
        });
        // SAFETY: ответ приходит на произвольной очереди; блок копируется системой
        let _: () =
            unsafe { msg_send![&*center, getNotificationSettingsWithCompletionHandler: &*handler] };
        true
    }

    /// Разрешение: система спрашивает слушателя один раз, дальше отвечает сама.
    async fn authorize() -> bool {
        let (tx, rx) = oneshot::channel::<bool>();
        if !ask_authorization(tx) {
            return false;
        }
        rx.await.unwrap_or(false)
    }

    fn ask_authorization(tx: oneshot::Sender<bool>) -> bool {
        let Some(center) = center() else {
            return false;
        };
        let tx = Mutex::new(Some(tx));
        let handler = RcBlock::new(move |granted: Bool, _err: *mut AnyObject| {
            if let Some(tx) = tx.lock().unwrap_or_else(|e| e.into_inner()).take() {
                let _ = tx.send(granted.as_bool());
            }
        });
        // SAFETY: ответ приходит на произвольной очереди; блок копируется системой
        let _: () = unsafe {
            msg_send![&*center, requestAuthorizationWithOptions: OPTIONS, completionHandler: &*handler]
        };
        true
    }

    pub async fn register(app: &AppHandle, prompt: bool) -> Option<PushToken> {
        match status().await {
            // Не спрашивали: без разрешения интерфейса окно не показываем
            0 if !prompt => return None,
            0 => {
                if !authorize().await {
                    log::info!("push: permission not granted");
                    return None;
                }
            }
            1 => return None,
            _ => {}
        }
        let (tx, rx) = oneshot::channel();
        WAITERS.lock().unwrap_or_else(|e| e.into_inner()).push(tx);
        // registerForRemoteNotifications — только с главного потока
        if let Err(e) = app.run_on_main_thread(|| {
            let Some(cls) = AnyClass::get(c"UIApplication") else {
                return;
            };
            // SAFETY: главный поток; общий объект приложения
            unsafe {
                let ui: *mut AnyObject = msg_send![cls, sharedApplication];
                if !ui.is_null() {
                    let _: () = msg_send![&*ui, registerForRemoteNotifications];
                }
            }
        }) {
            log::warn!("push: could not register: {e}");
            return None;
        }
        let token = match tokio::time::timeout(TOKEN_WAIT, rx).await {
            Ok(Ok(token)) => token,
            _ => {
                log::warn!("push: no token from the system in time");
                LAST.lock().unwrap_or_else(|e| e.into_inner()).clone()
            }
        }?;
        Some(PushToken {
            token,
            environment: environment(),
        })
    }

    /// Симулятор получает токены песочницы; на устройстве среду говорит профиль в бандле.
    fn environment() -> Environment {
        if cfg!(target_abi = "sim") {
            return Environment::Sandbox;
        }
        let profile = bundle_path()
            .map(|p| std::path::Path::new(&p).join("embedded.mobileprovision"))
            .and_then(|p| std::fs::read(p).ok());
        environment_from_profile(profile.as_deref())
    }

    fn bundle_path() -> Option<String> {
        let cls = AnyClass::get(c"NSBundle")?;
        // SAFETY: NSBundle.mainBundle.bundlePath; любой поток
        unsafe {
            let bundle: Option<Retained<NSObject>> = msg_send![cls, mainBundle];
            let path: Option<Retained<NSString>> = msg_send![&*bundle?, bundlePath];
            Some(path?.to_string())
        }
    }

    fn deliver(token: Option<String>) {
        if let Some(t) = &token {
            *LAST.lock().unwrap_or_else(|e| e.into_inner()) = Some(t.clone());
        }
        for tx in WAITERS.lock().unwrap_or_else(|e| e.into_inner()).drain(..) {
            let _ = tx.send(token.clone());
        }
    }

    type Handler = unsafe extern "C-unwind" fn(*mut AnyObject, Sel, *mut AnyObject, *mut AnyObject);

    /// `application:didRegisterForRemoteNotificationsWithDeviceToken:`
    unsafe extern "C-unwind" fn did_register(
        _this: *mut AnyObject,
        _sel: Sel,
        _app: *mut AnyObject,
        data: *mut AnyObject,
    ) {
        if data.is_null() {
            return deliver(None);
        }
        // SAFETY: система передаёт живой NSData
        let bytes = unsafe {
            let len: usize = msg_send![&*data, length];
            let ptr: *const c_void = msg_send![&*data, bytes];
            if ptr.is_null() || len == 0 {
                Vec::new()
            } else {
                std::slice::from_raw_parts(ptr.cast::<u8>(), len).to_vec()
            }
        };
        if bytes.is_empty() {
            return deliver(None);
        }
        log::info!("push: device token received ({} bytes)", bytes.len());
        deliver(Some(token_hex(&bytes)));
    }

    /// `application:didFailToRegisterForRemoteNotificationsWithError:`
    unsafe extern "C-unwind" fn did_fail(
        _this: *mut AnyObject,
        _sel: Sel,
        _app: *mut AnyObject,
        error: *mut AnyObject,
    ) {
        let text = if error.is_null() {
            String::new()
        } else {
            // SAFETY: NSError.localizedDescription
            unsafe {
                let d: Option<Retained<NSString>> = msg_send![&*error, localizedDescription];
                d.map(|d| d.to_string()).unwrap_or_default()
            }
        };
        log::warn!("push: registration failed: {text}");
        deliver(None);
    }

    fn add(class: &AnyClass, sel: Sel, imp: Imp, types: &CStr) {
        // SAFETY: класс зарегистрирован, сигнатура совпадает с types
        let added = unsafe {
            ffi::class_addMethod(
                (class as *const AnyClass).cast_mut(),
                sel,
                imp,
                types.as_ptr() as *const c_char,
            )
        };
        if !added.as_bool() {
            log::warn!("push: {} already has {sel}", class.name().to_string_lossy());
        }
    }

    pub fn install() {
        let Some(delegate) = AnyClass::get(c"AppDelegate") else {
            log::warn!("push: no AppDelegate class");
            return;
        };
        // SAFETY: приведение указателя на функцию к Imp; вызывает её только рантайм ObjC
        let registered: Imp = unsafe { std::mem::transmute::<Handler, Imp>(did_register) };
        add(
            delegate,
            sel!(application:didRegisterForRemoteNotificationsWithDeviceToken:),
            registered,
            c"v@:@@",
        );
        let failed: Imp = unsafe { std::mem::transmute::<Handler, Imp>(did_fail) };
        add(
            delegate,
            sel!(application:didFailToRegisterForRemoteNotificationsWithError:),
            failed,
            c"v@:@@",
        );
    }

    pub fn set_badge(app: &AppHandle, count: u32) {
        let count = count as isize;
        if let Some(center) = center() {
            // SAFETY: проверка селектора и вызов метода центра (iOS 16+); любой поток
            unsafe {
                let has: Bool = msg_send![&*center, respondsToSelector: sel!(setBadgeCount:withCompletionHandler:)];
                if has.as_bool() {
                    let none: Option<&block2::DynBlock<dyn Fn(*mut AnyObject)>> = None;
                    let _: () =
                        msg_send![&*center, setBadgeCount: count, withCompletionHandler: none];
                    return;
                }
            }
        }
        // iOS 15: свойство приложения — с главного потока
        let _ = app.run_on_main_thread(move || {
            let Some(cls) = AnyClass::get(c"UIApplication") else {
                return;
            };
            // SAFETY: главный поток; общий объект приложения
            unsafe {
                let ui: *mut AnyObject = msg_send![cls, sharedApplication];
                if !ui.is_null() {
                    let _: () = msg_send![&*ui, setApplicationIconBadgeNumber: count];
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_is_lowercase_hex() {
        assert_eq!(token_hex(&[0x00, 0xab, 0x0f, 0xff]), "00ab0fff");
        assert_eq!(token_hex(&[]), "");
    }

    #[test]
    fn environment_follows_the_profile() {
        assert_eq!(environment_from_profile(None), Environment::Production);
        let dev = b"\x30\x82junk<?xml version=\"1.0\"?><plist><dict><key>Entitlements</key><dict>\
            <key>aps-environment</key>\n\t\t<string>development</string>\
            <key>application-identifier</key><string>Y5KR5JPCRG.rest.daly.wave</string></dict></dict></plist>\x00\xff";
        assert_eq!(environment_from_profile(Some(dev)), Environment::Sandbox);
        let prod =
            b"<plist><dict><key>aps-environment</key><string>production</string></dict></plist>";
        assert_eq!(
            environment_from_profile(Some(prod)),
            Environment::Production
        );
        // Профиль без push (например, ad hoc без возможности) — боевая по умолчанию
        let none = b"<plist><dict><key>get-task-allow</key><true/></dict></plist>";
        assert_eq!(
            environment_from_profile(Some(none)),
            Environment::Production
        );
    }

    #[test]
    fn permission_names() {
        assert_eq!(permission(0), Permission::Default);
        assert_eq!(permission(1), Permission::Denied);
        assert_eq!(permission(2), Permission::Granted);
        assert_eq!(permission(3), Permission::Granted);
        assert_eq!(
            serde_json::to_value(PushState {
                supported: true,
                permission: Permission::Default
            })
            .unwrap(),
            serde_json::json!({ "supported": true, "permission": "default" })
        );
        assert_eq!(
            serde_json::to_value(Environment::Sandbox).unwrap(),
            "sandbox"
        );
    }
}
