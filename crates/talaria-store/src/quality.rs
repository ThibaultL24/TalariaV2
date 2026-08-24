// crates/talaria-store/src/quality.rs
//! Persistence for document snapshots, fragments, and event_candidates.

use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct DocumentSnapshotInsert {
    pub source_type: String,
    pub source_uri: String,
    pub source_identifier: Option<String>,
    pub language: String,
    pub title: Option<String>,
    pub content_hash: String,
    pub revision_id: Option<String>,
    pub wiki_page_id: Option<Uuid>,
    pub raw_document_id: Option<Uuid>,
    pub text: String,
    pub metadata: serde_json::Value,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct DocumentSnapshotRow {
    pub id: Uuid,
    pub source_type: String,
    pub source_uri: String,
    pub content_hash: String,
    pub title: Option<String>,
    pub text: String,
}

pub async fn insert_document_snapshot(
    pool: &PgPool,
    snap: &DocumentSnapshotInsert,
) -> anyhow::Result<Uuid> {
    // Append-only: never mutate an existing snapshot row (title/text/metadata stay frozen).
    let inserted: Option<(Uuid,)> = sqlx::query_as(
        r#"
        INSERT INTO document_snapshots (
            source_type, source_uri, source_identifier, language, title,
            content_hash, revision_id, wiki_page_id, raw_document_id, text, metadata
        )
        VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11)
        ON CONFLICT (source_type, source_uri, content_hash) DO NOTHING
        RETURNING id
        "#,
    )
    .bind(&snap.source_type)
    .bind(&snap.source_uri)
    .bind(&snap.source_identifier)
    .bind(&snap.language)
    .bind(&snap.title)
    .bind(&snap.content_hash)
    .bind(&snap.revision_id)
    .bind(snap.wiki_page_id)
    .bind(snap.raw_document_id)
    .bind(&snap.text)
    .bind(&snap.metadata)
    .fetch_optional(pool)
    .await?;
    if let Some((id,)) = inserted {
        return Ok(id);
    }
    let id: Uuid = sqlx::query_scalar(
        r#"
        SELECT id FROM document_snapshots
        WHERE source_type = $1 AND source_uri = $2 AND content_hash = $3
        "#,
    )
    .bind(&snap.source_type)
    .bind(&snap.source_uri)
    .bind(&snap.content_hash)
    .fetch_one(pool)
    .await?;
    Ok(id)
}

pub async fn find_document_snapshot(
    pool: &PgPool,
    source_type: &str,
    source_uri: &str,
    content_hash: &str,
) -> anyhow::Result<Option<Uuid>> {
    let id: Option<Uuid> = sqlx::query_scalar(
        r#"
        SELECT id FROM document_snapshots
        WHERE source_type = $1 AND source_uri = $2 AND content_hash = $3
        "#,
    )
    .bind(source_type)
    .bind(source_uri)
    .bind(content_hash)
    .fetch_optional(pool)
    .await?;
    Ok(id)
}

pub async fn count_sentence_fragments(pool: &PgPool, snapshot_id: Uuid) -> anyhow::Result<i64> {
    let n: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)::bigint FROM document_fragments
        WHERE snapshot_id = $1 AND fragment_kind = 'sentence'
        "#,
    )
    .bind(snapshot_id)
    .fetch_one(pool)
    .await?;
    Ok(n)
}

#[derive(Debug, Clone)]
pub struct DocumentFragmentInsert {
    pub snapshot_id: Uuid,
    pub fragment_kind: String,
    pub parent_fragment_id: Option<Uuid>,
    pub sentence_id: Option<Uuid>,
    pub text: String,
    pub start_offset: i32,
    pub end_offset: i32,
    pub clause_index: Option<i32>,
    pub ordinal: i32,
    pub metadata: serde_json::Value,
}

