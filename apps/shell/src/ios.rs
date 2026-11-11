//! iPhone и iPad: вход через ASWebAuthenticationSession — системное окно браузера поверх
//! приложения. Возврат `dalywave://auth?…` система отдаёт прямо сюда: не нужно уходить в Safari
//! и подтверждать «Открыть в DalyWave?», а iOS не усыпит приложение, пока человек входит.
//! Cookie общие с Safari: если в daly.rest уже вошли там, второй раз входить не придётся.

use std::cell::RefCell;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use block2::RcBlock;
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, NSObject, NSObjectProtocol, ProtocolObject};
use objc2::{AnyThread, DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send};
use objc2_authentication_services::{
    ASWebAuthenticationPresentationContextProviding, ASWebAuthenticationSession,
};
use objc2_foundation::{NSError, NSString, NSURL};
use tauri::{AppHandle, Manager};
use url::Url;

use crate::{MAIN, auth::Auth};

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
}

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
