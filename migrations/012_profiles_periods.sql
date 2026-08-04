-- migrations/012_profiles_periods.sql
-- Universal entity profiles (Wikidata-shaped) + time periods for Explorer filters.

CREATE TABLE IF NOT EXISTS periods (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    slug TEXT NOT NULL UNIQUE,
    label TEXT NOT NULL,
    start_year INT,
    end_year INT,
    kind TEXT NOT NULL DEFAULT 'century'
        CHECK (kind IN ('year', 'decade', 'century', 'era', 'reign', 'custom')),
    wikidata_qid TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS entity_profiles (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    entity_id UUID NOT NULL REFERENCES entities(id) ON DELETE CASCADE,
    profile_qid TEXT,
    profile_slug TEXT NOT NULL,
    profile_label TEXT NOT NULL,
    kind TEXT NOT NULL DEFAULT 'occupation'
        CHECK (kind IN ('occupation', 'position', 'field', 'custom')),
    confidence DOUBLE PRECISION NOT NULL DEFAULT 0.8,
    source_system TEXT NOT NULL DEFAULT 'wikidata',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (entity_id, profile_slug, kind)
);

CREATE TABLE IF NOT EXISTS entity_periods (
    entity_id UUID NOT NULL REFERENCES entities(id) ON DELETE CASCADE,
    period_id UUID NOT NULL REFERENCES periods(id) ON DELETE CASCADE,
    PRIMARY KEY (entity_id, period_id)
);

CREATE INDEX IF NOT EXISTS idx_entity_profiles_slug ON entity_profiles (profile_slug);
CREATE INDEX IF NOT EXISTS idx_entity_profiles_entity ON entity_profiles (entity_id);
CREATE INDEX IF NOT EXISTS idx_periods_years ON periods (start_year, end_year);
