//! Системные уведомления компьютеров (macOS, Windows), пока приложение открыто: уведомление сервера
//! приходит интерфейсу по WebSocket (`notification.new`), и, если окно не в фокусе, интерфейс зовёт
//! `notify_show`. Push с сервера на компьютеры нет — закрытое приложение уведомлений не получает.
//!
//! macOS — UNUserNotificationCenter (разрешение система спрашивает при первом уведомлении); щелчок
//! показывает окно и открывает ссылку `dalywave://…` уведомления, как любую ссылку dalywave://.
//! Без бандла (`cargo tauri dev` из target/) центр уведомлений недоступен — `false`.
//! Windows — всплывающее уведомление (tauri-plugin-notification); щелчок по нему ссылку не передаёт.
//! iPhone и iPad — `false`: там уведомления приходят через APNs (push.rs).

use tauri::AppHandle;

use crate::error::ShellResult;

/// Показать уведомление. `link` — `dalywave://…` для щелчка. `false` — не показано (запрещено,
/// не та платформа).
#[tauri::command]
pub async fn notify_show(
    app: AppHandle,
    id: String,
    title: String,
    body: String,
    link: Option<String>,
) -> ShellResult<bool> {
    #[cfg(target_os = "macos")]
    {
        macos::init(&app);
        if !macos::available() || !macos::allowed().await {
            return Ok(false);
        }
        Ok(macos::show(&id, &title, &body, link.as_deref()))
    }
    #[cfg(windows)]
    {
        use tauri_plugin_notification::NotificationExt;
        let _ = (id, link);
        match app.notification().builder().title(title).body(body).show() {
            Ok(()) => Ok(true),
            Err(e) => {
                log::warn!("notify: {e}");
                Ok(false)
            }
        }
    }
    #[cfg(not(any(target_os = "macos", windows)))]
    {
        let _ = (app, id, title, body, link);
        Ok(false)
    }
}

#[cfg(target_os = "macos")]
mod macos {
    use std::sync::{Mutex, Once, OnceLock};

    use block2::{DynBlock, RcBlock};
    use objc2::{
        AnyThread, define_class, msg_send,
        rc::{Allocated, Retained},
        runtime::{AnyClass, AnyObject, Bool, NSObject},
    };
    use objc2_foundation::NSString;
    use tauri::AppHandle;
    use tokio::sync::oneshot;

    #[link(name = "UserNotifications", kind = "framework")]
    unsafe extern "C" {}

    /// UNAuthorizationOptionSound | UNAuthorizationOptionAlert
    const OPTIONS: usize = (1 << 1) | (1 << 2);
    /// UNNotificationPresentationOptionBanner | List | Sound: окно не в фокусе, но приложение
    /// может быть активным — уведомление всё равно показать
    const PRESENT: usize = (1 << 4) | (1 << 3) | (1 << 1);

    static APP: OnceLock<AppHandle> = OnceLock::new();

    define_class!(
        // Делегат центра уведомлений: показ при активном приложении и щелчок.
        #[unsafe(super(NSObject))]
        #[name = "DalyWaveNotifyDelegate"]
        struct Delegate;

        impl Delegate {
            #[unsafe(method(userNotificationCenter:willPresentNotification:withCompletionHandler:))]
            fn will_present(
                &self,
                _center: *mut AnyObject,
                _notification: *mut AnyObject,
                handler: &DynBlock<dyn Fn(usize)>,
            ) {
                handler.call((PRESENT,));
            }

            #[unsafe(method(userNotificationCenter:didReceiveNotificationResponse:withCompletionHandler:))]
            fn did_receive(
                &self,
                _center: *mut AnyObject,
                response: *mut AnyObject,
                handler: &DynBlock<dyn Fn()>,
            ) {
                clicked(link_of(response));
                handler.call(());
            }
        }
    );

    /// Ссылка из `userInfo["link"]` ответа на уведомление.
    fn link_of(response: *mut AnyObject) -> Option<String> {
        if response.is_null() {
            return None;
        }
        // SAFETY: UNNotificationResponse.notification.request.content.userInfo — объекты системы
        unsafe {
            let notification: Option<Retained<NSObject>> = msg_send![&*response, notification];
            let request: Option<Retained<NSObject>> = msg_send![&*notification?, request];
            let content: Option<Retained<NSObject>> = msg_send![&*request?, content];
            let info: Option<Retained<NSObject>> = msg_send![&*content?, userInfo];
            let key = NSString::from_str("link");
            let link: Option<Retained<NSObject>> = msg_send![&*info?, objectForKey: &*key];
            let link = link?;
            let is_string: Bool = msg_send![&*link, isKindOfClass: AnyClass::get(c"NSString")?];
            if !is_string.as_bool() {
                return None;
            }
            let link: Retained<NSString> = Retained::cast_unchecked(link);
            Some(link.to_string())
        }
    }

