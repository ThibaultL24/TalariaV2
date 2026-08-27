-- migrations/027_unified_person_pipeline.sql
-- Structure only. Purge is talaria admin rebuild-person-pipeline.

-- Pipeline check: NOT VALID so existing legacy/quality/person rows do not fail migrate.
-- Rebuild command will VALIDATE after purge.
ALTER TABLE canonical_events DROP CONSTRAINT IF EXISTS canonical_events_pipeline_check;
ALTER TABLE canonical_events
    ADD CONSTRAINT canonical_events_pipeline_check
    CHECK (pipeline = 'person') NOT VALID;
ALTER TABLE canonical_events ALTER COLUMN pipeline SET DEFAULT 'person';

-- Recreate quality/person partial indexes on pipeline = 'person'.
DROP INDEX IF EXISTS idx_canonical_events_map_eligible_quality;
CREATE INDEX IF NOT EXISTS idx_canonical_events_map_eligible_quality
    ON canonical_events (entity_id)
    WHERE is_active AND pipeline = 'person' AND map_eligible;

DROP INDEX IF EXISTS idx_canonical_events_occurrence_stem;
CREATE INDEX IF NOT EXISTS idx_canonical_events_occurrence_stem
    ON canonical_events (entity_id, occurrence_stem)
    WHERE is_active AND pipeline = 'person' AND occurrence_stem IS NOT NULL;

DROP INDEX IF EXISTS idx_canonical_events_subject_type_time;
CREATE INDEX IF NOT EXISTS idx_canonical_events_subject_type_time
    ON canonical_events (entity_id, event_type, start_time)
    WHERE is_active AND pipeline = 'person';

DROP INDEX IF EXISTS idx_canonical_events_timeline_eligible;
CREATE INDEX IF NOT EXISTS idx_canonical_events_timeline_eligible
    ON canonical_events (entity_id)
    WHERE is_active AND pipeline = 'person' AND timeline_eligible;

DROP INDEX IF EXISTS uq_canonical_active_occurrence;
CREATE UNIQUE INDEX IF NOT EXISTS uq_canonical_active_occurrence
    ON canonical_events (entity_id, occurrence_key)
    WHERE is_active AND pipeline = 'person' AND occurrence_key IS NOT NULL;

DROP INDEX IF EXISTS uq_canonical_active_singleton_birth_death;
CREATE UNIQUE INDEX IF NOT EXISTS uq_canonical_active_singleton_birth_death
    ON canonical_events (entity_id, event_type)
    WHERE is_active
      AND pipeline = 'person'
      AND event_type IN ('birth', 'death');

DROP INDEX IF EXISTS uq_canonical_events_active_fingerprint;
CREATE UNIQUE INDEX IF NOT EXISTS uq_canonical_events_active_fingerprint
    ON canonical_events (fingerprint)
    WHERE is_active AND fingerprint IS NOT NULL AND pipeline = 'person';

DROP INDEX IF EXISTS uq_canonical_events_active_person_occurrence;

-- Non-unique lookup always. Unique QID only when no duplicates (rebuild merges first).
CREATE INDEX IF NOT EXISTS idx_entities_qid ON entities (qid);

DO $$
BEGIN
    IF EXISTS (
        SELECT 1 FROM pg_class c
        JOIN pg_namespace n ON n.oid = c.relnamespace
        WHERE c.relname = 'uq_entities_qid' AND n.nspname = 'public'
    ) THEN
        RETURN;
    END IF;
    IF (
        SELECT count(*) FROM (
            SELECT qid FROM entities WHERE qid IS NOT NULL GROUP BY qid HAVING count(*) > 1
        ) s
    ) = 0 THEN
        CREATE UNIQUE INDEX uq_entities_qid ON entities (qid) WHERE qid IS NOT NULL;
    ELSE
        RAISE NOTICE 'uq_entities_qid skipped: duplicate qids present; rebuild-person-pipeline will create after merge';
    END IF;
END $$;

ALTER TABLE event_candidates ALTER COLUMN snapshot_id DROP NOT NULL;
ALTER TABLE event_candidates ALTER COLUMN fragment_id DROP NOT NULL;
ALTER TABLE event_candidates
    ADD COLUMN IF NOT EXISTS raw_document_id UUID REFERENCES raw_documents(id) ON DELETE SET NULL;

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint WHERE conname = 'event_candidates_source_present'
    ) THEN
        ALTER TABLE event_candidates
            ADD CONSTRAINT event_candidates_source_present
            CHECK (snapshot_id IS NOT NULL OR raw_document_id IS NOT NULL);
    END IF;
END $$;

ALTER TABLE event_evidence
    ADD COLUMN IF NOT EXISTS evidence_hash TEXT,
    ADD COLUMN IF NOT EXISTS source_locator TEXT;

UPDATE event_evidence
SET evidence_hash = md5(
    canonical_event_id::text
    || coalesce(raw_document_id::text, '')
    || coalesce(quoted_text, '')
)
WHERE evidence_hash IS NULL;

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint WHERE conname = 'uq_event_evidence_dedup'
    ) THEN
        ALTER TABLE event_evidence
            ADD CONSTRAINT uq_event_evidence_dedup
            UNIQUE NULLS NOT DISTINCT (canonical_event_id, raw_document_id, evidence_hash);
    END IF;
END $$;
