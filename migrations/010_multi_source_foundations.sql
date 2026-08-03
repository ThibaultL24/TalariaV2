-- 010_multi_source_foundations.sql
-- Lot A: discovery runs, discovered documents, quality claims, eligibility flags.
-- Additive and reversible. Does not touch legacy rows or reclassify them as quality.

CREATE TABLE IF NOT EXISTS source_discovery_runs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    subject_entity_id UUID NOT NULL REFERENCES entities(id) ON DELETE CASCADE,
    subject_qid TEXT,
    subject_label TEXT NOT NULL,
    plan_json JSONB NOT NULL DEFAULT '{}'::jsonb,
    budgets_json JSONB NOT NULL DEFAULT '{}'::jsonb,
    status TEXT NOT NULL DEFAULT 'pending'
        CHECK (status IN ('pending', 'running', 'completed', 'failed', 'cancelled')),
    metrics_json JSONB NOT NULL DEFAULT '{}'::jsonb,
    error_json JSONB,
    connector_versions JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    started_at TIMESTAMPTZ,
    finished_at TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS idx_source_discovery_runs_subject
    ON source_discovery_runs (subject_entity_id, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_source_discovery_runs_status
    ON source_discovery_runs (status);

CREATE TABLE IF NOT EXISTS discovered_documents (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    run_id UUID NOT NULL REFERENCES source_discovery_runs(id) ON DELETE CASCADE,
    source_kind TEXT NOT NULL,
    external_id TEXT NOT NULL,
    canonical_url TEXT,
    title TEXT NOT NULL,
    language TEXT,
    document_type TEXT NOT NULL DEFAULT 'article',
    discovery_method TEXT NOT NULL DEFAULT 'subject_search',
    relevance_score REAL NOT NULL DEFAULT 0
        CHECK (relevance_score >= 0 AND relevance_score <= 1),
    subject_links JSONB NOT NULL DEFAULT '[]'::jsonb,
    publication_time JSONB,
    source_metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
    fetch_status TEXT NOT NULL DEFAULT 'pending'
        CHECK (fetch_status IN ('pending', 'fetched', 'snapshotted', 'skipped', 'failed')),
    skip_reason TEXT,
    snapshot_id UUID REFERENCES document_snapshots(id) ON DELETE SET NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (run_id, source_kind, external_id)
);

CREATE INDEX IF NOT EXISTS idx_discovered_documents_source
    ON discovered_documents (source_kind, external_id);

CREATE INDEX IF NOT EXISTS idx_discovered_documents_run_status
    ON discovered_documents (run_id, fetch_status);

-- Cultural fact claims (distinct from Intuition opinion `claims` table).
CREATE TABLE IF NOT EXISTS quality_claims (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    subject_entity_id UUID NOT NULL REFERENCES entities(id) ON DELETE CASCADE,
    fingerprint TEXT NOT NULL,
    predicate TEXT NOT NULL,
    event_type TEXT NOT NULL,
    object_json JSONB NOT NULL DEFAULT '{}'::jsonb,
    time_json JSONB NOT NULL DEFAULT '{"kind":"unknown"}'::jsonb,
    place_entity_id UUID REFERENCES entities(id) ON DELETE SET NULL,
    place_label TEXT,
    status TEXT NOT NULL DEFAULT 'open'
        CHECK (status IN ('open', 'consolidated', 'conflict', 'needs_review', 'superseded')),
    support_count INT NOT NULL DEFAULT 1 CHECK (support_count >= 0),
    conflict_json JSONB NOT NULL DEFAULT '{}'::jsonb,
    canonical_event_id UUID REFERENCES canonical_events(id) ON DELETE SET NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (fingerprint)
);

CREATE INDEX IF NOT EXISTS idx_quality_claims_subject
    ON quality_claims (subject_entity_id);

CREATE INDEX IF NOT EXISTS idx_quality_claims_status
    ON quality_claims (status);

CREATE TABLE IF NOT EXISTS quality_claim_supports (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    claim_id UUID NOT NULL REFERENCES quality_claims(id) ON DELETE CASCADE,
    event_candidate_id UUID REFERENCES event_candidates(id) ON DELETE SET NULL,
    snapshot_id UUID REFERENCES document_snapshots(id) ON DELETE SET NULL,
    source_kind TEXT NOT NULL,
    evidence_ptr JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (claim_id, event_candidate_id)
);

CREATE INDEX IF NOT EXISTS idx_quality_claim_supports_claim
    ON quality_claim_supports (claim_id);

CREATE TABLE IF NOT EXISTS place_resolutions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    place_entity_id UUID REFERENCES entities(id) ON DELETE CASCADE,
    place_label TEXT NOT NULL,
    method TEXT NOT NULL,
    wikidata_qid TEXT,
    lat DOUBLE PRECISION,
    lon DOUBLE PRECISION,
    precision TEXT NOT NULL DEFAULT 'unknown'
        CHECK (precision IN ('exact', 'approximate', 'centroid', 'unknown')),
    uncertainty_radius_m DOUBLE PRECISION,
    candidates_json JSONB NOT NULL DEFAULT '[]'::jsonb,
    score REAL,
    raw_json JSONB NOT NULL DEFAULT '{}'::jsonb,
    resolved_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE UNIQUE INDEX IF NOT EXISTS uq_place_resolutions_label_method_qid
    ON place_resolutions (place_label, method, (COALESCE(wikidata_qid, '')));


-- Eligibility triad on quality events (legacy rows keep defaults, untouched).
ALTER TABLE canonical_events
    ADD COLUMN IF NOT EXISTS historically_valid BOOLEAN NOT NULL DEFAULT true,
    ADD COLUMN IF NOT EXISTS timeline_eligible BOOLEAN NOT NULL DEFAULT true,
    ADD COLUMN IF NOT EXISTS location_precision TEXT,
    ADD COLUMN IF NOT EXISTS uncertainty_radius_m DOUBLE PRECISION,
    ADD COLUMN IF NOT EXISTS source_count INT NOT NULL DEFAULT 1,
    ADD COLUMN IF NOT EXISTS evidence_count INT NOT NULL DEFAULT 0;

CREATE INDEX IF NOT EXISTS idx_canonical_events_subject_type_time
    ON canonical_events (entity_id, event_type, start_time)
    WHERE pipeline = 'quality' AND is_active;

CREATE INDEX IF NOT EXISTS idx_canonical_events_timeline_eligible
    ON canonical_events (entity_id)
    WHERE pipeline = 'quality' AND is_active AND timeline_eligible;

CREATE INDEX IF NOT EXISTS idx_canonical_events_map_eligible_quality
    ON canonical_events (entity_id)
    WHERE pipeline = 'quality' AND is_active AND map_eligible;
