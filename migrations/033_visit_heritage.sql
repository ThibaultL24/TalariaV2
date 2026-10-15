-- Visit heritage is a projection beside biography. It never updates canonical_events.
CREATE TABLE heritage_sites (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    place_label TEXT NOT NULL,
    place_qid TEXT,
    category TEXT NOT NULL CHECK (category IN (
        'residence', 'monument', 'museum', 'memorial', 'burial', 'other'
    )),
    lat DOUBLE PRECISION,
    lon DOUBLE PRECISION,
    geom GEOGRAPHY(POINT, 4326),
    visitability TEXT NOT NULL DEFAULT 'unknown' CHECK (visitability IN (
        'unknown', 'verified_open', 'closed', 'inaccessible'
    )),
    official_url TEXT,
    practical_json JSONB NOT NULL DEFAULT '{}',
    practical_verified_at TIMESTAMPTZ,
    provenance_json JSONB NOT NULL,
    content_hash TEXT NOT NULL UNIQUE,
    fetched_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_heritage_sites_geom ON heritage_sites USING GIST (geom);
CREATE INDEX idx_heritage_sites_qid ON heritage_sites (place_qid) WHERE place_qid IS NOT NULL;

CREATE TABLE entity_heritage_links (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    entity_id UUID NOT NULL REFERENCES entities(id) ON DELETE CASCADE,
    site_id UUID NOT NULL REFERENCES heritage_sites(id) ON DELETE CASCADE,
    relation TEXT NOT NULL CHECK (relation IN (
        'birthplace', 'former_residence', 'workplace', 'burial_site',
        'historical_event_site', 'memorial', 'collection_related', 'honoree'
    )),
    temporal_class TEXT NOT NULL CHECK (temporal_class IN ('lived', 'memory', 'tourist')),
    source_event_id UUID REFERENCES canonical_events(id) ON DELETE SET NULL,
    provenance_json JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (entity_id, site_id, relation)
);

CREATE INDEX idx_entity_heritage_links_entity ON entity_heritage_links (entity_id);

CREATE TABLE cultural_activities (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    entity_id UUID NOT NULL REFERENCES entities(id) ON DELETE CASCADE,
    site_id UUID REFERENCES heritage_sites(id) ON DELETE SET NULL,
    kind TEXT NOT NULL CHECK (kind IN (
        'exhibition', 'conference', 'reenactment', 'festival', 'guided_visit', 'ceremony'
    )),
    title TEXT NOT NULL,
    time_json JSONB NOT NULL,
    occurrence_status TEXT NOT NULL CHECK (occurrence_status IN ('past', 'scheduled', 'unknown')),
    provenance_json JSONB NOT NULL,
    fingerprint TEXT NOT NULL UNIQUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_cultural_activities_entity ON cultural_activities (entity_id);

CREATE TABLE commemorations (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    honoree_entity_id UUID NOT NULL REFERENCES entities(id) ON DELETE CASCADE,
    site_id UUID REFERENCES heritage_sites(id) ON DELETE SET NULL,
    title TEXT NOT NULL,
    time_json JSONB NOT NULL,
    occurrence_status TEXT NOT NULL CHECK (occurrence_status IN ('past', 'future_verified')),
    place_persistence TEXT NOT NULL DEFAULT 'unknown' CHECK (place_persistence IN (
        'unknown', 'extant'
    )),
    provenance_json JSONB NOT NULL,
    fingerprint TEXT NOT NULL UNIQUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_commemorations_honoree ON commemorations (honoree_entity_id);

CREATE TABLE visit_itineraries (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    entity_id UUID NOT NULL REFERENCES entities(id) ON DELETE CASCADE,
    mode TEXT NOT NULL CHECK (mode IN ('walk', 'bike', 'drive')),
    origin_lat DOUBLE PRECISION,
    origin_lon DOUBLE PRECISION,
    routing_status TEXT NOT NULL CHECK (routing_status IN ('unavailable', 'computed')),
    generation_version TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE visit_itinerary_stops (
    itinerary_id UUID NOT NULL REFERENCES visit_itineraries(id) ON DELETE CASCADE,
    ordinal INT NOT NULL CHECK (ordinal >= 0),
    site_id UUID NOT NULL REFERENCES heritage_sites(id) ON DELETE CASCADE,
    straight_line_km DOUBLE PRECISION,
    PRIMARY KEY (itinerary_id, ordinal)
);

COMMENT ON TABLE heritage_sites IS
'Patrimonial places for Visit. visitability stays unknown unless a practical source verified it.';
COMMENT ON TABLE commemorations IS
'Memory events about a person. They are not biographical deeds and must not enter the life timeline.';
