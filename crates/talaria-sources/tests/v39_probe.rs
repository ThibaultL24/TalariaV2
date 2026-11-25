// crates/talaria-sources/tests/v39_probe.rs
//! Targeted gap probe. It does not open Postgres and does not rewrite the V3.7 or V3.8 files.

use std::time::{Duration, Instant};

use serde_json::{json, Value};
use talaria_quality::{
    anecdote_standing, novelty_of, quote_may_attest, relevant_paragraphs, KnownEvent, Novelty,
};
use talaria_sources::extractors::{extractor_stack_for, keep_extracted_raw, ExtractorInput};
use talaria_sources::PersonClass;

const GAPS: &[(&str, &str, i32)] = &[
    ("Battle of Arcole", "arcole", 1796),
    ("Armistice of Cherasco", "cherasco", 1796),
    ("Treaty of Campo Formio", "campo", 1797),
    ("Battle of Castiglione", "castiglione", 1796),
    ("Siege of Mantua (1796–1797)", "mantua", 1797),
    ("Battle of Dego", "dego", 1796),
    ("Battle of Ceva", "ceva", 1796),
    ("Battle of Tolentino", "tolentino", 1815),
];

const LONATO: KnownEvent<'static> = KnownEvent {
    id: "07764eba-3f83-4a1f-a737-a556caf4ce8f",
    logical_id: "472a66c3-2775-45cd-9660-fcaf7afb70a8",
    event_type: "battle",
    year: 1796,
    title: "The Battle of Lonato was fought on 3 and 4 August 1796",
    place_label: "Italy",
};

#[tokio::test]
#[ignore = "live gap probe; does not rewrite the V3.7 or V3.8 baselines"]
async fn v39_probe_targeted_gaps() {
    tokio::time::sleep(Duration::from_secs(2)).await;
    let started = Instant::now();
    let client = reqwest::Client::builder()
        .user_agent("TalariaEngine/0.1 (v39 gap probe; no ingest)")
        .timeout(Duration::from_secs(25))
        .build()
        .unwrap();
    let mut gaps = Vec::new();
    for (title, stem, year) in GAPS {
        let titles: &[&str] = if *title == "Battle of Dego" {
            &["Battle of Dego", "Second Battle of Dego"]
        } else {
            &[title]
        };
        let mut text = String::new();
        let mut error = None;
        for candidate in titles {
            tokio::time::sleep(Duration::from_millis(700)).await;
            match fetch_text(&client, "en", candidate).await {
                Ok(found) => {
                    text = found;
                    error = None;
                    break;
                }
                Err(err) => error = Some(err),
            }
        }
        gaps.push(diagnose(title, stem, *year, &text, error));
    }
    let hugo = fetch_text(&client, "en", "Victor Hugo").await.unwrap_or_default();
    let curie = fetch_text(&client, "en", "Marie Curie").await.unwrap_or_default();
    let marceline = fetch_text(&client, "fr", "Marceline Desbordes-Valmore")
        .await
        .unwrap_or_default();
    let biographies = json!({
        "victor_hugo": biography_span(&hugo, &["john brown", "guernsey", "exile"]),
        "marie_curie": biography_span(&curie, &["szczuki", "governess"]),
        "desbordes": biography_span(&marceline, &["calvaire", "douai", "1786"]),
    });
    let report = json!({
        "measured_at": "2026-11-16",
        "baseline": "2026-11-16-v38-live-probe.json",
        "fixed_benchmark": "unit tests in military, proposition, documentary and occurrence",
        "writes": "none",
        "database": "not opened",
        "paid_model": "not called",
        "cost": 0,
        "pages": GAPS.len() + 3,
        "elapsed_ms": started.elapsed().as_millis(),
        "gaps": gaps,
        "biographies": biographies,
    });
    let out = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/audits/2026-11-16-v39-live-probe.json");
    std::fs::write(&out, serde_json::to_string_pretty(&report).unwrap()).unwrap();
    let arcole = gap(&report, "Battle of Arcole");
    assert_eq!(arcole["stage"], "extracted_from_opening", "{arcole}");
    assert_eq!(gap(&report, "Armistice of Cherasco")["stage"], "extracted_from_opening");
    assert_eq!(gap(&report, "Treaty of Campo Formio")["stage"], "extracted_from_opening");
    assert!(report["elapsed_ms"].as_u64().unwrap() < 180_000);
}

fn gap<'a>(report: &'a Value, title: &str) -> &'a Value {
    report["gaps"]
        .as_array()
        .unwrap()
        .iter()
        .find(|gap| gap["title"] == title)
        .unwrap()
}

