//! Значок в строке меню macOS и в трее Windows: что играет, пауза, следующий и предыдущий
//! трек, «Показать DalyWave», выход. Команды плеера выполняет движок (player.rs); что он не
//! может сам (например, «играть» без очереди), уходит интерфейсу как `media:command`.

use tauri::{
    AppHandle, Manager, Wry,
    image::Image,
    menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent},
};

use crate::{
    media::{Command, Media, NowPlaying, Source},
    player,
    settings::{Locale, Settings},
    window,
};

const TRAY: &str = "main";

struct Texts {
    nothing: &'static str,
    play: &'static str,
    pause: &'static str,
    next: &'static str,
    previous: &'static str,
    show: &'static str,
    quit: &'static str,
}

const RU: Texts = Texts {
    nothing: "Ничего не играет",
    play: "Играть",
    pause: "Пауза",
    next: "Следующий трек",
    previous: "Предыдущий трек",
    show: "Показать DalyWave",
    quit: "Выйти из DalyWave",
};

const EN: Texts = Texts {
    nothing: "Nothing playing",
    play: "Play",
    pause: "Pause",
    next: "Next Track",
    previous: "Previous Track",
    show: "Show DalyWave",
    quit: "Quit DalyWave",
};

fn texts(locale: Locale) -> &'static Texts {
    match locale {
        Locale::Ru => &RU,
        Locale::En => &EN,
    }
}

/// Пункты меню, у которых меняется текст.
pub struct TrayItems {
    now: MenuItem<Wry>,
    toggle: MenuItem<Wry>,
    next: MenuItem<Wry>,
    previous: MenuItem<Wry>,
    show: MenuItem<Wry>,
    quit: MenuItem<Wry>,
}

pub fn create(app: &AppHandle) -> tauri::Result<TrayIcon> {
    let t = texts(app.state::<Settings>().locale());
    let none = None::<&str>;
    let items = TrayItems {
        now: MenuItem::with_id(app, "now", t.nothing, false, none)?,
        toggle: MenuItem::with_id(app, "toggle", t.play, true, none)?,
        next: MenuItem::with_id(app, "next", t.next, true, none)?,
        previous: MenuItem::with_id(app, "previous", t.previous, true, none)?,
        show: MenuItem::with_id(app, "show", t.show, true, none)?,
        quit: MenuItem::with_id(app, "quit", t.quit, true, none)?,
    };
    let menu = Menu::with_items(
        app,
        &[
            &items.now,
            &PredefinedMenuItem::separator(app)?,
            &items.toggle,
            &items.next,
            &items.previous,
            &PredefinedMenuItem::separator(app)?,
            &items.show,
            &PredefinedMenuItem::separator(app)?,
            &items.quit,
        ],
    )?;
    app.manage(items);

    // macOS: шаблон — система сама красит знак под светлую и тёмную строку меню
    #[cfg(target_os = "macos")]
    let icon = Image::from_bytes(include_bytes!("../icons/tray-template.png"))?;
    #[cfg(not(target_os = "macos"))]
    let icon = Image::from_bytes(include_bytes!("../icons/tray-windows.png"))?;

    TrayIconBuilder::with_id(TRAY)
        .icon(icon)
        .icon_as_template(cfg!(target_os = "macos"))
        .tooltip("DalyWave")
        .menu(&menu)
        // В строке меню macOS щелчок открывает меню; в трее Windows — показывает окно
        .show_menu_on_left_click(cfg!(target_os = "macos"))
        .on_menu_event(on_menu)
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
                && !cfg!(target_os = "macos")
            {
                window::show_main(tray.app_handle());
            }
        })
        .build(app)
}

fn on_menu(app: &AppHandle, event: MenuEvent) {
    match event.id().as_ref() {
        "toggle" => player::remote(app, Command::Toggle, Source::Tray),
        "next" => player::remote(app, Command::Next, Source::Tray),
        "previous" => player::remote(app, Command::Previous, Source::Tray),
        "show" => window::show_main(app),
        "quit" => app.exit(0),
        _ => {}
    }
}

/// Строка «что играет» в меню: обрезается, чтобы меню не растягивалось на пол-экрана.
fn now_line(now: &NowPlaying) -> String {
    const MAX: usize = 48;
    let line = format!("{} — {}", now.artist, now.title);
    if line.chars().count() <= MAX {
        return line;
    }
    let mut cut: String = line.chars().take(MAX - 1).collect();
    cut.push('…');
    cut
}

/// Обновить тексты: после смены трека, паузы или языка.
pub fn refresh(app: &AppHandle) {
    let Some(items) = app.try_state::<TrayItems>() else {
        return;
    };
    let t = texts(app.state::<Settings>().locale());
    let now = app.state::<Media>().current();
    let playing = now.as_ref().is_some_and(|n| n.playing);
    let line = now.as_ref().map(now_line);
    let _ = items.now.set_text(line.as_deref().unwrap_or(t.nothing));
    let _ = items
        .toggle
        .set_text(if playing { t.pause } else { t.play });
    let _ = items.next.set_text(t.next);
    let _ = items.previous.set_text(t.previous);
    let _ = items.show.set_text(t.show);
    let _ = items.quit.set_text(t.quit);
    if let Some(tray) = app.tray_by_id(TRAY) {
        let tooltip = line.map_or_else(|| "DalyWave".to_string(), |l| format!("DalyWave — {l}"));
        let _ = tray.set_tooltip(Some(tooltip));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn long_lines_are_cut() {
        let now = NowPlaying {
            title: "Очень длинное название трека, которое не помещается".into(),
            artist: "Parker the Bandit".into(),
            album: None,
            artwork_url: None,
            duration_ms: None,
            position_ms: 0,
            playing: true,
        };
        let line = now_line(&now);
        assert_eq!(line.chars().count(), 48);
        assert!(line.ends_with('…'));
    }
}
