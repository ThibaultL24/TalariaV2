-- 007_entity_kinds.sql
-- Typed entity kinds + surface aliases for mention resolution (additive).

ALTER TABLE entities
    ADD COLUMN IF NOT EXISTS kind TEXT NOT NULL DEFAULT 'unknown';

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint WHERE conname = 'entities_kind_check'
    ) THEN
        ALTER TABLE entities
            ADD CONSTRAINT entities_kind_check
            CHECK (kind IN ('person', 'place', 'object', 'organization', 'unknown'));
    END IF;
END $$;

CREATE TABLE IF NOT EXISTS entity_aliases (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    entity_id UUID NOT NULL REFERENCES entities(id) ON DELETE CASCADE,
    surface TEXT NOT NULL,
    language TEXT NOT NULL DEFAULT 'en',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (language, surface, entity_id)
);

CREATE INDEX IF NOT EXISTS idx_entity_aliases_lookup
    ON entity_aliases (language, lower(surface));
