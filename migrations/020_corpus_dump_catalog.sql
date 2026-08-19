-- 020_corpus_dump_catalog.sql
-- Generic dump ingest catalog (JSONL/etc. → snapshots/fragments).
-- Additive: dump_runs (Wikipedia extract-pages) is unchanged.

CREATE TABLE IF NOT EXISTS corpus_dump_runs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    source_kind TEXT NOT NULL,
    dump_uri TEXT NOT NULL,
    content_hash TEXT NOT NULL,
    reader_id TEXT NOT NULL,
    reader_version TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'planned'
        CHECK (status IN ('planned', 'running', 'completed', 'failed')),
    cursor_json JSONB NOT NULL DEFAULT '{}'::jsonb,
    metrics_json JSONB NOT NULL DEFAULT '{}'::jsonb,
    error TEXT,
    started_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    ended_at TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS idx_corpus_dump_runs_uri_hash
    ON corpus_dump_runs (dump_uri, content_hash);

CREATE INDEX IF NOT EXISTS idx_corpus_dump_runs_started
    ON corpus_dump_runs (started_at DESC);

CREATE TABLE IF NOT EXISTS corpus_dump_documents (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    run_id UUID NOT NULL REFERENCES corpus_dump_runs(id) ON DELETE CASCADE,
    external_id TEXT NOT NULL,
    snapshot_id UUID REFERENCES document_snapshots(id) ON DELETE SET NULL,
    content_hash TEXT,
    status TEXT NOT NULL DEFAULT 'planned'
        CHECK (status IN ('planned', 'ingested', 'skipped_unchanged', 'failed', 'filtered')),
    error TEXT,
    byte_offset BIGINT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (run_id, external_id)
);

CREATE INDEX IF NOT EXISTS idx_corpus_dump_documents_run_status
    ON corpus_dump_documents (run_id, status);
