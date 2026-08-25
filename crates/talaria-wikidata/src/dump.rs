// crates/talaria-wikidata/src/dump.rs
//! Offline Wikidata JSON dump stream → humans + occupations/positions.

use anyhow::{anyhow, Context, Result};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::{BufRead, BufReader, Read};
use std::path::Path;

const INSTANCE_OF: &str = "P31";
const OCCUPATION: &str = "P106";
const POSITION: &str = "P39";
const DATE_OF_BIRTH: &str = "P569";
const DATE_OF_DEATH: &str = "P570";
const HUMAN_QID: &str = "Q5";

#[derive(Debug, Clone)]
pub struct WikidataSitelink {
    pub site: String,
    pub title: String,
    pub wiki_lang: String,
}

#[derive(Debug, Clone)]
pub struct WikidataProfileRef {
    pub qid: String,
    pub label: String,
    pub slug: String,
    pub kind: String,
}

#[derive(Debug, Clone)]
pub struct WikidataHuman {
    pub qid: String,
    pub label: String,
    pub birth_year: Option<i32>,
    pub death_year: Option<i32>,
    pub profiles: Vec<WikidataProfileRef>,
    pub sitelinks: Vec<WikidataSitelink>,
}

#[derive(Debug, Default)]
pub struct DumpIngestStats {
    pub entities_seen: usize,
    pub entities_emitted: usize,
    pub humans_seen: usize,
    pub humans_emitted: usize,
    pub labels_cached: usize,
}

/// Stream a Wikidata dump (`.json`, `.json.bz2`, or NDJSON) and yield humans.
/// Occupation/position labels resolve from items seen earlier in the same stream.
pub fn stream_humans(
    path: &Path,
    limit: usize,
    mut on_human: impl FnMut(WikidataHuman) -> Result<()>,
) -> Result<DumpIngestStats> {
    let mut stats = DumpIngestStats::default();
    let mut labels: HashMap<String, String> = HashMap::new();
    let mut pending: Vec<Value> = Vec::new();

    for_each_entity(path, |entity| {
        stats.entities_seen += 1;
        let Some(qid) = entity.get("id").and_then(Value::as_str) else {
            return Ok(());
        };
        if let Some(label) = prefer_label(&entity) {
            labels.insert(qid.to_string(), label);
            stats.labels_cached = labels.len();
        }

        if !is_human(&entity) {
            return Ok(());
        }
        stats.humans_seen += 1;

        if limit > 0 && stats.humans_emitted >= limit {
            return Ok(());
        }

        match materialize_human(&entity, &labels) {
            Ok(human) if profiles_resolved(&human) => {
                on_human(human)?;
                stats.entities_emitted += 1;
                stats.humans_emitted += 1;
            }
            Ok(_) | Err(_) => pending.push(entity),
        }
        Ok(())
    })?;

    // Second chance: occupations often appear after the human QID in the dump.
    for entity in pending {
        if limit > 0 && stats.humans_emitted >= limit {
            break;
        }
        let human = materialize_human(&entity, &labels)?;
        on_human(human)?;
        stats.entities_emitted += 1;
        stats.humans_emitted += 1;
    }

    Ok(stats)
}

/// Stream a Wikidata dump and emit **full** entity JSON for QIDs in `keep`.
/// Never occupation-only structs. Callers must pass a neighborhood set — do not
/// default to the whole `latest-all` dump.
pub fn stream_entities_for_qids(
    path: &Path,
    keep: &HashSet<String>,
    mut on_entity: impl FnMut(Value) -> Result<()>,
) -> Result<DumpIngestStats> {
    let mut stats = DumpIngestStats::default();
    for_each_entity(path, |entity| {
        stats.entities_seen += 1;
        let Some(qid) = entity.get("id").and_then(Value::as_str) else {
            return Ok(());
        };
        if is_human(&entity) {
            stats.humans_seen += 1;
        }
        if keep.contains(qid) {
            let human = is_human(&entity);
            on_entity(entity)?;
            stats.entities_emitted += 1;
            if human {
                stats.humans_emitted += 1;
            }
        }
        Ok(())
    })?;
    Ok(stats)
}

fn profiles_resolved(human: &WikidataHuman) -> bool {
    human.profiles.iter().all(|p| !p.label.starts_with('Q') || p.label != p.qid)
}

