-- 011_lot_e_exploration_and_places.sql
-- Exploration queue, place aliases, occurrence keys, density targets on runs.

CREATE TABLE IF NOT EXISTS exploration_targets (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    subject_entity_id UUID NOT NULL REFERENCES entities(id) ON DELETE CASCADE,
    run_id UUID REFERENCES source_discovery_runs(id) ON DELETE SET NULL,
    target_kind TEXT NOT NULL,
    source_kind TEXT NOT NULL DEFAULT 'wikipedia',
    external_id TEXT NOT NULL,
    canonical_url TEXT,
    title TEXT NOT NULL,
    language TEXT,
    relationship_kind TEXT NOT NULL DEFAULT 'linked_from_subject',
    discovered_from UUID REFERENCES exploration_targets(id) ON DELETE SET NULL,
    depth INT NOT NULL DEFAULT 0 CHECK (depth >= 0),
    priority INT NOT NULL DEFAULT 100,
    status TEXT NOT NULL DEFAULT 'pending'
        CHECK (status IN ('pending', 'queued', 'fetching', 'done', 'failed', 'skipped')),
    attempts INT NOT NULL DEFAULT 0,
    next_retry_at TIMESTAMPTZ,
    fingerprint TEXT NOT NULL,
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (subject_entity_id, fingerprint)
);

CREATE INDEX IF NOT EXISTS idx_exploration_targets_queue
    ON exploration_targets (status, priority ASC, next_retry_at NULLS FIRST)
    WHERE status IN ('pending', 'queued');

CREATE INDEX IF NOT EXISTS idx_exploration_targets_subject_depth
    ON exploration_targets (subject_entity_id, depth);

