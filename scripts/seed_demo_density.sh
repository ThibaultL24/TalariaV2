#!/usr/bin/env bash
# scripts/seed_demo_density.sh — full densify pass: explorer (map) then agora (debates) for demo roster.
set -euo pipefail
API="${TALARIA_API:-http://127.0.0.1:8080}"
PER_PROVIDER="${CORPUS_LIMIT_PER_PROVIDER:-25}"
TOTAL="${CORPUS_LIMIT_TOTAL:-150}"
MAX_DOCS="${EXPLORER_MAX_DOCUMENTS:-400}"

figures=(
  "Napoleon|Q517"
  "Molière|Q687"
  "Marie Curie|Q7186"
  "Victor Hugo|Q535"
  "Joan of Arc|Q7226"
  "Louis XIV|Q7742"
  "Emmanuel Macron|Q3052772"
  "Donald Trump|Q22686"
  "Charles de Gaulle|Q2042"
  "Barack Obama|Q76"
)

mkdir -p /tmp/demo-density
: > /tmp/demo-density/explorer.tsv
: > /tmp/demo-density/agora.tsv

echo "=== EXPLORER (map/timeline) max_documents=$MAX_DOCS ==="
for row in "${figures[@]}"; do
  IFS='|' read -r subject qid <<<"$row"
  echo "→ explorer: $subject ($qid)"
  resp=$(curl -sS -m 60 -X POST "$API/api/v1/ingest/explorer" \
    -H 'Content-Type: application/json' \
    -d "{\"subject\":\"$subject\",\"qid\":\"$qid\",\"live\":true,\"wiki_lang\":\"en\",\"max_documents\":${MAX_DOCS}}") \
    || true
  echo "$resp" | tee "/tmp/demo-density/explorer-${qid}.json"
  job=$(python3 -c "import json,sys; print(json.load(sys.stdin).get('job_id',''))" <<<"$resp" 2>/dev/null || true)
  [[ -n "$job" ]] && echo -e "${qid}\t${subject}\t${job}" >> /tmp/demo-density/explorer.tsv
  sleep 1
done

echo
echo "=== AGORA (debates/theories) per_provider=$PER_PROVIDER total=$TOTAL ==="
for row in "${figures[@]}"; do
  IFS='|' read -r subject qid <<<"$row"
  echo "→ agora: $subject ($qid)"
  resp=$(curl -sS -m 60 -X POST "$API/api/v1/ingest/agora" \
    -H 'Content-Type: application/json' \
    -d "{\"subject\":\"$subject\",\"qid\":\"$qid\",\"live\":true,\"wiki_lang\":\"en\",\"corpus_limit_per_provider\":${PER_PROVIDER},\"corpus_limit_total\":${TOTAL}}") \
    || true
  echo "$resp" | tee "/tmp/demo-density/agora-${qid}.json"
  job=$(python3 -c "import json,sys; print(json.load(sys.stdin).get('job_id',''))" <<<"$resp" 2>/dev/null || true)
  [[ -n "$job" ]] && echo -e "${qid}\t${subject}\t${job}" >> /tmp/demo-density/agora.tsv
  sleep 2
done

echo
echo "Queued explorer=$(wc -l < /tmp/demo-density/explorer.tsv) agora=$(wc -l < /tmp/demo-density/agora.tsv)"
echo "Jobs listed in /tmp/demo-density/*.tsv"
