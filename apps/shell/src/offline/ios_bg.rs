//! iPhone и iPad: загрузки файлов фоновой сессией URLSession.
//!
//! Файлы качает сама система (процесс `nsurlsessiond`): загрузка продолжается, когда приложение
//! усыплено или выгружено, а прерванная — с места обрыва (resume data). Закончив, iOS будит приложение
//! в фоне и вызывает `application:handleEventsForBackgroundURLSession:completionHandler:` — этот метод
//! добавляется делегату приложения Tao при запуске (`class_addMethod`), Swift для этого не нужен.
//!
//! Куда положить файл, записано в `taskDescription` задачи («id трека|путь»): после перезапуска памяти
//! приложения нет, а задача помнит. Готовый файл перемещается в `<путь>.part` прямо в делегате (система
//! удаляет временный файл сразу после вызова), проверка размера и хеша и учёт в базе — в `mod.rs`.

use std::{
    collections::HashSet,
    ffi::c_char,
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
    time::{Duration, Instant},
};

use block2::{Block, RcBlock};
use objc2::{
    AnyThread, define_class, msg_send,
    rc::Retained,
    runtime::{AnyClass, AnyObject, Bool, Imp, NSObject, NSObjectProtocol, Sel},
    sel,
};
use objc2_foundation::{NSString, NSURL};
use tauri::AppHandle;
use tokio::sync::mpsc;
use uuid::Uuid;

/// Идентификатор фоновой сессии: по нему iOS возвращает приложению события после перезапуска.
const SESSION_ID: &str = "rest.daly.wave.downloads";
/// `NSURLErrorCancelled`
const CANCELLED: isize = -999;
const PROGRESS_EVERY: Duration = Duration::from_millis(500);

pub enum Event {
    Progress {
        track_id: Uuid,
        received: u64,
        total: u64,
    },
    /// Файл лежит в `<путь>.part`: проверить и переименовать.
    Finished {
        track_id: Uuid,
    },
    Failed {
        track_id: Uuid,
        error: String,
    },
}

/// Объект Objective-C, которым можно пользоваться из любого потока (NSURLSession потокобезопасна).
struct Shared(Retained<AnyObject>);
// SAFETY: NSURLSession и её задачи документированы как потокобезопасные
unsafe impl Send for Shared {}
unsafe impl Sync for Shared {}

/// Блок завершения из `handleEventsForBackgroundURLSession`: вызвать, когда события кончатся.
struct Completion(RcBlock<dyn Fn()>);
// SAFETY: блок только хранится и вызывается на главном потоке (через главную очередь)
unsafe impl Send for Completion {}

static SESSION: OnceLock<Shared> = OnceLock::new();
static TX: OnceLock<mpsc::UnboundedSender<Event>> = OnceLock::new();
static RX: Mutex<Option<mpsc::UnboundedReceiver<Event>>> = Mutex::new(None);
static IN_FLIGHT: Mutex<Option<HashSet<Uuid>>> = Mutex::new(None);
static COMPLETION: Mutex<Option<Completion>> = Mutex::new(None);
/// Ответ сервера с ошибкой: файл не перемещается, задача сообщает о сбое при завершении.
static HTTP_FAILED: Mutex<Option<std::collections::HashMap<Uuid, isize>>> = Mutex::new(None);
static LAST_PROGRESS: Mutex<Option<std::collections::HashMap<Uuid, Instant>>> = Mutex::new(None);

fn send(ev: Event) {
    if let Some(tx) = TX.get() {
        let _ = tx.send(ev);
    }
}

fn in_flight_mut<R>(f: impl FnOnce(&mut HashSet<Uuid>) -> R) -> R {
    let mut g = IN_FLIGHT.lock().unwrap_or_else(|e| e.into_inner());
    f(g.get_or_insert_with(HashSet::new))
}

/// Что сейчас качает система.
pub fn in_flight() -> HashSet<Uuid> {
    in_flight_mut(|s| s.clone())
}

/// События загрузок для цикла в `mod.rs` (забирается один раз).
pub fn events() -> mpsc::UnboundedReceiver<Event> {
    RX.lock()
        .unwrap_or_else(|e| e.into_inner())
        .take()
        .unwrap_or_else(|| mpsc::unbounded_channel().1)
}

