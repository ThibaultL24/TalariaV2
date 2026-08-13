// crates/talaria-api/src/routes/events.rs
use axum::{
    extract::{Path, Query, State},
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};
use talaria_store::CanonicalEventRow;
use uuid::Uuid;

use super::AppState;

#[derive(Debug, Deserialize)]
pub struct TimelineQuery {
    pub entity_id: Option<Uuid>,
    pub person: Option<String>,
    pub profile_slug: Option<String>,
    pub period_slug: Option<String>,
    #[serde(default = "default_limit")]
    pub limit: i64,
}

#[derive(Debug, Deserialize)]
pub struct GeoJsonQuery {
    pub entity_id: Option<Uuid>,
    pub person: Option<String>,
    pub profile_slug: Option<String>,
    pub period_slug: Option<String>,
    #[serde(default = "default_true")]
    pub map_eligible: bool,
    #[serde(default = "default_limit")]
    pub limit: i64,
}

fn default_limit() -> i64 {
    100
}

fn default_true() -> bool {
    true
}

pub async fn timeline(
    State(state): State<AppState>,
    Query(query): Query<TimelineQuery>,
) -> Json<Value> {
    let events = talaria_store::list_timeline_events(
        &state.pool,
        query.entity_id,
        query.person.as_deref(),
        query.profile_slug.as_deref(),
        query.period_slug.as_deref(),
        query.limit,
    )
    .await
    .unwrap_or_default();

    Json(json!({
        "events": events.iter().map(event_to_json).collect::<Vec<_>>(),
        "count": events.len(),
    }))
}

pub async fn geojson(
    State(state): State<AppState>,
    Query(query): Query<GeoJsonQuery>,
) -> Json<Value> {
    let events = talaria_store::list_geojson_events(
        &state.pool,
        query.entity_id,
        query.person.as_deref(),
        query.map_eligible,
        query.profile_slug.as_deref(),
        query.period_slug.as_deref(),
        query.limit,
    )
    .await
    .unwrap_or_default();

    let features: Vec<Value> = events
        .iter()
        .filter_map(|event| geojson_feature(event))
        .collect();

    Json(json!({
        "type": "FeatureCollection",
        "features": features,
    }))
}

pub async fn evidence(State(state): State<AppState>, Path(event_id): Path<Uuid>) -> Json<Value> {
    let rows = talaria_store::list_event_evidence(&state.pool, event_id)
        .await
        .unwrap_or_default();

    Json(json!({
        "event_id": event_id,
        "evidence": rows.iter().map(evidence_to_json).collect::<Vec<_>>(),
    }))
}

