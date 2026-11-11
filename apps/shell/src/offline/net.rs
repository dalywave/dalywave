//! Какая сейчас сеть: Wi-Fi (или провод), сотовая или никакой. По ней загрузки ждут Wi-Fi, а приложение
//! без сети сразу берёт скачанное, не дожидаясь таймаутов.
//!
//! macOS и iOS — SCNetworkReachability (SystemConfiguration): флаги для «любого адреса» отвечают
//! мгновенно и без запроса в сеть. Сотовая — флаг `IsWWAN` (есть только на iOS).

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Wifi,
    Cellular,
    None,
    Unknown,
}

impl Kind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Wifi => "wifi",
            Self::Cellular => "cellular",
            Self::None => "none",
            Self::Unknown => "unknown",
        }
    }
}

#[cfg(target_vendor = "apple")]
mod apple {
    use std::ffi::c_void;

    use super::Kind;

    /// `struct sockaddr_in` для 0.0.0.0 — «любой адрес», как в примере Reachability от Apple.
    #[repr(C)]
    struct SockaddrIn {
        sin_len: u8,
        sin_family: u8,
        sin_port: u16,
        sin_addr: u32,
        sin_zero: [u8; 8],
    }

    const AF_INET: u8 = 2;
    const REACHABLE: u32 = 1 << 1;
    const CONNECTION_REQUIRED: u32 = 1 << 2;
    const IS_WWAN: u32 = 1 << 18;

    #[link(name = "SystemConfiguration", kind = "framework")]
    unsafe extern "C" {
        fn SCNetworkReachabilityCreateWithAddress(
            allocator: *const c_void,
            address: *const SockaddrIn,
        ) -> *mut c_void;
        fn SCNetworkReachabilityGetFlags(target: *mut c_void, flags: *mut u32) -> u8;
    }

    #[link(name = "CoreFoundation", kind = "framework")]
    unsafe extern "C" {
        fn CFRelease(cf: *const c_void);
    }

    pub fn kind() -> Kind {
        let addr = SockaddrIn {
            sin_len: std::mem::size_of::<SockaddrIn>() as u8,
            sin_family: AF_INET,
            sin_port: 0,
            sin_addr: 0,
            sin_zero: [0; 8],
        };
        // SAFETY: адрес живёт до конца вызова, объект освобождается CFRelease
        unsafe {
            let r = SCNetworkReachabilityCreateWithAddress(std::ptr::null(), &addr);
            if r.is_null() {
                return Kind::Unknown;
            }
            let mut flags = 0u32;
            let ok = SCNetworkReachabilityGetFlags(r, &mut flags) != 0;
            CFRelease(r);
            if !ok {
                return Kind::Unknown;
            }
            if flags & REACHABLE == 0 || flags & CONNECTION_REQUIRED != 0 {
                Kind::None
            } else if flags & IS_WWAN != 0 {
                Kind::Cellular
            } else {
                Kind::Wifi
            }
        }
    }
}

#[cfg(target_vendor = "apple")]
pub fn kind() -> Kind {
    apple::kind()
}

/// Windows и Linux: сеть узнаётся по ответу сервера.
#[cfg(not(target_vendor = "apple"))]
pub fn kind() -> Kind {
    Kind::Unknown
}
