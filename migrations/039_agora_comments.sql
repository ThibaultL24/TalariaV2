-- 039_agora_comments.sql
-- Agora discussion on claims. Soft-delete parents; replies are not cascaded by the API.

CREATE TABLE comments (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    user_id UUID NOT NULL
        REFERENCES users(id)
        ON DELETE CASCADE,

    claim_id UUID NOT NULL
        REFERENCES soft_claims(id)
        ON DELETE CASCADE,

    parent_comment_id UUID
        REFERENCES comments(id)
        ON DELETE CASCADE,

    body TEXT NOT NULL
        CHECK (char_length(body) BETWEEN 1 AND 4000),

    status TEXT NOT NULL DEFAULT 'active'
        CHECK (status IN ('active', 'deleted')),

    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    edited_at TIMESTAMPTZ
);

CREATE INDEX idx_comments_claim_created
    ON comments (claim_id, created_at DESC);

CREATE INDEX idx_comments_parent_created
    ON comments (parent_comment_id, created_at ASC);

CREATE INDEX idx_comments_user_created
    ON comments (user_id, created_at DESC);

CREATE TABLE comment_reactions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    comment_id UUID NOT NULL
        REFERENCES comments(id)
        ON DELETE CASCADE,

    user_id UUID NOT NULL
        REFERENCES users(id)
        ON DELETE CASCADE,

    reaction_type TEXT NOT NULL
        CHECK (reaction_type IN (
            'relevant',
            'well_sourced',
            'interesting',
            'needs_nuance',
            'disagree'
        )),

    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    UNIQUE (comment_id, user_id, reaction_type)
);

CREATE INDEX idx_comment_reactions_comment
    ON comment_reactions (comment_id);

CREATE INDEX idx_comment_reactions_comment_type
    ON comment_reactions (comment_id, reaction_type);
