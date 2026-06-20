# pagetally

GDPR-compliant, cookie-free web analytics. Browser client + self-hostable Rust server.

## Two parts

| Path | What | Install |
|---|---|---|
| `client/` | Browser client (TypeScript) | `npm i pagetally` |
| `server/` | Ingest + read API (Rust + Postgres) | `cargo install pagetally-server` |

## Quick start

### 1. Run the server

```bash
DATABASE_URL=postgres://... \
ADMIN_TOKEN=$(openssl rand -hex 24) \
cargo run --release
```

Migrations run automatically on startup. **Do not run without `ADMIN_TOKEN`** unless the host is on a trusted network — `/stats/*` is open by default and the server logs a warning.

For a one-shot install on a fresh Debian/Ubuntu VM, see [`deploy/install.sh`](deploy/install.sh).

### 2. Embed the client

```bash
npm i pagetally
```

```ts
import { Analytics } from "pagetally";

new Analytics({
  siteId: "my-site",
  endpoint: "https://analytics.example.com/collect",
  respectDNT: true,
});
```

Client options:

| Option | Default | What |
|---|---|---|
| `siteId` | — (required) | Site identifier sent with every event |
| `endpoint` | — (required) | URL of the server `/collect` endpoint |
| `autoTrack` | `true` | Auto-fire pageviews on load and SPA navigation |
| `respectDNT` | `false` | Send nothing when DNT / GPC is on |
| `trackScroll` | `false` | Emit `scroll_depth` events at 25/50/75/100% |
| `trackOutboundLinks` | `false` | Emit `outbound` / `download` events on link clicks |

UTM tags (`utm_source` / `utm_medium` / `utm_campaign`) on the landing URL are always captured and attached to the pageview — no flag needed.

### 3. Read stats

All `/stats/*` endpoints require `Authorization: Bearer $ADMIN_TOKEN` when the server has `ADMIN_TOKEN` set.

```
GET /stats/summary?site=my-site&days=30
GET /stats/timeseries?site=my-site&days=30&bucket=day
GET /stats/top?site=my-site&dim=path&limit=10
GET /stats/events?site=my-site&name=scroll_depth&by=pct
GET /stats/vitals?site=my-site&days=30
```

`top?dim=path` returns `avgDurMs` per path. `summary` returns `avgTimeOnPageMs`. With sessions enabled (see below), `summary` also returns `uniqueVisitors` and `bounceRate`.

> **Note on `uniqueVisitors`:** the visitor hash is salted with a salt that rotates every UTC day (and is then deleted), so the same person hashes differently each day. Over a multi-day range `uniqueVisitors` therefore counts *visitor-days*, not distinct people — a visitor active on N days counts as N. This is a deliberate consequence of the cookie-free, unlinkable-by-design model. For a per-day figure, query a 1-day range per day.

`top` dimensions: `path`, `referrer`, `country`, `device`, `utm_source`, `utm_medium`, `utm_campaign`, and (sessions only) `browser`, `os`.

`events` returns the top event names for a site; add `name=<event>&by=<prop>` to get the distribution of one event's prop value (e.g. scroll-depth milestones).

## What gets collected

- Pageviews (path, referrer domain, device class, viewport bucket, country, UTM tags)
- Custom events (name + optional props) — including opt-in scroll depth and outbound/download clicks
- Web vitals (LCP, FCP, CLS, INP, TTFB)
- **Time on page** — visible duration only. The client never measures while the tab is hidden, and stops at 30 minutes per page.

Optional, only when `SESSIONS_ENABLED=1` (off by default):

- Unique visitors, sessions, bounce rate
- Browser + OS *family* (e.g. "Chrome / macOS", never versions)

**Privacy:** no cookies, no fingerprinting, **no raw IP storage — ever.** With sessions **off** (the default) the server reads neither the client IP nor the User-Agent. With sessions **on**, the IP and User-Agent are combined with a daily-rotating salt into an anonymized hash and immediately discarded; the salt is deleted after 48h, making old hashes permanently unlinkable. The browser client is ~3 KB gzipped.

## Configuration

Server env vars:

