//! Большой экран: вход на телевизоре без клавиатуры и режим вечеринки.
//!
//! Вход — как device flow (RFC 8628). Телевизор (Apple TV или страница `/party` в его браузере)
//! просит код: короткий `userCode` показывает на экране вместе с QR, а длинный `deviceCode` держит у
//! себя и раз в пару секунд спрашивает, подтверждён ли вход. Человек, уже вошедший на сайте или в
//! приложении, открывает `/tv?code=…` и подтверждает. Телевизор получает свою сессию — отдельную
//! строку в «Доступе», которая отзывается как любая другая. Код живёт 10 минут и годится один раз.
//!
//! Режим вечеринки — сцена для большого экрана поверх «Слушать вместе»: гости добавляют треки по QR,
//! а настройки вечеринки (только энергичное, без повторов артистов) влияют на волну хозяина.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::auth::{AppToken, Platform, User};

/// Сколько живёт код входа на телевизоре.
pub const TV_CODE_TTL_SECS: i64 = 600;
/// Как часто телевизор спрашивает, подтверждён ли вход.
pub const TV_POLL_SECS: u32 = 2;
/// Сколько длится вечеринка без продления.
pub const PARTY_HOURS: i64 = 12;

/// `POST /api/auth/tv/code`: телевизор просит код.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TvCodeRequest {
    /// `tvos` — приложение Apple TV (получит токен `dwd_…`), `web` — браузер телевизора (получит cookie).
    pub platform: Platform,
    /// Имя для «Доступа»; по умолчанию «Apple TV» или «Телевизор».
    #[serde(default)]
    pub device_name: Option<String>,
}

/// Код для экрана телевизора.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TvCode {
    /// Секрет телевизора для `POST /api/auth/tv/token`; на экран не выводится.
    pub device_code: String,
    /// Короткий код для экрана, например `KQ7M-4TXP`.
    pub user_code: String,
    /// Куда идти подтверждать: `https://wave.daly.rest/tv`.
    pub verification_url: String,
    /// То же с кодом — для QR.
    pub verification_url_complete: String,
    pub expires_at: DateTime<Utc>,
    /// Не спрашивать чаще, секунд.
    pub interval_secs: u32,
}

/// `POST /api/auth/tv/token`.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TvTokenRequest {
    pub device_code: String,
    #[serde(default)]
    pub app_version: Option<String>,
}

/// Браузер телевизора вошёл: cookie `dw_session` уже стоит.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TvSignedIn {
    pub user: User,
}

/// Ответ `POST /api/auth/tv/token`; поле `status` говорит, какой.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum TvToken {
    /// Ещё не подтвердили: спросить через `intervalSecs`.
    Pending,
    /// Приложение Apple TV: токен устройства (показывается один раз).
    Ready(AppToken),
    /// Браузер телевизора: сессия сайта в cookie.
    SignedIn(TvSignedIn),
}

/// `GET /api/auth/tv/{code}`: что подтверждает человек.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TvLogin {
    pub user_code: String,
    pub platform: Platform,
    pub device_name: String,
    pub expires_at: DateTime<Utc>,
}

/// `POST /api/auth/tv/approve`.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TvApprove {
    /// Код с экрана; регистр, пробелы и дефис не важны.
    pub user_code: String,
}

/// Настройки вечеринки.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PartySettings {
    /// Волна — только энергичное.
    pub energetic: bool,
    /// Волна не повторяет артистов, которые уже звучали на этой вечеринке.
    pub no_artist_repeat: bool,
    /// Переходы «Диджей» на устройстве, где играет звук.
    pub dj: bool,
}

impl Default for PartySettings {
    fn default() -> Self {
        Self {
            energetic: false,
            no_artist_repeat: true,
            dj: true,
        }
    }
}

/// `GET /api/party`; событие `party.changed`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Party {
    /// Вечеринка идёт.
    pub active: bool,
    pub settings: PartySettings,
    /// Закончится сама в это время.
    #[schema(required = true)]
    pub until: Option<DateTime<Utc>>,
}

/// `PATCH /api/party`. Включение открывает «Слушать вместе», если оно ещё не открыто.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PartyPatch {
    #[serde(default)]
    pub active: Option<bool>,
    #[serde(default)]
    pub energetic: Option<bool>,
    #[serde(default)]
    pub no_artist_repeat: Option<bool>,
    #[serde(default)]
    pub dj: Option<bool>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_is_tagged_by_status() {
        let json = serde_json::to_value(TvToken::Pending).unwrap();
        assert_eq!(json["status"], "pending");
        let json = serde_json::to_value(PartySettings::default()).unwrap();
        assert_eq!(json["noArtistRepeat"], true);
    }
}
