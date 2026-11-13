//! Фокусирование iOS (Focus Filters): в «Настройках» → «Фокусирование» → режим → «Фильтры фокусирования»
//! → DalyWave можно выбрать сочетание волны («Работа», «Дорога», «Ночь»…). Включился фокус — система
//! вызывает `SetFocusFilterIntent` приложения (Swift: `gen/apple/Sources/dalywave-shell/DalyWaveFocus.swift`),
//! выключился — тот же вызов без сочетания.
//!
//! Swift запоминает выбор в UserDefaults (система может вызвать фильтр, когда приложение не запущено) и
//! зовёт сюда `dalywave_focus_changed`; отсюда интерфейсу — событие `focus:wave`. При запуске интерфейс
//! спрашивает текущее командой `focus_wave` — Rust читает его у Swift (`+[DalyWaveFocus current]`).
//! Что делать с сочетанием (и делать ли — настройка «Фокусирование» в приложении), решает интерфейс.
#![cfg_attr(not(target_os = "ios"), allow(dead_code))]

use std::sync::OnceLock;

use serde::{Deserialize, Serialize};
use tauri::AppHandle;

pub const EVENT_WAVE: &str = "focus:wave";

/// Сочетание волны, выбранное в фильтре фокуса. JSON — общий со Swift.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FocusWave {
    pub id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mood: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub activity: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub character: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
}

static APP: OnceLock<AppHandle> = OnceLock::new();

pub fn install(app: &AppHandle) {
    let _ = APP.set(app.clone());
}

/// Разобрать JSON от Swift: пустая строка или `null` — фокус выключился.
pub fn parse(json: &str) -> Option<FocusWave> {
    let json = json.trim();
    if json.is_empty() || json == "null" {
        return None;
    }
    serde_json::from_str(json)
        .inspect_err(|e| log::warn!("focus: unreadable filter: {e}"))
        .ok()
}

/// Фокус сменился (Swift, `SetFocusFilterIntent.perform`). 0 — принято, 3 — приложение ещё не запустилось
/// (выбор всё равно сохранён в Swift и придёт командой `focus_wave`).
///
/// # Safety
/// `json` — NUL-строка UTF-8 или NULL.
#[cfg(target_os = "ios")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dalywave_focus_changed(json: *const std::ffi::c_char) -> i32 {
    let text = if json.is_null() {
        String::new()
    } else {
        // SAFETY: договор вызова — см. выше
        unsafe { std::ffi::CStr::from_ptr(json) }
            .to_string_lossy()
            .into_owned()
    };
    let wave = parse(&text);
    log::info!(
        "focus: {}",
        wave.as_ref().map_or("off", |w| w.name.as_str())
    );
    let Some(app) = APP.get() else {
        return 3;
    };
    let _ = tauri::Emitter::emit_to(app, crate::MAIN, EVENT_WAVE, wave);
    0
}

/// Сочетание текущего фокуса; `null` — фокус без фильтра DalyWave (или не iPhone и не iPad).
#[tauri::command]
pub async fn focus_wave() -> Option<FocusWave> {
    swift::current().as_deref().and_then(parse)
}

#[cfg(target_os = "ios")]
mod swift {
    use objc2::msg_send;
    use objc2::rc::{Retained, autoreleasepool};
    use objc2::runtime::AnyClass;
    use objc2_foundation::NSString;

    /// `+[DalyWaveFocus current]` — JSON сохранённого выбора или nil.
    pub fn current() -> Option<String> {
        let class = AnyClass::get(c"DalyWaveFocus")?;
        autoreleasepool(|_| {
            // SAFETY: метод класса без аргументов, возвращает NSString или nil
            let s: Option<Retained<NSString>> = unsafe { msg_send![class, current] };
            s.map(|s| s.to_string())
        })
    }
}

#[cfg(not(target_os = "ios"))]
mod swift {
    pub fn current() -> Option<String> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filter_json() {
        assert_eq!(parse(""), None);
        assert_eq!(parse("null"), None);
        assert_eq!(parse("{oops"), None);
        let w = parse(r#"{"id":"work","name":"Работа","activity":"work"}"#).unwrap();
        assert_eq!(w.name, "Работа");
        assert_eq!(w.activity.as_deref(), Some("work"));
        assert_eq!(w.mood, None);
        assert_eq!(
            serde_json::to_value(&w).unwrap(),
            serde_json::json!({ "id": "work", "name": "Работа", "activity": "work" })
        );
    }
}
