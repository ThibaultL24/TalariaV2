// crates/talaria-api/src/routes/demo.rs
//! Curated demo roster enrichment for the public showcase home.

use axum::{extract::State, Json};
use serde_json::{json, Value};
use sqlx::Row;

use super::AppState;

const DEMO_QIDS: &[&str] = &[
    "Q517", "Q687", "Q7186", "Q535", "Q7226", "Q7742", "Q3052772", "Q22686", "Q2042", "Q76",
];

pub async fn demo_roster(State(state): State<AppState>) -> Json<Value> {
    let rows = sqlx::query(
        r#"
        SELECT
            e.qid,
            e.id AS entity_id,
            COALESCE(e.canonical_name, e.wikipedia_title) AS label,
            (
                SELECT COUNT(*)::bigint
                FROM canonical_events ce
                WHERE ce.entity_id = e.id
                  AND ce.pipeline = 'person'
                  AND ce.is_active
            ) AS event_count,
            (
                SELECT COUNT(*)::bigint
                FROM canonical_events ce
                WHERE ce.entity_id = e.id
                  AND ce.pipeline = 'person'
                  AND ce.is_active
                  AND ce.map_eligible
            ) AS map_pin_count,
            (
                SELECT COUNT(*)::bigint FROM soft_claims sc WHERE sc.entity_id = e.id
            ) AS claim_count,
            (
                SELECT COUNT(*)::bigint
                FROM intuition_publications ip
                WHERE ip.subject_entity_id = e.id
            ) AS intuition_count
        FROM entities e
        WHERE e.qid = ANY($1)
        "#,
    )
    .bind(DEMO_QIDS)
    .fetch_all(&state.pool)
    .await
    .unwrap_or_default();

    let by_qid: std::collections::HashMap<String, Value> = rows
        .into_iter()
        .filter_map(|row| {
            let qid: String = row.try_get("qid").ok()?;
            Some((
                qid.clone(),
                json!({
                    "qid": qid,
                    "entity_id": row.try_get::<uuid::Uuid, _>("entity_id").ok(),
                    "label": row.try_get::<String, _>("label").ok(),
                    "event_count": row.try_get::<i64, _>("event_count").unwrap_or(0),
                    "map_pin_count": row.try_get::<i64, _>("map_pin_count").unwrap_or(0),
                    "claim_count": row.try_get::<i64, _>("claim_count").unwrap_or(0),
                    "intuition_count": row.try_get::<i64, _>("intuition_count").unwrap_or(0),
                    "known_locally": true,
                }),
            ))
        })
        .collect();

    let items: Vec<Value> = DEMO_QIDS
        .iter()
        .map(|qid| {
            by_qid.get(*qid).cloned().unwrap_or_else(|| {
                json!({
                    "qid": qid,
                    "entity_id": null,
                    "label": null,
                    "event_count": 0,
                    "map_pin_count": 0,
                    "claim_count": 0,
                    "intuition_count": 0,
                    "known_locally": false,
                })
            })
        })
        .collect();

    Json(json!({
        "items": items,
        "count": items.len(),
        "intuition": {
            "network": "testnet",
            "live_allowed": crate::intuition::live_publish_allowed(),
        }
    }))
}
