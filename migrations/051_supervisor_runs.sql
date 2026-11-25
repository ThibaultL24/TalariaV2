-- Supervision journal. Plans are append-only. They do not publish sites or edit canonical events.

CREATE TABLE supervisor_runs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    experience TEXT NOT NULL CHECK (experience IN ('istoria', 'poreia')),
    mode TEXT NOT NULL CHECK (mode IN ('read_only', 'execute')),
    subject_qid TEXT NOT NULL CHECK (subject_qid ~ '^Q[1-9][0-9]*$'),
    subject_label TEXT NOT NULL CHECK (length(btrim(subject_label)) > 0),
    provider TEXT NOT NULL,
    model TEXT NOT NULL,
    prompt_version TEXT NOT NULL,
    input_hash TEXT NOT NULL CHECK (length(input_hash) = 64),
    plan_json JSONB NOT NULL CHECK (jsonb_typeof(plan_json) = 'object'),
    status TEXT NOT NULL CHECK (status IN ('planned', 'completed', 'refused', 'failed')),
    token_input INTEGER NOT NULL DEFAULT 0 CHECK (token_input >= 0),
    token_output INTEGER NOT NULL DEFAULT 0 CHECK (token_output >= 0),
    cost_usd_micros INTEGER NOT NULL DEFAULT 0 CHECK (cost_usd_micros >= 0),
    error TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE UNIQUE INDEX idx_supervisor_runs_idempotent
    ON supervisor_runs (experience, prompt_version, input_hash);

CREATE INDEX idx_supervisor_runs_subject
    ON supervisor_runs (experience, subject_qid, created_at DESC);

CREATE TABLE supervisor_tasks (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    run_id UUID NOT NULL REFERENCES supervisor_runs(id) ON DELETE RESTRICT,
    task_key TEXT NOT NULL,
    kind TEXT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('queued', 'completed', 'refused', 'failed')),
    writes BOOLEAN NOT NULL DEFAULT FALSE CHECK (writes = FALSE),
    detail_json JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (run_id, task_key)
);

CREATE INDEX idx_supervisor_tasks_status ON supervisor_tasks (status, kind);

CREATE OR REPLACE FUNCTION supervisor_runs_append_only()
RETURNS trigger
LANGUAGE plpgsql
AS $$
BEGIN
    RAISE EXCEPTION 'supervisor_run_is_append_only';
END;
$$;

DROP TRIGGER IF EXISTS supervisor_runs_no_update ON supervisor_runs;
CREATE TRIGGER supervisor_runs_no_update
    BEFORE UPDATE OR DELETE ON supervisor_runs
    FOR EACH ROW
    EXECUTE FUNCTION supervisor_runs_append_only();

DROP TRIGGER IF EXISTS supervisor_tasks_no_update ON supervisor_tasks;
CREATE TRIGGER supervisor_tasks_no_update
    BEFORE UPDATE OR DELETE ON supervisor_tasks
    FOR EACH ROW
    EXECUTE FUNCTION supervisor_runs_append_only();
