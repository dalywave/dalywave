use std::{net::SocketAddr, sync::Arc};

use anyhow::Context;
use dalywave_server::{AppState, Config, boot, realtime};
use tokio::net::TcpListener;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Локально настройки лежат в .env в корне репозитория, на сервере их передаёт compose
    let _ = dotenvy::dotenv();
    // dalywave_api — сам бинарник (этот файл), dalywave_server — библиотека с обработчиками
    boot::init_tracing("dalywave_api=info,dalywave_server=info,tower_http=info");

    let config = Config::from_env()?;
    let db = boot::connect(&config.database_url, 10).await?;
    dalywave_server::MIGRATOR
        .run(&db)
        .await
        .context("run migrations")?;

    let state = AppState::new(db, config)?;
    tracing::info!(
        public_url = %state.config.public_url,
        oidc = state.oidc.is_some(),
        password = state.config.admin_password.is_some(),
        static_dir = ?state.config.static_dir,
        storage = ?state.storage,
        "configured"
    );
    if state.oidc.is_none() && state.config.admin_password.is_none() {
        tracing::warn!("no way to sign in: set DALYWAVE_OIDC_* or DALYWAVE_ADMIN_PASSWORD");
    }
    if state.storage.is_memory() {
        tracing::warn!("S3 is not configured: files are kept in memory and lost on restart");
    } else if let Err(e) = state.storage.ensure_bucket().await {
        tracing::warn!(error = %e, "S3 bucket check failed");
    }
    let listener_task = realtime::spawn_listener(state.db.clone(), Arc::clone(&state.hub));

    let listener = TcpListener::bind(state.config.listen)
        .await
        .with_context(|| format!("listen on {}", state.config.listen))?;
    tracing::info!("listening on {}", listener.local_addr()?);
    let hub = Arc::clone(&state.hub);
    axum::serve(
        listener,
        dalywave_server::app(state).into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(async move {
        boot::shutdown_signal().await;
        // WebSocket не держат остановку: им уходит закрытие «сервер перезапускается»
        hub.shutdown();
    })
    .await?;
    listener_task.abort();
    tracing::info!("stopped");
    Ok(())
}
