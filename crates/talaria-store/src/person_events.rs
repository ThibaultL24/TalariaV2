// crates/talaria-store/src/person_events.rs
//! Persist pipeline='person' facts (Explorer) with quote-only evidence.

use crate::visit::eligibility_for_event_type;
use chrono::{DateTime, Utc};
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use uuid::Uuid;

/// sha256 hex of `v1|{raw}|{locator}|{quote}` (empty `raw` when document id is None).
pub fn evidence_hash(
    raw_document_id: Option<Uuid>,
    locator: &str,
    quote_or_statement: &str,
) -> String {
    let raw = raw_document_id.map(|id| id.to_string()).unwrap_or_default();
    let payload = format!("v1|{raw}|{locator}|{quote_or_statement}");
    hex::encode(Sha256::digest(payload.as_bytes()))
}

#[derive(Debug, Clone)]
pub struct PersonEventInsert {
    pub entity_id: Uuid,
    pub event_type: String,
    pub epistemic_status: String,
    pub title: String,
    pub summary: Option<String>,
    pub start_time: Option<DateTime<Utc>>,
    pub time_json: serde_json::Value,
    pub place_label: Option<String>,
    pub lat: Option<f64>,
    pub lon: Option<f64>,
    pub confidence: f64,
    pub map_eligible: bool,
    pub fingerprint: String,
    pub occurrence_key: String,
    pub occurrence_stem: Option<String>,
    pub predicate: String,
    /// Wikidata QID for the resolved place identity (from TGN, WHG, or Wikidata search).
    /// Separate from geocoding — identity resolution establishes this before coordinates.
    pub place_identity_qid: Option<String>,
}

pub async fn find_active_person_event_by_occurrence(
    pool: &PgPool,
    entity_id: Uuid,
    occurrence_key: &str,
) -> anyhow::Result<Option<Uuid>> {
    let id = sqlx::query_scalar(
        r#"
        SELECT id FROM canonical_events
        WHERE entity_id = $1 AND occurrence_key = $2
          AND pipeline = 'person' AND is_active
        LIMIT 1
        "#,
    )
    .bind(entity_id)
    .bind(occurrence_key)
    .fetch_optional(pool)
    .await?;
    Ok(id)
}

/// Exact display-title match — catches re-ingests that diverge on place label / occurrence_key.
pub async fn find_active_person_event_by_title(
    pool: &PgPool,
    entity_id: Uuid,
    title: &str,
) -> anyhow::Result<Option<Uuid>> {
    let trimmed = title.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    let id = sqlx::query_scalar(
        r#"
        SELECT id FROM canonical_events
        WHERE entity_id = $1
          AND pipeline = 'person' AND is_active
          AND lower(trim(title)) = lower(trim($2))
        ORDER BY map_eligible DESC, evidence_count DESC NULLS LAST, created_at ASC
        LIMIT 1
        "#,
    )
    .bind(entity_id)
    .bind(trimmed)
    .fetch_optional(pool)
    .await?;
    Ok(id)
}

/// An exact occurrence reading. A shared title or a different key is not a match.
pub async fn find_recognized_person_event(
    pool: &PgPool,
    entity_id: Uuid,
    occurrence_key: &str,
) -> anyhow::Result<Option<Uuid>> {
    if let Some(id) =
        find_active_person_event_by_occurrence(pool, entity_id, occurrence_key).await?
    {
        return Ok(Some(id));
    }
    let fingerprint = format!("{entity_id}|{occurrence_key}");
    find_active_person_event_by_fingerprint(pool, &fingerprint).await
}

/// Global person fingerprint — unique index is not per-entity.
pub async fn find_active_person_event_by_fingerprint(
    pool: &PgPool,
    fingerprint: &str,
) -> anyhow::Result<Option<Uuid>> {
    let id = sqlx::query_scalar(
        r#"
        SELECT id FROM canonical_events
        WHERE fingerprint = $1 AND pipeline = 'person' AND is_active
        LIMIT 1
        "#,
    )
    .bind(fingerprint)
    .fetch_optional(pool)
    .await?;
    Ok(id)
}

