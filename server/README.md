# dullahan

**The headless backend for your site** — a self-hosted, cookie-free Rust binary
that gives a small site three things over plain HTTP + Postgres: privacy-first
**analytics**, a headless **blog/content API**, and a **contact** endpoint. It
serves its own browser tracker at `/pt.js`, so there is no separate package to
install and `cargo install dullahan` needs no Node.

## Install

```bash
cargo install dullahan
DATABASE_URL=postgres://… ADMIN_TOKEN=$(openssl rand -hex 24) dullahan
```

Migrations apply automatically on startup. Without `ADMIN_TOKEN` the stats reads
and blog writes are open — set it on any public deploy.

Full quick start, endpoint reference, configuration, privacy notes, and a
one-shot VM installer live in the repository:
**<https://github.com/anfocic/dullahan>**

License: MIT.
