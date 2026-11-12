//! «Распознать»: какая песня играет рядом. ShazamKit (`SHSession`) слушает микрофон через
//! `AVAudioEngine` — iPhone, iPad и macOS; на Windows и в Linux команды отвечают `unsupported`, а
//! интерфейс кнопку не показывает. Найденное (название, исполнитель, ISRC) интерфейс отправляет
//! серверу (`POST /api/recognize/resolve`) — тот находит трек в каталоге.
//!
//! Свою музыку слушать нельзя: пока идёт распознавание, движок на паузе. Не узнали, отменили или
//! сбой — музыка продолжается; узнали — пауза остаётся: дальше человек слушает найденное.
//! Слушаем до совпадения, но не дольше 15 с.
//!
//! Открыть экран распознавания просят пункт трея macOS и быстрое действие на значке iPhone
//! («Распознать»): событие `recognize:open`, а если интерфейс ещё не загрузился (приложение
//! запустили этим действием) — флаг, который он забирает `recognize_take_pending`.
//!
//! Проверка без микрофона (симулятор): `DALYWAVE_RECOGNIZE_MOCK` — через 2,5 с приходит
//! совпадение. `1` — пример по умолчанию, `Название|Исполнитель|ISRC` — своё, `none` — «не узнали»,
//! `permission` — нет доступа к микрофону. С `DALYWAVE_RECOGNIZE_OPEN=1` экран открывается сам при запуске.

use std::sync::{
    Mutex,
    atomic::{AtomicBool, AtomicU64, Ordering},
};
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::oneshot;

use crate::{
    MAIN,
    error::{ShellError, ShellResult},
    player::Player,
};

pub const EVENT_OPEN: &str = "recognize:open";

/// Дольше не слушаем: ShazamKit узнаёт за 3–8 с, дальше шансов почти нет
const LISTEN_FOR: Duration = Duration::from_secs(15);
const MOCK_DELAY: Duration = Duration::from_millis(2500);

/// Совпадение ShazamKit — как есть, сопоставление с каталогом делает сервер.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RecognizeMatch {
    pub title: String,
    pub artist: String,
    pub isrc: Option<String>,
    pub apple_music_id: Option<String>,
    pub shazam_id: Option<String>,
    pub artwork_url: Option<String>,
}

type Outcome = Result<RecognizeMatch, ShellError>;

/// Идущее распознавание: ответ ShazamKit приходит с его очереди, отмена — из интерфейса.
static CURRENT: Mutex<Option<(u64, oneshot::Sender<Outcome>)>> = Mutex::new(None);
static NEXT_ID: AtomicU64 = AtomicU64::new(1);
/// Экран распознавания попросили открыть до того, как интерфейс загрузился
static PENDING_OPEN: AtomicBool = AtomicBool::new(false);
/// ShazamKit отвечал ошибкой связи — по истечении времени это не «не узнали», а сбой
static SERVICE_ERROR: Mutex<Option<String>> = Mutex::new(None);

fn denied(code: &str, message: &str) -> ShellError {
    ShellError::Denied {
        code: code.to_string(),
        message: message.to_string(),
    }
}

/// Ответ для распознавания `id`; опоздавший (после отмены или времени) отбрасывается.
fn finish(id: u64, outcome: Outcome) {
    let mut current = CURRENT.lock().unwrap_or_else(|e| e.into_inner());
    if current.as_ref().is_some_and(|(cur, _)| *cur == id)
        && let Some((_, tx)) = current.take()
    {
        let _ = tx.send(outcome);
    }
}

fn mock() -> Option<String> {
    std::env::var("DALYWAVE_RECOGNIZE_MOCK")
        .ok()
        .filter(|v| !v.trim().is_empty())
}

fn mock_outcome(spec: &str) -> Outcome {
    match spec.trim() {
        "none" => Err(denied("no_match", "mock: no match")),
        "permission" => Err(denied("permission", "mock: microphone access denied")),
        "1" | "match" => Ok(RecognizeMatch {
            title: "Midnight City".into(),
            artist: "M83".into(),
            isrc: None,
            apple_music_id: None,
            shazam_id: None,
            artwork_url: None,
        }),
        custom => {
            let mut parts = custom.split('|').map(str::trim);
            let title = parts.next().unwrap_or_default().to_string();
            let artist = parts.next().unwrap_or_default().to_string();
            let isrc = parts.next().filter(|s| !s.is_empty()).map(str::to_string);
            Ok(RecognizeMatch {
                title,
                artist,
                isrc,
                apple_music_id: None,
                shazam_id: None,
                artwork_url: None,
            })
        }
    }
}

