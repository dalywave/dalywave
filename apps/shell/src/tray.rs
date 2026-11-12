//! Значок в строке меню macOS и в трее Windows: что играет, пауза, следующий и предыдущий
//! трек, «Показать DalyWave», мини-плеер, выход. Команды плеера выполняет движок (player.rs); что он не
//! может сам (например, «играть» без очереди), уходит интерфейсу как `media:command`.
//! И загрузчик музыки (downloader.rs): строка состояния, пауза и «Скачивать на этом Mac» (на
//! Windows — «на этом ПК»). Скачанное обновление — пунктом «Обновление готово — Перезапустить»
//! в начале меню (updates.rs), только пока оно готово.

use std::sync::Mutex;

use tauri::menu::{CheckMenuItem, Submenu};
use tauri::{
    AppHandle, Manager, Wry,
    image::Image,
    menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent},
};

use crate::downloader::{self, Summary, TrayState};
use crate::{
    media::{Command, Media, NowPlaying, Source},
    player,
    settings::{Locale, Settings},
    updates, window,
};

const TRAY: &str = "main";
/// Строки меню длиннее обрезаются, чтобы меню не растягивалось на пол-экрана
const LINE_MAX: usize = 48;
/// Строка загрузчика вмещает «качает «Трек» — Артист · 45 %» целиком
const DL_LINE_MAX: usize = 64;

struct Texts {
    nothing: &'static str,
    play: &'static str,
    pause: &'static str,
    next: &'static str,
    previous: &'static str,
    show: &'static str,
    mini: &'static str,
    recognize: &'static str,
    mini_close: &'static str,
    quit: &'static str,
    update: &'static str,
}

const RU: Texts = Texts {
    nothing: "Ничего не играет",
    play: "Играть",
    pause: "Пауза",
    next: "Следующий трек",
    previous: "Предыдущий трек",
    show: "Показать DalyWave",
    mini: "Мини-плеер",
    recognize: "Распознать песню",
    mini_close: "Закрыть мини-плеер",
    quit: "Выйти из DalyWave",
    update: "Обновление готово — Перезапустить",
};

const EN: Texts = Texts {
    nothing: "Nothing playing",
    play: "Play",
    pause: "Pause",
    next: "Next Track",
    previous: "Previous Track",
    show: "Show DalyWave",
    mini: "Mini Player",
    recognize: "Recognize Song",
    mini_close: "Close Mini Player",
    quit: "Quit DalyWave",
    update: "Update Ready — Restart",
};

fn texts(locale: Locale) -> &'static Texts {
    match locale {
        Locale::Ru => &RU,
        Locale::En => &EN,
    }
}

/// Загрузчик: строка состояния (заголовок подменю) и его пункты.
struct DlTexts {
    prefix: &'static str,
    /// `{track}`, `{artist}`, `{percent}`
    working: &'static str,
    working_unknown: &'static str,
    finishing: &'static str,
    idle: &'static str,
    paused: &'static str,
    offline: &'static str,
    starting: &'static str,
    resting: &'static str,
    stopping: &'static str,
    restarting: &'static str,
    token: &'static str,
    broken: &'static str,
    disabled: &'static str,
    signed_out: &'static str,
    forbidden: &'static str,
    pause: &'static str,
    resume: &'static str,
    enabled: &'static str,
}

const DL_RU: DlTexts = DlTexts {
    prefix: "Загрузчик: ",
    working: "качает «{track}» — {artist}",
    working_unknown: "качает трек",
    finishing: "доделывает «{track}», потом пауза",
    idle: "ждёт заданий",
    paused: "на паузе",
    offline: "нет связи с сервером",
    starting: "запускается…",
    resting: "отдыхает после проверки YouTube",
    stopping: "останавливается…",
    restarting: "перезапускается…",
    token: "сервер не принял токен",
    broken: "установка повреждена",
    disabled: "выключен",
    signed_out: "нужен вход",
    forbidden: "доступен администратору",
    pause: "Пауза",
    resume: "Продолжить",
    enabled: if cfg!(target_os = "macos") {
        "Скачивать на этом Mac"
    } else {
        "Скачивать на этом ПК"
    },
};

