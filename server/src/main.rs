use pagetally_server::{config::Config, db, email::Mailer, router, state::AppState};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::signal;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| "info,sqlx=warn".into()),
        )
        .init();

    let config = Config::from_env()?;
    tracing::info!(addr = %config.bind_addr, "starting pagetally");

    if config.admin_token.is_none() {
        tracing::warn!(
            "ADMIN_TOKEN is not set — /stats/* endpoints are publicly readable. \
             Set ADMIN_TOKEN to require Bearer auth."
        );
    }

    let pool = db::connect(&config.database_url).await?;
    db::migrate(&pool).await?;

    let mailer = config.email.clone().map(Mailer::new);

    let state = AppState {
        config: Arc::new(config.clone()),
        pool,
        mailer,
    };

    let app = router(state);

    let listener = tokio::net::TcpListener::bind(&config.bind_addr).await?;
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal())
    .await?;
    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
    tracing::info!("shutdown signal received, draining connections");
}
