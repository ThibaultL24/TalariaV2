-- 043_snac_constellations.sql
-- SNAC identity: constellations and one-to-one alignment links.
-- Additive. Does not write canonical events.

CREATE TABLE snac_constellations (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    snac_id TEXT,
    ark TEXT NOT NULL,
    snac_version TEXT,
    entity_type TEXT NOT NULL
        CHECK (entity_type IN ('person', 'corporateBody', 'family')),
    label TEXT NOT NULL,
    record_status TEXT NOT NULL
        CHECK (record_status IN ('stub', 'complete', 'split', 'withdrawn')),
    same_as JSONB NOT NULL DEFAULT '{"values":{},"raw":[],"internal_conflict":false}'::jsonb,
    exist_dates JSONB NOT NULL DEFAULT '{"kind":"unknown"}'::jsonb,
    snapshot_id UUID REFERENCES document_snapshots(id) ON DELETE SET NULL,
    raw_json JSONB NOT NULL DEFAULT '{}'::jsonb,
    source_url TEXT NOT NULL,
    retrieved_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (ark),
    CHECK (exist_dates->>'kind' IN ('exact', 'range', 'approx', 'unknown'))
);

CREATE UNIQUE INDEX uq_snac_constellations_snac_id
    ON snac_constellations (snac_id)
    WHERE snac_id IS NOT NULL;

CREATE TABLE entity_identity_links (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    entity_id UUID NOT NULL REFERENCES entities(id) ON DELETE CASCADE,
    constellation_id UUID NOT NULL REFERENCES snac_constellations(id) ON DELETE CASCADE,
    status TEXT NOT NULL
        CHECK (status IN ('aligned', 'ambiguous', 'conflict', 'rejected')),
    method TEXT NOT NULL
        CHECK (method IN (
            'qid_same_as',
            'viaf_same_as',
            'isni_same_as',
            'viaf_and_isni',
            'conflicting_qid',
            'conflicting_authority',
            'internal_identifier_conflict',
            'multiple_certain_matches',
            'constellation_claimed',
            'split_identity',
            'label_and_years',
            'label_only'
        )),
    evidence JSONB NOT NULL DEFAULT '{}'::jsonb,
    retrieved_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (entity_id, constellation_id)
);

-- A certain alignment is one Talaria entity and one SNAC constellation.
CREATE UNIQUE INDEX uq_entity_identity_links_one_aligned_entity
    ON entity_identity_links (entity_id)
    WHERE status = 'aligned';

CREATE UNIQUE INDEX uq_entity_identity_links_one_aligned_constellation
    ON entity_identity_links (constellation_id)
    WHERE status = 'aligned';

CREATE INDEX idx_entity_identity_links_entity_status
    ON entity_identity_links (entity_id, status);