const DL_EN: DlTexts = DlTexts {
    prefix: "Downloader: ",
    working: "downloading “{track}” — {artist}",
    working_unknown: "downloading a track",
    finishing: "finishing “{track}”, then pausing",
    idle: "waiting for jobs",
    paused: "paused",
    offline: "can't reach the server",
    starting: "starting…",
    resting: "resting after a YouTube check",
    stopping: "stopping…",
    restarting: "restarting…",
    token: "token rejected by the server",
    broken: "installation is damaged",
    disabled: "off",
    signed_out: "sign-in required",
    forbidden: "available to admins",
    pause: "Pause",
    resume: "Resume",
    enabled: if cfg!(target_os = "macos") {
        "Download on This Mac"
    } else {
        "Download on This PC"
    },
};

fn dl_texts(locale: Locale) -> &'static DlTexts {
    match locale {
        Locale::Ru => &DL_RU,
        Locale::En => &DL_EN,
    }
}

/// Пункты меню, у которых меняется текст.
pub struct TrayItems {
    now: MenuItem<Wry>,
    toggle: MenuItem<Wry>,
    next: MenuItem<Wry>,
    previous: MenuItem<Wry>,
    show: MenuItem<Wry>,
    mini: MenuItem<Wry>,
    /// «Распознать песню» — только macOS (ShazamKit, recognize.rs)
    recognize: MenuItem<Wry>,
    quit: MenuItem<Wry>,
    downloader: DlItems,
    update: UpdateItems,
}

/// Готовое обновление: пункт и черта под ним встают в начало меню, только пока оно готово.
struct UpdateItems {
    menu: Menu<Wry>,
    item: MenuItem<Wry>,
    separator: PredefinedMenuItem<Wry>,
    shown: Mutex<bool>,
}

struct DlItems {
    /// Подменю, заголовок — строка состояния загрузчика
    menu: Submenu<Wry>,
    toggle: MenuItem<Wry>,
    enabled: CheckMenuItem<Wry>,
    /// Что уже показано: состояние приходит до 4 раз в секунду, меню трогаем только при перемене
    shown: Mutex<Option<(String, String, bool)>>,
}

