use crate::state::AppState;
use crate::types::TopDimension;
use axum::Json;
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct RangeQuery {
    pub site: String,
    #[serde(default = "default_days")]
    pub days: u32,
}

fn default_days() -> u32 {
    30
}

#[derive(Debug, Deserialize)]
pub struct TimeseriesQuery {
    pub site: String,
    #[serde(default = "default_days")]
    pub days: u32,
    #[serde(default)]
    pub bucket: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct TopQuery {
    pub site: String,
    #[serde(default = "default_days")]
    pub days: u32,
    pub dim: String,
    #[serde(default = "default_limit")]
    pub limit: u32,
}

fn default_limit() -> u32 {
    10
}

#[derive(Debug, Deserialize)]
pub struct EventsQuery {
    pub site: String,
    #[serde(default = "default_days")]
    pub days: u32,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub by: Option<String>,
    #[serde(default = "default_limit")]
    pub limit: u32,
}

fn range(days: u32) -> (i64, i64) {
    let days = days.clamp(1, 365) as i64;
    let to_ts = chrono::Utc::now().timestamp_millis();
    let from_ts = to_ts - days * 24 * 60 * 60 * 1000;
    (from_ts, to_ts)
}

fn site_check(state: &AppState, site: &str) -> Result<(), StatusCode> {
    if let Some(allowed) = &state.config.allowed_sites
        && !allowed.iter().any(|s| s == site)
    {
        return Err(StatusCode::FORBIDDEN);
    }
    Ok(())
}

pub async fn summary(
    State(state): State<AppState>,
    Query(q): Query<RangeQuery>,
) -> Result<impl IntoResponse, StatusCode> {
    site_check(&state, &q.site)?;
    let (from_ts, to_ts) = range(q.days);
    let s = crate::db::summary(&state.pool, &q.site, from_ts, to_ts)
        .await
        .map_err(|err| {
            tracing::error!(error = %err, "summary query failed");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    Ok(Json(s))
}

pub async fn timeseries(
    State(state): State<AppState>,
    Query(q): Query<TimeseriesQuery>,
) -> Result<impl IntoResponse, StatusCode> {
    site_check(&state, &q.site)?;
    let (from_ts, to_ts) = range(q.days);
    let bucket = q.bucket.as_deref().unwrap_or("day");
    let bucket = if bucket == "hour" { "hour" } else { "day" };
    let rows = crate::db::timeseries(&state.pool, &q.site, from_ts, to_ts, bucket)
        .await
        .map_err(|err| {
            tracing::error!(error = %err, "timeseries query failed");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    Ok(Json(rows))
}

pub async fn top(
    State(state): State<AppState>,
    Query(q): Query<TopQuery>,
) -> Result<impl IntoResponse, StatusCode> {
    site_check(&state, &q.site)?;
    let dim = TopDimension::parse(&q.dim).ok_or(StatusCode::BAD_REQUEST)?;
    let limit = q.limit.clamp(1, 100) as i64;
    let (from_ts, to_ts) = range(q.days);
    let rows = crate::db::top(&state.pool, &q.site, from_ts, to_ts, dim, limit)
        .await
        .map_err(|err| {
            tracing::error!(error = %err, "top query failed");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    Ok(Json(rows))
}

pub async fn events(
    State(state): State<AppState>,
    Query(q): Query<EventsQuery>,
) -> Result<impl IntoResponse, StatusCode> {
    site_check(&state, &q.site)?;
    // A prop breakdown needs an event to break down; reject `by` without `name`.
    if q.by.is_some() && q.name.is_none() {
        return Err(StatusCode::BAD_REQUEST);
    }
    let limit = q.limit.clamp(1, 100) as i64;
    let (from_ts, to_ts) = range(q.days);
    let rows = crate::db::events(
        &state.pool,
        &q.site,
        from_ts,
        to_ts,
        q.name.as_deref(),
        q.by.as_deref(),
        limit,
    )
    .await
    .map_err(|err| {
        tracing::error!(error = %err, "events query failed");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    Ok(Json(rows))
}

pub async fn vitals(
    State(state): State<AppState>,
    Query(q): Query<RangeQuery>,
) -> Result<impl IntoResponse, StatusCode> {
    site_check(&state, &q.site)?;
    let (from_ts, to_ts) = range(q.days);
    let v = crate::db::vitals(&state.pool, &q.site, from_ts, to_ts)
        .await
        .map_err(|err| {
            tracing::error!(error = %err, "vitals query failed");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    Ok(Json(v))
}
