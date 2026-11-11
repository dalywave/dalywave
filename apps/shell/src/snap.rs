//! Windows 11: меню привязки окон (Snap Layouts) при наведении на свою кнопку «развернуть».
//!
//! Система показывает это меню, когда на `WM_NCHITTEST` окно отвечает `HTMAXBUTTON`. Кнопку
//! рисует интерфейс внутри WebView2, а дочерние окна WebView2 принадлежат другому процессу —
//! их ответы на проверку попадания не изменить. Поэтому поверх кнопки лежит своё невидимое
//! дочернее окно: на проверку попадания отвечает `HTMAXBUTTON`, а наведение, нажатие и щелчок
//! пересылает интерфейсу (сам WebView этих событий над кнопкой уже не получит). Невидимо оно
//! потому, что WebView2 рисует себя через DirectComposition поверх содержимого окон GDI.

use std::{
    mem::size_of,
    ptr::{null, null_mut},
    sync::{Mutex, OnceLock},
};

use serde::Serialize;
use tauri::{Emitter, WebviewWindow, Window};
use windows_sys::{
    Wdk::System::SystemServices::RtlGetVersion,
    Win32::{
        Foundation::{HWND, LPARAM, LRESULT, WPARAM},
        System::{LibraryLoader::GetModuleHandleW, SystemInformation::OSVERSIONINFOW},
        UI::{
            Input::KeyboardAndMouse::{TME_LEAVE, TME_NONCLIENT, TRACKMOUSEEVENT, TrackMouseEvent},
            WindowsAndMessaging::{
                CreateWindowExW, DefWindowProcW, DestroyWindow, HTMAXBUTTON, HWND_TOP, IDC_ARROW,
                IsZoomed, LoadCursorW, RegisterClassExW, SC_MAXIMIZE, SC_RESTORE, SWP_NOACTIVATE,
                SendMessageW, SetCursor, SetWindowPos, WM_NCDESTROY, WM_NCHITTEST,
                WM_NCLBUTTONDOWN, WM_NCLBUTTONUP, WM_NCMOUSELEAVE, WM_SETCURSOR, WM_SYSCOMMAND,
                WNDCLASSEXW, WS_CHILD, WS_VISIBLE,
            },
        },
    },
};

use crate::{MAIN, error::ShellResult, platform::Rect};

pub const EVENT_CAPTION: &str = "window:caption";

struct Overlay {
    hwnd: isize,
    parent: isize,
    /// В CSS-пикселях: при смене масштаба экрана пересчитывается
    rect: Rect,
    hovered: bool,
    pressed: bool,
    window: WebviewWindow,
}

static OVERLAY: Mutex<Option<Overlay>> = Mutex::new(None);

#[derive(Clone, Serialize)]
struct Caption {
    button: &'static str,
    state: &'static str,
}

/// Mica и меню привязки окон есть только в Windows 11 (сборка 22000 и новее).
pub fn is_windows_11() -> bool {
    // SAFETY: RtlGetVersion заполняет переданную структуру; размер указан
    unsafe {
        let mut info: OSVERSIONINFOW = std::mem::zeroed();
        info.dwOSVersionInfoSize = size_of::<OSVERSIONINFOW>() as u32;
        RtlGetVersion(&mut info) == 0 && info.dwBuildNumber >= 22000
    }
}

pub fn set_area(window: &WebviewWindow, rect: Option<Rect>) -> ShellResult<()> {
    let parent = window.hwnd()?.0 as isize;
    let target = window.clone();
    window.run_on_main_thread(move || apply(target, parent, rect))?;
    Ok(())
}

/// Масштаб экрана сменился: та же кнопка в CSS-пикселях — другое место в физических.
pub fn refresh(window: &Window) {
    let current = lock()
        .as_ref()
        .map(|o| (o.window.clone(), o.parent, o.rect));
    if let Some((target, parent, rect)) = current
        && target.label() == window.label()
    {
        let _ = window.run_on_main_thread(move || apply(target, parent, Some(rect)));
    }
}

fn lock() -> std::sync::MutexGuard<'static, Option<Overlay>> {
    OVERLAY.lock().unwrap_or_else(|e| e.into_inner())
}

fn class_name() -> *const u16 {
    static NAME: OnceLock<Vec<u16>> = OnceLock::new();
    NAME.get_or_init(|| "DalyWaveSnapArea\0".encode_utf16().collect())
        .as_ptr()
}

fn physical(rect: Rect, scale: f64) -> (i32, i32, i32, i32) {
    let px = |v: f64| (v * scale).round() as i32;
    (
        px(rect.x),
        px(rect.y),
        px(rect.width).max(1),
        px(rect.height).max(1),
    )
}

