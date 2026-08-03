-- 009_canonical_append_only.sql
-- Append-only canonical events: fingerprint, active flag, explicit supersession.
-- Legacy rows keep pipeline='legacy' and are NOT reinterpreted as quality-accepted.

ALTER TABLE canonical_events
    ADD COLUMN IF NOT EXISTS fingerprint TEXT,
    ADD COLUMN IF NOT EXISTS is_active BOOLEAN NOT NULL DEFAULT true,
    ADD COLUMN IF NOT EXISTS superseded_by UUID REFERENCES canonical_events(id) ON DELETE SET NULL,
    ADD COLUMN IF NOT EXISTS supersedes UUID REFERENCES canonical_events(id) ON DELETE SET NULL,
    ADD COLUMN IF NOT EXISTS place_entity_id UUID REFERENCES entities(id) ON DELETE SET NULL,
    ADD COLUMN IF NOT EXISTS predicate TEXT,
    ADD COLUMN IF NOT EXISTS assembler_version TEXT,
    ADD COLUMN IF NOT EXISTS pipeline TEXT NOT NULL DEFAULT 'legacy',
    ADD COLUMN IF NOT EXISTS event_candidate_id UUID;

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint WHERE conname = 'canonical_events_pipeline_check'
    ) THEN
        ALTER TABLE canonical_events
            ADD CONSTRAINT canonical_events_pipeline_check
            CHECK (pipeline IN ('legacy', 'quality'));
    END IF;
END $$;

-- Soft link from event_candidates (already has canonical_event_id FK).
-- event_candidate_id on canonical_events is soft (avoid circular FK enforcement).

CREATE UNIQUE INDEX IF NOT EXISTS uq_canonical_events_active_fingerprint
    ON canonical_events (fingerprint)
    WHERE is_active AND fingerprint IS NOT NULL AND pipeline = 'quality';

-- Max 1 active birth/death per subject on the quality pipeline only.
CREATE UNIQUE INDEX IF NOT EXISTS uq_canonical_active_singleton_birth_death
    ON canonical_events (entity_id, event_type)
    WHERE is_active
      AND pipeline = 'quality'
      AND event_type IN ('birth', 'death');

ALTER TABLE event_evidence
    ADD COLUMN IF NOT EXISTS event_candidate_id UUID REFERENCES event_candidates(id) ON DELETE SET NULL,
    ADD COLUMN IF NOT EXISTS fragment_id UUID REFERENCES document_fragments(id) ON DELETE SET NULL,
    ADD COLUMN IF NOT EXISTS clause_index INT;

CREATE INDEX IF NOT EXISTS idx_canonical_events_pipeline_active
    ON canonical_events (pipeline, is_active)
    WHERE is_active;
