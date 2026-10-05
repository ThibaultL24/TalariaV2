-- Visit lens: commemorative POIs (museums, memorials) separate from biographical timeline.
ALTER TABLE canonical_events
    ADD COLUMN IF NOT EXISTS visit_eligible BOOLEAN NOT NULL DEFAULT false;

UPDATE canonical_events
SET
    visit_eligible = true,
    timeline_eligible = false
WHERE pipeline = 'person'
  AND is_active
  AND event_type IN ('museum', 'memorial', 'statue', 'street_naming');

CREATE INDEX IF NOT EXISTS idx_canonical_events_visit_entity
    ON canonical_events (entity_id)
    WHERE visit_eligible AND is_active AND pipeline = 'person';
