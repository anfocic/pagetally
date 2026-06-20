#!/usr/bin/env bash
# Builds the client (so /pt.js embeds the real bundle) and runs the server for
# the E2E. Used as the Playwright webServer command; also runnable standalone.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

npm --prefix "$ROOT/client" run build >&2

exec env \
  DATABASE_URL="${DATABASE_URL:-postgres://fole@localhost/pagetally_e2e}" \
  ADMIN_TOKEN="${ADMIN_TOKEN:-e2e-token}" \
  BIND_ADDR="${BIND_ADDR:-127.0.0.1:3099}" \
  PAGETALLY_SCRIPT="$ROOT/client/dist/pt.js" \
  cargo run --manifest-path "$ROOT/server/Cargo.toml"