pub async fn insert_person_event(pool: &PgPool, event: &PersonEventInsert) -> anyhow::Result<Uuid> {
    let (timeline_eligible, visit_eligible) = eligibility_for_event_type(&event.event_type);
    let mut tx = pool.begin().await?;
    let logical_event_id =
        crate::logical_events::create_logical_event(&mut *tx, Some(event.occurrence_key.as_str()))
            .await?;
    let id: Uuid = if event.map_eligible && event.lat.is_some() && event.lon.is_some() {
        sqlx::query_scalar(
            r#"
            INSERT INTO canonical_events (
                entity_id, event_type, epistemic_status, title, summary, start_time, time_json,
                place_label, geom, confidence, map_eligible,
                historically_valid, timeline_eligible, visit_eligible, source_count, evidence_count,
                fingerprint, occurrence_key, occurrence_stem, is_active, predicate,
                assembler_version, pipeline, place_identity_qid, logical_event_id
            )
            VALUES (
                $1,$2,$3,$4,$5,$6,$7,$8,
                ST_SetSRID(ST_MakePoint($9,$10),4326)::geography,
                $11,$12,true,$13,$14,1,1,
                $15,$16,$17,true,$18,'person_ingest:v1','person',$19,$20
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
        .bind(event.lon)
        .bind(event.lat)
        .bind(event.confidence)
        .bind(event.map_eligible)
        .bind(timeline_eligible)
        .bind(visit_eligible)
        .bind(&event.fingerprint)
        .bind(&event.occurrence_key)
        .bind(&event.occurrence_stem)
        .bind(&event.predicate)
        .bind(&event.place_identity_qid)
        .bind(logical_event_id)
        .fetch_one(&mut *tx)
        .await?
    } else {
        sqlx::query_scalar(
            r#"
            INSERT INTO canonical_events (
                entity_id, event_type, epistemic_status, title, summary, start_time, time_json,
                place_label, confidence, map_eligible,
                historically_valid, timeline_eligible, visit_eligible, source_count, evidence_count,
                fingerprint, occurrence_key, occurrence_stem, is_active, predicate,
                assembler_version, pipeline, place_identity_qid, logical_event_id
            )
            VALUES (
                $1,$2,$3,$4,$5,$6,$7,$8,$9,false,
                true,$10,$11,1,1,
                $12,$13,$14,true,$15,'person_ingest:v1','person',$16,$17
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
        .bind(event.confidence)
        .bind(timeline_eligible)
        .bind(visit_eligible)
        .bind(&event.fingerprint)
        .bind(&event.occurrence_key)
        .bind(&event.occurrence_stem)
        .bind(&event.predicate)
        .bind(&event.place_identity_qid)
        .bind(logical_event_id)
        .fetch_one(&mut *tx)
        .await?
    };
    tx.commit().await?;
    Ok(id)
}

#[derive(Debug, Clone)]
pub struct PersonCandidateInsert {
    pub subject_surface: String,
    pub subject_entity_id: Uuid,
    pub event_type: String,
    pub predicate: String,
    pub time_json: serde_json::Value,
    pub place_label: Option<String>,
    pub evidence_ptrs: serde_json::Value,
    pub extractor_version: String,
    pub fingerprint: String,
    pub occurrence_key: String,
    pub primary_object: Option<String>,
    pub action_role: Option<String>,
    pub status: String,
    pub rejection_codes: Vec<String>,
    pub judgment_json: serde_json::Value,
    pub raw_document_id: Uuid,
}

/// Insert an `event_candidates` row for person ingest (nullable snapshot/fragment).
pub async fn insert_person_candidate(
    pool: &PgPool,
    c: &PersonCandidateInsert,
) -> anyhow::Result<Uuid> {
    let row: Option<(Uuid,)> = sqlx::query_as(
        r#"
        INSERT INTO event_candidates (
            snapshot_id, fragment_id, clause_index,
            subject_surface, subject_entity_id, event_type, predicate, time_json,
            place_mentions, object_mentions, participant_mentions,
            place_entity_id, place_label, evidence_ptrs,
            extractor_version, fingerprint, occurrence_key, primary_object, action_role,
            status, rejection_codes, judgment_json, raw_document_id
        )
        VALUES (
            NULL, NULL, 0,
            $1,$2,$3,$4,$5,'[]'::jsonb,'[]'::jsonb,'[]'::jsonb,
            NULL,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16
        )
        ON CONFLICT (fingerprint) DO NOTHING
        RETURNING id
        "#,
    )
    .bind(&c.subject_surface)
    .bind(c.subject_entity_id)
    .bind(&c.event_type)
    .bind(&c.predicate)
    .bind(&c.time_json)
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
    .bind(c.raw_document_id)
    .fetch_optional(pool)
    .await?;
    if let Some((id,)) = row {
        return Ok(id);
    }
    let existing: Uuid =
        sqlx::query_scalar(r#"SELECT id FROM event_candidates WHERE fingerprint = $1"#)
            .bind(&c.fingerprint)
            .fetch_one(pool)
            .await?;
    Ok(existing)
}

pub async fn insert_person_quote_evidence(
    pool: &PgPool,
    event_id: Uuid,
    quoted_text: &str,
    raw_document_id: Option<Uuid>,
    confidence: f64,
    source_locator: &str,
) -> anyhow::Result<Uuid> {
    let locator = source_locator;
    let hash = evidence_hash(raw_document_id, locator, quoted_text);
    let inserted: Option<Uuid> = sqlx::query_scalar(
        r#"
        INSERT INTO event_evidence (
            canonical_event_id, sentence_id, quoted_text, raw_document_id,
            confidence, evidence_type, source_locator, evidence_hash
        )
        VALUES ($1, NULL, $2, $3, $4, $5, $6, $7)
        ON CONFLICT ON CONSTRAINT uq_event_evidence_dedup DO NOTHING
        RETURNING id
        "#,
    )
    .bind(event_id)
    .bind(quoted_text)
    .bind(raw_document_id)
    .bind(confidence)
    .bind("llm_quote")
    .bind(locator)
    .bind(&hash)
    .fetch_optional(pool)
    .await?;
    if let Some(id) = inserted {
        return Ok(id);
    }
    let existing = sqlx::query_scalar(
        r#"
        SELECT id FROM event_evidence
        WHERE canonical_event_id = $1
          AND raw_document_id IS NOT DISTINCT FROM $2
          AND evidence_hash = $3
        LIMIT 1
        "#,
    )
    .bind(event_id)
    .bind(raw_document_id)
    .bind(&hash)
    .fetch_one(pool)
    .await?;
    Ok(existing)
}

pub async fn upsert_raw_wikipedia_document(
    pool: &PgPool,
    uri: &str,
    title: &str,
    language: &str,
    text: &str,
) -> anyhow::Result<Uuid> {
    let id = sqlx::query_scalar(
        r#"
        INSERT INTO raw_documents (source_type, source_uri, title, language, payload)
        VALUES ('wikipedia', $1, $2, $3, jsonb_build_object('text', $4::text))
        ON CONFLICT (source_type, source_uri)
        DO UPDATE SET payload = EXCLUDED.payload, title = EXCLUDED.title
        RETURNING id
        "#,
    )
    .bind(uri)
    .bind(title)
    .bind(language)
    .bind(text)
    .fetch_one(pool)
    .await?;
    Ok(id)
}

/// The whole article, its revision, and the section index. Passages stay on the evidence row.
pub async fn upsert_raw_wikipedia_article(
    pool: &PgPool,
    uri: &str,
    title: &str,
    language: &str,
    text: &str,
    revision: Option<i64>,
    sections: &serde_json::Value,
) -> anyhow::Result<Uuid> {
    let id = sqlx::query_scalar(
        r#"
        INSERT INTO raw_documents (source_type, source_uri, title, language, payload)
        VALUES (
            'wikipedia', $1, $2, $3,
            jsonb_build_object('text', $4::text, 'revision', $5::bigint, 'sections', $6::jsonb)
        )
        ON CONFLICT (source_type, source_uri)
        DO UPDATE SET payload = EXCLUDED.payload, title = EXCLUDED.title
        RETURNING id
        "#,
    )
    .bind(uri)
    .bind(title)
    .bind(language)
    .bind(text)
    .bind(revision)
    .bind(sections)
    .fetch_one(pool)
    .await?;
    Ok(id)
}

pub async fn upsert_raw_wikidata_document(
    pool: &PgPool,
    uri: &str,
    title: &str,
    text: &str,
) -> anyhow::Result<Uuid> {
    let id = sqlx::query_scalar(
        r#"
        INSERT INTO raw_documents (source_type, source_uri, title, language, payload)
        VALUES ('wikidata', $1, $2, 'en', jsonb_build_object('text', $3::text))
        ON CONFLICT (source_type, source_uri)
        DO UPDATE SET payload = EXCLUDED.payload, title = EXCLUDED.title
        RETURNING id
        "#,
    )
    .bind(uri)
    .bind(title)
    .bind(text)
    .fetch_one(pool)
    .await?;
    Ok(id)
}

pub async fn person_density_counts(pool: &PgPool, entity_id: Uuid) -> anyhow::Result<(i64, i64)> {
    let timeline: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)::bigint FROM canonical_events
        WHERE entity_id = $1 AND pipeline = 'person' AND is_active AND timeline_eligible
        "#,
    )
    .bind(entity_id)
    .fetch_one(pool)
    .await?;
    let map: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)::bigint FROM canonical_events
        WHERE entity_id = $1 AND pipeline = 'person' AND is_active AND map_eligible
        "#,
    )
    .bind(entity_id)
    .fetch_one(pool)
    .await?;
    Ok((timeline, map))
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct PersonEvent {
    pub id: Uuid,
    pub entity_id: Uuid,
    pub event_type: String,
    pub title: Option<String>,
    pub place_label: Option<String>,
    pub start_time: Option<DateTime<Utc>>,
    pub occurrence_key: Option<String>,
}

pub async fn find_active_person_events_for_entity(
    pool: &PgPool,
    entity_id: Uuid,
) -> anyhow::Result<Vec<PersonEvent>> {
    let events: Vec<PersonEvent> = sqlx::query_as(
        r#"
        SELECT id, entity_id, event_type, title, place_label, start_time, occurrence_key
        FROM canonical_events
        WHERE entity_id = $1 AND pipeline = 'person' AND is_active
        ORDER BY start_time ASC NULLS LAST
        "#,
    )
    .bind(entity_id)
    .fetch_all(pool)
    .await?;
    Ok(events)
}

pub async fn upsert_raw_corpus_document(
    pool: &PgPool,
    source_type: &str,
    external_id: &str,
    canonical_url: &str,
    title: &str,
    language: Option<&str>,
    payload: &serde_json::Value,
) -> anyhow::Result<Uuid> {
    let id = sqlx::query_scalar(
        r#"
        INSERT INTO raw_documents (source_type, source_uri, title, language, payload)
        VALUES ($1, $2, $3, $4, $5)
        ON CONFLICT (source_type, source_uri)
        DO UPDATE SET payload = EXCLUDED.payload, title = EXCLUDED.title
        RETURNING id
        "#,
    )
    .bind(source_type)
    .bind(canonical_url)
    .bind(title)
    .bind(language.unwrap_or("en"))
    .bind(payload)
    .fetch_one(pool)
    .await?;
    Ok(id)
}

#[derive(Debug, Clone, Default)]
pub struct ExtendedDensityCounts {
    pub timeline_events: i64,
    pub map_pins: i64,
    pub precise_dates: i64,
    pub precise_coords: i64,
    pub evidence_count: i64,
}

pub async fn person_extended_density_counts(
    pool: &PgPool,
    entity_id: Uuid,
) -> anyhow::Result<ExtendedDensityCounts> {
    let row: (i64, i64, i64, i64) = sqlx::query_as(
        r#"
        SELECT 
            COUNT(*) FILTER (WHERE timeline_eligible)::bigint as timeline,
            COUNT(*) FILTER (WHERE map_eligible)::bigint as map,
            COUNT(*) FILTER (WHERE date_precision IN ('day', 'month'))::bigint as precise_dates,
            COUNT(*) FILTER (WHERE coord_precision_level IN ('point', 'city'))::bigint as precise_coords
        FROM canonical_events
        WHERE entity_id = $1 AND pipeline = 'person' AND is_active
        "#,
    )
    .bind(entity_id)
    .fetch_one(pool)
    .await?;

    let evidence: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)::bigint FROM event_evidence ee
        JOIN canonical_events ce ON ee.event_id = ce.id
        WHERE ce.entity_id = $1 AND ce.pipeline = 'person' AND ce.is_active
        "#,
    )
    .bind(entity_id)
    .fetch_one(pool)
    .await?;

    Ok(ExtendedDensityCounts {
        timeline_events: row.0,
        map_pins: row.1,
        precise_dates: row.2,
        precise_coords: row.3,
        evidence_count: evidence,
    })
}

/// A correction of one published person event. The quote must already be stored as evidence.
#[derive(Debug, Clone)]
pub struct PersonSupersession {
    pub title: String,
    pub event_type: String,
    pub time_json: serde_json::Value,
    pub start_time: Option<DateTime<Utc>>,
    pub place_label: Option<String>,
    pub lat: Option<f64>,
    pub lon: Option<f64>,
    pub map_eligible: bool,
    pub place_identity_qid: Option<String>,
    pub evidence_quote: String,
    /// Fingerprint of the active row. A different value means the row changed.
    pub expected_fingerprint: Option<String>,
    /// Queryable copy of `time_json` precision. The typed date stays in `time_json`.
    pub date_precision: Option<String>,
    /// How closely the coordinates identify the event. A town reference is `approximate`.
    pub location_precision: Option<String>,
    pub uncertainty_radius_m: Option<f64>,
    /// `point` is a situated place. `city` is only the reference town.
    pub coord_precision_level: Option<String>,
}

#[derive(Debug, sqlx::FromRow)]
struct ActivePersonEvent {
    entity_id: Uuid,
    event_type: String,
    epistemic_status: String,
    summary: Option<String>,
    time_json: serde_json::Value,
    place_label: Option<String>,
    lat: Option<f64>,
    lon: Option<f64>,
    confidence: f64,
    fingerprint: Option<String>,
    occurrence_key: Option<String>,
    occurrence_stem: Option<String>,
    predicate: Option<String>,
    is_active: bool,
    logical_event_id: Option<Uuid>,
}

