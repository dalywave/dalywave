//! Интеграция с системой на iOS и macOS через objc2, без Swift.
//!
//! - iOS: AVAudioSession (категория `.playback`, активация), прерывания (звонок, Siri) и смена
//!   маршрута (отключение наушников).
//! - iOS и macOS: «Сейчас играет» (MPNowPlayingInfoCenter) и команды с экрана блокировки,
//!   гарнитуры, Пункта управления, медиаклавиш (MPRemoteCommandCenter).
//!
//! Обработчики команд и уведомлений вызываются на главном потоке; регистрировать их тоже нужно
//! с главного потока.

use std::ptr::NonNull;
use std::sync::Arc;

use block2::RcBlock;
#[cfg(target_os = "ios")]
use objc2::msg_send;
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, NSObjectProtocol, ProtocolObject};
use objc2::{AnyThread, Message};
use objc2_core_foundation::CGSize;
use objc2_foundation::{NSData, NSMutableDictionary, NSNumber, NSString};
use objc2_media_player::{
    MPChangePlaybackPositionCommandEvent, MPMediaItemArtwork, MPMediaItemPropertyAlbumTitle,
    MPMediaItemPropertyArtist, MPMediaItemPropertyArtwork, MPMediaItemPropertyPlaybackDuration,
    MPMediaItemPropertyTitle, MPNowPlayingInfoCenter, MPNowPlayingInfoPropertyDefaultPlaybackRate,
    MPNowPlayingInfoPropertyElapsedPlaybackTime, MPNowPlayingInfoPropertyPlaybackRate,
    MPRemoteCommand, MPRemoteCommandCenter, MPRemoteCommandEvent, MPRemoteCommandHandlerStatus,
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum RemoteCommand {
    Play,
    Pause,
    Toggle,
    Next,
    Previous,
    /// Перемотка с экрана блокировки, секунды.
    Seek(f64),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SessionEvent {
    InterruptionBegan,
    InterruptionEnded {
        should_resume: bool,
    },
    /// Пропало устройство вывода (выдернули наушники) — по правилам Apple нужна пауза.
    HeadphonesUnplugged,
}

#[derive(Clone, Debug, Default)]
pub struct NowPlaying {
    pub title: String,
    pub artist: String,
    pub album: String,
    pub duration_s: Option<f64>,
    pub elapsed_s: f64,
    /// 1.0 — играет, 0.0 — пауза.
    pub rate: f64,
}

fn any<T: Message>(o: &T) -> &AnyObject {
    // SAFETY: любой объект Objective-C — это AnyObject.
    unsafe { &*(o as *const T as *const AnyObject) }
}

/// Обложка для «Сейчас играет». Держите её, пока трек играет: система запрашивает картинку лениво.
pub struct Artwork(Retained<MPMediaItemArtwork>);

// SAFETY: MPMediaItemArtwork неизменяем после создания; используем только для передачи в
// словарь «Сейчас играет».
unsafe impl Send for Artwork {}
unsafe impl Sync for Artwork {}

impl Artwork {
    #[cfg(target_os = "ios")]
    pub fn from_bytes(bytes: &[u8]) -> Option<Artwork> {
        use objc2_ui_kit::UIImage;
        let data = NSData::with_bytes(bytes);
        let image = UIImage::imageWithData(&data)?;
        let keep = image.clone();
        let block =
            RcBlock::new(move |_size: CGSize| -> NonNull<UIImage> { NonNull::from(&*keep) });
        let size = CGSize {
            width: 600.0,
            height: 600.0,
        };
        // В objc2-media-player 0.3 этот инициализатор есть только для macOS — зовём селектор сами.
        let art: Retained<MPMediaItemArtwork> = unsafe {
            msg_send![MPMediaItemArtwork::alloc(), initWithBoundsSize: size, requestHandler: &*block]
        };
        Some(Artwork(art))
    }

    #[cfg(target_os = "macos")]
    pub fn from_bytes(bytes: &[u8]) -> Option<Artwork> {
        use objc2_app_kit::NSImage;
        let data = NSData::with_bytes(bytes);
        let image = NSImage::initWithData(NSImage::alloc(), &data)?;
        let keep = image.clone();
        let block =
            RcBlock::new(move |_size: CGSize| -> NonNull<NSImage> { NonNull::from(&*keep) });
        let size = CGSize {
            width: 600.0,
            height: 600.0,
        };
        let art = unsafe {
            MPMediaItemArtwork::initWithBoundsSize_requestHandler(
                MPMediaItemArtwork::alloc(),
                size,
                &block,
            )
        };
        Some(Artwork(art))
    }
}

/// iOS: категория `.playback` и активация сессии. Звать до создания движка.
#[cfg(target_os = "ios")]
pub fn activate_audio_session() -> Result<(f64, f64), String> {
    use objc2_avf_audio::{
        AVAudioSession, AVAudioSessionCategoryOptions, AVAudioSessionCategoryPlayback,
        AVAudioSessionModeDefault,
    };
    unsafe {
        let session = AVAudioSession::sharedInstance();
        let category = AVAudioSessionCategoryPlayback.ok_or("нет категории playback")?;
        let mode = AVAudioSessionModeDefault.ok_or("нет режима default")?;
        session
            .setCategory_mode_options_error(category, mode, AVAudioSessionCategoryOptions::empty())
            .map_err(|e| format!("setCategory: {}", e.localizedDescription()))?;
        session
            .setActive_error(true)
            .map_err(|e| format!("setActive: {}", e.localizedDescription()))?;
        Ok((session.sampleRate(), session.IOBufferDuration()))
    }
}

/// Держит регистрации команд; при уничтожении цели снимаются.
pub struct RemoteCommands {
    targets: Vec<(Retained<MPRemoteCommand>, Retained<AnyObject>)>,
}

// SAFETY: только хранение и снятие целей при выходе.
unsafe impl Send for RemoteCommands {}
unsafe impl Sync for RemoteCommands {}

impl Drop for RemoteCommands {
    fn drop(&mut self) {
        for (cmd, target) in &self.targets {
            unsafe { cmd.removeTarget(Some(target)) };
        }
    }
}

/// Регистрирует команды play, pause, toggle, next, previous и перемотку. Главный поток.
pub fn register_remote_commands<F>(handler: F) -> RemoteCommands
where
    F: Fn(RemoteCommand) + Send + Sync + 'static,
{
    let handler = Arc::new(handler);
    let mut targets = Vec::new();
    unsafe {
        let center = MPRemoteCommandCenter::sharedCommandCenter();
        let mut add = |cmd: Retained<MPRemoteCommand>,
                       f: Box<
            dyn Fn(NonNull<MPRemoteCommandEvent>) -> MPRemoteCommandHandlerStatus,
        >| {
            let block = RcBlock::new(move |ev: NonNull<MPRemoteCommandEvent>| f(ev));
            cmd.setEnabled(true);
            let target = cmd.addTargetWithHandler(&block);
            targets.push((cmd, target));
        };
        let simple = |c: RemoteCommand| {
            let h = handler.clone();
            Box::new(move |_ev: NonNull<MPRemoteCommandEvent>| {
                h(c);
                MPRemoteCommandHandlerStatus::Success
            })
                as Box<dyn Fn(NonNull<MPRemoteCommandEvent>) -> MPRemoteCommandHandlerStatus>
        };
        add(center.playCommand(), simple(RemoteCommand::Play));
        add(center.pauseCommand(), simple(RemoteCommand::Pause));
        add(
            center.togglePlayPauseCommand(),
            simple(RemoteCommand::Toggle),
        );
        add(center.nextTrackCommand(), simple(RemoteCommand::Next));
        add(
            center.previousTrackCommand(),
            simple(RemoteCommand::Previous),
        );
        let h = handler.clone();
        add(
            Retained::into_super(center.changePlaybackPositionCommand()),
            Box::new(move |ev: NonNull<MPRemoteCommandEvent>| {
                let ev = ev.cast::<MPChangePlaybackPositionCommandEvent>();
                let t = ev.as_ref().positionTime();
                h(RemoteCommand::Seek(t));
                MPRemoteCommandHandlerStatus::Success
            }),
        );
        // Перемотка на ±15 с не нужна: на экране блокировки остаются «назад/вперёд по трекам».
        center.skipForwardCommand().setEnabled(false);
        center.skipBackwardCommand().setEnabled(false);
    }
    RemoteCommands { targets }
}

/// Обновляет «Сейчас играет». Лучше звать с главного потока.
pub fn set_now_playing(np: &NowPlaying, artwork: Option<&Artwork>) {
    unsafe {
        let dict: Retained<NSMutableDictionary<NSString, AnyObject>> = NSMutableDictionary::new();
        dict.insert(
            MPMediaItemPropertyTitle,
            any(&*NSString::from_str(&np.title)),
        );
        dict.insert(
            MPMediaItemPropertyArtist,
            any(&*NSString::from_str(&np.artist)),
        );
        dict.insert(
            MPMediaItemPropertyAlbumTitle,
            any(&*NSString::from_str(&np.album)),
        );
        if let Some(d) = np.duration_s {
            dict.insert(
                MPMediaItemPropertyPlaybackDuration,
                any(&*NSNumber::new_f64(d)),
            );
        }
        dict.insert(
            MPNowPlayingInfoPropertyElapsedPlaybackTime,
            any(&*NSNumber::new_f64(np.elapsed_s)),
        );
        dict.insert(
            MPNowPlayingInfoPropertyPlaybackRate,
            any(&*NSNumber::new_f64(np.rate)),
        );
        dict.insert(
            MPNowPlayingInfoPropertyDefaultPlaybackRate,
            any(&*NSNumber::new_f64(1.0)),
        );
        if let Some(a) = artwork {
            dict.insert(MPMediaItemPropertyArtwork, any(&*a.0));
        }
        let center = MPNowPlayingInfoCenter::defaultCenter();
        center.setNowPlayingInfo(Some(&dict));
        // На macOS состояние воспроизведения нужно сообщать явно, иначе медиаклавиши не работают.
        #[cfg(target_os = "macos")]
        {
            use objc2_media_player::MPNowPlayingPlaybackState;
            center.setPlaybackState(if np.rate > 0.0 {
                MPNowPlayingPlaybackState::Playing
            } else {
                MPNowPlayingPlaybackState::Paused
            });
        }
    }
}

pub fn clear_now_playing() {
    unsafe { MPNowPlayingInfoCenter::defaultCenter().setNowPlayingInfo(None) };
}

/// Что сейчас записано в «Сейчас играет» (для журнала проверки).
pub fn now_playing_description() -> String {
    unsafe {
        let center = MPNowPlayingInfoCenter::defaultCenter();
        let Some(info) = center.nowPlayingInfo() else {
            return "nowPlayingInfo = nil".into();
        };
        let get = |k: &NSString| -> String {
            info.objectForKey(k)
                .map(|v| {
                    if let Ok(s) = v.clone().downcast::<NSString>() {
                        s.to_string()
                    } else if let Ok(n) = v.downcast::<NSNumber>() {
                        format!("{:.2}", n.doubleValue())
                    } else {
                        "<obj>".into()
                    }
                })
                .unwrap_or_else(|| "-".into())
        };
        format!(
            "title={} artist={} duration={} elapsed={} rate={} artwork={}",
            get(MPMediaItemPropertyTitle),
            get(MPMediaItemPropertyArtist),
            get(MPMediaItemPropertyPlaybackDuration),
            get(MPNowPlayingInfoPropertyElapsedPlaybackTime),
            get(MPNowPlayingInfoPropertyPlaybackRate),
            if info.objectForKey(MPMediaItemPropertyArtwork).is_some() {
                "yes"
            } else {
                "no"
            }
        )
    }
}

/// Наблюдатели уведомлений аудиосессии.
pub struct SessionObservers {
    observers: Vec<Retained<ProtocolObject<dyn NSObjectProtocol>>>,
}

unsafe impl Send for SessionObservers {}
unsafe impl Sync for SessionObservers {}

impl Drop for SessionObservers {
    fn drop(&mut self) {
        let nc = objc2_foundation::NSNotificationCenter::defaultCenter();
        for o in &self.observers {
            unsafe { nc.removeObserver(any(&**o)) };
        }
    }
}

#[cfg(target_os = "ios")]
fn user_info_usize(n: &objc2_foundation::NSNotification, key: Option<&NSString>) -> Option<usize> {
    let info = n.userInfo()?;
    let v = info.objectForKey(key?)?;
    Some(v.downcast::<NSNumber>().ok()?.unsignedIntegerValue())
}

/// iOS: прерывания и смена маршрута. Обработчик вызывается на главном потоке.
#[cfg(target_os = "ios")]
pub fn observe_session<F>(handler: F) -> SessionObservers
where
    F: Fn(SessionEvent) + Send + Sync + 'static,
{
    use objc2_avf_audio::{
        AVAudioSessionInterruptionNotification, AVAudioSessionInterruptionOptionKey,
        AVAudioSessionInterruptionTypeKey, AVAudioSessionRouteChangeNotification,
        AVAudioSessionRouteChangeReasonKey,
    };
    use objc2_foundation::{NSNotification, NSNotificationCenter, NSOperationQueue};

    let handler = Arc::new(handler);
    let nc = NSNotificationCenter::defaultCenter();
    let queue = NSOperationQueue::mainQueue();
    let mut observers = Vec::new();
    unsafe {
        let h = handler.clone();
        let block = RcBlock::new(move |n: NonNull<NSNotification>| {
            let n = n.as_ref();
            match user_info_usize(n, AVAudioSessionInterruptionTypeKey) {
                Some(1) => h(SessionEvent::InterruptionBegan),
                Some(0) => {
                    let opts = user_info_usize(n, AVAudioSessionInterruptionOptionKey).unwrap_or(0);
                    h(SessionEvent::InterruptionEnded {
                        should_resume: opts & 1 == 1,
                    });
                }
                _ => {}
            }
        });
        if let Some(name) = AVAudioSessionInterruptionNotification {
            observers.push(nc.addObserverForName_object_queue_usingBlock(
                Some(name),
                None,
                Some(&queue),
                &block,
            ));
        }
        let h = handler.clone();
        let block = RcBlock::new(move |n: NonNull<NSNotification>| {
            // 2 = AVAudioSessionRouteChangeReasonOldDeviceUnavailable.
            if user_info_usize(n.as_ref(), AVAudioSessionRouteChangeReasonKey) == Some(2) {
                h(SessionEvent::HeadphonesUnplugged);
            }
        });
        if let Some(name) = AVAudioSessionRouteChangeNotification {
            observers.push(nc.addObserverForName_object_queue_usingBlock(
                Some(name),
                None,
                Some(&queue),
                &block,
            ));
        }
    }
    SessionObservers { observers }
}

#[cfg(target_os = "ios")]
fn untyped(
    d: &objc2_foundation::NSDictionary<NSString, AnyObject>,
) -> &objc2_foundation::NSDictionary {
    // SAFETY: обобщённые параметры NSDictionary существуют только на уровне Rust.
    unsafe {
        &*(d as *const objc2_foundation::NSDictionary<NSString, AnyObject>
            as *const objc2_foundation::NSDictionary)
    }
}

/// Проверка без звонка: публикует такое же уведомление, как система (обработчики cpal тоже
/// его получают — останавливают и возобновляют поток вывода).
#[cfg(target_os = "ios")]
pub fn simulate_interruption(began: bool, should_resume: bool) {
    use objc2_avf_audio::{
        AVAudioSessionInterruptionNotification, AVAudioSessionInterruptionOptionKey,
        AVAudioSessionInterruptionTypeKey,
    };
    use objc2_foundation::{NSDictionary, NSNotificationCenter};
    unsafe {
        let (Some(name), Some(tk), Some(ok)) = (
            AVAudioSessionInterruptionNotification,
            AVAudioSessionInterruptionTypeKey,
            AVAudioSessionInterruptionOptionKey,
        ) else {
            return;
        };
        let t = NSNumber::new_usize(if began { 1 } else { 0 });
        let o = NSNumber::new_usize(if should_resume { 1 } else { 0 });
        let info: Retained<NSDictionary<NSString, AnyObject>> =
            NSDictionary::from_slices(&[tk, ok], &[any(&*t), any(&*o)]);
        NSNotificationCenter::defaultCenter().postNotificationName_object_userInfo(
            name,
            None,
            Some(untyped(&info)),
        );
    }
}

#[cfg(target_os = "ios")]
pub fn simulate_headphones_unplugged() {
    use objc2_avf_audio::{
        AVAudioSessionRouteChangeNotification, AVAudioSessionRouteChangeReasonKey,
    };
    use objc2_foundation::{NSDictionary, NSNotificationCenter};
    unsafe {
        let (Some(name), Some(rk)) = (
            AVAudioSessionRouteChangeNotification,
            AVAudioSessionRouteChangeReasonKey,
        ) else {
            return;
        };
        let r = NSNumber::new_usize(2);
        let info: Retained<NSDictionary<NSString, AnyObject>> =
            NSDictionary::from_slices(&[rk], &[any(&*r)]);
        NSNotificationCenter::defaultCenter().postNotificationName_object_userInfo(
            name,
            None,
            Some(untyped(&info)),
        );
    }
}

/// iOS: уход приложения в фон и возврат (для журнала проверки фонового воспроизведения).
#[cfg(target_os = "ios")]
pub fn observe_app_lifecycle<F>(handler: F) -> SessionObservers
where
    F: Fn(bool) + Send + Sync + 'static,
{
    use objc2_foundation::{NSNotification, NSNotificationCenter, NSOperationQueue};
    let handler = Arc::new(handler);
    let nc = NSNotificationCenter::defaultCenter();
    let queue = NSOperationQueue::mainQueue();
    let mut observers = Vec::new();
    for (name, background) in [
        ("UIApplicationDidEnterBackgroundNotification", true),
        ("UIApplicationWillEnterForegroundNotification", false),
    ] {
        let h = handler.clone();
        let block = RcBlock::new(move |_n: NonNull<NSNotification>| h(background));
        let name = NSString::from_str(name);
        observers.push(unsafe {
            nc.addObserverForName_object_queue_usingBlock(Some(&name), None, Some(&queue), &block)
        });
    }
    SessionObservers { observers }
}
