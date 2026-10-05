#!/usr/bin/env bash
# scripts/enrich_demo_visit.sh — Visit patrimoine + live discovery for all demo roster QIDs.
set -euo pipefail
cd "$(dirname "$0")/.."

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

echo "==> Visit enrich (fixture + live: Wikidata, Europeana, Serper, OpenAgenda)"
for row in "${rows[@]}"; do
  IFS='|' read -r subject qid lang <<<"$row"
  echo "--- $subject ($qid) lang=$lang"
  "${TALARIA[@]}" visit-enrich --fixture --live --subject "$subject" --qid "$qid" --wiki-lang "$lang"
  "${TALARIA[@]}" visit-audit --subject "$subject" --qid "$qid" --apply-dedupe >/dev/null
done

echo "==> Done. Refresh home roster and entity pages (?lens=visit)."
