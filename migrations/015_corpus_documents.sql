-- 015_corpus_documents.sql
-- Provider-agnostic bibliographic corpus layer (PR1).
-- Additive: does not alter quality/legacy event pipelines or fragment kinds.

-- Durable bibliographic identity (one row per provider document identity).
CREATE TABLE IF NOT EXISTS corpus_documents (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    source_kind TEXT NOT NULL,
    external_id TEXT NOT NULL,
    canonical_url TEXT,
    document_type TEXT NOT NULL,
    title TEXT NOT NULL,
    language TEXT,
    abstract_text TEXT,
    academic_status TEXT NOT NULL DEFAULT 'unknown'
        CHECK (academic_status IN (
            'peer_reviewed',
            'doctoral_defended',
            'academic_unreviewed',
            'primary_source',
            'catalog_record',
            'unknown'
        )),
    access_level TEXT NOT NULL DEFAULT 'unknown'
        CHECK (access_level IN ('open', 'restricted', 'metadata_only', 'unknown')),
    full_text_available BOOLEAN NOT NULL DEFAULT false,
    rights_uri TEXT,
    rights_holder TEXT,
    rights_normalized TEXT NOT NULL DEFAULT 'unknown'
        CHECK (rights_normalized IN ('open', 'restricted', 'metadata_only', 'unknown')),
    publisher_or_institution TEXT,
    publication_time JSONB NOT NULL DEFAULT '{"kind":"unknown"}'::jsonb,
    connector_version TEXT NOT NULL DEFAULT '',
    retrieved_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (source_kind, external_id)
);

CREATE INDEX IF NOT EXISTS idx_corpus_documents_type
    ON corpus_documents (document_type);
CREATE INDEX IF NOT EXISTS idx_corpus_documents_academic_status
    ON corpus_documents (academic_status);
CREATE INDEX IF NOT EXISTS idx_corpus_documents_access
    ON corpus_documents (access_level, full_text_available);
CREATE INDEX IF NOT EXISTS idx_corpus_documents_language
    ON corpus_documents (language);
CREATE INDEX IF NOT EXISTS idx_corpus_documents_title
    ON corpus_documents (title);

-- Normalized identifiers (ISBN/DOI/ARK/PPN/NNT/…).
CREATE TABLE IF NOT EXISTS document_identifiers (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    corpus_document_id UUID NOT NULL REFERENCES corpus_documents(id) ON DELETE CASCADE,
    scheme TEXT NOT NULL,
    value_raw TEXT NOT NULL,
    value_normalized TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (corpus_document_id, scheme, value_normalized)
);

-- Global uniqueness when the scheme is a document-level authority id.
CREATE UNIQUE INDEX IF NOT EXISTS uq_document_identifiers_global_scheme_value
    ON document_identifiers (scheme, value_normalized)
    WHERE scheme IN ('nnt', 'doi', 'isbn13', 'isbn10', 'ark', 'hal_id', 'num_sujet', 'oclc', 'olid');

CREATE INDEX IF NOT EXISTS idx_document_identifiers_lookup
    ON document_identifiers (scheme, value_normalized);

-- Contributors / institutions with typed roles (provider-agnostic).
CREATE TABLE IF NOT EXISTS document_contributions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    corpus_document_id UUID NOT NULL REFERENCES corpus_documents(id) ON DELETE CASCADE,
    role TEXT NOT NULL
        CHECK (role IN (
            'author',
            'thesis_advisor',
            'jury_member',
            'jury_president',
            'rapporteur',
            'institution',
            'doctoral_school',
            'cotutelle_institution',
            'research_partner',
            'editor',
            'publisher',
            'other'
        )),
    agent_name TEXT NOT NULL,
    name_normalized TEXT NOT NULL,
    identifier_scheme TEXT,
    identifier_value TEXT,
    entity_id UUID REFERENCES entities(id) ON DELETE SET NULL,
    ordinal INT NOT NULL DEFAULT 0 CHECK (ordinal >= 0),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE UNIQUE INDEX IF NOT EXISTS uq_document_contributions_identity
    ON document_contributions (
        corpus_document_id,
        role,
        name_normalized,
        (COALESCE(identifier_scheme, '')),
        (COALESCE(identifier_value, '')),
        ordinal
    );

CREATE INDEX IF NOT EXISTS idx_document_contributions_doc
    ON document_contributions (corpus_document_id, role);
CREATE INDEX IF NOT EXISTS idx_document_contributions_idref
    ON document_contributions (identifier_scheme, identifier_value)
    WHERE identifier_value IS NOT NULL;

-- Controlled / free subjects (RAMEAU, keywords, …).
CREATE TABLE IF NOT EXISTS document_subjects (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    corpus_document_id UUID NOT NULL REFERENCES corpus_documents(id) ON DELETE CASCADE,
    scheme TEXT NOT NULL DEFAULT 'keyword',
    label TEXT NOT NULL,
    identifier TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE UNIQUE INDEX IF NOT EXISTS uq_document_subjects_identity
    ON document_subjects (
        corpus_document_id,
        scheme,
        (COALESCE(identifier, '')),
        label
    );

CREATE INDEX IF NOT EXISTS idx_document_subjects_doc
    ON document_subjects (corpus_document_id);
CREATE INDEX IF NOT EXISTS idx_document_subjects_rameau
    ON document_subjects (scheme, identifier)
    WHERE identifier IS NOT NULL;

-- Immutable content versions linked to bibliographic identity.
CREATE TABLE IF NOT EXISTS corpus_document_snapshots (
    corpus_document_id UUID NOT NULL REFERENCES corpus_documents(id) ON DELETE CASCADE,
    snapshot_id UUID NOT NULL REFERENCES document_snapshots(id) ON DELETE CASCADE,
    revision_token TEXT,
    content_hash TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (corpus_document_id, snapshot_id)
);

CREATE INDEX IF NOT EXISTS idx_corpus_document_snapshots_hash
    ON corpus_document_snapshots (corpus_document_id, content_hash);

-- Explicable entity↔document matching (never opaque identity merge).
CREATE TABLE IF NOT EXISTS entity_document_links (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    entity_id UUID NOT NULL REFERENCES entities(id) ON DELETE CASCADE,
    corpus_document_id UUID NOT NULL REFERENCES corpus_documents(id) ON DELETE CASCADE,
    relation TEXT NOT NULL
        CHECK (relation IN ('about', 'by', 'mentioned_in')),
    match_version TEXT NOT NULL,
    score REAL NOT NULL CHECK (score >= 0 AND score <= 1),
    components_json JSONB NOT NULL DEFAULT '{}'::jsonb,
    evidence_summary TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (entity_id, corpus_document_id, relation, match_version)
);

CREATE INDEX IF NOT EXISTS idx_entity_document_links_entity
    ON entity_document_links (entity_id, relation, score DESC);
CREATE INDEX IF NOT EXISTS idx_entity_document_links_doc
    ON entity_document_links (corpus_document_id);

-- Tie a discovery-run row to the durable corpus document when persisted.
ALTER TABLE discovered_documents
    ADD COLUMN IF NOT EXISTS corpus_document_id UUID
        REFERENCES corpus_documents(id) ON DELETE SET NULL;

CREATE INDEX IF NOT EXISTS idx_discovered_documents_corpus
    ON discovered_documents (corpus_document_id)
    WHERE corpus_document_id IS NOT NULL;