/// Append a successor and keep the previous row. A second call returns the same successor.
pub async fn supersede_person_event(
    pool: &PgPool,
    event_id: Uuid,
    correction: &PersonSupersession,
) -> anyhow::Result<Uuid> {
    let mut tx = pool.begin().await?;
    let stored = sqlx::query_as::<_, ActivePersonEvent>(
        r#"
        SELECT entity_id, event_type, epistemic_status, summary, time_json, place_label,
               ST_Y(geom::geometry) AS lat, ST_X(geom::geometry) AS lon, confidence,
               fingerprint, occurrence_key, occurrence_stem, predicate, is_active,
               logical_event_id
        FROM canonical_events
        WHERE id = $1 AND pipeline = 'person'
        FOR UPDATE
        "#,
    )
    .bind(event_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| anyhow::anyhow!("event_not_found"))?;
    if !stored.is_active {
        return match active_successor(&mut tx, event_id).await? {
            Some(successor) if successor_matches(&successor, correction) => Ok(successor.id),
            Some(_) => anyhow::bail!("active_correction_differs"),
            None => anyhow::bail!("event_is_not_active"),
        };
    }
    if let Some(expected) = &correction.expected_fingerprint {
        if stored.fingerprint.as_deref() != Some(expected.as_str()) {
            anyhow::bail!("event_version_conflict");
        }
    }
    let quotes: Vec<String> = sqlx::query_scalar(
        r#"
        SELECT quoted_text FROM event_evidence
        WHERE canonical_event_id = $1 AND quoted_text IS NOT NULL
        "#,
    )
    .bind(event_id)
    .fetch_all(&mut *tx)
    .await?;
    if correction.evidence_quote.chars().count() < 20
        || !quotes
            .iter()
            .any(|quote| quote.contains(&correction.evidence_quote))
        || !correction_is_supported(&stored, correction)
        || prose_day_contradiction(&quotes, &correction.time_json)
    {
        anyhow::bail!("evidence_does_not_support_correction");
    }
    sqlx::query("UPDATE canonical_events SET is_active = false WHERE id = $1")
        .bind(event_id)
        .execute(&mut *tx)
        .await?;
    let (timeline_eligible, visit_eligible) = eligibility_for_event_type(&correction.event_type);
    let logical_event_id = match stored.logical_event_id {
        Some(id) => id,
        None => {
            let id = crate::logical_events::create_logical_event(
                &mut *tx,
                stored.occurrence_key.as_deref(),
            )
            .await?;
            sqlx::query(
                "UPDATE canonical_events SET logical_event_id = $1 WHERE id = $2 AND logical_event_id IS NULL",
            )
            .bind(id)
            .bind(event_id)
            .execute(&mut *tx)
            .await?;
            id
        }
    };
    let fingerprint = format!(
        "{}:supersession",
        stored.fingerprint.unwrap_or_else(|| event_id.to_string())
    );
    let new_id: Uuid =
        if correction.map_eligible && correction.lat.is_some() && correction.lon.is_some() {
            sqlx::query_scalar(
                r#"
            INSERT INTO canonical_events (
                entity_id, event_type, epistemic_status, title, summary, start_time, time_json,
                place_label, geom, confidence, map_eligible,
                historically_valid, timeline_eligible, visit_eligible, source_count, evidence_count,
                fingerprint, occurrence_key, occurrence_stem, is_active, supersedes, predicate,
                assembler_version, pipeline, place_identity_qid,
                date_precision, location_precision, uncertainty_radius_m, coord_precision_level,
                logical_event_id
            )
            VALUES (
                $1,$2,$3,$4,$5,$6,$7,$8,
                ST_SetSRID(ST_MakePoint($9,$10),4326)::geography,
                $11,$12,true,$13,$14,1,1,
                $15,$16,$17,true,$18,$19,'person_correction:v1','person',$20,
                $21,$22,$23,$24,$25
            )
            RETURNING id
            "#,
            )
            .bind(stored.entity_id)
            .bind(&correction.event_type)
            .bind(&stored.epistemic_status)
            .bind(&correction.title)
            .bind(&stored.summary)
            .bind(correction.start_time)
            .bind(&correction.time_json)
            .bind(&correction.place_label)
            .bind(correction.lon)
            .bind(correction.lat)
            .bind(stored.confidence)
            .bind(correction.map_eligible)
            .bind(timeline_eligible)
            .bind(visit_eligible)
            .bind(&fingerprint)
            .bind(&stored.occurrence_key)
            .bind(&stored.occurrence_stem)
            .bind(event_id)
            .bind(stored.predicate.unwrap_or_else(|| "corrected".into()))
            .bind(&correction.place_identity_qid)
            .bind(&correction.date_precision)
            .bind(&correction.location_precision)
            .bind(correction.uncertainty_radius_m)
            .bind(&correction.coord_precision_level)
            .bind(logical_event_id)
            .fetch_one(&mut *tx)
            .await?
        } else {
            sqlx::query_scalar(
                r#"
            INSERT INTO canonical_events (
                entity_id, event_type, epistemic_status, title, summary, start_time, time_json,
                place_label, confidence, map_eligible,
                historically_valid, timeline_eligible, visit_eligible, source_count, evidence_count,
                fingerprint, occurrence_key, occurrence_stem, is_active, supersedes, predicate,
                assembler_version, pipeline, place_identity_qid,
                date_precision, location_precision, uncertainty_radius_m, coord_precision_level,
                logical_event_id
            )
            VALUES (
                $1,$2,$3,$4,$5,$6,$7,$8,$9,false,true,$10,$11,1,1,
                $12,$13,$14,true,$15,$16,'person_correction:v1','person',$17,
                $18,$19,$20,$21,$22
            )
            RETURNING id
            "#,
            )
            .bind(stored.entity_id)
            .bind(&correction.event_type)
            .bind(&stored.epistemic_status)
            .bind(&correction.title)
            .bind(&stored.summary)
            .bind(correction.start_time)
            .bind(&correction.time_json)
            .bind(&correction.place_label)
            .bind(stored.confidence)
            .bind(timeline_eligible)
            .bind(visit_eligible)
            .bind(&fingerprint)
            .bind(&stored.occurrence_key)
            .bind(&stored.occurrence_stem)
            .bind(event_id)
            .bind(stored.predicate.unwrap_or_else(|| "corrected".into()))
            .bind(&correction.place_identity_qid)
            .bind(&correction.date_precision)
            .bind(&correction.location_precision)
            .bind(correction.uncertainty_radius_m)
            .bind(&correction.coord_precision_level)
            .bind(logical_event_id)
            .fetch_one(&mut *tx)
            .await?
        };
    sqlx::query("UPDATE canonical_events SET superseded_by = $1 WHERE id = $2")
        .bind(new_id)
        .bind(event_id)
        .execute(&mut *tx)
        .await?;
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
        ON CONFLICT ON CONSTRAINT uq_event_evidence_dedup DO NOTHING
        "#,
    )
    .bind(new_id)
    .bind(event_id)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(new_id)
}

pub const FILIATION_NONE: &str = "none";
pub const FILIATION_VERIFIED: &str = "verified";
pub const FILIATION_UNVERIFIED: &str = "unverified";

/// Where a requested person event should be shown.
///
/// `verified` means the walk followed a mutual correction, a parked restoration,
/// or a shared logical identity. `unverified` means an inherited `superseded_by`
/// was left in place and the archive itself is shown.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PersonEventResolution {
    pub requested_id: Uuid,
    pub display_id: Uuid,
    pub filiation: &'static str,
    pub linked_event_id: Option<Uuid>,
}

#[derive(Debug, sqlx::FromRow)]
struct ResolutionRow {
    is_active: bool,
    supersedes: Option<Uuid>,
    superseded_by: Option<Uuid>,
    logical_event_id: Option<Uuid>,
}

