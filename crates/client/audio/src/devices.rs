//! Устройства вывода: список для выбора «куда идёт звук» и слежение за его переменами.
//!
//! Идентификатор устройства — `cpal::DeviceId` строкой (`coreaudio:<UID>`, `wasapi:<id>`): он
//! не меняется между запусками, поэтому выбор можно запомнить. Тип подключения cpal на macOS не
//! сообщает (кроме составных устройств) — берём его из CoreAudio: значок AirPlay в интерфейсе
//! важнее всего.

use cpal::traits::{DeviceTrait, HostTrait};

use crate::AudioError;

/// Как устройство подключено — для значка в интерфейсе.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutputKind {
    BuiltIn,
    Headphones,
    Bluetooth,
    AirPlay,
    Usb,
    Hdmi,
    Other,
}

impl OutputKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::BuiltIn => "builtin",
            Self::Headphones => "headphones",
            Self::Bluetooth => "bluetooth",
            Self::AirPlay => "airplay",
            Self::Usb => "usb",
            Self::Hdmi => "hdmi",
            Self::Other => "other",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OutputDevice {
    /// `cpal::DeviceId` строкой — для [`crate::Engine::set_output_device`]
    pub id: String,
    /// Имя, как в настройках звука системы
    pub name: String,
    pub kind: OutputKind,
    /// Устройство вывода по умолчанию в системе
    pub is_default: bool,
}

/// Устройство вывода по умолчанию в системе.
pub fn default_output_device_id() -> Option<String> {
    let device = cpal::default_host().default_output_device()?;
    device.id().ok().map(|id| id.to_string())
}

/// Устройства вывода системы в её порядке — те же, что в настройках звука: на macOS здесь и
/// колонки AirPlay, которые система сейчас видит в сети, а служебные устройства (скрытые и те, что
/// не могут быть выходом по умолчанию, — «микрофоны» программ трансляции) пропущены.
pub fn output_devices() -> Result<Vec<OutputDevice>, AudioError> {
    let host = cpal::default_host();
    let default = default_output_device_id();
    let devices = host
        .output_devices()
        .map_err(|e| AudioError::Output(e.to_string()))?;
    let mut out = Vec::new();
    for device in devices {
        // Устройство могло пропасть, пока список читался: его просто нет в ответе
        let (Ok(id), Ok(desc)) = (device.id(), device.description()) else {
            continue;
        };
        let id = id.to_string();
        let is_default = default.as_deref() == Some(id.as_str());
        if !is_default && !selectable(&id) {
            continue;
        }
        let kind = platform_kind(&id).unwrap_or_else(|| kind_from_cpal(&desc));
        out.push(OutputDevice {
            is_default,
            name: desc.name().to_string(),
            kind,
            id,
        });
    }
    Ok(out)
}

fn kind_from_cpal(desc: &cpal::DeviceDescription) -> OutputKind {
    use cpal::{DeviceType, InterfaceType};
    if matches!(
        desc.device_type(),
        DeviceType::Headphones | DeviceType::Headset
    ) && !matches!(desc.interface_type(), InterfaceType::Bluetooth)
    {
        return OutputKind::Headphones;
    }
    match desc.interface_type() {
        InterfaceType::BuiltIn => OutputKind::BuiltIn,
        InterfaceType::Bluetooth => OutputKind::Bluetooth,
        InterfaceType::Usb => OutputKind::Usb,
        InterfaceType::Hdmi | InterfaceType::DisplayPort => OutputKind::Hdmi,
        _ => OutputKind::Other,
    }
}

#[cfg(not(target_os = "macos"))]
fn platform_kind(_id: &str) -> Option<OutputKind> {
    None
}

#[cfg(not(target_os = "macos"))]
fn selectable(_id: &str) -> bool {
    true
}

#[cfg(target_os = "macos")]
fn platform_kind(id: &str) -> Option<OutputKind> {
    macos::kind_by_uid(id.strip_prefix("coreaudio:")?)
}

#[cfg(target_os = "macos")]
fn selectable(id: &str) -> bool {
    id.strip_prefix("coreaudio:")
        .is_none_or(macos::selectable_by_uid)
}

#[cfg(target_os = "macos")]
pub use macos::{DeviceWatch, watch_output_devices};

