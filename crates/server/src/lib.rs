//! Сервер DalyWave: API под `/api`, вход через daly.rest, каталог Deezer, медиатека, очередь задач,
//! события в реальном времени, рекомендации и «Моя волна», админка и статика интерфейса.
//! Второй бинарник — `dalywave-worker`.

pub mod acquire;
pub mod audit;
pub mod auth;
pub mod boot;
pub mod catalog;
pub mod config;
pub mod crypto;
pub mod cursor;
pub mod deezer;
pub mod devices;
pub mod downloaders;
pub mod error;
pub mod extract;
pub mod images;
pub mod jobs;
pub mod ml;
pub mod oidc;
pub mod openapi;
pub mod ratelimit;
pub mod realtime;
pub mod reco;
pub mod routes;
pub mod security;
pub mod session;
pub mod spa;
pub mod state;
pub mod storage;
pub mod users;

pub use config::Config;
pub use state::AppState;

pub static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");

/// Полный роутер приложения: и для `main`, и для интеграционных тестов.
pub fn app(state: AppState) -> axum::Router {
    routes::router(state)
}
