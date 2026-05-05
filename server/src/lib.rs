pub mod config;
pub mod contact;
pub mod db;
pub mod email;
pub mod ingest;
pub mod stats;
pub mod state;
pub mod types;

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::middleware::{self, Next};
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

    let stats_routes = Router::new()
        .route("/stats/summary", get(stats::summary))
        .route("/stats/timeseries", get(stats::timeseries))
        .route("/stats/top", get(stats::top))
        .route("/stats/vitals", get(stats::vitals))
        .route_layer(middleware::from_fn_with_state(state.clone(), require_admin));

    Router::new()
        .route("/collect", post(ingest::collect))
        .route("/contact", post(contact::submit))
        .route("/health", get(health))
        .merge(stats_routes)
        .with_state(state)
        .layer(cors)
        .layer(TraceLayer::new_for_http())
}

async fn require_admin(
    State(state): State<AppState>,
    headers: HeaderMap,
    request: axum::extract::Request,
    next: Next,
) -> Result<axum::response::Response, StatusCode> {
    let Some(expected) = state.config.admin_token.as_deref() else {
        return Ok(next.run(request).await);
    };

    let presented = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "))
        .map(str::trim);

    match presented {
        Some(token) if constant_time_eq(token.as_bytes(), expected.as_bytes()) => {
            Ok(next.run(request).await)
        }
        _ => Err(StatusCode::UNAUTHORIZED),
    }
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

async fn health() -> &'static str {
    "ok"
}
