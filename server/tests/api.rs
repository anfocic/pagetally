use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use http_body_util::BodyExt;
use pagetally_server::{config::Config, router, router_with_metrics, state::AppState};
use serde_json::{Value, json};
use sqlx::PgPool;
use std::sync::Arc;
use std::time::Duration;
use tower::ServiceExt;

fn test_state(
    pool: PgPool,
    admin_token: Option<&str>,
    allowed_sites: Option<Vec<String>>,
) -> AppState {
    state_with(pool, admin_token, allowed_sites, false)
}

fn state_with(
    pool: PgPool,
    admin_token: Option<&str>,
    allowed_sites: Option<Vec<String>>,
    sessions_enabled: bool,
) -> AppState {
    AppState {
        config: Arc::new(Config {
            bind_addr: "0.0.0.0:0".into(),
            database_url: String::new(),
            allowed_sites,
            admin_token: admin_token.map(String::from),
            email: None,
            contact_to: None,
            stats_origins: None,
            behind_tls: false,
            sessions_enabled,
        }),
        pool,
        mailer: None,
        salt_cache: pagetally_server::salt::new_cache(),
    }
}

fn post_collect(body: Value) -> Request<Body> {
    Request::builder()
        .uri("/collect")
        .method("POST")
        .header(header::CONTENT_TYPE, "application/json")
        // SmartIpKeyExtractor needs an IP source; provide one explicitly.
        .header("x-forwarded-for", "10.0.0.1")
        .body(Body::from(body.to_string()))
        .unwrap()
}

