-- Tourism Wikidata sites are not biographical canonical_events.
-- Migration 041 applies only to databases already on the origin/main 033-040 sequence.
CREATE TABLE visit_sites (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    wikidata_qid TEXT NOT NULL UNIQUE CHECK (wikidata_qid ~ '^Q[1-9][0-9]*$'),
    place_entity_id UUID REFERENCES entities(id),
    label TEXT NOT NULL CHECK (length(btrim(label)) > 0),
    kind TEXT NOT NULL CHECK (kind IN ('building','museum','monument','memorial','burial_site','residence')),
    geom GEOGRAPHY(POINT, 4326) NOT NULL,
    publication_status TEXT NOT NULL DEFAULT 'published'
        CHECK (publication_status IN ('published','quarantine')),
    visitability TEXT NOT NULL DEFAULT 'unknown'
        CHECK (visitability IN ('unknown','verified_visitable','verified_closed','restricted')),
    osm_opening_hours TEXT,
    osm_object_type TEXT CHECK (osm_object_type IN ('node','way','relation')),
    osm_object_id BIGINT CHECK (osm_object_id > 0),
    osm_checked_at TIMESTAMPTZ,
    retrieved_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT visit_site_valid_lat_lon CHECK (
        ST_Y(geom::geometry) BETWEEN -90 AND 90
        AND ST_X(geom::geometry) BETWEEN -180 AND 180
    )
);

CREATE TABLE visit_person_sites (
    entity_id UUID NOT NULL REFERENCES entities(id) ON DELETE CASCADE,
    site_id UUID NOT NULL REFERENCES visit_sites(id) ON DELETE CASCADE,
    relation_pid TEXT NOT NULL CHECK (relation_pid IN ('P547','P138','P119','P551')),
    source_url TEXT NOT NULL CHECK (source_url LIKE 'https://www.wikidata.org/%'),
    retrieved_at TIMESTAMPTZ NOT NULL,
    wikidata_revision_id BIGINT,
    PRIMARY KEY (entity_id, site_id, relation_pid)
);

CREATE INDEX idx_visit_sites_published_geom
    ON visit_sites USING GIST (geom)
    WHERE publication_status = 'published';
CREATE INDEX idx_visit_person_sites_site ON visit_person_sites (site_id, entity_id);
CREATE INDEX idx_visit_person_sites_entity ON visit_person_sites (entity_id, relation_pid);