/// Только на главном потоке. Блокировку не держим во время вызовов Win32: они могут
/// синхронно вызвать оконную процедуру, а та берёт ту же блокировку.
fn apply(window: WebviewWindow, parent: isize, rect: Option<Rect>) {
    let Some(rect) = rect else {
        let old = lock().take();
        if let Some(old) = old {
            // SAFETY: окно создано нами на этом потоке
            unsafe { DestroyWindow(old.hwnd as HWND) };
            emit(&old.window, "leave");
        }
        return;
    };
    let scale = window.scale_factor().unwrap_or(1.0);
    let (x, y, width, height) = physical(rect, scale);
    let existing = {
        let mut slot = lock();
        match slot.as_mut() {
            Some(o) if o.parent == parent => {
                o.rect = rect;
                Some(o.hwnd)
            }
            _ => None,
        }
    };
    if let Some(hwnd) = existing {
        // SAFETY: окно живо, пока лежит в OVERLAY (WM_NCDESTROY убирает его оттуда)
        unsafe { SetWindowPos(hwnd as HWND, HWND_TOP, x, y, width, height, SWP_NOACTIVATE) };
        return;
    }
    // SAFETY: регистрация класса и создание дочернего окна над окном приложения
    let hwnd = unsafe {
        register_class();
        CreateWindowExW(
            0,
            class_name(),
            null(),
            WS_CHILD | WS_VISIBLE,
            x,
            y,
            width,
            height,
            parent as HWND,
            null_mut(),
            GetModuleHandleW(null()),
            null(),
        )
    };
    if hwnd.is_null() {
        log::warn!("could not create the snap layout area");
        return;
    }
    // Поверх WebView2 по порядку окон: иначе проверку попадания получит он
    unsafe { SetWindowPos(hwnd, HWND_TOP, x, y, width, height, SWP_NOACTIVATE) };
    *lock() = Some(Overlay {
        hwnd: hwnd as isize,
        parent,
        rect,
        hovered: false,
        pressed: false,
        window,
    });
}

unsafe fn register_class() {
    static REGISTERED: OnceLock<()> = OnceLock::new();
    REGISTERED.get_or_init(|| {
        let class = WNDCLASSEXW {
            cbSize: size_of::<WNDCLASSEXW>() as u32,
            style: 0,
            lpfnWndProc: Some(overlay_proc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            // SAFETY: модуль текущего процесса
            hInstance: unsafe { GetModuleHandleW(null()) },
            hIcon: null_mut(),
            hCursor: null_mut(),
            // Без кисти фона: окно ничего не рисует
            hbrBackground: null_mut(),
            lpszMenuName: null(),
            lpszClassName: class_name(),
            hIconSm: null_mut(),
        };
        // SAFETY: структура заполнена целиком, имя класса живёт до конца процесса
        unsafe { RegisterClassExW(&class) };
    });
}

fn emit(window: &WebviewWindow, state: &'static str) {
    let _ = window.emit_to(
        MAIN,
        EVENT_CAPTION,
        Caption {
            button: "maximize",
            state,
        },
    );
}

/// Меняет состояние под блокировкой и отдаёт окно для события уже без неё.
fn update<T>(f: impl FnOnce(&mut Overlay) -> T) -> Option<(T, WebviewWindow)> {
    let mut slot = lock();
    let overlay = slot.as_mut()?;
    let result = f(overlay);
    Some((result, overlay.window.clone()))
}

unsafe extern "system" fn overlay_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_NCHITTEST => {
            if let Some((true, window)) = update(|o| !std::mem::replace(&mut o.hovered, true)) {
                let mut track = TRACKMOUSEEVENT {
                    cbSize: size_of::<TRACKMOUSEEVENT>() as u32,
                    dwFlags: TME_LEAVE | TME_NONCLIENT,
                    hwndTrack: hwnd,
                    dwHoverTime: 0,
                };
                // SAFETY: структура заполнена, hwnd — это окно
                unsafe { TrackMouseEvent(&mut track) };
                emit(&window, "enter");
            }
            HTMAXBUTTON as LRESULT
        }
        WM_NCMOUSELEAVE => {
            if let Some((true, window)) = update(|o| {
                o.pressed = false;
                std::mem::replace(&mut o.hovered, false)
            }) {
                emit(&window, "leave");
            }
            0
        }
        WM_NCLBUTTONDOWN if wparam == HTMAXBUTTON as WPARAM => {
            if let Some(((), window)) = update(|o| o.pressed = true) {
                emit(&window, "down");
            }
            0
        }
        WM_NCLBUTTONUP if wparam == HTMAXBUTTON as WPARAM => {
            if let Some(((clicked, parent), window)) =
                update(|o| (std::mem::replace(&mut o.pressed, false), o.parent))
            {
                emit(&window, "up");
                if clicked {
                    let parent = parent as HWND;
                    // SAFETY: родитель жив, пока жив его дочерний оверлей
                    let command = if unsafe { IsZoomed(parent) } != 0 {
                        SC_RESTORE
                    } else {
                        SC_MAXIMIZE
                    };
                    unsafe { SendMessageW(parent, WM_SYSCOMMAND, command as WPARAM, 0) };
                }
            }
            0
        }
        WM_SETCURSOR => {
            // SAFETY: системный курсор-стрелка
            unsafe { SetCursor(LoadCursorW(null_mut(), IDC_ARROW)) };
            1
        }
        WM_NCDESTROY => {
            let mut slot = lock();
            if slot.as_ref().is_some_and(|o| o.hwnd == hwnd as isize) {
                *slot = None;
            }
            drop(slot);
            // SAFETY: обычная обработка остального
            unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
        }
        // SAFETY: обычная обработка остального
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}
