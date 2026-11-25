-- Execution journal for supervised documentary runs.
-- Status changes are events. A task identity cannot be rewritten, and a real insert must be allowed to record writes.

ALTER TABLE supervisor_runs
    ADD COLUMN duration_ms INTEGER NOT NULL DEFAULT 0 CHECK (duration_ms >= 0),
    ADD COLUMN cost_estimated BOOLEAN NOT NULL DEFAULT FALSE;

ALTER TABLE supervisor_tasks DROP CONSTRAINT IF EXISTS supervisor_tasks_writes_check;

DROP TRIGGER IF EXISTS supervisor_tasks_no_update ON supervisor_tasks;

CREATE OR REPLACE FUNCTION supervisor_tasks_keep_identity()
RETURNS trigger
LANGUAGE plpgsql
AS $$
BEGIN
    IF TG_OP = 'DELETE' THEN
        RAISE EXCEPTION 'supervisor_task_is_append_only';
    END IF;
    IF NEW.id IS DISTINCT FROM OLD.id
       OR NEW.run_id IS DISTINCT FROM OLD.run_id
       OR NEW.task_key IS DISTINCT FROM OLD.task_key
       OR NEW.kind IS DISTINCT FROM OLD.kind
       OR NEW.created_at IS DISTINCT FROM OLD.created_at THEN
        RAISE EXCEPTION 'supervisor_task_identity_is_frozen';
    END IF;
    RETURN NEW;
END;
$$;

DROP TRIGGER IF EXISTS supervisor_tasks_keep_identity ON supervisor_tasks;
CREATE TRIGGER supervisor_tasks_keep_identity
    BEFORE UPDATE OR DELETE ON supervisor_tasks
    FOR EACH ROW
    EXECUTE FUNCTION supervisor_tasks_keep_identity();

CREATE TABLE supervisor_task_events (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    task_id UUID NOT NULL REFERENCES supervisor_tasks(id) ON DELETE RESTRICT,
    status TEXT NOT NULL CHECK (status IN ('queued', 'running', 'completed', 'refused', 'failed')),
    writes BOOLEAN NOT NULL,
    detail_json JSONB NOT NULL CHECK (jsonb_typeof(detail_json) = 'object'),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_supervisor_task_events_task
    ON supervisor_task_events (task_id, created_at DESC);

CREATE OR REPLACE FUNCTION supervisor_task_events_append_only()
RETURNS trigger
LANGUAGE plpgsql
AS $$
BEGIN
    RAISE EXCEPTION 'supervisor_task_event_is_append_only';
END;
$$;

DROP TRIGGER IF EXISTS supervisor_task_events_no_change ON supervisor_task_events;
CREATE TRIGGER supervisor_task_events_no_change
    BEFORE UPDATE OR DELETE ON supervisor_task_events
    FOR EACH ROW
    EXECUTE FUNCTION supervisor_task_events_append_only();

CREATE INDEX IF NOT EXISTS idx_raw_documents_subject
    ON raw_documents (source_identifier)
    WHERE source_identifier IS NOT NULL;
