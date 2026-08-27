// crates/talaria-api/src/person_ingest/extract.rs
//! LLM prose extract vs structured Wikidata/WDQS/follow-page rules.

use talaria_quality::RawExtractItem;
use talaria_sources::place_hint_from_title;
use talaria_sources::wdqs::WdqsEvent;

use super::collect::subject_mentioned;
use crate::llm::{self, LlmExtractItem};

pub fn split_chunks(text: &str, max: usize) -> Vec<String> {
    if text.len() <= max {
        return vec![text.to_string()];
    }
    let mut out = Vec::new();
    let mut buf = String::new();
    for para in text.split("\n\n") {
        if buf.len() + para.len() + 2 > max && !buf.is_empty() {
            out.push(std::mem::take(&mut buf));
        }
        if !buf.is_empty() {
            buf.push_str("\n\n");
        }
        buf.push_str(para);
    }
    if !buf.is_empty() {
        out.push(buf);
    }
    out
}

pub async fn extract_prose_chunk(
    subject: &str,
    title: &str,
    chunk: &str,
) -> anyhow::Result<Vec<RawExtractItem>> {
    let items = llm::extract_chunk(subject, title, chunk).await?;
    Ok(items.into_iter().map(LlmExtractItem::into_raw).collect())
}

pub fn statements_to_raw_items(subject: &str, statements: &str) -> (String, Vec<RawExtractItem>) {
    let mut lines = Vec::new();
    let mut items = Vec::new();
    for line in statements.lines() {
        let parts: Vec<&str> = line.split('\t').collect();
        if parts.len() < 5 || parts[0] != "STATEMENT" {
            continue;
        }
        let event_type = parts[1].trim();
        let pred = parts[2].trim();
        let year = parts[3].trim().parse::<i32>().ok();
        let place = parts[4].trim();
        if event_type.is_empty() {
            continue;
        }
        let quote = format!(
            "{subject} | {event_type} | {pred} | {} | {place}",
            parts[3].trim()
        );
        lines.push(quote.clone());
        items.push(RawExtractItem {
            lane: "fact".into(),
            event_type: event_type.into(),
            role: "direct".into(),
            year,
            place_surface: if place.is_empty() {
                None
            } else {
                Some(place.to_string())
            },
            summary: format!("{pred} {place}").trim().to_string(),
            quoted_text: quote,
            confidence: 0.92,
        });
    }
    (lines.join("\n"), items)
}

pub fn year_from_wdqs_date(date: &str) -> Option<i32> {
    let y: i32 = date.get(..4)?.parse().ok()?;
    (y != 0).then_some(y)
}

fn coords_for_wdqs(ev: &WdqsEvent) -> Option<(f64, f64)> {
    match (ev.lat, ev.lon) {
        (Some(lat), Some(lon)) if lat.abs() <= 90.0 && lon.abs() <= 180.0 => Some((lat, lon)),
        _ => None,
    }
}

pub fn wdqs_event_to_extract(
    subject: &str,
    ev: &WdqsEvent,
) -> (String, RawExtractItem, Option<(f64, f64)>) {
    let year = year_from_wdqs_date(&ev.date);
    let place = ev
        .place_label
        .clone()
        .or_else(|| place_hint_from_title(&ev.label));
    let quote = format!(
        "{subject} | {} | {} | {} | {}",
        ev.event_type,
        ev.label,
        year.map(|y| y.to_string()).unwrap_or_default(),
        place.as_deref().unwrap_or("")
    );
    let item = RawExtractItem {
        lane: "fact".into(),
        event_type: if ev.event_type.is_empty() {
            "historical_fact".into()
        } else {
            ev.event_type.clone()
        },
        role: "direct".into(),
        year,
        place_surface: place,
        summary: ev.label.clone(),
        quoted_text: quote.clone(),
        confidence: 0.95,
    };
    (quote, item, coords_for_wdqs(ev))
}

fn event_type_from_title(title: &str) -> String {
    let l = title.to_lowercase();
    if l.contains("battle") || l.contains("bataille") || l.contains("siege") || l.contains("siège")
    {
        "battle".into()
    } else if l.contains("treaty") || l.contains("traité") || l.contains("treaties") {
        "diplomatic".into()
    } else if l.contains("palace") || l.contains("château") || l.contains("chateau") {
        "residence".into()
    } else {
        "historical_fact".into()
    }
}

fn year_from_text(text: &str) -> Option<i32> {
    talaria_sources::first_year_in_window(text, 1000, 2099)?.parse().ok()
}

/// Verbatim sentence that mentions the subject (not a synthetic title line).
pub fn mention_sentence<'a>(extract: &'a str, subject: &str) -> Option<&'a str> {
    let mut start = 0;
    let bytes = extract.as_bytes();
    for (i, ch) in extract.char_indices() {
        let is_end = ch == '.' || ch == '!' || ch == '?';
        let last = i + ch.len_utf8() == extract.len();
        if !is_end && !last {
            continue;
        }
        let end = if is_end { i + ch.len_utf8() } else { extract.len() };
        let sent = extract.get(start..end).unwrap_or("");
        if subject_mentioned(sent, subject) {
            return Some(sent.trim());
        }
        start = end;
        while start < bytes.len() && bytes[start].is_ascii_whitespace() {
            start += 1;
        }
    }
    None
}

