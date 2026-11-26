use std::{net::SocketAddr, sync::Arc};

use anyhow::Context;
use dalywave_server::{AppState, Config, boot, realtime};
use tokio::net::TcpListener;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // `dalywave-api vapid-keys` — новая пара Web Push для .env. Печатает только строки для .env (без логов),
    // чтобы вывод можно было сразу дописать в файл: `… vapid-keys >> .env`
    if std::env::args().nth(1).as_deref() == Some("vapid-keys") {
        let (public, private) = dalywave_server::notify::webpush::generate_keys()?;
        println!(
            "# Web Push (VAPID): dalywave-api vapid-keys, {}",
            chrono::Utc::now().format("%Y-%m-%d")
        );
        println!("DALYWAVE_VAPID_PUBLIC={public}");
        println!("DALYWAVE_VAPID_PRIVATE={private}");
        return Ok(());
    }
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

    dalywave_server::features::set(config.features);
    let state = AppState::new(db, config)?;
    tracing::info!(
        public_url = %state.config.public_url,
        oidc = ?state.oidc.iter().map(|p| p.id()).collect::<Vec<_>>(),
        local_auth = state.config.local_auth,
        apns = state.config.push.apns.is_some(),
        web_push = state.config.push.vapid.is_some(),
        password = state.config.admin_password.is_some(),
        demo = ?state.config.demo_account,
        static_dir = ?state.config.static_dir,
        storage = ?state.storage,
        deezer = state.config.features.deezer,
        downloader = state.config.features.downloader,
        music_dirs = ?state.config.library.dirs,
        "configured"
    );
    if state.oidc.is_empty() && !state.config.local_auth && state.config.admin_password.is_none() {
        tracing::warn!(
            "no way to sign in: set DALYWAVE_OIDC_*, DALYWAVE_LOCAL_AUTH=true or DALYWAVE_ADMIN_PASSWORD"
        );
    }
    if let Some(name) = &state.config.demo_account {
        tracing::warn!(
            account = %name,
            "DALYWAVE_DEMO_ACCOUNT is set: anyone can sign in to this account without a password"
        );
    }
    if state.storage.is_memory() {
        tracing::warn!("storage is not configured: files are kept in memory and lost on restart");
    } else if let Err(e) = state.storage.ensure_bucket().await {
        tracing::warn!(error = %e, "storage check failed");
    }
    let listener_task = realtime::spawn_listener(state.db.clone(), Arc::clone(&state.hub));

    let listener = TcpListener::bind(state.config.listen)
        .await
        .with_context(|| format!("listen on {}", state.config.listen))?;
    let addr = listener.local_addr()?;
    tracing::info!("listening on {addr}");
    // Объявление в локальной сети (`_dalywave._tcp`) живёт, пока работает API
    let _mdns = dalywave_server::mdns::announce(&state.config, addr.port());
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
