use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

pub use crate::UnknownName;

/// Роль в DalyWave. Роли совмещаются; человек без ролей видит экран «ждём доступа».
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, ToSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    /// Все права, включая админку.
    Admin,
    /// Слушать музыку и вести свою медиатеку.
    Listener,
}

names!(Role { Admin => "admin", Listener => "listener" });

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, ToSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum Permission {
    Play,
    Library,
    Admin,
    /// Домашний загрузчик: только `/api/fetcher/*`. Есть лишь у токена загрузчика `dwk_…`,
    /// роли людей его не дают.
    Downloader,
}

names!(Permission {
    Play => "play",
    Library => "library",
    Admin => "admin",
    Downloader => "downloader",
});

impl Permission {
    /// Права, которые дают роли людей.
    pub const PEOPLE: &'static [Permission] =
        &[Permission::Play, Permission::Library, Permission::Admin];
}

/// Как открыта текущая сессия.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum AuthVia {
    /// Вход через daly.rest на сайте.
    Oidc,
    /// Аварийный вход паролем из `.env`.
    Password,
    /// Токен приложения.
    Device,
}

names!(AuthVia { Oidc => "oidc", Password => "password", Device => "device" });

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum Platform {
    Web,
    Macos,
    Windows,
    Linux,
    Ios,
    Ipados,
    Android,
    /// Apple Watch: пульт, вход — от сопряжённого iPhone.
    Watchos,
    /// Apple TV: вход кодом с экрана, подтверждение на телефоне или сайте (`/api/auth/tv/*`).
    Tvos,
    /// Плагин Stream Deck: пульт, вход — как у приложений (`/api/auth/app/*`, код на свой порт).
    Streamdeck,
}

names!(Platform {
    Web => "web",
    Macos => "macos",
    Windows => "windows",
    Linux => "linux",
    Ios => "ios",
    Ipados => "ipados",
    Android => "android",
    Watchos => "watchos",
    Tvos => "tvos",
    Streamdeck => "streamdeck",
});

impl Platform {
    /// Устройство без своего входа: сессию ему выдаёт уже вошедшее приложение (часы — от iPhone).
    pub fn is_paired_accessory(self) -> bool {
        matches!(self, Platform::Watchos)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum SessionKind {
    /// Cookie `dw_session` сайта.
    Web,
    /// `Authorization: Bearer dwd_…` приложения.
    Device,
}

names!(SessionKind { Web => "web", Device => "device" });

/// Какие способы входа включены на сервере.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AuthProviders {
    /// Вход через daly.rest; `null`, если не настроен.
    #[schema(required = true)]
    pub oidc: Option<OidcProvider>,
    /// Включён ли аварийный вход паролем.
    pub password: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct OidcProvider {
    /// Подпись кнопки входа, например `daly.rest`.
    pub title: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct OidcStartQuery {
    /// Куда вернуть после входа: путь на этом же сайте, например `/library`.
    pub next: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PasswordLogin {
    pub password: String,
}

/// Человек в DalyWave.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct User {
    pub id: Uuid,
    pub name: String,
    #[schema(required = true)]
    pub email: Option<String>,
    #[schema(required = true)]
    pub avatar_url: Option<String>,
}

/// Устройство с приложением: у каждого свой токен.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Device {
    pub id: Uuid,
    pub name: String,
    pub platform: Platform,
    #[schema(required = true)]
    pub app_version: Option<String>,
    pub created_at: DateTime<Utc>,
    #[schema(required = true)]
    pub last_seen_at: Option<DateTime<Utc>>,
}

/// Кто вошёл. Отвечает и людям без ролей: интерфейс показывает им «ждём доступа».
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Me {
    pub user: User,
    pub roles: Vec<Role>,
    pub permissions: Vec<Permission>,
    pub via: AuthVia,
    pub session_id: Uuid,
    /// Устройство, если вход через токен приложения.
    #[schema(required = true)]
    pub device: Option<Device>,
}

/// Начало входа приложения через системный браузер. Параметры — в духе OAuth (snake_case).
#[derive(Debug, Clone, Default, Serialize, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct AppStartQuery {
    /// `dalywave://auth`, `http://127.0.0.1:<port>/callback` или `http://localhost:<port>/callback`.
    pub redirect_uri: Option<String>,
    /// base64url(SHA-256(code_verifier)), 43–128 символов.
    pub code_challenge: Option<String>,
    /// Только `S256`.
    pub code_challenge_method: Option<String>,
    /// Возвращается приложению без изменений.
    pub state: Option<String>,
    /// Имя устройства для списка сессий, например «MacBook Захара».
    pub device_name: Option<String>,
    pub platform: Option<Platform>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AppTokenRequest {
    /// Одноразовый код из адреса возврата, живёт 60 секунд.
    pub code: String,
    pub code_verifier: String,
    #[serde(default)]
    pub app_version: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AppToken {
    /// `dwd_…`: передаётся как `Authorization: Bearer`. Показывается один раз.
    pub token: String,
    pub device: Device,
    pub user: User,
}

/// Сессия для сопряжённого устройства (Apple Watch) от уже вошедшего приложения. Это не новый
/// способ входа: человек уже вошёл через daly.rest на iPhone, часы получают дочернюю сессию.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PairRequest {
    /// Только `watchos`.
    pub platform: Platform,
    /// Имя для списка устройств; по умолчанию «Apple Watch».
    #[serde(default)]
    pub device_name: Option<String>,
    #[serde(default)]
    pub app_version: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_match_json() {
        for r in Role::ALL {
            assert_eq!(serde_json::to_value(r).unwrap(), r.as_str());
            assert_eq!(r.as_str().parse::<Role>().unwrap(), *r);
        }
        for p in Permission::ALL {
            assert_eq!(serde_json::to_value(p).unwrap(), p.as_str());
        }
        for p in Platform::ALL {
            assert_eq!(serde_json::to_value(p).unwrap(), p.as_str());
            assert_eq!(p.as_str().parse::<Platform>().unwrap(), *p);
        }
        for v in AuthVia::ALL {
            assert_eq!(serde_json::to_value(v).unwrap(), v.as_str());
        }
        for k in SessionKind::ALL {
            assert_eq!(serde_json::to_value(k).unwrap(), k.as_str());
        }
        assert!("root".parse::<Role>().is_err());
    }

    #[test]
    fn camel_case_fields() {
        let me = AppTokenRequest {
            code: "c".into(),
            code_verifier: "v".into(),
            app_version: None,
        };
        let json = serde_json::to_value(&me).unwrap();
        assert_eq!(json["codeVerifier"], "v");
        let parsed: AppTokenRequest =
            serde_json::from_value(serde_json::json!({"code": "c", "codeVerifier": "v"})).unwrap();
        assert!(parsed.app_version.is_none());
    }
}
