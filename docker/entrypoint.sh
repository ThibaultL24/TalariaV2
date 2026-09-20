#!/bin/sh
# docker/entrypoint.sh — bind Render $PORT and start the API (migrations run in serve).
set -eu

PORT="${PORT:-8080}"
export TALARIA_BIND="${TALARIA_BIND:-0.0.0.0:${PORT}}"
export TALARIA_DATA_ROOT="${TALARIA_DATA_ROOT:-/data}"
mkdir -p "$TALARIA_DATA_ROOT/dumps" "$TALARIA_DATA_ROOT/pages" "$TALARIA_DATA_ROOT/wikidata" \
  "$TALARIA_DATA_ROOT/parquet" 2>/dev/null || true

if [ -z "${DATABASE_URL:-}" ]; then
  echo "DATABASE_URL is required" >&2
  exit 1
fi

echo "talaria serve bind=${TALARIA_BIND} data_root=${TALARIA_DATA_ROOT}"
exec /usr/local/bin/talaria serve
