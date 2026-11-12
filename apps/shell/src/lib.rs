//! Оболочка DalyWave для macOS, Windows, iPhone и iPad. Интерфейс — apps/ui (Nuxt, SPA); здесь
//! окно (на компьютерах — без стандартной рамки), звук через свой движок на Rust, вход через
//! системный браузер с токеном в связке ключей, «Сейчас играет» и пульт системы, трей, ссылки
//! dalywave://, автообновление и (на компьютерах) загрузчик музыки для сервера. Договор с интерфейсом —
//! BRIDGE.md.

mod audio;
mod auth;
// Статус «Слушает DalyWave» в Discord (Rich Presence)
#[cfg(desktop)]
mod discord;
#[cfg(desktop)]
mod downloader;
mod error;
mod haptics;
#[cfg(target_os = "ios")]
mod ios;
// Ссылки dalywave:// для интерфейса, в том числе «Поделиться → DalyWave»
mod links;
#[cfg(target_os = "macos")]
mod macos;
mod media;
#[cfg(desktop)]
mod mini;
// Системные уведомления компьютеров, пока приложение открыто (уведомления сервера по WebSocket)
mod notify;
mod offline;
mod platform;
mod player;
// Push iPhone и iPad (APNs): разрешение, токен для сервера, значок, нажатие на уведомление
mod push;
mod recognize;
mod settings;
// Siri и «Команды» (iPhone, iPad): App Intents и SiriKit в Swift зовут сюда
mod siri;
#[cfg(windows)]
mod snap;
#[cfg(desktop)]
mod tray;
#[cfg(desktop)]
mod updates;
// Виджеты iPhone и iPad (WidgetKit): снимок «что играет», недавнее и сочетания волны для Swift
mod widgets;
// Apple Watch (iPhone): ключ для часов через WatchConnectivity — Swift зовёт сюда
mod watch;
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
/// Метка окна мини-плеера (mini.rs): события плеера идут и ему
pub const MINI: &str = "mini";

