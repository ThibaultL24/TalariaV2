// crates/talaria-sources/tests/v37_probe.rs
//! Bounded live probe. It does not open Postgres and does not call a paid model.
//! The reference file is frozen; this test must not rewrite it.

use std::time::{Duration, Instant};

use serde_json::{json, Value};
use talaria_quality::{
    anchored_action_place, documentary_relation, narrative_reading, novelty_of, proposition_key,
    quote_may_attest, DocumentaryRelation, KnownEvent, NarrativeReading,
};
use talaria_sources::extractors::{
    extractor_stack_for, keep_extracted_raw, ExtractorInput,
};
use talaria_sources::PersonClass;

fn reference_path() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/benchmarks/v37-reference.json")
}

fn load_reference() -> Value {
    let raw = std::fs::read_to_string(reference_path()).expect("reference file");
    serde_json::from_str(&raw).expect("reference json")
}

#[test]
fn the_reference_stays_a_hand_chronology() {
    let reference = load_reference();
    assert_eq!(reference["frozen"], "2026-11-16");
    let persons = reference["persons"].as_array().expect("persons");
    assert_eq!(persons.len(), 6);
    let napoleon = persons.iter().find(|person| person["qid"] == "Q517").unwrap();
    let lonato = napoleon["events"]
        .as_array()
        .unwrap()
        .iter()
        .find(|event| event["id"] == "lonato")
        .unwrap();
    assert_eq!(lonato["class"], "attested_direct");
    assert_eq!(lonato["year"], 1796);
    let desbordes = persons.iter().find(|person| person["qid"] == "Q466987").unwrap();
    assert_eq!(desbordes["label"], "Marceline Desbordes-Valmore");
}

fn fold(value: &str) -> String {
    value
        .chars()
        .map(|ch| match ch {
            'à' | 'á' | 'â' | 'ä' => 'a',
            'è' | 'é' | 'ê' | 'ë' => 'e',
            'ì' | 'í' | 'î' | 'ï' => 'i',
            'ò' | 'ó' | 'ô' | 'ö' => 'o',
            'ù' | 'ú' | 'û' | 'ü' => 'u',
            'ç' => 'c',
            other => other.to_ascii_lowercase(),
        })
        .collect()
}

fn years_in(text: &str) -> Vec<i32> {
    let bytes = text.as_bytes();
    let mut found = Vec::new();
    let mut index = 0;
    while index + 3 < bytes.len() {
        if bytes[index].is_ascii_digit() {
            let start = index;
            while index < bytes.len() && bytes[index].is_ascii_digit() {
                index += 1;
            }
            let length = index - start;
            if length == 3 || length == 4 {
                if let Ok(year) = text[start..index].parse::<i32>() {
                    let after = text[index..].chars().take(16).collect::<String>().to_lowercase();
                    let bc = after.contains("bc") || after.contains("bce") || after.contains("av.");
                    found.push(if bc { -year } else { year });
                }
            }
            continue;
        }
        index += 1;
    }
    found
}

fn place_of(event: &Value) -> Option<String> {
    event["place"].as_str().map(fold)
}

fn event_year(event: &Value) -> Option<i32> {
    event["year"].as_i64().map(|year| year as i32)
}

struct Hit {
    event_type: String,
    year: Option<i32>,
    place: String,
    clause: String,
    extractor: String,
    page: String,
}

fn extract_page(subject: &str, page: &str, text: &str) -> (Vec<Hit>, usize) {
    let input = ExtractorInput {
        text: text.to_string(),
        page_title: Some(page.to_string()),
        subject_label: Some(subject.to_string()),
        document_type: "article".into(),
        subject_death_year: None,
        wikitext: None,
        known_places: vec![],
    };
    let mut hits = Vec::new();
    let mut dropped = 0;
    for extractor in extractor_stack_for(PersonClass::Unknown) {
        for raw in extractor.extract(&input) {
            if !keep_extracted_raw(&raw, page, subject) {
                dropped += 1;
                continue;
            }
            if !quote_may_attest(&raw.clause_text, raw.place_surface.as_deref()) {
                dropped += 1;
                continue;
            }
            let years = raw
                .time_surface
                .as_deref()
                .map(years_in)
                .filter(|years| !years.is_empty())
                .unwrap_or_else(|| years_in(&raw.clause_text));
            hits.push(Hit {
                event_type: raw.event_type,
                year: years.first().copied(),
                place: raw.place_surface.unwrap_or_default(),
                clause: raw.clause_text.chars().take(240).collect(),
                extractor: raw.extractor_id,
                page: page.to_string(),
            });
        }
    }
    (hits, dropped)
}

