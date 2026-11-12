//! iPhone и iPad: вход через ASWebAuthenticationSession — системное окно браузера поверх
//! приложения. Возврат `dalywave://auth?…` система отдаёт прямо сюда: не нужно уходить в Safari
//! и подтверждать «Открыть в DalyWave?», а iOS не усыпит приложение, пока человек входит.
//! Cookie общие с Safari: если в daly.rest уже вошли там, второй раз входить не придётся.
//!
//! Здесь же окно выбора AirPlay и Bluetooth (AVRoutePickerView) для шторки «Устройства» и тема
//! приложения: светлая, тёмная или как в системе.

use std::cell::RefCell;
use std::ptr::NonNull;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use block2::RcBlock;
use objc2::rc::Allocated;
use objc2::rc::Retained;
use objc2::runtime::{AnyClass, AnyObject, Bool, NSObject, NSObjectProtocol, ProtocolObject};
use objc2::{AnyThread, DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send};
use objc2_authentication_services::{
    ASWebAuthenticationPresentationContextProviding, ASWebAuthenticationSession,
};
use objc2_core_foundation::{CGPoint, CGRect, CGSize};
use objc2_foundation::{NSError, NSString, NSURL};
use objc2_ui_kit::{UIColor, UITraitCollection, UIUserInterfaceStyle};
use tauri::{AppHandle, Manager, WebviewWindow};
use url::Url;

use crate::{MAIN, auth::Auth, settings::ThemeChoice};

/// `ASWebAuthenticationSessionErrorCodeCanceledLogin`
const CANCELED_LOGIN: isize = 1;

define_class!(
    // Окно, над которым система покажет страницу входа.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "DalyWaveAuthPresentation"]
    #[ivars = Retained<NSObject>]
    struct Presentation;

    unsafe impl NSObjectProtocol for Presentation {}

    unsafe impl ASWebAuthenticationPresentationContextProviding for Presentation {
        #[unsafe(method(presentationAnchorForWebAuthenticationSession:))]
        fn presentation_anchor(&self, _session: &ASWebAuthenticationSession) -> *mut NSObject {
            Retained::as_ptr(self.ivars()).cast_mut()
        }
    }
);

impl Presentation {
    fn new(mtm: MainThreadMarker, window: Retained<NSObject>) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(window);
        unsafe { msg_send![super(this), init] }
    }
}

/// Открытая страница входа. Сессия и её окно живут до конца входа: система держит окно
/// слабой ссылкой.
struct Active {
    id: u64,
    session: Retained<ASWebAuthenticationSession>,
    _presentation: Retained<Presentation>,
}

thread_local! {
    static ACTIVE: RefCell<Option<Active>> = const { RefCell::new(None) };
    /// Невидимый AVRoutePickerView в окне: создаётся один раз и живёт до конца работы
    static ROUTE_PICKER: RefCell<Option<Retained<NSObject>>> = const { RefCell::new(None) };
}

/// `UIControlEventTouchUpInside`
const TOUCH_UP_INSIDE: usize = 1 << 6;

static NEXT_ID: AtomicU64 = AtomicU64::new(1);

/// UIWindow главного окна: WKWebView → его окно.
fn key_window(app: &AppHandle) -> Option<Retained<NSObject>> {
    let webview = app.get_webview_window(MAIN)?;
    let (tx, rx) = std::sync::mpsc::channel::<usize>();
    webview
        .with_webview(move |wv| {
            let view = wv.inner() as *mut AnyObject;
            let window: Option<Retained<NSObject>> = if view.is_null() {
                None
            } else {
                // SAFETY: Tauri отдаёт живой WKWebView; `window` — свойство UIView
                unsafe { msg_send![&*view, window] }
            };
            // Объект ObjC не Send — передаём владение указателем, забираем на том же потоке
            let _ = tx.send(window.map_or(0, |w| Retained::into_raw(w) as usize));
        })
        .ok()?;
    // На главном потоке Tauri выполняет with_webview сразу, ожидание — только страховка
    let ptr = rx.recv_timeout(Duration::from_secs(1)).ok()?;
    // SAFETY: указатель из Retained::into_raw выше, +1 к счётчику ссылок
    unsafe { Retained::from_raw(ptr as *mut NSObject) }
}

