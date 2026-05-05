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

### 3. Read stats

All `/stats/*` endpoints require `Authorization: Bearer $ADMIN_TOKEN` when the server has `ADMIN_TOKEN` set.

```
GET /stats/summary?site=my-site&days=30
GET /stats/timeseries?site=my-site&days=30&bucket=day
GET /stats/top?site=my-site&dim=path&limit=10
GET /stats/vitals?site=my-site&days=30
```

`top?dim=path` returns `avgDurMs` per path. `summary` returns `avgTimeOnPageMs`.

## What gets collected

- Pageviews (path, referrer domain, device class, viewport bucket, country)
- Custom events (name + optional props)
- Web vitals (LCP, FCP, CLS, INP, TTFB)
- **Time on page** — visible duration only. The client never measures while the tab is hidden, and stops at 30 minutes per page.

No cookies, no fingerprinting, no IP storage. The browser client is ~5 KB.

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

## Security

If you find a vulnerability, please report it privately — see [`SECURITY.md`](SECURITY.md). Do not open a public issue.

## License

MIT
