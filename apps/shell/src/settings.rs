//! Настройки оболочки: адрес сервера, язык системных меню, тема окна, загрузчик на Mac (включён ли,
//! на паузе ли), выбранное устройство вывода звука на компьютере, эквалайзер этого устройства и
//! помощь в обучении рекомендаций на компьютере.
//! Лежат в `settings.json` в папке настроек приложения; секреты сюда не попадают — токены живут
//! в связке ключей.

use std::{
    path::PathBuf,
    sync::{
        Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};
use url::Url;

use crate::error::{ShellError, ShellResult};

pub const PRODUCTION_SERVER: &str = "https://wave.daly.rest";
/// Демо-сервер с открытой музыкой: кнопка «Попробовать демо» на экране «Адрес сервера».
pub const DEMO_SERVER: &str = "https://demo.wave.daly.rest";
/// Недавних серверов в списке — не больше стольких.
const RECENT_MAX: usize = 8;

/// Сервер, выбранный при сборке. Сборки по умолчанию сразу смотрят на wave.daly.rest и экран
/// «Адрес сервера» не показывают; `DALYWAVE_DEFAULT_SERVER=none` при сборке (App Store, сторонние
/// сборки) — первый запуск начинается с выбора сервера; любой адрес — свой сервер по умолчанию.
pub fn preset_server() -> Option<String> {
    match option_env!("DALYWAVE_DEFAULT_SERVER").map(str::trim) {
        None => Some(PRODUCTION_SERVER.to_string()),
        Some("" | "none") => None,
        Some(url) => normalize_server_url(url).ok(),
    }
}

/// Сервер из списка недавних: адрес и название из его `/api/config`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecentServer {
    pub url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Когда выбран последний раз, секунды Unix
    #[serde(default)]
    pub used_at: u64,
}
/// В разработке интерфейс открыт с dev-сервера Nuxt, который проксирует /api на :8400
pub const DEV_SERVER: &str = "http://localhost:3400";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Locale {
    Ru,
    En,
    Uk,
}

impl Locale {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ru => "ru",
            Self::En => "en",
            Self::Uk => "uk",
        }
    }
}

/// Тема, выбранная в интерфейсе. Оболочка помнит её, чтобы при следующем запуске окно сразу
/// открылось с фоном этой темы — без вспышки тёмного фона на светлой теме и наоборот.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ThemeChoice {
    Dark,
    Light,
    /// Как в системе
    System,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Stored {
    server_url: Option<String>,
    /// Недавние серверы, последний выбранный — первым
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    recent_servers: Vec<RecentServer>,
    locale: Option<Locale>,
    /// Тема окна; нет — тёмная, как по умолчанию в интерфейсе
    #[serde(default, skip_serializing_if = "Option::is_none")]
    theme: Option<ThemeChoice>,
    /// Загрузчик на этом Mac; по умолчанию включён
    #[serde(default, skip_serializing_if = "Option::is_none")]
    downloader_enabled: Option<bool>,
    /// Пауза загрузчика не запоминается им самим: приложение передаёт `--paused` при запуске
    #[serde(default, skip_serializing_if = "Option::is_none")]
    downloader_paused: Option<bool>,
    /// Устройство вывода звука (`cpal::DeviceId` строкой); нет — системное по умолчанию
    #[serde(default, skip_serializing_if = "Option::is_none")]
    audio_output: Option<String>,
    /// Эквалайзер этого устройства: пресет и полосы «Своего»; нет — выключен
    #[serde(default, skip_serializing_if = "Option::is_none")]
    eq: Option<StoredEq>,
    /// «Помогать учить рекомендации на этом компьютере» (trainer.rs); нет — только у администраторов
    #[serde(default, skip_serializing_if = "Option::is_none")]
    taste_training: Option<bool>,
}

/// Эквалайзер в настройках: `preset` — id пресета (`off`, `bass-boost`… или `custom`), `gains` —
/// усиления десяти полос «Своего», дБ (помнятся и при другом пресете).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StoredEq {
    pub preset: String,
    #[serde(default)]
    pub gains: Vec<f32>,
}

pub struct Settings {
    path: Option<PathBuf>,
    default_server: String,
    /// Сервер уже выбран: сборка с адресом по умолчанию, адрес сохранён или на этом устройстве
    /// уже входили (обновление прежней версии). Нет — первый запуск начинается с выбора сервера.
    chosen: AtomicBool,
    stored: Mutex<Stored>,
}

