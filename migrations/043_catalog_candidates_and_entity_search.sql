-- Catalog hits may suggest a document/event connection, but are not evidence.
-- Candidate records never flow into event_evidence without a verified fragment.
CREATE TABLE IF NOT EXISTS event_document_candidates (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    canonical_event_id UUID NOT NULL REFERENCES canonical_events(id) ON DELETE CASCADE,
    raw_document_id UUID NOT NULL REFERENCES raw_documents(id) ON DELETE CASCADE,
    match_rule TEXT NOT NULL CHECK (match_rule IN ('place_mention','title_mention')),
    match_version TEXT NOT NULL CHECK (length(match_version) > 0),
    relevance_score DOUBLE PRECISION NOT NULL CHECK (relevance_score BETWEEN 0 AND 1),
    review_status TEXT NOT NULL DEFAULT 'pending'
        CHECK (review_status IN ('pending','rejected')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (canonical_event_id, raw_document_id, match_rule, match_version)
);
CREATE INDEX IF NOT EXISTS idx_event_document_candidates_review
    ON event_document_candidates (review_status, canonical_event_id);
CREATE INDEX IF NOT EXISTS idx_event_document_candidates_source
    ON event_document_candidates (raw_document_id);

-- Operator note: on large databases, build these indexes CONCURRENTLY outside
-- of the SQLx transactional migrator after measuring index cost.
CREATE EXTENSION IF NOT EXISTS pg_trgm;
CREATE INDEX IF NOT EXISTS idx_entities_canonical_name_trgm
    ON entities USING GIN (canonical_name gin_trgm_ops);
CREATE INDEX IF NOT EXISTS idx_entities_wikipedia_title_trgm
    ON entities USING GIN (wikipedia_title gin_trgm_ops);
CREATE INDEX IF NOT EXISTS idx_entity_aliases_surface_trgm
    ON entity_aliases USING GIN (surface gin_trgm_ops);
