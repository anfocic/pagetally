use crate::types::{RawPayload, Summary, TimeseriesPoint, TopDimension, TopRow, Vitals};

const PAGELEAVE_DUR_MAX_MS: i32 = 1_800_000;
use sqlx::Row;
use sqlx::postgres::{PgPool, PgPoolOptions};
use std::time::Duration;

pub async fn connect(database_url: &str) -> sqlx::Result<PgPool> {
    PgPoolOptions::new()
        .max_connections(10)
        .acquire_timeout(Duration::from_secs(5))
        .connect(database_url)
        .await
}

pub async fn migrate(pool: &PgPool) -> Result<(), sqlx::migrate::MigrateError> {
    sqlx::migrate!("./migrations").run(pool).await
}

pub async fn insert_event(
    pool: &PgPool,
    payload: &RawPayload,
    country: Option<&str>,
) -> sqlx::Result<()> {
    let (
        site_id,
        kind,
        path,
        ts,
        referrer,
        device,
        viewport,
        event_name,
        event_props,
        metrics,
        dur_ms,
    ) = match payload {
        RawPayload::Pageview { s, p, ts, r, d, v } => (
            s.as_str(),
            "pageview",
            p.as_str(),
            *ts,
            r.as_deref(),
            d.as_deref(),
            *v,
            None,
            None,
            None,
            None,
        ),
        RawPayload::Event { s, p, ts, n, pr } => (
            s.as_str(),
            "event",
            p.as_str(),
            *ts,
            None,
            None,
            None,
            Some(n.as_str()),
            pr.as_ref().map(|m| serde_json::to_value(m).unwrap()),
            None,
            None,
        ),
        RawPayload::Performance { s, p, ts, pf } => (
            s.as_str(),
            "performance",
            p.as_str(),
            *ts,
            None,
            None,
            None,
            None,
            None,
            Some(serde_json::to_value(pf).unwrap()),
            None,
        ),
        RawPayload::Pageleave { s, p, ts, dur } => (
            s.as_str(),
            "pageleave",
            p.as_str(),
            *ts,
            None,
            None,
            None,
            None,
            None,
            None,
            Some((*dur).clamp(0, PAGELEAVE_DUR_MAX_MS)),
        ),
    };

    sqlx::query(
        "INSERT INTO analytics_events
            (site_id, type, path, ts, referrer, device, viewport, event_name, event_props, metrics, country, dur_ms)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)",
    )
    .bind(site_id)
    .bind(kind)
    .bind(path)
    .bind(ts)
    .bind(referrer)
    .bind(device)
    .bind(viewport)
    .bind(event_name)
    .bind(event_props)
    .bind(metrics)
    .bind(country)
    .bind(dur_ms)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn summary(
    pool: &PgPool,
    site_id: &str,
    from_ts: i64,
    to_ts: i64,
) -> sqlx::Result<Summary> {
    let row = sqlx::query(
        "SELECT
            COUNT(*) FILTER (WHERE type = 'pageview')::bigint AS pageviews,
            COUNT(*) FILTER (WHERE type = 'event')::bigint     AS events,
            (AVG(dur_ms) FILTER (WHERE type = 'pageleave'))::float8 AS avg_time_on_page_ms,
            (
              SELECT path FROM analytics_events
               WHERE site_id = $1 AND ts BETWEEN $2 AND $3 AND type = 'pageview'
               GROUP BY path ORDER BY COUNT(*) DESC LIMIT 1
            ) AS top_path
         FROM analytics_events
         WHERE site_id = $1 AND ts BETWEEN $2 AND $3",
    )
    .bind(site_id)
    .bind(from_ts)
    .bind(to_ts)
    .fetch_one(pool)
    .await?;

    Ok(Summary {
        pageviews: row.try_get("pageviews").unwrap_or(0),
        events: row.try_get("events").unwrap_or(0),
        top_path: row.try_get::<Option<String>, _>("top_path").ok().flatten(),
        avg_time_on_page_ms: row
            .try_get::<Option<f64>, _>("avg_time_on_page_ms")
            .ok()
            .flatten(),
    })
}

