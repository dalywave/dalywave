//! Интеграционные тесты API. Нужен Postgres из `DATABASE_URL` (`make up`):
//! `DATABASE_URL=postgres://dalywave:dalywave@localhost:5400/dalywave cargo test`.
//! `sqlx::test` создаёт отдельную базу на каждый тест; daly.rest заменяет заглушка из `mock.rs`,
//! Deezer — заглушка с записанными ответами из `deezer.rs`, хранилище S3 — хранилище в памяти.

mod admin;
mod app_login;
mod catalog;
mod common;
mod deezer;
mod events;
mod home;
mod images;
mod jobs;
mod library;
mod mock;
mod password;
mod playlists;
mod realtime;
mod storage;
mod sync;
mod system;
mod web_login;
