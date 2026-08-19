// crates/talaria-api/src/routes/documents.rs
//! Corpus document + bibliography HTTP surface (PR1).

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

use super::AppState;

/// Bibliographic resources are not validated historical facts / quality events.
const EPISTEMIC: &str = "bibliographic_resource";
const EPISTEMIC_NOTE: &str =
    "Bibliographic metadata and abstracts are not validated historical evidence; they do not create quality events or soft claims.";

#[derive(Debug, Deserialize)]
pub struct EntityDocumentsQuery {
    #[serde(default)]
    pub types: Option<String>,
    #[serde(default)]
    pub providers: Option<String>,
    #[serde(default)]
    pub academic_status: Option<String>,
    #[serde(default)]
    pub access: Option<String>,
    #[serde(default)]
    pub language: Option<String>,
    #[serde(default)]
    pub relation: Option<String>,
    #[serde(default)]
    pub cursor: Option<String>,
    #[serde(default = "default_limit")]
    pub limit: i64,
}

#[derive(Debug, Deserialize)]
pub struct BibliographyQuery {
    #[serde(default = "default_relation_about")]
    pub relation: String,
    #[serde(default)]
    pub cursor: Option<String>,
    #[serde(default = "default_limit")]
    pub limit: i64,
}

fn default_limit() -> i64 {
    50
}

fn default_relation_about() -> String {
    "about".into()
}

fn parse_csv(s: Option<&String>) -> Vec<String> {
    s.map(|v| {
        v.split(',')
            .map(|p| p.trim().to_string())
            .filter(|p| !p.is_empty())
            .collect()
    })
    .unwrap_or_default()
}

fn parse_cursor(cursor: Option<&String>) -> (Option<f32>, Option<Uuid>) {
    let Some(c) = cursor
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
    else {
        return (None, None);
    };
    // format: {score:.6}_{uuid}
    let mut parts = c.splitn(2, '_');
    let score = parts.next().and_then(|s| s.parse::<f32>().ok());
    let id = parts.next().and_then(|s| Uuid::parse_str(s).ok());
    (score, id)
}

fn encode_cursor(score: f32, id: Uuid) -> String {
    format!("{score:.6}_{id}")
}

fn link_json(row: &talaria_store::EntityLinkedDocumentRow) -> Value {
    json!({
        "relation": row.relation,
        "score": row.score,
        "match_version": row.match_version,
        "components": row.components_json,
        "evidence_summary": row.evidence_summary,
    })
}

fn document_json(row: &talaria_store::EntityLinkedDocumentRow) -> Value {
    json!({
        "id": row.id,
        "source_kind": row.source_kind,
        "external_id": row.external_id,
        "title": row.title,
        "document_type": row.document_type,
        "academic_status": row.academic_status,
        "access": {
            "level": row.access_level,
            "full_text_available": row.full_text_available,
        },
        "language": row.language,
        "canonical_url": row.canonical_url,
        "publication_time": row.publication_time,
        "epistemic": EPISTEMIC,
        "link": link_json(row),
    })
}

pub async fn list_entity_documents(
    State(state): State<AppState>,
    Path(entity_id): Path<Uuid>,
    Query(q): Query<EntityDocumentsQuery>,
) -> Result<Json<Value>, StatusCode> {
    let limit = q.limit.clamp(1, 200);
    let (cursor_score, cursor_id) = parse_cursor(q.cursor.as_ref());
    let rows = talaria_store::list_entity_documents(
        &state.pool,
        entity_id,
        &talaria_store::EntityDocumentsFilter {
            relation: q.relation.as_deref(),
            document_types: &parse_csv(q.types.as_ref()),
            providers: &parse_csv(q.providers.as_ref()),
            academic_status: q.academic_status.as_deref(),
            access: q.access.as_deref(),
            language: q.language.as_deref(),
            limit: limit + 1,
            cursor_score,
            cursor_id,
        },
    )
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let has_more = rows.len() as i64 > limit;
    let page: Vec<_> = rows.into_iter().take(limit as usize).collect();
    let next_cursor = if has_more {
        page.last().map(|r| encode_cursor(r.score, r.id))
    } else {
        None
    };
    let items: Vec<Value> = page.iter().map(document_json).collect();
    Ok(Json(json!({
        "entity_id": entity_id,
        "epistemic": EPISTEMIC,
        "epistemic_note": EPISTEMIC_NOTE,
        "items": items,
        "next_cursor": next_cursor,
    })))
}

