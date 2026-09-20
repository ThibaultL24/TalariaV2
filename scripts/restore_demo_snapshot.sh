#!/usr/bin/env bash
# scripts/restore_demo_snapshot.sh — load a demo pg_dump into DATABASE_URL.
# WARNING: replaces the target database contents (schema + data).
set -euo pipefail
cd "$(dirname "$0")/.."
set -a
# shellcheck disable=SC1091
[[ -f .env ]] && . ./.env
set +a

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

if [[ "${CONFIRM_RESTORE:-}" != "1" ]]; then
  echo "This will REPLACE the database at:"
  echo "  $DATABASE_URL"
  echo "with:"
  echo "  $DUMP"
  echo "Re-run with CONFIRM_RESTORE=1 to proceed."
  exit 2
fi

echo "==> restoring $DUMP → $DATABASE_URL"
if command -v pg_restore >/dev/null 2>&1; then
  # Drop+recreate public schema objects via --clean; ignore "does not exist" noise.
  pg_restore --clean --if-exists --no-owner --no-acl --dbname="$DATABASE_URL" "$DUMP" \
    || true
else
  # Stream into the compose db container.
  # Parse db name from URL roughly.
  sudo docker compose exec -T db dropdb -U postgres --if-exists talaria_engine_development
  sudo docker compose exec -T db createdb -U postgres talaria_engine_development
  sudo docker compose exec -T db pg_restore -U postgres -d talaria_engine_development \
    --no-owner --no-acl <"$DUMP" || true
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
