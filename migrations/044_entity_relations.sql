-- 044_entity_relations.sql
-- SNAC relations between constellations. Not claims, and not canonical events.
-- Additive. A missing relation stays in the table with is_current = false.

CREATE TABLE entity_relations (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    subject_constellation_id UUID NOT NULL
        REFERENCES snac_constellations(id) ON DELETE CASCADE,
    object_constellation_id UUID NOT NULL
        REFERENCES snac_constellations(id) ON DELETE CASCADE,
    predicate TEXT NOT NULL
        CHECK (predicate IN (
            'corresponded_with', 'associated_with', 'acquaintance_of',
            'parent_of', 'child_of', 'sibling_of', 'spouse_of',
            'member_of', 'has_member', 'founder_of', 'founded_by',
            'unmapped'
        )),
    source_predicate TEXT NOT NULL,
    source_assertion_key TEXT NOT NULL,
    snac_relation_id TEXT,
    assertion_text TEXT,
    dates_json JSONB NOT NULL DEFAULT '{"kind":"unknown"}'::jsonb,
    validation_status TEXT NOT NULL
        CHECK (validation_status IN ('accepted', 'needs_review')),
    is_current BOOLEAN NOT NULL DEFAULT true,
    source_kind TEXT NOT NULL DEFAULT 'snac' CHECK (source_kind = 'snac'),
    source_url TEXT NOT NULL,
    snapshot_id UUID NOT NULL REFERENCES document_snapshots(id),
    content_hash TEXT NOT NULL,
    snac_version TEXT NOT NULL,
    retrieved_at TIMESTAMPTZ NOT NULL,
    superseded_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CHECK (subject_constellation_id <> object_constellation_id),
    CHECK (dates_json->>'kind' IN ('exact', 'range', 'approx', 'unknown')),
    CHECK (
        (validation_status = 'accepted' AND predicate <> 'unmapped')
        OR (validation_status = 'needs_review' AND predicate = 'unmapped')
    )
);

CREATE UNIQUE INDEX uq_entity_relations_current_assertion
    ON entity_relations (subject_constellation_id, source_assertion_key)
    WHERE is_current;

CREATE INDEX idx_entity_relations_subject_current
    ON entity_relations (subject_constellation_id, predicate)
    WHERE validation_status = 'accepted' AND is_current;

CREATE INDEX idx_entity_relations_object_current
    ON entity_relations (object_constellation_id, predicate)
    WHERE validation_status = 'accepted' AND is_current;
