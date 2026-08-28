-- migrations/028_pipeline_legacy_and_person.sql
-- Repair CHECK if 027 already applied as person-only. Dump stays legacy; explorer stays person.
ALTER TABLE canonical_events DROP CONSTRAINT IF EXISTS canonical_events_pipeline_check;
ALTER TABLE canonical_events
    ADD CONSTRAINT canonical_events_pipeline_check
    CHECK (pipeline IN ('legacy', 'person')) NOT VALID;
ALTER TABLE canonical_events ALTER COLUMN pipeline SET DEFAULT 'person';
