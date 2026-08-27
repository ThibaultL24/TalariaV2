// crates/talaria-api/src/person_ingest/grounding.rs
//! Ground extracts: prose requires a verbatim quote; structured uses a statement id.

use talaria_quality::{
    accept_items, agent_is_other_person, parse_lane, GroundedItem, RawExtractItem, RejectReason,
};

/// Prose path: `accept_items` (verbatim quote in document).
pub fn ground_prose(
    subject: &str,
    document: &str,
    raw: impl IntoIterator<Item = RawExtractItem>,
) -> Vec<GroundedItem> {
    accept_items(subject, document, raw)
}

/// Structured path: skip verbatim quote when a statement id is present.
pub fn ground_structured(
    subject: &str,
    document: &str,
    raw: impl IntoIterator<Item = RawExtractItem>,
    statement_id: Option<&str>,
) -> Vec<GroundedItem> {
    if statement_id.map(str::trim).filter(|s| !s.is_empty()).is_none() {
        return accept_items(subject, document, raw);
    }
    raw.into_iter()
        .filter_map(|item| structured_item(&item, subject).ok())
        .collect()
}

fn structured_item(item: &RawExtractItem, subject: &str) -> Result<GroundedItem, RejectReason> {
    if item.quoted_text.trim().is_empty() {
        return Err(RejectReason::EmptyQuote);
    }
    let lane = parse_lane(&item.lane);
    if lane == talaria_quality::Lane::Fact && agent_is_other_person(&item.quoted_text, subject) {
        return Err(RejectReason::OtherPersonAgent);
    }
    Ok(GroundedItem {
        lane,
        event_type: if item.event_type.trim().is_empty() {
            "historical_fact".into()
        } else {
            item.event_type.clone()
        },
        role: if item.role.trim().is_empty() {
            "direct".into()
        } else {
            item.role.clone()
        },
        year: item.year,
        place_surface: item.place_surface.clone(),
        summary: item.summary.clone(),
        quoted_text: item.quoted_text.clone(),
        confidence: item.confidence.clamp(0.0, 1.0),
    })
}

pub fn text_span_locator(uri: &str, title: &str) -> String {
    serde_json::json!({ "kind": "text_span", "uri": uri, "title": title }).to_string()
}

pub fn wikidata_locator(statement_id: &str) -> String {
    serde_json::json!({
        "kind": "wikidata_statement",
        "statement_id": statement_id,
    })
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use talaria_quality::accept_items;

    #[test]
    fn schrodinger_chunk_yields_no_curie_facts() {
        let doc = "On 6 April 1920, Schrödinger married Annemarie Bertel in Vienna.";
        let raw = [RawExtractItem {
            lane: "fact".into(),
            event_type: "marriage".into(),
            role: "direct".into(),
            year: Some(1920),
            place_surface: Some("Vienna".into()),
            summary: "marriage".into(),
            quoted_text: doc.into(),
            confidence: 0.9,
        }];
        assert!(accept_items("Marie Curie", doc, raw).is_empty());
    }

    #[test]
    fn structured_item_grounds_without_document_quote() {
        let raw = RawExtractItem {
            lane: "fact".into(),
            event_type: "birth".into(),
            role: "direct".into(),
            year: Some(1754),
            place_surface: Some("Versailles".into()),
            summary: "birth".into(),
            quoted_text: "Louis XVI | birth | P569 | 1754 | Versailles".into(),
            confidence: 0.9,
        };
        let got = ground_structured("Louis XVI", "", [raw], Some("Q7732$P569"));
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].event_type, "birth");
    }
}
