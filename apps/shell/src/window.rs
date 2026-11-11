//! Главное окно: создание, свои кнопки окна для Windows, события для интерфейса,
//! закрытие в трей и запрет уходить со своих страниц внутри WebView.

use std::sync::Mutex;

use serde::Serialize;
use tauri::{
    AppHandle, Emitter, Manager, Theme, WebviewWindow, WebviewWindowBuilder, Window, WindowEvent,
};
use tauri_plugin_opener::OpenerExt;
use tauri_plugin_window_state::{AppHandleExt, StateFlags};
use url::Url;

use crate::{
    MAIN,
    error::{ShellError, ShellResult},
    platform::{self, Rect},
};

/// Что окно помнит между запусками. Видимость не храним: окно, закрытое в трей,
/// при следующем запуске всё равно должно открыться.
pub const SAVED_STATE: StateFlags = StateFlags::SIZE
    .union(StateFlags::POSITION)
    .union(StateFlags::MAXIMIZED);

pub const EVENT_MAXIMIZED: &str = "window:maximized";
pub const EVENT_FULLSCREEN: &str = "window:fullscreen";
pub const EVENT_FOCUS: &str = "window:focus";

/// Последнее отправленное интерфейсу состояние: события уходят только при изменении.
#[derive(Default)]
pub struct WindowFlags {
    maximized: bool,
    fullscreen: bool,
}

/// Есть ли под окном системный фон.
pub struct Backdrop(pub &'static str);

#[derive(Clone, Serialize)]
struct Maximized {
    maximized: bool,
}

#[derive(Clone, Serialize)]
struct Fullscreen {
    fullscreen: bool,
}

#[derive(Clone, Serialize)]
struct Focus {
    focused: bool,
}

pub fn create_main(app: &AppHandle) -> tauri::Result<WebviewWindow> {
    let config = app
        .config()
        .app
        .windows
        .iter()
        .find(|w| w.label == MAIN)
        .cloned()
        .ok_or(tauri::Error::WindowNotFound)?;

    let backdrop = expected_backdrop();
    let info = platform::initial(app, backdrop);
    let dev_url = app.config().build.dev_url.clone();
    let opener = app.clone();
    let window = WebviewWindowBuilder::from_config(app, &config)?
        .initialization_script(platform::init_script(&info))
        .on_navigation(move |url| {
            if is_app_url(url, dev_url.as_ref()) {
                return true;
            }
            // Внешние ссылки — в системный браузер: в окне приложения живёт только интерфейс
            if matches!(url.scheme(), "http" | "https" | "mailto")
                && let Err(e) = opener.opener().open_url(url.as_str(), None::<&str>)
            {
                log::warn!("could not open {url} externally: {e}");
            }
            false
        })
        .build()?;

    let backdrop = apply_backdrop(&window, backdrop);
    app.manage(Backdrop(backdrop));
    app.manage(Mutex::new(WindowFlags {
        maximized: window.is_maximized().unwrap_or(false),
        fullscreen: window.is_fullscreen().unwrap_or(false),
    }));
    window.show()?;
    window.set_focus()?;
    Ok(window)
}

fn is_app_url(url: &Url, dev_url: Option<&Url>) -> bool {
    match url.scheme() {
        // Свои страницы: tauri://localhost (macOS) и http(s)://tauri.localhost (Windows)
        "tauri" => true,
        "about" => url.as_str() == "about:blank",
        "http" | "https" if url.host_str() == Some("tauri.localhost") => true,
        _ => dev_url.is_some_and(|dev| dev.origin() == url.origin()),
    }
}

#[cfg(windows)]
fn expected_backdrop() -> &'static str {
    if crate::snap::is_windows_11() {
        "mica"
    } else {
        "none"
    }
}

#[cfg(not(windows))]
fn expected_backdrop() -> &'static str {
    "none"
}

