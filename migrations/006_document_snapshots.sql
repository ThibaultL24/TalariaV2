-- 006_document_snapshots.sql
-- Immutable document snapshots + clause-aware fragments (additive, reversible).

CREATE TABLE IF NOT EXISTS document_snapshots (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    source_type TEXT NOT NULL,
    source_uri TEXT NOT NULL,
    source_identifier TEXT,
    language TEXT NOT NULL DEFAULT 'en',
    title TEXT,
    content_hash TEXT NOT NULL,
    revision_id TEXT,
    wiki_page_id UUID REFERENCES wiki_pages(id) ON DELETE SET NULL,
    raw_document_id UUID REFERENCES raw_documents(id) ON DELETE SET NULL,
    text TEXT NOT NULL,
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (source_type, source_uri, content_hash)
);

CREATE INDEX IF NOT EXISTS idx_document_snapshots_wiki_page
    ON document_snapshots (wiki_page_id);

CREATE TABLE IF NOT EXISTS document_fragments (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    snapshot_id UUID NOT NULL REFERENCES document_snapshots(id) ON DELETE CASCADE,
    fragment_kind TEXT NOT NULL DEFAULT 'sentence'
        CHECK (fragment_kind IN ('sentence', 'clause')),
    parent_fragment_id UUID REFERENCES document_fragments(id) ON DELETE CASCADE,
    sentence_id UUID REFERENCES sentences(id) ON DELETE SET NULL,
    text TEXT NOT NULL,
    start_offset INT NOT NULL CHECK (start_offset >= 0),
    end_offset INT NOT NULL,
    clause_index INT,
    ordinal INT NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CHECK (end_offset >= start_offset),
    CHECK (
        (fragment_kind = 'sentence' AND clause_index IS NULL)
        OR (fragment_kind = 'clause' AND clause_index IS NOT NULL)
    )
);

CREATE INDEX IF NOT EXISTS idx_document_fragments_snapshot
    ON document_fragments (snapshot_id);

CREATE INDEX IF NOT EXISTS idx_document_fragments_parent
    ON document_fragments (parent_fragment_id);

CREATE UNIQUE INDEX IF NOT EXISTS uq_document_fragments_sentence_ordinal
    ON document_fragments (snapshot_id, ordinal)
    WHERE fragment_kind = 'sentence';

CREATE UNIQUE INDEX IF NOT EXISTS uq_document_fragments_clause_index
    ON document_fragments (parent_fragment_id, clause_index)
    WHERE fragment_kind = 'clause';
