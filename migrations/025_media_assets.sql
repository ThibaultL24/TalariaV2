-- migrations/025_media_assets.sql
-- Attributed Commons media records (thumbs + license; never auto-events).

CREATE TABLE IF NOT EXISTS media_assets (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    commons_file TEXT NOT NULL,
    mid TEXT,
    sha1 TEXT,
    mime TEXT,
    license TEXT,
    attribution_text TEXT NOT NULL,
    thumb_url TEXT,
    depicts_qids TEXT[] NOT NULL DEFAULT '{}',
    revision_id TEXT,
    rights_normalized TEXT NOT NULL DEFAULT 'unknown',
    entity_id UUID REFERENCES entities(id) ON DELETE SET NULL,
    corpus_document_id UUID REFERENCES corpus_documents(id) ON DELETE SET NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE NULLS NOT DISTINCT (commons_file, sha1)
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_media_assets_mid
    ON media_assets (mid)
    WHERE mid IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_media_assets_entity_id
    ON media_assets (entity_id)
    WHERE entity_id IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_media_assets_corpus_document_id
    ON media_assets (corpus_document_id)
    WHERE corpus_document_id IS NOT NULL;