/// Распознавание есть на этой платформе (или включена проверка без микрофона).
fn supported() -> bool {
    cfg!(any(target_os = "macos", target_os = "ios")) || mock().is_some()
}

/// Пауза своей музыки на время распознавания. `true` — играла.
fn pause_own(app: &AppHandle) -> bool {
    let Some(engine) = app.state::<Player>().engine() else {
        return false;
    };
    let playing = engine.snapshot().state == dalywave_audio::PlaybackState::Playing;
    if playing {
        engine.pause();
    }
    playing
}

fn resume_own(app: &AppHandle) {
    if let Some(engine) = app.state::<Player>().engine() {
        engine.play();
    }
}

/// Попросить интерфейс открыть экран распознавания (трей, быстрое действие на значке).
#[cfg_attr(not(any(target_os = "macos", target_os = "ios")), allow(dead_code))]
pub fn open(app: &AppHandle) {
    PENDING_OPEN.store(true, Ordering::SeqCst);
    #[cfg(desktop)]
    {
        if crate::mini::is_open(app) {
            crate::mini::close(app);
        } else {
            crate::window::show_main(app);
        }
    }
    log::info!("recognize: open requested");
    let _ = app.emit_to(MAIN, EVENT_OPEN, ());
}

/// Подготовка при запуске: быстрое действие «Распознать» на значке iPhone. Звать после
/// `Builder::build` (классы делегатов уже есть) и до `App::run` (сцена ещё не подключилась).
#[cfg_attr(not(target_os = "ios"), allow(unused_variables))]
pub fn install(app: &AppHandle) {
    // Проверка без микрофона: экран распознавания открывается сам, как от быстрого действия
    if mock().is_some() && std::env::var("DALYWAVE_RECOGNIZE_OPEN").is_ok_and(|v| v == "1") {
        PENDING_OPEN.store(true, Ordering::SeqCst);
    }
    #[cfg(target_os = "ios")]
    quick::install(app);
}

/// Слушать микрофон до совпадения (не дольше 15 с). Ошибки: `no_match` — не узнали,
/// `permission` — нет доступа к микрофону, `no_microphone`, `cancelled`, `shazam` — сбой ShazamKit
/// (нет сети, не включена служба ShazamKit у App ID), `unsupported`.
#[tauri::command]
pub async fn recognize_listen(app: AppHandle) -> ShellResult<RecognizeMatch> {
    if !supported() {
        return Err(ShellError::Unsupported(
            "распознавание есть только на iPhone, iPad и Mac".into(),
        ));
    }
    let mock = mock();
    #[cfg(any(target_os = "macos", target_os = "ios"))]
    if mock.is_none() && !apple::microphone_allowed().await {
        return Err(denied("permission", "microphone access denied"));
    }

    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let (tx, rx) = oneshot::channel();
    {
        let mut current = CURRENT.lock().unwrap_or_else(|e| e.into_inner());
        if let Some((_, old)) = current.replace((id, tx)) {
            let _ = old.send(Err(ShellError::Cancelled));
        }
    }
    *SERVICE_ERROR.lock().unwrap_or_else(|e| e.into_inner()) = None;
    let was_playing = pause_own(&app);
    log::info!("recognize: listening (mock: {})", mock.is_some());

    match mock {
        Some(spec) => {
            tauri::async_runtime::spawn(async move {
                tokio::time::sleep(MOCK_DELAY).await;
                finish(id, mock_outcome(&spec));
            });
        }
        None => {
            #[cfg(any(target_os = "macos", target_os = "ios"))]
            app.run_on_main_thread(move || {
                if let Err(e) = apple::start(id) {
                    finish(id, Err(e));
                }
            })?;
        }
    }

    let outcome = match tokio::time::timeout(LISTEN_FOR, rx).await {
        Ok(Ok(outcome)) => outcome,
        Ok(Err(_)) => Err(ShellError::Cancelled),
        Err(_) => match SERVICE_ERROR
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take()
        {
            Some(message) => Err(denied("shazam", &message)),
            None => Err(denied("no_match", "no match in 15 s")),
        },
    };
    {
        let mut current = CURRENT.lock().unwrap_or_else(|e| e.into_inner());
        if current.as_ref().is_some_and(|(cur, _)| *cur == id) {
            current.take();
        }
    }
    #[cfg(any(target_os = "macos", target_os = "ios"))]
    let _ = app.run_on_main_thread(move || apple::stop(id));
    match &outcome {
        Ok(m) => log::info!("recognize: match «{}» — {}", m.title, m.artist),
        Err(e) => log::info!("recognize: {} ({e})", e.code()),
    }
    // Узнали — пауза остаётся: сейчас человек будет слушать найденное
    if outcome.is_err() && was_playing {
        resume_own(&app);
    }
    outcome
}

