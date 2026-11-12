//! Главное окно: создание, тема и фон, свои кнопки окна для Windows, события для интерфейса,
//! закрытие в трей и запрет уходить со своих страниц внутри WebView.
//! На iPhone и iPad окно одно и во весь экран: из этого модуля там нужны только создание и тема.

#[cfg(desktop)]
use std::sync::Mutex;

#[cfg(desktop)]
use serde::Serialize;
use tauri::{AppHandle, Manager, WebviewWindow, WebviewWindowBuilder, Window, WindowEvent};
#[cfg(desktop)]
use tauri::{Emitter, Theme, window::Color};
use tauri_plugin_opener::OpenerExt;
#[cfg(desktop)]
use tauri_plugin_window_state::{AppHandleExt, StateFlags};
use url::Url;

use crate::error::{ShellError, ShellResult};
#[cfg(desktop)]
use crate::platform::Rect;
use crate::settings::{Settings, ThemeChoice};
use crate::{MAIN, platform};

/// Что окно помнит между запусками. Видимость не храним: окно, закрытое в трей,
/// при следующем запуске всё равно должно открыться.
#[cfg(desktop)]
pub const SAVED_STATE: StateFlags = StateFlags::SIZE
    .union(StateFlags::POSITION)
    .union(StateFlags::MAXIMIZED);

#[cfg(desktop)]
pub const EVENT_MAXIMIZED: &str = "window:maximized";
#[cfg(desktop)]
pub const EVENT_FULLSCREEN: &str = "window:fullscreen";
#[cfg(desktop)]
pub const EVENT_FOCUS: &str = "window:focus";

/// Последнее отправленное интерфейсу состояние: события уходят только при изменении.
#[cfg(desktop)]
#[derive(Default)]
pub struct WindowFlags {
    maximized: bool,
    fullscreen: bool,
}

/// Есть ли под окном системный фон.
pub struct Backdrop(pub &'static str);

/// Фон под интерфейсом в тёмной теме (`--ink-1`, `#08090c`) и в светлой (`#ffffff`)
#[cfg(desktop)]
const DARK_BACKGROUND: Color = Color(8, 9, 12, 255);
#[cfg(desktop)]
const LIGHT_BACKGROUND: Color = Color(255, 255, 255, 255);

#[cfg(desktop)]
#[derive(Clone, Serialize)]
struct Maximized {
    maximized: bool,
}

#[cfg(desktop)]
#[derive(Clone, Serialize)]
struct Fullscreen {
    fullscreen: bool,
}

#[cfg(desktop)]
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
    // Сохранённая тема — до показа окна: светлая открывается сразу светлой, без тёмной вспышки
    apply_theme(&window, app.state::<Settings>().theme());
    #[cfg(desktop)]
    {
        app.manage(Mutex::new(WindowFlags {
            maximized: window.is_maximized().unwrap_or(false),
            fullscreen: window.is_fullscreen().unwrap_or(false),
        }));
        window.show()?;
        window.set_focus()?;
    }
    Ok(window)
}

