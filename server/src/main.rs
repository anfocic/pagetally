use pagetally_server::{config::Config, db, email::Mailer, router, state::AppState};
use std::sync::Arc;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info,sqlx=warn".into()))
        .init();

    let config = Config::from_env()?;
    tracing::info!(addr = %config.bind_addr, "starting pagetally");

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
    axum::serve(listener, app).await?;
    Ok(())
}