pub async fn detail(State(state): State<AppState>, Path(event_id): Path<Uuid>) -> Json<Value> {
    let Some(event) = talaria_store::get_canonical_event(&state.pool, event_id)
        .await
        .ok()
        .flatten()
    else {
        return Json(json!({ "event": Value::Null }));
    };

    let entity = talaria_store::get_entity(&state.pool, event.entity_id)
        .await
        .ok()
        .flatten();

    let evidence_rows = talaria_store::list_event_evidence(&state.pool, event_id)
        .await
        .unwrap_or_default();

    // Local window around evidence: enough for a short “how it happened”, not a bio dump.
    let narrative = talaria_store::list_event_narrative_context(&state.pool, event_id, 2)
        .await
        .unwrap_or_default();

    let wiki_lang = evidence_rows
        .iter()
        .find_map(|row| row.wiki_lang.clone())
        .or_else(|| narrative.first().map(|row| row.wiki_lang.clone()))
        .unwrap_or_else(|| "en".into());

    let wikipedia_title = evidence_rows
        .iter()
        .find_map(|row| row.wiki_title.clone())
        .or_else(|| entity.as_ref().map(|row| row.wikipedia_title.clone()))
        .or_else(|| narrative.first().map(|row| row.wiki_title.clone()));

    let article_url = wikipedia_title.as_ref().map(|title| {
        format!(
            "https://{wiki_lang}.wikipedia.org/wiki/{}",
            title.replace(' ', "_")
        )
    });

    let revision_url = evidence_rows.iter().find_map(|row| {
        let title = row.wiki_title.as_ref()?;
        let revision = row.revision_id?;
        let lang = row.wiki_lang.as_deref().unwrap_or("en");
        Some(format!(
            "https://{lang}.wikipedia.org/w/index.php?title={}&oldid={revision}",
            title.replace(' ', "_")
        ))
    });

    let fact_text = evidence_rows
        .first()
        .and_then(|row| {
            row.quoted_text
                .clone()
                .or_else(|| row.sentence_text.clone())
        })
        .or_else(|| event.summary.clone());

    let dossier = crate::narrative_dossier::build_event_dossier(
        &state.pool,
        &event,
        fact_text.as_deref(),
        &narrative,
        &evidence_rows,
        wikipedia_title.as_deref(),
        &wiki_lang,
        state.offline_only,
    )
    .await;

    // Persist DB evidence refs; dossier adds section-level citations for the paragraph.
    let _ = talaria_store::refresh_event_source_refs(&state.pool, event_id).await;
    let mut source_refs: Vec<Value> = dossier.source_refs;
    if source_refs.is_empty() {
        source_refs = evidence_rows
            .iter()
            .filter_map(source_ref_from_evidence)
            .collect();
    }

    let source_page_titles: Vec<Value> = source_refs
        .iter()
        .filter_map(|row| {
            row.get("page_title")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .fold(Vec::new(), |mut acc, title| {
            if !acc.iter().any(|t| t == &title) {
                acc.push(title);
            }
            acc
        })
        .into_iter()
        .map(Value::String)
        .collect();

    Json(json!({
        "event": event_to_json(&event),
        "entity": entity.as_ref().map(|row| json!({
            "id": row.id,
            "label": row.canonical_name.clone().unwrap_or_else(|| row.wikipedia_title.clone()),
            "wikipedia_title": row.wikipedia_title,
            "qid": row.qid,
        })),
        "links": {
            "wikipedia_url": article_url,
            "wikipedia_revision_url": revision_url,
            "wikidata_url": entity.as_ref().and_then(|row| {
                row.qid.as_ref().map(|qid| format!("https://www.wikidata.org/wiki/{qid}"))
            }),
        },
        "narrative": {
            "event_summary": dossier.event_summary,
            "how_it_happened": dossier.how_it_happened,
            "fact": fact_text,
            "context_sentences": narrative.iter().map(|row| json!({
                "text": row.text,
                "is_evidence": row.is_evidence,
                "ordinal": row.ordinal,
            })).collect::<Vec<_>>(),
        },
        "source_refs": source_refs,
        "source_page_titles": source_page_titles,
        "evidence": evidence_rows.iter().map(evidence_to_json).collect::<Vec<_>>(),
    }))
}

fn source_ref_from_evidence(row: &talaria_store::EventEvidenceRow) -> Option<Value> {
    let snippet = row
        .quoted_text
        .as_ref()
        .or(row.sentence_text.as_ref())?
        .clone();
    let page_title = row.wiki_title.clone().unwrap_or_else(|| "Wikipedia".into());
    let lang = row.wiki_lang.as_deref().unwrap_or("en");
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
    let citation_url = revision_url.clone().unwrap_or_else(|| page_url.clone());
    let label = format!("Wikipedia — {page_title}");
    let section_title = row
        .sentence_ordinal
        .map(|ordinal| format!("sentence {ordinal}"));

    Some(json!({
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
        "wikipedia_url": page_url.clone(),
        "page_url": page_url,
        "revision_url": revision_url,
        "revision_id": row.revision_id,
        "confidence": row.confidence,
        "evidence_id": row.id,
    }))
}

fn evidence_to_json(row: &talaria_store::EventEvidenceRow) -> Value {
    let lang = row.wiki_lang.as_deref().unwrap_or("en");
    let page_url = row.wiki_title.as_ref().map(|title| {
        format!(
            "https://{lang}.wikipedia.org/wiki/{}",
            title.replace(' ', "_")
        )
    });
    let revision_url = match (row.wiki_title.as_ref(), row.revision_id) {
        (Some(title), Some(revision)) => Some(format!(
            "https://{lang}.wikipedia.org/w/index.php?title={}&oldid={revision}",
            title.replace(' ', "_")
        )),
        _ => None,
    };

    json!({
        "id": row.id,
        "quoted_text": row.quoted_text,
        "sentence_text": row.sentence_text,
        "confidence": row.confidence,
        "wiki_title": row.wiki_title,
        "wiki_lang": row.wiki_lang,
        "revision_id": row.revision_id,
        "sentence_ordinal": row.sentence_ordinal,
        "char_start": row.char_start,
        "char_end": row.char_end,
        "page_url": page_url,
        "revision_url": revision_url,
        "citation_url": revision_url.or(page_url),
    })
}

fn event_to_json(event: &CanonicalEventRow) -> Value {
    json!({
        "id": event.id,
        "entity_id": event.entity_id,
        "person": event.person_name,
        "event_type": event.event_type,
        "epistemic_status": event.epistemic_status,
        "title": event.title,
        "summary": event.summary,
        "start_time": event.start_time,
        "place_label": event.place_label,
        "confidence": event.confidence,
        "map_eligible": event.map_eligible,
        "coordinates": coords_json(event),
    })
}

fn geojson_feature(event: &CanonicalEventRow) -> Option<Value> {
    let (lat, lon) = (event.lat?, event.lon?);
    Some(json!({
        "type": "Feature",
        "id": event.id.to_string(),
        "geometry": {
            "type": "Point",
            "coordinates": [lon, lat],
        },
        "properties": {
            "id": event.id,
            "entity_id": event.entity_id,
            "person": event.person_name,
            "event_type": event.event_type,
            "epistemic_status": event.epistemic_status,
            "title": event.title,
            "summary": event.summary,
            "start_time": event.start_time,
            "place_label": event.place_label,
            "confidence": event.confidence,
        }
    }))
}

fn coords_json(event: &CanonicalEventRow) -> Value {
    match (event.lat, event.lon) {
        (Some(lat), Some(lon)) => json!({ "lat": lat, "lon": lon }),
        _ => Value::Null,
    }
}