/// Прервать распознавание. `true` — оно шло.
#[tauri::command]
pub fn recognize_cancel() -> bool {
    let current = CURRENT.lock().unwrap_or_else(|e| e.into_inner()).take();
    match current {
        Some((_, tx)) => {
            let _ = tx.send(Err(ShellError::Cancelled));
            true
        }
        None => false,
    }
}

/// Забрать просьбу открыть экран распознавания, пришедшую до загрузки интерфейса.
#[tauri::command]
pub fn recognize_take_pending() -> bool {
    PENDING_OPEN.swap(false, Ordering::SeqCst)
}

/// ShazamKit и микрофон: Mac, iPhone, iPad.
#[cfg(any(target_os = "macos", target_os = "ios"))]
mod apple {
    use std::cell::RefCell;
    use std::ptr::NonNull;

    use block2::RcBlock;
    use objc2::rc::Retained;
    use objc2::runtime::{Bool, NSObject, NSObjectProtocol, ProtocolObject};
    use objc2::{AnyThread, DefinedClass, define_class, msg_send};
    use objc2_av_foundation::{AVAuthorizationStatus, AVCaptureDevice, AVMediaTypeAudio};
    use objc2_avf_audio::{AVAudioEngine, AVAudioPCMBuffer, AVAudioTime};
    use objc2_foundation::NSError;
    use objc2_shazam_kit::{
        SHErrorDomain, SHMatch, SHMediaItem, SHSession, SHSessionDelegate, SHSignature,
    };
    use tokio::sync::oneshot;

    use super::{RecognizeMatch, SERVICE_ERROR, denied, finish};
    use crate::error::{ShellError, ShellResult};

    /// `SHErrorCodeMatchAttemptFailed`: сервис Shazam не ответил (сеть, служба не включена)
    const MATCH_ATTEMPT_FAILED: isize = 202;
    /// Шаг подачи звука: ~43 мс при 48 кГц — так ShazamKit советует в примерах
    const TAP_FRAMES: u32 = 2048;

    /// Доступ к микрофону: спросить систему, если ещё не спрашивали.
    pub async fn microphone_allowed() -> bool {
        // SAFETY: константа AVFoundation, доступна с iOS 4 и macOS 10.7
        let Some(audio) = (unsafe { AVMediaTypeAudio }) else {
            return false;
        };
        // SAFETY: классовый метод, любой поток
        let status = unsafe { AVCaptureDevice::authorizationStatusForMediaType(audio) };
        if status == AVAuthorizationStatus::Authorized {
            return true;
        }
        if status != AVAuthorizationStatus::NotDetermined {
            return false;
        }
        let (tx, rx) = oneshot::channel::<bool>();
        {
            let tx = std::sync::Mutex::new(Some(tx));
            let handler = RcBlock::new(move |granted: Bool| {
                if let Some(tx) = tx.lock().unwrap_or_else(|e| e.into_inner()).take() {
                    let _ = tx.send(granted.as_bool());
                }
            });
            // SAFETY: ответ приходит на произвольной очереди; блок копируется системой
            unsafe {
                AVCaptureDevice::requestAccessForMediaType_completionHandler(audio, &handler)
            };
        }
        rx.await.unwrap_or(false)
    }

