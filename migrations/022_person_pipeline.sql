-- migrations/022_person_pipeline.sql
-- Single person ingest: pipeline='person' facts + quote-only evidence.

ALTER TABLE canonical_events DROP CONSTRAINT IF EXISTS canonical_events_pipeline_check;
ALTER TABLE canonical_events
    ADD CONSTRAINT canonical_events_pipeline_check
    CHECK (pipeline IN ('legacy', 'quality', 'person'));

CREATE UNIQUE INDEX IF NOT EXISTS uq_canonical_events_active_person_occurrence
    ON canonical_events (occurrence_key)
    WHERE is_active AND occurrence_key IS NOT NULL AND pipeline = 'person';

ALTER TABLE event_evidence ALTER COLUMN sentence_id DROP NOT NULL;

ALTER TABLE event_evidence
    ADD COLUMN IF NOT EXISTS raw_document_id UUID REFERENCES raw_documents(id) ON DELETE SET NULL;
