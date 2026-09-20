#!/usr/bin/env bash
# scripts/seed_demo_roster.sh — kick explorer ingest for modern demo figures.
set -euo pipefail
API="${TALARIA_API:-http://127.0.0.1:8080}"

figures=(
  "Emmanuel Macron|Q3052772"
  "Donald Trump|Q22686"
  "Charles de Gaulle|Q2042"
  "Barack Obama|Q76"
)

echo "API=$API"
for row in "${figures[@]}"; do
  IFS='|' read -r subject qid <<<"$row"
  echo "→ ingest explorer: $subject ($qid)"
  curl -sS -m 30 -X POST "$API/api/v1/ingest/explorer" \
    -H 'Content-Type: application/json' \
    -d "{\"subject\":\"$subject\",\"qid\":\"$qid\",\"live\":true,\"wiki_lang\":\"en\"}" \
    | tee "/tmp/demo-ingest-${qid}.json"
  echo
done

echo "Done. Poll jobs via /api/v1/ingest/explorer/{job_id}/status — home roster refreshes counts automatically."
