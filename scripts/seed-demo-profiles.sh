#!/usr/bin/env bash
# scripts/seed-demo-profiles.sh — assign universal demo profiles to local entities
set -o errexit -o pipefail
export TZ='Asia/Jakarta'
cd "$(dirname "$0")/.."
# shellcheck disable=SC1091
source .env

psql "$DATABASE_URL" -v ON_ERROR_STOP=1 <<'SQL'
-- Ensure century/era rows exist (API also seeds on serve)
INSERT INTO periods (slug, label, start_year, end_year, kind) VALUES
  ('15th-century', '15th century', 1401, 1500, 'century'),
  ('16th-century', '16th century', 1501, 1600, 'century'),
  ('17th-century', '17th century', 1601, 1700, 'century'),
  ('18th-century', '18th century', 1701, 1800, 'century'),
  ('19th-century', '19th century', 1801, 1900, 'century'),
  ('20th-century', '20th century', 1901, 2000, 'century'),
  ('21st-century', '21st century', 2001, 2100, 'century'),
  ('antiquity', 'Antiquity', -800, 500, 'era'),
  ('medieval', 'Medieval period', 500, 1500, 'era'),
  ('early-modern', 'Early modern period', 1500, 1800, 'era'),
  ('contemporary', 'Contemporary period', 1900, 2100, 'era')
ON CONFLICT (slug) DO NOTHING;

WITH mapping(title_pattern, slug, label, kind) AS (
  VALUES
    ('Napoleon%', 'military-leader', 'military leader', 'occupation'),
    ('Napoleon%', 'head-of-state', 'head of state', 'position'),
    ('Napoleon%', 'emperor', 'emperor', 'position'),
    ('Isaac Newton%', 'scientist', 'scientist', 'occupation'),
    ('Isaac Newton%', 'physicist', 'physicist', 'occupation'),
    ('Marie Curie%', 'scientist', 'scientist', 'occupation'),
    ('Marie Curie%', 'physicist', 'physicist', 'occupation'),
    ('Marie Curie%', 'chemist', 'chemist', 'occupation'),
    ('Alan Turing%', 'scientist', 'scientist', 'occupation'),
    ('Alan Turing%', 'computer-scientist', 'computer scientist', 'occupation'),
    ('Victor Hugo%', 'writer', 'writer', 'occupation'),
    ('Victor Hugo%', 'politician', 'politician', 'occupation'),
    ('Leonardo%', 'artist', 'artist', 'occupation'),
    ('Leonardo%', 'engineer', 'engineer', 'occupation'),
    ('Christopher Columbus%', 'explorer', 'explorer', 'occupation'),
    ('Columbus%', 'explorer', 'explorer', 'occupation'),
    ('Cleopatra%', 'head-of-state', 'head of state', 'position'),
    ('Cleopatra%', 'ruler', 'ruler', 'position')
)
INSERT INTO entity_profiles (entity_id, profile_slug, profile_label, kind, confidence, source_system)
SELECT e.id, m.slug, m.label, m.kind, 0.85, 'seed'
FROM entities e
JOIN mapping m ON e.wikipedia_title ILIKE m.title_pattern
ON CONFLICT (entity_id, profile_slug, kind) DO UPDATE
SET profile_label = EXCLUDED.profile_label;

-- Link entities to centuries from their earliest event year
INSERT INTO entity_periods (entity_id, period_id)
SELECT DISTINCT e.id, p.id
FROM entities e
JOIN canonical_events ce ON ce.entity_id = e.id
JOIN periods p ON p.kind = 'century'
  AND EXTRACT(YEAR FROM ce.start_time)::int BETWEEN p.start_year AND p.end_year
WHERE ce.start_time IS NOT NULL
ON CONFLICT DO NOTHING;

INSERT INTO entity_periods (entity_id, period_id)
SELECT DISTINCT e.id, p.id
FROM entities e
JOIN canonical_events ce ON ce.entity_id = e.id
JOIN periods p ON p.kind = 'era'
  AND EXTRACT(YEAR FROM ce.start_time)::int BETWEEN p.start_year AND p.end_year
WHERE ce.start_time IS NOT NULL
ON CONFLICT DO NOTHING;
SQL

echo "seeded profiles/periods"
psql "$DATABASE_URL" -c "SELECT profile_slug, count(*) FROM entity_profiles GROUP BY 1 ORDER BY 2 DESC;"
