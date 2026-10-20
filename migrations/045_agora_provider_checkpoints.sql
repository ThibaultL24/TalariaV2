-- Source-level recovery for the durable Agora job. These tables do not
-- change canonical events, claims, or original document snapshots.
CREATE TABLE ingest_provider_checkpoints (
    job_id uuid NOT NULL REFERENCES ingest_jobs(id) ON DELETE CASCADE,
    source_kind text NOT NULL CHECK (length(source_kind) BETWEEN 2 AND 64),
    connector_version text NOT NULL CHECK (length(connector_version) BETWEEN 1 AND 128),
    metrics_json jsonb NOT NULL CHECK (jsonb_typeof(metrics_json) = 'object'),
    completed_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (job_id, source_kind)
);

-- Stable provider/external IDs allow an interrupted provider to skip
-- individual documents already persisted successfully by a previous worker.
CREATE TABLE ingest_job_documents (
    job_id uuid NOT NULL REFERENCES ingest_jobs(id) ON DELETE CASCADE,
    source_kind text NOT NULL,
    external_id text NOT NULL,
    corpus_document_id uuid NOT NULL REFERENCES corpus_documents(id),
    created_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (job_id,source_kind,external_id)
);
CREATE INDEX idx_ingest_job_documents_corpus
    ON ingest_job_documents (job_id,corpus_document_id);

-- Backoff across job retries, not only after a process crash.
ALTER TABLE ingest_jobs
    ADD COLUMN next_retry_at timestamptz;
CREATE INDEX idx_ingest_jobs_retry_due
    ON ingest_jobs (next_retry_at,created_at)
    WHERE status='queued' AND next_retry_at IS NOT NULL;