    define_class!(
        // Ответы ShazamKit: с его очереди — в ожидающую команду.
        #[unsafe(super(NSObject))]
        #[name = "DalyWaveShazamDelegate"]
        #[ivars = u64]
        struct Delegate;

        unsafe impl NSObjectProtocol for Delegate {}

        unsafe impl SHSessionDelegate for Delegate {
            #[unsafe(method(session:didFindMatch:))]
            fn did_find_match(&self, _session: &SHSession, found: &SHMatch) {
                let id = *self.ivars();
                // SAFETY: SHMatch от системы, список элементов не пустой у совпадения
                let item = unsafe { found.mediaItems() }.firstObject();
                match item.and_then(|item| media_match(&item)) {
                    Some(m) => finish(id, Ok(m)),
                    None => log::warn!("recognize: match without a title"),
                }
            }

            #[unsafe(method(session:didNotFindMatchForSignature:error:))]
            fn did_not_find_match(
                &self,
                _session: &SHSession,
                _signature: &SHSignature,
                error: Option<&NSError>,
            ) {
                // Без ошибки — просто этот кусок звука не узнали: слушаем дальше
                let Some(error) = error else { return };
                let shazam = unsafe { SHErrorDomain }.is_some_and(|d| *error.domain() == *d);
                let message = error.localizedDescription().to_string();
                log::warn!("recognize: ShazamKit error {} ({message})", error.code());
                if shazam && error.code() == MATCH_ATTEMPT_FAILED {
                    *SERVICE_ERROR.lock().unwrap_or_else(|e| e.into_inner()) = Some(message);
                }
            }
        }
    );

    impl Delegate {
        fn new(id: u64) -> Retained<Self> {
            let this = Self::alloc().set_ivars(id);
            // SAFETY: NSObject init
            unsafe { msg_send![super(this), init] }
        }
    }

    fn text(s: Option<Retained<objc2_foundation::NSString>>) -> Option<String> {
        s.map(|s| s.to_string()).filter(|s| !s.trim().is_empty())
    }

    fn media_match(item: &SHMediaItem) -> Option<RecognizeMatch> {
        // SAFETY: свойства SHMediaItem только читаются
        unsafe {
            Some(RecognizeMatch {
                title: text(item.title())?,
                artist: text(item.artist()).unwrap_or_default(),
                isrc: text(item.isrc()),
                apple_music_id: text(item.appleMusicID()),
                shazam_id: text(item.shazamID()),
                artwork_url: item
                    .artworkURL()
                    .and_then(|u| u.absoluteString())
                    .map(|s| s.to_string()),
            })
        }
    }

    /// Идущее распознавание: движок микрофона и сессия живут на главном потоке.
    struct Active {
        id: u64,
        engine: Retained<AVAudioEngine>,
        session: Retained<SHSession>,
        _delegate: Retained<Delegate>,
    }

    thread_local! {
        static ACTIVE: RefCell<Option<Active>> = const { RefCell::new(None) };
    }

    /// Главный поток.
    pub fn start(id: u64) -> ShellResult<()> {
        stop_active();
        #[cfg(target_os = "ios")]
        session_category(true)?;
        // SAFETY: AVAudioEngine и SHSession создаются и настраиваются на главном потоке; блок
        // отвода вызывается на потоке звука — matchStreamingBuffer рассчитан на это
        unsafe {
            let session = SHSession::new();
            let delegate = Delegate::new(id);
            session.setDelegate(Some(ProtocolObject::from_ref(&*delegate)));

            let engine = AVAudioEngine::new();
            let input = engine.inputNode();
            let format = input.outputFormatForBus(0);
            if format.sampleRate() <= 0.0 || format.channelCount() == 0 {
                #[cfg(target_os = "ios")]
                let _ = session_category(false);
                return Err(denied("no_microphone", "no audio input"));
            }
            let feed = session.clone();
            let tap = RcBlock::new(
                move |buffer: NonNull<AVAudioPCMBuffer>, time: NonNull<AVAudioTime>| {
                    feed.matchStreamingBuffer_atTime(buffer.as_ref(), Some(time.as_ref()));
                },
            );
            input.installTapOnBus_bufferSize_format_block(
                0,
                TAP_FRAMES,
                Some(&format),
                RcBlock::as_ptr(&tap),
            );
            engine.prepare();
            if let Err(e) = engine.startAndReturnError() {
                input.removeTapOnBus(0);
                #[cfg(target_os = "ios")]
                let _ = session_category(false);
                return Err(ShellError::Internal(format!(
                    "microphone: {}",
                    e.localizedDescription()
                )));
            }
            ACTIVE.with(|a| {
                *a.borrow_mut() = Some(Active {
                    id,
                    engine,
                    session,
                    _delegate: delegate,
                })
            });
        }
        Ok(())
    }

    /// Главный поток. Чужое (более новое) распознавание не трогает.
    pub fn stop(id: u64) {
        let mine = ACTIVE.with(|a| a.borrow().as_ref().is_some_and(|x| x.id == id));
        if mine {
            stop_active();
        }
    }

