-- Visit lens — time-bounded exhibitions and events (separate from canonical life occurrences).
CREATE TABLE IF NOT EXISTS visit_opportunities (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    entity_id UUID NOT NULL REFERENCES entities(id) ON DELETE CASCADE,
    kind TEXT NOT NULL DEFAULT 'exhibition'
        CHECK (kind IN ('exhibition', 'event', 'festival', 'guided_tour', 'other')),
    title TEXT NOT NULL,
    summary TEXT,
    venue_label TEXT,
    geom GEOGRAPHY(POINT),
    starts_at TIMESTAMPTZ,
    ends_at TIMESTAMPTZ,
    canonical_url TEXT,
    source_kind TEXT NOT NULL,
    source_record_id TEXT NOT NULL,
    fetched_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (entity_id, source_kind, source_record_id)
);

CREATE INDEX IF NOT EXISTS idx_visit_opportunities_entity_dates
    ON visit_opportunities (entity_id, starts_at, ends_at);

CREATE INDEX IF NOT EXISTS idx_visit_opportunities_geom
    ON visit_opportunities USING GIST (geom)
    WHERE geom IS NOT NULL;
