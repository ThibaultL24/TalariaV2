// crates/talaria-store/src/person_events.rs
//! Persist pipeline='person' facts (Explorer) with quote-only evidence.

use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

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

pub async fn reinforce_person_event(pool: &PgPool, event_id: Uuid) -> anyhow::Result<()> {
    sqlx::query(
        r#"
        UPDATE canonical_events SET
            source_count = source_count + 1,
            evidence_count = evidence_count + 1
        WHERE id = $1 AND pipeline = 'person'
        "#,
    )
    .bind(event_id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn insert_person_event(pool: &PgPool, event: &PersonEventInsert) -> anyhow::Result<Uuid> {
    let id: Uuid = if event.map_eligible && event.lat.is_some() && event.lon.is_some() {
        sqlx::query_scalar(
            r#"
            INSERT INTO canonical_events (
                entity_id, event_type, epistemic_status, title, summary, start_time, time_json,
                place_label, geom, confidence, map_eligible,
                historically_valid, timeline_eligible, source_count, evidence_count,
                fingerprint, occurrence_key, occurrence_stem, is_active, predicate,
                assembler_version, pipeline
            )
            VALUES (
                $1,$2,$3,$4,$5,$6,$7,$8,
                ST_SetSRID(ST_MakePoint($9,$10),4326)::geography,
                $11,$12,true,true,1,1,
                $13,$14,$15,true,$16,'person_ingest:v1','person'
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
        .bind(&event.fingerprint)
        .bind(&event.occurrence_key)
        .bind(&event.occurrence_stem)
        .bind(&event.predicate)
        .fetch_one(pool)
        .await?
    } else {
        sqlx::query_scalar(
            r#"
            INSERT INTO canonical_events (
                entity_id, event_type, epistemic_status, title, summary, start_time, time_json,
                place_label, confidence, map_eligible,
                historically_valid, timeline_eligible, source_count, evidence_count,
                fingerprint, occurrence_key, occurrence_stem, is_active, predicate,
                assembler_version, pipeline
            )
            VALUES (
                $1,$2,$3,$4,$5,$6,$7,$8,$9,false,
                true,true,1,1,
                $10,$11,$12,true,$13,'person_ingest:v1','person'
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
        .bind(&event.fingerprint)
        .bind(&event.occurrence_key)
        .bind(&event.occurrence_stem)
        .bind(&event.predicate)
        .fetch_one(pool)
        .await?
    };
    Ok(id)
}

pub async fn insert_person_quote_evidence(
    pool: &PgPool,
    event_id: Uuid,
    quoted_text: &str,
    raw_document_id: Option<Uuid>,
    confidence: f64,
) -> anyhow::Result<Uuid> {
    let id = sqlx::query_scalar(
        r#"
        INSERT INTO event_evidence (
            canonical_event_id, sentence_id, quoted_text, raw_document_id, confidence, evidence_type
        )
        VALUES ($1, NULL, $2, $3, $4, 'llm_quote')
        RETURNING id
        "#,
    )
    .bind(event_id)
    .bind(quoted_text)
    .bind(raw_document_id)
    .bind(confidence)
    .fetch_one(pool)
    .await?;
    Ok(id)
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
