-- 016_assertion_stems.sql
-- Shared historical question (place-stripped) for competing-place abstention.
-- Additive: quality vs legacy coexistence unchanged.

ALTER TABLE quality_claims
    ADD COLUMN IF NOT EXISTS occurrence_stem TEXT;

ALTER TABLE canonical_events
    ADD COLUMN IF NOT EXISTS occurrence_stem TEXT;

CREATE INDEX IF NOT EXISTS idx_quality_claims_occurrence_stem
    ON quality_claims (subject_entity_id, occurrence_stem)
    WHERE occurrence_stem IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_canonical_events_occurrence_stem
    ON canonical_events (entity_id, occurrence_stem)
    WHERE pipeline = 'quality' AND is_active AND occurrence_stem IS NOT NULL;
