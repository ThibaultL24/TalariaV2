-- migrations/014_soft_claims.sql
-- Explorer soft-claim layer (facts, anecdotes, debates) with evidence.
-- Distinct from Intuition opinion `claims` in 005_raw_documents_and_claims.sql
-- and from cultural `quality_claims` in 010_multi_source_foundations.sql.

CREATE TABLE IF NOT EXISTS soft_claims (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    entity_id UUID NOT NULL REFERENCES entities(id) ON DELETE CASCADE,
    claim_kind TEXT NOT NULL
        CHECK (claim_kind IN (
            'fact', 'anecdote', 'context', 'theory', 'controversy',
            'debate_stance', 'attribution', 'life_event'
        )),
    text TEXT NOT NULL,
    epistemic_status TEXT NOT NULL DEFAULT 'attested',
    relation_to_subject TEXT NOT NULL DEFAULT 'direct'
        CHECK (relation_to_subject IN ('direct', 'indirect', 'historiography', 'legacy')),
    event_time TIMESTAMPTZ,
    place_label TEXT,
    confidence DOUBLE PRECISION NOT NULL DEFAULT 0,
    canonical_event_id UUID REFERENCES canonical_events(id) ON DELETE SET NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS soft_claim_evidence (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    claim_id UUID NOT NULL REFERENCES soft_claims(id) ON DELETE CASCADE,
    source_system TEXT NOT NULL,
    locator TEXT,
    quote TEXT,
    sentence_id UUID REFERENCES sentences(id) ON DELETE SET NULL,
    confidence DOUBLE PRECISION NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS soft_claim_relations (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    from_claim_id UUID NOT NULL REFERENCES soft_claims(id) ON DELETE CASCADE,
    to_claim_id UUID NOT NULL REFERENCES soft_claims(id) ON DELETE CASCADE,
    relation TEXT NOT NULL
        CHECK (relation IN ('supports', 'contradicts', 'debates', 'qualifies')),
    UNIQUE (from_claim_id, to_claim_id, relation)
);

CREATE INDEX IF NOT EXISTS idx_soft_claims_entity ON soft_claims (entity_id);
CREATE INDEX IF NOT EXISTS idx_soft_claims_kind ON soft_claims (claim_kind);
CREATE INDEX IF NOT EXISTS idx_soft_claims_event ON soft_claims (canonical_event_id);
CREATE INDEX IF NOT EXISTS idx_soft_claim_evidence_claim ON soft_claim_evidence (claim_id);