pub fn for_each_entity(path: &Path, mut on_entity: impl FnMut(Value) -> Result<()>) -> Result<()> {
    let file = File::open(path).with_context(|| format!("open {}", path.display()))?;
    let reader: Box<dyn Read> = match path.extension().and_then(|e| e.to_str()) {
        Some("bz2") => Box::new(bzip2::read::BzDecoder::new(file)),
        Some("gz") => Box::new(flate2::read::GzDecoder::new(file)),
        _ => Box::new(file),
    };
    let mut lines = BufReader::new(reader).lines();

    while let Some(line) = lines.next() {
        let line = line?;
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed == "[" || trimmed == "]" {
            continue;
        }
        let payload = trimmed.trim_end_matches(',').trim();
        if payload.is_empty() {
            continue;
        }
        let entity: Value = serde_json::from_str(payload)
            .with_context(|| format!("parse entity near: {}", &payload[..payload.len().min(80)]))?;
        if entity.get("type").and_then(Value::as_str) != Some("item") {
            continue;
        }
        on_entity(entity)?;
    }
    Ok(())
}

fn is_human(entity: &Value) -> bool {
    claim_item_ids(entity, INSTANCE_OF)
        .into_iter()
        .any(|id| id == HUMAN_QID)
}

fn materialize_human(entity: &Value, labels: &HashMap<String, String>) -> Result<WikidataHuman> {
    let qid = entity
        .get("id")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("human missing id"))?
        .to_string();
    let label = prefer_label(entity).unwrap_or_else(|| qid.clone());

    let mut profiles = Vec::new();
    for (prop, kind) in [(OCCUPATION, "occupation"), (POSITION, "position")] {
        for profile_qid in claim_item_ids(entity, prop) {
            let label = labels
                .get(&profile_qid)
                .cloned()
                .unwrap_or_else(|| profile_qid.clone());
            profiles.push(WikidataProfileRef {
                slug: slugify(&label),
                label,
                qid: profile_qid,
                kind: kind.to_string(),
            });
        }
    }

    Ok(WikidataHuman {
        qid,
        label,
        birth_year: claim_year(entity, DATE_OF_BIRTH),
        death_year: claim_year(entity, DATE_OF_DEATH),
        profiles,
        sitelinks: extract_sitelinks(entity),
    })
}

fn prefer_label(entity: &Value) -> Option<String> {
    let labels = entity.get("labels")?.as_object()?;
    for lang in ["en", "fr", "de", "es", "it"] {
        if let Some(value) = labels
            .get(lang)
            .and_then(|v| v.get("value"))
            .and_then(Value::as_str)
        {
            return Some(value.to_string());
        }
    }
    labels
        .values()
        .next()
        .and_then(|v| v.get("value"))
        .and_then(Value::as_str)
        .map(str::to_string)
}

