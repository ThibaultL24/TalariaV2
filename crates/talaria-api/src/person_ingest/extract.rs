// crates/talaria-api/src/person_ingest/extract.rs
//! LLM prose extract vs structured Wikidata/WDQS/follow-page rules.

use std::collections::HashSet;

use talaria_quality::{parse_typed_time, typed_time_year, RawExtractItem};
use talaria_sources::extractors::{
    default_extractor_stack, keep_extracted_raw, ExtractorInput, RawCandidate,
};
use talaria_sources::first_year_in_window;
use talaria_sources::place_hint_from_title;
use talaria_sources::wdqs::WdqsEvent;

use super::collect::subject_mentioned;
use crate::llm::{self, LlmExtractItem};

fn short_clause_summary(_event_type: &str, quote: &str) -> String {
    let trimmed = quote.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    let cut = trimmed
        .find(". ")
        .or_else(|| trimmed.find('。'))
        .unwrap_or(trimmed.len());
    let sentence = trimmed[..cut].trim();
    let max = 120usize;
    if sentence.chars().count() <= max {
        return sentence.to_string();
    }
    let short = sentence.chars().take(max).collect::<String>();
    format!("{}…", short.trim_end())
}

fn push_biography_lead(text: &str, seen: &mut HashSet<String>, out: &mut Vec<RawExtractItem>) {
    let sentence_end = text.find(". ").unwrap_or(text.len().min(800));
    let Some(sentence) = text.get(..sentence_end).map(str::trim).filter(|s| !s.is_empty()) else {
        return;
    };
    for event_type in ["birth", "death"] {
        let Some(surface) = talaria_quality::biography_lead_date(event_type, sentence) else {
            continue;
        };
        let year = surface
            .split_whitespace()
            .last()
            .and_then(|token| token.parse::<i32>().ok());
        if !seen.insert(format!("{event_type}:{surface}")) {
            continue;
        }
        out.push(RawExtractItem {
            lane: "fact".into(),
            event_type: event_type.into(),
            role: event_type.into(),
            year,
            place_surface: None,
            summary: surface,
            quoted_text: sentence.to_string(),
            confidence: 0.9,
        });
    }
}

pub fn apply_wikidata_identity_places(
    items: &mut [talaria_quality::RawExtractItem],
    birth_place: Option<&str>,
    death_place: Option<&str>,
) {
    for item in items {
        if item.event_type == "birth" && item.place_surface.is_none() {
            item.place_surface = birth_place.map(str::to_string);
        }
        if item.event_type == "death" && item.place_surface.is_none() {
            item.place_surface = death_place.map(str::to_string);
        }
    }
}

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

fn year_from_raw(raw: &RawCandidate) -> Option<i32> {
    raw.time_surface
        .as_deref()
        .map(|s| parse_typed_time(Some(s)))
        .and_then(|t| typed_time_year(&t))
        .or_else(|| {
            first_year_in_window(
                raw.time_surface
                    .as_deref()
                    .unwrap_or(raw.clause_text.as_str()),
                1000,
                2099,
            )?
            .parse()
            .ok()
        })
}

/// Deterministic biography extract. The LLM pass still runs after this.
pub fn extract_wiki_rules(
    subject: &str,
    title: &str,
    text: &str,
    death_year: Option<i32>,
) -> Vec<RawExtractItem> {
    let input = ExtractorInput {
        text: text.to_string(),
        page_title: Some(title.to_string()),
        subject_label: Some(subject.to_string()),
        document_type: "article".into(),
        subject_death_year: death_year,
        ..Default::default()
    };
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    push_biography_lead(text, &mut seen, &mut out);
    for extractor in default_extractor_stack() {
        for raw in extractor.extract(&input) {
            if !keep_extracted_raw(&raw, title, subject) {
                continue;
            }
            let year = year_from_raw(&raw);
            let proposed = raw.place_surface.as_deref().unwrap_or("");
            let place = talaria_quality::anchored_action_place(&raw.clause_text, &raw.event_type, proposed)
                .filter(|label| talaria_sources::is_plausible_place_label(label));
            if year.is_none() && place.is_none() && raw.object_surface.is_none() {
                continue;
            }
            let quote = raw.clause_text.trim();
            if quote.is_empty() {
                continue;
            }
            let key = talaria_quality::proposition_key(
                &raw.event_type,
                year,
                place.as_deref().unwrap_or(""),
                quote,
            );
            if !seen.insert(key) {
                continue;
            }
            let summary = short_clause_summary(&raw.event_type, quote);
            out.push(RawExtractItem {
                lane: "fact".into(),
                event_type: raw.event_type,
                role: if raw.predicate.is_empty() {
                    "direct".into()
                } else {
                    raw.predicate
                },
                year,
                place_surface: place,
                summary,
                quoted_text: quote.to_string(),
                confidence: 0.82,
            });
        }
    }
    out
}

