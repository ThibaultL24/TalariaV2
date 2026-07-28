-- migrations/002_place_geocodes.sql
CREATE TABLE place_geocodes (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    place_label TEXT NOT NULL,
    wiki_lang TEXT NOT NULL DEFAULT 'en',
    wikidata_qid TEXT,
    lat DOUBLE PRECISION,
    lon DOUBLE PRECISION,
    source TEXT NOT NULL DEFAULT 'wikidata',
    raw_json JSONB NOT NULL DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (wiki_lang, place_label)
);

CREATE INDEX idx_place_geocodes_label ON place_geocodes (wiki_lang, place_label);
