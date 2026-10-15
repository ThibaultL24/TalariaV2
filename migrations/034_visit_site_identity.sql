-- Visit sites stay unpublished until a source identifies the place itself.
ALTER TABLE heritage_sites
    ADD COLUMN identity_grain TEXT NOT NULL DEFAULT 'withheld_unknown',
    ADD COLUMN publication_status TEXT NOT NULL DEFAULT 'withheld',
    ADD COLUMN external_ids JSONB NOT NULL DEFAULT '{}',
    ADD COLUMN summary TEXT,
    ADD COLUMN media_asset_id UUID REFERENCES media_assets(id) ON DELETE SET NULL;

ALTER TABLE heritage_sites DROP CONSTRAINT IF EXISTS heritage_sites_identity_grain_check;
ALTER TABLE heritage_sites
    ADD CONSTRAINT heritage_sites_identity_grain_check
    CHECK (identity_grain IN ('site', 'settlement', 'region', 'withheld_unknown'));

ALTER TABLE heritage_sites DROP CONSTRAINT IF EXISTS heritage_sites_publication_status_check;
ALTER TABLE heritage_sites
    ADD CONSTRAINT heritage_sites_publication_status_check
    CHECK (publication_status IN ('published', 'withheld'));

-- Rows created by the biographical projection have no site identity yet.
UPDATE heritage_sites
SET identity_grain = 'withheld_unknown',
    publication_status = 'withheld'
WHERE place_qid IS NULL;

CREATE UNIQUE INDEX IF NOT EXISTS idx_heritage_sites_place_qid
    ON heritage_sites (place_qid)
    WHERE place_qid IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_heritage_sites_published_geom
    ON heritage_sites USING GIST (geom)
    WHERE publication_status = 'published' AND identity_grain = 'site';

COMMENT ON COLUMN heritage_sites.identity_grain IS
'site = identified heritage place. settlement/region must not be published as monuments.';
COMMENT ON COLUMN heritage_sites.publication_status IS
'published sites appear on Visit. withheld rows stay stored but are not listed.';
