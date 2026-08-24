-- migrations/024_wikibase_statements.sql
-- Canonical store for full Wikibase claims (qualifiers, ranks, references).

CREATE TABLE IF NOT EXISTS wikibase_statements (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    qid TEXT NOT NULL,
    guid TEXT NOT NULL,
    property TEXT NOT NULL,
    rank TEXT NOT NULL,
    snaktype TEXT NOT NULL,
    value_json JSONB NOT NULL DEFAULT '{}'::jsonb,
    qualifiers_json JSONB NOT NULL DEFAULT '{}'::jsonb,
    references_json JSONB NOT NULL DEFAULT '[]'::jsonb,
    revision_id TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (guid, revision_id)
);
CREATE INDEX IF NOT EXISTS idx_wikibase_statements_qid ON wikibase_statements (qid);
CREATE INDEX IF NOT EXISTS idx_wikibase_statements_pid ON wikibase_statements (qid, property);