pub(crate) fn is_app_url(url: &Url, dev_url: Option<&Url>) -> bool {
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
    // Без Mica окно прозрачно: фон цвета темы подложит apply_theme, чтобы не просвечивал рабочий стол
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

/// Тема окна для Tauri: `None` — как в системе.
#[cfg(desktop)]
pub fn window_theme(choice: ThemeChoice) -> Option<Theme> {
    match choice {
        ThemeChoice::Dark => Some(Theme::Dark),
        ThemeChoice::Light => Some(Theme::Light),
        ThemeChoice::System => None,
    }
}

/// Какая тема у окна на самом деле: для «как в системе» — та, что сейчас в системе.
#[cfg(desktop)]
pub fn effective_theme(window: &WebviewWindow, choice: ThemeChoice) -> Theme {
    window_theme(choice).unwrap_or_else(|| window.theme().unwrap_or(Theme::Dark))
}

#[cfg(desktop)]
pub fn background_color(theme: Theme) -> Color {
    match theme {
        Theme::Light => LIGHT_BACKGROUND,
        _ => DARK_BACKGROUND,
    }
}

/// Тема окна и фон под интерфейсом.
/// macOS: тема — внешний вид всего приложения (NSAppearance): «светофор», меню и полосы прокрутки
/// следуют ей сами; фон окна и WebView — цвета темы. Windows: тема — светлая или тёмная рамка и
/// оттенок Mica; фон — только без Mica (с ней он прозрачный).
/// iPhone и iPad: см. ios::apply_theme.
pub fn apply_theme(window: &WebviewWindow, choice: ThemeChoice) {
    #[cfg(desktop)]
    {
        if let Err(e) = window.set_theme(window_theme(choice)) {
            log::warn!("could not set the theme of {}: {e}", window.label());
        }
        paint_background(window, effective_theme(window, choice));
    }
    #[cfg(target_os = "ios")]
    crate::ios::apply_theme(window, choice);
    #[cfg(target_os = "android")]
    let _ = (window, choice);
}

/// Фон окна и WebView: виден, пока страница не нарисовалась, и при изменении размера окна.
#[cfg(desktop)]
fn paint_background(window: &WebviewWindow, theme: Theme) {
    // Под главным окном Mica: фон прозрачный, её оттенок сам следует теме окна
    if window.label() == MAIN && backdrop(window) == "mica" {
        return;
    }
    if let Err(e) = window.set_background_color(Some(background_color(theme))) {
        log::warn!("could not paint the background of {}: {e}", window.label());
    }
}

/// Система сменила тему: окнам «как в системе» — фон новой темы (рамка и Mica следуют сами).
#[cfg(desktop)]
fn follow_system_theme(window: &Window, theme: Theme) {
    let app = window.app_handle();
    if app.state::<Settings>().theme() != ThemeChoice::System {
        return;
    }
    if let Some(webview) = app.get_webview_window(window.label()) {
        paint_background(&webview, theme);
    }
}

/// Показать главное окно и вывести вперёд: из трея, по ссылке dalywave://, при втором запуске.
#[cfg(desktop)]
pub fn show_main(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(MAIN) {
        if window.is_minimized().unwrap_or(false) {
            let _ = window.unminimize();
        }
        let _ = window.show();
        let _ = window.set_focus();
    }
}

/// iPhone и iPad: окно и так на экране.
#[cfg(mobile)]
pub fn show_main(_app: &AppHandle) {}

#[cfg(mobile)]
pub fn on_event(_window: &Window, _event: &WindowEvent) {}

#[cfg(desktop)]
pub fn on_event(window: &Window, event: &WindowEvent) {
    if let WindowEvent::ThemeChanged(theme) = event {
        follow_system_theme(window, *theme);
        return;
    }
    // Мини-плеер: ⌘W и Alt+F4 закрывают его и возвращают главное окно
    if window.label() == crate::MINI {
        if let WindowEvent::CloseRequested { api, .. } = event {
            api.prevent_close();
            crate::mini::close(window.app_handle());
        }
        return;
    }
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
#[cfg(desktop)]
fn hide_to_tray(window: &Window) {
    if let Err(e) = window.app_handle().save_window_state(SAVED_STATE) {
        log::warn!("could not save the window state: {e}");
    }
    let _ = window.hide();
}

#[cfg(desktop)]
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

#[cfg(desktop)]
#[tauri::command]
pub fn window_minimize(window: Window) -> ShellResult<()> {
    Ok(window.minimize()?)
}

/// Возвращает новое состояние: `true` — окно развёрнуто.
#[cfg(desktop)]
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
#[cfg(desktop)]
#[tauri::command]
pub fn window_close(window: Window) {
    hide_to_tray(&window);
}

#[cfg(desktop)]
#[tauri::command]
pub fn window_start_dragging(window: Window) -> ShellResult<()> {
    Ok(window.start_dragging()?)
}

#[cfg(desktop)]
#[tauri::command]
pub fn window_is_maximized(window: Window) -> ShellResult<bool> {
    Ok(window.is_maximized()?)
}

/// Тема приложения: `dark`, `light` или `null` — как в системе. Применяется ко всем окнам сразу
/// (главное и мини-плеер) и запоминается: следующий запуск открывает окна уже в ней.
#[tauri::command]
pub fn window_set_theme(app: AppHandle, theme: Option<String>) -> ShellResult<()> {
    let choice = match theme.as_deref() {
        None => ThemeChoice::System,
        Some("dark") => ThemeChoice::Dark,
        Some("light") => ThemeChoice::Light,
        Some(other) => {
            return Err(ShellError::Invalid(format!(
                "тема должна быть dark, light или null, а не «{other}»"
            )));
        }
    };
    // Сначала в настройки: окно «как в системе» сверяется с ними, когда система сменит тему
    let saved = app.state::<Settings>().set_theme(choice);
    for window in app.webview_windows().values() {
        apply_theme(window, choice);
    }
    saved
}

/// Windows 11: где интерфейс нарисовал кнопку «развернуть» (CSS-пиксели от левого верхнего
/// угла окна). Наведение на неё показывает меню привязки окон. `null` — убрать.
/// На других системах ничего не делает.
#[cfg(desktop)]
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
