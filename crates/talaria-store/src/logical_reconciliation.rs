// crates/talaria-store/src/logical_reconciliation.rs
//! Candidate pairs for the Historical Critic. A proposal is not a merge.

use crate::fold_latin_accents;
use sqlx::PgPool;
use uuid::Uuid;

pub const REASON_SAME_PERSON: &str = "same_person";
pub const REASON_SAME_TYPE: &str = "same_event_type";
pub const REASON_COMPATIBLE_TIME: &str = "compatible_time";
pub const REASON_SAME_PLACE: &str = "same_place";
pub const REASON_COMPATIBLE_ACTION: &str = "compatible_action";
pub const REASON_SHARED_CITATION: &str = "shared_citation";

#[derive(Debug, Clone)]
pub struct ReconciliationReading {
    pub event_id: Uuid,
    pub entity_id: Uuid,
    pub event_type: String,
    pub predicate: Option<String>,
    pub time_json: serde_json::Value,
    pub place_label: Option<String>,
    pub place_identity_qid: Option<String>,
    pub logical_event_id: Option<Uuid>,
    pub lineage_id: Option<Uuid>,
    pub quotes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReconciliationProposal {
    pub left_event_id: Uuid,
    pub right_event_id: Uuid,
    pub reasons: Vec<&'static str>,
    pub left_quote: Option<String>,
    pub right_quote: Option<String>,
}

/// Active readings that share a person, a type, a containing year, and a place.
/// A shared title, a similar occurrence key, or a missing place is not enough.
pub fn propose_reconciliations(readings: &[ReconciliationReading]) -> Vec<ReconciliationProposal> {
    let mut proposals = Vec::new();
    for left_index in 0..readings.len() {
        for right in readings.iter().skip(left_index + 1) {
            if let Some(proposal) = compare(&readings[left_index], right) {
                proposals.push(proposal);
            }
        }
    }
    proposals.sort_by(|left, right| {
        (left.left_event_id, left.right_event_id).cmp(&(right.left_event_id, right.right_event_id))
    });
    proposals
}

pub async fn propose_logical_reconciliations(
    pool: &PgPool,
) -> anyhow::Result<Vec<ReconciliationProposal>> {
    let has_logical: bool = sqlx::query_scalar(
        r#"
        SELECT EXISTS (
            SELECT 1 FROM information_schema.columns
            WHERE table_schema = 'public'
              AND table_name = 'canonical_events'
              AND column_name = 'logical_event_id'
        )
        "#,
    )
    .fetch_one(pool)
    .await?;
    let sql = if has_logical {
        r#"
        SELECT id, entity_id, event_type, predicate, time_json, place_label,
               place_identity_qid, logical_event_id
        FROM canonical_events
        WHERE pipeline = 'person' AND is_active
        "#
    } else {
        r#"
        SELECT id, entity_id, event_type, predicate, time_json, place_label,
               place_identity_qid, NULL::uuid AS logical_event_id
        FROM canonical_events
        WHERE pipeline = 'person' AND is_active
        "#
    };
    let rows = sqlx::query_as::<_, ActiveReadingRow>(sql)
        .fetch_all(pool)
        .await?;
    if rows.is_empty() {
        return Ok(Vec::new());
    }
    let ids: Vec<Uuid> = rows.iter().map(|row| row.id).collect();
    let quotes = sqlx::query_as::<_, QuoteRow>(
        r#"
        SELECT canonical_event_id, quoted_text
        FROM event_evidence
        WHERE canonical_event_id = ANY($1) AND quoted_text IS NOT NULL
        "#,
    )
    .bind(&ids)
    .fetch_all(pool)
    .await?;
    let mut quotes_by_event: std::collections::HashMap<Uuid, Vec<String>> =
        std::collections::HashMap::new();
    for quote in quotes {
        if let Some(text) = quote.quoted_text {
            quotes_by_event
                .entry(quote.canonical_event_id)
                .or_default()
                .push(text);
        }
    }
    let readings = rows
        .into_iter()
        .map(|row| ReconciliationReading {
            event_id: row.id,
            entity_id: row.entity_id,
            event_type: row.event_type,
            predicate: row.predicate,
            time_json: row.time_json,
            place_label: row.place_label,
            place_identity_qid: row.place_identity_qid,
            logical_event_id: row.logical_event_id,
            lineage_id: row.logical_event_id,
            quotes: quotes_by_event.remove(&row.id).unwrap_or_default(),
        })
        .collect::<Vec<_>>();
    Ok(propose_reconciliations(&readings))
}

#[derive(Debug, sqlx::FromRow)]
struct ActiveReadingRow {
    id: Uuid,
    entity_id: Uuid,
    event_type: String,
    predicate: Option<String>,
    time_json: serde_json::Value,
    place_label: Option<String>,
    place_identity_qid: Option<String>,
    logical_event_id: Option<Uuid>,
}

#[derive(Debug, sqlx::FromRow)]
struct QuoteRow {
    canonical_event_id: Uuid,
    quoted_text: Option<String>,
}

fn compare(
    left: &ReconciliationReading,
    right: &ReconciliationReading,
) -> Option<ReconciliationProposal> {
    if left.event_id == right.event_id || left.entity_id != right.entity_id {
        return None;
    }
    if left.event_type != right.event_type {
        return None;
    }
    if let (Some(left_id), Some(right_id)) = (left.logical_event_id, right.logical_event_id) {
        if left_id == right_id {
            return None;
        }
    }
    if let (Some(left_line), Some(right_line)) = (left.lineage_id, right.lineage_id) {
        if left_line == right_line {
            return None;
        }
    }
    if !years_compatible(&left.time_json, &right.time_json) || !places_compatible(left, right) {
        return None;
    }
    if predicates_conflict(left.predicate.as_deref(), right.predicate.as_deref()) {
        return None;
    }
    let shared = shared_quote(&left.quotes, &right.quotes);
    let mut reasons = vec![
        REASON_SAME_PERSON,
        REASON_SAME_TYPE,
        REASON_COMPATIBLE_TIME,
        REASON_SAME_PLACE,
        REASON_COMPATIBLE_ACTION,
    ];
    if shared.is_some() {
        reasons.push(REASON_SHARED_CITATION);
    }
    let (left_quote, right_quote) = shared.unwrap_or((String::new(), String::new()));
    let (left_event_id, right_event_id, left_quote, right_quote) = if left.event_id < right.event_id
    {
        (left.event_id, right.event_id, left_quote, right_quote)
    } else {
        (right.event_id, left.event_id, right_quote, left_quote)
    };
    let left_quote = if left_quote.is_empty() {
        None
    } else {
        Some(left_quote)
    };
    let right_quote = if right_quote.is_empty() {
        None
    } else {
        Some(right_quote)
    };
    Some(ReconciliationProposal {
        left_event_id,
        right_event_id,
        reasons,
        left_quote,
        right_quote,
    })
}

fn predicates_conflict(left: Option<&str>, right: Option<&str>) -> bool {
    matches!((left, right), (Some(left), Some(right)) if left != right)
}

fn places_compatible(left: &ReconciliationReading, right: &ReconciliationReading) -> bool {
    if let (Some(left_qid), Some(right_qid)) = (
        left.place_identity_qid.as_deref(),
        right.place_identity_qid.as_deref(),
    ) {
        if !left_qid.is_empty() && left_qid == right_qid {
            return true;
        }
    }
    let left_label = normalized_place(left.place_label.as_deref());
    let right_label = normalized_place(right.place_label.as_deref());
    !left_label.is_empty() && left_label == right_label
}

fn normalized_place(label: Option<&str>) -> String {
    fold_latin_accents(label.unwrap_or("").trim())
}

fn years_compatible(left: &serde_json::Value, right: &serde_json::Value) -> bool {
    let Some((left_start, left_end)) = year_bounds(left) else {
        return false;
    };
    let Some((right_start, right_end)) = year_bounds(right) else {
        return false;
    };
    if left_end - left_start > 1 || right_end - right_start > 1 {
        return false;
    }
    (left_start >= right_start && left_end <= right_end)
        || (right_start >= left_start && right_end <= left_end)
}

fn year_bounds(time: &serde_json::Value) -> Option<(i32, i32)> {
    match time.get("kind").and_then(|value| value.as_str()) {
        Some("exact" | "approx") => {
            let year = year_field(time)?;
            Some((year, year))
        }
        Some("range") => {
            let start = time
                .get("start_year")
                .and_then(|value| value.as_i64())
                .and_then(|year| i32::try_from(year).ok())
                .or_else(|| year_field(time))?;
            let end = time
                .get("end_year")
                .and_then(|value| value.as_i64())
                .and_then(|year| i32::try_from(year).ok())
                .or_else(|| {
                    time.get("end")
                        .and_then(|value| value.as_str())
                        .and_then(leading_year)
                })
                .unwrap_or(start);
            Some((start.min(end), start.max(end)))
        }
        _ => None,
    }
}

fn year_field(time: &serde_json::Value) -> Option<i32> {
    time.get("year")
        .and_then(|value| value.as_i64())
        .and_then(|year| i32::try_from(year).ok())
        .or_else(|| {
            time.get("start")
                .and_then(|value| value.as_str())
                .and_then(leading_year)
        })
}

fn leading_year(value: &str) -> Option<i32> {
    let negative = value.starts_with('-');
    let digits: String = value
        .chars()
        .skip(usize::from(negative))
        .take_while(|character| character.is_ascii_digit())
        .collect();
    if digits.len() < 3 {
        return None;
    }
    let year: i32 = digits.parse().ok()?;
    Some(if negative { -year } else { year })
}

fn shared_quote(left: &[String], right: &[String]) -> Option<(String, String)> {
    for left_quote in left.iter().take(8) {
        let left_text = left_quote.trim();
        if left_text.chars().count() < 20 {
            continue;
        }
        for right_quote in right.iter().take(8) {
            let right_text = right_quote.trim();
            if right_text.chars().count() < 20 {
                continue;
            }
            let (shorter, longer) = if left_text.chars().count() <= right_text.chars().count() {
                (left_text, right_text)
            } else {
                (right_text, left_text)
            };
            if longer.to_lowercase().contains(&shorter.to_lowercase()) {
                return Some((clip(left_text), clip(right_text)));
            }
        }
    }
    None
}

fn clip(quote: &str) -> String {
    quote.chars().take(240).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn reading(
        id: u128,
        entity: u128,
        event_type: &str,
        time: serde_json::Value,
        place: Option<&str>,
        qid: Option<&str>,
        logical: Option<u128>,
    ) -> ReconciliationReading {
        ReconciliationReading {
            event_id: Uuid::from_u128(id),
            entity_id: Uuid::from_u128(entity),
            event_type: event_type.into(),
            predicate: Some("published".into()),
            time_json: time,
            place_label: place.map(str::to_string),
            place_identity_qid: qid.map(str::to_string),
            logical_event_id: logical.map(Uuid::from_u128),
            lineage_id: logical.map(Uuid::from_u128),
            quotes: Vec::new(),
        }
    }

    #[test]
    fn logical_identity_reconciliation_proposes_without_merging() {
        let year = json!({"kind": "exact", "precision": "year", "start": "1898"});
        let month = json!({"kind": "exact", "precision": "month", "start": "1898-07"});
        let other_year = json!({"kind": "exact", "precision": "year", "start": "1903"});
        let unknown = json!({"kind": "unknown", "surface": "date unknown"});
        let same_title_no_place = propose_reconciliations(&[
            reading(1, 7, "publication", year.clone(), None, None, Some(11)),
            reading(2, 7, "publication", month.clone(), None, None, Some(12)),
        ]);
        assert!(same_title_no_place.is_empty());

        let mut refined = reading(2, 7, "publication", month, Some("Paris"), None, Some(12));
        refined
            .quotes
            .push("Marie Curie published the radioactivity note in Paris in July 1898.".into());
        let mut original = reading(1, 7, "publication", year, Some("Paris"), None, Some(11));
        original
            .quotes
            .push("Marie Curie published the radioactivity note in Paris in July 1898.".into());
        let proposed = propose_reconciliations(&[original, refined]);
        assert_eq!(proposed.len(), 1);
        assert!(proposed[0].reasons.contains(&REASON_SHARED_CITATION));
        assert!(proposed[0].reasons.contains(&REASON_COMPATIBLE_TIME));
        assert_ne!(proposed[0].left_event_id, proposed[0].right_event_id);

        let different_years = propose_reconciliations(&[
            reading(
                1,
                7,
                "publication",
                json!({"kind": "exact", "precision": "year", "start": "1898"}),
                Some("Paris"),
                None,
                Some(11),
            ),
            reading(
                3,
                7,
                "publication",
                other_year,
                Some("Paris"),
                None,
                Some(13),
            ),
        ]);
        assert!(different_years.is_empty());

        let different_types = propose_reconciliations(&[
            reading(
                1,
                7,
                "publication",
                json!({"kind": "exact", "precision": "year", "start": "1898"}),
                Some("Paris"),
                None,
                Some(11),
            ),
            reading(
                4,
                7,
                "residence",
                json!({"kind": "exact", "precision": "year", "start": "1898"}),
                Some("Paris"),
                None,
                Some(14),
            ),
        ]);
        assert!(different_types.is_empty());

        let already_one = propose_reconciliations(&[
            reading(
                1,
                7,
                "publication",
                json!({"kind": "exact", "precision": "year", "start": "1898"}),
                Some("Paris"),
                Some("Q90"),
                Some(11),
            ),
            reading(
                5,
                7,
                "publication",
                json!({"kind": "exact", "precision": "day", "start": "1898-07-18"}),
                Some("Paris"),
                Some("Q90"),
                Some(11),
            ),
        ]);
        assert!(already_one.is_empty());

        let ancient = propose_reconciliations(&[
            reading(8, 9, "education", unknown.clone(), None, None, Some(21)),
            reading(9, 9, "education", unknown, None, None, Some(22)),
        ]);
        assert!(ancient.is_empty());

        let wide_range = propose_reconciliations(&[
            reading(
                1,
                7,
                "residence",
                json!({"kind": "range", "start_year": 1800, "end_year": 1815}),
                Some("Paris"),
                None,
                Some(11),
            ),
            reading(
                6,
                7,
                "residence",
                json!({"kind": "exact", "precision": "year", "start": "1804"}),
                Some("Paris"),
                None,
                Some(16),
            ),
        ]);
        assert!(wide_range.is_empty());
    }
}
