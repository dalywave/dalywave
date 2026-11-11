//! Интеграционные тесты API. Нужен Postgres из `DATABASE_URL` (`make up`):
//! `DATABASE_URL=postgres://dalywave:dalywave@localhost:5400/dalywave cargo test`.
//! `sqlx::test` создаёт отдельную базу на каждый тест; daly.rest заменяет заглушка из `mock.rs`.

mod admin;
mod app_login;
mod common;
mod mock;
mod password;
mod system;
mod web_login;
