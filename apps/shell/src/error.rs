use serde::{Serialize, Serializer, ser::SerializeStruct};

/// Ошибка команды. Интерфейс получает `{ code, message }`: по `code` решает, что показать
/// (тексты — в i18n интерфейса), `message` — для журнала.
#[derive(Debug, thiserror::Error)]
pub enum ShellError {
    #[error("вход отменён")]
    Cancelled,
    #[error("вход не завершён за 10 минут")]
    Timeout,
    /// Сервер отказал во входе: `expired`, `state`, `disabled`, `provider`, `invalid_request`,
    /// `invalid_grant`, `forbidden`, `too_many_requests` — коды сервера без изменений.
    #[error("{message}")]
    Denied { code: String, message: String },
    #[error("сервер недоступен: {0}")]
    Network(String),
    #[error("сервер ответил {status}: {message}")]
    Server { status: u16, message: String },
    #[error("связка ключей: {0}")]
    Keychain(String),
    #[error("{0}")]
    Invalid(String),
    #[error("обновление: {0}")]
    Update(String),
    #[error("{0}")]
    Internal(String),
}

impl ShellError {
    pub fn code(&self) -> &str {
        match self {
            Self::Cancelled => "cancelled",
            Self::Timeout => "timeout",
            Self::Denied { code, .. } => code,
            Self::Network(_) => "network",
            Self::Server { .. } => "server",
            Self::Keychain(_) => "keychain",
            Self::Invalid(_) => "invalid",
            Self::Update(_) => "update",
            Self::Internal(_) => "internal",
        }
    }

    pub fn internal(e: impl std::fmt::Display) -> Self {
        Self::Internal(e.to_string())
    }
}

impl Serialize for ShellError {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut s = serializer.serialize_struct("ShellError", 2)?;
        s.serialize_field("code", self.code())?;
        s.serialize_field("message", &self.to_string())?;
        s.end()
    }
}

impl From<tauri::Error> for ShellError {
    fn from(e: tauri::Error) -> Self {
        Self::internal(e)
    }
}

impl From<reqwest::Error> for ShellError {
    fn from(e: reqwest::Error) -> Self {
        Self::Network(e.to_string())
    }
}

impl From<keyring::Error> for ShellError {
    fn from(e: keyring::Error) -> Self {
        Self::Keychain(e.to_string())
    }
}

pub type ShellResult<T> = Result<T, ShellError>;