pub async fn extract_prose_chunk(
    pool: &sqlx::PgPool,
    raw_document_id: uuid::Uuid,
    subject: &str,
    title: &str,
    chunk: &str,
) -> anyhow::Result<Vec<RawExtractItem>> {
    let model = llm::extraction_model();
    let provider = llm::provider_label();
    let hash = talaria_ai::cache_key(&provider, &model, subject, title, chunk);
    if let Some(value) = sqlx::query_scalar::<_, serde_json::Value>(
        "SELECT output_json FROM ai_runs WHERE purpose='person_extraction' AND provider=$1 AND model=$2 AND prompt_version=$3 AND input_hash=$4"
    ).bind(&provider).bind(&model).bind(talaria_ai::EXTRACTION_VERSION).bind(&hash)
        .fetch_optional(pool).await? {
        talaria_ai::validate_extraction(&value, chunk)?;
        let items: Vec<LlmExtractItem> = serde_json::from_value(value["items"].clone())?;
        return Ok(items.into_iter().map(LlmExtractItem::into_raw).collect());
    }
    let started = std::time::Instant::now();
    let items = llm::extract_chunk(subject, title, chunk).await?;
    let value = serde_json::json!({"items": items});
    sqlx::query("INSERT INTO ai_runs (purpose, provider, model, prompt_version, input_hash, raw_document_id, output_json, latency_ms) VALUES ('person_extraction',$1,$2,$3,$4,$5,$6,$7) ON CONFLICT DO NOTHING")
        .bind(&provider).bind(&model).bind(talaria_ai::EXTRACTION_VERSION).bind(&hash)
        .bind(raw_document_id).bind(value).bind(started.elapsed().as_millis() as i64)
        .execute(pool).await?;
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
        let place_is_id = talaria_quality::is_wikidata_qid(place);
        let quote = if place_is_id {
            format!(
                "{subject} | {event_type} | {pred} | {}",
                parts[3].trim()
            )
        } else {
            format!(
                "{subject} | {event_type} | {pred} | {} | {place}",
                parts[3].trim()
            )
        };
        lines.push(quote.clone());
        let place_surface = if place.is_empty() || place_is_id {
            None
        } else {
            Some(place.to_string())
        };
        let summary = match &place_surface {
            Some(label) => format!("{pred} {label}"),
            None => pred.replace('_', " "),
        };
        items.push(RawExtractItem {
            lane: "fact".into(),
            event_type: event_type.into(),
            role: "direct".into(),
            year,
            place_surface,
            summary: summary.trim().to_string(),
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
    let date_surface = if ev.date.len() == 10 {
        ev.date.clone()
    } else {
        year.map(|y| y.to_string()).unwrap_or_default()
    };
    let quote = format!(
        "{subject} | {} | {} | {} | {}",
        ev.event_type,
        ev.label,
        date_surface,
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
    if l.contains("siège") || l.contains("siege") {
        "siege".into()
    } else if l.contains("battle") || l.contains("bataille") {
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
    talaria_sources::first_year_in_window(text, 1000, 2099)?
        .parse()
        .ok()
}

fn title_is_military_action(title: &str) -> bool {
    let l = title.to_lowercase();
    l.contains("battle")
        || l.contains("bataille")
        || l.contains("siege")
        || l.contains("siège")
        || l.contains("campagne")
        || l.contains("campaign of")
}

fn first_sentence(extract: &str) -> Option<&str> {
    let trimmed = extract.trim();
    if trimmed.is_empty() {
        return None;
    }
    let end = trimmed
        .find(['.', '!', '?'])
        .map(|i| i + 1)
        .unwrap_or(trimmed.len());
    Some(trimmed.get(..end)?.trim()).filter(|s| !s.is_empty())
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
        let end = if is_end {
            i + ch.len_utf8()
        } else {
            extract.len()
        };
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
    _military_subject: bool,
) -> Option<(String, RawExtractItem, Option<(f64, f64)>)> {
    let quote = episode_quote(extract, subject, title)?;
    let event_type = event_type_from_title(title);
    let year = year_from_text(&quote);
    let place = place_hint_from_title(title).filter(|place| quote_contains(&quote, place));
    let coords = place.as_ref().and(coords);
    let item = RawExtractItem {
        lane: "fact".into(),
        event_type,
        role: "direct".into(),
        year,
        place_surface: place,
        summary: title.to_string(),
        quoted_text: quote,
        confidence: 0.88,
    };
    Some((extract.to_string(), item, coords))
}

/// The sentence that names the subject, widened by one neighbour when that neighbour names the page.
fn episode_quote(extract: &str, subject: &str, title: &str) -> Option<String> {
    let sentences = sentence_spans(extract);
    for sentence in &sentences {
        if subject_mentioned(sentence, subject)
            && talaria_quality::passage_supports_title(title, sentence)
        {
            return Some((*sentence).to_string());
        }
    }
    for pair in sentences.windows(2) {
        let joined = format!("{} {}", pair[0], pair[1]);
        if subject_mentioned(&joined, subject)
            && talaria_quality::passage_supports_title(title, &joined)
        {
            return Some(joined);
        }
    }
    None
}

fn sentence_spans(extract: &str) -> Vec<&str> {
    let mut spans = Vec::new();
    let mut start = 0usize;
    let mut rest = extract;
    let mut offset = 0usize;
    while let Some(index) = rest.find(". ") {
        let end = offset + index + 1;
        if end > start {
            spans.push(extract[start..end].trim());
        }
        offset += index + 2;
        start = offset;
        rest = extract.get(offset..).unwrap_or("");
    }
    if start < extract.len() {
        spans.push(extract[start..].trim());
    }
    spans.into_iter().filter(|sentence| !sentence.is_empty()).collect()
}

fn quote_contains(quote: &str, place: &str) -> bool {
    quote.to_lowercase().contains(&place.to_lowercase())
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
        let identifiers = "STATEMENT\tresidence\tresided_in\t1769\tQ40104";
        let (_, raw) = statements_to_raw_items("Napoleon Bonaparte", identifiers);
        assert_eq!(raw[0].place_surface, None);
        assert!(!raw[0].summary.contains("Q40104"));
        assert!(!raw[0].quoted_text.contains("Q40104"));
    }

    #[test]
    fn chunks_split_long_text() {
        let text = "aaaa\n\nbbbb\n\ncccc";
        assert_eq!(split_chunks(text, 8).len(), 3);
    }

    #[test]
    fn a_biography_lead_keeps_the_birth_and_death_days() {
        let lead = "Napoleon Bonaparte (born Napoleone di Buonaparte; 15 August 1769 – 5 May 1821) was Emperor of the French. He later fought at Lodi.";
        let items = extract_wiki_rules("Napoleon Bonaparte", "Napoleon", lead, Some(1821));
        let birth = items.iter().find(|item| item.event_type == "birth").expect("birth");
        let death = items.iter().find(|item| item.event_type == "death").expect("death");
        assert_eq!(birth.year, Some(1769));
        assert_eq!(death.year, Some(1821));
        assert!(birth.summary.to_lowercase().contains("15"));
        assert!(death.summary.to_lowercase().contains("5 may") || death.summary.to_lowercase().contains("5"));
        let campaign = extract_wiki_rules(
            "Napoleon Bonaparte",
            "Siege of Mantua",
            "Siege of Mantua (4 June 1796 – 2 February 1797) was a siege.",
            Some(1821),
        );
        assert!(campaign.iter().all(|item| item.event_type != "birth"));
    }

    #[test]
    fn wiki_rules_extract_louis_xiv_dated_places() {
        let items = extract_wiki_rules(
            "Louis XIV",
            "Louis XIV",
            "Louis XIV est sacré le 7 juin 1654 en la cathédrale de Reims.\n\
             À partir de 1682, Louis XIV dirige son royaume depuis le château de Versailles.\n\
             Le 7 novembre 1659, Louis XIV signe le traité des Pyrénées.",
            Some(1715),
        );
        assert!(
            items.iter().any(|i| i.year == Some(1654)
                && i.place_surface
                    .as_deref()
                    .is_some_and(|p| p.contains("Reims"))),
            "missing Reims: {items:?}"
        );
        assert!(
            items.iter().any(|i| i.year == Some(1682)
                && i.place_surface
                    .as_deref()
                    .is_some_and(|p| p.to_lowercase().contains("versailles"))),
            "missing Versailles: {items:?}"
        );
    }

    #[test]
    fn wiki_rules_joan_sieges_from_prose_paragraph() {
        let items = extract_wiki_rules(
            "Joan of Arc",
            "Joan of Arc",
            "Joan of Arc (1412 – 30 May 1431) is a patron saint of France.\n\
             After Charles's coronation, Joan participated in the unsuccessful siege of Paris in September 1429 and the failed siege of La Charité in November.",
            Some(1431),
        );
        assert!(
            items.iter().any(|i| i.event_type == "siege"
                && i.year == Some(1429)
                && i.place_surface
                    .as_deref()
                    .is_some_and(|p| p.contains("Paris"))),
            "Joan Paris siege: {items:?}"
        );
    }

    #[test]
    fn wiki_rules_baudelaire_cafe_not_art_salon() {
        let items = extract_wiki_rules(
            "Charles Baudelaire",
            "Charles Baudelaire",
            "Baudelaire fréquentait beaucoup les cafés. Il affectionnait aussi La Rotonde, un café du Quartier latin.\n\
             His first published work was his art review Salon of 1845.",
            Some(1867),
        );
        assert!(
            items.iter().any(|i| i.event_type == "residence"
                && i.place_surface
                    .as_deref()
                    .is_some_and(|p| p.to_lowercase().contains("quartier"))),
            "café Quartier latin: {items:?}"
        );
        assert!(
            !items.iter().any(|i| i
                .place_surface
                .as_deref()
                .is_some_and(|p| p.to_lowercase().contains("1845"))),
            "must not geocode Salon of 1845: {items:?}"
        );
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
            true,
        )
        .expect("pin");
        assert_eq!(coords, Some((50.680, 4.412)));
        assert_eq!(item.event_type, "battle");
        assert_eq!(item.place_surface.as_deref(), Some("Waterloo"));
        let got = accept_items("Napoleon", &doc, [item]);
        assert!(
            !got[0].quoted_text.contains('|'),
            "follow quote must be page text, not a synthetic pipe line"
        );
        assert!(
            extract.contains(&got[0].quoted_text) || {
                talaria_quality::quote_is_grounded(extract, &got[0].quoted_text)
            }
        );
        assert!(got[0].quoted_text.contains("18 June 1815"));
    }

    #[test]
    fn follow_page_quote_is_verbatim_extract_span() {
        let extract = "The Battle of Waterloo was fought on Sunday 18 June 1815 near Waterloo. Napoleon's French army was defeated by the Duke of Wellington.";
        let (_, item, _) =
            follow_page_to_extract("Napoleon", "Battle of Waterloo", extract, None, true)
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
            false,
        )
        .is_none());
        assert!(follow_page_to_extract(
            "Napoleon",
            "Battle of Waterloo",
            extract,
            Some((50.680, 4.412)),
            true,
        )
        .is_none());
    }

    #[test]
    fn a_title_the_passage_does_not_name_is_not_assembled() {
        let prisoners = "Bonaparte returned with prisoners and cannons after 18 pitched battles.";
        assert!(follow_page_to_extract(
            "Napoleon",
            "Treaty of Campo Formio",
            prisoners,
            Some((48.86, 2.35)),
            true,
        )
        .is_none());
        let extract =
            "The siege was fought in 1877. Victor Hugo is named among later commemorations.";
        assert!(follow_page_to_extract("Victor Hugo", "Siege of Plevna", extract, None, false).is_none());
    }
}
