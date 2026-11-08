-- Human publication is an append-only review. Import scripts cannot flip status.

ALTER TABLE visit_sites DROP CONSTRAINT IF EXISTS visit_sites_publication_status_check;
ALTER TABLE visit_sites
    ADD CONSTRAINT visit_sites_publication_status_check
    CHECK (publication_status IN ('published', 'quarantine', 'rejected'));

CREATE OR REPLACE FUNCTION poreia_guard_site_publication()
RETURNS trigger
LANGUAGE plpgsql
AS $$
BEGIN
    IF NEW.publication_status IS DISTINCT FROM OLD.publication_status
       AND current_setting('talaria.poreia_review', true) IS DISTINCT FROM 'on' THEN
        RAISE EXCEPTION 'poreia_publication_requires_review';
    END IF;
    RETURN NEW;
END;
$$;

DROP TRIGGER IF EXISTS visit_sites_publication_guard ON visit_sites;
CREATE TRIGGER visit_sites_publication_guard
    BEFORE UPDATE ON visit_sites
    FOR EACH ROW
    EXECUTE FUNCTION poreia_guard_site_publication();

CREATE TABLE visit_site_reviews (
    id BIGSERIAL PRIMARY KEY,
    site_id UUID NOT NULL REFERENCES visit_sites(id) ON DELETE RESTRICT,
    wikidata_qid TEXT NOT NULL,
    decision TEXT NOT NULL CHECK (decision IN ('publish', 'reject', 'hold')),
    reason_code TEXT NOT NULL CHECK (reason_code IN (
        'location_verified', 'source_verified', 'relation_verified', 'license_verified',
        'insufficient_source', 'not_the_subject'
    )),
    evidence_url TEXT NOT NULL CHECK (evidence_url LIKE 'https://%'),
    actor_id UUID NOT NULL REFERENCES users(id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_visit_site_reviews_site_created
    ON visit_site_reviews (site_id, created_at);

CREATE OR REPLACE FUNCTION poreia_reject_review_mutation()
RETURNS trigger
LANGUAGE plpgsql
AS $$
BEGIN
    RAISE EXCEPTION 'visit_site_reviews_append_only';
END;
$$;

DROP TRIGGER IF EXISTS visit_site_reviews_append_only ON visit_site_reviews;
CREATE TRIGGER visit_site_reviews_append_only
    BEFORE UPDATE OR DELETE ON visit_site_reviews
    FOR EACH ROW
    EXECUTE FUNCTION poreia_reject_review_mutation();
