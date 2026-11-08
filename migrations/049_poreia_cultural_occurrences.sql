-- Contemporary cultural occurrences. Distinct from canonical_events and from
-- person-scoped visit_opportunities. A day is a date, not a clock time.

CREATE TABLE poreia_occurrences (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    site_id UUID REFERENCES visit_sites(id) ON DELETE SET NULL,
    kind TEXT NOT NULL CHECK (kind IN ('exhibition', 'performance', 'conference', 'other')),
    title TEXT NOT NULL CHECK (length(btrim(title)) > 0),
    summary TEXT,
    venue_label TEXT,
    geom GEOGRAPHY(POINT, 4326),
    occurs_on DATE,
    ends_on DATE,
    time_precision TEXT NOT NULL DEFAULT 'day'
        CHECK (time_precision IN ('day', 'month', 'year')),
    source_url TEXT NOT NULL CHECK (source_url LIKE 'https://%'),
    source_record_id TEXT NOT NULL CHECK (length(btrim(source_record_id)) > 0),
    publication_status TEXT NOT NULL DEFAULT 'quarantine'
        CHECK (publication_status IN ('published', 'quarantine', 'rejected')),
    fetched_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT poreia_occurrences_source_once UNIQUE (source_url, source_record_id),
    CONSTRAINT poreia_occurrences_date_order CHECK (
        occurs_on IS NULL OR ends_on IS NULL OR ends_on >= occurs_on
    ),
    CONSTRAINT poreia_occurrence_valid_lat_lon CHECK (
        geom IS NULL OR (
            ST_Y(geom::geometry) BETWEEN -90 AND 90
            AND ST_X(geom::geometry) BETWEEN -180 AND 180
        )
    )
);

CREATE INDEX idx_poreia_occurrences_published_geom
    ON poreia_occurrences USING GIST (geom)
    WHERE publication_status = 'published' AND geom IS NOT NULL;

CREATE INDEX idx_poreia_occurrences_published_dates
    ON poreia_occurrences (publication_status, occurs_on, ends_on);