    fn stop_active() {
        let Some(active) = ACTIVE.with(|a| a.borrow_mut().take()) else {
            return;
        };
        // SAFETY: главный поток, объекты из start
        unsafe {
            active.engine.inputNode().removeTapOnBus(0);
            active.engine.stop();
            active.session.setDelegate(None);
        }
        #[cfg(target_os = "ios")]
        if let Err(e) = session_category(false) {
            log::warn!("recognize: {e}");
        }
    }

    /// iOS: на время распознавания — запись и воспроизведение (вывод движка остаётся открытым и
    /// не уходит в разговорный динамик), потом снова `.playback`, как у движка.
    #[cfg(target_os = "ios")]
    fn session_category(record: bool) -> ShellResult<()> {
        use objc2_avf_audio::{
            AVAudioSession, AVAudioSessionCategoryOptions, AVAudioSessionCategoryPlayAndRecord,
            AVAudioSessionCategoryPlayback, AVAudioSessionModeDefault,
        };
        // SAFETY: общая аудиосессия приложения; константы AVFAudio
        unsafe {
            let session = AVAudioSession::sharedInstance();
            let mode = AVAudioSessionModeDefault.ok_or_else(|| ShellError::internal("no mode"))?;
            let (category, options) = if record {
                (
                    AVAudioSessionCategoryPlayAndRecord,
                    AVAudioSessionCategoryOptions::DefaultToSpeaker
                        | AVAudioSessionCategoryOptions::AllowBluetoothA2DP,
                )
            } else {
                (
                    AVAudioSessionCategoryPlayback,
                    AVAudioSessionCategoryOptions::empty(),
                )
            };
            let category = category.ok_or_else(|| ShellError::internal("no category"))?;
            session
                .setCategory_mode_options_error(category, mode, options)
                .map_err(|e| {
                    ShellError::Internal(format!("audio session: {}", e.localizedDescription()))
                })?;
            session.setActive_error(true).map_err(|e| {
                ShellError::Internal(format!("audio session: {}", e.localizedDescription()))
            })?;
        }
        Ok(())
    }
}

/// iPhone: быстрое действие «Распознать» на значке (UIApplicationShortcutItems в Info.plist).
/// Делегаты приложения и сцены — классы tao; нужные методы добавляются им при запуске.
#[cfg(target_os = "ios")]
mod quick {
    use std::ffi::{CStr, c_char};
    use std::sync::OnceLock;

    use objc2::runtime::{AnyClass, AnyObject, Bool, Imp, Sel};
    use objc2::{ffi, msg_send, sel};
    use tauri::AppHandle;

    /// UIApplicationShortcutItemType в Info.plist (project.yml)
    const SHORTCUT: &str = "rest.daly.wave.recognize";

    static APP: OnceLock<AppHandle> = OnceLock::new();
    /// Исходный `scene:willConnectToSession:options:` делегата сцены tao
    static WILL_CONNECT: OnceLock<usize> = OnceLock::new();

    /// Все три метода — `(self, _cmd, объект, объект, объект)`
    type Handler = unsafe extern "C-unwind" fn(
        *mut AnyObject,
        Sel,
        *mut AnyObject,
        *mut AnyObject,
        *mut AnyObject,
    );

    fn is_ours(item: *mut AnyObject) -> bool {
        if item.is_null() {
            return false;
        }
        // SAFETY: UIApplicationShortcutItem.type — NSString
        let kind: Option<objc2::rc::Retained<objc2_foundation::NSString>> =
            unsafe { msg_send![&*item, type] };
        kind.is_some_and(|k| k.to_string() == SHORTCUT)
    }

    fn fire(item: *mut AnyObject) -> bool {
        if !is_ours(item) {
            return false;
        }
        if let Some(app) = APP.get() {
            super::open(app);
        }
        true
    }

    fn reply(handler: *mut AnyObject, handled: bool) {
        if handler.is_null() {
            return;
        }
        // SAFETY: completionHandler — блок `void (^)(BOOL)` от UIKit
        let block = handler.cast::<block2::DynBlock<dyn Fn(Bool)>>();
        unsafe { (*block).call((Bool::new(handled),)) };
    }

    /// Без сцен: `application:performActionForShortcutItem:completionHandler:` — и на холодном
    /// запуске, и когда приложение уже открыто.
    unsafe extern "C-unwind" fn app_shortcut(
        _this: *mut AnyObject,
        _sel: Sel,
        _app: *mut AnyObject,
        item: *mut AnyObject,
        handler: *mut AnyObject,
    ) {
        reply(handler, fire(item));
    }

