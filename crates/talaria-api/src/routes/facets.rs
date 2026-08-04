// crates/talaria-api/src/routes/facets.rs
use axum::{extract::State, Json};
use serde_json::{json, Value};

use super::AppState;

pub async fn list_periods(State(state): State<AppState>) -> Json<Value> {
    let rows = talaria_store::list_periods(&state.pool)
        .await
        .unwrap_or_default();
    Json(json!({
        "periods": rows.iter().map(|row| json!({
            "id": row.id,
            "slug": row.slug,
            "label": row.label,
            "start_year": row.start_year,
            "end_year": row.end_year,
            "kind": row.kind,
            "wikidata_qid": row.wikidata_qid,
        })).collect::<Vec<_>>(),
    }))
}

pub async fn list_profiles(State(state): State<AppState>) -> Json<Value> {
    let rows = talaria_store::list_profile_catalog(&state.pool)
        .await
        .unwrap_or_default();
    Json(json!({
        "profiles": rows.iter().map(|(slug, label, count)| json!({
            "slug": slug,
            "label": label,
            "entity_count": count,
        })).collect::<Vec<_>>(),
    }))
}