pub async fn insert_document_fragment(
    pool: &PgPool,
    frag: &DocumentFragmentInsert,
) -> anyhow::Result<Uuid> {
    if frag.fragment_kind == "sentence" {
        let row: Option<(Uuid,)> = sqlx::query_as(
            r#"
            INSERT INTO document_fragments (
                snapshot_id, fragment_kind, parent_fragment_id, sentence_id,
                text, start_offset, end_offset, clause_index, ordinal, metadata
            )
            VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)
            ON CONFLICT (snapshot_id, ordinal) WHERE fragment_kind = 'sentence'
            DO NOTHING
            RETURNING id
            "#,
        )
        .bind(frag.snapshot_id)
        .bind(&frag.fragment_kind)
        .bind(frag.parent_fragment_id)
        .bind(frag.sentence_id)
        .bind(&frag.text)
        .bind(frag.start_offset)
        .bind(frag.end_offset)
        .bind(frag.clause_index)
        .bind(frag.ordinal)
        .bind(&frag.metadata)
        .fetch_optional(pool)
        .await?;
        if let Some((id,)) = row {
            return Ok(id);
        }
        let id: Uuid = sqlx::query_scalar(
            r#"
            SELECT id FROM document_fragments
            WHERE snapshot_id = $1 AND fragment_kind = 'sentence' AND ordinal = $2
            "#,
        )
        .bind(frag.snapshot_id)
        .bind(frag.ordinal)
        .fetch_one(pool)
        .await?;
        return Ok(id);
    }

    if frag.fragment_kind == "clause" {
        let row: Option<(Uuid,)> = sqlx::query_as(
            r#"
            INSERT INTO document_fragments (
                snapshot_id, fragment_kind, parent_fragment_id, sentence_id,
                text, start_offset, end_offset, clause_index, ordinal, metadata
            )
            VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)
            ON CONFLICT (parent_fragment_id, clause_index) WHERE fragment_kind = 'clause'
            DO NOTHING
            RETURNING id
            "#,
        )
        .bind(frag.snapshot_id)
        .bind(&frag.fragment_kind)
        .bind(frag.parent_fragment_id)
        .bind(frag.sentence_id)
        .bind(&frag.text)
        .bind(frag.start_offset)
        .bind(frag.end_offset)
        .bind(frag.clause_index)
        .bind(frag.ordinal)
        .bind(&frag.metadata)
        .fetch_optional(pool)
        .await?;
        if let Some((id,)) = row {
            return Ok(id);
        }
        let id: Uuid = sqlx::query_scalar(
            r#"
            SELECT id FROM document_fragments
            WHERE parent_fragment_id = $1 AND clause_index = $2 AND fragment_kind = 'clause'
            "#,
        )
        .bind(frag.parent_fragment_id)
        .bind(frag.clause_index)
        .fetch_one(pool)
        .await?;
        return Ok(id);
    }

    let id: Uuid = sqlx::query_scalar(
        r#"
        INSERT INTO document_fragments (
            snapshot_id, fragment_kind, parent_fragment_id, sentence_id,
            text, start_offset, end_offset, clause_index, ordinal, metadata
        )
        VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)
        RETURNING id
        "#,
    )
    .bind(frag.snapshot_id)
    .bind(&frag.fragment_kind)
    .bind(frag.parent_fragment_id)
    .bind(frag.sentence_id)
    .bind(&frag.text)
    .bind(frag.start_offset)
    .bind(frag.end_offset)
    .bind(frag.clause_index)
    .bind(frag.ordinal)
    .bind(&frag.metadata)
    .fetch_one(pool)
    .await?;
    Ok(id)
}

