//! Тактильная отдача на iPhone: `haptic(kind)` из интерфейса. На компьютерах и iPad без Taptic
//! Engine — ничего (UIKit сам молчит там, где мотора нет).
//!
//! | kind | Генератор | Где в интерфейсе |
//! |---|---|---|
//! | `light` / `medium` / `heavy` / `soft` / `rigid` | UIImpactFeedbackGenerator | «играть», «следующий», смена настроения волны — light; долгое нажатие с меню — medium |
//! | `selection` | UISelectionFeedbackGenerator | шторка встала на ступень, жест закрытия перешёл порог |
//! | `success` / `warning` / `error` | UINotificationFeedbackGenerator | лайк — success |

use tauri::AppHandle;

use crate::error::{ShellError, ShellResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Impact(isize),
    Selection,
    Notification(isize),
}

fn parse(kind: &str) -> Option<Kind> {
    // UIImpactFeedbackStyle: light 0, medium 1, heavy 2, soft 3, rigid 4;
    // UINotificationFeedbackType: success 0, warning 1, error 2
    Some(match kind {
        "light" => Kind::Impact(0),
        "medium" => Kind::Impact(1),
        "heavy" => Kind::Impact(2),
        "soft" => Kind::Impact(3),
        "rigid" => Kind::Impact(4),
        "selection" => Kind::Selection,
        "success" => Kind::Notification(0),
        "warning" => Kind::Notification(1),
        "error" => Kind::Notification(2),
        _ => return None,
    })
}

#[cfg(target_os = "ios")]
mod ios {
    use std::{cell::RefCell, collections::HashMap};

    use objc2::{
        msg_send,
        rc::{Allocated, Retained},
        runtime::{AnyClass, NSObject},
    };

    use super::Kind;

    thread_local! {
        // Генераторы живут между нажатиями: первый отклик без задержки на запуск мотора
        static GENERATORS: RefCell<HashMap<(u8, isize), Retained<NSObject>>> = RefCell::new(HashMap::new());
    }

    fn generator(kind: Kind) -> Option<Retained<NSObject>> {
        let (key, class, style) = match kind {
            Kind::Impact(s) => ((0u8, s), c"UIImpactFeedbackGenerator", Some(s)),
            Kind::Selection => ((1, 0), c"UISelectionFeedbackGenerator", None),
            Kind::Notification(_) => ((2, 0), c"UINotificationFeedbackGenerator", None),
        };
        GENERATORS.with(|g| {
            if let Some(x) = g.borrow().get(&key) {
                return Some(x.clone());
            }
            let cls = AnyClass::get(class)?;
            // SAFETY: генераторы UIKit — alloc + init (initWithStyle: у удара); главный поток
            let obj: Retained<NSObject> = unsafe {
                let a: Allocated<NSObject> = msg_send![cls, alloc];
                match style {
                    Some(s) => msg_send![a, initWithStyle: s],
                    None => msg_send![a, init],
                }
            };
            g.borrow_mut().insert(key, obj.clone());
            Some(obj)
        })
    }

    /// Главный поток.
    pub fn play(kind: Kind) {
        let Some(g) = generator(kind) else { return };
        // SAFETY: методы генераторов UIKit на главном потоке
        unsafe {
            match kind {
                Kind::Impact(_) => {
                    let _: () = msg_send![&*g, impactOccurred];
                }
                Kind::Selection => {
                    let _: () = msg_send![&*g, selectionChanged];
                }
                Kind::Notification(t) => {
                    let _: () = msg_send![&*g, notificationOccurred: t];
                }
            }
            // Следующий отклик — без задержки
            let _: () = msg_send![&*g, prepare];
        }
    }
}

/// Тактильный отклик. Неизвестный `kind` — ошибка `invalid`; на компьютерах — ничего.
#[tauri::command]
pub fn haptic(app: AppHandle, kind: String) -> ShellResult<()> {
    let Some(k) = parse(&kind) else {
        return Err(ShellError::Invalid(format!("нет такого отклика: {kind}")));
    };
    #[cfg(target_os = "ios")]
    app.run_on_main_thread(move || ios::play(k))?;
    #[cfg(not(target_os = "ios"))]
    let _ = (app, k);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kinds() {
        assert_eq!(parse("light"), Some(Kind::Impact(0)));
        assert_eq!(parse("medium"), Some(Kind::Impact(1)));
        assert_eq!(parse("selection"), Some(Kind::Selection));
        assert_eq!(parse("success"), Some(Kind::Notification(0)));
        assert_eq!(parse("boom"), None);
    }
}
