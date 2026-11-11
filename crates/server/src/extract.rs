//! Экстракторы axum с ошибками в общем формате API: `{ "error": { "code", "message" } }`.
//! Встроенные отвечают на неверный ввод простым текстом, а интерфейс ждёт JSON.

use axum::{
    body::Bytes,
    extract::{FromRequest, FromRequestParts, Request},
    http::request::Parts,
    response::{IntoResponse, Response},
};
use serde::{Serialize, de::DeserializeOwned};

use crate::error::AppError;

/// JSON-тело запроса или ответа.
pub struct Json<T>(pub T);

impl<S, T> FromRequest<S> for Json<T>
where
    T: DeserializeOwned,
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        let axum::Json(value) = axum::Json::<T>::from_request(req, state).await?;
        Ok(Self(value))
    }
}

impl<T: Serialize> IntoResponse for Json<T> {
    fn into_response(self) -> Response {
        axum::Json(self.0).into_response()
    }
}

/// JSON-тело, которое можно не передавать: пустое тело — значение по умолчанию.
/// Тип содержимого уже проверил защитный слой (`security::guard`).
pub struct JsonOrDefault<T>(pub T);

impl<S, T> FromRequest<S> for JsonOrDefault<T>
where
    T: DeserializeOwned + Default,
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        let bytes = Bytes::from_request(req, state)
            .await
            .map_err(|_| AppError::PayloadTooLarge)?;
        if bytes.iter().all(u8::is_ascii_whitespace) {
            return Ok(Self(T::default()));
        }
        serde_json::from_slice(&bytes)
            .map(Self)
            .map_err(|e| AppError::BadRequest(format!("неверный JSON: {e}")))
    }
}

/// Параметры адреса после `?`.
pub struct Query<T>(pub T);

impl<S, T> FromRequestParts<S> for Query<T>
where
    T: DeserializeOwned,
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        match axum::extract::Query::<T>::from_request_parts(parts, state).await {
            Ok(axum::extract::Query(value)) => Ok(Self(value)),
            Err(e) => Err(AppError::BadRequest(format!(
                "неверные параметры запроса: {}",
                e.body_text()
            ))),
        }
    }
}

/// Параметры пути, например `{id}`.
pub struct Path<T>(pub T);

impl<S, T> FromRequestParts<S> for Path<T>
where
    T: DeserializeOwned + Send,
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        match axum::extract::Path::<T>::from_request_parts(parts, state).await {
            Ok(axum::extract::Path(value)) => Ok(Self(value)),
            Err(e) => Err(AppError::BadRequest(format!(
                "неверный адрес: {}",
                e.body_text()
            ))),
        }
    }
}