/// «id трека|путь» из `taskDescription`.
fn describe(task: *mut AnyObject) -> Option<(Uuid, PathBuf)> {
    if task.is_null() {
        return None;
    }
    // SAFETY: task — живой NSURLSessionTask от системы
    let desc: Option<Retained<NSString>> = unsafe { msg_send![task, taskDescription] };
    let desc = desc?.to_string();
    let (id, path) = desc.split_once('|')?;
    Some((id.parse().ok()?, PathBuf::from(path)))
}

fn part(dest: &Path) -> PathBuf {
    dalywave_downloads::part_path(dest)
}

define_class!(
    // Делегат фоновой сессии: события приходят на её служебной очереди
    #[unsafe(super(NSObject))]
    #[name = "DalyWaveDownloadDelegate"]
    struct Delegate;

    unsafe impl NSObjectProtocol for Delegate {}

    impl Delegate {
        #[unsafe(method(URLSession:downloadTask:didFinishDownloadingToURL:))]
        fn did_finish(&self, _session: *mut AnyObject, task: *mut AnyObject, location: *mut NSURL) {
            let Some((track_id, dest)) = describe(task) else { return };
            // SAFETY: response — NSHTTPURLResponse у задач http(s)
            let status: isize = unsafe {
                let res: *mut AnyObject = msg_send![task, response];
                if res.is_null() { 0 } else { msg_send![res, statusCode] }
            };
            if !(200..300).contains(&status) {
                HTTP_FAILED
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .get_or_insert_with(Default::default)
                    .insert(track_id, status);
                return;
            }
            // SAFETY: location — живой NSURL временного файла
            let src = unsafe { location.as_ref() }.and_then(|u| u.path()).map(|p| p.to_string());
            let Some(src) = src else { return };
            let to = part(&dest);
            if let Some(dir) = to.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            let _ = std::fs::remove_file(&to);
            let moved = std::fs::rename(&src, &to)
                .or_else(|_| std::fs::copy(&src, &to).map(|_| ()));
            match moved {
                Ok(()) => send(Event::Finished { track_id }),
                Err(e) => send(Event::Failed { track_id, error: format!("файл не перенести: {e}") }),
            }
        }

        #[unsafe(method(URLSession:downloadTask:didWriteData:totalBytesWritten:totalBytesExpectedToWrite:))]
        fn did_write(&self, _session: *mut AnyObject, task: *mut AnyObject, _written: i64, total: i64, expected: i64) {
            let Some((track_id, _)) = describe(task) else { return };
            let mut last = LAST_PROGRESS.lock().unwrap_or_else(|e| e.into_inner());
            let map = last.get_or_insert_with(Default::default);
            if map.get(&track_id).is_some_and(|t| t.elapsed() < PROGRESS_EVERY) {
                return;
            }
            map.insert(track_id, Instant::now());
            send(Event::Progress { track_id, received: total.max(0) as u64, total: expected.max(0) as u64 });
        }

        #[unsafe(method(URLSession:task:didCompleteWithError:))]
        fn did_complete(&self, _session: *mut AnyObject, task: *mut AnyObject, error: *mut AnyObject) {
            let Some((track_id, dest)) = describe(task) else { return };
            in_flight_mut(|s| s.remove(&track_id));
            let http = HTTP_FAILED
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .get_or_insert_with(Default::default)
                .remove(&track_id);
            if error.is_null() {
                if let Some(code) = http {
                    send(Event::Failed { track_id, error: format!("HTTP {code}") });
                }
                return;
            }
            // SAFETY: error — живой NSError
            unsafe {
                let code: isize = msg_send![error, code];
                let text: Option<Retained<NSString>> = msg_send![error, localizedDescription];
                // Прерванное продолжится с места обрыва: система отдаёт resume data
                let info: *mut AnyObject = msg_send![error, userInfo];
                if !info.is_null() {
                    let key = NSString::from_str("NSURLSessionDownloadTaskResumeData");
                    let data: *mut AnyObject = msg_send![info, objectForKey: &*key];
                    if !data.is_null() {
                        let len: usize = msg_send![data, length];
                        let bytes: *const u8 = msg_send![data, bytes];
                        if !bytes.is_null() && len > 0 {
                            let slice = std::slice::from_raw_parts(bytes, len);
                            let _ = std::fs::write(dest.with_extension("resume"), slice);
                        }
                    }
                }
                let error = if code == CANCELLED {
                    "остановлено".to_string()
                } else {
                    text.map(|t| t.to_string()).unwrap_or_else(|| format!("ошибка {code}"))
                };
                send(Event::Failed { track_id, error });
            }
        }

        #[unsafe(method(URLSessionDidFinishEventsForBackgroundURLSession:))]
        fn did_finish_events(&self, _session: *mut AnyObject) {
            log::info!("offline: background session events delivered");
            let Some(done) = COMPLETION.lock().unwrap_or_else(|e| e.into_inner()).take() else { return };
            // Блок завершения — на главном потоке, как требует UIKit
            let block = RcBlock::new(move || done.0.call(()));
            // SAFETY: NSOperationQueue.mainQueue и addOperationWithBlock: — публичный API
            unsafe {
                let Some(cls) = AnyClass::get(c"NSOperationQueue") else { return };
                let main: *mut AnyObject = msg_send![cls, mainQueue];
                let _: () = msg_send![main, addOperationWithBlock: &*block];
            }
        }
    }
);

