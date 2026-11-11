//! Типы API и WebSocket — общие для сервера и приложений.
//!
//! Поля в JSON — camelCase, время — RFC 3339 в UTC. Из этих типов собирается OpenAPI
//! (`/api/openapi.json`), а из него — типы интерфейса, поэтому вручную они нигде не дублируются.

pub mod admin;
pub mod auth;
pub mod error;
pub mod system;

pub use error::{ErrorBody, ErrorInfo};
