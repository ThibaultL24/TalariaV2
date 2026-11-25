-- 055_editorial_assertions.sql
-- A chapter sentence can point at one logical fact and the passages that support that sentence.
-- Passages stay in event_evidence. This table does not copy documents or change canonical facts.
-- Not applied to talaria_engine_development until a separate authorization.

CREATE TABLE IF NOT EXISTS editorial_assertions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    group_id UUID NOT NULL REFERENCES editorial_groups(id) ON DELETE CASCADE,
    display_order INT NOT NULL DEFAULT 0,
    prose TEXT NOT NULL,
    assertion_kind TEXT NOT NULL,
    logical_event_id UUID REFERENCES logical_events(id) ON DELETE RESTRICT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT editorial_assertions_kind_check CHECK (
        assertion_kind IN ('attested', 'editorial', 'uncertainty', 'gap')
    ),
    CONSTRAINT editorial_assertions_prose_not_blank CHECK (length(btrim(prose)) > 0),
    CONSTRAINT editorial_assertions_gap_has_no_event CHECK (
        assertion_kind <> 'gap' OR logical_event_id IS NULL
    ),
    CONSTRAINT editorial_assertions_attested_has_event CHECK (
        assertion_kind <> 'attested' OR logical_event_id IS NOT NULL
    )
);

CREATE INDEX IF NOT EXISTS idx_editorial_assertions_group_order
    ON editorial_assertions (group_id, display_order, id);

CREATE TABLE IF NOT EXISTS editorial_assertion_passages (
    assertion_id UUID NOT NULL REFERENCES editorial_assertions(id) ON DELETE CASCADE,
    event_evidence_id UUID NOT NULL REFERENCES event_evidence(id) ON DELETE RESTRICT,
    evidence_hash TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (assertion_id, event_evidence_id),
    CONSTRAINT editorial_assertion_passages_hash_not_blank CHECK (length(btrim(evidence_hash)) > 0)
);

CREATE INDEX IF NOT EXISTS idx_editorial_assertion_passages_evidence
    ON editorial_assertion_passages (event_evidence_id);