impl Delegate {
    fn new() -> Retained<Self> {
        let this = Self::alloc().set_ivars(());
        // SAFETY: обычная инициализация NSObject
        unsafe { msg_send![super(this), init] }
    }
}

/// iOS будит приложение ради событий фоновой сессии: запомнить блок завершения.
unsafe extern "C-unwind" fn handle_events(
    _this: *mut AnyObject,
    _cmd: Sel,
    _app: *mut AnyObject,
    identifier: *mut NSString,
    handler: *mut Block<dyn Fn()>,
) {
    // SAFETY: система передаёт живые идентификатор и блок; блок копируется (+1)
    let id = unsafe { identifier.as_ref() }
        .map(|s| s.to_string())
        .unwrap_or_default();
    log::info!("offline: woken for background session {id}");
    if let Some(block) = unsafe { RcBlock::copy(handler) } {
        *COMPLETION.lock().unwrap_or_else(|e| e.into_inner()) = Some(Completion(block));
    }
}

/// Добавить делегату приложения метод фоновых загрузок. Главный поток.
fn hook_app_delegate() {
    // SAFETY: UIApplication.sharedApplication.delegate — объект Tao; метод добавляется в его класс
    unsafe {
        let Some(app_cls) = AnyClass::get(c"UIApplication") else {
            return;
        };
        let app: *mut AnyObject = msg_send![app_cls, sharedApplication];
        if app.is_null() {
            return;
        }
        let delegate: *mut AnyObject = msg_send![app, delegate];
        let Some(delegate) = delegate.as_ref() else {
            log::warn!("offline: no app delegate to receive background downloads");
            return;
        };
        let cls = delegate.class() as *const AnyClass as *mut AnyClass;
        let imp: Imp = std::mem::transmute::<
            unsafe extern "C-unwind" fn(
                *mut AnyObject,
                Sel,
                *mut AnyObject,
                *mut NSString,
                *mut Block<dyn Fn()>,
            ),
            Imp,
        >(handle_events);
        let types: *const c_char = c"v@:@@@?".as_ptr();
        let added = objc2::ffi::class_addMethod(
            cls,
            sel!(application:handleEventsForBackgroundURLSession:completionHandler:),
            imp,
            types,
        );
        log::info!(
            "offline: background session handler {}",
            if added.as_bool() {
                "installed"
            } else {
                "already present"
            }
        );
    }
}

/// Создать фоновую сессию (её события после перезапуска приходят, как только она создана) и узнать,
/// что система уже качает. Главный поток, при запуске.
pub fn init(_app: &AppHandle) {
    let (tx, rx) = mpsc::unbounded_channel();
    let _ = TX.set(tx);
    *RX.lock().unwrap_or_else(|e| e.into_inner()) = Some(rx);
    hook_app_delegate();
    // SAFETY: публичный API Foundation; делегат сессия держит сама до invalidate
    unsafe {
        let (Some(cfg_cls), Some(session_cls)) = (
            AnyClass::get(c"NSURLSessionConfiguration"),
            AnyClass::get(c"NSURLSession"),
        ) else {
            return;
        };
        let id = NSString::from_str(SESSION_ID);
        let cfg: Retained<AnyObject> =
            msg_send![cfg_cls, backgroundSessionConfigurationWithIdentifier: &*id];
        let _: () = msg_send![&*cfg, setSessionSendsLaunchEvents: true];
        let _: () = msg_send![&*cfg, setDiscretionary: false];
        let delegate = Delegate::new();
        let session: Retained<AnyObject> = msg_send![
            session_cls,
            sessionWithConfiguration: &*cfg,
            delegate: &*delegate,
            delegateQueue: std::ptr::null::<AnyObject>()
        ];
        // Задачи, начатые до перезапуска: их не начинать заново
        let block = RcBlock::new(|tasks: *mut AnyObject| {
            if tasks.is_null() {
                return;
            }
            let count: usize = msg_send![tasks, count];
            for i in 0..count {
                let task: *mut AnyObject = msg_send![tasks, objectAtIndex: i];
                if let Some((id, _)) = describe(task) {
                    in_flight_mut(|s| s.insert(id));
                }
            }
            log::info!("offline: {count} background downloads in progress");
        });
        let _: () = msg_send![&*session, getAllTasksWithCompletionHandler: &*block];
        let _ = SESSION.set(Shared(session));
    }
}