async fn load_resolution_row(
    pool: &PgPool,
    event_id: Uuid,
) -> anyhow::Result<Option<ResolutionRow>> {
    let row = sqlx::query_as(
        r#"
        SELECT is_active, supersedes, superseded_by, logical_event_id
        FROM canonical_events
        WHERE id = $1 AND pipeline = 'person'
        "#,
    )
    .bind(event_id)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

fn demonstrated_successor(current: Uuid, row: &ResolutionRow, next: &ResolutionRow) -> bool {
    let same_identity =
        row.logical_event_id.is_some() && row.logical_event_id == next.logical_event_id;
    let mutual = next.supersedes == Some(current);
    let parked = row.supersedes == row.superseded_by && row.superseded_by.is_some();
    same_identity || mutual || parked
}

/// Follow only a demonstrated version link. An inherited one-way `superseded_by`
/// stays on the requested archive. Historical references are not rewritten.
pub async fn resolve_person_event(
    pool: &PgPool,
    event_id: Uuid,
) -> anyhow::Result<PersonEventResolution> {
    let mut current = event_id;
    let mut followed = false;
    for _ in 0..8 {
        let Some(row) = load_resolution_row(pool, current).await? else {
            anyhow::bail!("event_not_found");
        };
        if row.is_active {
            return Ok(PersonEventResolution {
                requested_id: event_id,
                display_id: current,
                filiation: if followed {
                    FILIATION_VERIFIED
                } else {
                    FILIATION_NONE
                },
                linked_event_id: None,
            });
        }
        let Some(next_id) = row.superseded_by else {
            return Ok(PersonEventResolution {
                requested_id: event_id,
                display_id: current,
                filiation: FILIATION_NONE,
                linked_event_id: None,
            });
        };
        let Some(next) = load_resolution_row(pool, next_id).await? else {
            return Ok(PersonEventResolution {
                requested_id: event_id,
                display_id: current,
                filiation: FILIATION_UNVERIFIED,
                linked_event_id: Some(next_id),
            });
        };
        if demonstrated_successor(current, &row, &next) {
            current = next_id;
            followed = true;
            continue;
        }
        return Ok(PersonEventResolution {
            requested_id: event_id,
            display_id: current,
            filiation: FILIATION_UNVERIFIED,
            linked_event_id: Some(next_id),
        });
    }
    anyhow::bail!("supersession_chain_too_deep")
}

/// Display id of `resolve_person_event`. Historical references are not rewritten.
pub async fn resolve_active_person_event(pool: &PgPool, event_id: Uuid) -> anyhow::Result<Uuid> {
    Ok(resolve_person_event(pool, event_id).await?.display_id)
}

/// Restore the previous active row. The correction stays stored and inactive.
///
/// The restored row no longer points forward. The parked correction keeps
/// `supersedes` (the version it replaced) and `superseded_by` (that same restored
/// version), so resolve can walk back to the active row and a later correction
/// remains reachable. Evidence is left on every version.
pub async fn revert_person_supersession(pool: &PgPool, successor_id: Uuid) -> anyhow::Result<Uuid> {
    let mut tx = pool.begin().await?;
    let previous: Option<Uuid> = sqlx::query_scalar(
        r#"
        SELECT supersedes FROM canonical_events
        WHERE id = $1 AND is_active AND pipeline = 'person'
        FOR UPDATE
        "#,
    )
    .bind(successor_id)
    .fetch_optional(&mut *tx)
    .await?;
    let Some(previous) = previous else {
        anyhow::bail!("successor_is_not_an_active_correction");
    };
    let pointed_here: Option<Option<Uuid>> = sqlx::query_scalar(
        r#"
        SELECT superseded_by FROM canonical_events
        WHERE id = $1 AND pipeline = 'person'
        FOR UPDATE
        "#,
    )
    .bind(previous)
    .fetch_optional(&mut *tx)
    .await?;
    if pointed_here.flatten() != Some(successor_id) {
        anyhow::bail!("supersession_link_is_not_mutual");
    }
    sqlx::query(
        r#"
        UPDATE canonical_events
        SET is_active = false, superseded_by = $2
        WHERE id = $1
        "#,
    )
    .bind(successor_id)
    .bind(previous)
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        r#"
        UPDATE canonical_events
        SET is_active = true, superseded_by = NULL
        WHERE id = $1
        "#,
    )
    .bind(previous)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(previous)
}

#[derive(Debug, sqlx::FromRow)]
struct SuccessorRow {
    id: Uuid,
    event_type: String,
    time_json: serde_json::Value,
    place_label: Option<String>,
    lat: Option<f64>,
    lon: Option<f64>,
    place_identity_qid: Option<String>,
    date_precision: Option<String>,
    location_precision: Option<String>,
    uncertainty_radius_m: Option<f64>,
    coord_precision_level: Option<String>,
}

async fn active_successor(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    event_id: Uuid,
) -> anyhow::Result<Option<SuccessorRow>> {
    let row = sqlx::query_as::<_, SuccessorRow>(
        r#"
        SELECT id, event_type, time_json, place_label,
               ST_Y(geom::geometry) AS lat, ST_X(geom::geometry) AS lon,
               place_identity_qid, date_precision, location_precision,
               uncertainty_radius_m, coord_precision_level
        FROM canonical_events
        WHERE supersedes = $1 AND is_active AND pipeline = 'person'
        LIMIT 1
        "#,
    )
    .bind(event_id)
    .fetch_optional(&mut **tx)
    .await?;
    Ok(row)
}

fn successor_matches(successor: &SuccessorRow, correction: &PersonSupersession) -> bool {
    successor.event_type == correction.event_type
        && successor.time_json == correction.time_json
        && successor.place_label == correction.place_label
        && successor.place_identity_qid == correction.place_identity_qid
        && !points_differ(successor.lat, correction.lat)
        && !points_differ(successor.lon, correction.lon)
        && successor.date_precision == correction.date_precision
        && successor.location_precision == correction.location_precision
        && !points_differ(
            successor.uncertainty_radius_m,
            correction.uncertainty_radius_m,
        )
        && successor.coord_precision_level == correction.coord_precision_level
}

fn correction_is_supported(stored: &ActivePersonEvent, correction: &PersonSupersession) -> bool {
    let quote = correction.evidence_quote.to_lowercase();
    let mut supported = false;
    if correction.event_type != stored.event_type {
        let office = matches!(correction.event_type.as_str(), "office")
            && (quote.contains("duke of")
                || quote.contains("duc de")
                || quote.contains("ennobl")
                || quote.contains("anobli"));
        let battle = correction.event_type == "battle"
            && (quote.contains("fought")
                || quote.contains("defeated")
                || quote.contains("battle of")
                || quote.contains("bataille"));
        if !office && !battle {
            return false;
        }
        supported = true;
    }
    if correction.time_json != stored.time_json {
        if !time_quote_supports(&correction.time_json, &correction.evidence_quote) {
            return false;
        }
        supported = true;
    }
    let place_changed = correction.place_label.as_deref() != stored.place_label.as_deref();
    let point_changed =
        points_differ(stored.lat, correction.lat) || points_differ(stored.lon, correction.lon);
    if place_changed || point_changed {
        let Some(label) = correction.place_label.as_deref() else {
            return false;
        };
        if !quote.contains(&label.to_lowercase()) {
            return false;
        }
        supported = true;
    }
    supported
}

fn points_differ(stored: Option<f64>, proposed: Option<f64>) -> bool {
    match (stored, proposed) {
        (Some(left), Some(right)) => (left - right).abs() > 0.000_1,
        (None, None) => false,
        _ => true,
    }
}

fn prose_day_contradiction(quotes: &[String], time: &serde_json::Value) -> bool {
    if time.get("precision").and_then(|value| value.as_str()) != Some("day") {
        return false;
    }
    let Some(start) = time.get("start").and_then(|value| value.as_str()) else {
        return false;
    };
    quotes.iter().any(|quote| {
        let prose = quote.trim_start();
        if prose.starts_with('[') {
            return false;
        }
        full_days_in(quote).into_iter().any(|found| found != start)
    })
}

fn full_days_in(quote: &str) -> Vec<String> {
    let lower = quote.to_lowercase();
    let months = [
        ("january", "01"),
        ("janvier", "01"),
        ("february", "02"),
        ("février", "02"),
        ("fevrier", "02"),
        ("march", "03"),
        ("mars", "03"),
        ("april", "04"),
        ("avril", "04"),
        ("may", "05"),
        ("mai", "05"),
        ("june", "06"),
        ("juin", "06"),
        ("july", "07"),
        ("juillet", "07"),
        ("august", "08"),
        ("août", "08"),
        ("aout", "08"),
        ("september", "09"),
        ("septembre", "09"),
        ("october", "10"),
        ("octobre", "10"),
        ("november", "11"),
        ("novembre", "11"),
        ("december", "12"),
        ("décembre", "12"),
        ("decembre", "12"),
    ];
    let mut found = Vec::new();
    for (name, number) in months {
        let mut rest = lower.as_str();
        while let Some(at) = rest.find(name) {
            let day = rest[..at].split_whitespace().next_back().and_then(|token| {
                token
                    .trim_matches(|c: char| !c.is_ascii_digit())
                    .parse::<i32>()
                    .ok()
            });
            let year = rest[at + name.len()..]
                .split_whitespace()
                .next()
                .and_then(|token| {
                    let digits: String = token.chars().filter(|c| c.is_ascii_digit()).collect();
                    (digits.len() == 4).then_some(digits)
                });
            if let (Some(day), Some(year)) = (day, year) {
                if (1..=31).contains(&day) {
                    found.push(format!("{year}-{number}-{day:02}"));
                }
            }
            rest = &rest[at + name.len()..];
        }
    }
    found
}

