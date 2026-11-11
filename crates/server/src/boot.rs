//! Общий запуск бинарников `dalywave-api` и `dalywave-worker`: логи, база с повторами, сигнал остановки.

use std::{io::IsTerminal, time::Duration};

use anyhow::Context;
use sqlx::{PgPool, postgres::PgPoolOptions};
use tracing_subscriber::EnvFilter;

/// Фильтр из DALYWAVE_LOG; DALYWAVE_LOG_FORMAT=json — строки JSON для сервера, иначе читаемый вывод.
pub fn init_tracing(default_filter: &str) {
    let filter = std::env::var("DALYWAVE_LOG")
        .ok()
        .filter(|f| !f.trim().is_empty())
        .and_then(|f| EnvFilter::try_new(f).ok())
        .unwrap_or_else(|| EnvFilter::new(default_filter));
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

/// База в compose может подниматься дольше приложения, поэтому пробуем несколько раз.
pub async fn connect(url: &str, max_connections: u32) -> anyhow::Result<PgPool> {
    let mut attempt = 0;
    loop {
        attempt += 1;
        match PgPoolOptions::new()
            .max_connections(max_connections)
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

/// Ctrl+C локально и SIGTERM от docker.
pub async fn shutdown_signal() {
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
