//! Типы API и WebSocket — общие для сервера и приложений.
//!
//! Поля в JSON — camelCase, время — RFC 3339 в UTC. Из этих типов собирается OpenAPI
//! (`/api/openapi.json`), а из него — типы интерфейса, поэтому вручную они нигде не дублируются.

/// Строковые имена перечислений: те же, что в JSON, и те же, что хранятся в базе.
macro_rules! names {
    ($ty:ident { $($variant:ident => $name:literal),+ $(,)? }) => {
        impl $ty {
            pub const ALL: &'static [$ty] = &[$($ty::$variant),+];

            pub fn as_str(self) -> &'static str {
                match self {
                    $($ty::$variant => $name),+
                }
            }
        }

        impl std::str::FromStr for $ty {
            type Err = $crate::UnknownName;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                match s {
                    $($name => Ok($ty::$variant),)+
                    _ => Err($crate::UnknownName(s.to_string())),
                }
            }
        }

        impl std::fmt::Display for $ty {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str(self.as_str())
            }
        }
    };
}

pub mod admin;
pub mod auth;
pub mod catalog;
pub mod error;
pub mod events;
pub mod home;
pub mod jobs;
pub mod library;
pub mod realtime;
pub mod sync;
pub mod system;

pub use error::{ErrorBody, ErrorInfo};

/// Строка не совпала ни с одним значением перечисления.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownName(pub String);

impl std::fmt::Display for UnknownName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "unknown value `{}`", self.0)
    }
}

impl std::error::Error for UnknownName {}
