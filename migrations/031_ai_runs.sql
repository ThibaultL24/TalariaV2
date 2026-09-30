-- Derived AI output is separate from immutable historical occurrences.
CREATE TABLE ai_runs (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    purpose text NOT NULL CHECK (purpose IN ('person_extraction')),
    provider text NOT NULL,
    model text NOT NULL,
    prompt_version text NOT NULL,
    input_hash text NOT NULL CHECK (length(input_hash) = 64),
    raw_document_id uuid NOT NULL REFERENCES raw_documents(id),
    output_json jsonb NOT NULL CHECK (jsonb_typeof(output_json) = 'object'),
    latency_ms bigint NOT NULL CHECK (latency_ms >= 0),
    created_at timestamptz NOT NULL DEFAULT now(),
    UNIQUE (purpose, provider, model, prompt_version, input_hash)
);
CREATE INDEX idx_ai_runs_document ON ai_runs(raw_document_id, created_at);
-- Cursor ordering uses an explicit NULL bucket, then timestamp and UUID.
CREATE INDEX idx_person_entity_timeline_v3 ON canonical_events
    (entity_id, (start_time IS NULL), start_time, id)
    WHERE is_active AND timeline_eligible AND pipeline = 'person';
CREATE INDEX idx_person_entity_type_time_v3 ON canonical_events
    (entity_id, event_type, start_time, id)
    WHERE is_active AND timeline_eligible AND pipeline = 'person';