fn time_quote_supports(time: &serde_json::Value, quote: &str) -> bool {
    let Some(start) = time.get("start").and_then(|value| value.as_str()) else {
        return false;
    };
    if start.len() < 4 || !quote.contains(&start[..4]) {
        return false;
    }
    if time.get("precision").and_then(|value| value.as_str()) != Some("day") {
        return true;
    }
    if quote.contains(start) {
        return true;
    }
    let Ok(day) = start.get(8..10).unwrap_or("").parse::<i32>() else {
        return false;
    };
    let months: &[&str] = match start.get(5..7).unwrap_or("") {
        "01" => &["january", "janvier"],
        "02" => &["february", "février", "fevrier"],
        "03" => &["march", "mars"],
        "04" => &["april", "avril"],
        "05" => &["may", "mai"],
        "06" => &["june", "juin"],
        "07" => &["july", "juillet"],
        "08" => &["august", "août", "aout"],
        "09" => &["september", "septembre"],
        "10" => &["october", "octobre"],
        "11" => &["november", "novembre"],
        "12" => &["december", "décembre", "decembre"],
        _ => return false,
    };
    let folded = quote.to_lowercase();
    folded.contains(&day.to_string()) && months.iter().any(|month| folded.contains(month))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_inputs_same_hash() {
        let id = Uuid::nil();
        assert_eq!(
            evidence_hash(Some(id), "span:0-10", "quote"),
            evidence_hash(Some(id), "span:0-10", "quote")
        );
    }

    #[test]
    fn different_quote_different_hash() {
        let id = Uuid::nil();
        assert_ne!(
            evidence_hash(Some(id), "span:0-10", "a"),
            evidence_hash(Some(id), "span:0-10", "b")
        );
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn a_correction_keeps_the_previous_row_and_can_be_reversed(pool: sqlx::PgPool) {
        use chrono::TimeZone;
        let entity = crate::ensure_subject_entity(&pool, "Q9001797", "Napoleon")
            .await
            .unwrap();
        let quote = "Napoleon defeated the Austrian army at Lodi on 10 May 1796.";
        let event_id = insert_person_event(
            &pool,
            &PersonEventInsert {
                entity_id: entity,
                event_type: "battle".into(),
                epistemic_status: "attested".into(),
                title: "Battle of Lodi".into(),
                summary: Some("Battle of Lodi".into()),
                start_time: Some(Utc.with_ymd_and_hms(1796, 1, 1, 0, 0, 0).unwrap()),
                time_json: serde_json::json!({
                    "kind": "exact",
                    "precision": "year",
                    "start": "1796-01-01",
                    "surface": "1796"
                }),
                place_label: Some("Lodi".into()),
                lat: Some(59.6),
                lon: Some(29.67),
                confidence: 0.9,
                map_eligible: true,
                fingerprint: "lodi-correction-fixture".into(),
                occurrence_key: "lodi-correction".into(),
                occurrence_stem: None,
                predicate: "fought_at".into(),
                place_identity_qid: None,
            },
        )
        .await
        .unwrap();
        insert_person_quote_evidence(
            &pool,
            event_id,
            quote,
            None,
            0.9,
            "https://example.test/lodi",
        )
        .await
        .unwrap();
        let rows_before: i64 = sqlx::query_scalar(
            "SELECT count(*)::bigint FROM canonical_events WHERE entity_id = $1",
        )
        .bind(entity)
        .fetch_one(&pool)
        .await
        .unwrap();
        let refused = supersede_person_event(
            &pool,
            event_id,
            &PersonSupersession {
                title: "Battle of Lodi (1800)".into(),
                event_type: "battle".into(),
                time_json: serde_json::json!({
                    "kind": "exact",
                    "precision": "year",
                    "start": "1800-01-01"
                }),
                start_time: Some(Utc.with_ymd_and_hms(1800, 1, 1, 0, 0, 0).unwrap()),
                place_label: Some("Lodi".into()),
                lat: Some(45.314),
                lon: Some(9.503),
                map_eligible: true,
                place_identity_qid: Some("Q6258".into()),
                evidence_quote: quote.into(),
                expected_fingerprint: None,
                date_precision: None,
                location_precision: None,
                uncertainty_radius_m: None,
                coord_precision_level: None,
            },
        )
        .await;
        assert!(refused.is_err());
        let rows_after: i64 = sqlx::query_scalar(
            "SELECT count(*)::bigint FROM canonical_events WHERE entity_id = $1",
        )
        .bind(entity)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(rows_before, rows_after);

        let correction = PersonSupersession {
            title: "Battle of Lodi (1796)".into(),
            event_type: "battle".into(),
            time_json: serde_json::json!({
                "kind": "exact",
                "precision": "day",
                "start": "1796-05-10",
                "surface": "10 May 1796"
            }),
            start_time: Some(Utc.with_ymd_and_hms(1796, 5, 10, 0, 0, 0).unwrap()),
            place_label: Some("Lodi".into()),
            lat: Some(45.314),
            lon: Some(9.503),
            map_eligible: true,
            place_identity_qid: Some("Q6258".into()),
            evidence_quote: quote.into(),
            expected_fingerprint: Some("lodi-correction-fixture".into()),
            date_precision: Some("day".into()),
            location_precision: None,
            uncertainty_radius_m: None,
            coord_precision_level: None,
        };
        let successor = supersede_person_event(&pool, event_id, &correction)
            .await
            .unwrap();
        let repeated = supersede_person_event(&pool, event_id, &correction)
            .await
            .unwrap();
        assert_eq!(successor, repeated);
        let active: i64 = sqlx::query_scalar(
            "SELECT count(*)::bigint FROM canonical_events WHERE entity_id = $1 AND is_active",
        )
        .bind(entity)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(active, 1);
        let (precision, lat): (String, f64) = sqlx::query_as(
            "SELECT time_json->>'precision', ST_Y(geom::geometry) FROM person_timeline_projection WHERE entity_id = $1",
        )
        .bind(entity)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(precision, "day");
        assert!((lat - 45.314).abs() < 0.01);
        let old_quotes: i64 = sqlx::query_scalar(
            "SELECT count(*)::bigint FROM event_evidence WHERE canonical_event_id = $1",
        )
        .bind(event_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        let new_quotes: i64 = sqlx::query_scalar(
            "SELECT count(*)::bigint FROM event_evidence WHERE canonical_event_id = $1",
        )
        .bind(successor)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(old_quotes, 1);
        assert_eq!(new_quotes, 1);
        assert_eq!(
            resolve_active_person_event(&pool, event_id).await.unwrap(),
            successor
        );
        assert_eq!(
            resolve_active_person_event(&pool, successor).await.unwrap(),
            successor
        );
        let restored = revert_person_supersession(&pool, successor).await.unwrap();
        assert_eq!(restored, event_id);
        let (precision, lat): (String, f64) = sqlx::query_as(
            "SELECT time_json->>'precision', ST_Y(geom::geometry) FROM person_timeline_projection WHERE entity_id = $1",
        )
        .bind(entity)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(precision, "year");
        assert!((lat - 59.6).abs() < 0.01);
        let successor_active: bool =
            sqlx::query_scalar("SELECT is_active FROM canonical_events WHERE id = $1")
                .bind(successor)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert!(!successor_active);
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn four_prepared_corrections_apply_repeat_and_revert(pool: sqlx::PgPool) {
        use chrono::TimeZone;
        let entity = crate::ensure_subject_entity(&pool, "Q9001798", "Napoleon")
            .await
            .unwrap();
        let notice = "[OPEN_ALEX] Russia and the Napoleonic Wars, 1807-1815";

        async fn plant(
            pool: &sqlx::PgPool,
            entity: Uuid,
            key: &str,
            place: &str,
            qid: &str,
            lat: f64,
            lon: f64,
            year: i32,
            quote: &str,
            notice: &str,
        ) -> Uuid {
            let id = insert_person_event(
                pool,
                &PersonEventInsert {
                    entity_id: entity,
                    event_type: "battle".into(),
                    epistemic_status: "attested".into(),
                    title: format!("Battle of {place}"),
                    summary: Some(quote.into()),
                    start_time: Some(Utc.with_ymd_and_hms(year, 1, 1, 0, 0, 0).unwrap()),
                    time_json: serde_json::json!({
                        "kind": "exact",
                        "precision": "year",
                        "start": year.to_string(),
                        "surface": year.to_string()
                    }),
                    place_label: Some(place.into()),
                    lat: Some(lat),
                    lon: Some(lon),
                    confidence: 0.9,
                    map_eligible: true,
                    fingerprint: key.into(),
                    occurrence_key: key.into(),
                    occurrence_stem: None,
                    predicate: "fought_at".into(),
                    place_identity_qid: Some(qid.into()),
                },
            )
            .await
            .unwrap();
            insert_person_quote_evidence(pool, id, quote, None, 0.9, key)
                .await
                .unwrap();
            insert_person_quote_evidence(pool, id, notice, None, 0.4, &format!("{key}-notice"))
                .await
                .unwrap();
            id
        }

        let lodi_quote = "Battle of Lodi was fought on Tuesday 10 May 1796 at Lodi, Lombardy.";
        let mondovi_quote = "The Battle of Mondovì was fought on 21 April 1796 between the French army of Napoleon Bonaparte and the army of the Kingdom of Sardinia-Piedmont.";
        let friedland_quote = "On 14 June Napoleon obtained an overwhelming victory over the Russians at the Battle of Friedland.";
        let eylau_quote =
            "Napoleon fought a bloody stalemate at the Battle of Eylau in February 1807.";
        let lodi = plant(
            &pool,
            entity,
            "prep-lodi",
            "Lodi",
            "Q6258",
            45.314,
            9.503,
            1796,
            lodi_quote,
            notice,
        )
        .await;
        let mondovi = plant(
            &pool,
            entity,
            "prep-mondovi",
            "Mondovì",
            "Q50775",
            59.6017,
            29.6686,
            1796,
            mondovi_quote,
            notice,
        )
        .await;
        let friedland = plant(
            &pool,
            entity,
            "prep-friedland",
            "Friedland",
            "Q13431",
            41.9361,
            2.8131,
            1807,
            friedland_quote,
            notice,
        )
        .await;
        let eylau = plant(
            &pool,
            entity,
            "prep-eylau",
            "Eylau",
            "Q13422",
            42.0334,
            2.7371,
            1807,
            eylau_quote,
            notice,
        )
        .await;

        let stale = supersede_person_event(
            &pool,
            lodi,
            &PersonSupersession {
                title: "Battle of Lodi (1796)".into(),
                event_type: "battle".into(),
                time_json: serde_json::json!({"kind":"exact","precision":"day","start":"1796-05-10","surface":"10 May 1796"}),
                start_time: Some(Utc.with_ymd_and_hms(1796, 5, 10, 0, 0, 0).unwrap()),
                place_label: Some("Lodi".into()),
                lat: Some(45.314),
                lon: Some(9.503),
                map_eligible: true,
                place_identity_qid: Some("Q6258".into()),
                evidence_quote: lodi_quote.into(),
                expected_fingerprint: Some("not-the-row".into()),
                date_precision: None,
                location_precision: None,
                uncertainty_radius_m: None,
                coord_precision_level: None,
            },
        )
        .await;
        assert!(stale.is_err());
        let still_year: String = sqlx::query_scalar(
            "SELECT time_json->>'precision' FROM canonical_events WHERE id = $1 AND is_active",
        )
        .bind(lodi)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(still_year, "year");

        let contradicted = plant(
            &pool,
            entity,
            "prep-conflict",
            "Lodi",
            "Q6258",
            45.314,
            9.503,
            1796,
            "The Battle of Lodi was fought on 11 May 1796.",
            notice,
        )
        .await;
        insert_person_quote_evidence(
            &pool,
            contradicted,
            "The Battle of Lodi was fought on 10 May 1796.",
            None,
            0.9,
            "prep-conflict-other",
        )
        .await
        .unwrap();
        let refused = supersede_person_event(
            &pool,
            contradicted,
            &PersonSupersession {
                title: "Battle of Lodi (1796)".into(),
                event_type: "battle".into(),
                time_json: serde_json::json!({"kind":"exact","precision":"day","start":"1796-05-10"}),
                start_time: Some(Utc.with_ymd_and_hms(1796, 5, 10, 0, 0, 0).unwrap()),
                place_label: Some("Lodi".into()),
                lat: Some(45.314),
                lon: Some(9.503),
                map_eligible: true,
                place_identity_qid: Some("Q6258".into()),
                evidence_quote: "The Battle of Lodi was fought on 10 May 1796.".into(),
                expected_fingerprint: Some("prep-conflict".into()),
                date_precision: None,
                location_precision: None,
                uncertainty_radius_m: None,
                coord_precision_level: None,
            },
        )
        .await;
        assert!(refused.is_err());

        let corrections = [
            (
                lodi,
                "prep-lodi",
                "Lodi",
                "Q6258",
                45.314,
                9.503,
                "1796-05-10",
                "day",
                lodi_quote,
            ),
            (
                mondovi,
                "prep-mondovi",
                "Mondovì",
                "Q50775",
                44.3833,
                7.8167,
                "1796-04-21",
                "day",
                mondovi_quote,
            ),
            (
                friedland,
                "prep-friedland",
                "Friedland",
                "Q13431",
                54.443,
                21.011,
                "1807",
                "year",
                friedland_quote,
            ),
            (
                eylau,
                "prep-eylau",
                "Eylau",
                "Q13422",
                54.4,
                20.6333,
                "1807",
                "year",
                eylau_quote,
            ),
        ];
        let mut successors = Vec::new();
        for (id, key, place, qid, lat, lon, start, precision, quote) in corrections {
            let time = if precision == "day" {
                serde_json::json!({"kind":"exact","precision":"day","start":start,"surface":start})
            } else {
                serde_json::json!({"kind":"exact","precision":"year","start":start,"surface":start})
            };
            let year: i32 = start[..4].parse().unwrap();
            let month: u32 = if precision == "day" {
                start[5..7].parse().unwrap()
            } else {
                1
            };
            let day: u32 = if precision == "day" {
                start[8..10].parse().unwrap()
            } else {
                1
            };
            let request = PersonSupersession {
                title: format!("Battle of {place}"),
                event_type: "battle".into(),
                time_json: time,
                start_time: Some(Utc.with_ymd_and_hms(year, month, day, 0, 0, 0).unwrap()),
                place_label: Some(place.into()),
                lat: Some(lat),
                lon: Some(lon),
                map_eligible: true,
                place_identity_qid: Some(qid.into()),
                evidence_quote: quote.into(),
                expected_fingerprint: Some(key.into()),
                date_precision: (precision == "day").then(|| "day".into()),
                location_precision: (place == "Mondovì").then(|| "approximate".into()),
                uncertainty_radius_m: (place == "Mondovì").then_some(5_000.0),
                coord_precision_level: (place == "Mondovì").then(|| "city".into()),
            };
            let successor = supersede_person_event(&pool, id, &request).await.unwrap();
            let repeated = supersede_person_event(&pool, id, &request).await.unwrap();
            assert_eq!(successor, repeated);
            let (active_precision, active_lat, active_type, active_qid): (
                String,
                f64,
                String,
                String,
            ) = sqlx::query_as(
                r#"
                SELECT time_json->>'precision', ST_Y(geom::geometry), event_type, place_identity_qid
                FROM person_timeline_projection WHERE id = $1
                "#,
            )
            .bind(successor)
            .fetch_one(&pool)
            .await
            .unwrap();
            assert_eq!(active_precision, precision);
            assert_eq!(active_type, "battle");
            assert_eq!(active_qid, qid);
            if place == "Mondovì" {
                let (level, radius, located): (String, f64, String) = sqlx::query_as(
                    "SELECT coord_precision_level, uncertainty_radius_m, location_precision FROM canonical_events WHERE id = $1",
                )
                .bind(successor)
                .fetch_one(&pool)
                .await
                .unwrap();
                assert_eq!(level, "city");
                assert_eq!(located, "approximate");
                assert!((radius - 5_000.0).abs() < 0.1);
            }
            assert!((active_lat - lat).abs() < 0.001);
            let old_evidence: i64 = sqlx::query_scalar(
                "SELECT count(*)::bigint FROM event_evidence WHERE canonical_event_id = $1",
            )
            .bind(id)
            .fetch_one(&pool)
            .await
            .unwrap();
            let new_evidence: i64 = sqlx::query_scalar(
                "SELECT count(*)::bigint FROM event_evidence WHERE canonical_event_id = $1",
            )
            .bind(successor)
            .fetch_one(&pool)
            .await
            .unwrap();
            assert_eq!(old_evidence, 2);
            assert_eq!(new_evidence, 2);
            let original_lat = match key {
                "prep-lodi" => 45.314,
                "prep-mondovi" => 59.6017,
                "prep-friedland" => 41.9361,
                _ => 42.0334,
            };
            successors.push((id, successor, original_lat));
        }
        let active: i64 = sqlx::query_scalar(
            "SELECT count(*)::bigint FROM canonical_events WHERE entity_id = $1 AND is_active AND occurrence_key LIKE 'prep-%' AND occurrence_key <> 'prep-conflict'",
        )
        .bind(entity)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(active, 4);
        for (previous, successor, original_lat) in successors {
            let restored = revert_person_supersession(&pool, successor).await.unwrap();
            assert_eq!(restored, previous);
            let (restored_precision, restored_lat): (String, f64) = sqlx::query_as(
                "SELECT time_json->>'precision', ST_Y(geom::geometry) FROM person_timeline_projection WHERE id = $1",
            )
            .bind(previous)
            .fetch_one(&pool)
            .await
            .unwrap();
            assert_eq!(restored_precision, "year");
            assert!(
                (restored_lat - original_lat).abs() < 0.001,
                "{restored_lat} {original_lat}"
            );
        }
        let (lodi_lat, lodi_precision): (f64, String) = sqlx::query_as(
            "SELECT ST_Y(geom::geometry), time_json->>'precision' FROM canonical_events WHERE id = $1 AND is_active",
        )
        .bind(lodi)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(lodi_precision, "year");
        assert!((lodi_lat - 45.314).abs() < 0.001);
        let (mondovi_lat,): (f64,) = sqlx::query_as(
            "SELECT ST_Y(geom::geometry) FROM canonical_events WHERE id = $1 AND is_active",
        )
        .bind(mondovi)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert!((mondovi_lat - 59.6017).abs() < 0.001);
    }

    #[tokio::test]
    #[ignore = "writes the development database: Lodi day precision only"]
    async fn apply_lodi_day_on_development() {
        use chrono::TimeZone;
        let url = std::env::var("DATABASE_URL").expect("DATABASE_URL");
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(2)
            .connect(&url)
            .await
            .expect("development database");
        let database: String = sqlx::query_scalar("SELECT current_database()")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(database, "talaria_engine_development");
        let lodi = Uuid::parse_str("ac73895e-a8b3-4d17-8d45-da4e42e6496e").unwrap();
        let expected_fingerprint = "14eb944a-fa31-459a-be7d-1f32488e2bd6|e87261c853af8f1d574fb528108e697758ef21f6794f3999c7f8c20785c84ec0";
        let before: (bool, String, String, String, f64, f64, String, String) = sqlx::query_as(
            r#"
            SELECT is_active, event_type, title, time_json->>'precision',
                   ST_Y(geom::geometry), ST_X(geom::geometry), place_identity_qid, fingerprint
            FROM canonical_events WHERE id = $1 AND pipeline = 'person'
            "#,
        )
        .bind(lodi)
        .fetch_one(&pool)
        .await
        .expect("Lodi row");
        assert!(before.0, "Lodi is no longer active");
        assert_eq!(before.1, "battle");
        assert_eq!(before.3, "year");
        assert!((before.4 - 45.314).abs() < 0.001);
        assert!((before.5 - 9.503).abs() < 0.001);
        assert_eq!(before.6, "Q6258");
        assert_eq!(before.7, expected_fingerprint);
        assert!(sqlx::query_scalar::<_, Option<Uuid>>(
            "SELECT superseded_by FROM canonical_events WHERE id = $1"
        )
        .bind(lodi)
        .fetch_one(&pool)
        .await
        .unwrap()
        .is_none());
        let quote: String = sqlx::query_scalar(
            r#"
            SELECT quoted_text FROM event_evidence
            WHERE canonical_event_id = $1
              AND quoted_text LIKE '%fought on Tuesday 10 May 1796%'
            LIMIT 1
            "#,
        )
        .bind(lodi)
        .fetch_one(&pool)
        .await
        .expect("the 10 May 1796 proof");
        let others_before: String = sqlx::query_scalar(
            r#"
            SELECT md5(COALESCE(string_agg(md5(concat_ws('|', ce.id::text, ce.time_json::text, coalesce(ST_AsText(ce.geom), ''), ce.event_type)), '' ORDER BY ce.id::text), ''))
            FROM canonical_events ce
            JOIN entities e ON e.id = ce.entity_id
            WHERE e.qid = 'Q517' AND ce.is_active AND ce.pipeline = 'person' AND ce.id <> $1
            "#,
        )
        .bind(lodi)
        .fetch_one(&pool)
        .await
        .unwrap();
        let active_before: i64 = sqlx::query_scalar(
            r#"
            SELECT count(*)::bigint FROM canonical_events ce
            JOIN entities e ON e.id = ce.entity_id
            WHERE e.qid = 'Q517' AND ce.is_active AND ce.pipeline = 'person'
            "#,
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(active_before, 465);
        let evidence_before: i64 = sqlx::query_scalar(
            "SELECT count(*)::bigint FROM event_evidence WHERE canonical_event_id = $1",
        )
        .bind(lodi)
        .fetch_one(&pool)
        .await
        .unwrap();
        let candidate: Uuid =
            sqlx::query_scalar("SELECT id FROM event_candidates WHERE canonical_event_id = $1")
                .bind(lodi)
                .fetch_one(&pool)
                .await
                .unwrap();

        let request = PersonSupersession {
            title: before.2.clone(),
            event_type: "battle".into(),
            time_json: serde_json::json!({
                "kind": "exact",
                "start": "1796-05-10",
                "end": null,
                "precision": "day",
                "calendar": "gregorian",
                "surface": "10 May 1796"
            }),
            start_time: Some(Utc.with_ymd_and_hms(1796, 5, 10, 0, 0, 0).unwrap()),
            place_label: Some("Lodi".into()),
            lat: Some(45.314),
            lon: Some(9.503),
            map_eligible: true,
            place_identity_qid: Some("Q6258".into()),
            evidence_quote: quote,
            expected_fingerprint: Some(expected_fingerprint.into()),
            date_precision: None,
            location_precision: None,
            uncertainty_radius_m: None,
            coord_precision_level: None,
        };
        let successor = match supersede_person_event(&pool, lodi, &request).await {
            Ok(id) => id,
            Err(error) => panic!("Lodi was not written: {error}"),
        };
        let repeated = supersede_person_event(&pool, lodi, &request).await.unwrap();
        let ok = async {
            if repeated != successor {
                anyhow::bail!("repeating the correction created another row");
            }
            if resolve_active_person_event(&pool, lodi).await? != successor
                || resolve_active_person_event(&pool, successor).await? != successor
            {
                anyhow::bail!("the active version cannot be resolved from the previous identifier");
            }
            let (precision, start, event_type, lat, lon, qid, active_title): (
                String,
                String,
                String,
                f64,
                f64,
                String,
                String,
            ) = sqlx::query_as(
                r#"
                SELECT time_json->>'precision', time_json->>'start', event_type,
                       ST_Y(geom::geometry), ST_X(geom::geometry), place_identity_qid, title
                FROM person_timeline_projection WHERE id = $1
                "#,
            )
            .bind(successor)
            .fetch_one(&pool)
            .await
            .unwrap();
            if precision != "day"
                || start != "1796-05-10"
                || event_type != "battle"
                || qid != "Q6258"
                || active_title != before.2
                || (lat - 45.314).abs() > 0.001
                || (lon - 9.503).abs() > 0.001
            {
                anyhow::bail!("active Lodi row does not match the authorised correction");
            }
            let old_active: bool =
                sqlx::query_scalar("SELECT is_active FROM canonical_events WHERE id = $1")
                    .bind(lodi)
                    .fetch_one(&pool)
                    .await
                    .unwrap();
            if old_active {
                anyhow::bail!("the previous Lodi row is still active");
            }
            let old_evidence: i64 = sqlx::query_scalar(
                "SELECT count(*)::bigint FROM event_evidence WHERE canonical_event_id = $1",
            )
            .bind(lodi)
            .fetch_one(&pool)
            .await
            .unwrap();
            let new_evidence: i64 = sqlx::query_scalar(
                "SELECT count(*)::bigint FROM event_evidence WHERE canonical_event_id = $1",
            )
            .bind(successor)
            .fetch_one(&pool)
            .await
            .unwrap();
            if old_evidence != evidence_before || new_evidence != evidence_before {
                anyhow::bail!("evidence was not kept on both versions");
            }
            let shown: i64 = sqlx::query_scalar(
                "SELECT count(*)::bigint FROM person_timeline_projection WHERE id = $1",
            )
            .bind(lodi)
            .fetch_one(&pool)
            .await
            .unwrap();
            if shown != 0 {
                anyhow::bail!("the previous Lodi row is still on the timeline");
            }
            let active_after: i64 = sqlx::query_scalar(
                r#"
                SELECT count(*)::bigint FROM canonical_events ce
                JOIN entities e ON e.id = ce.entity_id
                WHERE e.qid = 'Q517' AND ce.is_active AND ce.pipeline = 'person'
                "#,
            )
            .fetch_one(&pool)
            .await
            .unwrap();
            if active_after != 465 {
                anyhow::bail!("the active Napoleon count changed from 465 to {active_after}");
            }
            let others_after: String = sqlx::query_scalar(
                r#"
                SELECT md5(COALESCE(string_agg(md5(concat_ws('|', ce.id::text, ce.time_json::text, coalesce(ST_AsText(ce.geom), ''), ce.event_type)), '' ORDER BY ce.id::text), ''))
                FROM canonical_events ce
                JOIN entities e ON e.id = ce.entity_id
                WHERE e.qid = 'Q517' AND ce.is_active AND ce.pipeline = 'person' AND ce.id <> $1
                "#,
            )
            .bind(successor)
            .fetch_one(&pool)
            .await
            .unwrap();
            if others_before != others_after {
                anyhow::bail!("another Napoleon event changed");
            }
            let candidate_target: Uuid =
                sqlx::query_scalar("SELECT canonical_event_id FROM event_candidates WHERE id = $1")
                    .bind(candidate)
                    .fetch_one(&pool)
                    .await
                    .unwrap();
            if candidate_target != lodi {
                anyhow::bail!("the historical candidate pointer was rewritten");
            }
            println!("lodi_successor={successor}");
            Ok(())
        };
        if let Err(error) = ok.await {
            let _ = revert_person_supersession(&pool, successor).await;
            panic!("Lodi correction failed and was restored: {error}");
        }
    }

    #[tokio::test]
    #[ignore = "writes the development database: Mondovì day and city reference only"]
    async fn apply_mondovi_day_and_city_on_development() {
        use chrono::TimeZone;
        const CITY_REFERENCE_LAT: f64 = 44.3833;
        const CITY_REFERENCE_LON: f64 = 7.8167;
        const CITY_REFERENCE_RADIUS_M: f64 = 5_000.0;
        let url = std::env::var("DATABASE_URL").expect("DATABASE_URL");
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(2)
            .connect(&url)
            .await
            .expect("development database");
        let database: String = sqlx::query_scalar("SELECT current_database()")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(database, "talaria_engine_development");
        let mondovi = Uuid::parse_str("7b618070-7668-4102-9dea-5c3937c25b8c").unwrap();
        let expected_fingerprint = "14eb944a-fa31-459a-be7d-1f32488e2bd6|c01d4d8f172b826d6c60ab23ef5f95357e15bdd263aca36edf24bb1838f12f85";
        let before: (bool, String, String, String, f64, f64, String, String) = sqlx::query_as(
            r#"
            SELECT is_active, event_type, title, time_json->>'precision',
                   ST_Y(geom::geometry), ST_X(geom::geometry), place_identity_qid, fingerprint
            FROM canonical_events WHERE id = $1 AND pipeline = 'person'
            "#,
        )
        .bind(mondovi)
        .fetch_one(&pool)
        .await
        .expect("Mondovì row");
        assert!(before.0, "Mondovì is no longer active");
        assert_eq!(before.1, "battle");
        assert_eq!(before.3, "year");
        assert!(
            (before.4 - 59.6017).abs() < 0.001,
            "the stored point is no longer the Russian coordinate"
        );
        assert!(
            (before.5 - 29.6686).abs() < 0.001,
            "the stored point is no longer the Russian coordinate"
        );
        assert_eq!(before.6, "Q50775");
        assert_eq!(before.7, expected_fingerprint);
        assert!(sqlx::query_scalar::<_, Option<Uuid>>(
            "SELECT superseded_by FROM canonical_events WHERE id = $1"
        )
        .bind(mondovi)
        .fetch_one(&pool)
        .await
        .unwrap()
        .is_none());
        let prose_count: i64 = sqlx::query_scalar(
            r#"
            SELECT count(*)::bigint FROM event_evidence
            WHERE canonical_event_id = $1 AND left(btrim(quoted_text), 1) <> '['
            "#,
        )
        .bind(mondovi)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(
            prose_count, 1,
            "a new prose passage appeared; the correction needs another reading"
        );
        let quote: String = sqlx::query_scalar(
            r#"
            SELECT quoted_text FROM event_evidence
            WHERE canonical_event_id = $1 AND left(btrim(quoted_text), 1) <> '['
            "#,
        )
        .bind(mondovi)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert!(quote.contains("21 April 1796"));
        assert!(quote.contains("Mondovì"));
        assert!(quote.contains("Sardinia"));
        assert!(!quote.to_lowercase().contains("russia"));
        let others_before: String = sqlx::query_scalar(
            r#"
            SELECT md5(COALESCE(string_agg(md5(concat_ws('|', ce.id::text, ce.time_json::text, coalesce(ST_AsText(ce.geom), ''), ce.event_type)), '' ORDER BY ce.id::text), ''))
            FROM canonical_events ce
            JOIN entities e ON e.id = ce.entity_id
            WHERE e.qid = 'Q517' AND ce.is_active AND ce.pipeline = 'person' AND ce.id <> $1
            "#,
        )
        .bind(mondovi)
        .fetch_one(&pool)
        .await
        .unwrap();
        let active_before: i64 = sqlx::query_scalar(
            r#"
            SELECT count(*)::bigint FROM canonical_events ce
            JOIN entities e ON e.id = ce.entity_id
            WHERE e.qid = 'Q517' AND ce.is_active AND ce.pipeline = 'person'
            "#,
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(active_before, 465);
        let evidence_before: i64 = sqlx::query_scalar(
            "SELECT count(*)::bigint FROM event_evidence WHERE canonical_event_id = $1",
        )
        .bind(mondovi)
        .fetch_one(&pool)
        .await
        .unwrap();
        let candidate: Uuid =
            sqlx::query_scalar("SELECT id FROM event_candidates WHERE canonical_event_id = $1")
                .bind(mondovi)
                .fetch_one(&pool)
                .await
                .unwrap();
        let request = PersonSupersession {
            title: before.2.clone(),
            event_type: "battle".into(),
            time_json: serde_json::json!({
                "kind": "exact",
                "start": "1796-04-21",
                "end": null,
                "precision": "day",
                "calendar": "gregorian",
                "surface": "21 April 1796"
            }),
            start_time: Some(Utc.with_ymd_and_hms(1796, 4, 21, 0, 0, 0).unwrap()),
            place_label: Some("Mondovì".into()),
            lat: Some(CITY_REFERENCE_LAT),
            lon: Some(CITY_REFERENCE_LON),
            map_eligible: true,
            place_identity_qid: Some("Q50775".into()),
            evidence_quote: quote,
            expected_fingerprint: Some(expected_fingerprint.into()),
            date_precision: Some("day".into()),
            location_precision: Some("approximate".into()),
            uncertainty_radius_m: Some(CITY_REFERENCE_RADIUS_M),
            coord_precision_level: Some("city".into()),
        };
        let successor = match supersede_person_event(&pool, mondovi, &request).await {
            Ok(id) => id,
            Err(error) => panic!("Mondovì was not written: {error}"),
        };
        let repeated = supersede_person_event(&pool, mondovi, &request)
            .await
            .unwrap();
        let ok = async {
            if repeated != successor {
                anyhow::bail!("repeating the correction created another row");
            }
            if resolve_active_person_event(&pool, mondovi).await? != successor {
                anyhow::bail!("the active version cannot be resolved from the previous identifier");
            }
            let (precision, start, event_type, lat, lon, qid, level, radius, located, title): (
                String,
                String,
                String,
                f64,
                f64,
                String,
                String,
                f64,
                String,
                String,
            ) = sqlx::query_as(
                r#"
                SELECT time_json->>'precision', time_json->>'start', event_type,
                       ST_Y(geom::geometry), ST_X(geom::geometry), place_identity_qid,
                       coord_precision_level, uncertainty_radius_m, location_precision, title
                FROM canonical_events WHERE id = $1 AND is_active
                "#,
            )
            .bind(successor)
            .fetch_one(&pool)
            .await
            .unwrap();
            let stored_date: String =
                sqlx::query_scalar("SELECT date_precision FROM canonical_events WHERE id = $1")
                    .bind(successor)
                    .fetch_one(&pool)
                    .await
                    .unwrap();
            if stored_date != "day" {
                anyhow::bail!("the date precision projection was not stored");
            }
            if precision != "day"
                || start != "1796-04-21"
                || event_type != "battle"
                || qid != "Q50775"
                || level != "city"
                || located != "approximate"
                || title != before.2
                || (lat - CITY_REFERENCE_LAT).abs() > 0.001
                || (lon - CITY_REFERENCE_LON).abs() > 0.001
                || (radius - CITY_REFERENCE_RADIUS_M).abs() > 0.1
            {
                anyhow::bail!("active Mondovì row does not match the authorised correction");
            }
            let shown_old: i64 = sqlx::query_scalar(
                "SELECT count(*)::bigint FROM person_timeline_projection WHERE id = $1",
            )
            .bind(mondovi)
            .fetch_one(&pool)
            .await
            .unwrap();
            let shown_new: i64 = sqlx::query_scalar(
                "SELECT count(*)::bigint FROM person_timeline_projection WHERE id = $1",
            )
            .bind(successor)
            .fetch_one(&pool)
            .await
            .unwrap();
            if shown_old != 0 || shown_new != 1 {
                anyhow::bail!("the timeline projection does not keep only the active Mondovì");
            }
            let old_evidence: i64 = sqlx::query_scalar(
                "SELECT count(*)::bigint FROM event_evidence WHERE canonical_event_id = $1",
            )
            .bind(mondovi)
            .fetch_one(&pool)
            .await
            .unwrap();
            let new_evidence: i64 = sqlx::query_scalar(
                "SELECT count(*)::bigint FROM event_evidence WHERE canonical_event_id = $1",
            )
            .bind(successor)
            .fetch_one(&pool)
            .await
            .unwrap();
            if old_evidence != evidence_before || new_evidence != evidence_before {
                anyhow::bail!("evidence was not kept on both versions");
            }
            let active_after: i64 = sqlx::query_scalar(
                r#"
                SELECT count(*)::bigint FROM canonical_events ce
                JOIN entities e ON e.id = ce.entity_id
                WHERE e.qid = 'Q517' AND ce.is_active AND ce.pipeline = 'person'
                "#,
            )
            .fetch_one(&pool)
            .await
            .unwrap();
            if active_after != 465 {
                anyhow::bail!("the active Napoleon count changed from 465 to {active_after}");
            }
            let others_after: String = sqlx::query_scalar(
                r#"
                SELECT md5(COALESCE(string_agg(md5(concat_ws('|', ce.id::text, ce.time_json::text, coalesce(ST_AsText(ce.geom), ''), ce.event_type)), '' ORDER BY ce.id::text), ''))
                FROM canonical_events ce
                JOIN entities e ON e.id = ce.entity_id
                WHERE e.qid = 'Q517' AND ce.is_active AND ce.pipeline = 'person' AND ce.id <> $1
                "#,
            )
            .bind(successor)
            .fetch_one(&pool)
            .await
            .unwrap();
            if others_before != others_after {
                anyhow::bail!("another Napoleon event changed");
            }
            let candidate_target: Uuid =
                sqlx::query_scalar("SELECT canonical_event_id FROM event_candidates WHERE id = $1")
                    .bind(candidate)
                    .fetch_one(&pool)
                    .await
                    .unwrap();
            if candidate_target != mondovi {
                anyhow::bail!("the historical candidate pointer was rewritten");
            }
            println!("mondovi_successor={successor}");
            Ok(())
        };
        if let Err(error) = ok.await {
            let _ = revert_person_supersession(&pool, successor).await;
            panic!("Mondovì correction failed and was restored: {error}");
        }
    }
}