impl Settings {
    pub fn load(app: &AppHandle) -> Self {
        let path = app
            .path()
            .app_config_dir()
            .ok()
            .map(|d| d.join("settings.json"));
        let stored = path
            .as_ref()
            .and_then(|p| std::fs::read(p).ok())
            .and_then(|bytes| serde_json::from_slice::<Stored>(&bytes).ok())
            .unwrap_or_default();
        // Переменная окружения задаёт адрес по умолчанию: удобно для проверок против своего сервера
        let env_server = std::env::var("DALYWAVE_SERVER_URL")
            .ok()
            .and_then(|v| normalize_server_url(&v).ok());
        let preset = env_server
            .or_else(|| tauri::is_dev().then(|| DEV_SERVER.to_string()))
            .or_else(preset_server);
        // Прежние версии адрес не сохраняли: вход на этом устройстве значит, что сервер выбран
        let signed_in_before = app
            .path()
            .app_data_dir()
            .ok()
            .and_then(|d| std::fs::read(d.join("sessions.json")).ok())
            .and_then(|b| {
                serde_json::from_slice::<serde_json::Map<String, serde_json::Value>>(&b).ok()
            })
            .is_some_and(|m| !m.is_empty());
        let chosen = preset.is_some() || stored.server_url.is_some() || signed_in_before;
        Self {
            path,
            // Без выбора адрес ни к чему не ведёт: интерфейс сначала покажет экран выбора
            default_server: preset.unwrap_or_else(|| PRODUCTION_SERVER.to_string()),
            chosen: AtomicBool::new(chosen),
            stored: Mutex::new(stored),
        }
    }

    pub fn server_chosen(&self) -> bool {
        self.chosen.load(Ordering::Relaxed)
    }

    /// Недавние серверы, последний выбранный — первым.
    pub fn recent_servers(&self) -> Vec<RecentServer> {
        self.lock().recent_servers.clone()
    }

    /// Убрать сервер из недавних (вход на нём остаётся в связке ключей до выхода).
    pub fn forget_server(&self, url: &str) -> ShellResult<()> {
        let mut stored = self.lock();
        stored.recent_servers.retain(|s| s.url != url);
        self.save(&stored)
    }

    pub fn server_url(&self) -> String {
        self.lock()
            .server_url
            .clone()
            .unwrap_or_else(|| self.default_server.clone())
    }

    /// `None` — вернуть адрес по умолчанию. `name` — название из `/api/config` для списка недавних.
    pub fn set_server_url(&self, url: Option<&str>, name: Option<&str>) -> ShellResult<String> {
        let url = url.map(normalize_server_url).transpose()?;
        let mut stored = self.lock();
        let current = url.clone().unwrap_or_else(|| self.default_server.clone());
        remember_recent(&mut stored.recent_servers, &current, name, unix_now());
        stored.server_url = url;
        self.save(&stored)?;
        self.chosen.store(true, Ordering::Relaxed);
        Ok(stored
            .server_url
            .clone()
            .unwrap_or_else(|| self.default_server.clone()))
    }

    pub fn locale(&self) -> Locale {
        self.lock().locale.unwrap_or(Locale::Ru)
    }

    pub fn set_locale(&self, locale: Locale) -> ShellResult<()> {
        let mut stored = self.lock();
        stored.locale = Some(locale);
        self.save(&stored)
    }

    pub fn theme(&self) -> ThemeChoice {
        self.lock().theme.unwrap_or(ThemeChoice::Dark)
    }

    /// Каждое окно сообщает тему при запуске — файл переписывается, только если она изменилась.
    pub fn set_theme(&self, theme: ThemeChoice) -> ShellResult<()> {
        let mut stored = self.lock();
        if stored.theme == Some(theme) {
            return Ok(());
        }
        stored.theme = Some(theme);
        self.save(&stored)
    }

    #[cfg(desktop)]
    pub fn downloader_enabled(&self) -> bool {
        self.lock().downloader_enabled.unwrap_or(true)
    }

    #[cfg(desktop)]
    pub fn set_downloader_enabled(&self, enabled: bool) -> ShellResult<()> {
        let mut stored = self.lock();
        stored.downloader_enabled = Some(enabled);
        self.save(&stored)
    }

    #[cfg(desktop)]
    pub fn downloader_paused(&self) -> bool {
        self.lock().downloader_paused.unwrap_or(false)
    }

    #[cfg(desktop)]
    pub fn set_downloader_paused(&self, paused: bool) -> ShellResult<()> {
        let mut stored = self.lock();
        stored.downloader_paused = Some(paused);
        self.save(&stored)
    }

    #[cfg(desktop)]
    pub fn taste_training(&self) -> Option<bool> {
        self.lock().taste_training
    }

    #[cfg(desktop)]
    pub fn set_taste_training(&self, enabled: Option<bool>) -> ShellResult<()> {
        let mut stored = self.lock();
        stored.taste_training = enabled;
        self.save(&stored)
    }

    #[cfg(desktop)]
    pub fn audio_output(&self) -> Option<String> {
        self.lock().audio_output.clone()
    }

    #[cfg(desktop)]
    pub fn set_audio_output(&self, id: Option<String>) -> ShellResult<()> {
        let mut stored = self.lock();
        stored.audio_output = id;
        self.save(&stored)
    }

