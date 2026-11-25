-- 053_logical_events.sql
-- Stable identity of a historical fact. canonical_events keeps the versions.
-- occurrence_key stays a reading fingerprint and is not this identity.
-- Chapters, shared subjects, and coverage are later lots.

CREATE TABLE IF NOT EXISTS logical_events (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    origin_occurrence_key TEXT
);

ALTER TABLE canonical_events
    ADD COLUMN IF NOT EXISTS logical_event_id UUID REFERENCES logical_events(id) ON DELETE RESTRICT;

CREATE INDEX IF NOT EXISTS idx_canonical_events_logical_event
    ON canonical_events (logical_event_id)
    WHERE logical_event_id IS NOT NULL;

-- One active version of a fact for a person. Inactive versions may share the id.
CREATE UNIQUE INDEX IF NOT EXISTS uq_canonical_active_logical_event
    ON canonical_events (entity_id, logical_event_id)
    WHERE is_active AND logical_event_id IS NOT NULL;
