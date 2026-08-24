-- migrations/023_wiki_fragment_metadata.sql
-- Allow section/infobox fragment kinds and store per-fragment JSON metadata.

ALTER TABLE document_fragments DROP CONSTRAINT IF EXISTS document_fragments_fragment_kind_check;
ALTER TABLE document_fragments ADD CONSTRAINT document_fragments_fragment_kind_check
    CHECK (fragment_kind IN ('sentence', 'clause', 'section', 'infobox'));

-- 006's clause_index CHECK was table-level and unnamed (Postgres typically names it
-- document_fragments_check1). Drop by definition, then recreate with a stable name.
DO $$
DECLARE
    r RECORD;
BEGIN
    FOR r IN
        SELECT c.conname
        FROM pg_constraint c
        WHERE c.conrelid = 'document_fragments'::regclass
          AND c.contype = 'c'
          AND pg_get_constraintdef(c.oid) LIKE '%clause_index%'
    LOOP
        EXECUTE format('ALTER TABLE document_fragments DROP CONSTRAINT IF EXISTS %I', r.conname);
    END LOOP;
END $$;

ALTER TABLE document_fragments ADD CONSTRAINT document_fragments_clause_check
    CHECK (
        (fragment_kind = 'clause' AND clause_index IS NOT NULL)
        OR (fragment_kind IN ('sentence', 'section', 'infobox') AND clause_index IS NULL)
    );

ALTER TABLE document_fragments
    ADD COLUMN IF NOT EXISTS metadata JSONB NOT NULL DEFAULT '{}'::jsonb;
