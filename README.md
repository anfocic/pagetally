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
docker run -e DATABASE_URL=postgres://... -p 3001:3001 pagetally/server
```

Or build from source:

```bash
cd server
DATABASE_URL=postgres://... cargo run --release
```

Migrations run automatically on startup.

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

```
GET /stats/summary?site=my-site&days=30
GET /stats/timeseries?site=my-site&days=30&bucket=day
GET /stats/top?site=my-site&dim=path&limit=10
GET /stats/vitals?site=my-site&days=30
```

## Configuration

Server env vars:

| Var | Required | Default |
|---|---|---|
| `DATABASE_URL` | yes | — |
| `BIND_ADDR` | no | `0.0.0.0:3001` |
| `ALLOWED_SITES` | no | unrestricted |
| `RESEND_API_KEY` | no | (disables email) |
| `EMAIL_FROM` | no | — |
| `EMAIL_FROM_NAME` | no | `pagetally` |

## License

MIT
