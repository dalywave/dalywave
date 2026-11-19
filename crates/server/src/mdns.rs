//! Объявление сервера в локальной сети: служба `_dalywave._tcp` (mDNS/Bonjour). Apple TV и
//! приложения находят сервер без ввода адреса; ручной ввод остаётся запасным.
//!
//! В TXT — публичный адрес (`url`), название и версия: клиенты подключаются по `url`, а не по
//! адресу из локальной сети, — так вход и ссылки те же, что на сайте. Выключается
//! `DALYWAVE_MDNS=false` (в docker без сети хоста объявление всё равно не выходит в локальную сеть).

use std::collections::HashMap;

use mdns_sd::{ServiceDaemon, ServiceInfo};

use crate::config::Config;

/// Тип службы DNS-SD.
pub const SERVICE_TYPE: &str = "_dalywave._tcp.local.";

/// Работающее объявление: пока значение живо, сервер отвечает на запросы в локальной сети.
pub struct Announcement {
    daemon: ServiceDaemon,
    fullname: String,
}

impl Drop for Announcement {
    fn drop(&mut self) {
        // «Прощание» (TTL 0): клиенты сразу убирают сервер из списка
        let _ = self.daemon.unregister(&self.fullname);
        let _ = self.daemon.shutdown();
    }
}

/// Объявить сервер, если это не выключено. Ошибки не мешают работе API — только в журнал.
pub fn announce(config: &Config, port: u16) -> Option<Announcement> {
    if !config.identity.mdns {
        return None;
    }
    let info = match service_info(config, port, &host_label()) {
        Ok(info) => info,
        Err(e) => {
            tracing::warn!(error = %e, "mDNS: could not describe the service");
            return None;
        }
    };
    let daemon = match ServiceDaemon::new() {
        Ok(d) => d,
        Err(e) => {
            tracing::warn!(error = %e, "mDNS: could not start, the server is not announced on the local network");
            return None;
        }
    };
    let fullname = info.get_fullname().to_string();
    if let Err(e) = daemon.register(info) {
        tracing::warn!(error = %e, "mDNS: could not announce the server");
        let _ = daemon.shutdown();
        return None;
    }
    tracing::info!(service = %fullname, "announced on the local network (mDNS)");
    Some(Announcement { daemon, fullname })
}

/// Описание службы: имя экземпляра — название сервера, TXT — адрес, название, версия.
pub fn service_info(config: &Config, port: u16, host: &str) -> anyhow::Result<ServiceInfo> {
    let properties: HashMap<String, String> = txt(config).into_iter().collect();
    let info = ServiceInfo::new(
        SERVICE_TYPE,
        &instance_name(&config.identity.name),
        &format!("{host}.local."),
        "",
        port,
        properties,
    )?
    // Адреса — всех сетевых интерфейсов, и обновляются вместе с ними
    .enable_addr_auto();
    Ok(info)
}

/// Записи TXT. Каждая короче 255 байт: адрес и название ограничены заранее.
pub fn txt(config: &Config) -> Vec<(String, String)> {
    vec![
        ("app".into(), "dalywave".into()),
        ("url".into(), config.public_origin()),
        ("name".into(), truncate(&config.identity.name, 63)),
        ("version".into(), env!("CARGO_PKG_VERSION").into()),
    ]
}

/// Имя экземпляра DNS-SD: до 63 байт, без точек (точка в имени экземпляра ломает часть клиентов).
fn instance_name(name: &str) -> String {
    let clean = name.replace('.', " ").trim().to_string();
    if clean.is_empty() {
        return "DalyWave".into();
    }
    truncate(&clean, 63)
}

fn truncate(s: &str, max_bytes: usize) -> String {
    let mut out = String::new();
    for c in s.chars() {
        if out.len() + c.len_utf8() > max_bytes {
            break;
        }
        out.push(c);
    }
    out
}

/// Имя машины для записи A/AAAA: латиница, цифры и дефис; иначе — `dalywave`.
fn host_label() -> String {
    let raw = std::env::var("HOSTNAME")
        .ok()
        .or_else(|| std::fs::read_to_string("/etc/hostname").ok())
        .unwrap_or_default();
    let label: String = raw
        .trim()
        .split('.')
        .next()
        .unwrap_or_default()
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-')
        .take(40)
        .collect();
    if label.is_empty() {
        "dalywave".into()
    } else {
        format!("{label}-dalywave")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config(name: &str) -> Config {
        let mut c = Config::from_lookup(&[], |k| match k {
            "DALYWAVE_PUBLIC_URL" => Some("https://wave.example.com/".into()),
            "DALYWAVE_SECRET_KEY" => Some("ab".repeat(32)),
            _ => None,
        })
        .unwrap();
        c.identity.name = name.into();
        c
    }

    #[test]
    fn txt_carries_public_address_and_name() {
        let t: HashMap<_, _> = txt(&config("Дом")).into_iter().collect();
        assert_eq!(t["app"], "dalywave");
        assert_eq!(t["url"], "https://wave.example.com");
        assert_eq!(t["name"], "Дом");
        assert_eq!(t["version"], env!("CARGO_PKG_VERSION"));
    }

    #[test]
    fn service_name_is_dns_sd_safe() {
        let info = service_info(&config("wave.daly.rest"), 8400, "box-dalywave").unwrap();
        assert_eq!(info.get_type(), SERVICE_TYPE);
        assert_eq!(info.get_fullname(), "wave daly rest._dalywave._tcp.local.");
        assert_eq!(info.get_hostname(), "box-dalywave.local.");
        assert_eq!(info.get_port(), 8400);
        assert_eq!(
            info.get_property_val_str("url"),
            Some("https://wave.example.com")
        );
        let long = "Очень длинное название сервера, которое не помещается в метку DNS";
        assert!(instance_name(long).len() <= 63);
        assert_eq!(instance_name("  "), "DalyWave");
    }

    #[test]
    fn disabled_announces_nothing() {
        let mut c = config("Дом");
        c.identity.mdns = false;
        assert!(announce(&c, 8400).is_none());
    }
}
