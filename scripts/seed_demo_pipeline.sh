#!/usr/bin/env bash
# scripts/seed_demo_pipeline.sh — multi-profile dump → mine anecdotes → map events
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

set -a
# shellcheck disable=SC1091
[[ -f .env ]] && . ./.env
set +a

: "${DATABASE_URL:?DATABASE_URL required}"
: "${TALARIA_DATA_ROOT:=/home/ubuntu/wiki-dump}"
export TALARIA_DATA_ROOT

echo "==> generate multi-profile dump (fixtures + optional Wikipedia extracts)"
python3 scripts/seed_demo_dump.py

DUMP="$TALARIA_DATA_ROOT/dumps/enwiki-20250101-pages-articles-multistream.xml.bz2"

echo "==> migrate"
cargo build -q -p talaria-api
cargo run -q -p talaria-api -- migrate

echo "==> reset dump tables for clean density measurement"
reset_sql=$(cat <<'SQL'
TRUNCATE phrase_candidates, candidate_judgments, event_evidence, canonical_events,
         sentences, claims, soft_claims, raw_documents, entities, wiki_pages, dump_runs, place_geocodes
RESTART IDENTITY CASCADE;
SQL
)
if command -v psql >/dev/null 2>&1; then
  psql "$DATABASE_URL" -v ON_ERROR_STOP=1 -c "$reset_sql"
else
  sudo docker exec -i workspace-db-1 psql -U postgres -d talaria_engine_development -v ON_ERROR_STOP=1 -c "$reset_sql"
fi

echo "==> extract-pages"
cargo run -q -p talaria-api -- extract-pages --dump "$DUMP"

echo "==> split-sentences"
cargo run -q -p talaria-api -- split-sentences

echo "==> cosmos-extract --mock"
cargo run -q -p talaria-api -- cosmos-extract --mock

echo "==> dump-mine (anecdotes + extra keywords)"
cargo run -q -p talaria-api -- dump-mine

echo "==> judge-candidates"
cargo run -q -p talaria-api -- judge-candidates

echo "==> claims-extract"
cargo run -q -p talaria-api -- claims-extract

echo "==> seed profiles"
if command -v psql >/dev/null 2>&1; then
  bash scripts/seed-demo-profiles.sh
fi

echo "==> density"
report_sql=$(cat <<'SQL'
SELECT e.wikipedia_title AS person,
       COUNT(ce.*) AS facts,
       COUNT(ce.*) FILTER (WHERE ce.map_eligible AND ce.geom IS NOT NULL) AS map_points,
       COUNT(ce.*) FILTER (WHERE ce.event_type = 'anecdote') AS anecdotes
FROM entities e
LEFT JOIN canonical_events ce ON ce.entity_id = e.id
GROUP BY e.wikipedia_title
HAVING COUNT(ce.*) > 0
ORDER BY facts DESC;

SELECT claim_kind, COUNT(*) FROM soft_claims GROUP BY 1 ORDER BY 2 DESC;

SELECT extractor, COUNT(*) FROM phrase_candidates GROUP BY 1 ORDER BY 2 DESC;
SQL
)
if command -v psql >/dev/null 2>&1; then
  psql "$DATABASE_URL" -c "$report_sql"
else
  sudo docker exec -i workspace-db-1 psql -U postgres -d talaria_engine_development -c "$report_sql"
fi
