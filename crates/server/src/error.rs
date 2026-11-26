use axum::{
    Json,
    extract::rejection::JsonRejection,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use dalywave_protocol::{ErrorBody, ErrorInfo};

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("нужно войти")]
    Unauthorized,
    #[error("неверный пароль")]
    WrongPassword,
    /// Вход есть, ролей нет: интерфейс показывает «ждём доступа».
    #[error("доступ ещё не выдан: администратор должен назначить роль")]
    NoAccess,
    #[error("{0}")]
    Forbidden(String),
    #[error("запрос пришёл с чужого сайта")]
    BadOrigin,
    #[error("вход паролем выключен")]
    PasswordDisabled,
    /// Общий аккаунт публичного демо (`DALYWAVE_DEMO_ACCOUNT`): вход, пароль и права в нём не меняются.
    #[error("в демо-аккаунте это недоступно: он общий для всех гостей")]
    DemoAccount,
    #[error("не найдено")]
    NotFound,
    /// 404 со своим кодом: сущность есть, а нужной части у неё нет — приложение отличает это от «нет такого».
    #[error("{message}")]
    Missing { code: &'static str, message: String },
    #[error("{0}")]
    BadRequest(String),
    /// Код приложения неверен, истёк, уже использован или не сходится с PKCE.
    #[error("{0}")]
    InvalidGrant(String),
    #[error("{message}")]
    Conflict { code: &'static str, message: String },
    /// 422 со своим кодом: запрос понятен, но выполнить его нельзя (например, ссылка не на музыку).
    #[error("{message}")]
    Unprocessable { code: &'static str, message: String },
    #[error("метод не поддерживается")]
    MethodNotAllowed,
    #[error("слишком большое тело запроса")]
    PayloadTooLarge,
    #[error("тело запроса должно быть в JSON")]
    UnsupportedMediaType,
    #[error("слишком много попыток, попробуйте через минуту")]
    TooManyRequests,
    /// Внешний сервис (Deezer, хранилище) не ответил: стоит повторить позже.
    #[error("{0}")]
    Unavailable(String),
    #[error(transparent)]
    Db(sqlx::Error),
    #[error(transparent)]
    Internal(#[from] anyhow::Error),
}

pub type AppResult<T> = Result<T, AppError>;

pub fn bad(msg: impl Into<String>) -> AppError {
    AppError::BadRequest(msg.into())
}

impl AppError {
    pub fn status_and_code(&self) -> (StatusCode, &'static str) {
        match self {
            Self::Unauthorized => (StatusCode::UNAUTHORIZED, "unauthorized"),
            Self::WrongPassword => (StatusCode::UNAUTHORIZED, "wrong_password"),
            Self::NoAccess => (StatusCode::FORBIDDEN, "no_access"),
            Self::Forbidden(_) => (StatusCode::FORBIDDEN, "forbidden"),
            Self::BadOrigin => (StatusCode::FORBIDDEN, "bad_origin"),
            Self::PasswordDisabled => (StatusCode::FORBIDDEN, "password_disabled"),
            Self::DemoAccount => (StatusCode::FORBIDDEN, "demo_account"),
            Self::NotFound => (StatusCode::NOT_FOUND, "not_found"),
            Self::Missing { code, .. } => (StatusCode::NOT_FOUND, code),
            Self::BadRequest(_) => (StatusCode::BAD_REQUEST, "bad_request"),
            Self::InvalidGrant(_) => (StatusCode::BAD_REQUEST, "invalid_grant"),
            Self::Conflict { code, .. } => (StatusCode::CONFLICT, code),
            Self::Unprocessable { code, .. } => (StatusCode::UNPROCESSABLE_ENTITY, code),
            Self::MethodNotAllowed => (StatusCode::METHOD_NOT_ALLOWED, "method_not_allowed"),
            Self::PayloadTooLarge => (StatusCode::PAYLOAD_TOO_LARGE, "payload_too_large"),
            Self::UnsupportedMediaType => {
                (StatusCode::UNSUPPORTED_MEDIA_TYPE, "unsupported_media_type")
            }
            Self::TooManyRequests => (StatusCode::TOO_MANY_REQUESTS, "too_many_requests"),
            Self::Unavailable(_) => (StatusCode::SERVICE_UNAVAILABLE, "unavailable"),
            Self::Db(_) | Self::Internal(_) => (StatusCode::INTERNAL_SERVER_ERROR, "internal"),
        }
    }
}

impl From<getrandom::Error> for AppError {
    fn from(err: getrandom::Error) -> Self {
        Self::Internal(anyhow::Error::new(err).context("system random generator failed"))
    }
}

impl From<sqlx::Error> for AppError {
    fn from(err: sqlx::Error) -> Self {
        if matches!(err, sqlx::Error::RowNotFound) {
            return Self::NotFound;
        }
        Self::Db(err)
    }
}

impl From<JsonRejection> for AppError {
    fn from(rejection: JsonRejection) -> Self {
        match rejection.status() {
            StatusCode::PAYLOAD_TOO_LARGE => Self::PayloadTooLarge,
            StatusCode::UNSUPPORTED_MEDIA_TYPE => Self::UnsupportedMediaType,
            _ => Self::BadRequest(format!("неверный JSON: {}", rejection.body_text())),
        }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, code) = self.status_and_code();
        let message = if status == StatusCode::INTERNAL_SERVER_ERROR {
            tracing::error!(error = ?self, "internal error");
            "внутренняя ошибка сервера".to_string()
        } else {
            self.to_string()
        };
        let body = ErrorBody {
            error: ErrorInfo {
                code: code.to_string(),
                message,
            },
        };
        (status, Json(body)).into_response()
    }
}
