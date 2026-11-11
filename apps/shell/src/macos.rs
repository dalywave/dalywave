//! macOS: замер «светофора», который система рисует поверх интерфейса.

use objc2::MainThreadMarker;
use objc2_app_kit::{NSWindow, NSWindowButton};
use tauri::WebviewWindow;

use crate::platform::Rect;

/// Кнопки окна в CSS-пикселях от левого верхнего угла окна. Только на главном потоке
/// (команды без `async` выполняются на нём).
pub fn traffic_lights(window: &WebviewWindow) -> Option<Rect> {
    MainThreadMarker::new()?;
    let ptr = window.ns_window().ok()?;
    if ptr.is_null() {
        return None;
    }
    // SAFETY: Tauri отдаёт живой NSWindow этого окна; обращаемся к нему с главного потока
    let ns_window: &NSWindow = unsafe { &*(ptr as *const NSWindow) };
    let close = ns_window.standardWindowButton(NSWindowButton::CloseButton)?;
    let zoom = ns_window.standardWindowButton(NSWindowButton::ZoomButton)?;
    // В координатах окна начало — внизу слева
    let first = close.convertRect_toView(close.bounds(), None);
    let last = zoom.convertRect_toView(zoom.bounds(), None);
    let height = ns_window.frame().size.height;
    Some(Rect {
        x: first.origin.x,
        y: height - (first.origin.y + first.size.height),
        width: last.origin.x + last.size.width - first.origin.x,
        height: first.size.height,
    })
}
