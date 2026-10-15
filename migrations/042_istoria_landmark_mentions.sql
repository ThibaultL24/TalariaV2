-- Istoria: a derived, replaceable projection over existing person events and
-- evidence sentences that occur verbatim in a locally stored Wikipedia lead.
-- No canonical event or historical evidence is modified.
CREATE TABLE entity_landmark_mentions (
    entity_id UUID NOT NULL REFERENCES entities(id) ON DELETE CASCADE,
    canonical_event_id UUID NOT NULL REFERENCES canonical_events(id) ON DELETE CASCADE,
    event_evidence_id UUID NOT NULL REFERENCES event_evidence(id) ON DELETE CASCADE,
    sentence_id UUID NOT NULL REFERENCES sentences(id) ON DELETE CASCADE,
    wiki_page_id UUID NOT NULL REFERENCES wiki_pages(id) ON DELETE CASCADE,
    wikipedia_revision_id BIGINT,
    algorithm_version SMALLINT NOT NULL DEFAULT 1 CHECK (algorithm_version > 0),
    importance_score SMALLINT NOT NULL CHECK (importance_score BETWEEN 0 AND 100),
    indexed_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (entity_id, canonical_event_id, event_evidence_id, algorithm_version)
);

CREATE INDEX idx_landmark_mentions_entity_score
    ON entity_landmark_mentions (entity_id, algorithm_version, importance_score DESC, canonical_event_id);
CREATE INDEX idx_landmark_mentions_event
    ON entity_landmark_mentions (canonical_event_id);
CREATE INDEX idx_landmark_mentions_wiki_page
    ON entity_landmark_mentions (wiki_page_id);
CREATE INDEX IF NOT EXISTS idx_wiki_sections_lead_page
    ON wiki_sections (wiki_page_id)
    WHERE ordinal = 0 AND title = 'Lead';
CREATE INDEX IF NOT EXISTS idx_event_evidence_event_sentence
    ON event_evidence (canonical_event_id, sentence_id);