pub fn create(app: &AppHandle) -> tauri::Result<TrayIcon> {
    let t = texts(app.state::<Settings>().locale());
    let none = None::<&str>;
    let now = MenuItem::with_id(app, "now", t.nothing, false, none)?;
    let toggle = MenuItem::with_id(app, "toggle", t.play, true, none)?;
    let next = MenuItem::with_id(app, "next", t.next, true, none)?;
    let previous = MenuItem::with_id(app, "previous", t.previous, true, none)?;
    let show = MenuItem::with_id(app, "show", t.show, true, none)?;
    let mini = MenuItem::with_id(app, "mini", t.mini, true, none)?;
    let recognize = MenuItem::with_id(app, "recognize", t.recognize, true, none)?;
    let quit = MenuItem::with_id(app, "quit", t.quit, true, none)?;
    let downloader = dl_items(app)?;
    let separator = || PredefinedMenuItem::separator(app);
    let menu = Menu::with_items(
        app,
        &[
            &now,
            &separator()?,
            &toggle,
            &next,
            &previous,
            &separator()?,
        ],
    )?;
    menu.append_items(&[&downloader.menu, &separator()?])?;
    menu.append_items(&[&show, &mini])?;
    #[cfg(target_os = "macos")]
    menu.append(&recognize)?;
    menu.append_items(&[&separator()?, &quit])?;
    let update = UpdateItems {
        menu: menu.clone(),
        item: MenuItem::with_id(app, "update", t.update, true, none)?,
        separator: separator()?,
        shown: Mutex::new(false),
    };
    app.manage(TrayItems {
        now,
        toggle,
        next,
        previous,
        show,
        mini,
        recognize,
        quit,
        downloader,
        update,
    });
    refresh_downloader(app);
    refresh_update(app);

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

fn dl_items(app: &AppHandle) -> tauri::Result<DlItems> {
    let t = dl_texts(app.state::<Settings>().locale());
    let toggle = MenuItem::with_id(app, "dl-toggle", t.pause, false, None::<&str>)?;
    let enabled = CheckMenuItem::with_id(app, "dl-enabled", t.enabled, true, true, None::<&str>)?;
    let menu = Submenu::with_id_and_items(
        app,
        "downloader",
        format!("{}{}", t.prefix, t.starting),
        true,
        &[&toggle, &enabled],
    )?;
    Ok(DlItems {
        menu,
        toggle,
        enabled,
        shown: Mutex::new(None),
    })
}

fn on_menu(app: &AppHandle, event: MenuEvent) {
    match event.id().as_ref() {
        "toggle" => player::remote(app, Command::Toggle, Source::Tray),
        "next" => player::remote(app, Command::Next, Source::Tray),
        "previous" => player::remote(app, Command::Previous, Source::Tray),
        "dl-toggle" => downloader::toggle_paused(app),
        "dl-enabled" => downloader::toggle_enabled(app),
        "show" => {
            if crate::mini::is_open(app) {
                crate::mini::close(app);
            } else {
                window::show_main(app);
            }
        }
        "recognize" => crate::recognize::open(app),
        "mini" => {
            if let Err(e) = crate::mini::toggle(app) {
                log::warn!("mini player: {e}");
            }
        }
        "update" => updates::restart(app),
        "quit" => app.exit(0),
        _ => {}
    }
}

/// `&` в меню — знак клавиши-ускорителя: в названиях треков его надо удвоить.
fn menu_text(text: &str) -> String {
    text.replace('&', "&&")
}

/// Обрезать строку до `max` символов многоточием.
fn cut(line: &str, max: usize) -> String {
    if line.chars().count() <= max {
        return line.to_string();
    }
    let mut cut: String = line.chars().take(max.saturating_sub(1)).collect();
    cut = cut.trim_end().to_string();
    cut.push('…');
    cut
}

/// Строка «что играет» в меню: обрезается, чтобы меню не растягивалось на пол-экрана.
fn now_line(now: &NowPlaying) -> String {
    cut(&format!("{} — {}", now.artist, now.title), LINE_MAX)
}

/// Строка загрузчика: «Загрузчик: качает «Трек» — Артист · 45 %». Обрезается название, а не
/// процент.
fn downloader_line(t: &DlTexts, state: &TrayState) -> String {
    let (body, suffix) = match &state.summary {
        Summary::Working {
            track,
            artist,
            percent,
            finishing,
        } => {
            let suffix = format!(" · {percent}\u{a0}%");
            match (track, artist, finishing) {
                (Some(track), _, true) => (t.finishing.replace("{track}", track), String::new()),
                (Some(track), Some(artist), false) => (
                    t.working
                        .replace("{track}", track)
                        .replace("{artist}", artist),
                    suffix,
                ),
                (Some(track), None, false) => (
                    t.working
                        .replace("{track}", track)
                        .replace(" — {artist}", ""),
                    suffix,
                ),
                (None, ..) => (t.working_unknown.to_string(), suffix),
            }
        }
        Summary::Idle => (t.idle.into(), String::new()),
        Summary::Paused => (t.paused.into(), String::new()),
        Summary::Offline => (t.offline.into(), String::new()),
        Summary::Starting => (t.starting.into(), String::new()),
        Summary::Resting => (t.resting.into(), String::new()),
        Summary::Stopping => (t.stopping.into(), String::new()),
        Summary::Restarting => (t.restarting.into(), String::new()),
        Summary::TokenRejected => (t.token.into(), String::new()),
        Summary::Broken => (t.broken.into(), String::new()),
        Summary::Disabled => (t.disabled.into(), String::new()),
        Summary::SignedOut => (t.signed_out.into(), String::new()),
        Summary::Forbidden => (t.forbidden.into(), String::new()),
    };
    let room = DL_LINE_MAX.saturating_sub(t.prefix.chars().count() + suffix.chars().count());
    format!("{}{}{suffix}", t.prefix, cut(&body, room))
}

/// Обновить пункты загрузчика: после события загрузчика, команды или смены языка. Главный поток.
pub fn refresh_downloader(app: &AppHandle) {
    let (Some(items), Some(state)) = (app.try_state::<TrayItems>(), downloader::tray_state(app))
    else {
        return;
    };
    let t = dl_texts(app.state::<Settings>().locale());
    let dl = &items.downloader;
    let line = downloader_line(t, &state);
    let toggle = if state.paused { t.resume } else { t.pause };
    let can_pause = state.can_pause();
    let next = (line, toggle.to_string(), can_pause);
    {
        let mut shown = dl.shown.lock().unwrap_or_else(|e| e.into_inner());
        if shown.as_ref() != Some(&next) {
            let _ = dl.menu.set_text(menu_text(&next.0));
            let _ = dl.toggle.set_text(&next.1);
            let _ = dl.toggle.set_enabled(next.2);
            let _ = dl.enabled.set_text(t.enabled);
            *shown = Some(next);
        }
    }
    // Флажок muda переключает сам при щелчке — ставим настоящее значение всегда
    let _ = dl.enabled.set_checked(state.enabled);
}

/// Показать или убрать «Обновление готово — Перезапустить»: после перемены у обновления и смены
/// языка. Главный поток.
pub fn refresh_update(app: &AppHandle) {
    let Some(items) = app.try_state::<TrayItems>() else {
        return;
    };
    let t = texts(app.state::<Settings>().locale());
    let u = &items.update;
    let ready = updates::ready_version(app).is_some();
    let mut shown = u.shown.lock().unwrap_or_else(|e| e.into_inner());
    let _ = u.item.set_text(t.update);
    if ready && !*shown {
        *shown = u.menu.insert_items(&[&u.item, &u.separator], 0).is_ok();
    } else if !ready && *shown {
        let _ = u.menu.remove(&u.item);
        let _ = u.menu.remove(&u.separator);
        *shown = false;
    }
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
    let _ = items.now.set_text(
        line.as_deref()
            .map_or_else(|| t.nothing.to_string(), menu_text),
    );
    let _ = items
        .toggle
        .set_text(if playing { t.pause } else { t.play });
    let _ = items.next.set_text(t.next);
    let _ = items.previous.set_text(t.previous);
    let _ = items.show.set_text(t.show);
    let _ = items.mini.set_text(if crate::mini::is_open(app) {
        t.mini_close
    } else {
        t.mini
    });
    let _ = items.recognize.set_text(t.recognize);
    let _ = items.quit.set_text(t.quit);
    if let Some(tray) = app.tray_by_id(TRAY) {
        let tooltip = line.map_or_else(|| "DalyWave".to_string(), |l| format!("DalyWave — {l}"));
        let _ = tray.set_tooltip(Some(tooltip));
    }
    refresh_downloader(app);
    refresh_update(app);
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
        assert!(line.chars().count() <= LINE_MAX);
        assert!(line.ends_with('…'));
    }

    #[test]
    fn ampersands_are_not_accelerators() {
        assert_eq!(menu_text("Simon & Garfunkel"), "Simon && Garfunkel");
    }

    #[test]
    fn downloader_lines() {
        let state = |summary| TrayState {
            summary,
            enabled: true,
            paused: false,
        };
        let working = state(Summary::Working {
            track: Some("Another World".into()),
            artist: Some("CONTROL Resonant".into()),
            percent: 45,
            finishing: false,
        });
        assert_eq!(
            downloader_line(&DL_RU, &working),
            "Загрузчик: качает «Another World» — CONTROL Resonant · 45\u{a0}%"
        );
        let long = state(Summary::Working {
            track: Some("Очень длинное название трека, которое не помещается в меню".into()),
            artist: Some("Parker the Bandit".into()),
            percent: 7,
            finishing: false,
        });
        let line = downloader_line(&DL_RU, &long);
        assert!(line.ends_with("· 7\u{a0}%"), "{line}");
        assert!(line.contains('…'));
        assert_eq!(
            downloader_line(&DL_RU, &state(Summary::Idle)),
            "Загрузчик: ждёт заданий"
        );
        assert_eq!(
            downloader_line(&DL_EN, &state(Summary::Offline)),
            "Downloader: can't reach the server"
        );
        assert!(!state(Summary::Forbidden).can_pause());
        assert!(state(Summary::Paused).can_pause());
    }
}
