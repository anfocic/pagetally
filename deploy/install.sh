#!/usr/bin/env bash
# One-shot installer for pagetally on a fresh Debian/Ubuntu box.
# Run AS ROOT. Requires: a domain pointing at this box, ports 80/443 open.
#
# Usage:
#   sudo DOMAIN=analytics.example.com ACME_EMAIL=you@example.com \
#        PG_PASSWORD=$(openssl rand -hex 24) \
#        ./install.sh
#
# What it does:
#   1. apt install postgresql, caddy, build deps
#   2. create OS user `pagetally`, dir /opt/pagetally
#   3. create PG role + DB
#   4. install Rust toolchain (rustup, user-local)
#   5. build pagetally-server from ../server (release)
#   6. drop binary into /opt/pagetally + write env file
#   7. install systemd unit + Caddyfile, enable + start
#
# Re-running is safe: each step checks for existing state.

set -euo pipefail

if [[ $EUID -ne 0 ]]; then
    echo "must run as root" >&2; exit 1
fi
: "${DOMAIN:?DOMAIN env var required}"
: "${ACME_EMAIL:?ACME_EMAIL env var required}"
: "${PG_PASSWORD:?PG_PASSWORD env var required}"

ALLOWED_SITES="${ALLOWED_SITES:-}"
ADMIN_TOKEN="${ADMIN_TOKEN:-}"
PG_DB="${PG_DB:-pagetally}"
PG_USER="${PG_USER:-pagetally}"

ENV_FILE="/opt/pagetally/pagetally.env"

# Re-runs reuse the existing token. Only generate on first install.
if [[ -z "$ADMIN_TOKEN" ]]; then
    if [[ -f "$ENV_FILE" ]] && grep -q '^ADMIN_TOKEN=' "$ENV_FILE"; then
        ADMIN_TOKEN="$(grep '^ADMIN_TOKEN=' "$ENV_FILE" | cut -d= -f2-)"
    else
        ADMIN_TOKEN="$(openssl rand -hex 24)"
        ADMIN_TOKEN_GENERATED=1
    fi
fi

REPO_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SERVER_DIR="$REPO_DIR/server"

echo "==> apt packages"
apt-get update -qq
apt-get install -y --no-install-recommends \
    postgresql ca-certificates curl debian-keyring debian-archive-keyring \
    apt-transport-https build-essential pkg-config libssl-dev git gettext-base

if ! command -v caddy >/dev/null 2>&1; then
    echo "==> installing caddy"
    curl -1sLf 'https://dl.cloudsmith.io/public/caddy/stable/gpg.key' | gpg --dearmor -o /usr/share/keyrings/caddy-stable-archive-keyring.gpg
    curl -1sLf 'https://dl.cloudsmith.io/public/caddy/stable/debian.deb.txt' > /etc/apt/sources.list.d/caddy-stable.list
    apt-get update -qq
    apt-get install -y caddy
fi

echo "==> os user + dirs"
id pagetally >/dev/null 2>&1 || useradd --system --create-home --home /opt/pagetally --shell /usr/sbin/nologin pagetally
install -d -o pagetally -g pagetally -m 750 /opt/pagetally

echo "==> postgres role + db"
sudo -u postgres psql -tAc "SELECT 1 FROM pg_roles WHERE rolname='${PG_USER}'" | grep -q 1 \
    || sudo -u postgres psql -c "CREATE ROLE ${PG_USER} LOGIN PASSWORD '${PG_PASSWORD}'"
sudo -u postgres psql -tAc "SELECT 1 FROM pg_database WHERE datname='${PG_DB}'" | grep -q 1 \
    || sudo -u postgres createdb -O "${PG_USER}" "${PG_DB}"

echo "==> rust toolchain (user-local for pagetally)"
if ! sudo -u pagetally test -x /opt/pagetally/.cargo/bin/cargo; then
    sudo -u pagetally bash -c 'curl --proto "=https" --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable --profile minimal'
fi

echo "==> building release binary (this takes a few minutes)"
install -d -o pagetally -g pagetally /opt/pagetally/build-src
cp -r "$SERVER_DIR/Cargo.toml" "$SERVER_DIR/src" "$SERVER_DIR/migrations" /opt/pagetally/build-src/
chown -R pagetally:pagetally /opt/pagetally/build-src
sudo -u pagetally bash -c 'cd /opt/pagetally/build-src && /opt/pagetally/.cargo/bin/cargo build --release --bin pagetally-server'
install -o pagetally -g pagetally -m 755 /opt/pagetally/build-src/target/release/pagetally-server /opt/pagetally/pagetally-server
rm -rf /opt/pagetally/build-src/target

echo "==> migrations dir (sqlx reads from CWD/migrations on boot)"
rm -rf /opt/pagetally/migrations
cp -r "$SERVER_DIR/migrations" /opt/pagetally/migrations
chown -R pagetally:pagetally /opt/pagetally/migrations

echo "==> env file"
PG_PORT=$(pg_lsclusters --no-header | awk '$4=="online"{print $3; exit}')
PG_PORT="${PG_PORT:-5432}"
if [[ ! -f "$ENV_FILE" ]]; then
    cat > "$ENV_FILE" <<EOF
DATABASE_URL=postgres://${PG_USER}:${PG_PASSWORD}@127.0.0.1:${PG_PORT}/${PG_DB}
BIND_ADDR=127.0.0.1:3011
ALLOWED_SITES=${ALLOWED_SITES}
ADMIN_TOKEN=${ADMIN_TOKEN}
RUST_LOG=info,sqlx=warn
EOF
    chown pagetally:pagetally "$ENV_FILE"
    chmod 600 "$ENV_FILE"
fi

echo "==> systemd unit"
install -m 644 "$REPO_DIR/deploy/pagetally.service" /etc/systemd/system/pagetally.service
systemctl daemon-reload
systemctl enable --now pagetally

echo "==> caddyfile"
mkdir -p /etc/caddy
DOMAIN="$DOMAIN" ACME_EMAIL="$ACME_EMAIL" envsubst < "$REPO_DIR/deploy/Caddyfile" > /etc/caddy/Caddyfile.tmp
mv /etc/caddy/Caddyfile.tmp /etc/caddy/Caddyfile
systemctl reload caddy 2>/dev/null || systemctl restart caddy

echo
echo "=========================================="
echo "  pagetally is up at https://${DOMAIN}"
echo "=========================================="
echo "  health:    curl https://${DOMAIN}/health"
echo "  logs:      journalctl -u pagetally -f"
echo "  redeploy:  re-run install.sh (rebuilds binary)"
echo "=========================================="
if [[ "${ADMIN_TOKEN_GENERATED:-0}" == "1" ]]; then
    echo
    echo "Generated ADMIN_TOKEN — save it now, it gates /stats/*:"
    echo "    $ADMIN_TOKEN"
    echo "(stored in $ENV_FILE; re-runs of install.sh will reuse it.)"
fi
