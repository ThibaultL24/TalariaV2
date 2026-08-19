// crates/talaria-sources/src/wdqs.rs
//! Wikidata Query Service event harvest (P710 / P1344 participation + biography).

use std::collections::HashSet;
use std::path::Path;
use std::time::Duration;

use serde_json::Value;

const WDQS_ENDPOINT: &str = "https://query.wikidata.org/sparql";
const UA: &str = "TalariaEngine/0.1 (wdqs; research; https://github.com/talaria)";

#[derive(Debug, Clone, PartialEq)]
pub struct WdqsEvent {
    pub event_qid: String,
    pub label: String,
    pub date: String,
    pub place_qid: Option<String>,
    pub place_label: Option<String>,
    pub event_type: String,
    pub lat: Option<f64>,
    pub lon: Option<f64>,
}

pub fn parse_sparql_bindings(payload: &Value, default_type: Option<&str>) -> Vec<WdqsEvent> {
    let Some(bindings) = payload.pointer("/results/bindings").and_then(|v| v.as_array()) else {
        return vec![];
    };
    let mut out = Vec::new();
    for row in bindings {
        let event_uri = binding_str(row, "event").or_else(|| binding_str(row, "battle"));
        let Some(event_qid) = event_uri.as_deref().and_then(event_id_from_uri) else {
            continue;
        };
        let date = binding_str(row, "date").and_then(|d| normalize_date(&d));
        let label = binding_str(row, "eventLabel")
            .or_else(|| binding_str(row, "battleLabel"))
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| event_qid.clone());
        let place_qid = binding_str(row, "place").as_deref().and_then(qid_from_uri);
        let place_label = binding_str(row, "placeLabel").filter(|s| !s.is_empty());
        let ev_type = binding_str(row, "evType")
            .filter(|s| !s.is_empty())
            .or_else(|| default_type.map(str::to_string))
            .unwrap_or_else(|| "historical_fact".into());
        let event_type = classify_event(&ev_type, &label);
        let dated_required = !matches!(
            event_type.as_str(),
            "birth" | "death" | "residence" | "office" | "education" | "publication"
        );
        let Some(date) = date.or_else(|| {
            if dated_required {
                None
            } else {
                Some(String::new())
            }
        }) else {
            continue;
        };
        let (lat, lon) = parse_geo(row);
        out.push(WdqsEvent {
            event_qid,
            label,
            date,
            place_qid,
            place_label,
            event_type,
            lat,
            lon,
        });
    }
    out
}

pub fn merge_events_for_person(
    p710: &[WdqsEvent],
    p1344: &[WdqsEvent],
    _p607_battles: &[WdqsEvent],
) -> Vec<WdqsEvent> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for ev in p710.iter().chain(p1344.iter()) {
        if !seen.insert(ev.event_qid.clone()) {
            continue;
        }
        out.push(ev.clone());
    }
    out.sort_by(|a, b| a.date.cmp(&b.date).then_with(|| a.event_qid.cmp(&b.event_qid)));
    out
}

pub fn events_to_statement_text(events: &[WdqsEvent]) -> String {
    let mut lines = Vec::with_capacity(events.len());
    for ev in events {
        let year = ev.date.get(..4).unwrap_or(&ev.date);
        let pred = predicate_for(&ev.event_type);
        let place = ev.place_label.clone().unwrap_or_default();
        let lat = ev.lat.map(|v| format!("{v:.6}")).unwrap_or_default();
        let lon = ev.lon.map(|v| format!("{v:.6}")).unwrap_or_default();
        lines.push(format!(
            "STATEMENT\t{}\t{pred}\t{year}\t{place}\t{}\t{lat}\t{lon}",
            ev.event_type, ev.label
        ));
    }
    lines.join("\n")
}

pub fn events_from_fixture_dir(dir: &Path) -> anyhow::Result<Vec<WdqsEvent>> {
    let p710 = load_json(&dir.join("participant.json"))?;
    let p1344 = load_json(&dir.join("participant_in.json"))?;
    let battles = load_json(&dir.join("battles.json"))?;
    Ok(merge_events_for_person(
        &parse_sparql_bindings(&p710, None),
        &parse_sparql_bindings(&p1344, None),
        &parse_sparql_bindings(&battles, Some("battle")),
    ))
}

