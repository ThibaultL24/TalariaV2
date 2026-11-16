// crates/talaria-store/src/editorial.rs
//! Chapters and thematic groups for a biography.
//! Membership points at a logical fact. It never rewrites the canonical version.

use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

pub const GROUP_CHRONOLOGICAL: &str = "chronological";
pub const GROUP_THEMATIC: &str = "thematic";
pub const GROUP_NARRATIVE: &str = "narrative";
pub const GROUP_DOCUMENTARY: &str = "documentary";

pub const STATUS_DRAFT: &str = "draft";
pub const STATUS_PUBLISHED: &str = "published";

pub const ROLE_PRIMARY: &str = "primary";
pub const ROLE_SUPPORTING: &str = "supporting";
pub const ROLE_CONTEXT: &str = "context";

pub const ASSERTION_ATTESTED: &str = "attested";
pub const ASSERTION_EDITORIAL: &str = "editorial";
pub const ASSERTION_UNCERTAINTY: &str = "uncertainty";
pub const ASSERTION_GAP: &str = "gap";

pub const PASSAGE_CURRENT: &str = "current";
pub const PASSAGE_NEEDS_REVIEW: &str = "needs_review";

const MAX_PAGE: i64 = 100;

const KINDS: [&str; 4] = [
    GROUP_CHRONOLOGICAL,
    GROUP_THEMATIC,
    GROUP_NARRATIVE,
    GROUP_DOCUMENTARY,
];
const STATUSES: [&str; 2] = [STATUS_DRAFT, STATUS_PUBLISHED];
const ROLES: [&str; 3] = [ROLE_PRIMARY, ROLE_SUPPORTING, ROLE_CONTEXT];
const ASSERTION_KINDS: [&str; 4] = [
    ASSERTION_ATTESTED,
    ASSERTION_EDITORIAL,
    ASSERTION_UNCERTAINTY,
    ASSERTION_GAP,
];

