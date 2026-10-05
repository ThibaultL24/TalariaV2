#!/usr/bin/env bash
# scripts/seed_visit_demo.sh — heritage POIs + visit opportunities for demo roster figures.
# Prefer ./scripts/enrich_demo_full.sh for fixture + live discovery AND explorer person ingest.
set -euo pipefail
cd "$(dirname "$0")/.."
TALARIA=(cargo run -p talaria-api -- visit-enrich --fixture)
pairs=(
  "Napoleon Bonaparte|Q517"
  "Marie Curie|Q7186"
  "Victor Hugo|Q535"
  "Joan of Arc|Q7226"
  "Louis XIV|Q7742"
  "Molière|Q687"
  "Charles de Gaulle|Q2042"
  "Emmanuel Macron|Q3052772"
  "Donald Trump|Q22686"
  "Barack Obama|Q76"
)
for pair in "${pairs[@]}"; do
  subject="${pair%%|*}"
  qid="${pair##*|}"
  echo "==> $subject ($qid)"
  "${TALARIA[@]}" --subject "$subject" --qid "$qid"
done
