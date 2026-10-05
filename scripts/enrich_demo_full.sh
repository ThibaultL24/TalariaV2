#!/usr/bin/env bash
# scripts/enrich_demo_full.sh — Visit (fixture + live) then explorer person ingest for demo roster.
set -euo pipefail
cd "$(dirname "$0")/.."

LOG="/tmp/enrich_demo_full.log"
TALARIA=(cargo run -q -p talaria-api --)

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

{
  echo "========== $(date -Iseconds) enrich_demo_full start =========="

  echo "==> Phase 1/2 — Visit (fixture + live discovery)"
  for row in "${rows[@]}"; do
    IFS='|' read -r subject qid lang <<<"$row"
    echo "--- visit $subject ($qid) lang=$lang"
    "${TALARIA[@]}" visit-enrich --fixture --live --subject "$subject" --qid "$qid" --wiki-lang "$lang"
    "${TALARIA[@]}" visit-audit --subject "$subject" --qid "$qid" --apply-dedupe >/dev/null
  done

  echo "==> Phase 2/2 — Explorer person ingest (pipeline=person, live Wikipedia/Wikidata)"
  for row in "${rows[@]}"; do
    IFS='|' read -r subject qid lang <<<"$row"
    echo "--- explorer $subject ($qid) lang=$lang"
    cmd=(
      "${TALARIA[@]}"
      explorer-ingest
      --subject "$subject"
      --qid "$qid"
      --wiki-lang "$lang"
      --max-documents 1500
    )
    case "$qid" in
      Q517) cmd+=(--seed-list fixtures/seeds/napoleon_wiki_titles.txt) ;;
      Q7186) cmd+=(--seed-list fixtures/seeds/marie_curie_wiki_titles.txt) ;;
    esac
    "${cmd[@]}" | tee "/tmp/demo-explorer-${qid}.json"
  done

  echo "==> $(date -Iseconds) enrich_demo_full done"
} 2>&1 | tee -a "$LOG"
