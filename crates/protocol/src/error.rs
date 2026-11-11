use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Тело любой ошибки API: `{ "error": { "code": "...", "message": "..." } }`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct ErrorBody {
    pub error: ErrorInfo,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct ErrorInfo {
    /// Машинный код в snake_case: по нему интерфейс выбирает текст и действие.
    pub code: String,
    /// Понятный текст по-русски.
    pub message: String,
}
