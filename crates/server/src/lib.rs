//! Сервер DalyWave: API под `/api`, вход через daly.rest, сессии, админка и статика интерфейса.

pub mod audit;
pub mod auth;
pub mod config;
pub mod crypto;
pub mod devices;
pub mod error;
pub mod extract;
pub mod oidc;
pub mod openapi;
pub mod ratelimit;
pub mod routes;
pub mod security;
pub mod session;
pub mod spa;
pub mod state;
pub mod users;

pub use config::Config;
pub use state::AppState;

pub static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");

/// Полный роутер приложения: и для `main`, и для интеграционных тестов.
pub fn app(state: AppState) -> axum::Router {
    routes::router(state)
}