pub fn follow_page_to_extract(
    subject: &str,
    title: &str,
    extract: &str,
    coords: Option<(f64, f64)>,
) -> Option<(String, RawExtractItem, Option<(f64, f64)>)> {
    let quote = mention_sentence(extract, subject)?;
    let event_type = event_type_from_title(title);
    let year = year_from_text(extract).or_else(|| year_from_text(title));
    let place = place_hint_from_title(title);
    let item = RawExtractItem {
        lane: "fact".into(),
        event_type,
        role: "direct".into(),
        year,
        place_surface: place,
        summary: title.to_string(),
        quoted_text: quote.to_string(),
        confidence: 0.88,
    };
    Some((extract.to_string(), item, coords))
}

#[cfg(test)]
mod tests {
    use super::*;
    use talaria_quality::accept_items;

    #[test]
    fn wikidata_birth_statement_becomes_fact() {
        let statements = "STATEMENT\tbirth\tborn_in\t1867\tWarsaw";
        let (doc, raw) = statements_to_raw_items("Marie Curie", statements);
        let got = accept_items("Marie Curie", &doc, raw);
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].event_type, "birth");
        assert_eq!(got[0].place_surface.as_deref(), Some("Warsaw"));
    }

    #[test]
    fn chunks_split_long_text() {
        let text = "aaaa\n\nbbbb\n\ncccc";
        assert_eq!(split_chunks(text, 8).len(), 3);
    }

    #[test]
    fn year_from_wdqs_iso_date() {
        assert_eq!(year_from_wdqs_date("1805-12-02"), Some(1805));
        assert_eq!(year_from_wdqs_date(""), None);
    }

    #[test]
    fn wdqs_battle_with_coords_is_a_grounded_napoleon_fact() {
        let ev = talaria_sources::wdqs::WdqsEvent {
            event_qid: "Q179250".into(),
            label: "Battle of Austerlitz".into(),
            date: "1805-12-02".into(),
            place_qid: None,
            place_label: Some("Austerlitz".into()),
            event_type: "battle".into(),
            lat: Some(49.1281),
            lon: Some(16.7622),
        };
        let (quote, item, coords) = wdqs_event_to_extract("Napoleon", &ev);
        assert_eq!(coords, Some((49.1281, 16.7622)));
        assert_eq!(item.event_type, "battle");
        assert_eq!(item.place_surface.as_deref(), Some("Austerlitz"));
        let got = accept_items("Napoleon", &quote, [item]);
        assert_eq!(got.len(), 1);
    }

    #[test]
    fn follow_page_mentioning_napoleon_is_a_grounded_map_fact() {
        let extract = "The Battle of Waterloo was fought on Sunday 18 June 1815 near Waterloo. Napoleon's French army was defeated by the Duke of Wellington.";
        let (doc, item, coords) = follow_page_to_extract(
            "Napoleon",
            "Battle of Waterloo",
            extract,
            Some((50.680, 4.412)),
        )
        .expect("pin");
        assert_eq!(coords, Some((50.680, 4.412)));
        assert_eq!(item.event_type, "battle");
        assert_eq!(item.place_surface.as_deref(), Some("Waterloo"));
        let got = accept_items("Napoleon", &doc, [item]);
        assert_eq!(got.len(), 1);
        assert!(
            !got[0].quoted_text.contains('|'),
            "follow quote must be page text, not a synthetic pipe line"
        );
        assert!(extract.contains(&got[0].quoted_text) || {
            talaria_quality::quote_is_grounded(extract, &got[0].quoted_text)
        });
    }

    #[test]
    fn follow_page_quote_is_verbatim_extract_span() {
        let extract = "The Battle of Waterloo was fought on Sunday 18 June 1815 near Waterloo. Napoleon's French army was defeated by the Duke of Wellington.";
        let (_, item, _) = follow_page_to_extract(
            "Napoleon",
            "Battle of Waterloo",
            extract,
            None,
        )
        .expect("pin");
        assert!(!item.quoted_text.contains("Napoleon | battle |"));
        assert!(extract.contains(item.quoted_text.trim()) || extract.contains(&item.quoted_text));
    }

    #[test]
    fn follow_page_without_subject_mention_is_dropped() {
        let extract = "Wellington commanded the allied army on 18 June 1815 near Waterloo.";
        assert!(follow_page_to_extract(
            "Napoleon",
            "Battle of Waterloo",
            extract,
            Some((50.680, 4.412)),
        )
        .is_none());
    }

    #[test]
    fn victor_hugo_followed_battle_title_is_not_role_supported() {
        let extract =
            "The siege was fought in 1877. Victor Hugo is named among later commemorations.";
        let (doc, item, _) =
            follow_page_to_extract("Victor Hugo", "Siege of Plevna", extract, None).expect("mention");
        assert!(!item.quoted_text.contains('|'));
        let got = accept_items("Victor Hugo", &doc, [item]);
        assert_eq!(got.len(), 1);
        let m = crate::person_ingest::gating::classify_item(
            "Victor Hugo",
            &[],
            &got[0],
            "Siege of Plevna",
            true,
            false,
        );
        assert_eq!(m, talaria_quality::AttributionMatch::Unattributed);
        assert!(!talaria_quality::auto_accept_attribution(m));
    }
}