#[cfg(windows)]
fn apply_backdrop(window: &WebviewWindow, expected: &'static str) -> &'static str {
    if expected == "mica" {
        // Тёмный или светлый оттенок Mica — по теме окна (её задаёт window_set_theme)
        match window_vibrancy::apply_mica(window, None) {
            Ok(()) => return "mica",
            Err(e) => log::warn!("Mica is unavailable: {e}"),
        }
    }
    // Без Mica окно прозрачно: подложим цвет фона интерфейса, чтобы не просвечивал рабочий стол
    let _ = window.set_background_color(Some(tauri::window::Color(8, 9, 12, 255)));
    "none"
}

#[cfg(not(windows))]
fn apply_backdrop(_window: &WebviewWindow, expected: &'static str) -> &'static str {
    expected
}

pub fn backdrop(window: &WebviewWindow) -> &'static str {
    window
        .try_state::<Backdrop>()
        .map(|b| b.0)
        .unwrap_or("none")
}

/// Показать главное окно и вывести вперёд: из трея, по ссылке dalywave://, при втором запуске.
pub fn show_main(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(MAIN) {
        if window.is_minimized().unwrap_or(false) {
            let _ = window.unminimize();
        }
        let _ = window.show();
        let _ = window.set_focus();
    }
}

pub fn on_event(window: &Window, event: &WindowEvent) {
    if window.label() != MAIN {
        return;
    }
    match event {
        // Музыка играет в WebView: окно прячется, а не закрывается. Выйти — из трея или ⌘Q
        WindowEvent::CloseRequested { api, .. } => {
            api.prevent_close();
            hide_to_tray(window);
        }
        WindowEvent::Resized(_) => report_state(window),
        WindowEvent::Focused(focused) => {
            let _ = window.emit_to(MAIN, EVENT_FOCUS, Focus { focused: *focused });
        }
        #[cfg(windows)]
        WindowEvent::ScaleFactorChanged { .. } => crate::snap::refresh(window),
        _ => {}
    }
}

/// Спрятать окно, сразу запомнив размер и место: из трея приложение часто закрывают
/// не выходом, а выключением компьютера.
fn hide_to_tray(window: &Window) {
    if let Err(e) = window.app_handle().save_window_state(SAVED_STATE) {
        log::warn!("could not save the window state: {e}");
    }
    let _ = window.hide();
}

fn report_state(window: &Window) {
    let maximized = window.is_maximized().unwrap_or(false);
    let fullscreen = window.is_fullscreen().unwrap_or(false);
    let Some(flags) = window.try_state::<Mutex<WindowFlags>>() else {
        return;
    };
    let (maximized_changed, fullscreen_changed) = {
        let mut flags = flags.lock().unwrap_or_else(|e| e.into_inner());
        let changes = (flags.maximized != maximized, flags.fullscreen != fullscreen);
        flags.maximized = maximized;
        flags.fullscreen = fullscreen;
        changes
    };
    if maximized_changed {
        let _ = window.emit_to(MAIN, EVENT_MAXIMIZED, Maximized { maximized });
    }
    if fullscreen_changed {
        let _ = window.emit_to(MAIN, EVENT_FULLSCREEN, Fullscreen { fullscreen });
    }
}

#[tauri::command]
pub fn window_minimize(window: Window) -> ShellResult<()> {
    Ok(window.minimize()?)
}

/// Возвращает новое состояние: `true` — окно развёрнуто.
#[tauri::command]
pub fn window_toggle_maximize(window: Window) -> ShellResult<bool> {
    if window.is_maximized()? {
        window.unmaximize()?;
        Ok(false)
    } else {
        window.maximize()?;
        Ok(true)
    }
}

/// Как системное закрытие: окно прячется в трей, музыка продолжает играть.
#[tauri::command]
pub fn window_close(window: Window) {
    hide_to_tray(&window);
}

#[tauri::command]
pub fn window_start_dragging(window: Window) -> ShellResult<()> {
    Ok(window.start_dragging()?)
}

#[tauri::command]
pub fn window_is_maximized(window: Window) -> ShellResult<bool> {
    Ok(window.is_maximized()?)
}

/// Тема системных частей окна (Mica, «светофор», меню, полосы прокрутки):
/// `dark`, `light` или `null` — как в системе.
#[tauri::command]
pub fn window_set_theme(window: Window, theme: Option<String>) -> ShellResult<()> {
    let theme = match theme.as_deref() {
        None => None,
        Some("dark") => Some(Theme::Dark),
        Some("light") => Some(Theme::Light),
        Some(other) => {
            return Err(ShellError::Invalid(format!(
                "тема должна быть dark, light или null, а не «{other}»"
            )));
        }
    };
    Ok(window.set_theme(theme)?)
}

/// Windows 11: где интерфейс нарисовал кнопку «развернуть» (CSS-пиксели от левого верхнего
/// угла окна). Наведение на неё показывает меню привязки окон. `null` — убрать.
/// На других системах ничего не делает.
#[tauri::command]
pub fn window_set_snap_area(window: WebviewWindow, rect: Option<Rect>) -> ShellResult<()> {
    if let Some(r) = rect {
        let valid = [r.x, r.y, r.width, r.height].iter().all(|v| v.is_finite())
            && r.width >= 0.0
            && r.height >= 0.0;
        if !valid {
            return Err(ShellError::Invalid(
                "прямоугольник кнопки задан неверно".into(),
            ));
        }
    }
    #[cfg(windows)]
    crate::snap::set_area(&window, rect)?;
    #[cfg(not(windows))]
    let _ = (window, rect);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_urls() {
        let dev = Url::parse("http://localhost:3400").unwrap();
        let ok = |u: &str| is_app_url(&Url::parse(u).unwrap(), Some(&dev));
        assert!(ok("tauri://localhost/library"));
        assert!(ok("http://tauri.localhost/"));
        assert!(ok("https://tauri.localhost/album/1"));
        assert!(ok("http://localhost:3400/search?q=a"));
        assert!(!ok("http://localhost:8400/api/auth/oidc/start"));
        assert!(!ok("https://wave.daly.rest/"));
        assert!(!ok("https://daly.rest/authorize"));
        assert!(!ok("data:text/html,<script>alert(1)</script>"));
        assert!(ok("about:blank"));
        assert!(!is_app_url(
            &Url::parse("http://localhost:3400/").unwrap(),
            None
        ));
    }
}
