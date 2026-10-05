#!/usr/bin/env bash
# scripts/enrich_demo_explorer.sh — queue explorer person ingest via HTTP (async jobs).
# For synchronous full ingest on all demo figures, use ./scripts/enrich_demo_full.sh
set -euo pipefail
API="${TALARIA_API:-http://127.0.0.1:8080}"

# subject|qid|wiki_lang
rows=(
  "Napoleon Bonaparte|Q517|fr"
  "Molière|Q687|fr"
  "Marie Curie|Q7186|fr"
  "Victor Hugo|Q535|fr"
  "Joan of Arc|Q7226|fr"
  "Louis XIV|Q7742|fr"
  "Emmanuel Macron|Q3052772|fr"
  "Donald Trump|Q22686|en"
  "Charles de Gaulle|Q2042|fr"
  "Barack Obama|Q76|en"
)

echo "API=$API"
for row in "${rows[@]}"; do
  IFS='|' read -r subject qid lang <<<"$row"
  echo "→ explorer ingest: $subject ($qid)"
  curl -sS -m 120 -X POST "$API/api/v1/ingest/explorer" \
    -H 'Content-Type: application/json' \
    -d "{\"subject\":\"$subject\",\"qid\":\"$qid\",\"live\":true,\"wiki_lang\":\"$lang\",\"max_documents\":800}" \
    | tee "/tmp/demo-explorer-${qid}.json"
  echo
done

echo "Poll: curl $API/api/v1/ingest/explorer/<job_id>/status"
