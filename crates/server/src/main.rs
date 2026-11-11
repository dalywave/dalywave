use std::{io::IsTerminal, net::SocketAddr, time::Duration};

use anyhow::Context;
use dalywave_server::{AppState, Config};
use sqlx::postgres::PgPoolOptions;
use tokio::net::TcpListener;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Локально настройки лежат в .env в корне репозитория, на сервере их передаёт compose
    let _ = dotenvy::dotenv();
    init_tracing();

    let config = Config::from_env()?;
    let db = connect(&config.database_url).await?;
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
        "configured"
    );
    if state.oidc.is_none() && state.config.admin_password.is_none() {
        tracing::warn!("no way to sign in: set DALYWAVE_OIDC_* or DALYWAVE_ADMIN_PASSWORD");
    }

    let listener = TcpListener::bind(state.config.listen)
        .await
        .with_context(|| format!("listen on {}", state.config.listen))?;
    tracing::info!("listening on {}", listener.local_addr()?);
    axum::serve(
        listener,
        dalywave_server::app(state).into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal())
    .await?;
    tracing::info!("stopped");
    Ok(())
}

/// Фильтр из DALYWAVE_LOG; DALYWAVE_LOG_FORMAT=json — строки JSON для сервера, иначе читаемый вывод.
fn init_tracing() {
    // dalywave_api — сам бинарник (этот файл), dalywave_server — библиотека с обработчиками
    let filter = std::env::var("DALYWAVE_LOG")
        .ok()
        .and_then(|f| EnvFilter::try_new(f).ok())
        .unwrap_or_else(|| {
            EnvFilter::new("dalywave_api=info,dalywave_server=info,tower_http=info")
        });
    let json = std::env::var("DALYWAVE_LOG_FORMAT").is_ok_and(|f| f.trim() == "json");
    let builder = tracing_subscriber::fmt().with_env_filter(filter);
    if json {
        builder
            .json()
            .flatten_event(true)
            .with_current_span(true)
            .with_span_list(false)
            .init();
    } else {
        // Цвета — только в терминале: в файле и в docker logs они превращаются в мусор
        builder.with_ansi(std::io::stdout().is_terminal()).init();
    }
}

/// База в compose может подниматься дольше API, поэтому пробуем несколько раз.
async fn connect(url: &str) -> anyhow::Result<sqlx::PgPool> {
    let mut attempt = 0;
    loop {
        attempt += 1;
        match PgPoolOptions::new()
            .max_connections(10)
            .acquire_timeout(Duration::from_secs(5))
            .connect(url)
            .await
        {
            Ok(pool) => return Ok(pool),
            Err(e) if attempt < 15 => {
                tracing::warn!(attempt, error = %e, "database is not ready, retrying");
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
            Err(e) => return Err(e).context("connect to database"),
        }
    }
}

/// Ctrl+C локально и SIGTERM от docker: начатые запросы дорабатывают, новые не принимаются.
async fn shutdown_signal() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    #[cfg(unix)]
    let terminate = async {
        if let Ok(mut s) = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        {
            s.recv().await;
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();
    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
    tracing::info!("shutting down");
}
