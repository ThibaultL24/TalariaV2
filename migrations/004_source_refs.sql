-- migrations/004_source_refs.sql
-- POC-parity: persist resolved citation refs on canonical events.

ALTER TABLE canonical_events
    ADD COLUMN IF NOT EXISTS source_refs JSONB NOT NULL DEFAULT '[]'::jsonb;

ALTER TABLE canonical_events
    ADD COLUMN IF NOT EXISTS source_page_titles JSONB NOT NULL DEFAULT '[]'::jsonb;

CREATE INDEX IF NOT EXISTS idx_canonical_events_source_refs
    ON canonical_events USING GIN (source_refs);
