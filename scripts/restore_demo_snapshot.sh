#!/usr/bin/env bash
# scripts/restore_demo_snapshot.sh — load a demo pg_dump into DATABASE_URL.
# WARNING: replaces the target database contents (schema + data).
#
# Usage (Render):
#   DATABASE_URL='postgres://…@….render.com/talaria?sslmode=require' \
#     CONFIRM_RESTORE=1 ./scripts/restore_demo_snapshot.sh
#
# If DATABASE_URL is already set in the environment, .env will NOT override it.
set -euo pipefail
cd "$(dirname "$0")/.."

# Preserve an explicitly exported URL before loading .env (local default).
_PRESET_DATABASE_URL="${DATABASE_URL:-}"

set -a
# shellcheck disable=SC1091
[[ -f .env ]] && . ./.env
set +a

if [[ -n "$_PRESET_DATABASE_URL" ]]; then
  DATABASE_URL="$_PRESET_DATABASE_URL"
fi
unset _PRESET_DATABASE_URL

: "${DATABASE_URL:?DATABASE_URL required}"

DUMP="${1:-demo/snapshots/talaria-demo-latest.dump}"
if [[ ! -f "$DUMP" ]] && [[ "${DEMO_SNAPSHOT_DOWNLOAD:-}" == "1" ]] && [[ -n "${S3_BUCKET:-}" ]]; then
  echo "==> local dump missing; downloading from object storage"
  ./scripts/sync_object_storage.sh pull "$DUMP"
fi
if [[ ! -f "$DUMP" ]]; then
  echo "dump not found: $DUMP" >&2
  echo "Run ./scripts/export_demo_snapshot.sh first, or DEMO_SNAPSHOT_DOWNLOAD=1 with S3_* env, or pass a path." >&2
  exit 1
fi

# Redact password in logs
SAFE_URL="$(echo "$DATABASE_URL" | sed -E 's#://([^:/]+):([^@/]+)@#://\1:***@#')"

if [[ "${CONFIRM_RESTORE:-}" != "1" ]]; then
  echo "This will REPLACE the database at:"
  echo "  $SAFE_URL"
  echo "with:"
  echo "  $DUMP"
  echo "Re-run with CONFIRM_RESTORE=1 to proceed."
  exit 2
fi

case "$DATABASE_URL" in
  *localhost*|*127.0.0.1*|*@db:*)
    echo "==> NOTE: target looks LOCAL ($SAFE_URL)"
    echo "    For Render, pass DATABASE_URL=… on the same command line (do not rely on .env)."
    ;;
  *render.com*|*onrender.com*)
    echo "==> target looks like Render ($SAFE_URL)"
    ;;
esac

echo "==> restoring $DUMP → $SAFE_URL"

# Wipe public schema so dumps older/newer than live migrations don't fight FKs
# (e.g. intuition_term_bindings vs intuition_publications).
psql "$DATABASE_URL" -v ON_ERROR_STOP=1 <<'SQL'
DROP SCHEMA IF EXISTS public CASCADE;
CREATE SCHEMA public;
CREATE EXTENSION IF NOT EXISTS postgis;
CREATE EXTENSION IF NOT EXISTS pgcrypto;
GRANT ALL ON SCHEMA public TO PUBLIC;
SQL

if command -v pg_restore >/dev/null 2>&1; then
  pg_restore --no-owner --no-acl --dbname="$DATABASE_URL" "$DUMP" \
    || true
else
  sudo docker compose exec -T db pg_restore -U postgres -d talaria_engine_development \
    --no-owner --no-acl <"$DUMP" || true
fi

# If schema was applied outside sqlx (psql -f migrations/*.sql), _sqlx_migrations
# can be empty — then `talaria serve` re-runs CREATE TABLE and crash-loops (502).
mig_n="$(psql "$DATABASE_URL" -tAc "SELECT COUNT(*) FROM _sqlx_migrations" 2>/dev/null || echo 0)"
mig_n="${mig_n//[[:space:]]/}"
if [[ "${mig_n:-0}" -eq 0 ]] && [[ -f scripts/mark_sqlx_migrations.py ]]; then
  echo "==> _sqlx_migrations empty — marking embedded migrations as applied"
  python3 scripts/mark_sqlx_migrations.py | psql "$DATABASE_URL" -v ON_ERROR_STOP=1
fi

echo "==> post-restore counts"
psql "$DATABASE_URL" -c "
SELECT
  (SELECT COUNT(*) FROM entities) AS entities,
  (SELECT COUNT(*) FROM canonical_events WHERE is_active AND pipeline='person') AS person_events,
  (SELECT COUNT(*) FROM soft_claims) AS claims;
"

if [[ -f "${DUMP%.dump}.manifest.json" ]]; then
  echo "==> manifest"
  cat "${DUMP%.dump}.manifest.json"
elif [[ -f demo/snapshots/talaria-demo-latest.manifest.json ]]; then
  echo "==> latest manifest"
  cat demo/snapshots/talaria-demo-latest.manifest.json
fi

echo "==> restore complete"
echo "    If this was Render: hard-refresh https://talaria-7qgr.onrender.com ; API applies newer migrations on boot."