fn claim_item_ids(entity: &Value, property: &str) -> Vec<String> {
    let Some(claims) = entity
        .get("claims")
        .and_then(|c| c.get(property))
        .and_then(Value::as_array)
    else {
        return Vec::new();
    };

    claims
        .iter()
        .filter_map(|claim| {
            claim
                .pointer("/mainsnak/datavalue/value/id")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .collect()
}

fn claim_year(entity: &Value, property: &str) -> Option<i32> {
    let claims = entity.get("claims")?.get(property)?.as_array()?;
    for claim in claims {
        let Some(time) = claim
            .pointer("/mainsnak/datavalue/value/time")
            .and_then(Value::as_str)
        else {
            continue;
        };
        if let Some(parsed) = crate::time::parse_wikibase_time(time, None, None) {
            return Some(parsed.year);
        }
    }
    None
}

fn extract_sitelinks(entity: &Value) -> Vec<WikidataSitelink> {
    let Some(links) = entity.get("sitelinks").and_then(Value::as_object) else {
        return Vec::new();
    };
    links
        .iter()
        .filter_map(|(site, value)| {
            if !site.ends_with("wiki") || site.contains("commons") || site.contains("wikidata") {
                return None;
            }
            let title = value.get("title")?.as_str()?.to_string();
            let wiki_lang = site.trim_end_matches("wiki").to_string();
            if wiki_lang.is_empty() {
                return None;
            }
            Some(WikidataSitelink {
                site: site.clone(),
                title,
                wiki_lang,
            })
        })
        .collect()
}

pub fn slugify(label: &str) -> String {
    let mut out = String::new();
    let mut prev_dash = false;
    for ch in label.chars() {
        let lower = ch.to_ascii_lowercase();
        if lower.is_ascii_alphanumeric() {
            out.push(lower);
            prev_dash = false;
        } else if !prev_dash && !out.is_empty() {
            out.push('-');
            prev_dash = true;
        }
    }
    out.trim_matches('-').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn parses_human_with_prior_occupation_labels() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(
            file,
            r#"[
{{"type":"item","id":"Q82955","labels":{{"en":{{"language":"en","value":"politician"}}}}}},
{{"type":"item","id":"Q1744","labels":{{"en":{{"language":"en","value":"Madonna"}}}},"claims":{{"P31":[{{"mainsnak":{{"datavalue":{{"value":{{"id":"Q5"}}}}}}}}],"P106":[{{"mainsnak":{{"datavalue":{{"value":{{"id":"Q82955"}}}}}}}}]}},"sitelinks":{{"enwiki":{{"site":"enwiki","title":"Madonna"}}}}}}
]"#
        )
        .unwrap();

        let mut humans = Vec::new();
        let stats = stream_humans(file.path(), 0, |h| {
            humans.push(h);
            Ok(())
        })
        .unwrap();

        assert_eq!(stats.humans_emitted, 1);
        assert_eq!(humans[0].qid, "Q1744");
        assert_eq!(humans[0].profiles[0].slug, "politician");
        assert_eq!(humans[0].sitelinks[0].title, "Madonna");
    }

    #[test]
    fn claim_year_parses_bce_via_stream_humans() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(
            file,
            r#"[
{{"type":"item","id":"Q1048","labels":{{"en":{{"language":"en","value":"Julius Caesar"}}}},"claims":{{"P31":[{{"mainsnak":{{"datavalue":{{"value":{{"id":"Q5"}}}}}}}}],"P569":[{{"mainsnak":{{"snaktype":"novalue"}}}},{{"mainsnak":{{"datavalue":{{"value":{{"time":"-0044-03-15T00:00:00Z"}}}}}}}}]}}}}
]"#
        )
        .unwrap();

        let mut humans = Vec::new();
        stream_humans(file.path(), 0, |h| {
            humans.push(h);
            Ok(())
        })
        .unwrap();

        assert_eq!(humans[0].birth_year, Some(-44));
    }

    fn napoleon_paris_dump(file: &mut NamedTempFile) {
        writeln!(
            file,
            r#"[
{{"type":"item","id":"Q90","labels":{{"en":{{"language":"en","value":"Paris"}}}}}},
{{"type":"item","id":"Q517","labels":{{"en":{{"language":"en","value":"Napoleon"}}}},"claims":{{"P31":[{{"mainsnak":{{"datavalue":{{"value":{{"id":"Q5"}}}}}}}}],"P551":[{{"mainsnak":{{"datavalue":{{"value":{{"id":"Q90"}}}}}}}}]}}}}
]"#
        )
        .unwrap();
    }

    #[test]
    fn streams_kept_qids_with_full_claims() {
        let mut file = NamedTempFile::new().unwrap();
        napoleon_paris_dump(&mut file);

        let keep = HashSet::from(["Q517".to_string()]);
        let mut received = Vec::new();
        let stats = stream_entities_for_qids(file.path(), &keep, |entity| {
            received.push(entity);
            Ok(())
        })
        .unwrap();

        assert_eq!(received.len(), 1);
        assert_eq!(received[0]["id"], "Q517");
        assert_eq!(stats.entities_emitted, 1);
        assert_eq!(stats.humans_emitted, 1);
        assert!(
            received[0].get("claims").is_some(),
            "callback must receive full entity JSON including claims"
        );
        assert_eq!(
            received[0].pointer("/claims/P551/0/mainsnak/datavalue/value/id"),
            Some(&Value::String("Q90".into()))
        );
        assert!(received.iter().all(|e| e["id"] != "Q90"));
    }

    #[test]
    fn neighborhood_stats_count_places_separately_from_humans() {
        let mut file = NamedTempFile::new().unwrap();
        napoleon_paris_dump(&mut file);

        let keep = HashSet::from(["Q90".to_string(), "Q517".to_string()]);
        let mut received = Vec::new();
        let stats = stream_entities_for_qids(file.path(), &keep, |entity| {
            received.push(entity);
            Ok(())
        })
        .unwrap();

        assert_eq!(received.len(), 2);
        assert_eq!(stats.entities_emitted, 2);
        assert_eq!(stats.humans_emitted, 1);
    }
}
