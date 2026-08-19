-- 017_intuition_publications.sql
-- Queue for Intuition debate export (opinions only). Cultural events stay in Talaria.

CREATE TABLE IF NOT EXISTS intuition_publications (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    subject_entity_id UUID NOT NULL REFERENCES entities(id) ON DELETE CASCADE,
    debate_id TEXT NOT NULL,
    bundle_fingerprint TEXT NOT NULL,
    kind TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'planned'
        CHECK (status IN ('planned', 'exported', 'published', 'failed')),
    payload_json JSONB NOT NULL DEFAULT '{}'::jsonb,
    chain_id INT,
    question_term_id TEXT,
    triple_term_id TEXT,
    tx_hash TEXT,
    last_error TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (bundle_fingerprint)
);

CREATE INDEX IF NOT EXISTS idx_intuition_publications_subject
    ON intuition_publications (subject_entity_id, status);

CREATE INDEX IF NOT EXISTS idx_intuition_publications_debate
    ON intuition_publications (debate_id);