| Var | Required | Default |
|---|---|---|
| `DATABASE_URL` | yes | — |
| `BIND_ADDR` | no | `0.0.0.0:3001` |
| `ADMIN_TOKEN` | recommended | unset (stats are public) |
| `ALLOWED_SITES` | no | unrestricted |
| `RESEND_API_KEY` | no | (disables email) |
| `EMAIL_FROM` | no | — |
| `EMAIL_FROM_NAME` | no | `pagetally` |
| `CONTACT_TO` | no | (disables `/contact`) |
| `STATS_ORIGINS` | no | `*` (any origin) |
| `BEHIND_TLS` | no | `false` (disables HSTS) |
| `SESSIONS_ENABLED` | no | `false` (no IP/UA processing; opt-in for unique visitors, sessions, bounce rate, browser/OS) |
| `LOG_FORMAT` | no | `text` (set `json` for structured logs) |
| `RUST_LOG` | no | `info,sqlx=warn` |

## Operator hardening (self-host checklist)

The defaults are safe for a private deploy. For a public-internet host:

- **Set `ADMIN_TOKEN`.** Without it `/stats/*` is open. The server logs a warning at startup if unset.
- **Set `ALLOWED_SITES`** if you only collect for known sites — otherwise any caller can write any `siteId` and bloat your DB.
- **Set `STATS_ORIGINS`** to your dashboard origin so a browser elsewhere can't read `/stats/*` responses even if the admin token leaks.
- **Set `BEHIND_TLS=1`** once the deploy is fronted by HTTPS so the server emits `Strict-Transport-Security`. The other security headers (`X-Content-Type-Options`, `Referrer-Policy`, `X-Frame-Options`) ship unconditionally.
- **Rate limiting** is built in (per-IP, in-process): `/collect` allows ~120/min burst 60, `/contact` allows ~5/min burst 3. The server reads the client IP from `x-forwarded-for` / `x-real-ip` (with the TCP peer as fallback), so make sure your reverse proxy sets one of those. For a hostile public deploy, layer additional limits at Caddy/nginx.
- **Strip the `x-country` header at the proxy** before re-injecting it from a GeoIP lookup — the server trusts whatever the client sends if no proxy strips it.
- **Watch your access logs.** The `/collect` body never stores IPs, but your reverse proxy and `tower-http` request traces likely log the client IP. Configure log retention / redaction to match your privacy posture.

## Privacy notes (for SDK consumers)

The library doesn't fingerprint or store IPs, but two channels can still leak PII if you're not careful:

- **URL paths.** `pagetally` strips `?query` and `#hash` but not path segments. A path like `/users/jane@example.com/orders/42` will be stored verbatim. Strip or hash sensitive segments client-side before navigating, or pass a sanitized path to `analytics.page(path)`.
- **Custom event props.** `analytics.track(name, props)` stores `props` as-is. Don't pass emails, names, or tokens. Use a stable `userId` hash if you need correlation.

## Metrics

`GET /metrics` exposes Prometheus-format metrics for HTTP traffic (request rate, latency histograms, status codes per route). Scrape it with Prometheus / Grafana Agent / Vector.

The endpoint is **unauthenticated** — keep it on an internal interface or block external access at your reverse proxy. Standard practice for `/metrics` everywhere; pagetally follows the convention.

```
# HELP axum_http_requests_total Total HTTP requests.
# TYPE axum_http_requests_total counter
axum_http_requests_total{method="GET",path="/health",status="200"} 1
...
```

## Load testing

A small wrapper around [`oha`](https://github.com/hatoo/oha) lives at [`scripts/loadtest.sh`](scripts/loadtest.sh):

```bash
brew install oha
BASE=http://127.0.0.1:3001 ./scripts/loadtest.sh collect-burst    # single-IP abuse
BASE=http://127.0.0.1:3001 ./scripts/loadtest.sh collect-spread   # parallel IPs
BASE=http://127.0.0.1:3001 ./scripts/loadtest.sh stats-read       # read path
```

Reference numbers from a release build on an M-class laptop, single Postgres on the same box:

| Scenario | Throughput | p99 | Notes |
|---|---|---|---|
| `/collect` from one IP | ~71k rps | <3 ms | Rate-limit returns 429 after the burst is exhausted, server stays responsive |
| `/stats/summary` reads | ~20k rps | ~5 ms | Hits Postgres on every request |

Treat these as smoke-test floors, not throughput guarantees — production numbers depend on disk, Postgres tuning, and the size of the `analytics_events` table.

## Security

If you find a vulnerability, please report it privately — see [`SECURITY.md`](SECURITY.md). Do not open a public issue.

## License

MIT
