//! Исполнитель очереди задач DalyWave: копии картинок, обновление каталога, сверка подписок, а с
//! `server` в `DALYWAVE_FETCHER_MODE` — скачивание аудио через серверный fetcher (иначе его берёт
//! загрузчик на Mac). Тот же образ, что у API, команда `dalywave-worker`.

use std::sync::Arc;

use anyhow::Context as _;
use dalywave_server::{
    Config, boot,
    deezer::Deezer,
    jobs::{
        handlers,
        worker::{Context, Settings, Worker},
    },
    storage::Storage,
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let _ = dotenvy::dotenv();
    boot::init_tracing("dalywave_worker=info,dalywave_server=info");

    let config = Config::from_env()?;
    let db = boot::connect(&config.database_url, 10).await?;
    // Миграции может начать и воркер: sqlx берёт advisory lock, двойного применения не будет
    dalywave_server::MIGRATOR
        .run(&db)
        .await
        .context("run migrations")?;

    let storage = match &config.s3 {
        Some(s3) => Storage::from_config(s3)?,
        None => {
            tracing::warn!("S3 is not configured: files are kept in memory and lost on restart");
            Storage::memory()
        }
    };
    if let Err(e) = storage.ensure_bucket().await {
        tracing::warn!(error = %e, "S3 bucket check failed");
    }
    let deezer = Deezer::new(&config.deezer.url, config.deezer.rate_worker)?;
    let registry = handlers::registry(&config.fetcher);
    tracing::info!(
        server_fetcher = config.fetcher.server,
        fetcher_url = %config.fetcher.url,
        "audio is acquired by {}",
        if config.fetcher.server { "the server fetcher and home downloaders" } else { "home downloaders" }
    );
    let ctx = Context::new(db, Arc::new(config), Arc::new(deezer), Arc::new(storage))?;
    tracing::info!(
        apns = ctx.push.apns.is_some(),
        web_push = ctx.push.webpush.is_some(),
        "push channels"
    );
    let worker = Worker::new(Arc::new(ctx), registry, Settings::default());
    worker.run(boot::shutdown_signal()).await
}
