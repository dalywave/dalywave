//! Человек у компьютера или отошёл: macOS сообщает о блокировке экрана распределёнными уведомлениями
//! `com.apple.screenIsLocked` и `com.apple.screenIsUnlocked`. Интерфейсу — событие `system:lock`
//! `{ locked }`, а он — серверу сигналом подхвата (stores/handoff.ts, crates/server/src/handoff.rs):
//! экран заблокирован давно, а к телефону подключили наушники — «Продолжить здесь?».

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