pub const EVENT_SERVER: &str = "server:changed";

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default();
    // Первым: второй запуск передаёт ссылку dalywave:// первому и показывает его окно
    #[cfg(desktop)]
    let builder = builder.plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
        mini::show_app(app);
    }));
    let builder = builder
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
                // «skipped 4 bytes of junk» на каждом MP4 — норма разбора, не предупреждение
                .level_for("symphonia_core", log::LevelFilter::Error)
                .level_for("symphonia_format_isomp4", log::LevelFilter::Warn)
                .level_for("ureq", log::LevelFilter::Warn)
                .level_for("rustls", log::LevelFilter::Warn)
                .max_file_size(2_000_000)
                .rotation_strategy(tauri_plugin_log::RotationStrategy::KeepSome(3))
                .timezone_strategy(tauri_plugin_log::TimezoneStrategy::UseLocal)
                .build(),
        )
        .plugin(tauri_plugin_opener::init());
    #[cfg(desktop)]
    let builder = builder
        .plugin(
            tauri_plugin_window_state::Builder::new()
                .with_state_flags(window::SAVED_STATE)
                .build(),
        )
        .plugin(tauri_plugin_updater::Builder::new().build());
    // Загрузчик музыки — sidecar (downloader.rs); права в capabilities не нужны: запускает Rust
    #[cfg(desktop)]
    let builder = builder.plugin(tauri_plugin_shell::init());
    // Windows: всплывающие уведомления, пока приложение открыто (notify.rs; права для JS не нужны)
    #[cfg(windows)]
    let builder = builder.plugin(tauri_plugin_notification::init());
    // «Поделиться» кладёт ссылку в буфер обмена (только запись — права в capabilities/main.json)
    #[cfg(desktop)]
    let builder = builder.plugin(tauri_plugin_clipboard_manager::init());
    let app = builder
        .setup(|app| {
            let handle = app.handle();
            #[cfg(target_os = "ios")]
            platform::init_device();
            app.manage(Settings::load(handle));
            app.manage(auth::Auth::new(&app.package_info().version.to_string()));
            app.manage(media::Media::default());
            widgets::init(handle);
            links::init(handle);
            #[cfg(desktop)]
            discord::init(handle);
            #[cfg(desktop)]
            app.manage(updates::Updates::default());
            // Звук — сразу, в фоновом потоке: к первой команде интерфейса движок готов
            player::init(handle);
            audio::init(handle);
            // Локальная база и загрузки: синхронизация и докачка идут в фоне
            offline::init(handle);

            let main = window::create_main(handle)?;
            // До трея: меню сразу показывает состояние загрузчика
            #[cfg(desktop)]
            downloader::init(handle);
            #[cfg(desktop)]
            tray::create(handle)?;
            #[cfg(windows)]
            media::init_window(handle, &main);
            #[cfg(not(windows))]
            let _ = main;

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

            #[cfg(desktop)]
            updates::schedule(handle);
            log::info!(
                "DalyWave {} started on {}, server {}",
                app.package_info().version,
                platform::os(),
                app.state::<Settings>().server_url()
            );
            Ok(())
        })
        .on_window_event(window::on_event)
        .invoke_handler(tauri::generate_handler![
            platform::platform_info,
            #[cfg(desktop)]
            window::window_minimize,
            #[cfg(desktop)]
            window::window_toggle_maximize,
            #[cfg(desktop)]
            window::window_close,
            #[cfg(desktop)]
            window::window_start_dragging,
            #[cfg(desktop)]
            window::window_is_maximized,
            #[cfg(desktop)]
            window::window_set_theme,
            #[cfg(desktop)]
            window::window_set_snap_area,
            auth::auth_login,
            auth::auth_cancel,
            auth::auth_logout,
            auth::auth_token,
            auth::auth_session,
            player::player_load,
            player::player_set_upcoming,
            player::player_play,
            player::player_pause,
            player::player_toggle,
            player::player_seek,
            player::player_next,
            player::player_previous,
            player::player_stop,
            player::player_set_volume,
            player::player_set_transition,
            player::player_set_normalize,
            player::player_set_levels,
            player::player_state,
            player::player_set_eq,
            player::player_eq,
            player::player_set_sleep,
            player::player_sleep,
            player::player_set_modes,
            player::player_now,
            player::player_remote,
            #[cfg(desktop)]
            mini::mini_open,
            #[cfg(desktop)]
            mini::mini_close,
            #[cfg(desktop)]
            mini::mini_toggle,
            offline::offline_status,
            offline::offline_set_settings,
            offline::offline_sync,
            offline::offline_read,
            offline::offline_pin,
            offline::offline_unpin,
            offline::offline_clear,
            offline::offline_downloads,
            offline::offline_reserve,
            offline::offline_source,
            offline::offline_wave_next,
            offline::offline_wave_react,
            offline::offline_played,
            haptics::haptic,
            notify::notify_show,
            push::push_state,
            push::push_register,
            push::push_set_badge,
            recognize::recognize_listen,
            recognize::recognize_cancel,
            recognize::recognize_take_pending,
            siri::siri_take,
            audio::audio_outputs,
            audio::audio_set_output,
            audio::audio_show_picker,
            open_external,
            links::links_take,
            #[cfg(desktop)]
            discord::discord_state,
            #[cfg(desktop)]
            discord::discord_set_enabled,
            server_url_get,
            server_url_set,
            shell_set_locale,
            #[cfg(desktop)]
            updates::update_check,
            #[cfg(desktop)]
            updates::update_install,
            #[cfg(desktop)]
            updates::update_state,
            #[cfg(desktop)]
            downloader::downloader_state,
            #[cfg(desktop)]
            downloader::downloader_set_enabled,
            #[cfg(desktop)]
            downloader::downloader_set_paused,
            #[cfg(desktop)]
            downloader::downloader_recreate_token,
            #[cfg(desktop)]
            downloader::downloader_open_log,
        ])
        .build(tauri::generate_context!())
        .expect("failed to build DalyWave");
    // Быстрое действие «Распознать» на значке iPhone: до запуска цикла приложения
    recognize::install(app.handle());
    // Siri: SiriKit будит приложение `application:handleIntent:…` — метод нужен до запуска цикла
    siri::install(app.handle());
    // Push: токен APNs приходит делегату приложения — методы нужны до запуска цикла
    push::install(app.handle());
    // Apple Watch: часы могут разбудить iPhone в фоне просьбой о ключе — приложение нужно сразу
    #[cfg(target_os = "ios")]
    watch::install(app.handle());

    app.run(|app, event| {
        match event {
            // macOS: щелчок по значку в Dock, когда окно закрыто в трей
            #[cfg(target_os = "macos")]
            tauri::RunEvent::Reopen {
                has_visible_windows: false,
                ..
            } => mini::show_app(app),
            // Выход — ⌘Q, «Выйти» в трее, перезапуск после обновления: загрузчик возвращает
            // задание серверу и останавливается, окно тем временем уже не видно
            #[cfg(desktop)]
            tauri::RunEvent::Exit => {
                if let Some(main) = app.get_webview_window(MAIN) {
                    let _ = main.hide();
                }
                downloader::shutdown(app);
                // macOS: готовое обновление ставится при обычном выходе (updates.rs)
                #[cfg(target_os = "macos")]
                updates::install_on_exit(app);
            }
            _ => {}
        }
        #[cfg(mobile)]
        let _ = app;
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
    app.state::<player::Player>().forget_token();
    // Загрузчик переходит на новый сервер со своим токеном
    #[cfg(desktop)]
    downloader::changed(&app);
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
    #[cfg(desktop)]
    tray::refresh(&app);
    Ok(())
}
