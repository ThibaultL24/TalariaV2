-- 018_soft_claim_debate_fields.sql
-- Typed debate metadata on explorer soft_claims (historiography extractor).
-- Additive; does not alter quality/legacy events.

ALTER TABLE soft_claims
    ADD COLUMN IF NOT EXISTS debate_type TEXT,
    ADD COLUMN IF NOT EXISTS evidence_layer TEXT;

CREATE INDEX IF NOT EXISTS idx_soft_claims_debate_type
    ON soft_claims (debate_type)
    WHERE debate_type IS NOT NULL;
