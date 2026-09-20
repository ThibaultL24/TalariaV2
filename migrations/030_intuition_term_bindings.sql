-- 030_intuition_term_bindings.sql
-- Typed atom/triple bindings for Intuition publications (additive).

CREATE TABLE IF NOT EXISTS intuition_term_bindings (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    publication_id UUID NOT NULL
        REFERENCES intuition_publications(id) ON DELETE CASCADE,
    role TEXT NOT NULL,
    local_kind TEXT NOT NULL,
    local_id TEXT,
    chain_id INT NOT NULL,
    term_id TEXT NOT NULL,
    ipfs_uri TEXT,
    data_hash TEXT,
    classification TEXT,
    created_on_chain BOOLEAN NOT NULL DEFAULT FALSE,
    verified_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (publication_id, role)
);

CREATE INDEX IF NOT EXISTS idx_intuition_term_bindings_term
    ON intuition_term_bindings (chain_id, term_id);

CREATE INDEX IF NOT EXISTS idx_intuition_term_bindings_local
    ON intuition_term_bindings (local_kind, local_id)
    WHERE local_id IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_intuition_term_bindings_publication_role
    ON intuition_term_bindings (publication_id, role);

-- Soft claim id for stable claim→publication lookup (replaces ILIKE payload scan).
ALTER TABLE intuition_publications
    ADD COLUMN IF NOT EXISTS soft_claim_id UUID;

CREATE INDEX IF NOT EXISTS idx_intuition_publications_soft_claim
    ON intuition_publications (soft_claim_id)
    WHERE soft_claim_id IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_intuition_publications_retryable
    ON intuition_publications (status, updated_at)
    WHERE status IN ('pending', 'failed', 'pin_failed');
