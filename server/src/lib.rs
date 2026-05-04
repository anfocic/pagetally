pub mod config;
pub mod db;
pub mod email;
pub mod ingest;
pub mod stats;
pub mod state;
pub mod types;

use axum::routing::{get, post};
use axum::Router;
use state::AppState;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;

pub fn router(state: AppState) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    Router::new()
        .route("/collect", post(ingest::collect))
        .route("/stats/summary", get(stats::summary))
        .route("/stats/timeseries", get(stats::timeseries))
        .route("/stats/top", get(stats::top))
        .route("/stats/vitals", get(stats::vitals))
        .route("/health", get(health))
        .with_state(state)
        .layer(cors)
        .layer(TraceLayer::new_for_http())
}

async fn health() -> &'static str {
    "ok"
}