CREATE TABLE IF NOT EXISTS place_aliases (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    place_entity_id UUID REFERENCES entities(id) ON DELETE CASCADE,
    alias TEXT NOT NULL,
    language TEXT NOT NULL DEFAULT 'en',
    valid_from_year INT,
    valid_to_year INT,
    wikidata_qid TEXT,
    lat DOUBLE PRECISION,
    lon DOUBLE PRECISION,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE UNIQUE INDEX IF NOT EXISTS uq_place_aliases_lang_alias
    ON place_aliases (language, lower(alias));

CREATE INDEX IF NOT EXISTS idx_place_aliases_qid
    ON place_aliases (wikidata_qid)
    WHERE wikidata_qid IS NOT NULL;

ALTER TABLE event_candidates
    ADD COLUMN IF NOT EXISTS occurrence_key TEXT,
    ADD COLUMN IF NOT EXISTS primary_object TEXT,
    ADD COLUMN IF NOT EXISTS action_role TEXT;

CREATE INDEX IF NOT EXISTS idx_event_candidates_occurrence
    ON event_candidates (occurrence_key)
    WHERE occurrence_key IS NOT NULL;

ALTER TABLE canonical_events
    ADD COLUMN IF NOT EXISTS occurrence_key TEXT,
    ADD COLUMN IF NOT EXISTS primary_object TEXT;

CREATE UNIQUE INDEX IF NOT EXISTS uq_canonical_active_occurrence
    ON canonical_events (entity_id, occurrence_key)
    WHERE is_active AND pipeline = 'quality' AND occurrence_key IS NOT NULL;

ALTER TABLE source_discovery_runs
    ADD COLUMN IF NOT EXISTS density_targets JSONB NOT NULL DEFAULT '{}'::jsonb;

-- Seed historical place aliases (generic gazetteer — not Napoleon-hardcoded rules).
INSERT INTO place_aliases (alias, language, wikidata_qid, lat, lon)
SELECT v.alias, v.language, v.qid, v.lat, v.lon
FROM (VALUES
    ('Ajaccio', 'en', 'Q40104', 41.9267, 8.7369),
    ('Paris', 'en', 'Q90', 48.8566, 2.3522),
    ('Waterloo', 'en', 'Q179251', 50.6794, 4.4047),
    ('Austerlitz', 'en', 'Q163055', 49.1533, 16.875),
    ('Leipzig', 'en', 'Q2079', 51.3397, 12.3731),
    ('Borodino', 'en', 'Q893775', 55.526, 35.821),
    ('Moscow', 'en', 'Q649', 55.7558, 37.6173),
    ('Elba', 'en', 'Q204949', 42.777, 10.192),
    ('Saint Helena', 'en', 'Q34497', -15.965, -5.712),
    ('St Helena', 'en', 'Q34497', -15.965, -5.712),
    ('Sainte-Hélène', 'fr', 'Q34497', -15.965, -5.712),
    ('Fontainebleau', 'en', 'Q182871', 48.4047, 2.7016),
    ('Malmaison', 'en', 'Q667099', 48.8706, 2.1681),
    ('Toulon', 'en', 'Q1567', 43.1242, 5.928),
    ('Brienne', 'en', 'Q83202', 48.3933, 4.5228),
    ('Brienne-le-Château', 'fr', 'Q83202', 48.3933, 4.5228),
    ('Marengo', 'en', 'Q1026462', 44.888, 8.679),
    ('Jena', 'en', 'Q3150', 50.9272, 11.586),
    ('Wagram', 'en', 'Q489249', 48.25, 16.5667),
    ('Friedland', 'en', 'Q487447', 54.443, 21.011),
    ('Tilsit', 'en', 'Q189439', 55.0833, 21.8833),
    ('Cairo', 'en', 'Q85', 30.0444, 31.2357),
    ('Egypt', 'en', 'Q79', 30.0444, 31.2357),
    ('Vienna', 'en', 'Q1741', 48.2082, 16.3738),
    ('Schönbrunn', 'en', 'Q131313', 48.1845, 16.3122),
    ('Madrid', 'en', 'Q2807', 40.4168, -3.7038),
    ('Lisbon', 'en', 'Q597', 38.7223, -9.1393),
    ('Berlin', 'en', 'Q64', 52.52, 13.405),
    ('Milan', 'en', 'Q490', 45.4642, 9.19),
    ('Rome', 'en', 'Q220', 41.9028, 12.4964),
    ('Boulogne', 'en', 'Q81924', 50.7264, 1.6147),
    ('Boulogne-sur-Mer', 'fr', 'Q81924', 50.7264, 1.6147),
    ('Cannes', 'en', 'Q39984', 43.5528, 7.0174),
    ('Grenoble', 'en', 'Q1289', 45.1885, 5.7245),
    ('Lyon', 'en', 'Q456', 45.764, 4.8357),
    ('Auxerre', 'en', 'Q167600', 47.7982, 3.5733),
    ('Valence', 'en', 'Q8848', 44.9334, 4.8924),
    ('Corsica', 'en', 'Q14112', 42.0396, 9.0129),
    ('Ulm', 'en', 'Q3012', 48.4011, 9.9876),
    ('Eylau', 'en', 'Q488797', 54.4, 20.6333),
    ('Aspern', 'en', 'Q694408', 48.2167, 16.4667),
    ('Essling', 'en', 'Q694408', 48.2167, 16.4667),
    ('Smolensk', 'en', 'Q23313', 54.7826, 32.0853),
    ('Dresden', 'en', 'Q1731', 51.0504, 13.7373),
    ('Lützen', 'en', 'Q10784', 51.2583, 12.1417),
    ('Bautzen', 'en', 'Q14872', 51.1803, 14.4347),
    ('Ligny', 'en', 'Q696153', 50.512, 4.574),
    ('Quatre Bras', 'en', 'Q843223', 50.571, 4.638),
    ('Arcole', 'en', 'Q480123', 45.358, 11.278),
    ('Rivoli', 'en', 'Q46939', 45.571, 10.837),
    ('Lodi', 'en', 'Q6244', 45.314, 9.503),
    ('Mantua', 'en', 'Q9014', 45.1564, 10.7914),
    ('Acre', 'en', 'Q126084', 32.926, 35.083),
    ('Aboukir', 'en', 'Q308691', 31.3167, 30.0667),
    ('Notre-Dame', 'en', 'Q2981', 48.853, 2.3499),
    ('Amiens', 'en', 'Q41604', 49.8941, 2.2958),
    ('Erfurt', 'en', 'Q1729', 50.9787, 11.0328),
    ('Bayonne', 'en', 'Q134674', 43.4929, -1.4748),
    ('Vitoria', 'en', 'Q14336', 42.8499, -2.6729),
    ('Trafalgar', 'en', 'Q17259', 36.183, -6.0),
    ('Plymouth', 'en', 'Q43382', 50.3755, -4.1427),
    ('Rochefort', 'en', 'Q206933', 45.942, -0.9588)
) AS v(alias, language, qid, lat, lon)
WHERE NOT EXISTS (
    SELECT 1 FROM place_aliases p
    WHERE p.language = v.language AND lower(p.alias) = lower(v.alias)
);
