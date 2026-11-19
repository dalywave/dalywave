//! Сервер DalyWave: API под `/api`, вход через daly.rest, каталог Deezer, медиатека, очередь задач,
//! события в реальном времени, рекомендации и «Моя волна», админка и статика интерфейса.
//! Второй бинарник — `dalywave-worker`.

pub mod accounts;
pub mod acquire;
pub mod admin_live;
pub mod audit;
pub mod auth;
pub mod boot;
pub mod catalog;
pub mod charts;
pub mod choice;
pub mod chorus;
pub mod config;
pub mod connect;
pub mod crypto;
pub mod cursor;
pub mod deezer;
pub mod devices;
pub mod downloaders;
pub mod embed;
pub mod error;
pub mod export;
pub mod extract;
pub mod features;
pub mod handoff;
pub mod images;
pub mod import;
pub mod info;
pub mod jam;
pub mod jobs;
pub mod library;
pub mod links;
pub mod lyrics;
pub mod mdns;
pub mod ml;
pub mod notify;
pub mod oidc;
pub mod openapi;
pub mod overlay;
pub mod party;
pub mod presave;
pub mod ratelimit;
pub mod realtime;
pub mod reco;
pub mod renditions;
pub mod rewind;
pub mod routes;
pub mod security;
pub mod session;
pub mod share;
pub mod social;
pub mod spa;
pub mod state;
pub mod storage;
pub mod story;
pub mod users;
pub mod versions;
pub mod video;

pub use config::Config;
pub use state::AppState;

pub static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");

/// Полный роутер приложения: и для `main`, и для интеграционных тестов.
pub fn app(state: AppState) -> axum::Router {
    routes::router(state)
}
