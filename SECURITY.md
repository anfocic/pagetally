# Security policy

## Reporting a vulnerability

If you believe you've found a security issue in pagetally, please **do not** open a public GitHub issue.

Instead, use GitHub's private vulnerability reporting on this repository (Security → Report a vulnerability), or email the maintainer listed in `Cargo.toml` / `package.json`. Include:

- A description of the issue
- Steps to reproduce (or a proof of concept)
- The version / commit you tested against
- The impact you believe it has

You should expect an acknowledgement within a few days. Fixes for confirmed issues are released as soon as practical, with credit if you'd like it.

## Scope

In scope:

- The Rust server (`server/`) — ingest, stats, lead, auth middleware
- The browser client (`client/`) — anything that could leak user data, bypass DNT, or break the cookie-free guarantee
- The default deploy scripts (`deploy/`) when used as documented

Out of scope:

- Operator misconfiguration (e.g. running without `ADMIN_TOKEN` on the public internet — the server warns about this at startup)
- Issues in third-party services (Postgres, Caddy, Resend) unless triggered by an unsafe default in pagetally
- Vulnerabilities in old, unsupported versions

## Hardening notes for operators

- Always set `ADMIN_TOKEN` when exposing the server to the public internet. Without it, `/stats/*` is readable by anyone.
- Restrict `ALLOWED_SITES` to the site IDs you actually own; otherwise anyone can write events with any `siteId`.
- The `/collect` and `/lead` endpoints are intentionally unauthenticated — they accept input from browsers — but you should rate-limit them at your reverse proxy.
- Keep the server behind TLS (Caddy in `deploy/install.sh` does this automatically).
