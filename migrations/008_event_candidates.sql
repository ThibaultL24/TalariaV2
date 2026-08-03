-- 008_event_candidates.sql
-- Strict EventCandidate persistence with structured rejection codes.

CREATE TABLE IF NOT EXISTS event_candidates (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    snapshot_id UUID NOT NULL REFERENCES document_snapshots(id) ON DELETE CASCADE,
    fragment_id UUID NOT NULL REFERENCES document_fragments(id) ON DELETE CASCADE,
    clause_index INT NOT NULL DEFAULT 0,

    subject_surface TEXT NOT NULL,
    subject_entity_id UUID REFERENCES entities(id) ON DELETE SET NULL,

    event_type TEXT NOT NULL,
    predicate TEXT NOT NULL,

    -- Typed time: {kind: exact|range|approx|unknown, year?, start?, end?, surface?}
    time_json JSONB NOT NULL DEFAULT '{"kind":"unknown"}'::jsonb,

    -- Separate mention bags: [{surface, entity_id?, kind?, role?}, ...]
    place_mentions JSONB NOT NULL DEFAULT '[]'::jsonb,
    object_mentions JSONB NOT NULL DEFAULT '[]'::jsonb,
    participant_mentions JSONB NOT NULL DEFAULT '[]'::jsonb,

    -- Resolved place only when kind=place
    place_entity_id UUID REFERENCES entities(id) ON DELETE SET NULL,
    place_label TEXT,

    -- Clause-level evidence pointers
    evidence_ptrs JSONB NOT NULL DEFAULT '[]'::jsonb,

    extractor_version TEXT NOT NULL,
    fingerprint TEXT NOT NULL,

    status TEXT NOT NULL DEFAULT 'pending'
        CHECK (status IN ('pending', 'needs_review', 'accepted', 'rejected', 'assembled')),
    rejection_codes TEXT[] NOT NULL DEFAULT '{}',
    judgment_json JSONB NOT NULL DEFAULT '{}'::jsonb,

    canonical_event_id UUID REFERENCES canonical_events(id) ON DELETE SET NULL,
    phrase_candidate_id UUID REFERENCES phrase_candidates(id) ON DELETE SET NULL,

    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (fingerprint)
);

CREATE INDEX IF NOT EXISTS idx_event_candidates_status
    ON event_candidates (status);

CREATE INDEX IF NOT EXISTS idx_event_candidates_subject
    ON event_candidates (subject_entity_id);

CREATE INDEX IF NOT EXISTS idx_event_candidates_snapshot
    ON event_candidates (snapshot_id);
