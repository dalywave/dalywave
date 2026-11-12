//! Настройки оболочки: адрес сервера, язык системных меню, загрузчик на Mac (включён ли, на
//! паузе ли), выбранное устройство вывода звука на компьютере и эквалайзер этого устройства. Лежат в `settings.json` в папке настроек приложения; секреты сюда не попадают —
//! токены живут в связке ключей.

use std::{path::PathBuf, sync::Mutex};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};
use url::Url;

use crate::error::{ShellError, ShellResult};

pub const PRODUCTION_SERVER: &str = "https://wave.daly.rest";
/// В разработке интерфейс открыт с dev-сервера Nuxt, который проксирует /api на :8400
pub const DEV_SERVER: &str = "http://localhost:3400";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Locale {
    Ru,
    En,
}

impl Locale {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ru => "ru",
            Self::En => "en",
        }
    }
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Stored {
    server_url: Option<String>,
    locale: Option<Locale>,
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
        let default_server = std::env::var("DALYWAVE_SERVER_URL")
            .ok()
            .and_then(|v| normalize_server_url(&v).ok())
            .unwrap_or_else(|| {
                if tauri::is_dev() {
                    DEV_SERVER
                } else {
                    PRODUCTION_SERVER
                }
                .to_string()
            });
        Self {
            path,
            default_server,
            stored: Mutex::new(stored),
        }
    }

    pub fn server_url(&self) -> String {
        self.lock()
            .server_url
            .clone()
            .unwrap_or_else(|| self.default_server.clone())
    }

    /// `None` — вернуть адрес по умолчанию.
    pub fn set_server_url(&self, url: Option<&str>) -> ShellResult<String> {
        let url = url.map(normalize_server_url).transpose()?;
        let mut stored = self.lock();
        stored.server_url = url;
        self.save(&stored)?;
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

/// Адрес сервера без пути и завершающего «/». Простой http — только для своей машины:
/// по нему уходит токен устройства.
pub fn normalize_server_url(raw: &str) -> ShellResult<String> {
    let bad = || {
        ShellError::Invalid(format!(
            "адрес сервера должен быть вида https://wave.daly.rest, а не «{raw}»"
        ))
    };
    let url = Url::parse(raw.trim()).map_err(|_| bad())?;
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
        assert!(normalize_server_url("wave.daly.rest").is_err());
        assert!(normalize_server_url("ftp://wave.daly.rest").is_err());
    }
}
