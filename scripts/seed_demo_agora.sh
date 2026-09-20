#!/usr/bin/env bash
# scripts/seed_demo_agora.sh — densify theories/debates for the demo roster via Agora ingest.
set -euo pipefail
API="${TALARIA_API:-http://127.0.0.1:8080}"
# Keep catalog pulls bounded so the run finishes (Europeana can explode otherwise).
PER_PROVIDER="${CORPUS_LIMIT_PER_PROVIDER:-12}"
TOTAL="${CORPUS_LIMIT_TOTAL:-72}"

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

echo "API=$API per_provider=$PER_PROVIDER total=$TOTAL"
mkdir -p /tmp/demo-agora
: > /tmp/demo-agora/jobs.tsv

for row in "${figures[@]}"; do
  IFS='|' read -r subject qid <<<"$row"
  echo "→ agora ingest: $subject ($qid)"
  resp=$(curl -sS -m 60 -X POST "$API/api/v1/ingest/agora" \
    -H 'Content-Type: application/json' \
    -d "{\"subject\":\"$subject\",\"qid\":\"$qid\",\"live\":true,\"wiki_lang\":\"en\",\"corpus_limit_per_provider\":${PER_PROVIDER},\"corpus_limit_total\":${TOTAL}}") \
    || true
  echo "$resp" | tee "/tmp/demo-agora/${qid}.json"
  job=$(python3 -c "import json,sys; print(json.load(sys.stdin).get('job_id',''))" <<<"$resp" 2>/dev/null || true)
  if [[ -n "$job" ]]; then
    echo -e "${qid}\t${subject}\t${job}" >> /tmp/demo-agora/jobs.tsv
  fi
  echo
  # Small pause so the API does not stampede all catalog providers at once.
  sleep 2
done

echo "Queued $(wc -l < /tmp/demo-agora/jobs.tsv) agora jobs."
echo "Poll: while read q s j; do curl -s $API/api/v1/ingest/\$j; echo; done < /tmp/demo-agora/jobs.tsv"
echo "Claims check later via GET /api/v1/demo/roster"