#[cfg(target_os = "macos")]
mod macos {
    use std::ffi::c_void;
    use std::ptr::NonNull;

    use objc2_core_audio::{
        AudioDeviceID, AudioObjectAddPropertyListener, AudioObjectGetPropertyData, AudioObjectID,
        AudioObjectPropertyAddress, AudioObjectPropertySelector, AudioObjectRemovePropertyListener,
        kAudioDevicePropertyDataSource, kAudioDevicePropertyDeviceCanBeDefaultDevice,
        kAudioDevicePropertyIsHidden, kAudioDevicePropertyTransportType,
        kAudioDeviceTransportTypeAirPlay, kAudioDeviceTransportTypeBluetooth,
        kAudioDeviceTransportTypeBluetoothLE, kAudioDeviceTransportTypeBuiltIn,
        kAudioDeviceTransportTypeDisplayPort, kAudioDeviceTransportTypeHDMI,
        kAudioDeviceTransportTypeUSB, kAudioHardwarePropertyDefaultOutputDevice,
        kAudioHardwarePropertyDevices, kAudioHardwarePropertyTranslateUIDToDevice,
        kAudioObjectPropertyElementMain, kAudioObjectPropertyScopeGlobal,
        kAudioObjectPropertyScopeOutput, kAudioObjectSystemObject, kAudioObjectUnknown,
    };
    use objc2_core_foundation::CFString;

    use super::OutputKind;
    use crate::AudioError;

    /// Источник встроенного выхода — гнездо наушников (`'hdpn'`), а не динамики (`'ispk'`)
    const DATA_SOURCE_HEADPHONES: u32 = u32::from_be_bytes(*b"hdpn");

    fn address(selector: AudioObjectPropertySelector, scope: u32) -> AudioObjectPropertyAddress {
        AudioObjectPropertyAddress {
            mSelector: selector,
            mScope: scope,
            mElement: kAudioObjectPropertyElementMain,
        }
    }

    fn get_u32(object: AudioObjectID, addr: &AudioObjectPropertyAddress) -> Option<u32> {
        let mut value = 0u32;
        let mut size = size_of::<u32>() as u32;
        // SAFETY: свойство — UInt32, буфер под него, размер передан; статус проверяется
        let status = unsafe {
            AudioObjectGetPropertyData(
                object,
                NonNull::from(addr),
                0,
                std::ptr::null(),
                NonNull::from(&mut size),
                NonNull::from(&mut value).cast(),
            )
        };
        (status == 0).then_some(value)
    }

    fn device_by_uid(uid: &str) -> Option<AudioDeviceID> {
        let uid = CFString::from_str(uid);
        let uid_ref: *const CFString = &*uid;
        let addr = address(
            kAudioHardwarePropertyTranslateUIDToDevice,
            kAudioObjectPropertyScopeGlobal,
        );
        let mut device: AudioDeviceID = kAudioObjectUnknown;
        let mut size = size_of::<AudioDeviceID>() as u32;
        // SAFETY: квалификатор — CFStringRef (живёт до конца вызова), ответ — AudioObjectID
        let status = unsafe {
            AudioObjectGetPropertyData(
                kAudioObjectSystemObject as AudioObjectID,
                NonNull::from(&addr),
                size_of::<*const CFString>() as u32,
                (&uid_ref as *const *const CFString).cast(),
                NonNull::from(&mut size),
                NonNull::from(&mut device).cast(),
            )
        };
        (status == 0 && device != kAudioObjectUnknown).then_some(device)
    }

    /// Как в настройках звука macOS: не скрыто и может быть выходом по умолчанию.
    pub(super) fn selectable_by_uid(uid: &str) -> bool {
        let Some(device) = device_by_uid(uid) else {
            return false;
        };
        let hidden = get_u32(
            device,
            &address(
                kAudioDevicePropertyIsHidden,
                kAudioObjectPropertyScopeGlobal,
            ),
        );
        let can_be_default = get_u32(
            device,
            &address(
                kAudioDevicePropertyDeviceCanBeDefaultDevice,
                kAudioObjectPropertyScopeOutput,
            ),
        );
        hidden != Some(1) && can_be_default != Some(0)
    }

