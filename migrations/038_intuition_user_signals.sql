-- 038_intuition_user_signals.sql
-- Immutable Intuition deposits. Distinct from mutable Talaria stance (user_interactions).
-- One row per confirmed tx. Stance replacement must not delete these rows.

CREATE TABLE intuition_user_signals (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    user_id UUID NOT NULL
        REFERENCES users(id)
        ON DELETE CASCADE,

    claim_id UUID NOT NULL
        REFERENCES soft_claims(id)
        ON DELETE CASCADE,

    -- Current Talaria stance row at sync time. SET NULL if PR3 replaces/deletes it.
    interaction_id UUID
        REFERENCES user_interactions(id)
        ON DELETE SET NULL,

    action_type TEXT NOT NULL
        CHECK (action_type IN ('support', 'dispute')),

    chain_id BIGINT NOT NULL,
    term_id TEXT NOT NULL,
    curve_id NUMERIC(78, 0) NOT NULL,

    tx_hash TEXT NOT NULL,
    assets NUMERIC(78, 0) NOT NULL,
    shares NUMERIC(78, 0),
    receiver TEXT NOT NULL,

    status TEXT NOT NULL
        CHECK (status IN ('submitted', 'confirmed', 'failed')),

    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    confirmed_at TIMESTAMPTZ,

    UNIQUE (tx_hash)
);

CREATE INDEX idx_intuition_user_signals_user_created
    ON intuition_user_signals (user_id, created_at DESC);

CREATE INDEX idx_intuition_user_signals_claim_created
    ON intuition_user_signals (claim_id, created_at DESC);

CREATE INDEX idx_intuition_user_signals_term
    ON intuition_user_signals (term_id);
