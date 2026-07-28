-- migrations/003_epistemic_status.sql
-- Epistemic status is orthogonal to event_type (category).

ALTER TABLE canonical_events
    ADD COLUMN IF NOT EXISTS epistemic_status TEXT NOT NULL DEFAULT 'attested';

ALTER TABLE canonical_events
    DROP CONSTRAINT IF EXISTS canonical_events_epistemic_status_check;

ALTER TABLE canonical_events
    ADD CONSTRAINT canonical_events_epistemic_status_check
    CHECK (epistemic_status IN (
        'established',
        'attested',
        'uncertain',
        'theory',
        'rumor'
    ));

CREATE INDEX IF NOT EXISTS idx_canonical_events_epistemic
    ON canonical_events (epistemic_status);

CREATE INDEX IF NOT EXISTS idx_canonical_events_type
    ON canonical_events (event_type);
