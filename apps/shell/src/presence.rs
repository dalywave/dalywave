//! Человек у компьютера или отошёл: macOS сообщает о блокировке экрана распределёнными уведомлениями
//! `com.apple.screenIsLocked` и `com.apple.screenIsUnlocked`. Интерфейсу — событие `system:lock`
//! `{ locked }`, а он — серверу сигналом подхвата (stores/handoff.ts, crates/server/src/handoff.rs):
//! экран заблокирован давно, а к телефону подключили наушники — «Продолжить здесь?».
//!
//! Простой системы ([`system_idle_ms`]): сколько не трогали мышь и клавиатуру. Пока здесь играет звук,
//! интерфейс раз в ~30 с сообщает его серверу (`connect.presence`): выбрали музыку на другом устройстве,
//! а здесь давно никого — играть там (crates/server/src/choice.rs).

#![cfg_attr(not(target_os = "macos"), allow(dead_code))]

use serde::Serialize;

pub const EVENT_LOCK: &str = "system:lock";

#[derive(Clone, Copy, Serialize)]
pub struct Lock {
    pub locked: bool,
}

#[cfg(target_os = "macos")]
pub fn init(app: &tauri::AppHandle) {
    use std::ptr::NonNull;

    use block2::RcBlock;
    use objc2_foundation::{NSDistributedNotificationCenter, NSNotification, NSString};
    use tauri::Emitter;

    let center = NSDistributedNotificationCenter::defaultCenter();
    for (name, locked) in [
        ("com.apple.screenIsLocked", true),
        ("com.apple.screenIsUnlocked", false),
    ] {
        let app = app.clone();
        let block = RcBlock::new(move |_n: NonNull<NSNotification>| {
            log::info!("screen {}", if locked { "locked" } else { "unlocked" });
            let _ = app.emit_to(crate::MAIN, EVENT_LOCK, Lock { locked });
        });
        let name = NSString::from_str(name);
        // SAFETY: имя и блок живут до конца вызова (центр копирует блок); очередь не задана — блок
        // вызывается там, где пришло уведомление (главный цикл), а в нём только emit
        let observer = unsafe {
            center.addObserverForName_object_queue_usingBlock(Some(&name), None, None, &block)
        };
        // Наблюдатель нужен всё время работы приложения
        std::mem::forget(observer);
    }
}

#[cfg(not(target_os = "macos"))]
pub fn init(_app: &tauri::AppHandle) {}

/// Сколько миллисекунд во всей системе не было ввода (мышь, клавиатура, трекпад); `None` — система
/// этого не сообщает (iPhone, iPad: там пользуются самим приложением, интерфейс знает это сам).
#[tauri::command]
pub fn system_idle_ms() -> Option<u64> {
    idle_ms()
}

#[cfg(target_os = "macos")]
fn idle_ms() -> Option<u64> {
    /// `kCGEventSourceStateCombinedSessionState`: ввод всех источников сеанса.
    const COMBINED_SESSION: i32 = 0;
    /// `kCGAnyInputEventType`
    const ANY_INPUT: u32 = u32::MAX;
    #[link(name = "CoreGraphics", kind = "framework")]
    unsafe extern "C" {
        fn CGEventSourceSecondsSinceLastEventType(state: i32, event_type: u32) -> f64;
    }
    // SAFETY: функция без указателей, только читает время последнего события сеанса
    let secs = unsafe { CGEventSourceSecondsSinceLastEventType(COMBINED_SESSION, ANY_INPUT) };
    (secs.is_finite() && secs >= 0.0).then_some((secs * 1000.0) as u64)
}

#[cfg(windows)]
fn idle_ms() -> Option<u64> {
    use windows_sys::Win32::{
        System::SystemInformation::GetTickCount,
        UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO},
    };
    let mut info = LASTINPUTINFO {
        cbSize: size_of::<LASTINPUTINFO>() as u32,
        dwTime: 0,
    };
    // SAFETY: структура своя, размер указан; функция только заполняет её
    if unsafe { GetLastInputInfo(&mut info) } == 0 {
        return None;
    }
    // Оба счётчика — миллисекунды с запуска системы, переполняются раз в 49 дней: разность по модулю
    // SAFETY: без аргументов
    let now = unsafe { GetTickCount() };
    Some(u64::from(now.wrapping_sub(info.dwTime)))
}

#[cfg(not(any(target_os = "macos", windows)))]
fn idle_ms() -> Option<u64> {
    None
}
