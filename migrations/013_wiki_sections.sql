-- migrations/013_wiki_sections.sql
-- Local Wikipedia sections for offline narrative dossiers.

CREATE TABLE IF NOT EXISTS wiki_sections (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    wiki_page_id UUID NOT NULL REFERENCES wiki_pages(id) ON DELETE CASCADE,
    ordinal INT NOT NULL,
    title TEXT NOT NULL,
    text TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (wiki_page_id, ordinal)
);

CREATE INDEX IF NOT EXISTS idx_wiki_sections_page ON wiki_sections (wiki_page_id);
CREATE INDEX IF NOT EXISTS idx_wiki_sections_title ON wiki_sections (title);
