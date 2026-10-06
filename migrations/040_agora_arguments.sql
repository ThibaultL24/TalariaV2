-- 040_agora_arguments.sql
-- User-proposed Agora arguments reuse soft_claims + soft_claim_relations.
-- Sources attach only to indexed corpus_documents (Scholar). No free-text URLs.

ALTER TABLE soft_claims
    ADD COLUMN IF NOT EXISTS origin TEXT NOT NULL DEFAULT 'pipeline'
        CHECK (origin IN ('pipeline', 'user')),
    ADD COLUMN IF NOT EXISTS contribution_status TEXT NOT NULL DEFAULT 'active'
        CHECK (contribution_status IN ('proposed', 'active', 'rejected')),
    ADD COLUMN IF NOT EXISTS created_by_user_id UUID REFERENCES users(id) ON DELETE SET NULL;

CREATE INDEX IF NOT EXISTS idx_soft_claims_created_by
    ON soft_claims (created_by_user_id)
    WHERE created_by_user_id IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_soft_claims_origin
    ON soft_claims (entity_id, origin);

CREATE INDEX IF NOT EXISTS idx_soft_claim_relations_to_rel
    ON soft_claim_relations (to_claim_id, relation);

ALTER TABLE soft_claim_evidence
    ADD COLUMN IF NOT EXISTS corpus_document_id UUID
        REFERENCES corpus_documents(id) ON DELETE SET NULL,
    ADD COLUMN IF NOT EXISTS fragment_id UUID
        REFERENCES document_fragments(id) ON DELETE SET NULL;

CREATE INDEX IF NOT EXISTS idx_soft_claim_evidence_document
    ON soft_claim_evidence (corpus_document_id)
    WHERE corpus_document_id IS NOT NULL;

CREATE TABLE IF NOT EXISTS soft_claim_sources (
    claim_id UUID NOT NULL
        REFERENCES soft_claims(id) ON DELETE CASCADE,
    corpus_document_id UUID NOT NULL
        REFERENCES corpus_documents(id) ON DELETE CASCADE,
    attached_by_user_id UUID
        REFERENCES users(id) ON DELETE SET NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (claim_id, corpus_document_id)
);

CREATE INDEX IF NOT EXISTS idx_soft_claim_sources_document
    ON soft_claim_sources (corpus_document_id);
