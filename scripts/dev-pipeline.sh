#!/usr/bin/env bash
# scripts/dev-pipeline.sh — mock pipeline on a small Wikipedia extract
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

LIMIT="${1:-100}"

echo "==> migrate"
cargo run -p talaria-api -- migrate

if [[ -n "${DUMP:-}" ]]; then
  echo "==> extract-pages (limit=$LIMIT)"
  cargo run -p talaria-api -- extract-pages --dump "$DUMP" --limit "$LIMIT" --skip-existing
else
  echo "==> skip extract-pages (set DUMP=/path/to/pages-articles-multistream.xml.bz2)"
fi

echo "==> split-sentences"
cargo run -p talaria-api -- split-sentences --skip-existing

echo "==> cosmos-extract (mock)"
cargo run -p talaria-api -- cosmos-extract --mock --skip-existing

echo "==> judge-candidates"
cargo run -p talaria-api -- judge-candidates

echo "==> geocode-places"
cargo run -p talaria-api -- geocode-places

echo "==> status"
curl -s "http://localhost:8080/api/v1/status" || true
echo
echo "Done. Try: curl 'http://localhost:8080/api/v1/timeline?person=Alan%20Turing'"
