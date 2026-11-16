-- 054_editorial_groups.sql
-- Editorial biography structure. A group organises logical facts for one person.
-- It does not change canonical type, evidence, dates, or historical status.
-- Publication of a chapter is not attestation of its members.

CREATE TABLE IF NOT EXISTS editorial_groups (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    entity_id UUID NOT NULL REFERENCES entities(id) ON DELETE RESTRICT,
    slug TEXT NOT NULL,
    title TEXT NOT NULL,
    group_kind TEXT NOT NULL,
    summary TEXT,
    summary_origin TEXT,
    status TEXT NOT NULL DEFAULT 'draft',
    display_order INT NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT editorial_groups_kind_check CHECK (
        group_kind IN ('chronological', 'thematic', 'narrative', 'documentary')
    ),
    CONSTRAINT editorial_groups_status_check CHECK (status IN ('draft', 'published')),
    CONSTRAINT editorial_groups_slug_entity UNIQUE (entity_id, slug)
);

CREATE INDEX IF NOT EXISTS idx_editorial_groups_entity_order
    ON editorial_groups (entity_id, display_order, id);

CREATE TABLE IF NOT EXISTS editorial_memberships (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    group_id UUID NOT NULL REFERENCES editorial_groups(id) ON DELETE CASCADE,
    logical_event_id UUID NOT NULL REFERENCES logical_events(id) ON DELETE RESTRICT,
    editorial_role TEXT NOT NULL,
    display_order INT NOT NULL DEFAULT 0,
    revision_note TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT editorial_memberships_role_check CHECK (
        editorial_role IN ('primary', 'supporting', 'context')
    ),
    CONSTRAINT editorial_memberships_unique UNIQUE (group_id, logical_event_id)
);

CREATE INDEX IF NOT EXISTS idx_editorial_memberships_group_order
    ON editorial_memberships (group_id, display_order, id);

CREATE INDEX IF NOT EXISTS idx_editorial_memberships_logical
    ON editorial_memberships (logical_event_id);