    fn clicked(link: Option<String>) {
        let Some(app) = APP.get() else { return };
        match link.and_then(|l| url::Url::parse(&l).ok()) {
            // Ссылка сама показывает окно
            Some(url) => crate::auth::handle_urls(app, vec![url]),
            None => crate::mini::show_app(app),
        }
    }

    fn center() -> Option<Retained<NSObject>> {
        let cls = AnyClass::get(c"UNUserNotificationCenter")?;
        // SAFETY: классовый метод, любой поток; общий центр приложения (есть только у бандла — available)
        unsafe { msg_send![cls, currentNotificationCenter] }
    }

    /// Центр уведомлений есть только у приложения в бандле: без него `currentNotificationCenter`
    /// бросает исключение Objective-C.
    pub fn available() -> bool {
        let Some(cls) = AnyClass::get(c"NSBundle") else {
            return false;
        };
        // SAFETY: NSBundle.mainBundle — любой поток
        unsafe {
            let bundle: Option<Retained<NSObject>> = msg_send![cls, mainBundle];
            let Some(bundle) = bundle else { return false };
            let id: Option<Retained<NSString>> = msg_send![&*bundle, bundleIdentifier];
            let path: Option<Retained<NSString>> = msg_send![&*bundle, bundlePath];
            id.is_some() && path.is_some_and(|p| p.to_string().ends_with(".app"))
        }
    }

    /// Делегат — один раз: щелчок по уведомлению приходит ему.
    pub fn init(app: &AppHandle) {
        static ONCE: Once = Once::new();
        let _ = APP.set(app.clone());
        if !available() {
            return;
        }
        ONCE.call_once(|| {
            let Some(center) = center() else { return };
            let this: Allocated<Delegate> = Delegate::alloc();
            // SAFETY: init NSObject; центр держит делегата слабо — живёт до конца процесса
            let delegate: Retained<Delegate> = unsafe { msg_send![this, init] };
            unsafe {
                let _: () = msg_send![&*center, setDelegate: &*delegate];
            }
            std::mem::forget(delegate);
        });
    }

    /// Разрешение: при первом вызове система спрашивает, дальше отвечает сама.
    pub async fn allowed() -> bool {
        let (tx, rx) = oneshot::channel::<bool>();
        // Объекты Objective-C не переживают await: future команды должна быть Send
        if !request(tx) {
            return false;
        }
        rx.await.unwrap_or(false)
    }

    fn request(tx: oneshot::Sender<bool>) -> bool {
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

    pub fn show(id: &str, title: &str, body: &str, link: Option<&str>) -> bool {
        let Some(center) = center() else { return false };
        let (Some(content_cls), Some(request_cls), Some(sound_cls), Some(dict_cls)) = (
            AnyClass::get(c"UNMutableNotificationContent"),
            AnyClass::get(c"UNNotificationRequest"),
            AnyClass::get(c"UNNotificationSound"),
            AnyClass::get(c"NSDictionary"),
        ) else {
            return false;
        };
        // SAFETY: обычные объекты UserNotifications и Foundation: alloc + init, сеттеры свойств
        unsafe {
            let a: Allocated<NSObject> = msg_send![content_cls, alloc];
            let content: Retained<NSObject> = msg_send![a, init];
            let _: () = msg_send![&*content, setTitle: &*NSString::from_str(title)];
            let _: () = msg_send![&*content, setBody: &*NSString::from_str(body)];
            let sound: Option<Retained<NSObject>> = msg_send![sound_cls, defaultSound];
            if let Some(sound) = &sound {
                let _: () = msg_send![&*content, setSound: &**sound];
            }
            if let Some(link) = link {
                let info: Option<Retained<NSObject>> = msg_send![
                    dict_cls,
                    dictionaryWithObject: &*NSString::from_str(link),
                    forKey: &*NSString::from_str("link")
                ];
                if let Some(info) = info {
                    let _: () = msg_send![&*content, setUserInfo: &*info];
                }
            }
            // Без триггера (nil) — показать сразу
            let none_trigger: Option<&NSObject> = None;
            let request: Option<Retained<NSObject>> = msg_send![
                request_cls,
                requestWithIdentifier: &*NSString::from_str(id),
                content: &*content,
                trigger: none_trigger
            ];
            let Some(request) = request else { return false };
            let none: Option<&DynBlock<dyn Fn(*mut AnyObject)>> = None;
            let _: () =
                msg_send![&*center, addNotificationRequest: &*request, withCompletionHandler: none];
        }
        true
    }
}