const CHROME_WIN: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 \
     (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";

fn post_collect_ua(body: Value, ip: &str, ua: &str) -> Request<Body> {
    Request::builder()
        .uri("/collect")
        .method("POST")
        .header(header::CONTENT_TYPE, "application/json")
        .header("x-forwarded-for", ip)
        .header(header::USER_AGENT, ua)
        .body(Body::from(body.to_string()))
        .unwrap()
}

async fn body_json(resp: axum::response::Response) -> Value {
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap_or(Value::Null)
}

async fn wait_for_count(pool: &PgPool, expected: i64) {
    for _ in 0..50 {
        let n: i64 = sqlx::query_scalar("SELECT count(*) FROM analytics_events")
            .fetch_one(pool)
            .await
            .unwrap();
        if n >= expected {
            return;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    panic!("timed out waiting for {expected} rows");
}

#[sqlx::test]
async fn health_returns_ok(pool: PgPool) {
    let app = router(test_state(pool, None, None));
    let resp = app
        .oneshot(Request::get("/health").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[sqlx::test]
async fn collect_inserts_pageview(pool: PgPool) {
    let app = router(test_state(pool.clone(), None, None));
    let resp = app
        .oneshot(post_collect(json!({
            "t": "pageview",
            "s": "site-1",
            "p": "/about",
            "ts": 1_700_000_000_000_i64,
            "d": "desktop",
            "v": 1280
        })))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::ACCEPTED);
    wait_for_count(&pool, 1).await;

    let row: (String, String, String) =
        sqlx::query_as("SELECT site_id, type, path FROM analytics_events LIMIT 1")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(row, ("site-1".into(), "pageview".into(), "/about".into()));
}

#[sqlx::test]
async fn collect_rejects_unknown_site_when_allowlisted(pool: PgPool) {
    let app = router(test_state(pool.clone(), None, Some(vec!["site-a".into()])));
    let resp = app
        .oneshot(post_collect(json!({
            "t": "pageview",
            "s": "site-b",
            "p": "/",
            "ts": 1_700_000_000_000_i64
        })))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    let n: i64 = sqlx::query_scalar("SELECT count(*) FROM analytics_events")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(n, 0);
}

#[sqlx::test]
async fn collect_rejects_oversize_path(pool: PgPool) {
    let app = router(test_state(pool, None, None));
    let resp = app
        .oneshot(post_collect(json!({
            "t": "pageview",
            "s": "s",
            "p": "/".repeat(3000),
            "ts": 1_700_000_000_000_i64
        })))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[sqlx::test]
async fn collect_rejects_oversize_body(pool: PgPool) {
    let app = router(test_state(pool, None, None));
    // Stuff a giant `pr` to exceed the 16KB body limit.
    let big = "x".repeat(20_000);
    let resp = app
        .oneshot(post_collect(json!({
            "t": "event",
            "s": "s",
            "p": "/",
            "ts": 1_700_000_000_000_i64,
            "n": "big",
            "pr": { "blob": big }
        })))
        .await
        .unwrap();
    assert!(
        resp.status() == StatusCode::PAYLOAD_TOO_LARGE || resp.status() == StatusCode::BAD_REQUEST,
        "got {}",
        resp.status()
    );
}

#[sqlx::test]
async fn stats_summary_requires_admin_token(pool: PgPool) {
    let app = router(test_state(pool, Some("secret-token"), None));
    let resp = app
        .oneshot(
            Request::get("/stats/summary?site=site-1&days=30")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[sqlx::test]
async fn stats_summary_rejects_wrong_token(pool: PgPool) {
    let app = router(test_state(pool, Some("secret-token"), None));
    let resp = app
        .oneshot(
            Request::get("/stats/summary?site=s&days=30")
                .header(header::AUTHORIZATION, "Bearer wrong")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[sqlx::test]
async fn stats_summary_accepts_correct_token(pool: PgPool) {
    let app = router(test_state(pool, Some("secret-token"), None));
    let resp = app
        .oneshot(
            Request::get("/stats/summary?site=s&days=30")
                .header(header::AUTHORIZATION, "Bearer secret-token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_json(resp).await;
    assert_eq!(body["pageviews"], 0);
    assert_eq!(body["events"], 0);
}

#[sqlx::test]
async fn stats_summary_counts_inserted_pageviews(pool: PgPool) {
    let state = test_state(pool.clone(), None, None);
    let app = router(state);

    let now_ms = chrono::Utc::now().timestamp_millis();
    for path in ["/a", "/a", "/b"] {
        let r = app
            .clone()
            .oneshot(post_collect(json!({
                "t": "pageview",
                "s": "site-1",
                "p": path,
                "ts": now_ms
            })))
            .await
            .unwrap();
        assert_eq!(r.status(), StatusCode::ACCEPTED);
    }
    wait_for_count(&pool, 3).await;

    let resp = app
        .oneshot(
            Request::get("/stats/summary?site=site-1&days=365")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_json(resp).await;
    assert_eq!(body["pageviews"], 3);
    assert_eq!(body["top_path"], "/a");
}

#[sqlx::test]
async fn security_headers_present(pool: PgPool) {
    let app = router(test_state(pool, None, None));
    let resp = app
        .oneshot(Request::get("/health").body(Body::empty()).unwrap())
        .await
        .unwrap();
    let h = resp.headers();
    assert_eq!(h.get("x-content-type-options").unwrap(), "nosniff");
    assert_eq!(h.get("referrer-policy").unwrap(), "no-referrer");
    assert_eq!(h.get("x-frame-options").unwrap(), "DENY");
    assert_eq!(
        h.get("content-security-policy").unwrap(),
        "default-src 'none'; frame-ancestors 'none'"
    );
    // HSTS is gated on BEHIND_TLS=1
    assert!(h.get("strict-transport-security").is_none());
    // Every response carries an x-request-id
    let rid = h.get("x-request-id").expect("x-request-id");
    assert!(!rid.is_empty());
}

#[sqlx::test]
async fn request_id_is_propagated_when_client_sends_one(pool: PgPool) {
    let app = router(test_state(pool, None, None));
    let resp = app
        .oneshot(
            Request::get("/health")
                .header("x-request-id", "abc-123-test")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.headers().get("x-request-id").unwrap(), "abc-123-test");
}

#[sqlx::test]
async fn metrics_endpoint_returns_prometheus_text(pool: PgPool) {
    // router_with_metrics installs a process-global Prometheus recorder, so
    // this is the only test that may exercise it. Other tests use the bare
    // router() to stay parallel-safe.
    let app = router_with_metrics(test_state(pool, None, None));

    // Generate a request so at least one counter exists.
    let _ = app
        .clone()
        .oneshot(Request::get("/health").body(Body::empty()).unwrap())
        .await
        .unwrap();

    let resp = app
        .oneshot(Request::get("/metrics").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let text = std::str::from_utf8(&body).unwrap();
    assert!(text.contains("axum_http_requests_total"), "body: {text}");
}

#[sqlx::test]
async fn collect_stores_utm_and_top_breaks_down(pool: PgPool) {
    let state = test_state(pool.clone(), None, None);
    let app = router(state);

    let now_ms = chrono::Utc::now().timestamp_millis();
    for src in ["newsletter", "newsletter", "twitter"] {
        let r = app
            .clone()
            .oneshot(post_collect(json!({
                "t": "pageview",
                "s": "site-1",
                "p": "/",
                "ts": now_ms,
                "u": { "s": src, "m": "email", "c": "spring" }
            })))
            .await
            .unwrap();
        assert_eq!(r.status(), StatusCode::ACCEPTED);
    }
    wait_for_count(&pool, 3).await;

    let resp = app
        .oneshot(
            Request::get("/stats/top?site=site-1&days=365&dim=utm_source")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_json(resp).await;
    assert_eq!(body[0]["key"], "newsletter");
    assert_eq!(body[0]["count"], 2);
    assert_eq!(body[1]["key"], "twitter");
}

#[sqlx::test]
async fn stats_events_lists_names_and_breaks_down_by_prop(pool: PgPool) {
    let app = router(test_state(pool.clone(), None, None));

    let now_ms = chrono::Utc::now().timestamp_millis();
    let events = [
        json!({"n": "scroll_depth", "pr": {"pct": 50}}),
        json!({"n": "scroll_depth", "pr": {"pct": 50}}),
        json!({"n": "scroll_depth", "pr": {"pct": 100}}),
        json!({"n": "outbound", "pr": {"href": "example.com"}}),
    ];
    for e in events {
        let r = app
            .clone()
            .oneshot(post_collect(json!({
                "t": "event",
                "s": "site-1",
                "p": "/",
                "ts": now_ms,
                "n": e["n"],
                "pr": e["pr"],
            })))
            .await
            .unwrap();
        assert_eq!(r.status(), StatusCode::ACCEPTED);
    }
    wait_for_count(&pool, 4).await;

    // No name → top event names.
    let resp = app
        .clone()
        .oneshot(
            Request::get("/stats/events?site=site-1&days=365")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_json(resp).await;
    assert_eq!(body[0]["key"], "scroll_depth");
    assert_eq!(body[0]["count"], 3);

    // name + by → distribution of that event's prop value.
    let resp = app
        .clone()
        .oneshot(
            Request::get("/stats/events?site=site-1&days=365&name=scroll_depth&by=pct")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_json(resp).await;
    assert_eq!(body[0]["key"], "50");
    assert_eq!(body[0]["count"], 2);
    assert_eq!(body[1]["key"], "100");
    assert_eq!(body[1]["count"], 1);

    // by without name is rejected.
    let resp = app
        .oneshot(
            Request::get("/stats/events?site=site-1&by=pct")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[sqlx::test]
async fn collect_round_trips_view_id(pool: PgPool) {
    let app = router(test_state(pool.clone(), None, None));
    let resp = app
        .oneshot(post_collect(json!({
            "t": "pageview",
            "s": "site-1",
            "p": "/",
            "ts": 1_700_000_000_000_i64,
            "vid": "abc123view"
        })))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::ACCEPTED);
    wait_for_count(&pool, 1).await;
    let vid: Option<String> =
        sqlx::query_scalar("SELECT view_id FROM analytics_events WHERE type = 'pageview' LIMIT 1")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(vid.as_deref(), Some("abc123view"));
}

#[sqlx::test]
async fn collect_rejects_oversize_vid(pool: PgPool) {
    let app = router(test_state(pool.clone(), None, None));
    let resp = app
        .oneshot(post_collect(json!({
            "t": "event",
            "s": "site-1",
            "p": "/",
            "ts": 1_700_000_000_000_i64,
            "n": "x",
            "vid": "v".repeat(100)
        })))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[sqlx::test]
async fn sessions_disabled_stores_no_visitor_data(pool: PgPool) {
    // Default config: even with IP + UA present, nothing is derived.
    let app = router(test_state(pool.clone(), None, None));
    let resp = app
        .oneshot(post_collect_ua(
            json!({"t": "pageview", "s": "site-1", "p": "/", "ts": 1_700_000_000_000_i64}),
            "203.0.113.5",
            CHROME_WIN,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::ACCEPTED);
    wait_for_count(&pool, 1).await;
    let (vh, br, os): (Option<String>, Option<String>, Option<String>) =
        sqlx::query_as("SELECT visitor_hash, browser, os FROM analytics_events LIMIT 1")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(vh, None);
    assert_eq!(br, None);
    assert_eq!(os, None);
}

#[sqlx::test]
async fn sessions_enabled_records_visitor_hash_and_ua(pool: PgPool) {
    let app = router(state_with(pool.clone(), None, None, true));
    let resp = app
        .oneshot(post_collect_ua(
            json!({"t": "pageview", "s": "site-1", "p": "/", "ts": 1_700_000_000_000_i64}),
            "203.0.113.5",
            CHROME_WIN,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::ACCEPTED);
    wait_for_count(&pool, 1).await;
    let (vh, br, os): (Option<String>, Option<String>, Option<String>) =
        sqlx::query_as("SELECT visitor_hash, browser, os FROM analytics_events LIMIT 1")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(vh.as_ref().map(|s| s.len()), Some(18));
    assert_eq!(br.as_deref(), Some("Chrome"));
    assert_eq!(os.as_deref(), Some("Windows"));
}

#[sqlx::test]
async fn summary_reports_unique_visitors_and_bounce_rate_when_enabled(pool: PgPool) {
    let app = router(state_with(pool.clone(), None, None, true));
    let now_ms = chrono::Utc::now().timestamp_millis();

    // Visitor A (one IP): two pageviews → not a bounce.
    for _ in 0..2 {
        app.clone()
            .oneshot(post_collect_ua(
                json!({"t": "pageview", "s": "s", "p": "/a", "ts": now_ms}),
                "1.1.1.1",
                CHROME_WIN,
            ))
            .await
            .unwrap();
    }
    // Visitor B (different IP): one pageview → a bounce.
    app.clone()
        .oneshot(post_collect_ua(
            json!({"t": "pageview", "s": "s", "p": "/b", "ts": now_ms}),
            "2.2.2.2",
            CHROME_WIN,
        ))
        .await
        .unwrap();
    wait_for_count(&pool, 3).await;

    let resp = app
        .oneshot(
            Request::get("/stats/summary?site=s&days=365")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body = body_json(resp).await;
    assert_eq!(body["uniqueVisitors"], 2);
    assert_eq!(body["bounceRate"], 0.5);
}

#[sqlx::test]
async fn summary_omits_session_metrics_when_disabled(pool: PgPool) {
    let app = router(test_state(pool.clone(), None, None));
    app.clone()
        .oneshot(post_collect(json!({
            "t": "pageview", "s": "s", "p": "/", "ts": chrono::Utc::now().timestamp_millis()
        })))
        .await
        .unwrap();
    wait_for_count(&pool, 1).await;

    let resp = app
        .oneshot(
            Request::get("/stats/summary?site=s&days=365")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body = body_json(resp).await;
    assert!(body.get("uniqueVisitors").is_none(), "got {body}");
    assert!(body.get("bounceRate").is_none(), "got {body}");
}

#[sqlx::test]
async fn top_breaks_down_by_browser_when_enabled(pool: PgPool) {
    let app = router(state_with(pool.clone(), None, None, true));
    let now_ms = chrono::Utc::now().timestamp_millis();
    app.clone()
        .oneshot(post_collect_ua(
            json!({"t": "pageview", "s": "s", "p": "/", "ts": now_ms}),
            "1.1.1.1",
            CHROME_WIN,
        ))
        .await
        .unwrap();
    wait_for_count(&pool, 1).await;

    let resp = app
        .oneshot(
            Request::get("/stats/top?site=s&days=365&dim=browser")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body = body_json(resp).await;
    assert_eq!(body[0]["key"], "Chrome");
    assert_eq!(body[0]["count"], 1);
}

#[sqlx::test]
async fn salt_rotates_daily_and_cleans_up_old(pool: PgPool) {
    use pagetally_server::salt::{current_salt, new_cache};
    let cache = new_cache();
    let d1 = chrono::NaiveDate::from_ymd_opt(2026, 1, 1).unwrap();
    let d2 = chrono::NaiveDate::from_ymd_opt(2026, 1, 2).unwrap();
    let d5 = chrono::NaiveDate::from_ymd_opt(2026, 1, 5).unwrap();

    let s1 = current_salt(&pool, &cache, d1).await.unwrap();
    let s1_again = current_salt(&pool, &cache, d1).await.unwrap();
    assert_eq!(s1, s1_again, "salt is stable within a day");

    let s2 = current_salt(&pool, &cache, d2).await.unwrap();
    assert_ne!(s1, s2, "salt rotates across days");

    // Advancing to d5 deletes salts older than d3 (d5 - 2), so d1 and d2 go.
    current_salt(&pool, &cache, d5).await.unwrap();
    let remaining: Vec<chrono::NaiveDate> =
        sqlx::query_scalar("SELECT day FROM daily_salts ORDER BY day")
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(remaining, vec![d5]);
}

#[sqlx::test]
async fn pageleave_dur_is_clamped(pool: PgPool) {
    let app = router(test_state(pool.clone(), None, None));
    let resp = app
        .oneshot(post_collect(json!({
            "t": "pageleave",
            "s": "site-1",
            "p": "/",
            "ts": 1_700_000_000_000_i64,
            "dur": 99_999_999_i32  // way over 30min cap
        })))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::ACCEPTED);
    wait_for_count(&pool, 1).await;
    let dur: i32 =
        sqlx::query_scalar("SELECT dur_ms FROM analytics_events WHERE type = 'pageleave' LIMIT 1")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(dur, 1_800_000);
}