pub async fn fetch_events_for_person(qid: &str) -> anyhow::Result<Vec<WdqsEvent>> {
    let qid = qid.trim().to_uppercase();
    anyhow::ensure!(
        qid.starts_with('Q') && qid[1..].chars().all(|c| c.is_ascii_digit()),
        "invalid qid {qid}"
    );
    let client = reqwest::Client::builder()
        .user_agent(UA)
        .timeout(Duration::from_secs(120))
        .build()?;
    let payload = sparql_json(&client, &events_for_person_query(&qid, 2000)).await?;
    Ok(merge_events_for_person(
        &parse_sparql_bindings(&payload, None),
        &[],
        &[],
    ))
}

pub fn events_for_person_query(qid: &str, limit: u32) -> String {
    let lim = limit.clamp(1, 10_000);
    format!(
        r#"PREFIX wd: <http://www.wikidata.org/entity/>
PREFIX wdt: <http://www.wikidata.org/prop/direct/>
PREFIX p: <http://www.wikidata.org/prop/>
PREFIX ps: <http://www.wikidata.org/prop/statement/>
PREFIX pq: <http://www.wikidata.org/prop/qualifier/>
PREFIX wikibase: <http://wikiba.se/ontology#>
PREFIX bd: <http://www.bigdata.com/rdf#>
SELECT DISTINCT ?event ?eventLabel ?date ?place ?placeLabel ?evType ?geo ?pgeo WHERE {{
  {{
    ?event wdt:P710 wd:{qid} .
    BIND("historical_fact" AS ?evType)
    OPTIONAL {{ ?event wdt:P585 ?date . }}
    OPTIONAL {{ ?event wdt:P580 ?date . }}
    OPTIONAL {{ ?event wdt:P276 ?place . }}
  }} UNION {{
    wd:{qid} wdt:P1344 ?event .
    BIND("historical_fact" AS ?evType)
    OPTIONAL {{ ?event wdt:P585 ?date . }}
    OPTIONAL {{ ?event wdt:P580 ?date . }}
    OPTIONAL {{ ?event wdt:P276 ?place . }}
  }} UNION {{
    wd:{qid} wdt:P19 ?place .
    wd:{qid} wdt:P569 ?date .
    BIND("birth" AS ?evType)
    BIND(IRI(CONCAT("http://www.wikidata.org/entity/{qid}-birth")) AS ?event)
  }} UNION {{
    wd:{qid} wdt:P20 ?place .
    wd:{qid} wdt:P570 ?date .
    BIND("death" AS ?evType)
    BIND(IRI(CONCAT("http://www.wikidata.org/entity/{qid}-death")) AS ?event)
  }} UNION {{
    wd:{qid} p:P551 ?st .
    ?st ps:P551 ?place .
    OPTIONAL {{ ?st pq:P580 ?date . }}
    OPTIONAL {{ ?st pq:P585 ?date . }}
    BIND("residence" AS ?evType)
    BIND(IRI(CONCAT("http://www.wikidata.org/entity/{qid}-res-", STRAFTER(STR(?place), "entity/"))) AS ?event)
  }} UNION {{
    wd:{qid} p:P69 ?st .
    ?st ps:P69 ?place .
    OPTIONAL {{ ?st pq:P580 ?date . }}
    OPTIONAL {{ ?st pq:P582 ?date . }}
    BIND("education" AS ?evType)
    BIND(IRI(CONCAT("http://www.wikidata.org/entity/{qid}-edu-", STRAFTER(STR(?place), "entity/"))) AS ?event)
  }} UNION {{
    wd:{qid} p:P39 ?st .
    ?st ps:P39 ?office .
    OPTIONAL {{ ?st pq:P580 ?date . }}
    OPTIONAL {{ ?st pq:P585 ?date . }}
    OPTIONAL {{ ?st pq:P937 ?place . }}
    OPTIONAL {{ ?office wdt:P159 ?place . }}
    BIND("office" AS ?evType)
    BIND(IRI(CONCAT("http://www.wikidata.org/entity/{qid}-off-", STRAFTER(STR(?office), "entity/"))) AS ?event)
  }} UNION {{
    wd:{qid} wdt:P800 ?event .
    OPTIONAL {{ ?event wdt:P577 ?date . }}
    OPTIONAL {{ ?event wdt:P291 ?place . }}
    BIND("publication" AS ?evType)
  }}
  OPTIONAL {{ ?event wdt:P625 ?geo . }}
  OPTIONAL {{ ?place wdt:P625 ?pgeo . }}
  SERVICE wikibase:label {{ bd:serviceParam wikibase:language "fr,en". }}
}}
ORDER BY ?date
LIMIT {lim}
"#
    )
}

