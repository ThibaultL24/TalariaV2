// crates/talaria-store/src/canonical_events.rs
use crate::entities::{fold_latin_accents, person_match_sql};
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct CanonicalEventInsert {
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
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct CanonicalEventRow {
    pub id: Uuid,
    pub entity_id: Uuid,
    pub person_name: String,
    pub event_type: String,
    pub epistemic_status: String,
    pub title: String,
    pub summary: Option<String>,
    pub start_time: Option<DateTime<Utc>>,
    pub place_label: Option<String>,
    pub confidence: f64,
    pub map_eligible: bool,
    pub lat: Option<f64>,
    pub lon: Option<f64>,
}

pub async fn find_existing_event(
    pool: &PgPool,
    entity_id: Uuid,
    event_type: &str,
    start_time: Option<DateTime<Utc>>,
    place_label: Option<&str>,
) -> anyhow::Result<Option<Uuid>> {
    let id: Option<Uuid> = sqlx::query_scalar(
        r#"
        SELECT id FROM canonical_events
        WHERE entity_id = $1
          AND event_type = $2
          AND start_time IS NOT DISTINCT FROM $3
          AND place_label IS NOT DISTINCT FROM $4
        LIMIT 1
        "#,
    )
    .bind(entity_id)
    .bind(event_type)
    .bind(start_time)
    .bind(place_label)
    .fetch_optional(pool)
    .await?;

    Ok(id)
}

pub async fn insert_canonical_event(
    pool: &PgPool,
    event: &CanonicalEventInsert,
) -> anyhow::Result<Uuid> {
    let id: Uuid = if event.map_eligible {
        sqlx::query_scalar(
            r#"
            INSERT INTO canonical_events (
                entity_id, event_type, epistemic_status, title, summary, start_time, time_json,
                place_label, geom, confidence, map_eligible
            )
            VALUES (
                $1, $2, $3, $4, $5, $6, $7, $8,
                ST_SetSRID(ST_MakePoint($9, $10), 4326)::geography,
                $11, $12
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
        .fetch_one(pool)
        .await?
    } else {
        sqlx::query_scalar(
            r#"
            INSERT INTO canonical_events (
                entity_id, event_type, epistemic_status, title, summary, start_time, time_json,
                place_label, confidence, map_eligible
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
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
        .bind(event.map_eligible)
        .fetch_one(pool)
        .await?
    };

    Ok(id)
}

pub async fn insert_event_evidence(
    pool: &PgPool,
    canonical_event_id: Uuid,
    sentence_id: Uuid,
    phrase_candidate_id: Uuid,
    quoted_text: &str,
    confidence: f64,
) -> anyhow::Result<Uuid> {
    let id: Uuid = sqlx::query_scalar(
        r#"
        INSERT INTO event_evidence (
            canonical_event_id, sentence_id, phrase_candidate_id,
            quoted_text, confidence
        )
        VALUES ($1, $2, $3, $4, $5)
        RETURNING id
        "#,
    )
    .bind(canonical_event_id)
    .bind(sentence_id)
    .bind(phrase_candidate_id)
    .bind(quoted_text)
    .bind(confidence)
    .fetch_one(pool)
    .await?;

    refresh_event_source_refs(pool, canonical_event_id).await?;
    Ok(id)
}

pub async fn list_timeline_events(
    pool: &PgPool,
    entity_id: Option<Uuid>,
    person: Option<&str>,
    profile_slug: Option<&str>,
    period_slug: Option<&str>,
    limit: i64,
) -> anyhow::Result<Vec<CanonicalEventRow>> {
    let period = match period_slug {
        Some(slug) => crate::profiles::get_period_by_slug(pool, slug).await?,
        None => None,
    };
    let start_year = period.as_ref().and_then(|row| row.start_year);
    let end_year = period.as_ref().and_then(|row| row.end_year);
    let person_sql = person_match_sql(2, 7);
    let sql = format!(
        r#"
        SELECT
            ce.id,
            ce.entity_id,
            COALESCE(e.canonical_name, e.wikipedia_title) AS person_name,
            ce.event_type,
            ce.epistemic_status,
            ce.title,
            ce.summary,
            ce.start_time,
            ce.place_label,
            ce.confidence,
            ce.map_eligible,
            ST_Y(ce.geom::geometry) AS lat,
            ST_X(ce.geom::geometry) AS lon
        FROM canonical_events ce
        INNER JOIN entities e ON e.id = ce.entity_id
        WHERE ($1::uuid IS NULL OR ce.entity_id = $1)
          AND {person_sql}
          AND (
            $3::text IS NULL
            OR EXISTS (
                SELECT 1 FROM entity_profiles ep
                WHERE ep.entity_id = ce.entity_id AND ep.profile_slug = $3
            )
          )
          AND (
            $4::int IS NULL
            OR ce.start_time IS NULL
            OR EXTRACT(YEAR FROM ce.start_time)::int BETWEEN COALESCE($4, -9999) AND COALESCE($5, 9999)
          )
        ORDER BY ce.start_time ASC NULLS LAST, ce.created_at ASC
        LIMIT $6
        "#
    );

    let rows = sqlx::query_as::<_, CanonicalEventRow>(&sql)
        .bind(entity_id)
        .bind(person.map(|value| format!("%{value}%")))
        .bind(profile_slug)
        .bind(start_year)
        .bind(end_year)
        .bind(limit)
        .bind(person.map(fold_latin_accents).map(|value| format!("%{value}%")))
        .fetch_all(pool)
        .await?;

    Ok(rows)
}

pub async fn list_geojson_events(
    pool: &PgPool,
    entity_id: Option<Uuid>,
    person: Option<&str>,
    map_eligible_only: bool,
    profile_slug: Option<&str>,
    period_slug: Option<&str>,
    limit: i64,
) -> anyhow::Result<Vec<CanonicalEventRow>> {
    let period = match period_slug {
        Some(slug) => crate::profiles::get_period_by_slug(pool, slug).await?,
        None => None,
    };
    let start_year = period.as_ref().and_then(|row| row.start_year);
    let end_year = period.as_ref().and_then(|row| row.end_year);
    let person_sql = person_match_sql(2, 8);
    let sql = format!(
        r#"
        SELECT
            ce.id,
            ce.entity_id,
            COALESCE(e.canonical_name, e.wikipedia_title) AS person_name,
            ce.event_type,
            ce.epistemic_status,
            ce.title,
            ce.summary,
            ce.start_time,
            ce.place_label,
            ce.confidence,
            ce.map_eligible,
            ST_Y(ce.geom::geometry) AS lat,
            ST_X(ce.geom::geometry) AS lon
        FROM canonical_events ce
        INNER JOIN entities e ON e.id = ce.entity_id
        WHERE ($1::uuid IS NULL OR ce.entity_id = $1)
          AND {person_sql}
          AND ($3 = false OR ce.map_eligible = true)
          AND ce.geom IS NOT NULL
          AND (
            $4::text IS NULL
            OR EXISTS (
                SELECT 1 FROM entity_profiles ep
                WHERE ep.entity_id = ce.entity_id AND ep.profile_slug = $4
            )
          )
          AND (
            $5::int IS NULL
            OR ce.start_time IS NULL
            OR EXTRACT(YEAR FROM ce.start_time)::int BETWEEN COALESCE($5, -9999) AND COALESCE($6, 9999)
          )
        ORDER BY ce.start_time ASC NULLS LAST
        LIMIT $7
        "#
    );

    let rows = sqlx::query_as::<_, CanonicalEventRow>(&sql)
        .bind(entity_id)
        .bind(person.map(|value| format!("%{value}%")))
        .bind(map_eligible_only)
        .bind(profile_slug)
        .bind(start_year)
        .bind(end_year)
        .bind(limit)
        .bind(person.map(fold_latin_accents).map(|value| format!("%{value}%")))
        .fetch_all(pool)
        .await?;

    Ok(rows)
}

pub async fn get_canonical_event(
    pool: &PgPool,
    event_id: Uuid,
) -> anyhow::Result<Option<CanonicalEventRow>> {
    let row = sqlx::query_as::<_, CanonicalEventRow>(
        r#"
        SELECT
            ce.id,
            ce.entity_id,
            COALESCE(e.canonical_name, e.wikipedia_title) AS person_name,
            ce.event_type,
            ce.epistemic_status,
            ce.title,
            ce.summary,
            ce.start_time,
            ce.place_label,
            ce.confidence,
            ce.map_eligible,
            ST_Y(ce.geom::geometry) AS lat,
            ST_X(ce.geom::geometry) AS lon
        FROM canonical_events ce
        INNER JOIN entities e ON e.id = ce.entity_id
        WHERE ce.id = $1
        "#,
    )
    .bind(event_id)
    .fetch_optional(pool)
    .await?;

    Ok(row)
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct NarrativeContextRow {
    pub id: Uuid,
    pub ordinal: i32,
    pub text: String,
    pub is_evidence: bool,
    pub wiki_title: String,
    pub wiki_lang: String,
}

/// Sentences around evidence spans on the same wiki page (narrative context).
pub async fn list_event_narrative_context(
    pool: &PgPool,
    canonical_event_id: Uuid,
    window: i32,
) -> anyhow::Result<Vec<NarrativeContextRow>> {
    let rows = sqlx::query_as::<_, NarrativeContextRow>(
        r#"
        WITH evidence_sentences AS (
            SELECT DISTINCT s.id, s.wiki_page_id, s.ordinal
            FROM event_evidence ee
            INNER JOIN sentences s ON s.id = ee.sentence_id
            WHERE ee.canonical_event_id = $1
        )
        SELECT
            s.id,
            s.ordinal,
            s.text,
            EXISTS(
                SELECT 1 FROM evidence_sentences es WHERE es.id = s.id
            ) AS is_evidence,
            wp.title AS wiki_title,
            wp.wiki_lang
        FROM sentences s
        INNER JOIN wiki_pages wp ON wp.id = s.wiki_page_id
        WHERE EXISTS (
            SELECT 1
            FROM evidence_sentences es
            WHERE es.wiki_page_id = s.wiki_page_id
              AND s.ordinal BETWEEN es.ordinal - $2 AND es.ordinal + $2
        )
        ORDER BY s.ordinal ASC
        "#,
    )
    .bind(canonical_event_id)
    .bind(window)
    .fetch_all(pool)
    .await?;

    Ok(rows)
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct EventEvidenceRow {
    pub id: Uuid,
    pub quoted_text: Option<String>,
    pub sentence_text: Option<String>,
    pub confidence: f64,
    pub wiki_title: Option<String>,
    pub wiki_lang: Option<String>,
    pub revision_id: Option<i64>,
    pub sentence_ordinal: Option<i32>,
    pub char_start: Option<i32>,
    pub char_end: Option<i32>,
}

pub async fn list_event_evidence(
    pool: &PgPool,
    canonical_event_id: Uuid,
) -> anyhow::Result<Vec<EventEvidenceRow>> {
    let rows = sqlx::query_as::<_, EventEvidenceRow>(
        r#"
        SELECT
            ee.id,
            ee.quoted_text,
            s.text AS sentence_text,
            ee.confidence,
            wp.title AS wiki_title,
            wp.wiki_lang,
            wp.revision_id,
            s.ordinal AS sentence_ordinal,
            s.char_start,
            s.char_end
        FROM event_evidence ee
        LEFT JOIN sentences s ON s.id = ee.sentence_id
        LEFT JOIN wiki_pages wp ON wp.id = s.wiki_page_id
        WHERE ee.canonical_event_id = $1
        ORDER BY ee.confidence DESC, ee.created_at ASC
        "#,
    )
    .bind(canonical_event_id)
    .fetch_all(pool)
    .await?;

    Ok(rows)
}

/// Rebuild POC-shaped `source_refs` / `source_page_titles` from linked evidence.
pub async fn refresh_event_source_refs(
    pool: &PgPool,
    canonical_event_id: Uuid,
) -> anyhow::Result<()> {
    let rows = list_event_evidence(pool, canonical_event_id).await?;
    let mut titles: Vec<String> = Vec::new();
    let refs: Vec<serde_json::Value> = rows
        .iter()
        .filter_map(|row| {
            let snippet = row
                .quoted_text
                .as_ref()
                .or(row.sentence_text.as_ref())?
                .clone();
            let page_title = row.wiki_title.clone().unwrap_or_else(|| "Wikipedia".into());
            let lang = row.wiki_lang.as_deref().unwrap_or("en");
            if !titles.iter().any(|t| t == &page_title) {
                titles.push(page_title.clone());
            }
            let page_url = format!(
                "https://{lang}.wikipedia.org/wiki/{}",
                page_title.replace(' ', "_")
            );
            let revision_url = row.revision_id.map(|oldid| {
                format!(
                    "https://{lang}.wikipedia.org/w/index.php?title={}&oldid={oldid}",
                    page_title.replace(' ', "_")
                )
            });
            let citation_url = revision_url
                .clone()
                .unwrap_or_else(|| page_url.clone());
            let label = format!("Wikipedia — {page_title}");
            let section_title = row
                .sentence_ordinal
                .map(|ordinal| format!("sentence {ordinal}"));
            Some(serde_json::json!({
                "type": "evidence_pointer",
                "kind": "wikipedia_sentence",
                "source_system": "wikipedia",
                "language": lang,
                "page_title": page_title.clone(),
                "source_page_title": page_title,
                "oldid": row.revision_id,
                "snippet": snippet.clone(),
                "quote": snippet,
                "label": label,
                "section_title": section_title,
                "sentence_ordinal": row.sentence_ordinal,
                "offset_start": row.char_start,
                "offset_end": row.char_end,
                "url": citation_url.clone(),
                "source_url": citation_url,
                "wikipedia_url": page_url,
                "revision_url": revision_url,
                "revision_id": row.revision_id,
                "confidence": row.confidence,
                "evidence_id": row.id,
            }))
        })
        .collect();

    sqlx::query(
        r#"
        UPDATE canonical_events
        SET source_refs = $2,
            source_page_titles = $3
        WHERE id = $1
        "#,
    )
    .bind(canonical_event_id)
    .bind(serde_json::Value::Array(refs))
    .bind(serde_json::Value::Array(
        titles.into_iter().map(serde_json::Value::String).collect(),
    ))
    .execute(pool)
    .await?;

    Ok(())
}