fn diagnose(title: &str, stem: &str, expected_year: i32, text: &str, error: Option<String>) -> Value {
    if text.len() < 200 {
        return json!({
            "title": title,
            "stage": "document_missing",
            "error": error,
        });
    }
    let lower = text.to_lowercase();
    let head: String = text.chars().take(4000).collect();
    let in_document = lower.contains(stem);
    let in_window = head.to_lowercase().contains(stem);
    let window_hits = kept(title, &head);
    let full_hits = kept(title, text);
    let matching: Vec<_> = full_hits
        .iter()
        .filter(|hit| {
            format!("{} {}", hit.place, hit.clause)
                .to_lowercase()
                .contains(stem)
        })
        .collect();
    let best = matching.iter().copied().min_by_key(|hit| {
        let place_hit = hit.place.to_lowercase().contains(stem);
        let clause = hit.clause.to_lowercase();
        let lead = ["battle of ", "siege of ", "treaty of ", "armistice of "]
            .iter()
            .any(|cue| clause.contains(cue));
        if place_hit && lead {
            0
        } else if place_hit {
            1
        } else if lead {
            2
        } else {
            3
        }
    });
    let stage = if !in_document {
        "absent_from_document"
    } else if matching.is_empty() && in_window {
        "action_or_attribution"
    } else if matching.is_empty() {
        "outside_opening"
    } else if window_hits.iter().any(|hit| {
        format!("{} {}", hit.place, hit.clause)
            .to_lowercase()
            .contains(stem)
    }) {
        "extracted_from_opening"
    } else {
        "extracted_beyond_v38_window"
    };
    let first = best;
    let decision = first.and_then(|hit| {
        (hit.event_type == "battle").then(|| {
            novelty_of(&hit.event_type, hit.year, &hit.place, &hit.clause, &[LONATO])
        })
    });
    json!({
        "title": title,
        "stage": stage,
        "chars": text.chars().count(),
        "stem_in_first_4000": in_window,
        "expected_year": expected_year,
        "year": first.and_then(|hit| hit.year),
        "place": first.map(|hit| hit.place.clone()),
        "event_type": first.map(|hit| hit.event_type.clone()),
        "quote": first.map(|hit| hit.clause.chars().take(280).collect::<String>()),
        "reconciliation": decision.map(label),
        "window_hits": window_hits.len(),
        "full_hits": full_hits.len(),
    })
}

fn label(novelty: Novelty) -> &'static str {
    match novelty {
        Novelty::Duplicate => "existing",
        Novelty::AdditionalEvidence => "additional_evidence",
        Novelty::New => "distinct",
        Novelty::Uncertain | Novelty::Variant => "uncertain",
        Novelty::Rejected => "rejected",
    }
}

struct Hit {
    event_type: String,
    year: Option<i32>,
    place: String,
    clause: String,
}

fn kept(page: &str, text: &str) -> Vec<Hit> {
    let input = ExtractorInput {
        text: text.to_string(),
        page_title: Some(page.to_string()),
        subject_label: Some("Napoleon".into()),
        document_type: "article".into(),
        subject_death_year: Some(1821),
        wikitext: None,
        known_places: vec![],
    };
    let mut hits = Vec::new();
    for extractor in extractor_stack_for(PersonClass::Unknown) {
        for raw in extractor.extract(&input) {
            if !keep_extracted_raw(&raw, page, "Napoleon") {
                continue;
            }
            if !quote_may_attest(&raw.clause_text, raw.place_surface.as_deref()) {
                continue;
            }
            let year = raw
                .time_surface
                .as_deref()
                .and_then(|surface| surface_year(surface))
                .or_else(|| surface_year(&raw.clause_text));
            hits.push(Hit {
                event_type: raw.event_type,
                year,
                place: raw.place_surface.unwrap_or_default(),
                clause: raw.clause_text,
            });
        }
    }
    hits
}

fn surface_year(text: &str) -> Option<i32> {
    let bytes = text.as_bytes();
    let mut index = 0;
    while index + 3 < bytes.len() {
        if bytes[index].is_ascii_digit() {
            let start = index;
            while index < bytes.len() && bytes[index].is_ascii_digit() {
                index += 1;
            }
            if index - start == 4 {
                return text[start..index].parse().ok();
            }
            continue;
        }
        index += 1;
    }
    None
}

fn biography_span(text: &str, needles: &[&str]) -> Value {
    let paras = relevant_paragraphs(text, needles);
    let readings: Vec<_> = paras
        .iter()
        .map(|para| {
            json!({
                "quote": para.chars().take(240).collect::<String>(),
                "standing": format!("{:?}", anecdote_standing("travel", para, false)),
            })
        })
        .collect();
    json!({
        "chars": text.chars().count(),
        "paragraphs_read": readings.len(),
        "passages": readings,
    })
}

async fn fetch_text(client: &reqwest::Client, lang: &str, title: &str) -> Result<String, String> {
    let first = wiki_extract(client, lang, title).await;
    if first.is_ok() {
        return first;
    }
    tokio::time::sleep(Duration::from_secs(6)).await;
    match wiki_extract(client, lang, title).await {
        Ok(text) => Ok(text),
        Err(retry) => Err(format!("{first:?}; retry: {retry}")),
    }
}

async fn wiki_extract(client: &reqwest::Client, lang: &str, title: &str) -> Result<String, String> {
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
    Ok(extract.chars().take(60_000).collect())
}
