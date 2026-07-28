-- migrations/001_init.sql
CREATE EXTENSION IF NOT EXISTS pgcrypto;
CREATE EXTENSION IF NOT EXISTS postgis;

CREATE TABLE wiki_pages (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    page_id BIGINT,
    title TEXT NOT NULL,
    wiki_lang TEXT NOT NULL DEFAULT 'en',
    revision_id BIGINT,
    content_hash TEXT NOT NULL,
    dump_date DATE,
    raw_path TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (wiki_lang, title)
);

CREATE TABLE sentences (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    wiki_page_id UUID NOT NULL REFERENCES wiki_pages(id) ON DELETE CASCADE,
    ordinal INT NOT NULL,
    text TEXT NOT NULL,
    char_start INT,
    char_end INT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (wiki_page_id, ordinal)
);

CREATE TABLE entities (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    qid TEXT,
    wikipedia_title TEXT NOT NULL,
    wiki_lang TEXT NOT NULL DEFAULT 'en',
    canonical_name TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (wiki_lang, wikipedia_title)
);

CREATE TABLE phrase_candidates (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    sentence_id UUID NOT NULL REFERENCES sentences(id) ON DELETE CASCADE,
    entity_id UUID REFERENCES entities(id) ON DELETE SET NULL,
    person_surface TEXT NOT NULL,
    time_surface TEXT,
    place_surface TEXT,
    verb_pivot TEXT,
    combinator_hash TEXT NOT NULL,
    extractor TEXT NOT NULL DEFAULT 'cosmos:tuple_extraction',
    status TEXT NOT NULL DEFAULT 'pending',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (combinator_hash)
);

CREATE TABLE candidate_judgments (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    phrase_candidate_id UUID NOT NULL REFERENCES phrase_candidates(id) ON DELETE CASCADE,
    judge_kind TEXT NOT NULL,
    score DOUBLE PRECISION NOT NULL DEFAULT 0,
    label TEXT NOT NULL,
    result_json JSONB NOT NULL DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE canonical_events (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    entity_id UUID NOT NULL REFERENCES entities(id) ON DELETE CASCADE,
    event_type TEXT NOT NULL DEFAULT 'historical_fact',
    title TEXT NOT NULL,
    summary TEXT,
    start_time TIMESTAMPTZ,
    end_time TIMESTAMPTZ,
    time_json JSONB NOT NULL DEFAULT '{}',
    place_label TEXT,
    geom GEOGRAPHY(POINT, 4326),
    confidence DOUBLE PRECISION NOT NULL DEFAULT 0,
    map_eligible BOOLEAN NOT NULL DEFAULT false,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE event_evidence (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    canonical_event_id UUID NOT NULL REFERENCES canonical_events(id) ON DELETE CASCADE,
    sentence_id UUID NOT NULL REFERENCES sentences(id) ON DELETE CASCADE,
    phrase_candidate_id UUID REFERENCES phrase_candidates(id) ON DELETE SET NULL,
    quoted_text TEXT,
    evidence_type TEXT NOT NULL DEFAULT 'sentence_span',
    confidence DOUBLE PRECISION NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_sentences_wiki_page ON sentences(wiki_page_id);
CREATE INDEX idx_phrase_candidates_sentence ON phrase_candidates(sentence_id);
CREATE INDEX idx_phrase_candidates_status ON phrase_candidates(status);
CREATE INDEX idx_canonical_events_entity ON canonical_events(entity_id);
CREATE INDEX idx_canonical_events_geom ON canonical_events USING GIST(geom);

CREATE TABLE dump_runs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    dump_path TEXT NOT NULL,
    wiki_lang TEXT NOT NULL DEFAULT 'en',
    status TEXT NOT NULL DEFAULT 'pending',
    pages_indexed INT NOT NULL DEFAULT 0,
    started_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    ended_at TIMESTAMPTZ
);
