-- migrations/005_raw_documents_and_claims.sql
-- V1-inspired provenance + opinion lane.
-- Cultural facts (biography, places, dates) stay in canonical_events.
-- claims is reserved for avis / théories / débats destined for Intuition export.

CREATE TABLE IF NOT EXISTS raw_documents (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    source_type TEXT NOT NULL,
    source_uri TEXT NOT NULL,
    source_identifier TEXT,
    title TEXT,
    language TEXT NOT NULL DEFAULT 'en',
    content_hash TEXT,
    revision_id TEXT,
    license TEXT,
    wiki_page_id UUID REFERENCES wiki_pages(id) ON DELETE SET NULL,
    payload JSONB NOT NULL DEFAULT '{}'::jsonb,
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_raw_documents_source
    ON raw_documents (source_type, source_uri);

CREATE INDEX IF NOT EXISTS idx_raw_documents_wiki_page
    ON raw_documents (wiki_page_id);

CREATE TABLE IF NOT EXISTS claims (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    subject_entity_id UUID NOT NULL REFERENCES entities(id) ON DELETE CASCADE,
    predicate TEXT NOT NULL,
    object_entity_id UUID REFERENCES entities(id) ON DELETE SET NULL,
    value_json JSONB,
    epistemic_status TEXT NOT NULL DEFAULT 'attested'
        CHECK (epistemic_status IN (
            'established', 'attested', 'uncertain', 'theory', 'rumor'
        )),
    confidence DOUBLE PRECISION NOT NULL DEFAULT 0
        CHECK (confidence >= 0 AND confidence <= 1),
    status TEXT NOT NULL DEFAULT 'draft',
    exportable BOOLEAN NOT NULL DEFAULT true,
    phrase_candidate_id UUID REFERENCES phrase_candidates(id) ON DELETE SET NULL,
    sentence_id UUID REFERENCES sentences(id) ON DELETE SET NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_claims_subject ON claims (subject_entity_id);
CREATE INDEX IF NOT EXISTS idx_claims_epistemic ON claims (epistemic_status);
CREATE INDEX IF NOT EXISTS idx_claims_exportable ON claims (exportable)
    WHERE exportable = true;