/// Открыть страницу входа. Главный поток. `false` — сессия не началась (тогда — Safari).
pub fn start_web_auth(app: &AppHandle, start: &Url) -> bool {
    let Some(mtm) = MainThreadMarker::new() else {
        return false;
    };
    let Some(window) = key_window(app) else {
        log::warn!("sign-in sheet: no window to present over");
        return false;
    };
    cancel_web_auth();
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let handle = app.clone();
    let completion = RcBlock::new(move |url: *mut NSURL, error: *mut NSError| {
        // Сессию закрыли мы сами (новый вход или отмена) — ответ ей больше не нужен
        let mine = ACTIVE.with(|a| {
            let mut a = a.borrow_mut();
            let mine = a.as_ref().is_some_and(|x| x.id == id);
            if mine {
                a.take();
            }
            mine
        });
        if !mine {
            return;
        }
        let auth = handle.state::<Auth>();
        // SAFETY: система передаёт живые NSURL и NSError или nil
        let url = unsafe { url.as_ref() }
            .and_then(|u| u.absoluteString())
            .map(|s| s.to_string());
        match (url, unsafe { error.as_ref() }) {
            (Some(url), _) => match Url::parse(&url) {
                Ok(url) => auth.deliver(&url),
                Err(e) => {
                    log::warn!("sign-in sheet returned a bad link: {e}");
                    auth.cancel();
                }
            },
            (None, Some(e)) => {
                if e.code() != CANCELED_LOGIN {
                    log::warn!("sign-in sheet failed: {}", e.localizedDescription());
                }
                auth.cancel();
            }
            (None, None) => {
                auth.cancel();
            }
        }
    });
    let ns_url = NSURL::URLWithString(&NSString::from_str(start.as_str()));
    let Some(ns_url) = ns_url else {
        return false;
    };
    let scheme = NSString::from_str("dalywave");
    // SAFETY: блок живёт, пока его держит сессия (она копирует его при создании)
    #[allow(deprecated)] // initWithURL:callback: — только с iOS 17.4, а приложение — с 15
    let session = unsafe {
        ASWebAuthenticationSession::initWithURL_callbackURLScheme_completionHandler(
            ASWebAuthenticationSession::alloc(),
            &ns_url,
            Some(&scheme),
            RcBlock::as_ptr(&completion),
        )
    };
    let presentation = Presentation::new(mtm, window);
    unsafe {
        session.setPresentationContextProvider(Some(ProtocolObject::from_ref(&*presentation)));
        session.setPrefersEphemeralWebBrowserSession(false);
    }
    if !unsafe { session.start() } {
        log::warn!("sign-in sheet did not start");
        return false;
    }
    ACTIVE.with(|a| {
        *a.borrow_mut() = Some(Active {
            id,
            session,
            _presentation: presentation,
        })
    });
    log::info!("sign-in sheet opened");
    true
}

/// Закрыть страницу входа, если она открыта. Главный поток.
pub fn cancel_web_auth() {
    if let Some(active) = ACTIVE.with(|a| a.borrow_mut().take()) {
        unsafe { active.session.cancel() };
    }
}

/// Открыть системное окно «Куда выводить звук» (AirPlay, Bluetooth, динамик). Главный поток.
///
/// Открыть его напрямую API не даёт — только кнопка AVRoutePickerView. Поэтому в окне живёт
/// невидимый picker (почти прозрачный и без касаний: не перехватывает нажатия интерфейса), и мы
/// «нажимаем» его кнопку сами. Стоит он внизу по центру — на iPad к этому месту привязан поповер.
pub fn show_route_picker(app: &AppHandle) -> Result<(), String> {
    if MainThreadMarker::new().is_none() {
        return Err("окно выбора открывается только с главного потока".into());
    }
    let window = key_window(app).ok_or("нет окна приложения")?;
    let picker = match ROUTE_PICKER.with(|p| p.borrow().clone()) {
        Some(p) => p,
        None => {
            let picker = create_route_picker()?;
            ROUTE_PICKER.with(|p| *p.borrow_mut() = Some(picker.clone()));
            picker
        }
    };
    // SAFETY: picker — UIView, window — UIWindow; все вызовы — на главном потоке
    unsafe {
        let bounds: CGRect = msg_send![&*window, bounds];
        let frame = CGRect {
            origin: CGPoint {
                x: bounds.size.width / 2.0 - 0.5,
                y: bounds.size.height - 1.0,
            },
            size: CGSize {
                width: 1.0,
                height: 1.0,
            },
        };
        let _: () = msg_send![&*picker, setFrame: frame];
        let parent: Option<Retained<NSObject>> = msg_send![&*picker, window];
        if parent.as_deref() != Some(&*window) {
            let _: () = msg_send![&*window, addSubview: &*picker];
        }
        let _: () = msg_send![&*picker, layoutIfNeeded];
        let button = route_picker_button(&picker).ok_or("у AVRoutePickerView нет кнопки")?;
        let _: () = msg_send![&*button, sendActionsForControlEvents: TOUCH_UP_INSIDE];
    }
    log::info!("route picker opened");
    Ok(())
}

fn create_route_picker() -> Result<Retained<NSObject>, String> {
    // AVKit подключён в gen/apple/project.yml: статическая библиотека Rust его не тянет
    let class = AnyClass::get(c"AVRoutePickerView").ok_or("нет AVRoutePickerView (AVKit)")?;
    let zero = CGRect {
        origin: CGPoint { x: 0.0, y: 0.0 },
        size: CGSize {
            width: 1.0,
            height: 1.0,
        },
    };
    // SAFETY: AVRoutePickerView — UIView: alloc + initWithFrame:, свойства UIView; главный поток
    unsafe {
        let allocated: Allocated<NSObject> = msg_send![class, alloc];
        let picker: Retained<NSObject> = msg_send![allocated, initWithFrame: zero];
        let _: () = msg_send![&*picker, setAlpha: 0.011f64];
        let _: () = msg_send![&*picker, setUserInteractionEnabled: false];
        Ok(picker)
    }
}