fn hit_matches(event: &Value, hit: &Hit) -> bool {
    let class = event["class"].as_str().unwrap_or("");
    if matches!(class, "off_topic" | "wrong_place" | "legendary") {
        return false;
    }
    let place_ok = match place_of(event) {
        None => true,
        Some(place) => {
            let hay = fold(&format!("{} {}", hit.place, hit.clause));
            hay.contains(&place)
        }
    };
    let year_ok = match event_year(event) {
        None => true,
        Some(year) => hit.year == Some(year) || hit.year == Some(year.abs()) || years_in(&hit.clause).iter().any(|found| *found == year || *found == year.abs()),
    };
    place_ok && year_ok
}

fn classify_notice(title: &str, focus_places: &[String]) -> &'static str {
    let focuses: Vec<&str> = focus_places.iter().map(String::as_str).collect();
    let relations: Vec<DocumentaryRelation> = if focuses.is_empty() {
        vec![documentary_relation(title, None)]
    } else {
        focuses
            .iter()
            .map(|place| documentary_relation(title, Some(place)))
            .collect()
    };
    if relations.contains(&DocumentaryRelation::Direct) {
        "direct"
    } else if relations.contains(&DocumentaryRelation::Context)
        || relations.contains(&DocumentaryRelation::BibliographicNotice)
    {
        "notice_or_context"
    } else if relations.iter().all(|relation| *relation == DocumentaryRelation::OffTopic) {
        "off_topic"
    } else {
        "undetermined"
    }
}

async fn wiki_extract(
    client: &reqwest::Client,
    lang: &str,
    title: &str,
    cap: usize,
) -> Result<String, String> {
    let response = client
        .get(format!("https://{lang}.wikipedia.org/w/api.php"))
        .header("accept-encoding", "identity")
        .query(&[
            ("action", "query"),
            ("prop", "extracts"),
            ("explaintext", "1"),
            ("titles", title),
            ("redirects", "1"),
            ("format", "json"),
        ])
        .send()
        .await
        .map_err(|err| err.to_string())?;
    let status = response.status();
    let body = response.text().await.map_err(|err| err.to_string())?;
    if !status.is_success() {
        return Err(format!("{status} for {title}"));
    }
    let parsed: Value = serde_json::from_str(&body).map_err(|err| format!("{err} for {title}"))?;
    let extract = parsed
        .pointer("/query/pages")
        .and_then(|pages| pages.as_object())
        .and_then(|pages| pages.values().next())
        .and_then(|page| page.get("extract"))
        .and_then(|extract| extract.as_str())
        .unwrap_or("")
        .trim();
    if extract.chars().count() < 200 {
        return Err(format!("short extract for {title}"));
    }
    Ok(extract.chars().take(cap).collect())
}

