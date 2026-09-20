#!/usr/bin/env bash
# scripts/bootstrap_demo.sh — local / VPS: DB + restore snapshot + build web + serve API.
# Beta ingest: leave OPENAI_API_KEY unset — person ingest uses structured Wikipedia/Wikidata only.
set -euo pipefail
cd "$(dirname "$0")/.."

DUMP="${1:-demo/snapshots/talaria-demo-latest.dump}"

echo "==> starting Postgres (docker compose)"
sudo service docker start 2>/dev/null || true
sudo docker compose up -d

echo "==> waiting for Postgres on 5433"
for i in $(seq 1 60); do
  if sudo docker compose exec -T db pg_isready -U postgres >/dev/null 2>&1; then
    break
  fi
  sleep 1
done

if [[ ! -f .env ]]; then
  cp .env.example .env
  echo "created .env from .env.example — edit DATABASE_URL if needed"
fi

set -a
# shellcheck disable=SC1091
. ./.env
set +a

if [[ -f "$DUMP" ]]; then
  echo "==> restoring demo snapshot"
  CONFIRM_RESTORE=1 ./scripts/restore_demo_snapshot.sh "$DUMP"
else
  echo "WARN: no snapshot at $DUMP — API will start empty; run export on a filled machine first."
fi

echo "==> building web explorer"
(cd web && npm ci && npm run build)

echo "==> building API"
cargo build -p talaria-api

echo "==> starting API on ${TALARIA_BIND:-0.0.0.0:8080} (serves web/dist)"
echo "    Demo roster loads from DB immediately."
echo "    Beta ingest: search a new person — works without OPENAI_API_KEY (structured sources only)."
exec ./target/debug/talaria serve