#[derive(Debug, Clone)]
pub struct NewEditorialGroup<'a> {
    pub entity_id: Uuid,
    pub slug: &'a str,
    pub title: &'a str,
    pub group_kind: &'a str,
    pub summary: Option<&'a str>,
    pub summary_origin: Option<&'a str>,
    pub display_order: i32,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct EditorialGroup {
    pub id: Uuid,
    pub entity_id: Uuid,
    pub slug: String,
    pub title: String,
    pub group_kind: String,
    pub summary: Option<String>,
    pub summary_origin: Option<String>,
    pub status: String,
    pub display_order: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct EditorialGroupPage {
    pub groups: Vec<EditorialGroup>,
    pub has_more: bool,
}

#[derive(Debug, Clone)]
pub struct EditorialEventFilter<'a> {
    pub event_type: Option<&'a str>,
    pub epistemic_status: Option<&'a str>,
    pub map_eligible: Option<bool>,
    pub timeline_eligible: Option<bool>,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct EditorialEvent {
    pub membership_id: Uuid,
    pub logical_event_id: Uuid,
    pub editorial_role: String,
    pub display_order: i32,
    pub canonical_event_id: Uuid,
    pub event_type: String,
    pub epistemic_status: String,
    pub title: String,
    pub time_json: serde_json::Value,
    pub place_label: Option<String>,
    pub map_eligible: bool,
    pub timeline_eligible: bool,
    pub lat: Option<f64>,
    pub lon: Option<f64>,
    pub coord_precision_level: Option<String>,
    pub uncertainty_radius_m: Option<f64>,
    pub evidence_count: i64,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ChapterCoverage {
    pub members: i64,
    pub established: i64,
    pub attested: i64,
    pub in_review: i64,
    pub without_coordinates: i64,
    pub time_not_exact: i64,
    pub with_evidence: i64,
    pub evidence_rows: i64,
    pub independent_sources: i64,
}

#[derive(Debug, Clone)]
pub struct EditorialEventPage {
    pub events: Vec<EditorialEvent>,
    pub has_more: bool,
}

pub async fn create_editorial_group(
    pool: &PgPool,
    group: NewEditorialGroup<'_>,
) -> anyhow::Result<Uuid> {
    require_slug(group.slug)?;
    require_choice(group.group_kind, &KINDS, "editorial_kind_invalid")?;
    if group.title.trim().is_empty() {
        anyhow::bail!("editorial_title_required");
    }
    let id = sqlx::query_scalar(
        r#"
        INSERT INTO editorial_groups (
            entity_id, slug, title, group_kind, summary, summary_origin, status, display_order
        )
        VALUES ($1, $2, $3, $4, $5, $6, 'draft', $7)
        RETURNING id
        "#,
    )
    .bind(group.entity_id)
    .bind(group.slug)
    .bind(group.title.trim())
    .bind(group.group_kind)
    .bind(group.summary)
    .bind(group.summary_origin)
    .bind(group.display_order)
    .fetch_one(pool)
    .await
    .map_err(map_write_error)?;
    Ok(id)
}

pub async fn set_editorial_group_status(
    pool: &PgPool,
    group_id: Uuid,
    status: &str,
) -> anyhow::Result<()> {
    require_choice(status, &STATUSES, "editorial_status_invalid")?;
    let updated = sqlx::query(
        r#"
        UPDATE editorial_groups
        SET status = $2, updated_at = NOW()
        WHERE id = $1
        "#,
    )
    .bind(group_id)
    .bind(status)
    .execute(pool)
    .await?
    .rows_affected();
    if updated == 0 {
        anyhow::bail!("editorial_group_not_found");
    }
    Ok(())
}

/// Remove the group and its memberships. Canonical events stay in place.
pub async fn delete_editorial_group(pool: &PgPool, group_id: Uuid) -> anyhow::Result<()> {
    let deleted = sqlx::query("DELETE FROM editorial_groups WHERE id = $1")
        .bind(group_id)
        .execute(pool)
        .await?
        .rows_affected();
    if deleted == 0 {
        anyhow::bail!("editorial_group_not_found");
    }
    Ok(())
}

pub async fn add_editorial_membership(
    pool: &PgPool,
    group_id: Uuid,
    logical_event_id: Uuid,
    editorial_role: &str,
    display_order: i32,
    revision_note: Option<&str>,
) -> anyhow::Result<Uuid> {
    require_choice(editorial_role, &ROLES, "editorial_role_invalid")?;
    let subject: Option<Uuid> =
        sqlx::query_scalar("SELECT entity_id FROM editorial_groups WHERE id = $1")
            .bind(group_id)
            .fetch_optional(pool)
            .await?;
    let Some(entity_id) = subject else {
        anyhow::bail!("editorial_group_not_found");
    };
    let belongs: bool = sqlx::query_scalar(
        r#"
        SELECT EXISTS (
            SELECT 1 FROM canonical_events
            WHERE logical_event_id = $1
              AND entity_id = $2
              AND is_active
              AND pipeline = 'person'
        )
        "#,
    )
    .bind(logical_event_id)
    .bind(entity_id)
    .fetch_one(pool)
    .await?;
    if !belongs {
        anyhow::bail!("logical_event_not_in_subject");
    }
    let id = sqlx::query_scalar(
        r#"
        INSERT INTO editorial_memberships (
            group_id, logical_event_id, editorial_role, display_order, revision_note
        )
        VALUES ($1, $2, $3, $4, $5)
        RETURNING id
        "#,
    )
    .bind(group_id)
    .bind(logical_event_id)
    .bind(editorial_role)
    .bind(display_order)
    .bind(revision_note)
    .fetch_one(pool)
    .await
    .map_err(map_write_error)?;
    Ok(id)
}

pub fn is_editorial_slug(slug: &str) -> bool {
    require_slug(slug).is_ok()
}

pub async fn find_published_editorial_group(
    pool: &PgPool,
    entity_id: Uuid,
    slug: &str,
) -> anyhow::Result<Option<EditorialGroup>> {
    if !is_editorial_slug(slug) {
        return Ok(None);
    }
    let group = sqlx::query_as(
        r#"
        SELECT id, entity_id, slug, title, group_kind, summary, summary_origin,
               status, display_order, created_at, updated_at
        FROM editorial_groups
        WHERE entity_id = $1 AND slug = $2 AND status = 'published'
        "#,
    )
    .bind(entity_id)
    .bind(slug)
    .fetch_optional(pool)
    .await?;
    Ok(group)
}

/// Counts the chapter can actually support. There is no completeness rate:
/// that would require an explicit list of expected facts, which this model does not store.
pub async fn chapter_coverage(pool: &PgPool, group_id: Uuid) -> anyhow::Result<ChapterCoverage> {
    let coverage = sqlx::query_as(
        r#"
        SELECT
            count(*)::bigint AS members,
            count(*) FILTER (WHERE ce.epistemic_status = 'established')::bigint AS established,
            count(*) FILTER (WHERE ce.epistemic_status = 'attested')::bigint AS attested,
            count(*) FILTER (WHERE ce.epistemic_status IN ('uncertain', 'theory', 'rumor'))::bigint AS in_review,
            count(*) FILTER (WHERE ce.geom IS NULL OR NOT ce.map_eligible)::bigint AS without_coordinates,
            count(*) FILTER (WHERE ce.time_json->>'kind' IS DISTINCT FROM 'exact')::bigint AS time_not_exact,
            count(*) FILTER (WHERE EXISTS (
                SELECT 1 FROM event_evidence ev WHERE ev.canonical_event_id = ce.id
            ))::bigint AS with_evidence,
            COALESCE(sum((
                SELECT count(*) FROM event_evidence ev WHERE ev.canonical_event_id = ce.id
            )), 0)::bigint AS evidence_rows,
            (
                SELECT count(DISTINCT ev.raw_document_id)::bigint
                FROM event_evidence ev
                JOIN canonical_events active
                  ON active.id = ev.canonical_event_id
                 AND active.is_active
                 AND active.pipeline = 'person'
                JOIN editorial_memberships membership
                  ON membership.logical_event_id = active.logical_event_id
                 AND membership.group_id = $1
                WHERE ev.raw_document_id IS NOT NULL
            ) AS independent_sources
        FROM editorial_memberships m
        JOIN canonical_events ce
          ON ce.logical_event_id = m.logical_event_id
         AND ce.is_active
         AND ce.pipeline = 'person'
        WHERE m.group_id = $1
        "#,
    )
    .bind(group_id)
    .fetch_one(pool)
    .await?;
    Ok(coverage)
}

pub async fn list_editorial_groups(
    pool: &PgPool,
    entity_id: Uuid,
    status: Option<&str>,
    limit: i64,
    after_order: Option<i32>,
    after_id: Option<Uuid>,
) -> anyhow::Result<EditorialGroupPage> {
    if let Some(status) = status {
        require_choice(status, &STATUSES, "editorial_status_invalid")?;
    }
    let page_size = limit.clamp(1, MAX_PAGE);
    let rows: Vec<EditorialGroup> = sqlx::query_as(
        r#"
        SELECT id, entity_id, slug, title, group_kind, summary, summary_origin,
               status, display_order, created_at, updated_at
        FROM editorial_groups
        WHERE entity_id = $1
          AND ($2::text IS NULL OR status = $2)
          AND (
            $3::int IS NULL
            OR display_order > $3
            OR (display_order = $3 AND id > $4)
          )
        ORDER BY display_order, id
        LIMIT $5
        "#,
    )
    .bind(entity_id)
    .bind(status)
    .bind(after_order)
    .bind(after_id)
    .bind(page_size + 1)
    .fetch_all(pool)
    .await?;
    let has_more = rows.len() as i64 > page_size;
    let mut groups = rows;
    if has_more {
        groups.pop();
    }
    Ok(EditorialGroupPage { groups, has_more })
}

pub async fn list_editorial_group_events(
    pool: &PgPool,
    group_id: Uuid,
    filter: EditorialEventFilter<'_>,
    limit: i64,
    after_order: Option<i32>,
    after_id: Option<Uuid>,
) -> anyhow::Result<EditorialEventPage> {
    let exists: bool =
        sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM editorial_groups WHERE id = $1)")
            .bind(group_id)
            .fetch_one(pool)
            .await?;
    if !exists {
        anyhow::bail!("editorial_group_not_found");
    }
    let page_size = limit.clamp(1, MAX_PAGE);
    let rows: Vec<EditorialEvent> = sqlx::query_as(
        r#"
        SELECT
            m.id AS membership_id,
            m.logical_event_id,
            m.editorial_role,
            m.display_order,
            ce.id AS canonical_event_id,
            ce.event_type,
            ce.epistemic_status,
            ce.title,
            ce.time_json,
            ce.place_label,
            ce.map_eligible,
            ce.timeline_eligible,
            ST_Y(ce.geom::geometry) AS lat,
            ST_X(ce.geom::geometry) AS lon,
            ce.coord_precision_level,
            ce.uncertainty_radius_m,
            (SELECT count(*) FROM event_evidence ev WHERE ev.canonical_event_id = ce.id) AS evidence_count
        FROM editorial_memberships m
        JOIN canonical_events ce
          ON ce.logical_event_id = m.logical_event_id
         AND ce.is_active
         AND ce.pipeline = 'person'
        WHERE m.group_id = $1
          AND ($2::text IS NULL OR ce.event_type = $2)
          AND ($3::text IS NULL OR ce.epistemic_status = $3)
          AND ($4::bool IS NULL OR ce.map_eligible = $4)
          AND ($5::bool IS NULL OR ce.timeline_eligible = $5)
          AND (
            $6::int IS NULL
            OR m.display_order > $6
            OR (m.display_order = $6 AND m.id > $7)
          )
        ORDER BY m.display_order, m.id
        LIMIT $8
        "#,
    )
    .bind(group_id)
    .bind(filter.event_type)
    .bind(filter.epistemic_status)
    .bind(filter.map_eligible)
    .bind(filter.timeline_eligible)
    .bind(after_order)
    .bind(after_id)
    .bind(page_size + 1)
    .fetch_all(pool)
    .await?;
    let has_more = rows.len() as i64 > page_size;
    let mut events = rows;
    if has_more {
        events.pop();
    }
    Ok(EditorialEventPage { events, has_more })
}

#[derive(Debug, Clone)]
pub struct NewEditorialAssertion<'a> {
    pub group_id: Uuid,
    pub display_order: i32,
    pub prose: &'a str,
    pub assertion_kind: &'a str,
    pub logical_event_id: Option<Uuid>,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct AssertionPassageReview {
    pub event_evidence_id: Uuid,
    pub state: String,
}

/// Records one sentence of a chapter. An attested sentence names a logical fact.
/// A gap names something the corpus does not contain, so it has no fact.
pub async fn create_editorial_assertion(
    pool: &PgPool,
    assertion: NewEditorialAssertion<'_>,
) -> anyhow::Result<Uuid> {
    let prose = assertion.prose.trim();
    if prose.is_empty() {
        anyhow::bail!("editorial_assertion_prose_required");
    }
    require_choice(
        assertion.assertion_kind,
        &ASSERTION_KINDS,
        "editorial_assertion_kind_invalid",
    )?;
    if assertion.assertion_kind == ASSERTION_GAP && assertion.logical_event_id.is_some() {
        anyhow::bail!("editorial_gap_has_no_event");
    }
    if assertion.assertion_kind == ASSERTION_ATTESTED && assertion.logical_event_id.is_none() {
        anyhow::bail!("editorial_attested_needs_event");
    }
    let id = sqlx::query_scalar(
        r#"
        INSERT INTO editorial_assertions (
            group_id, display_order, prose, assertion_kind, logical_event_id
        )
        VALUES ($1, $2, $3, $4, $5)
        RETURNING id
        "#,
    )
    .bind(assertion.group_id)
    .bind(assertion.display_order)
    .bind(prose)
    .bind(assertion.assertion_kind)
    .bind(assertion.logical_event_id)
    .fetch_one(pool)
    .await?;
    Ok(id)
}

/// Cites one existing passage. The stored fingerprint is that row's evidence_hash.
/// The passage must belong to the same logical fact as the sentence.
pub async fn link_assertion_passage(
    pool: &PgPool,
    assertion_id: Uuid,
    event_evidence_id: Uuid,
) -> anyhow::Result<()> {
    let kind: String =
        sqlx::query_scalar("SELECT assertion_kind FROM editorial_assertions WHERE id = $1")
            .bind(assertion_id)
            .fetch_one(pool)
            .await?;
    if kind == ASSERTION_GAP {
        anyhow::bail!("editorial_gap_has_no_passage");
    }
    let linked = sqlx::query(
        r#"
        INSERT INTO editorial_assertion_passages (assertion_id, event_evidence_id, evidence_hash)
        SELECT assertion.id, evidence.id, evidence.evidence_hash
        FROM editorial_assertions assertion
        JOIN event_evidence evidence ON evidence.id = $2
        JOIN canonical_events cited ON cited.id = evidence.canonical_event_id
        WHERE assertion.id = $1
          AND evidence.evidence_hash IS NOT NULL
          AND length(btrim(evidence.evidence_hash)) > 0
          AND (
                assertion.logical_event_id IS NULL
                OR cited.logical_event_id = assertion.logical_event_id
              )
        ON CONFLICT (assertion_id, event_evidence_id) DO NOTHING
        "#,
    )
    .bind(assertion_id)
    .bind(event_evidence_id)
    .execute(pool)
    .await?;
    if linked.rows_affected() == 0 {
        let exists: bool = sqlx::query_scalar(
            r#"
            SELECT EXISTS (
                SELECT 1 FROM editorial_assertion_passages
                WHERE assertion_id = $1 AND event_evidence_id = $2
            )
            "#,
        )
        .bind(assertion_id)
        .bind(event_evidence_id)
        .fetch_one(pool)
        .await?;
        if !exists {
            anyhow::bail!("editorial_passage_not_linkable");
        }
    }
    Ok(())
}

/// A passage stays current when the active version of the logical fact still carries
/// the same fingerprint, document, and quoted text. Supersession copies those three.
/// A missing copy, or a changed fingerprint, asks for the citation to be read again.
pub async fn review_assertion_passages(
    pool: &PgPool,
    assertion_id: Uuid,
) -> anyhow::Result<Vec<AssertionPassageReview>> {
    let rows = sqlx::query_as(
        r#"
        SELECT
            passage.event_evidence_id,
            CASE WHEN EXISTS (
                SELECT 1
                FROM event_evidence cited
                JOIN editorial_assertions assertion ON assertion.id = passage.assertion_id
                JOIN canonical_events active
                  ON active.logical_event_id = assertion.logical_event_id
                 AND active.is_active
                 AND active.pipeline = 'person'
                JOIN event_evidence active_evidence
                  ON active_evidence.canonical_event_id = active.id
                 AND active_evidence.evidence_hash = passage.evidence_hash
                 AND active_evidence.quoted_text IS NOT DISTINCT FROM cited.quoted_text
                 AND active_evidence.raw_document_id IS NOT DISTINCT FROM cited.raw_document_id
                WHERE cited.id = passage.event_evidence_id
                  AND cited.evidence_hash = passage.evidence_hash
            ) THEN 'current' ELSE 'needs_review' END AS state
        FROM editorial_assertion_passages passage
        WHERE passage.assertion_id = $1
        ORDER BY passage.created_at, passage.event_evidence_id
        "#,
    )
    .bind(assertion_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

fn require_slug(slug: &str) -> anyhow::Result<()> {
    let length_ok = (1..=80).contains(&slug.len());
    let chars_ok = slug
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
    let edges_ok = !slug.starts_with('-') && !slug.ends_with('-') && !slug.contains("--");
    if length_ok && chars_ok && edges_ok {
        return Ok(());
    }
    anyhow::bail!("editorial_slug_invalid");
}

fn require_choice(value: &str, allowed: &[&str], error: &'static str) -> anyhow::Result<()> {
    if allowed.contains(&value) {
        Ok(())
    } else {
        anyhow::bail!(error);
    }
}

fn map_write_error(err: sqlx::Error) -> anyhow::Error {
    let constraint = err
        .as_database_error()
        .and_then(|error| error.constraint())
        .unwrap_or("");
    let code = err
        .as_database_error()
        .and_then(|error| error.code())
        .map(|code| code.into_owned());
    if code.as_deref() == Some("23505") {
        if constraint.contains("slug") {
            return anyhow::anyhow!("editorial_slug_taken");
        }
        return anyhow::anyhow!("editorial_membership_exists");
    }
    err.into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn editorial_slug_rejects_a_title_with_spaces() {
        assert!(require_slug("italian-campaign").is_ok());
        assert!(require_slug("Italian Campaign").is_err());
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn editorial_groups_cover_five_biographies_without_changing_facts(pool: sqlx::PgPool) {
        use chrono::TimeZone;
        let utc = chrono::Utc;
        let napoleon = crate::ensure_subject_entity(&pool, "Q517-editorial", "Napoleon Bonaparte")
            .await
            .unwrap();
        let hugo = crate::ensure_subject_entity(&pool, "Q535-editorial", "Victor Hugo")
            .await
            .unwrap();
        let curie = crate::ensure_subject_entity(&pool, "Q7186-editorial", "Marie Curie")
            .await
            .unwrap();
        let leonardo = crate::ensure_subject_entity(&pool, "Q762-editorial", "Leonardo da Vinci")
            .await
            .unwrap();
        let aristotle = crate::ensure_subject_entity(&pool, "Q868-editorial", "Aristotle")
            .await
            .unwrap();

        let (lodi, lodi_logical) = fact(
            &pool,
            napoleon,
            "battle",
            "Battle of Lodi",
            Some(utc.with_ymd_and_hms(1796, 1, 1, 0, 0, 0).unwrap()),
            json_time("exact", "year", "1796"),
            Some("Lodi"),
            Some((45.3, 9.5)),
            "editorial-lodi",
            "attested",
        )
        .await;
        let quote = "Napoleon reached Lodi on 1796-05-10 during the Italian campaign.";
        crate::insert_person_quote_evidence(
            &pool,
            lodi,
            quote,
            None,
            0.9,
            "https://example.test/lodi",
        )
        .await
        .unwrap();
        let (_march, march_logical) = fact(
            &pool,
            napoleon,
            "travel",
            "March toward Milan",
            Some(utc.with_ymd_and_hms(1796, 5, 1, 0, 0, 0).unwrap()),
            json_time("exact", "month", "1796-05"),
            Some("Milan"),
            None,
            "editorial-march",
            "attested",
        )
        .await;
        let (_armistice, armistice_logical) = fact(
            &pool,
            napoleon,
            "diplomatic",
            "Armistice talks",
            Some(utc.with_ymd_and_hms(1796, 5, 15, 0, 0, 0).unwrap()),
            json_time("exact", "day", "1796-05-15"),
            Some("Cherasco"),
            None,
            "editorial-armistice",
            "uncertain",
        )
        .await;

        let campaign = create_editorial_group(
            &pool,
            NewEditorialGroup {
                entity_id: napoleon,
                slug: "italian-campaign",
                title: "Italian campaign",
                group_kind: GROUP_CHRONOLOGICAL,
                summary: Some("A drafted reading of the 1796 campaign."),
                summary_origin: Some("editor"),
                display_order: 1,
            },
        )
        .await
        .unwrap();
        assert_eq!(group_status(&pool, campaign).await, STATUS_DRAFT);
        add_editorial_membership(&pool, campaign, lodi_logical, ROLE_PRIMARY, 1, None)
            .await
            .unwrap();
        add_editorial_membership(&pool, campaign, march_logical, ROLE_SUPPORTING, 2, None)
            .await
            .unwrap();
        add_editorial_membership(
            &pool,
            campaign,
            armistice_logical,
            ROLE_CONTEXT,
            3,
            Some("talks remain uncertain"),
        )
        .await
        .unwrap();
        let duplicate =
            add_editorial_membership(&pool, campaign, lodi_logical, ROLE_PRIMARY, 9, None).await;
        assert!(duplicate
            .unwrap_err()
            .to_string()
            .contains("editorial_membership_exists"));
        let foreign = add_editorial_membership(
            &pool,
            campaign,
            fact(
                &pool,
                hugo,
                "exile",
                "Exile in Guernsey",
                Some(utc.with_ymd_and_hms(1855, 1, 1, 0, 0, 0).unwrap()),
                json_time("exact", "year", "1855"),
                Some("Guernsey"),
                None,
                "editorial-hugo-exile",
                "attested",
            )
            .await
            .1,
            ROLE_CONTEXT,
            4,
            None,
        )
        .await;
        assert!(foreign
            .unwrap_err()
            .to_string()
            .contains("logical_event_not_in_subject"));

        let first = list_editorial_group_events(&pool, campaign, empty_filter(), 2, None, None)
            .await
            .unwrap();
        assert!(first.has_more);
        assert_eq!(first.events.len(), 2);
        assert_eq!(first.events[0].canonical_event_id, lodi);
        assert!(first
            .events
            .iter()
            .any(|event| event.event_type == "travel"));
        let cursor = &first.events[1];
        let second = list_editorial_group_events(
            &pool,
            campaign,
            empty_filter(),
            2,
            Some(cursor.display_order),
            Some(cursor.membership_id),
        )
        .await
        .unwrap();
        assert!(!second.has_more);
        assert_eq!(second.events.len(), 1);
        assert_eq!(second.events[0].event_type, "diplomatic");
        assert_eq!(second.events[0].epistemic_status, "uncertain");

        let battles = list_editorial_group_events(
            &pool,
            campaign,
            EditorialEventFilter {
                event_type: Some("battle"),
                epistemic_status: None,
                map_eligible: None,
                timeline_eligible: None,
            },
            10,
            None,
            None,
        )
        .await
        .unwrap();
        assert_eq!(battles.events.len(), 1);

        let before = canonical_snapshot(&pool, lodi).await;
        let evidence_before: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM event_evidence WHERE canonical_event_id = $1")
                .bind(lodi)
                .fetch_one(&pool)
                .await
                .unwrap();
        set_editorial_group_status(&pool, campaign, STATUS_PUBLISHED)
            .await
            .unwrap();
        assert_eq!(canonical_snapshot(&pool, lodi).await, before);
        assert_eq!(group_status(&pool, campaign).await, STATUS_PUBLISHED);

        let successor = crate::supersede_person_event(
            &pool,
            lodi,
            &crate::PersonSupersession {
                title: "Battle of Lodi".into(),
                event_type: "battle".into(),
                time_json: serde_json::json!({
                    "kind": "exact", "precision": "day", "start": "1796-05-10", "surface": "1796-05-10"
                }),
                start_time: Some(utc.with_ymd_and_hms(1796, 5, 10, 0, 0, 0).unwrap()),
                place_label: Some("Lodi".into()),
                lat: Some(45.3),
                lon: Some(9.5),
                map_eligible: true,
                place_identity_qid: None,
                evidence_quote: quote.into(),
                expected_fingerprint: Some("editorial-lodi".into()),
                date_precision: Some("day".into()),
                location_precision: None,
                uncertainty_radius_m: None,
                coord_precision_level: None,
            },
        )
        .await
        .unwrap();
        let after_correction =
            list_editorial_group_events(&pool, campaign, empty_filter(), 10, None, None)
                .await
                .unwrap();
        let shown = after_correction
            .events
            .iter()
            .find(|event| event.logical_event_id == lodi_logical)
            .unwrap();
        assert_eq!(shown.canonical_event_id, successor);
        assert_ne!(shown.canonical_event_id, lodi);
        let restored = crate::revert_person_supersession(&pool, successor)
            .await
            .unwrap();
        let after_revert =
            list_editorial_group_events(&pool, campaign, empty_filter(), 10, None, None)
                .await
                .unwrap();
        let shown = after_revert
            .events
            .iter()
            .find(|event| event.logical_event_id == lodi_logical)
            .unwrap();
        assert_eq!(restored, lodi);
        assert_eq!(shown.canonical_event_id, lodi);
        let evidence_after: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM event_evidence WHERE canonical_event_id = $1")
                .bind(lodi)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(evidence_after, evidence_before);

        let (_paper, paper_logical) = fact(
            &pool,
            curie,
            "publication",
            "Research on radioactivity",
            Some(utc.with_ymd_and_hms(1898, 1, 1, 0, 0, 0).unwrap()),
            json_time("exact", "year", "1898"),
            None,
            None,
            "editorial-curie-paper",
            "uncertain",
        )
        .await;
        let research = create_editorial_group(
            &pool,
            NewEditorialGroup {
                entity_id: curie,
                slug: "radioactivity",
                title: "Research on radioactivity",
                group_kind: GROUP_THEMATIC,
                summary: None,
                summary_origin: None,
                display_order: 1,
            },
        )
        .await
        .unwrap();
        add_editorial_membership(&pool, research, paper_logical, ROLE_PRIMARY, 1, None)
            .await
            .unwrap();
        let curie_before = canonical_snapshot(&pool, _paper).await;
        set_editorial_group_status(&pool, research, STATUS_PUBLISHED)
            .await
            .unwrap();
        assert_eq!(canonical_snapshot(&pool, _paper).await, curie_before);
        let listed = list_editorial_group_events(&pool, research, empty_filter(), 10, None, None)
            .await
            .unwrap();
        assert_eq!(listed.events[0].epistemic_status, "uncertain");
        assert!(listed.events[0].lat.is_none());

        let (_novel, novel_logical) = fact(
            &pool,
            hugo,
            "publication",
            "Les Misérables",
            Some(utc.with_ymd_and_hms(1862, 1, 1, 0, 0, 0).unwrap()),
            json_time("exact", "year", "1862"),
            None,
            None,
            "editorial-hugo-novel",
            "attested",
        )
        .await;
        let exile_group = create_editorial_group(
            &pool,
            NewEditorialGroup {
                entity_id: hugo,
                slug: "guernsey-exile",
                title: "Years of exile",
                group_kind: GROUP_NARRATIVE,
                summary: Some("Residence, writing, and public commitment."),
                summary_origin: Some("editor"),
                display_order: 2,
            },
        )
        .await
        .unwrap();
        let hugo_exile = list_logical(&pool, hugo, "Exile in Guernsey").await;
        add_editorial_membership(&pool, exile_group, hugo_exile, ROLE_PRIMARY, 1, None)
            .await
            .unwrap();
        add_editorial_membership(&pool, exile_group, novel_logical, ROLE_SUPPORTING, 2, None)
            .await
            .unwrap();
        create_editorial_group(
            &pool,
            NewEditorialGroup {
                entity_id: hugo,
                slug: "italian-campaign",
                title: "A slug reused by another person",
                group_kind: GROUP_NARRATIVE,
                summary: None,
                summary_origin: None,
                display_order: 3,
            },
        )
        .await
        .unwrap();
        let repeated_slug = create_editorial_group(
            &pool,
            NewEditorialGroup {
                entity_id: hugo,
                slug: "guernsey-exile",
                title: "Duplicate",
                group_kind: GROUP_NARRATIVE,
                summary: None,
                summary_origin: None,
                display_order: 4,
            },
        )
        .await;
        assert!(repeated_slug
            .unwrap_err()
            .to_string()
            .contains("editorial_slug_taken"));

        let (_study, study_logical) = fact(
            &pool,
            leonardo,
            "publication",
            "The Last Supper study",
            Some(utc.with_ymd_and_hms(1495, 1, 1, 0, 0, 0).unwrap()),
            serde_json::json!({"kind": "approx", "year": 1495, "surface": "about 1495"}),
            Some("Milan"),
            None,
            "editorial-leonardo-study",
            "uncertain",
        )
        .await;
        let (_machine, machine_logical) = fact(
            &pool,
            leonardo,
            "historical_fact",
            "Notes on a flying machine",
            None,
            serde_json::json!({"kind": "approx", "surface": "late fifteenth century"}),
            None,
            None,
            "editorial-leonardo-machine",
            "uncertain",
        )
        .await;
        let painting = create_editorial_group(
            &pool,
            NewEditorialGroup {
                entity_id: leonardo,
                slug: "painting",
                title: "Painting",
                group_kind: GROUP_THEMATIC,
                summary: None,
                summary_origin: None,
                display_order: 1,
            },
        )
        .await
        .unwrap();
        let notebooks = create_editorial_group(
            &pool,
            NewEditorialGroup {
                entity_id: leonardo,
                slug: "notebooks",
                title: "Notebooks",
                group_kind: GROUP_DOCUMENTARY,
                summary: None,
                summary_origin: None,
                display_order: 2,
            },
        )
        .await
        .unwrap();
        add_editorial_membership(&pool, painting, study_logical, ROLE_PRIMARY, 1, None)
            .await
            .unwrap();
        add_editorial_membership(&pool, notebooks, study_logical, ROLE_CONTEXT, 1, None)
            .await
            .unwrap();
        add_editorial_membership(&pool, notebooks, machine_logical, ROLE_PRIMARY, 2, None)
            .await
            .unwrap();
        let painting_events =
            list_editorial_group_events(&pool, painting, empty_filter(), 10, None, None)
                .await
                .unwrap();
        let notebook_events =
            list_editorial_group_events(&pool, notebooks, empty_filter(), 10, None, None)
                .await
                .unwrap();
        assert_eq!(painting_events.events.len(), 1);
        assert_eq!(notebook_events.events.len(), 2);
        assert!(notebook_events
            .events
            .iter()
            .any(|event| event.logical_event_id == study_logical));

        let (_lyceum, lyceum_logical) = fact(
            &pool,
            aristotle,
            "education",
            "Teaching at the Lyceum",
            None,
            serde_json::json!({"kind": "unknown", "surface": "date unknown"}),
            None,
            None,
            "editorial-aristotle-lyceum",
            "uncertain",
        )
        .await;
        let school = create_editorial_group(
            &pool,
            NewEditorialGroup {
                entity_id: aristotle,
                slug: "lyceum",
                title: "The Lyceum",
                group_kind: GROUP_CHRONOLOGICAL,
                summary: None,
                summary_origin: None,
                display_order: 1,
            },
        )
        .await
        .unwrap();
        add_editorial_membership(&pool, school, lyceum_logical, ROLE_PRIMARY, 1, None)
            .await
            .unwrap();
        let ancient = list_editorial_group_events(&pool, school, empty_filter(), 10, None, None)
            .await
            .unwrap();
        assert_eq!(ancient.events[0].time_json["kind"], "unknown");
        assert!(ancient.events[0].lat.is_none());
        assert!(ancient.events[0].place_label.is_none());

        let invalid = create_editorial_group(
            &pool,
            NewEditorialGroup {
                entity_id: aristotle,
                slug: "battles",
                title: "Battles",
                group_kind: "campaign",
                summary: None,
                summary_origin: None,
                display_order: 2,
            },
        )
        .await;
        assert!(invalid
            .unwrap_err()
            .to_string()
            .contains("editorial_kind_invalid"));

        delete_editorial_group(&pool, campaign).await.unwrap();
        assert!(
            list_editorial_group_events(&pool, campaign, empty_filter(), 10, None, None)
                .await
                .is_err()
        );
        let lodi_still: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM canonical_events WHERE id = $1")
                .bind(lodi)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(lodi_still, 1);
        let memberships: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM editorial_memberships WHERE group_id = $1")
                .bind(campaign)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(memberships, 0);

        let hugo_groups = list_editorial_groups(&pool, hugo, Some(STATUS_DRAFT), 10, None, None)
            .await
            .unwrap();
        assert!(hugo_groups.groups.len() >= 2);
        assert!(hugo_groups
            .groups
            .iter()
            .all(|group| group.status == STATUS_DRAFT));
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn an_assertion_follows_the_passage_across_a_new_version(pool: sqlx::PgPool) {
        use chrono::TimeZone;
        let utc = chrono::Utc;
        let hugo = crate::ensure_subject_entity(&pool, "Q535-assertion", "Victor Hugo")
            .await
            .unwrap();
        let (residence, logical) = fact(
            &pool,
            hugo,
            "residence",
            "The family settles at Marine Terrace",
            Some(utc.with_ymd_and_hms(1852, 1, 1, 0, 0, 0).unwrap()),
            json_time("exact", "year", "1852"),
            Some("Jersey"),
            None,
            "assertion-marine-terrace",
            "attested",
        )
        .await;
        let quote = "On 16 August the Hugo family settled at a house called Marine Terrace.";
        let evidence = crate::insert_person_quote_evidence(
            &pool,
            residence,
            quote,
            None,
            0.9,
            "https://example.test/hugo",
        )
        .await
        .unwrap();
        let chapter = create_editorial_group(
            &pool,
            NewEditorialGroup {
                entity_id: hugo,
                slug: "exile",
                title: "Years of exile",
                group_kind: GROUP_NARRATIVE,
                summary: None,
                summary_origin: Some("editor"),
                display_order: 1,
            },
        )
        .await
        .unwrap();
        let attested = create_editorial_assertion(
            &pool,
            NewEditorialAssertion {
                group_id: chapter,
                display_order: 1,
                prose: "A stored passage says the family settled at Marine Terrace.",
                assertion_kind: ASSERTION_ATTESTED,
                logical_event_id: Some(logical),
            },
        )
        .await
        .unwrap();
        link_assertion_passage(&pool, attested, evidence)
            .await
            .unwrap();
        let review = review_assertion_passages(&pool, attested).await.unwrap();
        assert_eq!(review.len(), 1);
        assert_eq!(review[0].state, PASSAGE_CURRENT);

        let gap = create_editorial_assertion(
            &pool,
            NewEditorialAssertion {
                group_id: chapter,
                display_order: 2,
                prose: "No canonical fact records the later return from exile.",
                assertion_kind: ASSERTION_GAP,
                logical_event_id: None,
            },
        )
        .await
        .unwrap();
        assert!(link_assertion_passage(&pool, gap, evidence).await.is_err());
        assert!(create_editorial_assertion(
            &pool,
            NewEditorialAssertion {
                group_id: chapter,
                display_order: 3,
                prose: "A gap cannot name a fact.",
                assertion_kind: ASSERTION_GAP,
                logical_event_id: Some(logical),
            },
        )
        .await
        .is_err());
        assert!(create_editorial_assertion(
            &pool,
            NewEditorialAssertion {
                group_id: chapter,
                display_order: 4,
                prose: "An attested sentence needs a fact.",
                assertion_kind: ASSERTION_ATTESTED,
                logical_event_id: None,
            },
        )
        .await
        .is_err());

        sqlx::query("UPDATE canonical_events SET is_active = false WHERE id = $1")
            .bind(residence)
            .execute(&pool)
            .await
            .unwrap();
        let successor: Uuid = sqlx::query_scalar(
            r#"
            INSERT INTO canonical_events (
                entity_id, event_type, epistemic_status, title, time_json, place_label,
                confidence, map_eligible, historically_valid, timeline_eligible, visit_eligible,
                fingerprint, occurrence_key, is_active, supersedes, predicate,
                assembler_version, pipeline, logical_event_id
            )
            SELECT entity_id, event_type, epistemic_status, title, time_json, place_label,
                   confidence, false, historically_valid, timeline_eligible, visit_eligible,
                   fingerprint || '-next', occurrence_key || '-next', true, id, predicate,
                   assembler_version, pipeline, logical_event_id
            FROM canonical_events
            WHERE id = $1
            RETURNING id
            "#,
        )
        .bind(residence)
        .fetch_one(&pool)
        .await
        .unwrap();
        sqlx::query("UPDATE canonical_events SET superseded_by = $1 WHERE id = $2")
            .bind(successor)
            .bind(residence)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query(
            r#"
            INSERT INTO event_evidence (
                canonical_event_id, sentence_id, quoted_text, raw_document_id,
                confidence, evidence_type, source_locator, evidence_hash
            )
            SELECT $1, sentence_id, quoted_text, raw_document_id,
                   confidence, evidence_type, source_locator, evidence_hash
            FROM event_evidence
            WHERE canonical_event_id = $2
            "#,
        )
        .bind(successor)
        .bind(residence)
        .execute(&pool)
        .await
        .unwrap();
        let after_copy = review_assertion_passages(&pool, attested).await.unwrap();
        assert_eq!(after_copy[0].state, PASSAGE_CURRENT);

        sqlx::query("DELETE FROM event_evidence WHERE canonical_event_id = $1")
            .bind(successor)
            .execute(&pool)
            .await
            .unwrap();
        let after_loss = review_assertion_passages(&pool, attested).await.unwrap();
        assert_eq!(after_loss[0].state, PASSAGE_NEEDS_REVIEW);
        let title: String = sqlx::query_scalar("SELECT title FROM canonical_events WHERE id = $1")
            .bind(residence)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(title, "The family settles at Marine Terrace");
    }

    fn empty_filter<'a>() -> EditorialEventFilter<'a> {
        EditorialEventFilter {
            event_type: None,
            epistemic_status: None,
            map_eligible: None,
            timeline_eligible: None,
        }
    }

    fn json_time(kind: &str, precision: &str, start: &str) -> serde_json::Value {
        serde_json::json!({"kind": kind, "precision": precision, "start": start})
    }

    async fn fact(
        pool: &sqlx::PgPool,
        entity_id: Uuid,
        event_type: &str,
        title: &str,
        start_time: Option<chrono::DateTime<Utc>>,
        time_json: serde_json::Value,
        place: Option<&str>,
        coords: Option<(f64, f64)>,
        key: &str,
        epistemic_status: &str,
    ) -> (Uuid, Uuid) {
        let id = crate::insert_person_event(
            pool,
            &crate::PersonEventInsert {
                entity_id,
                event_type: event_type.into(),
                epistemic_status: epistemic_status.into(),
                title: title.into(),
                summary: None,
                start_time,
                time_json,
                place_label: place.map(str::to_string),
                lat: coords.map(|pair| pair.0),
                lon: coords.map(|pair| pair.1),
                confidence: 0.7,
                map_eligible: coords.is_some(),
                fingerprint: key.into(),
                occurrence_key: key.into(),
                occurrence_stem: None,
                predicate: "recorded".into(),
                place_identity_qid: None,
            },
        )
        .await
        .unwrap();
        let logical = crate::logical_event_id_of(pool, id).await.unwrap().unwrap();
        (id, logical)
    }

    async fn group_status(pool: &sqlx::PgPool, group_id: Uuid) -> String {
        sqlx::query_scalar("SELECT status FROM editorial_groups WHERE id = $1")
            .bind(group_id)
            .fetch_one(pool)
            .await
            .unwrap()
    }

    async fn canonical_snapshot(
        pool: &sqlx::PgPool,
        event_id: Uuid,
    ) -> (String, String, serde_json::Value, bool) {
        sqlx::query_as(
            "SELECT epistemic_status, title, time_json, is_active FROM canonical_events WHERE id = $1",
        )
        .bind(event_id)
        .fetch_one(pool)
        .await
        .unwrap()
    }

    async fn list_logical(pool: &sqlx::PgPool, entity_id: Uuid, title: &str) -> Uuid {
        sqlx::query_scalar(
            r#"
            SELECT logical_event_id FROM canonical_events
            WHERE entity_id = $1 AND title = $2 AND is_active
            "#,
        )
        .bind(entity_id)
        .bind(title)
        .fetch_one(pool)
        .await
        .unwrap()
    }
}
