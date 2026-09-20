#!/usr/bin/env bash
# scripts/demo_tunnel.sh — expose local Talaria API (+ static web) via Cloudflare Quick Tunnel.
# Requires: cloudflared (https://developers.cloudflare.com/cloudflare-one/connections/connect-apps/install-and-setup/installation/)
# Usage: ./scripts/demo_tunnel.sh
#        TALARIA_BIND_URL=http://127.0.0.1:8080 ./scripts/demo_tunnel.sh
set -euo pipefail
cd "$(dirname "$0")/.."

TARGET="${TALARIA_BIND_URL:-http://127.0.0.1:8080}"

if ! command -v cloudflared >/dev/null 2>&1; then
  echo "cloudflared not found." >&2
  echo "Install: https://developers.cloudflare.com/cloudflare-one/connections/connect-apps/install-and-setup/installation/" >&2
  echo "Or:      npm i -g cloudflared   /   brew install cloudflare/cloudflare/cloudflared" >&2
  exit 1
fi

if ! curl -sf -m 3 "${TARGET}/health" >/dev/null; then
  echo "API not healthy at ${TARGET}/health — start with: cargo run -p talaria-api -- serve" >&2
  exit 1
fi

echo "==> Cloudflare Quick Tunnel → ${TARGET}"
echo "    (no account needed; URL is ephemeral)"
exec cloudflared tunnel --url "$TARGET"
