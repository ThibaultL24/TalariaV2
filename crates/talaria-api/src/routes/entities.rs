// crates/talaria-api/src/routes/entities.rs
use axum::{
    extract::{Path, Query, State},
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};
use talaria_wikidata::{person_search_score, sort_person_search_hits, WikidataClient};
use uuid::Uuid;

use super::AppState;

#[derive(Debug, Deserialize)]
pub struct EntitySearchQuery {
    pub q: String,
    #[serde(default = "default_lang")]
    pub lang: String,
    #[serde(default = "default_search_limit")]
    pub limit: i64,
}

#[derive(Debug, Deserialize)]
pub struct EntityGetQuery {}

fn default_lang() -> String {
    "en".into()
}

fn default_search_limit() -> i64 {
    10
}

pub async fn search(
    State(state): State<AppState>,
    Query(query): Query<EntitySearchQuery>,
) -> Json<Value> {
    let trimmed = query.q.trim();
    if trimmed.len() < 2 {
        return Json(json!({ "items": [] }));
    }

    let local = talaria_store::search_local_entities(&state.pool, trimmed, query.limit)
        .await
        .unwrap_or_default();

    let mut items: Vec<Value> = local
        .into_iter()
        .map(|entity| {
            let label = entity
                .canonical_name
                .clone()
                .unwrap_or_else(|| entity.wikipedia_title.clone());
            json!({
                "entity_id": entity.id,
                "qid": entity.qid,
                "label": label,
                "description": entity.wikipedia_title,
                "known_locally": true,
                "event_count": entity.event_count,
                "wikipedia_title": entity.wikipedia_title,
            })
        })
        .collect();

    let local_qids: std::collections::HashSet<String> = items
        .iter()
        .filter_map(|item| {
            item.get("qid")
                .and_then(|value| value.as_str())
                .map(str::to_string)
        })
        .collect();

    if !state.offline_only && items.len() < query.limit as usize {
        if let Ok(client) = WikidataClient::new() {
            if let Ok(hits) = client
                .search_entities(trimmed, &query.lang, query.limit.max(10) as u32)
                .await
            {
                let known: Vec<String> = local_qids.iter().cloned().collect();
                let hits = sort_person_search_hits(hits, &known);
                for hit in hits {
                    if items.len() >= query.limit as usize {
                        break;
                    }
                    if local_qids.contains(&hit.qid) {
                        continue;
                    }
                    if let Ok(Some(entity)) =
                        talaria_store::find_entity_by_qid(&state.pool, &hit.qid).await
                    {
                        let label = entity
                            .canonical_name
                            .clone()
                            .unwrap_or_else(|| entity.wikipedia_title.clone());
                        items.push(json!({
                            "entity_id": entity.id,
                            "qid": entity.qid,
                            "label": label,
                            "description": hit.description,
                            "known_locally": true,
                            "event_count": entity.event_count,
                            "wikipedia_title": entity.wikipedia_title,
                        }));
                        continue;
                    }

                    items.push(json!({
                        "entity_id": Value::Null,
                        "qid": hit.qid,
                        "label": hit.label,
                        "description": hit.description,
                        "known_locally": false,
                        "event_count": 0,
                    }));
                }
            }
        }
    }

    items.sort_by(|a, b| {
        let a_local = a.get("known_locally").and_then(|v| v.as_bool()).unwrap_or(false);
        let b_local = b.get("known_locally").and_then(|v| v.as_bool()).unwrap_or(false);
        b_local.cmp(&a_local).then_with(|| {
            let a_s = person_search_score(
                a.get("label").and_then(|v| v.as_str()).unwrap_or(""),
                a.get("description").and_then(|v| v.as_str()),
            );
            let b_s = person_search_score(
                b.get("label").and_then(|v| v.as_str()).unwrap_or(""),
                b.get("description").and_then(|v| v.as_str()),
            );
            b_s.cmp(&a_s)
        })
    });
    items.truncate(query.limit as usize);

    Json(json!({ "items": items }))
}

