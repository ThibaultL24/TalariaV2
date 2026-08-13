-- 019_intuition_publication_v2_status.sql
-- v2 queue statuses: pending / pin_failed / failed / published.
-- Keep v1 planned/exported readable; never rewrite those rows.

ALTER TABLE intuition_publications
    DROP CONSTRAINT IF EXISTS intuition_publications_status_check;

ALTER TABLE intuition_publications
    ADD CONSTRAINT intuition_publications_status_check
    CHECK (status IN (
        'planned',
        'exported',
        'pending',
        'pin_failed',
        'failed',
        'published'
    ));

ALTER TABLE intuition_publications
    ALTER COLUMN status SET DEFAULT 'pending';
