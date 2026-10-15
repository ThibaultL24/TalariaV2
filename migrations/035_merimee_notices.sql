-- Local Mérimée notices (data.gouv.fr CSV). Not published as Visit sites until discover reconciles.

CREATE TABLE merimee_import_runs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    source_uri TEXT NOT NULL,
    file_checksum TEXT,
    started_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    finished_at TIMESTAMPTZ,
    status TEXT NOT NULL DEFAULT 'running' CHECK (status IN ('running', 'completed', 'failed', 'partial')),
    rows_seen BIGINT NOT NULL DEFAULT 0,
    rows_upserted BIGINT NOT NULL DEFAULT 0,
    rows_rejected BIGINT NOT NULL DEFAULT 0,
    rows_unchanged BIGINT NOT NULL DEFAULT 0,
    last_row_number BIGINT NOT NULL DEFAULT 0,
    error_summary TEXT
);

CREATE TABLE merimee_notices (
    reference TEXT PRIMARY KEY,
    title TEXT NOT NULL,
    commune TEXT,
    department TEXT,
    region TEXT,
    protection_kind TEXT,
    authors TEXT,
    historique TEXT,
    description TEXT,
    personalities_raw TEXT,
    external_links_raw TEXT,
    place_wikidata_qid TEXT,
    lat DOUBLE PRECISION,
    lon DOUBLE PRECISION,
    geom GEOGRAPHY(POINT, 4326),
    heritage_site_id UUID REFERENCES heritage_sites(id) ON DELETE SET NULL,
    content_hash TEXT NOT NULL,
    source_dataset TEXT NOT NULL DEFAULT 'data.gouv.fr/immeubles-proteges-MH',
    source_resource_version TEXT,
    provenance_json JSONB NOT NULL DEFAULT '{}',
    first_imported_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_merimee_notices_geom ON merimee_notices USING GIST (geom);
CREATE INDEX idx_merimee_notices_wikidata ON merimee_notices (place_wikidata_qid)
    WHERE place_wikidata_qid IS NOT NULL;
CREATE INDEX idx_merimee_notices_heritage ON merimee_notices (heritage_site_id)
    WHERE heritage_site_id IS NOT NULL;
CREATE INDEX idx_merimee_notices_content_hash ON merimee_notices (content_hash);

CREATE TABLE merimee_notice_persons (
    notice_reference TEXT NOT NULL REFERENCES merimee_notices(reference) ON DELETE CASCADE,
    person_name_raw TEXT NOT NULL,
    person_name_normalized TEXT NOT NULL,
    role_label TEXT,
    PRIMARY KEY (notice_reference, person_name_raw)
);

CREATE INDEX idx_merimee_notice_persons_norm ON merimee_notice_persons (person_name_normalized);
CREATE INDEX idx_merimee_notice_persons_notice ON merimee_notice_persons (notice_reference);

CREATE TABLE merimee_entity_matches (
    entity_id UUID NOT NULL REFERENCES entities(id) ON DELETE CASCADE,
    notice_reference TEXT NOT NULL REFERENCES merimee_notices(reference) ON DELETE CASCADE,
    match_kind TEXT NOT NULL CHECK (match_kind IN ('wikidata_qid', 'person_field', 'manual')),
    confidence TEXT NOT NULL CHECK (confidence IN ('high', 'medium', 'low')),
    provenance_json JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (entity_id, notice_reference, match_kind)
);

CREATE INDEX idx_merimee_entity_matches_entity ON merimee_entity_matches (entity_id);
CREATE INDEX idx_merimee_entity_matches_notice ON merimee_entity_matches (notice_reference);

COMMENT ON TABLE merimee_notices IS
'Official MH notices keyed by Mérimée reference (PA…). Import does not publish Visit sites.';
COMMENT ON TABLE merimee_notice_persons IS
'Structured person links from Personnes_liees_a_l_edifice only — not parsed from free description.';
