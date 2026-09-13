-- migrations/029_evidence_precision_authority.sql
-- P0 Schema: evidence fragment_id, date_precision projection, coord_precision_level, authority bundle.
-- Additive and non-destructive. Does not modify existing data beyond backfill.

--------------------------------------------------------------------------------
-- 1. Evidence triplet: Add fragment_id to event_evidence
--    Full triplet becomes: source_locator + quoted_text + fragment_id
--    evidence_hash continues to deduplicate via (event_id, raw_document_id, quoted_text).
--------------------------------------------------------------------------------

ALTER TABLE event_evidence
    ADD COLUMN IF NOT EXISTS fragment_id UUID REFERENCES document_fragments(id) ON DELETE SET NULL;

-- Index for evidence by fragment (useful for fragment-level provenance queries)
CREATE INDEX IF NOT EXISTS idx_event_evidence_fragment
    ON event_evidence (fragment_id)
    WHERE fragment_id IS NOT NULL;

--------------------------------------------------------------------------------
-- 2. Date precision: Queryable projection of TypedTime kind×precision
--    Semantic source of truth remains time_json; date_precision is a derived column.
--    Values: 'day', 'month', 'year', 'range', 'approx', 'unknown'
--------------------------------------------------------------------------------

ALTER TABLE canonical_events
    ADD COLUMN IF NOT EXISTS date_precision TEXT;

-- Backfill date_precision from existing time_json where possible
UPDATE canonical_events
SET date_precision = CASE
    WHEN time_json->>'kind' = 'exact' THEN
        CASE
            WHEN (time_json->>'day') IS NOT NULL AND (time_json->>'day')::int > 0 THEN 'day'
            WHEN (time_json->>'month') IS NOT NULL AND (time_json->>'month')::int > 0 THEN 'month'
            ELSE 'year'
        END
    WHEN time_json->>'kind' = 'range' THEN 'range'
    WHEN time_json->>'kind' = 'approx' THEN 'approx'
    ELSE 'unknown'
END
WHERE date_precision IS NULL AND time_json IS NOT NULL AND time_json != '{}'::jsonb;

-- Index for filtering events by date precision
CREATE INDEX IF NOT EXISTS idx_canonical_events_date_precision
    ON canonical_events (date_precision)
    WHERE is_active AND date_precision IS NOT NULL;

--------------------------------------------------------------------------------
-- 3. Coordinate precision level: Queryable projection for map zoom filtering
--    Values: 'point', 'city', 'region', 'country', 'unknown'
--    Derived from location_precision + uncertainty_radius_m.
--------------------------------------------------------------------------------

ALTER TABLE canonical_events
    ADD COLUMN IF NOT EXISTS coord_precision_level TEXT
        CHECK (coord_precision_level IS NULL OR coord_precision_level IN (
            'point', 'city', 'region', 'country', 'unknown'
        ));

-- Backfill coord_precision_level from existing location_precision/uncertainty
UPDATE canonical_events
SET coord_precision_level = CASE
    WHEN geom IS NULL THEN NULL
    WHEN location_precision = 'exact' THEN
        CASE
            WHEN uncertainty_radius_m IS NULL OR uncertainty_radius_m < 1000 THEN 'point'
            WHEN uncertainty_radius_m < 10000 THEN 'city'
            WHEN uncertainty_radius_m < 100000 THEN 'region'
            ELSE 'country'
        END
    WHEN location_precision = 'approximate' THEN 'city'
    WHEN location_precision = 'centroid' THEN 'region'
    ELSE 'unknown'
END
WHERE coord_precision_level IS NULL AND geom IS NOT NULL;

-- Index for zoom-dependent filtering
CREATE INDEX IF NOT EXISTS idx_canonical_events_coord_precision
    ON canonical_events (coord_precision_level)
    WHERE is_active AND map_eligible AND coord_precision_level IS NOT NULL;

--------------------------------------------------------------------------------
-- 4. Authority bundle: Store authority identifiers on entities
--    P268 (BnF), P214 (VIAF), P213 (ISNI), P269 (IdRef)
--    Stored as JSONB for extensibility; indexed for lookup.
--------------------------------------------------------------------------------

ALTER TABLE entities
    ADD COLUMN IF NOT EXISTS authority_ids JSONB NOT NULL DEFAULT '{}'::jsonb;

-- Example authority_ids structure:
-- {
--   "bnf": "11887067q",
--   "viaf": "12345678",
--   "isni": "0000000121032683",
--   "idref": "026794586"
-- }

-- GIN index for authority ID queries (e.g., finding entity by VIAF)
CREATE INDEX IF NOT EXISTS idx_entities_authority_ids
    ON entities USING gin (authority_ids jsonb_path_ops);

--------------------------------------------------------------------------------
-- 5. Place identity grounding: Add place_identity_qid for grounded places
--    Separate from geocoding — identity resolution comes first.
--------------------------------------------------------------------------------

ALTER TABLE canonical_events
    ADD COLUMN IF NOT EXISTS place_identity_qid TEXT;

-- Index for place identity lookups
CREATE INDEX IF NOT EXISTS idx_canonical_events_place_identity
    ON canonical_events (place_identity_qid)
    WHERE place_identity_qid IS NOT NULL;

-- Add grounding method to track how place identity was resolved
ALTER TABLE place_resolutions
    ADD COLUMN IF NOT EXISTS identity_source TEXT;

-- Values: 'wikidata', 'tgn', 'whg', 'geonames', 'alias_gazetteer', 'manual'

COMMENT ON COLUMN canonical_events.date_precision IS 
    'Queryable projection of time_json kind×precision. Semantic source of truth is time_json.';

COMMENT ON COLUMN canonical_events.coord_precision_level IS 
    'Geographic precision for zoom filtering: point (<1km), city (<10km), region (<100km), country.';

COMMENT ON COLUMN canonical_events.place_identity_qid IS 
    'Wikidata QID for the resolved place identity, separate from geocoded coordinates.';

COMMENT ON COLUMN entities.authority_ids IS 
    'Authority identifiers bundle: bnf (P268), viaf (P214), isni (P213), idref (P269).';

COMMENT ON COLUMN event_evidence.fragment_id IS 
    'Document fragment providing this evidence. Full triplet: source_locator + quoted_text + fragment_id.';