pub async fn get_entity(
    State(state): State<AppState>,
    Path(entity_id): Path<Uuid>,
    Query(_query): Query<EntityGetQuery>,
) -> Json<Value> {
    let entity = talaria_store::get_entity(&state.pool, entity_id)
        .await
        .ok()
        .flatten();

    match entity {
        Some(entity) => {
            let label = entity
                .canonical_name
                .clone()
                .unwrap_or_else(|| entity.wikipedia_title.clone());
            let profiles = talaria_store::list_entity_profiles(&state.pool, entity.id)
                .await
                .unwrap_or_default();
            Json(json!({
                "entity": {
                    "id": entity.id,
                    "qid": entity.qid,
                    "label": label,
                    "wikipedia_title": entity.wikipedia_title,
                    "event_count": entity.event_count,
                    "profiles": profiles.iter().map(|row| json!({
                        "slug": row.profile_slug,
                        "label": row.profile_label,
                        "kind": row.kind,
                        "qid": row.profile_qid,
                        "confidence": row.confidence,
                        "source_system": row.source_system,
                    })).collect::<Vec<_>>(),
                }
            }))
        }
        None => Json(json!({ "entity": Value::Null })),
    }
}

#[derive(Debug, Deserialize)]
pub struct EntityClaimsQuery {
    #[serde(default = "default_claims_limit")]
    pub limit: i64,
    #[serde(default = "default_true")]
    pub debates_only: bool,
}

fn default_claims_limit() -> i64 {
    50
}

fn default_true() -> bool {
    true
}

pub async fn list_claims(
    State(state): State<AppState>,
    Path(entity_id): Path<Uuid>,
    Query(query): Query<EntityClaimsQuery>,
) -> Json<Value> {
    let claims = talaria_store::list_claims_for_entity(
        &state.pool,
        entity_id,
        query.limit,
        query.debates_only,
    )
    .await
    .unwrap_or_default();

    let mut items = Vec::with_capacity(claims.len());
    for claim in claims {
        let evidence = talaria_store::list_claim_evidence(&state.pool, claim.id)
            .await
            .unwrap_or_default();
        let mut evidence_items = Vec::with_capacity(evidence.len());
        for row in &evidence {
            let mut item = json!({
                "id": row.id,
                "source_system": row.source_system,
                "locator": row.locator,
                "quote": row.quote,
                "sentence_id": row.sentence_id,
                "confidence": row.confidence,
            });
            if let Some(locator) = row.locator.as_deref() {
                if let Ok(Some(doc)) = talaria_store::find_corpus_document_by_locator(
                    &state.pool,
                    locator,
                    Some(row.source_system.as_str()),
                )
                .await
                {
                    item["document_id"] = json!(doc.id);
                    item["document_title"] = json!(doc.title);
                    item["document_url"] = json!(doc.canonical_url);
                    item["document_type"] = json!(doc.document_type);
                    item["source_kind"] = json!(doc.source_kind);
                }
            }
            evidence_items.push(item);
        }
        items.push(json!({
            "id": claim.id,
            "claim_kind": claim.claim_kind,
            "text": claim.text,
            "epistemic_status": claim.epistemic_status,
            "relation_to_subject": claim.relation_to_subject,
            "event_time": claim.event_time,
            "place_label": claim.place_label,
            "confidence": claim.confidence,
            "canonical_event_id": claim.canonical_event_id,
            "debate_type": claim.debate_type,
            "evidence_layer": claim.evidence_layer,
            "evidence": evidence_items,
        }));
    }

    Json(json!({
        "claims": items,
        "count": items.len(),
        "epistemic": "historiographic_opinion",
        "epistemic_note": "Soft claims are theories, debates, and historiographic interpretations — not quality map/timeline facts.",
    }))
}
