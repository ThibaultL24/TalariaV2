-- Sourced historical links and licensed-media candidates for Poreia.
-- These rows are not canonical_events. New rows start in quarantine.

ALTER TABLE visit_sites DROP CONSTRAINT IF EXISTS visit_sites_kind_check;
ALTER TABLE visit_sites
    ADD CONSTRAINT visit_sites_kind_check
    CHECK (kind IN (
        'building', 'museum', 'monument', 'memorial', 'burial_site', 'residence',
        'bridge', 'cultural_institution'
    ));

CREATE TABLE visit_site_narratives (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    site_id UUID NOT NULL REFERENCES visit_sites(id) ON DELETE CASCADE,
    subject_qid TEXT CHECK (subject_qid IS NULL OR subject_qid ~ '^Q[1-9][0-9]*$'),
    entity_id UUID REFERENCES entities(id) ON DELETE SET NULL,
    relation_role TEXT NOT NULL CHECK (relation_role IN (
        'burial', 'commemoration', 'named_after', 'residence', 'commissioned', 'thematic'
    )),
    relation_pid TEXT CHECK (relation_pid IS NULL OR relation_pid ~ '^P[1-9][0-9]*$'),
    summary TEXT NOT NULL CHECK (length(btrim(summary)) > 0),
    source_url TEXT NOT NULL CHECK (source_url LIKE 'https://%'),
    retrieved_at TIMESTAMPTZ NOT NULL,
    wikidata_revision_id BIGINT,
    publication_status TEXT NOT NULL DEFAULT 'quarantine'
        CHECK (publication_status IN ('published', 'quarantine', 'rejected')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT visit_site_narratives_source_once
        UNIQUE (site_id, subject_qid, relation_role, source_url)
);

CREATE INDEX idx_visit_site_narratives_site_status
    ON visit_site_narratives (site_id, publication_status);

CREATE TABLE visit_site_media (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    site_id UUID NOT NULL REFERENCES visit_sites(id) ON DELETE CASCADE,
    commons_title TEXT NOT NULL CHECK (length(btrim(commons_title)) > 0),
    author TEXT,
    license TEXT,
    attribution TEXT,
    source_url TEXT NOT NULL CHECK (source_url LIKE 'https://%'),
    publication_status TEXT NOT NULL DEFAULT 'quarantine'
        CHECK (publication_status IN ('published', 'quarantine', 'rejected')),
    retrieved_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (site_id, source_url)
);

CREATE INDEX idx_visit_site_media_site_status
    ON visit_site_media (site_id, publication_status);