    /// Сцены, приложение открыто: `windowScene:performActionForShortcutItem:completionHandler:`.
    unsafe extern "C-unwind" fn scene_shortcut(
        _this: *mut AnyObject,
        _sel: Sel,
        _scene: *mut AnyObject,
        item: *mut AnyObject,
        handler: *mut AnyObject,
    ) {
        reply(handler, fire(item));
    }

    /// Сцены, холодный запуск: действие приходит в параметрах подключения сцены.
    unsafe extern "C-unwind" fn will_connect(
        this: *mut AnyObject,
        sel: Sel,
        scene: *mut AnyObject,
        session: *mut AnyObject,
        options: *mut AnyObject,
    ) {
        if let Some(&original) = WILL_CONNECT.get() {
            // SAFETY: исходная реализация того же селектора с той же сигнатурой
            let original: Handler = unsafe { std::mem::transmute(original) };
            unsafe { original(this, sel, scene, session, options) };
        }
        // После исходного: на нём tao запускает приложение, и APP уже есть
        if !options.is_null() {
            // SAFETY: UISceneConnectionOptions.shortcutItem
            let item: *mut AnyObject = unsafe { msg_send![&*options, shortcutItem] };
            fire(item);
        }
    }

    fn add(class: &AnyClass, sel: Sel, imp: Imp, types: &CStr) {
        // SAFETY: класс зарегистрирован, сигнатура совпадает с types
        let added = unsafe {
            ffi::class_addMethod(
                (class as *const AnyClass).cast_mut(),
                sel,
                imp,
                types.as_ptr() as *const c_char,
            )
        };
        if !added.as_bool() {
            log::warn!(
                "quick action: {} already has {sel}",
                class.name().to_string_lossy()
            );
        }
    }

    pub fn install(app: &AppHandle) {
        let _ = APP.set(app.clone());
        if let Some(delegate) = AnyClass::get(c"AppDelegate") {
            // SAFETY: приведение указателя на функцию к Imp; вызывает её только рантайм ObjC
            let imp: Imp = unsafe { std::mem::transmute::<Handler, Imp>(app_shortcut) };
            add(
                delegate,
                sel!(application:performActionForShortcutItem:completionHandler:),
                imp,
                c"v@:@@@?",
            );
        }
        if let Some(scene) = AnyClass::get(c"TaoSceneDelegate") {
            let imp: Imp = unsafe { std::mem::transmute::<Handler, Imp>(scene_shortcut) };
            add(
                scene,
                sel!(windowScene:performActionForShortcutItem:completionHandler:),
                imp,
                c"v@:@@@?",
            );
            if let Some(method) = scene.instance_method(sel!(scene:willConnectToSession:options:)) {
                let ours: Imp = unsafe { std::mem::transmute::<Handler, Imp>(will_connect) };
                // SAFETY: подмена реализации до UIApplicationMain; исходная вызывается из нашей
                let original =
                    unsafe { ffi::method_setImplementation(method as *const _ as *const _, ours) };
                if let Some(original) = original {
                    let _ = WILL_CONNECT.set(original as usize);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mock_specs() {
        assert_eq!(mock_outcome("1").map(|m| m.artist).ok(), Some("M83".into()));
        let custom = mock_outcome("Кукушка|Кино|RUA111111111").ok();
        assert_eq!(custom.as_ref().map(|m| m.title.as_str()), Some("Кукушка"));
        assert_eq!(
            custom.and_then(|m| m.isrc),
            Some("RUA111111111".to_string())
        );
        assert_eq!(
            mock_outcome("none").err().map(|e| e.code().to_string()),
            Some("no_match".into())
        );
        assert_eq!(
            mock_outcome("permission")
                .err()
                .map(|e| e.code().to_string()),
            Some("permission".into())
        );
    }

    #[test]
    fn late_answers_are_dropped() {
        let (tx, mut rx) = oneshot::channel();
        *CURRENT.lock().unwrap() = Some((42, tx));
        finish(41, Err(ShellError::Cancelled));
        assert!(rx.try_recv().is_err());
        finish(42, Err(ShellError::Cancelled));
        assert!(matches!(rx.try_recv(), Ok(Err(ShellError::Cancelled))));
    }
}
