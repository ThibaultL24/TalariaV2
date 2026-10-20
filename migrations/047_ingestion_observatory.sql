-- Authorization extends existing SIWE sessions. No user is promoted implicitly.
CREATE TABLE administration_users (
    user_id uuid PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    granted_at timestamptz NOT NULL DEFAULT now()
);
-- Period cohort and stable keyset-compatible ordering for administrative reads.
CREATE INDEX ingest_jobs_created_id ON ingest_jobs (created_at DESC, id DESC);
