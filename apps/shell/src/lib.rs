//! Оболочка DalyWave для macOS и Windows. Интерфейс — apps/ui (Nuxt, SPA); здесь окно без
//! стандартной рамки, вход через системный браузер с токеном в связке ключей, трей,
//! медиаклавиши и системная плашка «Сейчас играет», ссылки dalywave:// и автообновление.
//! Договор с интерфейсом — BRIDGE.md.

mod auth;
mod error;
#[cfg(target_os = "macos")]
mod macos;
mod media;
mod platform;
mod settings;
#[cfg(windows)]
mod snap;
mod tray;
mod updates;
mod window;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_deep_link::DeepLinkExt;
use tauri_plugin_opener::OpenerExt;

use crate::{
    error::{ShellError, ShellResult},
    settings::{Locale, Settings},
};

/// Метка главного окна
pub const MAIN: &str = "main";

pub const EVENT_SERVER: &str = "server:changed";

pub fn run() {
    let app = tauri::Builder::default()
        // Первым: второй запуск передаёт ссылку dalywave:// первому и показывает его окно
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            window::show_main(app);
        }))
        .plugin(tauri_plugin_deep_link::init())
        .plugin(
            tauri_plugin_log::Builder::new()
                .level(if tauri::is_dev() {
                    log::LevelFilter::Debug
                } else {
                    log::LevelFilter::Info
                })
                .level_for("tao", log::LevelFilter::Warn)
                .level_for("keyring_core", log::LevelFilter::Warn)
                .level_for("reqwest", log::LevelFilter::Warn)
                .level_for("hyper_util", log::LevelFilter::Warn)
                .max_file_size(2_000_000)
                .rotation_strategy(tauri_plugin_log::RotationStrategy::KeepSome(3))
                .timezone_strategy(tauri_plugin_log::TimezoneStrategy::UseLocal)
                .build(),
        )
        .plugin(tauri_plugin_opener::init())
        .plugin(
            tauri_plugin_window_state::Builder::new()
                .with_state_flags(window::SAVED_STATE)
                .build(),
        )
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(|app| {
            let handle = app.handle();
            app.manage(Settings::load(handle));
            app.manage(auth::Auth::new(&app.package_info().version.to_string()));
            app.manage(media::Media::default());
            app.manage(updates::Updates::default());

            let main = window::create_main(handle)?;
            tray::create(handle)?;
            media::init(handle, &main);

            let links = handle.clone();
            app.deep_link()
                .on_open_url(move |event| auth::handle_urls(&links, event.urls()));
            // Приложение запустили ссылкой: она уже пришла до подписки
            if let Ok(Some(urls)) = app.deep_link().get_current() {
                auth::handle_urls(handle, urls);
            }
            // Установщик регистрирует схему сам; в разработке — сборка из target/
            #[cfg(any(windows, target_os = "linux"))]
            if tauri::is_dev()
                && let Err(e) = app.deep_link().register_all()
            {
                log::warn!("could not register dalywave:// links: {e}");
            }

            updates::schedule(handle);
            log::info!(
                "DalyWave {} started, server {}",
                app.package_info().version,
                app.state::<Settings>().server_url()
            );
            Ok(())
        })
        .on_window_event(window::on_event)
        .invoke_handler(tauri::generate_handler![
            platform::platform_info,
            window::window_minimize,
            window::window_toggle_maximize,
            window::window_close,
            window::window_start_dragging,
            window::window_is_maximized,
            window::window_set_theme,
            window::window_set_snap_area,
            auth::auth_login,
            auth::auth_cancel,
            auth::auth_logout,
            auth::auth_token,
            auth::auth_session,
            media::now_playing_update,
            open_external,
            server_url_get,
            server_url_set,
            shell_set_locale,
            updates::update_check,
            updates::update_install,
        ])
        .build(tauri::generate_context!())
        .expect("failed to build DalyWave");

    app.run(|app, event| {
        // macOS: щелчок по значку в Dock, когда окно закрыто в трей
        #[cfg(target_os = "macos")]
        if let tauri::RunEvent::Reopen {
            has_visible_windows: false,
            ..
        } = event
        {
            window::show_main(app);
        }
        #[cfg(not(target_os = "macos"))]
        let _ = (app, event);
    });
}

/// Открыть ссылку в системном браузере (http, https, mailto).
#[tauri::command]
fn open_external(app: AppHandle, url: String) -> ShellResult<()> {
    let parsed =
        url::Url::parse(&url).map_err(|_| ShellError::Invalid(format!("не ссылка: «{url}»")))?;
    if !matches!(parsed.scheme(), "http" | "https" | "mailto") {
        return Err(ShellError::Invalid(format!(
            "открываются только http, https и mailto, а не {}",
            parsed.scheme()
        )));
    }
    app.opener()
        .open_url(parsed.as_str(), None::<&str>)
        .map_err(ShellError::internal)
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ServerChanged {
    server_url: String,
}

#[tauri::command]
fn server_url_get(app: AppHandle) -> String {
    app.state::<Settings>().server_url()
}

/// `null` — вернуть адрес по умолчанию. Вход на каждом сервере свой.
#[tauri::command]
fn server_url_set(app: AppHandle, url: Option<String>) -> ShellResult<String> {
    let server_url = app.state::<Settings>().set_server_url(url.as_deref())?;
    let _ = app.emit_to(
        MAIN,
        EVENT_SERVER,
        ServerChanged {
            server_url: server_url.clone(),
        },
    );
    Ok(server_url)
}

/// Язык системных частей: меню в трее и строке меню.
#[tauri::command]
fn shell_set_locale(app: AppHandle, locale: Locale) -> ShellResult<()> {
    app.state::<Settings>().set_locale(locale)?;
    tray::refresh(&app);
    Ok(())
}