/// Кнопка внутри AVRoutePickerView — первый UIButton среди дочерних видов.
fn route_picker_button(picker: &NSObject) -> Option<Retained<NSObject>> {
    let button_class = AnyClass::get(c"UIButton")?;
    // SAFETY: subviews — NSArray<UIView>, обход по индексу; главный поток
    unsafe {
        let subviews: Retained<NSObject> = msg_send![picker, subviews];
        let count: usize = msg_send![&*subviews, count];
        (0..count).find_map(|i| {
            let view: Retained<NSObject> = msg_send![&*subviews, objectAtIndex: i];
            let is_button: Bool = msg_send![&*view, isKindOfClass: button_class];
            is_button.as_bool().then_some(view)
        })
    }
}

/// Тема на iPhone и iPad. `overrideUserInterfaceStyle` окна приложения: по нему WKWebView отдаёт
/// странице `prefers-color-scheme`, системные окна (меню, вход, AirPlay) рисуются в той же теме,
/// а строка состояния (её стиль решает контроллер, `UIViewControllerBasedStatusBarAppearance`)
/// берёт светлый или тёмный текст. Фон окна, WebView и его прокрутки — цвета темы: он виден до
/// первой отрисовки страницы и при оттягивании прокрутки. Можно звать с любого потока.
pub fn apply_theme(webview: &WebviewWindow, theme: ThemeChoice) {
    let result = webview.with_webview(move |wv| {
        let view = wv.inner() as *mut AnyObject;
        // SAFETY: Tauri отдаёт живой WKWebView и зовёт замыкание на главном потоке
        if let Some(view) = unsafe { view.as_ref() } {
            unsafe { paint_theme(view, theme) };
        }
    });
    if let Err(e) = result {
        log::warn!("could not apply the theme: {e}");
    }
}

/// SAFETY: `webview` — WKWebView; только главный поток.
unsafe fn paint_theme(webview: &AnyObject, theme: ThemeChoice) {
    let style = match theme {
        ThemeChoice::Dark => UIUserInterfaceStyle::Dark,
        ThemeChoice::Light => UIUserInterfaceStyle::Light,
        ThemeChoice::System => UIUserInterfaceStyle::Unspecified,
    };
    let color = theme_color(theme);
    unsafe {
        let window: Option<Retained<AnyObject>> = msg_send![webview, window];
        match window {
            Some(window) => {
                let _: () = msg_send![&*window, setOverrideUserInterfaceStyle: style];
                let _: () = msg_send![&*window, setBackgroundColor: &*color];
                // Строка состояния со стилем Default берёт текст по теме контроллера —
                // попросим её перечитать стиль сразу, а не при следующей смене экрана
                let root: Option<Retained<AnyObject>> = msg_send![&*window, rootViewController];
                if let Some(root) = root {
                    let _: () = msg_send![&*root, setNeedsStatusBarAppearanceUpdate];
                }
            }
            None => log::warn!("theme: the web view is not in a window yet"),
        }
        // Вид окна Tao, в котором лежит WebView, — тоже: Wry красит его цветом из настроек окна
        let parent: Option<Retained<AnyObject>> = msg_send![webview, superview];
        if let Some(parent) = parent {
            let _: () = msg_send![&*parent, setBackgroundColor: &*color];
        }
        let _: () = msg_send![webview, setBackgroundColor: &*color];
        let scroll: Option<Retained<AnyObject>> = msg_send![webview, scrollView];
        if let Some(scroll) = scroll {
            let _: () = msg_send![&*scroll, setBackgroundColor: &*color];
        }
    }
}

/// Фон под интерфейсом: тёмный `#08090c` (`--ink-1`), светлый — белый. «Как в системе» —
/// динамический цвет: UIKit сам перекрашивает его, когда система меняет тему.
fn theme_color(theme: ThemeChoice) -> Retained<UIColor> {
    let dark = UIColor::colorWithRed_green_blue_alpha(8.0 / 255.0, 9.0 / 255.0, 12.0 / 255.0, 1.0);
    let light = UIColor::whiteColor();
    match theme {
        ThemeChoice::Dark => dark,
        ThemeChoice::Light => light,
        ThemeChoice::System => {
            let provider = RcBlock::new(move |traits: NonNull<UITraitCollection>| {
                // SAFETY: UIKit передаёт живую коллекцию признаков на главном потоке
                let style = unsafe { traits.as_ref().userInterfaceStyle() };
                let color = if style == UIUserInterfaceStyle::Dark {
                    &dark
                } else {
                    &light
                };
                NonNull::from(&**color)
            });
            // SAFETY: блок возвращает цвета, которые держит сам (+0, как положено блоку);
            // UIKit копирует блок и держит его вместе с цветами
            unsafe { UIColor::colorWithDynamicProvider(&provider) }
        }
    }
}