async fn sparql_json(client: &reqwest::Client, query: &str) -> anyhow::Result<Value> {
    let mut attempt = 0u32;
    loop {
        attempt += 1;
        let resp = client
            .post(WDQS_ENDPOINT)
            .header("Accept", "application/sparql-results+json")
            .header("Content-Type", "application/sparql-query")
            .body(query.to_string())
            .send()
            .await?;
        let status = resp.status();
        if status.as_u16() == 429 && attempt < 4 {
            let wait = resp
                .headers()
                .get("retry-after")
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.parse::<u64>().ok())
                .unwrap_or(65)
                .clamp(15, 90);
            tracing::warn!(attempt, wait, "WDQS 429 — backing off");
            tokio::time::sleep(Duration::from_secs(wait)).await;
            continue;
        }
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!(
                "WDQS HTTP {status}: {}",
                body.chars().take(240).collect::<String>()
            );
        }
        return Ok(resp.json().await?);
    }
}

fn load_json(path: &Path) -> anyhow::Result<Value> {
    let raw = std::fs::read_to_string(path)?;
    Ok(serde_json::from_str(&raw)?)
}

fn binding_str(row: &Value, key: &str) -> Option<String> {
    row.get(key)
        .and_then(|v| v.get("value"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

fn qid_from_uri(uri: &str) -> Option<String> {
    let id = uri.rsplit('/').next().unwrap_or(uri).to_uppercase();
    if id.starts_with('Q') && id[1..].chars().all(|c| c.is_ascii_digit()) {
        Some(id)
    } else {
        None
    }
}

fn event_id_from_uri(uri: &str) -> Option<String> {
    let id = uri.rsplit('/').next().unwrap_or(uri).to_uppercase();
    if !id.starts_with('Q') {
        return None;
    }
    let rest = &id[1..];
    if rest.chars().next()?.is_ascii_digit() {
        Some(id)
    } else {
        None
    }
}

fn normalize_date(raw: &str) -> Option<String> {
    let t = raw.trim().trim_start_matches('+');
    let day = t.split('T').next()?.get(..10)?;
    if day.len() == 10 && day.as_bytes()[4] == b'-' {
        Some(day.to_string())
    } else {
        None
    }
}

fn parse_geo(row: &Value) -> (Option<f64>, Option<f64>) {
    parse_wkt(binding_str(row, "geo").as_deref())
        .or_else(|| parse_wkt(binding_str(row, "pgeo").as_deref()))
        .unwrap_or((None, None))
}

fn parse_wkt(raw: Option<&str>) -> Option<(Option<f64>, Option<f64>)> {
    let s = raw?.trim();
    let inner = s
        .strip_prefix("Point(")
        .or_else(|| s.strip_prefix("POINT("))?
        .strip_suffix(')')?;
    let mut parts = inner.split_whitespace();
    let lon: f64 = parts.next()?.parse().ok()?;
    let lat: f64 = parts.next()?.parse().ok()?;
    Some((Some(lat), Some(lon)))
}

fn classify_event(raw: &str, label: &str) -> String {
    let lower = label.to_lowercase();
    if raw.eq_ignore_ascii_case("battle")
        || lower.contains("battle of")
        || lower.contains("bataille")
        || lower.starts_with("siege of")
        || lower.starts_with("siège")
    {
        return "battle".into();
    }
    if raw.eq_ignore_ascii_case("diplomatic")
        || raw.eq_ignore_ascii_case("treaty")
        || lower.contains("treaty")
        || lower.contains("traité")
        || lower.contains("concordat")
        || lower.contains("peace of")
        || lower.contains("congress of")
        || lower.contains("conférence")
    {
        return "diplomatic".into();
    }
    let t = raw.trim().to_lowercase();
    if t.is_empty() {
        "historical_fact".into()
    } else {
        t
    }
}

fn predicate_for(event_type: &str) -> &'static str {
    match event_type {
        "battle" => "fought_at",
        "diplomatic" => "signed",
        "residence" => "resided_in",
        "birth" => "born_in",
        "death" => "died_in",
        "office" => "held_office",
        "education" => "studied_at",
        "publication" => "published",
        _ => "participant_in",
    }
}