/// Начать (или продолжить с места обрыва) загрузку файла трека в `<dest>.part`.
pub fn start(track_id: Uuid, url: &str, part_dest: &Path, allow_cellular: bool) {
    let Some(session) = SESSION.get() else { return };
    // part_dest — `<путь>.part`; в описании задачи — итоговый путь
    let dest = part_dest.with_extension("");
    let resume = dest.with_extension("resume");
    // SAFETY: публичный API Foundation на потокобезопасной сессии
    unsafe {
        let task: Option<Retained<AnyObject>> = match std::fs::read(&resume) {
            Ok(bytes) if !bytes.is_empty() => {
                let _ = std::fs::remove_file(&resume);
                let Some(data_cls) = AnyClass::get(c"NSData") else {
                    return;
                };
                let data: Retained<AnyObject> =
                    msg_send![data_cls, dataWithBytes: bytes.as_ptr(), length: bytes.len()];
                msg_send![&*session.0, downloadTaskWithResumeData: &*data]
            }
            _ => {
                let Some(ns_url) = NSURL::URLWithString(&NSString::from_str(url)) else {
                    return;
                };
                let Some(req_cls) = AnyClass::get(c"NSMutableURLRequest") else {
                    return;
                };
                let req: Retained<AnyObject> = msg_send![req_cls, requestWithURL: &*ns_url];
                let _: () = msg_send![&*req, setAllowsCellularAccess: allow_cellular];
                msg_send![&*session.0, downloadTaskWithRequest: &*req]
            }
        };
        let Some(task) = task else {
            send(Event::Failed {
                track_id,
                error: "задача не создалась".into(),
            });
            return;
        };
        let desc = NSString::from_str(&format!("{track_id}|{}", dest.display()));
        let _: () = msg_send![&*task, setTaskDescription: &*desc];
        let _: () = msg_send![&*task, resume];
    }
    in_flight_mut(|s| s.insert(track_id));
    log::info!("offline: background download of {track_id} started");
}

/// Остановить загрузку трека (откреплён, вытеснен, «Удалить всё»).
pub fn cancel(track_id: Uuid) {
    let Some(session) = SESSION.get() else { return };
    if !in_flight().contains(&track_id) {
        return;
    }
    let block = RcBlock::new(move |tasks: *mut AnyObject| {
        if tasks.is_null() {
            return;
        }
        // SAFETY: массив задач от системы
        unsafe {
            let count: usize = msg_send![tasks, count];
            for i in 0..count {
                let task: *mut AnyObject = msg_send![tasks, objectAtIndex: i];
                if describe(task).is_some_and(|(id, _)| id == track_id) {
                    let _: () = msg_send![task, cancel];
                }
            }
        }
    });
    // SAFETY: публичный API на потокобезопасной сессии
    unsafe {
        let _: () = msg_send![&*session.0, getAllTasksWithCompletionHandler: &*block];
    }
    in_flight_mut(|s| s.remove(&track_id));
}

/// Папка загрузок не уходит в резервную копию iCloud: это кэш, его можно скачать заново.
pub fn exclude_from_backup(dir: &Path) {
    let Some(path) = dir.to_str() else { return };
    // SAFETY: NSURL fileURLWithPath и setResourceValue:forKey:error: — публичный API
    unsafe {
        let url = NSURL::fileURLWithPath(&NSString::from_str(path));
        let Some(num) = AnyClass::get(c"NSNumber") else {
            return;
        };
        let yes: Retained<AnyObject> = msg_send![num, numberWithBool: true];
        let key = NSString::from_str("NSURLIsExcludedFromBackupKey");
        let ok: Bool = msg_send![
            &*url,
            setResourceValue: &*yes,
            forKey: &*key,
            error: std::ptr::null_mut::<*mut AnyObject>()
        ];
        if !ok.as_bool() {
            log::warn!("offline: could not exclude downloads from iCloud backup");
        }
    }
}