    pub fn eq(&self) -> Option<StoredEq> {
        self.lock().eq.clone()
    }

    pub fn set_eq(&self, eq: Option<StoredEq>) -> ShellResult<()> {
        let mut stored = self.lock();
        stored.eq = eq;
        self.save(&stored)
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Stored> {
        self.stored.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn save(&self, stored: &Stored) -> ShellResult<()> {
        let Some(path) = &self.path else {
            return Ok(());
        };
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(ShellError::internal)?;
        }
        let json = serde_json::to_vec_pretty(stored).map_err(ShellError::internal)?;
        std::fs::write(path, json).map_err(ShellError::internal)
    }
}

fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// Поднять сервер в начало недавних; название — новое, если пришло, иначе прежнее.
fn remember_recent(list: &mut Vec<RecentServer>, url: &str, name: Option<&str>, now: u64) {
    let old_name = list
        .iter()
        .find(|s| s.url == url)
        .and_then(|s| s.name.clone());
    list.retain(|s| s.url != url);
    let name = name
        .map(str::trim)
        .filter(|n| !n.is_empty())
        .map(|n| n.chars().take(80).collect())
        .or(old_name);
    list.insert(
        0,
        RecentServer {
            url: url.to_string(),
            name,
            used_at: now,
        },
    );
    list.truncate(RECENT_MAX);
}

/// Адрес сервера без пути и завершающего «/». Простой http — только для своей машины:
/// по нему уходит токен устройства.
pub fn normalize_server_url(raw: &str) -> ShellResult<String> {
    let bad = || {
        ShellError::Invalid(format!(
            "адрес сервера должен быть вида https://wave.daly.rest, а не «{raw}»"
        ))
    };
    let trimmed = raw.trim();
    // «demo.wave.daly.rest» без схемы — это https
    let with_scheme = if trimmed.contains("://") {
        trimmed.to_string()
    } else {
        format!("https://{trimmed}")
    };
    let url = Url::parse(&with_scheme).map_err(|_| bad())?;
    let host = url.host_str().ok_or_else(bad)?;
    let local = matches!(host, "localhost" | "127.0.0.1" | "[::1]");
    let scheme_ok = url.scheme() == "https" || (url.scheme() == "http" && local);
    if !scheme_ok
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || !matches!(url.path(), "" | "/")
    {
        return Err(bad());
    }
    Ok(url.origin().ascii_serialization())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn server_urls() {
        assert_eq!(
            normalize_server_url("https://wave.daly.rest/").unwrap(),
            "https://wave.daly.rest"
        );
        assert_eq!(
            normalize_server_url(" http://localhost:3400 ").unwrap(),
            "http://localhost:3400"
        );
        assert_eq!(
            normalize_server_url("http://127.0.0.1:8400").unwrap(),
            "http://127.0.0.1:8400"
        );
        assert!(
            normalize_server_url("http://wave.daly.rest").is_err(),
            "токен не уходит по http"
        );
        assert!(normalize_server_url("https://wave.daly.rest/api").is_err());
        assert!(normalize_server_url("https://user@wave.daly.rest").is_err());
        assert_eq!(
            normalize_server_url("demo.wave.daly.rest").unwrap(),
            "https://demo.wave.daly.rest",
            "без схемы — https"
        );
        assert!(normalize_server_url("wave daly rest").is_err());
        assert!(normalize_server_url("ftp://wave.daly.rest").is_err());
    }

    #[test]
    fn recent_servers_keep_order_and_names() {
        let mut list = Vec::new();
        remember_recent(&mut list, "https://a.example", Some("Дом"), 1);
        remember_recent(&mut list, "https://b.example", None, 2);
        remember_recent(&mut list, "https://a.example", None, 3);
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].url, "https://a.example");
        assert_eq!(list[0].name.as_deref(), Some("Дом"), "название не теряется");
        assert_eq!(list[0].used_at, 3);
        for i in 0..20 {
            remember_recent(&mut list, &format!("https://s{i}.example"), None, 10 + i);
        }
        assert_eq!(list.len(), RECENT_MAX);
        assert_eq!(list[0].url, "https://s19.example");
    }

    #[test]
    fn locale_names_match_the_interface() {
        for (locale, name) in [(Locale::Ru, "ru"), (Locale::En, "en"), (Locale::Uk, "uk")] {
            assert_eq!(locale.as_str(), name);
            assert_eq!(serde_json::to_string(&locale).unwrap(), format!("\"{name}\""));
            assert_eq!(serde_json::from_str::<Locale>(&format!("\"{name}\"")).unwrap(), locale);
        }
    }

    #[test]
    fn owner_builds_preset_the_production_server() {
        // Без DALYWAVE_DEFAULT_SERVER при сборке сервер уже выбран
        if option_env!("DALYWAVE_DEFAULT_SERVER").is_none() {
            assert_eq!(preset_server().as_deref(), Some(PRODUCTION_SERVER));
        }
    }
}