pub async fn list_entity_bibliography(
    State(state): State<AppState>,
    Path(entity_id): Path<Uuid>,
    Query(q): Query<BibliographyQuery>,
) -> Result<Json<Value>, StatusCode> {
    let relation = match q.relation.as_str() {
        "by" | "about" | "mentioned_in" => q.relation.as_str(),
        _ => "about",
    };
    let limit = q.limit.clamp(1, 200);
    let (cursor_score, cursor_id) = parse_cursor(q.cursor.as_ref());
    let rows = talaria_store::list_entity_documents(
        &state.pool,
        entity_id,
        &talaria_store::EntityDocumentsFilter {
            relation: Some(relation),
            document_types: &[],
            providers: &[],
            academic_status: None,
            access: None,
            language: None,
            limit: limit + 1,
            cursor_score,
            cursor_id,
        },
    )
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let has_more = rows.len() as i64 > limit;
    let page: Vec<_> = rows.into_iter().take(limit as usize).collect();
    let next_cursor = if has_more {
        page.last().map(|r| encode_cursor(r.score, r.id))
    } else {
        None
    };

    let mut items = Vec::new();
    for row in &page {
        let idents = talaria_store::list_document_identifiers(&state.pool, row.id)
            .await
            .unwrap_or_default();
        let contribs = talaria_store::list_document_contributions(&state.pool, row.id)
            .await
            .unwrap_or_default();
        items.push(json!({
            "id": row.id,
            "title": row.title,
            "document_type": row.document_type,
            "source_kind": row.source_kind,
            "external_id": row.external_id,
            "canonical_url": row.canonical_url,
            "academic_status": row.academic_status,
            "access": {
                "level": row.access_level,
                "full_text_available": row.full_text_available,
            },
            "language": row.language,
            "publication_time": row.publication_time,
            "epistemic": EPISTEMIC,
            "identifiers": idents.iter().map(|i| json!({
                "scheme": i.scheme,
                "value": i.value_raw,
                "normalized": i.value_normalized,
            })).collect::<Vec<_>>(),
            "contributions": contribs.iter().map(|c| json!({
                "role": c.role,
                "name": c.agent_name,
                "identifier_scheme": c.identifier_scheme,
                "identifier_value": c.identifier_value,
                "ordinal": c.ordinal,
            })).collect::<Vec<_>>(),
            "link": link_json(row),
        }));
    }

    Ok(Json(json!({
        "entity_id": entity_id,
        "relation": relation,
        "epistemic": EPISTEMIC,
        "epistemic_note": EPISTEMIC_NOTE,
        "items": items,
        "next_cursor": next_cursor,
    })))
}

pub async fn get_document(
    State(state): State<AppState>,
    Path(document_id): Path<Uuid>,
) -> Result<Json<Value>, StatusCode> {
    let Some(doc) = talaria_store::get_corpus_document(&state.pool, document_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    else {
        return Err(StatusCode::NOT_FOUND);
    };
    let idents = talaria_store::list_document_identifiers(&state.pool, document_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let contribs = talaria_store::list_document_contributions(&state.pool, document_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let snapshot_count = talaria_store::count_corpus_snapshots(&state.pool, document_id)
        .await
        .unwrap_or(0);

    Ok(Json(json!({
        "id": doc.id,
        "source_kind": doc.source_kind,
        "external_id": doc.external_id,
        "canonical_url": doc.canonical_url,
        "document_type": doc.document_type,
        "title": doc.title,
        "language": doc.language,
        "abstract": doc.abstract_text,
        "academic_status": doc.academic_status,
        "access": {
            "level": doc.access_level,
            "full_text_available": doc.full_text_available,
        },
        "rights": {
            "uri": doc.rights_uri,
            "holder": doc.rights_holder,
            "normalized": doc.rights_normalized,
        },
        "publisher_or_institution": doc.publisher_or_institution,
        "publication_time": doc.publication_time,
        "connector_version": doc.connector_version,
        "snapshot_count": snapshot_count,
        "epistemic": EPISTEMIC,
        "epistemic_note": EPISTEMIC_NOTE,
        "identifiers": idents.iter().map(|i| json!({
            "scheme": i.scheme,
            "value": i.value_raw,
            "normalized": i.value_normalized,
        })).collect::<Vec<_>>(),
        "contributions": contribs.iter().map(|c| json!({
            "role": c.role,
            "name": c.agent_name,
            "identifier_scheme": c.identifier_scheme,
            "identifier_value": c.identifier_value,
            "ordinal": c.ordinal,
        })).collect::<Vec<_>>(),
    })))
}

pub async fn list_document_fragments(
    State(_state): State<AppState>,
    Path(document_id): Path<Uuid>,
) -> Json<Value> {
    Json(json!({
        "document_id": document_id,
        "items": [],
        "next_cursor": null,
        "epistemic": EPISTEMIC,
        "note": "corpus fragments land in PR2; sentence/clause quality fragments unchanged",
    }))
}