pub async fn timeseries(
    pool: &PgPool,
    site_id: &str,
    from_ts: i64,
    to_ts: i64,
    bucket: &str,
) -> sqlx::Result<Vec<TimeseriesPoint>> {
    let trunc = if bucket == "hour" { "hour" } else { "day" };
    let rows = sqlx::query(&format!(
        "SELECT date_trunc('{trunc}', to_timestamp(ts / 1000.0)) AS bucket,
                COUNT(*)::bigint AS pageviews
         FROM analytics_events
         WHERE site_id = $1 AND ts BETWEEN $2 AND $3 AND type = 'pageview'
         GROUP BY bucket ORDER BY bucket ASC"
    ))
    .bind(site_id)
    .bind(from_ts)
    .bind(to_ts)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| TimeseriesPoint {
            bucket: r.get("bucket"),
            pageviews: r.try_get("pageviews").unwrap_or(0),
        })
        .collect())
}

pub async fn top(
    pool: &PgPool,
    site_id: &str,
    from_ts: i64,
    to_ts: i64,
    dim: TopDimension,
    limit: i64,
) -> sqlx::Result<Vec<TopRow>> {
    if matches!(dim, TopDimension::Path) {
        let rows = sqlx::query(
            "SELECT path AS key,
                    COUNT(*) FILTER (WHERE type = 'pageview')::bigint AS count,
                    (AVG(dur_ms) FILTER (WHERE type = 'pageleave'))::float8 AS avg_dur_ms
             FROM analytics_events
             WHERE site_id = $1 AND ts BETWEEN $2 AND $3
                   AND type IN ('pageview', 'pageleave')
                   AND path IS NOT NULL
             GROUP BY path
             HAVING COUNT(*) FILTER (WHERE type = 'pageview') > 0
             ORDER BY count DESC
             LIMIT $4",
        )
        .bind(site_id)
        .bind(from_ts)
        .bind(to_ts)
        .bind(limit)
        .fetch_all(pool)
        .await?;

        return Ok(rows
            .into_iter()
            .map(|r| TopRow {
                key: r
                    .try_get::<Option<String>, _>("key")
                    .ok()
                    .flatten()
                    .unwrap_or_else(|| "(none)".into()),
                count: r.try_get("count").unwrap_or(0),
                avg_dur_ms: r.try_get::<Option<f64>, _>("avg_dur_ms").ok().flatten(),
            })
            .collect());
    }

    let col = dim.column();
    let rows = sqlx::query(&format!(
        "SELECT {col} AS key, COUNT(*)::bigint AS count
         FROM analytics_events
         WHERE site_id = $1 AND ts BETWEEN $2 AND $3 AND type = 'pageview'
               AND {col} IS NOT NULL
         GROUP BY {col} ORDER BY count DESC LIMIT $4"
    ))
    .bind(site_id)
    .bind(from_ts)
    .bind(to_ts)
    .bind(limit)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| TopRow {
            key: r
                .try_get::<Option<String>, _>("key")
                .ok()
                .flatten()
                .unwrap_or_else(|| "(none)".into()),
            count: r.try_get("count").unwrap_or(0),
            avg_dur_ms: None,
        })
        .collect())
}

pub async fn vitals(
    pool: &PgPool,
    site_id: &str,
    from_ts: i64,
    to_ts: i64,
) -> sqlx::Result<Vitals> {
    let row = sqlx::query(
        "SELECT
            (percentile_cont(0.75) WITHIN GROUP (ORDER BY (metrics->>'lcp')::numeric))::float8  AS lcp,
            (percentile_cont(0.75) WITHIN GROUP (ORDER BY (metrics->>'fcp')::numeric))::float8  AS fcp,
            (percentile_cont(0.75) WITHIN GROUP (ORDER BY (metrics->>'cls')::numeric))::float8  AS cls,
            (percentile_cont(0.75) WITHIN GROUP (ORDER BY (metrics->>'inp')::numeric))::float8  AS inp,
            (percentile_cont(0.75) WITHIN GROUP (ORDER BY (metrics->>'ttfb')::numeric))::float8 AS ttfb
         FROM analytics_events
         WHERE site_id = $1 AND ts BETWEEN $2 AND $3 AND type = 'performance'",
    )
    .bind(site_id)
    .bind(from_ts)
    .bind(to_ts)
    .fetch_one(pool)
    .await?;

    let pick = |k: &str| -> Option<f64> { row.try_get::<Option<f64>, _>(k).ok().flatten() };

    Ok(Vitals {
        lcp_p75: pick("lcp"),
        fcp_p75: pick("fcp"),
        cls_p75: pick("cls"),
        inp_p75: pick("inp"),
        ttfb_p75: pick("ttfb"),
    })
}
