//! Мини-плеер на компьютере: маленькое окно поверх всех — обложка, название, кнопки, полоса
//! прокрутки. Это не второй плеер, а другой вид того же: звук играет движок, окно получает те же
//! события `player:*` (player.rs шлёт их и сюда) и управляет пультом (`player_remote`,
//! `player_seek`). Открывается из трея, из «⋯» панели плеера и по ⌘⇧M; главное окно на это время
//! прячется, закрытие мини-плеера возвращает его. Место окна помнит плагин window-state.

use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder};
use tauri_plugin_opener::OpenerExt;
use tauri_plugin_window_state::AppHandleExt;

use crate::{
    MAIN, MINI,
    error::{ShellError, ShellResult},
    platform, window,
};

/// Размер окна, логические пиксели: обложка 72 и две строки, кнопки, полоса прокрутки.
pub const WIDTH: f64 = 360.0;
pub const HEIGHT: f64 = 136.0;
/// Отступ от края экрана при первом открытии
const MARGIN: f64 = 24.0;

/// Открыть мини-плеер (или вывести вперёд) и спрятать главное окно.
pub fn open(app: &AppHandle) -> ShellResult<()> {
    if let Some(mini) = app.get_webview_window(MINI) {
        mini.show()?;
        mini.set_focus()?;
    } else {
        let mini = build(app)?;
        mini.show()?;
        mini.set_focus()?;
    }
    if let Some(main) = app.get_webview_window(MAIN) {
        let _ = main.hide();
    }
    log::info!("mini player: open");
    crate::tray::refresh(app);
    Ok(())
}

/// Закрыть мини-плеер: место запоминается, главное окно возвращается.
pub fn close(app: &AppHandle) {
    if let Some(mini) = app.get_webview_window(MINI) {
        log::info!("mini player: closed, back to the main window");
        if let Err(e) = app.save_window_state(window::SAVED_STATE) {
            log::warn!("could not save the mini player position: {e}");
        }
        let _ = mini.destroy();
    }
    window::show_main(app);
    crate::tray::refresh(app);
}

/// Вывести приложение вперёд (Dock, второй запуск): открыт мини-плеер — он, иначе главное окно.
/// Окно поверх всех macOS не считает видимым, и без этого щелчок по Dock вернул бы главное окно
/// рядом с мини-плеером.
pub fn show_app(app: &AppHandle) {
    match app.get_webview_window(MINI) {
        Some(mini) if mini.is_visible().unwrap_or(false) => {
            let _ = mini.set_focus();
        }
        _ => window::show_main(app),
    }
}

pub fn is_open(app: &AppHandle) -> bool {
    app.get_webview_window(MINI)
        .is_some_and(|w| w.is_visible().unwrap_or(false))
}

pub fn toggle(app: &AppHandle) -> ShellResult<()> {
    if is_open(app) {
        close(app);
        Ok(())
    } else {
        open(app)
    }
}

fn build(app: &AppHandle) -> ShellResult<WebviewWindow> {
    let main = app.get_webview_window(MAIN);
    let info = main
        .as_ref()
        .map(platform::current)
        .ok_or_else(|| ShellError::Invalid("главного окна нет".into()))?;
    // Тот же интерфейс, страница /mini; признак окна — до любого кода страницы
    let script = format!(
        "{}\nObject.defineProperty(window, '__DALYWAVE_MINI__', {{ value: true, enumerable: false }});",
        platform::init_script(&info)
    );
    let dev_url = app.config().build.dev_url.clone();
    let opener = app.clone();
    let (x, y) = first_position(main.as_ref());
    let builder = WebviewWindowBuilder::new(app, MINI, WebviewUrl::App("mini".into()))
        .title("DalyWave")
        .inner_size(WIDTH, HEIGHT)
        .position(x, y)
        .resizable(false)
        .maximizable(false)
        .minimizable(false)
        .always_on_top(true)
        .visible(false)
        .focused(true)
        .background_color(tauri::window::Color(8, 9, 12, 255))
        .initialization_script(&script)
        .on_navigation(move |url| {
            if window::is_app_url(url, dev_url.as_ref()) {
                return true;
            }
            if matches!(url.scheme(), "http" | "https")
                && let Err(e) = opener.opener().open_url(url.as_str(), None::<&str>)
            {
                log::warn!("could not open {url} externally: {e}");
            }
            false
        });
    // macOS: системные скругление и тень, «светофор» спрятан — кнопки рисует страница.
    // Windows: окно без рамки, Windows 11 сама скругляет углы
    #[cfg(target_os = "macos")]
    let builder = builder
        .title_bar_style(tauri::TitleBarStyle::Overlay)
        .hidden_title(true);
    #[cfg(not(target_os = "macos"))]
    let builder = builder.decorations(false).shadow(true);
    let mini = builder.build()?;
    #[cfg(target_os = "macos")]
    {
        let w = mini.clone();
        let _ = app.run_on_main_thread(move || crate::macos::hide_window_buttons(&w));
    }
    Ok(mini)
}

/// Первое открытие: правый верхний угол экрана главного окна (дальше — где оставили).
fn first_position(main: Option<&WebviewWindow>) -> (f64, f64) {
    let monitor = main.and_then(|m| m.current_monitor().ok().flatten());
    let Some(m) = monitor else {
        return (MARGIN, MARGIN * 2.0);
    };
    let scale = m.scale_factor();
    let area = m.work_area();
    let x = (area.position.x as f64 + area.size.width as f64) / scale - WIDTH - MARGIN;
    let y = area.position.y as f64 / scale + MARGIN;
    (x, y)
}

#[tauri::command]
pub async fn mini_open(app: AppHandle) -> ShellResult<()> {
    open(&app)
}

#[tauri::command]
pub async fn mini_close(app: AppHandle) {
    close(&app);
}

#[tauri::command]
pub async fn mini_toggle(app: AppHandle) -> ShellResult<()> {
    toggle(&app)
}
