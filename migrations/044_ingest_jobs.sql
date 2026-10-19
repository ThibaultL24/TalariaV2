-- Persistent coordinator for the two HTTP ingestion lanes.
-- Append-only canonical events and source evidence are untouched.
CREATE TABLE ingest_jobs (
    id uuid PRIMARY KEY,
    lane text NOT NULL CHECK (lane IN ('explorer','agora')),
    subject text NOT NULL CHECK (length(btrim(subject)) BETWEEN 2 AND 240),
    qid text,
    identity_key text NOT NULL CHECK (length(identity_key) BETWEEN 3 AND 300),
    entity_id uuid REFERENCES entities(id) ON DELETE SET NULL,
    request_json jsonb NOT NULL CHECK (jsonb_typeof(request_json) = 'object'),
    progress_json jsonb NOT NULL DEFAULT '{"phase":"queued"}'::jsonb,
    report_json jsonb,
    error text,
    status text NOT NULL DEFAULT 'queued'
        CHECK (status IN ('queued','running','done','failed')),
    attempts integer NOT NULL DEFAULT 0 CHECK (attempts >= 0),
    owner_token uuid,
    lease_until timestamptz,
    created_at timestamptz NOT NULL DEFAULT now(),
    started_at timestamptz,
    finished_at timestamptz,
    updated_at timestamptz NOT NULL DEFAULT now(),
    CONSTRAINT ingest_jobs_lease_contract CHECK (
        (status='running' AND owner_token IS NOT NULL AND lease_until IS NOT NULL)
        OR (status <> 'running' AND owner_token IS NULL AND lease_until IS NULL)
    )
);
-- The partial unique index is the cross-replica admission gate. QID and
-- name-only requests have different keys; different QIDs never share a key.
CREATE UNIQUE INDEX ingest_jobs_active_identity
    ON ingest_jobs (lane,identity_key) WHERE status IN ('queued','running');
CREATE INDEX ingest_jobs_due
    ON ingest_jobs (status,lease_until,created_at);
CREATE INDEX ingest_jobs_entity_history
    ON ingest_jobs (entity_id,created_at DESC) WHERE entity_id IS NOT NULL;