#[derive(Debug, Clone)]
pub struct EventCandidateInsert {
    pub snapshot_id: Uuid,
    pub fragment_id: Uuid,
    pub clause_index: i32,
    pub subject_surface: String,
    pub subject_entity_id: Option<Uuid>,
    pub event_type: String,
    pub predicate: String,
    pub time_json: serde_json::Value,
    pub place_mentions: serde_json::Value,
    pub object_mentions: serde_json::Value,
    pub participant_mentions: serde_json::Value,
    pub place_entity_id: Option<Uuid>,
    pub place_label: Option<String>,
    pub evidence_ptrs: serde_json::Value,
    pub extractor_version: String,
    /// Snapshot-local extraction idempotence (≠ historical occurrence).
    pub fingerprint: String,
    /// Shared historical identity across extractors / sources.
    pub occurrence_key: Option<String>,
    pub primary_object: Option<String>,
    pub action_role: Option<String>,
    pub status: String,
    pub rejection_codes: Vec<String>,
    pub judgment_json: serde_json::Value,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct EventCandidateRow {
    pub id: Uuid,
    pub snapshot_id: Uuid,
    pub fragment_id: Uuid,
    pub clause_index: i32,
    pub subject_surface: String,
    pub subject_entity_id: Option<Uuid>,
    pub event_type: String,
    pub predicate: String,
    pub time_json: serde_json::Value,
    pub place_mentions: serde_json::Value,
    pub object_mentions: serde_json::Value,
    pub participant_mentions: serde_json::Value,
    pub place_entity_id: Option<Uuid>,
    pub place_label: Option<String>,
    pub evidence_ptrs: serde_json::Value,
    pub extractor_version: String,
    pub fingerprint: String,
    pub status: String,
    pub rejection_codes: Vec<String>,
    pub judgment_json: serde_json::Value,
    pub canonical_event_id: Option<Uuid>,
}

/// Insert candidate; on fingerprint conflict return existing id (idempotent retry).
pub async fn upsert_event_candidate(
    pool: &PgPool,
    c: &EventCandidateInsert,
) -> anyhow::Result<(Uuid, bool)> {
    let row: Option<(Uuid,)> = sqlx::query_as(
        r#"
        INSERT INTO event_candidates (
            snapshot_id, fragment_id, clause_index,
            subject_surface, subject_entity_id, event_type, predicate, time_json,
            place_mentions, object_mentions, participant_mentions,
            place_entity_id, place_label, evidence_ptrs,
            extractor_version, fingerprint, occurrence_key, primary_object, action_role,
            status, rejection_codes, judgment_json
        )
        VALUES (
            $1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17,$18,$19,$20,$21,$22
        )
        ON CONFLICT (fingerprint) DO NOTHING
        RETURNING id
        "#,
    )
    .bind(c.snapshot_id)
    .bind(c.fragment_id)
    .bind(c.clause_index)
    .bind(&c.subject_surface)
    .bind(c.subject_entity_id)
    .bind(&c.event_type)
    .bind(&c.predicate)
    .bind(&c.time_json)
    .bind(&c.place_mentions)
    .bind(&c.object_mentions)
    .bind(&c.participant_mentions)
    .bind(c.place_entity_id)
    .bind(&c.place_label)
    .bind(&c.evidence_ptrs)
    .bind(&c.extractor_version)
    .bind(&c.fingerprint)
    .bind(&c.occurrence_key)
    .bind(&c.primary_object)
    .bind(&c.action_role)
    .bind(&c.status)
    .bind(&c.rejection_codes)
    .bind(&c.judgment_json)
    .fetch_optional(pool)
    .await?;

    if let Some((id,)) = row {
        return Ok((id, true));
    }

    let existing: Uuid =
        sqlx::query_scalar(r#"SELECT id FROM event_candidates WHERE fingerprint = $1"#)
            .bind(&c.fingerprint)
            .fetch_one(pool)
            .await?;
    Ok((existing, false))
}

pub async fn update_event_candidate_judgment(
    pool: &PgPool,
    id: Uuid,
    status: &str,
    rejection_codes: &[String],
    judgment_json: &serde_json::Value,
    subject_entity_id: Option<Uuid>,
    place_entity_id: Option<Uuid>,
    place_label: Option<&str>,
    place_mentions: &serde_json::Value,
    object_mentions: &serde_json::Value,
    participant_mentions: &serde_json::Value,
) -> anyhow::Result<()> {
    sqlx::query(
        r#"
        UPDATE event_candidates SET
            status = $2,
            rejection_codes = $3,
            judgment_json = $4,
            subject_entity_id = COALESCE($5, subject_entity_id),
            place_entity_id = $6,
            place_label = $7,
            place_mentions = $8,
            object_mentions = $9,
            participant_mentions = $10,
            updated_at = NOW()
        WHERE id = $1
        "#,
    )
    .bind(id)
    .bind(status)
    .bind(rejection_codes)
    .bind(judgment_json)
    .bind(subject_entity_id)
    .bind(place_entity_id)
    .bind(place_label)
    .bind(place_mentions)
    .bind(object_mentions)
    .bind(participant_mentions)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn mark_candidate_assembled(
    pool: &PgPool,
    candidate_id: Uuid,
    canonical_event_id: Uuid,
) -> anyhow::Result<()> {
    sqlx::query(
        r#"
        UPDATE event_candidates SET
            status = 'assembled',
            canonical_event_id = $2,
            updated_at = NOW()
        WHERE id = $1
        "#,
    )
    .bind(candidate_id)
    .bind(canonical_event_id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn list_event_candidates_by_status(
    pool: &PgPool,
    status: &str,
    limit: i64,
) -> anyhow::Result<Vec<EventCandidateRow>> {
    let rows = sqlx::query_as::<_, EventCandidateRow>(
        r#"
        SELECT id, snapshot_id, fragment_id, clause_index,
               subject_surface, subject_entity_id, event_type, predicate, time_json,
               place_mentions, object_mentions, participant_mentions,
               place_entity_id, place_label, evidence_ptrs,
               extractor_version, fingerprint, status, rejection_codes,
               judgment_json, canonical_event_id
        FROM event_candidates
        WHERE status = $1
        ORDER BY created_at ASC
        LIMIT $2
        "#,
    )
    .bind(status)
    .bind(limit)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn get_event_candidate_by_fingerprint(
    pool: &PgPool,
    fingerprint: &str,
) -> anyhow::Result<Option<EventCandidateRow>> {
    let row = sqlx::query_as::<_, EventCandidateRow>(
        r#"
        SELECT id, snapshot_id, fragment_id, clause_index,
               subject_surface, subject_entity_id, event_type, predicate, time_json,
               place_mentions, object_mentions, participant_mentions,
               place_entity_id, place_label, evidence_ptrs,
               extractor_version, fingerprint, status, rejection_codes,
               judgment_json, canonical_event_id
        FROM event_candidates
        WHERE fingerprint = $1
        "#,
    )
    .bind(fingerprint)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

#[derive(Debug, Clone, Default)]
pub struct QualityReportCounts {
    pub candidates: i64,
    pub rejected: i64,
    pub needs_review: i64,
    pub accepted: i64,
    pub assembled: i64,
    pub quality_events_active: i64,
    pub quality_events_map_eligible: i64,
}

pub async fn quality_report_counts(pool: &PgPool) -> anyhow::Result<QualityReportCounts> {
    let candidates: i64 = sqlx::query_scalar(r#"SELECT COUNT(*)::bigint FROM event_candidates"#)
        .fetch_one(pool)
        .await?;
    let rejected: i64 = sqlx::query_scalar(
        r#"SELECT COUNT(*)::bigint FROM event_candidates WHERE status = 'rejected'"#,
    )
    .fetch_one(pool)
    .await?;
    let needs_review: i64 = sqlx::query_scalar(
        r#"SELECT COUNT(*)::bigint FROM event_candidates WHERE status = 'needs_review'"#,
    )
    .fetch_one(pool)
    .await?;
    let accepted: i64 = sqlx::query_scalar(
        r#"SELECT COUNT(*)::bigint FROM event_candidates WHERE status IN ('accepted','assembled')"#,
    )
    .fetch_one(pool)
    .await?;
    let assembled: i64 = sqlx::query_scalar(
        r#"SELECT COUNT(*)::bigint FROM event_candidates WHERE status = 'assembled'"#,
    )
    .fetch_one(pool)
    .await?;
    let quality_events_active: i64 = sqlx::query_scalar(
        r#"SELECT COUNT(*)::bigint FROM canonical_events WHERE pipeline = 'quality' AND is_active"#,
    )
    .fetch_one(pool)
    .await?;
    let quality_events_map_eligible: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)::bigint FROM canonical_events
        WHERE pipeline = 'quality' AND is_active AND map_eligible
        "#,
    )
    .fetch_one(pool)
    .await?;

    Ok(QualityReportCounts {
        candidates,
        rejected,
        needs_review,
        accepted,
        assembled,
        quality_events_active,
        quality_events_map_eligible,
    })
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct RejectionReasonCount {
    pub code: String,
    pub count: i64,
}

pub async fn rejection_reason_counts(pool: &PgPool) -> anyhow::Result<Vec<RejectionReasonCount>> {
    let rows = sqlx::query_as::<_, RejectionReasonCount>(
        r#"
        SELECT unnest(rejection_codes) AS code, COUNT(*)::bigint AS count
        FROM event_candidates
        WHERE status = 'rejected'
        GROUP BY 1
        ORDER BY count DESC, code ASC
        "#,
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn upsert_entity_with_kind(
    pool: &PgPool,
    wiki_lang: &str,
    surface: &str,
    kind: &str,
) -> anyhow::Result<Uuid> {
    let id: Uuid = sqlx::query_scalar(
        r#"
        INSERT INTO entities (wikipedia_title, wiki_lang, canonical_name, kind)
        VALUES ($1, $2, $3, $4)
        ON CONFLICT (wiki_lang, wikipedia_title) DO UPDATE SET
            canonical_name = EXCLUDED.canonical_name,
            kind = CASE
                WHEN entities.kind = 'unknown' THEN EXCLUDED.kind
                ELSE entities.kind
            END
        RETURNING id
        "#,
    )
    .bind(surface)
    .bind(wiki_lang)
    .bind(surface)
    .bind(kind)
    .fetch_one(pool)
    .await?;

    sqlx::query(
        r#"
        INSERT INTO entity_aliases (entity_id, surface, language)
        VALUES ($1, $2, $3)
        ON CONFLICT (language, surface, entity_id) DO NOTHING
        "#,
    )
    .bind(id)
    .bind(surface)
    .bind(wiki_lang)
    .execute(pool)
    .await?;

    Ok(id)
}

pub async fn get_entity_kind(pool: &PgPool, entity_id: Uuid) -> anyhow::Result<Option<String>> {
    let kind: Option<String> = sqlx::query_scalar(r#"SELECT kind FROM entities WHERE id = $1"#)
        .bind(entity_id)
        .fetch_optional(pool)
        .await?;
    Ok(kind)
}

pub async fn quality_lifespan_years(
    pool: &PgPool,
    entity_id: Uuid,
) -> anyhow::Result<(Option<i32>, Option<i32>, bool, bool)> {
    let birth: Option<(Option<DateTime<Utc>>,)> = sqlx::query_as(
        r#"
        SELECT start_time FROM canonical_events
        WHERE entity_id = $1 AND event_type = 'birth'
          AND pipeline = 'quality' AND is_active
        LIMIT 1
        "#,
    )
    .bind(entity_id)
    .fetch_optional(pool)
    .await?;

    let death: Option<(Option<DateTime<Utc>>,)> = sqlx::query_as(
        r#"
        SELECT start_time FROM canonical_events
        WHERE entity_id = $1 AND event_type = 'death'
          AND pipeline = 'quality' AND is_active
        LIMIT 1
        "#,
    )
    .bind(entity_id)
    .fetch_optional(pool)
    .await?;

    use chrono::Datelike;
    let birth_year = birth.and_then(|(t,)| t.map(|t| t.year()));
    let death_year = death.and_then(|(t,)| t.map(|t| t.year()));

    Ok((
        birth_year,
        death_year,
        birth_year.is_some(),
        death_year.is_some(),
    ))
}

#[derive(Debug, Clone)]
pub struct QualityEventInsert {
    pub entity_id: Uuid,
    pub event_type: String,
    pub epistemic_status: String,
    pub title: String,
    pub summary: Option<String>,
    pub start_time: Option<DateTime<Utc>>,
    pub time_json: serde_json::Value,
    pub place_label: Option<String>,
    pub place_entity_id: Option<Uuid>,
    pub lat: Option<f64>,
    pub lon: Option<f64>,
    pub confidence: f64,
    pub map_eligible: bool,
    pub historically_valid: bool,
    pub timeline_eligible: bool,
    /// Prefer same value as `occurrence_key` for active quality uniqueness.
    pub fingerprint: String,
    pub occurrence_key: Option<String>,
    pub occurrence_stem: Option<String>,
    pub primary_object: Option<String>,
    pub predicate: String,
    pub assembler_version: String,
    pub event_candidate_id: Uuid,
    pub supersedes: Option<Uuid>,
    pub source_count: i32,
    pub evidence_count: i32,
}

/// Append-only insert for quality pipeline. Never mutates prior rows in place.
pub async fn insert_quality_canonical_event(
    pool: &PgPool,
    event: &QualityEventInsert,
) -> anyhow::Result<Uuid> {
    // If superseding, deactivate old row first (explicit supersession).
    if let Some(old_id) = event.supersedes {
        sqlx::query(
            r#"
            UPDATE canonical_events
            SET is_active = false
            WHERE id = $1 AND pipeline = 'quality'
            "#,
        )
        .bind(old_id)
        .execute(pool)
        .await?;
    }

    let id: Uuid = if event.map_eligible && event.lat.is_some() && event.lon.is_some() {
        sqlx::query_scalar(
            r#"
            INSERT INTO canonical_events (
                entity_id, event_type, epistemic_status, title, summary, start_time, time_json,
                place_label, place_entity_id, geom, confidence, map_eligible,
                historically_valid, timeline_eligible, source_count, evidence_count,
                fingerprint, occurrence_key, occurrence_stem, primary_object, is_active, supersedes, predicate,
                assembler_version, pipeline, event_candidate_id
            )
            VALUES (
                $1,$2,$3,$4,$5,$6,$7,$8,$9,
                ST_SetSRID(ST_MakePoint($10,$11),4326)::geography,
                $12,$13,$14,$15,$16,$17,$18,$19,$20,$21,true,$22,$23,$24,'quality',$25
            )
            RETURNING id
            "#,
        )
        .bind(event.entity_id)
        .bind(&event.event_type)
        .bind(&event.epistemic_status)
        .bind(&event.title)
        .bind(&event.summary)
        .bind(event.start_time)
        .bind(&event.time_json)
        .bind(&event.place_label)
        .bind(event.place_entity_id)
        .bind(event.lon)
        .bind(event.lat)
        .bind(event.confidence)
        .bind(event.map_eligible)
        .bind(event.historically_valid)
        .bind(event.timeline_eligible)
        .bind(event.source_count)
        .bind(event.evidence_count)
        .bind(&event.fingerprint)
        .bind(&event.occurrence_key)
        .bind(&event.occurrence_stem)
        .bind(&event.primary_object)
        .bind(event.supersedes)
        .bind(&event.predicate)
        .bind(&event.assembler_version)
        .bind(event.event_candidate_id)
        .fetch_one(pool)
        .await?
    } else {
        sqlx::query_scalar(
            r#"
            INSERT INTO canonical_events (
                entity_id, event_type, epistemic_status, title, summary, start_time, time_json,
                place_label, place_entity_id, confidence, map_eligible,
                historically_valid, timeline_eligible, source_count, evidence_count,
                fingerprint, occurrence_key, occurrence_stem, primary_object, is_active, supersedes, predicate,
                assembler_version, pipeline, event_candidate_id
            )
            VALUES (
                $1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17,$18,$19,true,$20,$21,$22,'quality',$23
            )
            RETURNING id
            "#,
        )
        .bind(event.entity_id)
        .bind(&event.event_type)
        .bind(&event.epistemic_status)
        .bind(&event.title)
        .bind(&event.summary)
        .bind(event.start_time)
        .bind(&event.time_json)
        .bind(&event.place_label)
        .bind(event.place_entity_id)
        .bind(event.confidence)
        .bind(event.map_eligible)
        .bind(event.historically_valid)
        .bind(event.timeline_eligible)
        .bind(event.source_count)
        .bind(event.evidence_count)
        .bind(&event.fingerprint)
        .bind(&event.occurrence_key)
        .bind(&event.occurrence_stem)
        .bind(&event.primary_object)
        .bind(event.supersedes)
        .bind(&event.predicate)
        .bind(&event.assembler_version)
        .bind(event.event_candidate_id)
        .fetch_one(pool)
        .await?
    };

    if let Some(old_id) = event.supersedes {
        sqlx::query(r#"UPDATE canonical_events SET superseded_by = $1 WHERE id = $2"#)
            .bind(id)
            .bind(old_id)
            .execute(pool)
            .await?;
    }

    Ok(id)
}

pub async fn find_active_quality_event_by_fingerprint(
    pool: &PgPool,
    fingerprint: &str,
) -> anyhow::Result<Option<Uuid>> {
    let id: Option<Uuid> = sqlx::query_scalar(
        r#"
        SELECT id FROM canonical_events
        WHERE fingerprint = $1 AND pipeline = 'quality' AND is_active
        LIMIT 1
        "#,
    )
    .bind(fingerprint)
    .fetch_optional(pool)
    .await?;
    Ok(id)
}

/// Look up an active quality event by shared historical occurrence identity.
pub async fn find_active_quality_event_by_occurrence_key(
    pool: &PgPool,
    entity_id: Uuid,
    occurrence_key: &str,
) -> anyhow::Result<Option<Uuid>> {
    let id: Option<Uuid> = sqlx::query_scalar(
        r#"
        SELECT id FROM canonical_events
        WHERE entity_id = $1
          AND occurrence_key = $2
          AND pipeline = 'quality'
          AND is_active
        LIMIT 1
        "#,
    )
    .bind(entity_id)
    .bind(occurrence_key)
    .fetch_optional(pool)
    .await?;
    Ok(id)
}

/// Reinforce an existing quality event with another source/evidence (no new map point).
pub async fn reinforce_quality_event(pool: &PgPool, event_id: Uuid) -> anyhow::Result<()> {
    sqlx::query(
        r#"
        UPDATE canonical_events SET
            source_count = source_count + 1,
            evidence_count = evidence_count + 1
        WHERE id = $1 AND pipeline = 'quality'
        "#,
    )
    .bind(event_id)
    .execute(pool)
    .await?;
    Ok(())
}

/// Promote timeline event to map-eligible after place resolution (append-only fields only).
pub async fn apply_place_to_quality_event(
    pool: &PgPool,
    event_id: Uuid,
    place_label: &str,
    place_entity_id: Option<Uuid>,
    lat: f64,
    lon: f64,
    precision: &str,
    uncertainty_radius_m: Option<f64>,
) -> anyhow::Result<()> {
    sqlx::query(
        r#"
        UPDATE canonical_events SET
            place_label = $2,
            place_entity_id = $3,
            geom = ST_SetSRID(ST_MakePoint($4, $5), 4326)::geography,
            map_eligible = true,
            location_precision = $6,
            uncertainty_radius_m = $7
        WHERE id = $1 AND pipeline = 'quality' AND is_active
        "#,
    )
    .bind(event_id)
    .bind(place_label)
    .bind(place_entity_id)
    .bind(lon)
    .bind(lat)
    .bind(precision)
    .bind(uncertainty_radius_m)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn find_active_singleton(
    pool: &PgPool,
    entity_id: Uuid,
    event_type: &str,
) -> anyhow::Result<Option<Uuid>> {
    let id: Option<Uuid> = sqlx::query_scalar(
        r#"
        SELECT id FROM canonical_events
        WHERE entity_id = $1 AND event_type = $2
          AND pipeline = 'quality' AND is_active
        LIMIT 1
        "#,
    )
    .bind(entity_id)
    .bind(event_type)
    .fetch_optional(pool)
    .await?;
    Ok(id)
}

pub async fn count_active_quality_by_type(
    pool: &PgPool,
    entity_id: Uuid,
    event_type: &str,
) -> anyhow::Result<i64> {
    let n: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)::bigint FROM canonical_events
        WHERE entity_id = $1 AND event_type = $2
          AND pipeline = 'quality' AND is_active
        "#,
    )
    .bind(entity_id)
    .bind(event_type)
    .fetch_one(pool)
    .await?;
    Ok(n)
}

/// Reject `candidate_id` with `singleton_cardinality_violation` when a quality `birth` or
/// `death` event already exists for the subject.  Returns `true` when the rejection was applied
/// (caller should return early), `false` when the candidate may proceed.
pub async fn reject_if_singleton_exists(
    pool: &PgPool,
    candidate_id: Uuid,
    subject_entity_id: Uuid,
    event_type: &str,
    place_entity_id: Option<Uuid>,
    place_label: Option<&str>,
    place_mentions: &serde_json::Value,
    object_mentions: &serde_json::Value,
    participant_mentions: &serde_json::Value,
) -> anyhow::Result<bool> {
    if event_type != "birth" && event_type != "death" {
        return Ok(false);
    }
    if find_active_singleton(pool, subject_entity_id, event_type)
        .await?
        .is_none()
    {
        return Ok(false);
    }
    update_event_candidate_judgment(
        pool,
        candidate_id,
        "rejected",
        &["singleton_cardinality_violation".into()],
        &serde_json::json!({"at": "assemble"}),
        Some(subject_entity_id),
        place_entity_id,
        place_label,
        place_mentions,
        object_mentions,
        participant_mentions,
    )
    .await?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_document_fragment_binds_metadata_as_tenth_param() {
        let src = include_str!("quality.rs");
        let prod = src.split("#[cfg(test)]").next().expect("prod source");
        let tenth = prod.matches("VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)").count();
        assert_eq!(
            tenth, 3,
            "all three INSERT branches must bind metadata as $10"
        );
        assert!(prod.contains("text, start_offset, end_offset, clause_index, ordinal, metadata"));
    }

    #[test]
    fn document_fragment_insert_carries_json_metadata() {
        let frag = DocumentFragmentInsert {
            snapshot_id: Uuid::nil(),
            fragment_kind: "section".into(),
            parent_fragment_id: None,
            sentence_id: None,
            text: "== Life ==".into(),
            start_offset: 0,
            end_offset: 10,
            clause_index: None,
            ordinal: 0,
            metadata: serde_json::json!({"heading": "Life"}),
        };
        assert_eq!(frag.metadata["heading"], "Life");
        assert_eq!(frag.fragment_kind, "section");
    }

    #[test]
    fn migration_023_extends_fragment_kinds_and_adds_metadata() {
        let sql = include_str!("../../../migrations/023_wiki_fragment_metadata.sql");
        assert!(sql.contains("'section'"));
        assert!(sql.contains("'infobox'"));
        assert!(sql.contains("metadata JSONB NOT NULL DEFAULT '{}'::jsonb"));
        assert!(sql.contains("document_fragments_fragment_kind_check"));
        assert!(sql.contains("document_fragments_clause_check"));
    }
}
