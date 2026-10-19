-- Bounded, typed per-attempt telemetry. This is an append-only audit of
-- provider execution; it does not store extracted texts or alter historical data.
CREATE TABLE ingest_provider_attempt_metrics (
    job_id uuid NOT NULL REFERENCES ingest_jobs(id) ON DELETE CASCADE,
    source_kind text NOT NULL CHECK (length(source_kind) BETWEEN 2 AND 64),
    attempt integer NOT NULL CHECK (attempt BETWEEN 1 AND 3),
    status text NOT NULL DEFAULT 'running' CHECK (
        status IN ('running','healthy','partial','failed','rate_limited','not_configured')
    ),
    error_kind text CHECK (
        error_kind IN ('timeout','rate_limit','cursor_stall','upstream','none')
    ),
    discover_calls bigint NOT NULL DEFAULT 0 CHECK (discover_calls >= 0),
    fetch_calls bigint NOT NULL DEFAULT 0 CHECK (fetch_calls >= 0),
    discovered bigint NOT NULL DEFAULT 0 CHECK (discovered >= 0),
    persisted bigint NOT NULL DEFAULT 0 CHECK (persisted >= 0),
    skipped bigint NOT NULL DEFAULT 0 CHECK (skipped >= 0),
    matched bigint NOT NULL DEFAULT 0 CHECK (matched >= 0),
    timeouts bigint NOT NULL DEFAULT 0 CHECK (timeouts >= 0),
    errors bigint NOT NULL DEFAULT 0 CHECK (errors >= 0),
    elapsed_ms bigint NOT NULL DEFAULT 0 CHECK (elapsed_ms >= 0),
    started_at timestamptz NOT NULL DEFAULT now(),
    finished_at timestamptz,
    PRIMARY KEY (job_id,source_kind,attempt),
    CONSTRAINT ingest_provider_attempt_terminal CHECK (
        (status='running' AND finished_at IS NULL)
        OR (status <> 'running' AND finished_at IS NOT NULL)
    )
);
-- The PK supports all per-job diagnostics; avoid redundant write-heavy indexes.
