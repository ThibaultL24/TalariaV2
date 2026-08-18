#!/usr/bin/env bash
# scripts/seed_napoleon_pipeline.sh — rebuild Napoleon demo corpus and measure density.
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

echo "==> generate demo dump (Napoleon + other bios + anecdotes)"
python3 scripts/seed_demo_dump.py

DUMP="$TALARIA_DATA_ROOT/dumps/enwiki-20250101-pages-articles-multistream.xml.bz2"

echo "==> migrate"
cargo build -q -p talaria-api
cargo run -q -p talaria-api -- migrate

echo "==> reset cultural tables for clean density measurement"
# Prefer docker exec psql when host psql is missing.
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
cargo run -q -p talaria-api -- extract-pages --dump "$DUMP" --skip-existing

echo "==> split-sentences"
cargo run -q -p talaria-api -- split-sentences

echo "==> cosmos-extract --mock (life_events)"
cargo run -q -p talaria-api -- cosmos-extract --mock

echo "==> dump-mine"
cargo run -q -p talaria-api -- dump-mine

echo "==> judge-candidates"
cargo run -q -p talaria-api -- judge-candidates

echo "==> claims-extract"
cargo run -q -p talaria-api -- claims-extract

echo "==> seed opinion claims (Intuition lane only — not map facts)"
claims_sql=$(cat <<'SQL'
INSERT INTO claims (subject_entity_id, predicate, value_json, epistemic_status, confidence, status, exportable)
SELECT e.id, 'community_theory',
       jsonb_build_object(
         'thesis', 'Napoleonic origins debates belong in the opinion layer',
         'note', 'Cultural biography stays in Talaria; avis go to Intuition'
       ),
       'theory', 0.55, 'draft', true
FROM entities e
WHERE e.wikipedia_title IN ('Napoleon', 'Napoleon Bonaparte')
ON CONFLICT DO NOTHING;

INSERT INTO raw_documents (source_type, source_uri, source_identifier, title, language, wiki_page_id, content_hash, license)
SELECT 'wikipedia_api',
       'https://en.wikipedia.org/wiki/' || replace(wp.title, ' ', '_'),
       wp.title,
       wp.title,
       wp.wiki_lang,
       wp.id,
       wp.content_hash,
       'CC-BY-SA-3.0'
FROM wiki_pages wp
ON CONFLICT (source_type, source_uri) DO NOTHING;
SQL
)
if command -v psql >/dev/null 2>&1; then
  psql "$DATABASE_URL" -v ON_ERROR_STOP=1 -c "$claims_sql"
else
  sudo docker exec -i workspace-db-1 psql -U postgres -d talaria_engine_development -v ON_ERROR_STOP=1 -c "$claims_sql"
fi

echo "==> density report"
report_sql=$(cat <<'SQL'
SELECT 'wiki_pages' AS metric, COUNT(*)::text AS value FROM wiki_pages
UNION ALL SELECT 'sentences', COUNT(*)::text FROM sentences
UNION ALL SELECT 'phrase_candidates', COUNT(*)::text FROM phrase_candidates
UNION ALL SELECT 'canonical_events', COUNT(*)::text FROM canonical_events
UNION ALL SELECT 'map_eligible_events', COUNT(*)::text FROM canonical_events WHERE map_eligible
UNION ALL SELECT 'claims_opinion_lane', COUNT(*)::text FROM claims
UNION ALL SELECT 'raw_documents', COUNT(*)::text FROM raw_documents
UNION ALL SELECT 'napoleon_events', COUNT(*)::text
  FROM canonical_events ce
  JOIN entities e ON e.id = ce.entity_id
  WHERE e.wikipedia_title ILIKE '%Napoleon%'
     OR e.canonical_name ILIKE '%Napoleon%';

SELECT e.wikipedia_title AS person,
       COUNT(ce.*) AS events,
       COUNT(ce.*) FILTER (WHERE ce.map_eligible) AS mappable,
       MIN(ce.start_time)::date AS first_year,
       MAX(ce.start_time)::date AS last_year
FROM entities e
LEFT JOIN canonical_events ce ON ce.entity_id = e.id
WHERE e.wikipedia_title ILIKE '%Napoleon%'
   OR e.canonical_name ILIKE '%Napoleon%'
GROUP BY e.wikipedia_title
ORDER BY events DESC;

SELECT ce.event_type, ce.title, ce.place_label, ce.start_time::date AS year, ce.map_eligible, ce.epistemic_status
FROM canonical_events ce
JOIN entities e ON e.id = ce.entity_id
WHERE e.wikipedia_title ILIKE '%Napoleon%'
   OR e.canonical_name ILIKE '%Napoleon%'
ORDER BY ce.start_time NULLS LAST, ce.title
LIMIT 40;
SQL
)
if command -v psql >/dev/null 2>&1; then
  psql "$DATABASE_URL" -c "$report_sql"
else
  sudo docker exec -i workspace-db-1 psql -U postgres -d talaria_engine_development -c "$report_sql"
fi

echo
echo "Try: curl 'http://localhost:8080/api/v1/timeline?person=Napoleon&limit=500'"
echo "     curl 'http://localhost:8080/api/v1/events/geojson?person=Napoleon&limit=500'"
