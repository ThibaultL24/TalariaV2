-- Read-only operational dashboard: bounded newest-job scans without touching
-- append-only historical events, their claims or provenance.
CREATE INDEX idx_ingest_jobs_admin_recent
    ON ingest_jobs (created_at DESC, id DESC);
CREATE INDEX idx_ingest_jobs_admin_filters
    ON ingest_jobs (lane, status, created_at DESC, id DESC);
