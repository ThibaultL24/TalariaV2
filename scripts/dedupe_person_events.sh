#!/usr/bin/env bash
# scripts/dedupe_person_events.sh — deactivate exact title duplicates (pipeline=person).
# Keeps the best row per (entity_id, lower(title)): map pin > evidence > oldest.
set -euo pipefail
cd "$(dirname "$0")/.."
set -a
# shellcheck disable=SC1091
[[ -f .env ]] && . ./.env
set +a
: "${DATABASE_URL:?DATABASE_URL required}"

BEFORE=$(psql "$DATABASE_URL" -Atc "SELECT COUNT(*) FROM canonical_events WHERE is_active AND pipeline='person'")

psql "$DATABASE_URL" -v ON_ERROR_STOP=1 <<'SQL'
WITH ranked AS (
  SELECT
    id,
    ROW_NUMBER() OVER (
      PARTITION BY entity_id, lower(trim(title))
      ORDER BY
        (geom IS NOT NULL) DESC,
        map_eligible DESC,
        COALESCE(evidence_count, 0) DESC,
        (place_identity_qid IS NOT NULL) DESC,
        created_at ASC,
        id ASC
    ) AS rn,
    FIRST_VALUE(id) OVER (
      PARTITION BY entity_id, lower(trim(title))
      ORDER BY
        (geom IS NOT NULL) DESC,
        map_eligible DESC,
        COALESCE(evidence_count, 0) DESC,
        (place_identity_qid IS NOT NULL) DESC,
        created_at ASC,
        id ASC
    ) AS keep_id
  FROM canonical_events
  WHERE is_active AND pipeline = 'person'
)
UPDATE canonical_events ce
SET
  is_active = false,
  superseded_by = ranked.keep_id
FROM ranked
WHERE ce.id = ranked.id
  AND ranked.rn > 1
  AND ce.is_active;
SQL

AFTER=$(psql "$DATABASE_URL" -Atc "SELECT COUNT(*) FROM canonical_events WHERE is_active AND pipeline='person'")
DEACTIVATED=$((BEFORE - AFTER))

echo "active person events: $BEFORE → $AFTER (deactivated $DEACTIVATED)"

psql "$DATABASE_URL" -c "
SELECT e.canonical_name,
  COUNT(*) FILTER (WHERE ce.is_active) AS active,
  COUNT(*) FILTER (WHERE NOT ce.is_active AND ce.superseded_by IS NOT NULL) AS superseded_dups
FROM entities e
JOIN canonical_events ce ON ce.entity_id = e.id AND ce.pipeline='person'
WHERE e.qid IN ('Q517','Q687','Q7186','Q535','Q7226','Q7742','Q3052772','Q22686','Q2042','Q76')
GROUP BY e.canonical_name
ORDER BY superseded_dups DESC;
"

REMAINING=$(psql "$DATABASE_URL" -Atc "
SELECT COUNT(*) FROM (
  SELECT entity_id, lower(trim(title))
  FROM canonical_events
  WHERE is_active AND pipeline='person'
  GROUP BY 1,2 HAVING COUNT(*)>1
) s;
")
echo "remaining exact-title duplicate groups: $REMAINING"
