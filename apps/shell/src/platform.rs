//! Что интерфейсу нужно знать о платформе: ОС, версия, какие кнопки окна рисует он сам,
//! где лежит «светофор» macOS и есть ли под окном системный фон (Mica).

#[cfg(target_os = "ios")]
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, WebviewWindow};

use crate::settings::Settings;

/// Прямоугольник в CSS-пикселях от левого верхнего угла окна.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TitleBar {
    /// Рекомендуемая высота своей строки заголовка: на macOS «светофор» в ней по центру,
    /// на Windows — высота системных кнопок окна.
    pub height: f64,
    /// Где система рисует свои кнопки окна (только macOS); слева от них интерфейс ничего не ставит.
    pub controls: Option<Rect>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlatformInfo {
    /// `macos`, `windows`, `linux`, `ios`, `ipados`
    pub os: &'static str,
    pub arch: &'static str,
    pub app_version: String,
    /// `mac-inset` — кнопки рисует macOS внутри окна; `windows` — интерфейс рисует свои
    /// и вызывает команды окна; `none` — у окна обычная системная рамка.
    pub window_controls: &'static str,
    pub title_bar: TitleBar,
    /// `mica` — под прозрачными местами интерфейса системный фон Windows 11.
    pub backdrop: &'static str,
    /// Запущено через `cargo tauri dev` с dev-сервера интерфейса.
    pub dev: bool,
    pub server_url: String,
    pub locale: &'static str,
}

pub const OS: &str = if cfg!(target_os = "macos") {
    "macos"
} else if cfg!(windows) {
    "windows"
} else if cfg!(target_os = "ios") {
    "ios"
} else {
    "linux"
};

pub const WINDOW_CONTROLS: &str = if cfg!(target_os = "macos") {
    "mac-inset"
} else if cfg!(windows) {
    "windows"
} else {
    "none"
};

/// iPhone или iPad: узнаётся у UIKit на главном потоке при запуске.
#[cfg(target_os = "ios")]
struct Device {
    os: &'static str,
    model: String,
}

#[cfg(target_os = "ios")]
static DEVICE: OnceLock<Device> = OnceLock::new();

/// Запомнить, iPhone это или iPad. Главный поток, до создания окна.
#[cfg(target_os = "ios")]
pub fn init_device() {
    use objc2::MainThreadMarker;
    use objc2_ui_kit::{UIDevice, UIUserInterfaceIdiom};
    let Some(mtm) = MainThreadMarker::new() else {
        return;
    };
    let device = UIDevice::currentDevice(mtm);
    let pad = device.userInterfaceIdiom() == UIUserInterfaceIdiom::Pad;
    let _ = DEVICE.set(Device {
        os: if pad { "ipados" } else { "ios" },
        model: device.model().to_string(),
    });
}

/// ОС для интерфейса и сервера: на iPad — `ipados`.
pub fn os() -> &'static str {
    #[cfg(target_os = "ios")]
    if let Some(d) = DEVICE.get() {
        return d.os;
    }
    OS
}

/// iOS: «iPhone» или «iPad» — имя устройства для списка сессий на сервере.
#[cfg(target_os = "ios")]
pub fn device_model() -> String {
    DEVICE
        .get()
        .map(|d| d.model.clone())
        .unwrap_or_else(|| "iPhone".into())
}

/// «Светофор» при trafficLightPosition {18, 26} из tauri.macos.conf.json, замерено на macOS 27:
/// кнопки 14×14 через 23 пт, центр на 24 — середина строки заголовка 48 пт из макета.
#[cfg(target_os = "macos")]
const MAC_CONTROLS: Rect = Rect {
    x: 18.0,
    y: 17.0,
    width: 60.0,
    height: 14.0,
};

pub fn arch() -> &'static str {
    std::env::consts::ARCH
}

/// Сведения для первой отрисовки: попадают в `window.__DALYWAVE_SHELL__` до загрузки страницы.
pub fn initial(app: &AppHandle, backdrop: &'static str) -> PlatformInfo {
    let settings = app.state::<Settings>();
    #[cfg(target_os = "macos")]
    let title_bar = TitleBar {
        height: (MAC_CONTROLS.y * 2.0 + MAC_CONTROLS.height).round(),
        controls: Some(MAC_CONTROLS),
    };
    #[cfg(all(desktop, not(target_os = "macos")))]
    let title_bar = TitleBar {
        height: 32.0,
        controls: None,
    };
    // Телефон и планшет: окна с заголовком нет, отступы — env(safe-area-inset-*)
    #[cfg(mobile)]
    let title_bar = TitleBar {
        height: 0.0,
        controls: None,
    };
    PlatformInfo {
        os: os(),
        arch: arch(),
        app_version: app.package_info().version.to_string(),
        window_controls: WINDOW_CONTROLS,
        title_bar,
        backdrop,
        dev: tauri::is_dev(),
        server_url: settings.server_url(),
        locale: settings.locale().as_str(),
    }
}

/// То же с живыми замерами окна: положение «светофора» берётся у самого AppKit.
pub fn current(window: &WebviewWindow) -> PlatformInfo {
    let info = initial(window.app_handle(), crate::window::backdrop(window));
    #[cfg(target_os = "macos")]
    if let Some(controls) = crate::macos::traffic_lights(window) {
        return PlatformInfo {
            title_bar: TitleBar {
                height: (controls.y * 2.0 + controls.height).round(),
                controls: Some(controls),
            },
            ..info
        };
    }
    info
}

/// Скрипт до любого кода страницы: интерфейс сразу рисует свой заголовок нужной формы.
pub fn init_script(info: &PlatformInfo) -> String {
    let json = serde_json::to_string(info).unwrap_or_else(|_| "{}".into());
    format!(
        "Object.defineProperty(window, '__DALYWAVE_SHELL__', {{ value: Object.freeze({json}), enumerable: false }});"
    )
}

#[tauri::command]
pub fn platform_info(window: WebviewWindow) -> PlatformInfo {
    current(&window)
}
