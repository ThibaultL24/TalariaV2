-- 037_user_interactions.sql
-- Stateful Talaria interactions (off-chain). Not activity events (search/view/click).
--
-- Target contracts (PR3):
--   person  → entities.id WHERE kind = 'person'
--   place   → entities.id WHERE kind = 'place'
--   claim   → soft_claims.id
--   event   → canonical_events.id
--   source  → corpus_documents.id  (v1 = Scholar corpus document only)
--   route   → NOT IN THIS MIGRATION. Visit has visit_opportunities / generated
--             itineraries, no stable route UUID. TODO PR8.
--
-- recommend is omitted until route exists (it had no other allowed target).

CREATE TABLE user_interactions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    user_id UUID NOT NULL
        REFERENCES users(id)
        ON DELETE CASCADE,

    action_type TEXT NOT NULL
        CHECK (action_type IN (
            'interest',
            'follow',
            'save',
            'support',
            'dispute',
            'uncertain',
            'useful',
            'credible',
            'not_credible',
            'want_to_visit',
            'visited'
        )),

    target_type TEXT NOT NULL
        CHECK (target_type IN (
            'person',
            'claim',
            'source',
            'place',
            'event'
        )),

    target_id UUID NOT NULL,

    visibility TEXT NOT NULL DEFAULT 'private'
        CHECK (visibility IN ('private', 'public')),

    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    UNIQUE (user_id, action_type, target_type, target_id),

    CONSTRAINT user_interactions_action_target_matrix CHECK (
        (
            target_type = 'person'
            AND action_type IN ('interest', 'follow', 'save')
        )
        OR
        (
            target_type = 'claim'
            AND action_type IN ('support', 'dispute', 'uncertain', 'save')
        )
        OR
        (
            target_type = 'source'
            AND action_type IN ('useful', 'credible', 'not_credible', 'save')
        )
        OR
        (
            target_type = 'place'
            AND action_type IN ('save', 'want_to_visit', 'visited')
        )
        OR
        (
            target_type = 'event'
            AND action_type = 'save'
        )
    )
);

-- One epistemic stance per user+claim. API replaces transactionally (support→dispute).
CREATE UNIQUE INDEX uq_user_claim_epistemic_stance
    ON user_interactions (user_id, target_id)
    WHERE target_type = 'claim'
      AND action_type IN ('support', 'dispute', 'uncertain');

-- GET /me/interactions cursor pages newest-first.
CREATE INDEX idx_user_interactions_user_created
    ON user_interactions (user_id, created_at DESC);

-- Filter mine by action without scanning the user's full history.
CREATE INDEX idx_user_interactions_user_action
    ON user_interactions (user_id, action_type);

-- Lookup / existence by polymorphic target.
CREATE INDEX idx_user_interactions_target
    ON user_interactions (target_type, target_id);

-- Public batch summary: GROUP BY target_id, action_type WHERE visibility='public'.
CREATE INDEX idx_user_interactions_target_action_public
    ON user_interactions (target_type, target_id, action_type)
    WHERE visibility = 'public';
