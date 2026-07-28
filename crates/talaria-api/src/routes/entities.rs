// crates/talaria-api/src/routes/entities.rs
use axum::{
    extract::{Path, Query, State},
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};
use talaria_wikidata::WikidataClient;
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
pub struct EntityGetQuery {
    #[serde(default = "default_lang")]
    #[allow(dead_code)]
    pub lang: String,
}

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
        .filter_map(|item| item.get("qid").and_then(|value| value.as_str()).map(str::to_string))
        .collect();

    if items.len() < query.limit as usize {
        if let Ok(client) = WikidataClient::new() {
            let remaining = (query.limit as usize).saturating_sub(items.len());
            if let Ok(hits) = client
                .search_entities(trimmed, &query.lang, remaining as u32)
                .await
            {
                for hit in hits {
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
            Json(json!({
                "entity": {
                    "id": entity.id,
                    "qid": entity.qid,
                    "label": label,
                    "wikipedia_title": entity.wikipedia_title,
                    "event_count": entity.event_count,
                }
            }))
        }
        None => Json(json!({ "entity": Value::Null })),
    }
}
