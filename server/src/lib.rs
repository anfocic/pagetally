pub mod config;
pub mod contact;
pub mod db;
pub mod email;
pub mod ingest;
pub mod salt;
pub mod state;
pub mod stats;
pub mod types;
pub mod ua;

use axum::Router;
use axum::extract::{DefaultBodyLimit, State};
use axum::http::{HeaderMap, HeaderName, HeaderValue, StatusCode, header};
use axum::middleware::{self, Next};
use axum::routing::{get, post};
use axum_prometheus::PrometheusMetricLayer;
use sha2::{Digest, Sha256};
use state::AppState;
use std::sync::Arc;
use std::time::Duration;
use tower_governor::key_extractor::SmartIpKeyExtractor;
use tower_governor::{GovernorLayer, governor::GovernorConfigBuilder};
use tower_http::cors::{Any, CorsLayer};
use tower_http::request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer};
use tower_http::set_header::SetResponseHeaderLayer;
use tower_http::timeout::TimeoutLayer;
use tower_http::trace::{DefaultMakeSpan, DefaultOnResponse, TraceLayer};
use tracing::Level;

/// Build the application router with HTTP metrics + a `/metrics` endpoint.
/// This installs a process-wide Prometheus recorder, so it can only be called
/// once. `router()` (without metrics) is the entry point for tests and
/// fixtures that may run in parallel.
pub fn router_with_metrics(state: AppState) -> Router {
    let (metrics_layer, metrics_handle) = PrometheusMetricLayer::pair();
    let metrics_route = Router::new().route(
        "/metrics",
        get(move || {
            let handle = metrics_handle.clone();
            async move { handle.render() }
        }),
    );
    router(state).merge(metrics_route).layer(metrics_layer)
}

pub fn router(state: AppState) -> Router {
    let cors_collect = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let cors_stats = match state.config.stats_origins.as_ref() {
        Some(origins) if !origins.is_empty() => {
            let parsed: Vec<HeaderValue> = origins
                .iter()
                .filter_map(|o| HeaderValue::from_str(o).ok())
                .collect();
            CorsLayer::new()
                .allow_origin(parsed)
                .allow_methods([axum::http::Method::GET])
                .allow_headers([header::AUTHORIZATION, header::CONTENT_TYPE])
        }
        _ => CorsLayer::new()
            .allow_origin(Any)
            .allow_methods([axum::http::Method::GET])
            .allow_headers([header::AUTHORIZATION, header::CONTENT_TYPE]),
    };

    let stats_routes = Router::new()
        .route("/stats/summary", get(stats::summary))
        .route("/stats/timeseries", get(stats::timeseries))
        .route("/stats/top", get(stats::top))
        .route("/stats/events", get(stats::events))
        .route("/stats/vitals", get(stats::vitals))
        .route_layer(middleware::from_fn_with_state(state.clone(), require_admin))
        .layer(cors_stats);

    const PUBLIC_BODY_LIMIT: usize = 16 * 1024;

    // /collect: high volume, generous limit. burst absorbs SPA navigations
    // that fire pageleave + pageview close together. 500ms replenish period =
    // ~120/min sustained once the burst is spent.
    let collect_governor = Arc::new(
        GovernorConfigBuilder::default()
            .per_millisecond(500)
            .burst_size(60)
            .key_extractor(SmartIpKeyExtractor)
            .finish()
            .expect("collect rate-limit config is valid"),
    );
    // tower_governor's keyed store never evicts on its own; without this the
    // per-IP map grows without bound — a memory-exhaustion DoS under IP churn or
    // spoofed `x-forwarded-for`. Periodically drop fully-replenished entries.
    {
        let limiter = collect_governor.limiter().clone();
        tokio::spawn(async move {
            let mut tick = tokio::time::interval(Duration::from_secs(60));
            loop {
                tick.tick().await;
                limiter.retain_recent();
            }
        });
    }

    // /contact: low volume, strict. 5/min steady, burst 3.
    let contact_governor = Arc::new(
        GovernorConfigBuilder::default()
            .per_second(12)
            .burst_size(3)
            .key_extractor(SmartIpKeyExtractor)
            .finish()
            .expect("contact rate-limit config is valid"),
    );
    {
        let limiter = contact_governor.limiter().clone();
        tokio::spawn(async move {
            let mut tick = tokio::time::interval(Duration::from_secs(60));
            loop {
                tick.tick().await;
                limiter.retain_recent();
            }
        });
    }

    let collect_route = Router::new()
        .route("/collect", post(ingest::collect))
        .layer(GovernorLayer {
            config: collect_governor,
        })
        .layer(cors_collect.clone());

    let contact_route = Router::new()
        .route("/contact", post(contact::submit))
        .layer(GovernorLayer {
            config: contact_governor,
        })
        .layer(cors_collect);

    let public_routes = Router::new()
        .merge(collect_route)
        .merge(contact_route)
        .layer(DefaultBodyLimit::max(PUBLIC_BODY_LIMIT));

    let mut app = Router::new()
        .merge(public_routes)
        .route("/health", get(health))
        .merge(stats_routes)
        .with_state(state.clone());

    // Security response headers (defense in depth — most are also useful when
    // clients embed our endpoints in their own pages). CSP `default-src 'none'`
    // is appropriate because every response is JSON or plain text — nothing
    // we serve should ever load subresources or execute script.
    app = app
        .layer(SetResponseHeaderLayer::if_not_present(
            HeaderName::from_static("x-content-type-options"),
            HeaderValue::from_static("nosniff"),
        ))
        .layer(SetResponseHeaderLayer::if_not_present(
            HeaderName::from_static("referrer-policy"),
            HeaderValue::from_static("no-referrer"),
        ))
        .layer(SetResponseHeaderLayer::if_not_present(
            HeaderName::from_static("x-frame-options"),
            HeaderValue::from_static("DENY"),
        ))
        .layer(SetResponseHeaderLayer::if_not_present(
            HeaderName::from_static("content-security-policy"),
            HeaderValue::from_static("default-src 'none'; frame-ancestors 'none'"),
        ));

    if state.config.behind_tls {
        app = app.layer(SetResponseHeaderLayer::if_not_present(
            HeaderName::from_static("strict-transport-security"),
            HeaderValue::from_static("max-age=31536000; includeSubDomains"),
        ));
    }

    let x_request_id = HeaderName::from_static("x-request-id");

    app.layer(TimeoutLayer::new(Duration::from_secs(15)))
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(
                    DefaultMakeSpan::new()
                        .level(Level::INFO)
                        .include_headers(false),
                )
                .on_response(DefaultOnResponse::new().level(Level::INFO)),
        )
        .layer(PropagateRequestIdLayer::new(x_request_id.clone()))
        .layer(SetRequestIdLayer::new(x_request_id, MakeRequestUuid))
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

/// Constant-time token comparison. Both sides are hashed to a fixed-width
/// digest first, so the comparison leaks neither the contents nor the length of
/// the expected token (the previous length check returned early on a mismatch,
/// revealing the correct token length via timing).
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    let a = Sha256::digest(a);
    let b = Sha256::digest(b);
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

async fn health() -> &'static str {
    "ok"
}