async fn catalog_titles(client: &reqwest::Client, source: &str, query: &str) -> Result<Vec<String>, String> {
    let pairs: Vec<(&str, &str)> = if source == "hal" {
        vec![
            ("q", query),
            ("fl", "title_s"),
            ("wt", "json"),
            ("rows", "5"),
        ]
    } else {
        vec![("search", query), ("per-page", "5")]
    };
    let response = client
        .get(if source == "hal" {
            "https://api.hal.science/search/"
        } else {
            "https://api.openalex.org/works"
        })
        .header("accept-encoding", "identity")
        .query(&pairs)
        .send()
        .await
        .map_err(|err| err.to_string())?;
    let status = response.status();
    let body = response.text().await.map_err(|err| err.to_string())?;
    if !status.is_success() {
        return Err(format!("{status}: {}", body.chars().take(180).collect::<String>()));
    }
    let parsed: Value = serde_json::from_str(&body).map_err(|err| err.to_string())?;
    if source == "hal" {
        Ok(parsed
            .pointer("/response/docs")
            .and_then(|docs| docs.as_array())
            .map(|docs| {
                docs.iter()
                    .filter_map(|doc| doc.get("title_s"))
                    .flat_map(|title| match title {
                        Value::String(text) => vec![text.clone()],
                        Value::Array(items) => items
                            .iter()
                            .filter_map(|item| item.as_str().map(str::to_string))
                            .collect(),
                        _ => vec![],
                    })
                    .collect()
            })
            .unwrap_or_default())
    } else {
        Ok(parsed
            .pointer("/results")
            .and_then(|docs| docs.as_array())
            .map(|docs| {
                docs.iter()
                    .filter_map(|doc| doc.get("display_name").and_then(|name| name.as_str()))
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default())
    }
}

#[tokio::test]
#[ignore = "live Wikipedia, HAL and OpenAlex calls; no database and no paid model"]
async fn bounded_live_probe_writes_a_measurement() {
    let started = Instant::now();
    let reference = load_reference();
    let client = reqwest::Client::builder()
        .user_agent("TalariaEngine/0.1 (documentary probe; no ingest)")
        .timeout(Duration::from_secs(25))
        .build()
        .unwrap();

    let identity = client
        .get("https://www.wikidata.org/w/api.php")
        .header("accept-encoding", "identity")
        .query(&[
            ("action", "wbgetentities"),
            ("ids", "Q517|Q535|Q7186|Q762|Q868|Q466987"),
            ("props", "labels|descriptions"),
            ("languages", "en|mul"),
            ("format", "json"),
        ])
        .send()
        .await
        .unwrap()
        .json::<Value>()
        .await
        .unwrap();
    let napoleon_label = identity
        .pointer("/entities/Q517/labels/en/value")
        .and_then(|value| value.as_str())
        .unwrap_or("");
    assert!(napoleon_label.eq_ignore_ascii_case("Napoleon"));

    let mut people = Vec::new();
    for person in reference["persons"].as_array().unwrap() {
        let label = person["label"].as_str().unwrap();
        let mut pages_ok = 0;
        let mut page_errors = Vec::new();
        let mut hits = Vec::new();
        let mut dropped = 0;
        for page in person["pages"].as_array().unwrap() {
            let title = page.as_str().unwrap();
            let cap = if person["pages"].as_array().unwrap().len() > 1 {
                4000
            } else {
                20000
            };
            match wiki_extract(&client, "en", title, cap).await {
                Ok(extract) => {
                    pages_ok += 1;
                    let (page_hits, page_dropped) = extract_page(label, title, &extract);
                    hits.extend(page_hits);
                    dropped += page_dropped;
                }
                Err(err) => page_errors.push(err),
            }
        }
        let focus_places: Vec<String> = person["events"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|event| {
                matches!(
                    event["class"].as_str(),
                    Some("attested_direct" | "attested_imprecise" | "partial_date" | "no_coordinates" | "absent_searchable")
                )
            })
            .filter_map(place_of)
            .filter(|place| place.chars().count() >= 4 && place != "paris" && place != "italy" && place != "athens")
            .collect();
        let query = person["label"].as_str().unwrap();
        let hal_query = if person["pages"].as_array().unwrap().len() > 1 {
            person["events"]
                .as_array()
                .unwrap()
                .iter()
                .filter_map(place_of)
                .find(|place| {
                    place.chars().count() >= 5 && place != "paris" && place != "italy" && place != "athens"
                })
                .unwrap_or_else(|| fold(query))
        } else {
            fold(query)
        };
        let hal = catalog_titles(&client, "hal", &hal_query).await.unwrap_or_else(|err| vec![format!("error:{err}")]);
        let openalex = catalog_titles(&client, "openalex", query)
            .await
            .unwrap_or_else(|err| vec![format!("error:{err}")]);
        let mut notices = 0;
        let mut off_topic = 0;
        let mut catalog_direct = 0;
        for title in hal.iter().chain(openalex.iter()) {
            if title.starts_with("error:") {
                continue;
            }
            match classify_notice(title, &focus_places) {
                "direct" => catalog_direct += 1,
                "off_topic" => off_topic += 1,
                _ => notices += 1,
            }
        }
        let mut matched = Vec::new();
        let mut missing = Vec::new();
        for event in person["events"].as_array().unwrap() {
            let class = event["class"].as_str().unwrap();
            if matches!(class, "off_topic" | "wrong_place" | "legendary" | "redundant") {
                continue;
            }
            if hits.iter().any(|hit| hit_matches(event, hit)) {
                matched.push(event["id"].as_str().unwrap());
            } else {
                missing.push(event["id"].as_str().unwrap());
            }
        }
        let traps: Vec<&str> = person["traps"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|trap| trap.as_str())
            .collect();
        let trap_hits = hits
            .iter()
            .filter(|hit| {
                let clause = fold(&hit.clause);
                traps.iter().any(|trap| clause.contains(&fold(trap)))
            })
            .count();
        let unique_keys: std::collections::BTreeSet<(String, Option<i32>, String)> = hits
            .iter()
            .map(|hit| (hit.event_type.clone(), hit.year, fold(&hit.place)))
            .collect();
        let anecdote_hits: Vec<&str> = person["events"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|event| event["anecdote"] == true && matched.contains(&event["id"].as_str().unwrap()))
            .map(|event| event["id"].as_str().unwrap())
            .collect();
        let examples: Vec<Value> = hits
            .iter()
            .take(8)
            .map(|hit| {
                json!({
                    "type": hit.event_type,
                    "year": hit.year,
                    "place": hit.place,
                    "page": hit.page,
                    "extractor": hit.extractor,
                    "clause": hit.clause,
                })
            })
            .collect();
        people.push(json!({
            "label": label,
            "qid": person["qid"],
            "pages_fetched": pages_ok,
            "page_errors": page_errors,
            "candidate_rows": hits.len(),
            "dropped_before_attest": dropped,
            "unique_candidates": unique_keys.len(),
            "reference_found": matched,
            "reference_missing": missing,
            "catalog_titles": hal.len() + openalex.len(),
            "catalog_sample": hal.iter().chain(openalex.iter()).take(6).cloned().collect::<Vec<_>>(),
            "catalog_notices_or_context": notices,
            "catalog_off_topic": off_topic,
            "catalog_direct_titles": catalog_direct,
            "trap_hits": trap_hits,
            "anecdote_candidates": anecdote_hits,
            "anecdotes_validated": 0,
            "examples": examples,
        }));
    }

    let report = json!({
        "measured_at": "2026-11-16",
        "writes": "none",
        "database": "not opened",
        "paid_model": "not called",
        "cost": 0,
        "caps": {
            "wikipedia_extract_chars_battle": 4000,
            "wikipedia_extract_chars_biography": 20000,
            "catalog_rows": 5,
            "sources": ["wikipedia extracts", "wikidata labels", "hal", "openalex"]
        },
        "elapsed_ms": started.elapsed().as_millis(),
        "people": people,
        "identity_check": napoleon_label,
    });
    let out = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/audits/2026-11-16-v37-live-probe.json");
    if !out.exists() {
        std::fs::write(&out, serde_json::to_string_pretty(&report).unwrap()).unwrap();
    }
    assert!(started.elapsed() < Duration::from_secs(180));
    assert!(report["people"].as_array().unwrap().len() == 6);
}

const KNOWN_NAPOLEON: &[KnownEvent<'static>] = &[
    KnownEvent {
        id: "07764eba-3f83-4a1f-a737-a556caf4ce8f",
        logical_id: "472a66c3-2775-45cd-9660-fcaf7afb70a8",
        event_type: "battle",
        year: 1796,
        title: "The Battle of Lonato was fought on 3 and 4 August 1796",
        place_label: "Italy",
    },
    KnownEvent {
        id: "stored-montenotte",
        logical_id: "stored-montenotte",
        event_type: "battle",
        year: 1796,
        title: "The Battle of Montenotte was fought on 12 April 1796",
        place_label: "Montenotte",
    },
    KnownEvent {
        id: "stored-millesimo",
        logical_id: "stored-millesimo",
        event_type: "battle",
        year: 1796,
        title: "The Battle of Millesimo was fought on 13 April 1796",
        place_label: "Millesimo",
    },
    KnownEvent {
        id: "stored-mondovi",
        logical_id: "stored-mondovi",
        event_type: "battle",
        year: 1796,
        title: "The Battle of Mondovì was fought on 21 April 1796",
        place_label: "Mondovì",
    },
    KnownEvent {
        id: "stored-lodi",
        logical_id: "stored-lodi",
        event_type: "battle",
        year: 1796,
        title: "The Battle of Lodi was fought on 10 May 1796",
        place_label: "Lodi",
    },
    KnownEvent {
        id: "stored-rivoli",
        logical_id: "stored-rivoli",
        event_type: "battle",
        year: 1797,
        title: "The Battle of Rivoli was fought on 14 January 1797",
        place_label: "Rivoli",
    },
    KnownEvent {
        id: "stored-bassano",
        logical_id: "stored-bassano",
        event_type: "battle",
        year: 1796,
        title: "The Battle of Bassano was fought on 8 September 1796",
        place_label: "Bassano",
    },
];

#[tokio::test]
#[ignore = "second live probe; does not rewrite the V3.7 baseline"]
async fn v38_probe_dedup_place_and_french_gap() {
    tokio::time::sleep(Duration::from_secs(3)).await;
    let started = Instant::now();
    let reference = load_reference();
    let client = reqwest::Client::builder()
        .user_agent("TalariaEngine/0.1 (v38 probe; no ingest)")
        .timeout(Duration::from_secs(25))
        .build()
        .unwrap();
    let mut people = Vec::new();
    for person in reference["persons"].as_array().unwrap() {
        let label = person["label"].as_str().unwrap();
        let mut hits = Vec::new();
        let mut dropped = 0usize;
        let mut pages = 0usize;
        let mut french_added = false;
        let mut errors = Vec::new();
        for page in person["pages"].as_array().unwrap() {
            let title = page.as_str().unwrap();
            let cap = if person["pages"].as_array().unwrap().len() > 1 {
                4000
            } else {
                20000
            };
            match wiki_extract(&client, "en", title, cap).await {
                Ok(extract) => {
                    pages += 1;
                    let (page_hits, page_dropped) = extract_page(label, title, &extract);
                    hits.extend(page_hits);
                    dropped += page_dropped;
                }
                Err(err) => {
                    tokio::time::sleep(Duration::from_secs(6)).await;
                    match wiki_extract(&client, "en", title, cap).await {
                        Ok(extract) => {
                            pages += 1;
                            let (page_hits, page_dropped) = extract_page(label, title, &extract);
                            hits.extend(page_hits);
                            dropped += page_dropped;
                        }
                        Err(retry_err) => errors.push(format!("{title}: {err}; retry: {retry_err}")),
                    }
                }
            }
            tokio::time::sleep(Duration::from_millis(400)).await;
        }
        if person["qid"] == "Q466987" {
            if let Ok(extract) = wiki_extract(&client, "fr", "Marceline Desbordes-Valmore", 4000).await {
                french_added = extract.contains("1786") && extract.to_lowercase().contains("douai");
                let (page_hits, page_dropped) = extract_page(label, "Marceline Desbordes-Valmore", &extract);
                hits.extend(page_hits);
                dropped += page_dropped;
                pages += 1;
            }
        }
        let mut realm_rejected = 0usize;
        let mut keys = std::collections::BTreeSet::new();
        let mut decisions = Vec::new();
        let mut anecdotes = Vec::new();
        for hit in &hits {
            let place = anchored_action_place(&hit.clause, &hit.event_type, &hit.place);
            if matches!(hit.event_type.as_str(), "battle" | "siege") && place.is_none() {
                realm_rejected += 1;
                continue;
            }
            let place = place.unwrap_or_default();
            let key = proposition_key(&hit.event_type, hit.year, &place, &hit.clause);
            if !keys.insert(key) {
                continue;
            }
            if person["qid"] == "Q517" && hit.event_type == "battle" {
                let decision = novelty_of(&hit.event_type, hit.year, &place, &hit.clause, KNOWN_NAPOLEON);
                decisions.push(format!(
                    "{} {} {place}: {decision:?}",
                    hit.event_type,
                    hit.year.unwrap_or(0)
                ));
            }
            if narrative_reading(&hit.event_type, &hit.clause, &hit.clause) == NarrativeReading::Candidate {
                anecdotes.push(hit.clause.chars().take(160).collect::<String>());
            }
        }
        let new_count = decisions.iter().filter(|line| line.contains("New")).count();
        let duplicate_count = decisions.iter().filter(|line| line.contains("Duplicate")).count();
        people.push(json!({
            "label": label,
            "qid": person["qid"],
            "pages_fetched": pages,
            "page_errors": errors,
            "french_gap_fetch": french_added,
            "rows_before_dedup": hits.len(),
            "unique_after_anchor": keys.len(),
            "dropped_before_attest": dropped,
            "realm_places_rejected": realm_rejected,
            "napoleon_new": new_count,
            "napoleon_duplicates": duplicate_count,
            "decisions": decisions,
            "anecdote_candidates": anecdotes.len(),
            "anecdotes_validated": 0,
            "anecdote_examples": anecdotes.into_iter().take(3).collect::<Vec<_>>(),
        }));
    }
    let report = json!({
        "measured_at": "2026-11-16",
        "baseline": "2026-11-16-v37-live-probe.json",
        "writes": "none",
        "database": "not opened",
        "paid_model": "not called",
        "cost": 0,
        "comparable_to_v37": "same page list and character caps; one extra French page for Desbordes-Valmore because the English window lacked the birth date",
        "elapsed_ms": started.elapsed().as_millis(),
        "people": people,
    });
    let out = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/audits/2026-11-16-v38-live-probe.json");
    std::fs::write(&out, serde_json::to_string_pretty(&report).unwrap()).unwrap();
    let napoleon = report["people"]
        .as_array()
        .unwrap()
        .iter()
        .find(|person| person["qid"] == "Q517")
        .unwrap();
    let decisions = napoleon["decisions"].as_array().unwrap();
    assert!(
        decisions.iter().any(|line| line.as_str().unwrap_or("").contains("Lonato")
            && line.as_str().unwrap_or("").contains("Duplicate")),
        "Lonato must be recognized as the stored battle, got {decisions:?}"
    );
    assert!(started.elapsed() < Duration::from_secs(180));
}
