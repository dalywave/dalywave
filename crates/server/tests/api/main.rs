//! Интеграционные тесты API. Нужен Postgres из `DATABASE_URL` (`make up`):
//! `DATABASE_URL=postgres://dalywave:dalywave@localhost:5400/dalywave cargo test`.
//! `sqlx::test` создаёт отдельную базу на каждый тест; daly.rest заменяет заглушка из `mock.rs`,
//! Deezer — заглушка с записанными ответами из `deezer.rs`, хранилище S3 — хранилище в памяти.

mod admin;
mod admin_sections;
mod analysis;
mod app_login;
mod catalog;
mod common;
mod connect;
mod deezer;
mod describe;
mod events;
mod explicit;
mod fake_ml;
mod fetcher;
mod folders;
mod friends;
mod history;
mod home;
mod images;
mod import_sources;
mod imports;
mod jam;
mod jobs;
mod library;
mod lyrics;
mod mock;
mod notifications;
mod offline;
mod password;
mod playlist_set;
mod playlists;
mod quality;
mod realtime;
mod reco;
mod recognize;
mod rewind;
mod share;
mod storage;
mod stream;
mod sync;
mod synth;
mod system;
mod taste;
mod taste_model;
mod updates;
mod versions;
mod watch;
mod wave_prep;
mod wave_presets;
mod web_login;
