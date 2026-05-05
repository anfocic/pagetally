use crate::state::AppState;
use crate::types::RawPayload;
use axum::Json;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};

pub async fn collect(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<RawPayload>,
) -> StatusCode {
    let site_id = payload.site_id();
    if let Some(allowed) = &state.config.allowed_sites
        && !allowed.iter().any(|s| s == site_id)
    {
        return StatusCode::FORBIDDEN;
    }

    let country = headers
        .get("x-country")
        .and_then(|v| v.to_str().ok())
        .filter(|c| c.len() == 2)
        .map(|c| c.to_uppercase());

    let pool = state.pool.clone();
    tokio::spawn(async move {
        if let Err(err) = crate::db::insert_event(&pool, &payload, country.as_deref()).await {
            tracing::warn!(error = %err, "failed to insert event");
        }
    });

    StatusCode::ACCEPTED
}