    pub(super) fn kind_by_uid(uid: &str) -> Option<OutputKind> {
        let device = device_by_uid(uid)?;
        let transport = get_u32(
            device,
            &address(
                kAudioDevicePropertyTransportType,
                kAudioObjectPropertyScopeGlobal,
            ),
        )?;
        Some(match transport {
            t if t == kAudioDeviceTransportTypeAirPlay => OutputKind::AirPlay,
            t if t == kAudioDeviceTransportTypeBluetooth
                || t == kAudioDeviceTransportTypeBluetoothLE =>
            {
                OutputKind::Bluetooth
            }
            t if t == kAudioDeviceTransportTypeUSB => OutputKind::Usb,
            t if t == kAudioDeviceTransportTypeHDMI
                || t == kAudioDeviceTransportTypeDisplayPort =>
            {
                OutputKind::Hdmi
            }
            t if t == kAudioDeviceTransportTypeBuiltIn => {
                let source = get_u32(
                    device,
                    &address(
                        kAudioDevicePropertyDataSource,
                        kAudioObjectPropertyScopeOutput,
                    ),
                );
                if source == Some(DATA_SOURCE_HEADPHONES) {
                    OutputKind::Headphones
                } else {
                    OutputKind::BuiltIn
                }
            }
            _ => OutputKind::Other,
        })
    }

    type Callback = Box<dyn Fn() + Send + Sync>;

    /// Слежение за списком устройств и устройством по умолчанию; снимается при уничтожении.
    pub struct DeviceWatch {
        callback: *mut Callback,
    }

    // SAFETY: колбэк Send + Sync, указатель только передаётся CoreAudio и освобождается в drop
    unsafe impl Send for DeviceWatch {}
    unsafe impl Sync for DeviceWatch {}

    const WATCHED: [AudioObjectPropertySelector; 2] = [
        kAudioHardwarePropertyDevices,
        kAudioHardwarePropertyDefaultOutputDevice,
    ];

    unsafe extern "C-unwind" fn on_change(
        _object: AudioObjectID,
        _count: u32,
        _addresses: NonNull<AudioObjectPropertyAddress>,
        data: *mut c_void,
    ) -> i32 {
        // SAFETY: data — указатель из DeviceWatch, живой, пока слушатель зарегистрирован
        let callback = unsafe { &*(data as *const Callback) };
        callback();
        0
    }

    /// Звать `on_change`, когда устройства появляются, пропадают или меняется устройство по
    /// умолчанию. Колбэк приходит в потоке CoreAudio и может прийти пачкой — тяжёлую работу
    /// стоит отложить.
    pub fn watch_output_devices(
        on_change_cb: impl Fn() + Send + Sync + 'static,
    ) -> Result<DeviceWatch, AudioError> {
        let callback: *mut Callback = Box::into_raw(Box::new(Box::new(on_change_cb)));
        let watch = DeviceWatch { callback };
        for selector in WATCHED {
            let addr = address(selector, kAudioObjectPropertyScopeGlobal);
            // SAFETY: адрес живёт до конца вызова, data — до снятия слушателя в drop
            let status = unsafe {
                AudioObjectAddPropertyListener(
                    kAudioObjectSystemObject as AudioObjectID,
                    NonNull::from(&addr),
                    Some(on_change),
                    callback.cast(),
                )
            };
            if status != 0 {
                return Err(AudioError::Output(format!(
                    "CoreAudio listener {selector:#x}: {status}"
                )));
            }
        }
        Ok(watch)
    }

    impl Drop for DeviceWatch {
        fn drop(&mut self) {
            for selector in WATCHED {
                let addr = address(selector, kAudioObjectPropertyScopeGlobal);
                // SAFETY: те же объект, адрес, функция и data, что при регистрации
                unsafe {
                    AudioObjectRemovePropertyListener(
                        kAudioObjectSystemObject as AudioObjectID,
                        NonNull::from(&addr),
                        Some(on_change),
                        self.callback.cast(),
                    );
                }
            }
            // SAFETY: указатель из Box::into_raw, слушатели сняты — CoreAudio его больше не тронет
            drop(unsafe { Box::from_raw(self.callback) });
        }
    }
}
