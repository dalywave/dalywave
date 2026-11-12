//! Локальные уведомления iPhone и iPad (UNUserNotificationCenter): «Итоги {год} готовы». Разрешение
//! система спрашивает один раз — при первом уведомлении, которое правда есть о чём отправить; дальше
//! ответ помнит она сама. Пуш-сервера нет: уведомление ставит само приложение, пока работает.
//! На компьютерах — ничего (`false`).

use tauri::AppHandle;

use crate::error::ShellResult;

/// Поставить уведомление `id` на время `at_ms` (мс Unix; прошедшее или `None` — сразу). Повтор с тем же
/// `id` заменяет прежнее. `false` — уведомлений нет (не iOS или слушатель запретил).
#[tauri::command]
pub async fn notify_schedule(
    app: AppHandle,
    id: String,
    title: String,
    body: String,
    at_ms: Option<i64>,
) -> ShellResult<bool> {
    let _ = &app;
    #[cfg(target_os = "ios")]
    {
        if !ios::allowed().await {
            return Ok(false);
        }
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_millis() as i64);
        let delay = at_ms
            .map(|at| (at - now) as f64 / 1000.0)
            .filter(|s| *s >= 1.0);
        ios::schedule(&id, &title, &body, delay);
        Ok(true)
    }
    #[cfg(not(target_os = "ios"))]
    {
        let _ = (id, title, body, at_ms);
        Ok(false)
    }
}

/// Убрать ещё не показанное уведомление `id` (итоги уже посмотрели).
#[tauri::command]
pub fn notify_cancel(id: String) -> ShellResult<()> {
    #[cfg(target_os = "ios")]
    ios::cancel(&id);
    #[cfg(not(target_os = "ios"))]
    let _ = id;
    Ok(())
}

#[cfg(target_os = "ios")]
mod ios {
    use block2::{DynBlock, RcBlock};
    use objc2::{
        msg_send,
        rc::{Allocated, Retained},
        runtime::{AnyClass, AnyObject, Bool, NSObject},
    };
    use objc2_foundation::NSString;
    use tokio::sync::oneshot;

    // Класс UNUserNotificationCenter живёт в UserNotifications: без ссылки фреймворк не загружен
    #[link(name = "UserNotifications", kind = "framework")]
    unsafe extern "C" {}

    /// UNAuthorizationOptionSound | UNAuthorizationOptionAlert
    const OPTIONS: usize = (1 << 1) | (1 << 2);

    fn center() -> Option<Retained<NSObject>> {
        let cls = AnyClass::get(c"UNUserNotificationCenter")?;
        // SAFETY: классовый метод, любой поток; общий центр приложения
        unsafe { msg_send![cls, currentNotificationCenter] }
    }

    /// Разрешение на уведомления: при первом вызове система спрашивает слушателя, дальше отвечает сама.
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
        let tx = std::sync::Mutex::new(Some(tx));
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

    pub fn schedule(id: &str, title: &str, body: &str, delay_s: Option<f64>) {
        let Some(center) = center() else { return };
        let (Some(content_cls), Some(request_cls), Some(sound_cls)) = (
            AnyClass::get(c"UNMutableNotificationContent"),
            AnyClass::get(c"UNNotificationRequest"),
            AnyClass::get(c"UNNotificationSound"),
        ) else {
            return;
        };
        // SAFETY: обычные объекты UserNotifications: alloc + init, сеттеры свойств; любой поток
        unsafe {
            let a: Allocated<NSObject> = msg_send![content_cls, alloc];
            let content: Retained<NSObject> = msg_send![a, init];
            let _: () = msg_send![&*content, setTitle: &*NSString::from_str(title)];
            let _: () = msg_send![&*content, setBody: &*NSString::from_str(body)];
            let sound: Option<Retained<NSObject>> = msg_send![sound_cls, defaultSound];
            if let Some(sound) = &sound {
                let _: () = msg_send![&*content, setSound: &**sound];
            }
            let trigger: Option<Retained<NSObject>> = match delay_s {
                Some(s) => AnyClass::get(c"UNTimeIntervalNotificationTrigger")
                    .and_then(|cls| msg_send![cls, triggerWithTimeInterval: s, repeats: Bool::NO]),
                None => None,
            };
            // Без триггера (nil) — показать сразу
            let request: Option<Retained<NSObject>> = msg_send![
                request_cls,
                requestWithIdentifier: &*NSString::from_str(id),
                content: &*content,
                trigger: trigger.as_deref()
            ];
            if let Some(request) = request {
                let none: Option<&DynBlock<dyn Fn(*mut AnyObject)>> = None;
                let _: () = msg_send![&*center, addNotificationRequest: &*request, withCompletionHandler: none];
            }
        }
    }

    pub fn cancel(id: &str) {
        let Some(center) = center() else { return };
        let Some(array) = AnyClass::get(c"NSArray") else {
            return;
        };
        // SAFETY: массив из одной строки и метод центра; любой поток
        unsafe {
            let ids: Option<Retained<NSObject>> =
                msg_send![array, arrayWithObject: &*NSString::from_str(id)];
            if let Some(ids) = ids {
                let _: () =
                    msg_send![&*center, removePendingNotificationRequestsWithIdentifiers: &*ids];
            }
        }
    }
}
