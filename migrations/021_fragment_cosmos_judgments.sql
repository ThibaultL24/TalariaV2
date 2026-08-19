-- 021_fragment_cosmos_judgments.sql
-- Auditable Cosmos filter scores on document_fragments.
-- Does not create canonical_events. Versioned unique key keeps audit rows.

CREATE TABLE IF NOT EXISTS fragment_cosmos_judgments (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    fragment_id UUID NOT NULL REFERENCES document_fragments(id) ON DELETE CASCADE,
    analyzer_id TEXT NOT NULL,
    version TEXT NOT NULL,
    score REAL NOT NULL CHECK (score >= 0 AND score <= 1),
    accepted BOOLEAN NOT NULL,
    signals JSONB NOT NULL DEFAULT '[]'::jsonb,
    tuples JSONB NOT NULL DEFAULT '[]'::jsonb,
    reject_reason TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (fragment_id, analyzer_id, version)
);

CREATE INDEX IF NOT EXISTS idx_fragment_cosmos_judgments_fragment
    ON fragment_cosmos_judgments (fragment_id);

CREATE INDEX IF NOT EXISTS idx_fragment_cosmos_judgments_accepted
    ON fragment_cosmos_judgments (accepted);
