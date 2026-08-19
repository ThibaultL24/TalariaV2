// crates/talaria-api/src/place_conflict.rs
//! Assemble-time abstention when two sources disagree on place.

use talaria_quality::{competing_places, ABSTAIN_COMPETING_PLACE, RejectionCode};
use talaria_store::{
    list_place_labels_for_occurrence_stem, mark_quality_claims_conflict_by_stem,
    mark_quality_events_uncertain_by_stem,
};
use uuid::Uuid;

pub async fn abstain_if_competing_place(
    pool: &sqlx::PgPool,
    subject_id: Uuid,
    stem: &str,
    incoming_place: Option<&str>,
) -> anyhow::Result<Option<Vec<String>>> {
    let rows = list_place_labels_for_occurrence_stem(pool, subject_id, stem).await?;
    let existing: Vec<Option<&str>> = rows.iter().map(|s| Some(s.as_str())).collect();
    let Some(places) = competing_places(incoming_place, &existing) else {
        return Ok(None);
    };
    let conflict_json = serde_json::json!({
        "reason": ABSTAIN_COMPETING_PLACE,
        "places": places,
    });
    mark_quality_claims_conflict_by_stem(pool, subject_id, stem, &conflict_json).await?;
    mark_quality_events_uncertain_by_stem(pool, subject_id, stem).await?;
    Ok(Some(places))
}

pub fn competing_place_codes() -> Vec<String> {
    vec![RejectionCode::CompetingPlace.as_str().to_string()]
}
