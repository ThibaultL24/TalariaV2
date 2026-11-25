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
    let Some(bindings) = payload
        .pointer("/results/bindings")
        .and_then(|v| v.as_array())
    else {
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
        let mut event_type = classify_event(&ev_type, &label);
        let has_place = place_qid.is_some() || place_label.is_some();
        if event_type == "coronation" && (date.is_none() || !has_place) {
            event_type = "historical_fact".into();
        }
        let (lat, lon) = parse_geo(row);
        let dated_required = !matches!(
            event_type.as_str(),
            "birth" | "death" | "residence" | "office" | "education" | "publication"
        );
        let Some(date) = date.or_else(|| {
            if dated_required && lat.is_none() {
                None
            } else {
                Some(String::new())
            }
        }) else {
            continue;
        };
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
    out.sort_by(|a, b| {
        a.date
            .cmp(&b.date)
            .then_with(|| a.event_qid.cmp(&b.event_qid))
    });
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

/// Cultural happenings documented on Wikidata with this person as main subject (P921).
#[derive(Debug, Clone, PartialEq)]
pub struct WdqsCultureItem {
    pub item_qid: String,
    pub label: String,
    pub start: Option<String>,
    pub end: Option<String>,
    pub place_label: Option<String>,
    pub lat: Option<f64>,
    pub lon: Option<f64>,
}

pub fn culture_about_person_query(qid: &str, limit: u32) -> String {
    let lim = limit.clamp(1, 200);
    format!(
        r#"PREFIX wd: <http://www.wikidata.org/entity/>
PREFIX wdt: <http://www.wikidata.org/prop/direct/>
PREFIX wikibase: <http://wikiba.se/ontology#>
PREFIX bd: <http://www.bigdata.com/rdf#>
SELECT DISTINCT ?item ?itemLabel ?start ?end ?place ?placeLabel ?geo ?pgeo WHERE {{
  {{
    ?item wdt:P921 wd:{qid} .
  }} UNION {{
    ?item wdt:P180 wd:{qid} .
  }}
  OPTIONAL {{ ?item wdt:P580 ?start . }}
  OPTIONAL {{ ?item wdt:P582 ?end . }}
  OPTIONAL {{ ?item wdt:P585 ?start . }}
  OPTIONAL {{ ?item wdt:P276 ?place . }}
  OPTIONAL {{ ?item wdt:P625 ?geo . }}
  OPTIONAL {{ ?place wdt:P625 ?pgeo . }}
  SERVICE wikibase:label {{ bd:serviceParam wikibase:language "fr,en". }}
}}
LIMIT {lim}
"#,
        qid = qid,
        lim = lim
    )
}

pub fn parse_culture_bindings(payload: &Value) -> Vec<WdqsCultureItem> {
    let Some(bindings) = payload
        .pointer("/results/bindings")
        .and_then(|v| v.as_array())
    else {
        return vec![];
    };
    let mut out = Vec::new();
    for row in bindings {
        let Some(item_qid) = binding_str(row, "item").as_deref().and_then(qid_from_uri) else {
            continue;
        };
        let label = binding_str(row, "itemLabel").unwrap_or_else(|| item_qid.clone());
        let start = binding_str(row, "start").and_then(|d| normalize_date(&d));
        let end = binding_str(row, "end").and_then(|d| normalize_date(&d));
        let place_label = binding_str(row, "placeLabel").filter(|s| !s.is_empty());
        let (lat, lon) = parse_geo(row);
        out.push(WdqsCultureItem {
            item_qid,
            label,
            start,
            end,
            place_label,
            lat,
            lon,
        });
    }
    out
}

pub async fn fetch_culture_about_person(qid: &str) -> anyhow::Result<Vec<WdqsCultureItem>> {
    let qid = qid.trim().to_uppercase();
    anyhow::ensure!(
        qid.starts_with('Q') && qid[1..].chars().all(|c| c.is_ascii_digit()),
        "invalid qid {qid}"
    );
    let client = reqwest::Client::builder()
        .user_agent(UA)
        .timeout(Duration::from_secs(90))
        .build()?;
    let payload = sparql_json(&client, &culture_about_person_query(&qid, 120)).await?;
    Ok(parse_culture_bindings(&payload))
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
  }} UNION {{
    wd:{qid} wdt:P793 ?event .
    BIND("historical_fact" AS ?evType)
    OPTIONAL {{ ?event wdt:P585 ?date . }}
    OPTIONAL {{ ?event wdt:P580 ?date . }}
    OPTIONAL {{ ?event wdt:P276 ?place . }}
  }} UNION {{
    wd:{qid} wdt:P119 ?place .
    OPTIONAL {{ wd:{qid} wdt:P570 ?date . }}
    BIND("death" AS ?evType)
    BIND(IRI(CONCAT("http://www.wikidata.org/entity/{qid}-burial")) AS ?event)
  }} UNION {{
    wd:{qid} wdt:P937 ?place .
    BIND("residence" AS ?evType)
    BIND(IRI(CONCAT("http://www.wikidata.org/entity/{qid}-work-", STRAFTER(STR(?place), "entity/"))) AS ?event)
  }} UNION {{
    wd:{qid} wdt:P2632 ?place .
    BIND("residence" AS ?evType)
    BIND(IRI(CONCAT("http://www.wikidata.org/entity/{qid}-det-", STRAFTER(STR(?place), "entity/"))) AS ?event)
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
    let trimmed = raw.trim();
    let (negative, rest) = if let Some(rest) = trimmed.strip_prefix('+') {
        (false, rest)
    } else if let Some(rest) = trimmed.strip_prefix('-') {
        (true, rest)
    } else {
        (false, trimmed)
    };
    let head = rest.split('T').next()?.trim();
    let mut parts = head.split('-');
    let year = parts.next()?;
    if year.len() < 3 || !year.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let year_label = if negative {
        format!("-{year}")
    } else {
        year.to_string()
    };
    let month = parts.next();
    let day = parts.next();
    match (month, day) {
        (Some(month), Some(day))
            if month.len() == 2 && day.len() == 2 && month != "00" && day != "00" =>
        {
            Some(format!("{year_label}-{month}-{day}"))
        }
        (Some(month), _) if month.len() == 2 && month != "00" => {
            Some(format!("{year_label}-{month}"))
        }
        _ => Some(year_label),
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

fn label_type(lower: &str) -> Option<&'static str> {
    if lower.contains("battle of")
        || lower.contains("bataille")
        || lower.starts_with("siege of")
        || lower.starts_with("siège")
    {
        Some("battle")
    } else if lower.contains("treaty")
        || lower.contains("traité")
        || lower.contains("concordat")
        || lower.contains("peace of")
        || lower.contains("congress of")
        || lower.contains("conférence")
    {
        Some("diplomatic")
    } else if lower.contains("coronation") || lower.contains("couronnement") {
        Some("coronation")
    } else if lower.contains("abdication") {
        Some("abdication")
    } else if lower.contains("exile") || lower.contains("exil") {
        Some("exile")
    } else if lower.contains("marriage") || lower.contains("mariage") {
        Some("marriage")
    } else {
        None
    }
}

fn classify_event(raw: &str, label: &str) -> String {
    let lower = label.to_lowercase();
    if raw.eq_ignore_ascii_case("battle") || label_type(&lower) == Some("battle") {
        return "battle".into();
    }
    if raw.eq_ignore_ascii_case("diplomatic")
        || raw.eq_ignore_ascii_case("treaty")
        || label_type(&lower) == Some("diplomatic")
    {
        return "diplomatic".into();
    }
    if raw.eq_ignore_ascii_case("historical_fact") || raw.trim().is_empty() {
        return label_type(&lower).unwrap_or("historical_fact").into();
    }
    raw.trim().to_lowercase()
}

/// QLever has no Wikibase label service. Label joins time out on the public
/// endpoint, so the query keeps the event, the date, and the place.
pub fn events_for_person_query_qlever(qid: &str, limit: u32) -> String {
    let lim = limit.clamp(1, 10_000);
    format!(
        r#"PREFIX wd: <http://www.wikidata.org/entity/>
PREFIX wdt: <http://www.wikidata.org/prop/direct/>
SELECT DISTINCT ?event ?date ?place WHERE {{
  {{
    ?event wdt:P710 wd:{qid} .
  }} UNION {{
    wd:{qid} wdt:P1344 ?event .
  }}
  OPTIONAL {{ ?event wdt:P585 ?date . }}
  OPTIONAL {{ ?event wdt:P276 ?place . }}
}}
LIMIT {lim}
"#
    )
}

const QLEVER_WIKIDATA_ENDPOINT: &str = "https://qlever.dev/api/wikidata";

/// Ask the public QLever endpoint. This does not replace `events_for_person_query`.
pub async fn probe_qlever(qid: &str) -> Result<(usize, usize), String> {
    let query = events_for_person_query_qlever(qid, 5);
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(40))
        .build()
        .map_err(|error| error.to_string())?;
    let response = client
        .post(QLEVER_WIKIDATA_ENDPOINT)
        .header("Accept", "application/sparql-results+json")
        .header("User-Agent", UA)
        .form(&[("query", query.as_str())])
        .send()
        .await
        .map_err(|error| error.to_string())?;
    let status = response.status();
    let body = response.text().await.map_err(|error| error.to_string())?;
    if !status.is_success() {
        return Err(format!(
            "qlever_http_{}: {}",
            status.as_u16(),
            body.chars().take(180).collect::<String>()
        ));
    }
    let payload: Value = serde_json::from_str(&body).map_err(|error| error.to_string())?;
    let raw = payload
        .pointer("/results/bindings")
        .and_then(Value::as_array)
        .map(Vec::len)
        .unwrap_or(0);
    Ok((parse_sparql_bindings(&payload, None).len(), raw))
}

fn predicate_for(event_type: &str) -> &'static str {
    match event_type {
        "battle" => "fought_at",
        "diplomatic" => "signed",
        "residence" => "resided_in",
        "birth" => "born_in",
        "death" => "died_in",
        "office" => "held_office",
        "coronation" => "crowned",
        "abdication" => "abdicated",
        "education" => "studied_at",
        "publication" => "published",
        _ => "participant_in",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn bce_dates_keep_their_sign_and_year_precision() {
        assert_eq!(
            normalize_date("+1805-12-02T00:00:00Z").as_deref(),
            Some("1805-12-02")
        );
        assert_eq!(
            normalize_date("-0044-03-15T00:00:00Z").as_deref(),
            Some("-0044-03-15")
        );
        assert_eq!(
            normalize_date("-0100-00-00T00:00:00Z").as_deref(),
            Some("-0100")
        );
    }

    #[test]
    fn a_coronation_is_not_left_as_a_generic_fact() {
        assert_eq!(
            classify_event("historical_fact", "Coronation of Napoleon"),
            "coronation"
        );
        assert_eq!(classify_event("birth", "Coronation of Napoleon"), "birth");
        assert_eq!(classify_event("battle", "Battle of Austerlitz"), "battle");
    }

    #[test]
    fn a_qlever_response_keeps_the_date_place_and_coordinates() {
        let payload = json!({"results": {"bindings": [{
            "event": {"type": "uri", "value": "http://www.wikidata.org/entity/Q123"},
            "eventLabel": {"type": "literal", "value": "Coronation of Napoleon"},
            "date": {"type": "literal", "value": "+1804-12-02T00:00:00Z"},
            "place": {"type": "uri", "value": "http://www.wikidata.org/entity/Q90"},
            "placeLabel": {"type": "literal", "value": "Paris"},
            "geo": {"type": "literal", "value": "Point(2.312200 48.856600)"},
            "evType": {"type": "literal", "value": "historical_fact"}
        }]}});
        let events = parse_sparql_bindings(&payload, None);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].date, "1804-12-02");
        assert_eq!(events[0].event_type, "coronation");
        assert_eq!(events[0].place_qid.as_deref(), Some("Q90"));
        assert_eq!(events[0].place_label.as_deref(), Some("Paris"));
        assert!((events[0].lat.unwrap() - 48.8566).abs() < 0.0001);
        assert!((events[0].lon.unwrap() - 2.3122).abs() < 0.0001);
        let query = events_for_person_query_qlever("Q1048", 5);
        assert!(query.contains("wdt:P585"));
        assert!(query.contains("wdt:P276"));
        assert!(!query.contains("SERVICE"));
    }

    #[test]
    fn a_coronation_without_a_place_is_not_promoted() {
        let payload = json!({"results": {"bindings": [{
            "event": {"type": "uri", "value": "http://www.wikidata.org/entity/Q123"},
            "eventLabel": {"type": "literal", "value": "Coronation of Napoleon"},
            "date": {"type": "literal", "value": "+1804-12-02T00:00:00Z"},
            "evType": {"type": "literal", "value": "historical_fact"}
        }]}});
        let events = parse_sparql_bindings(&payload, None);
        assert_eq!(events[0].event_type, "historical_fact");
    }

    #[tokio::test]
    #[ignore = "probes qlever.dev and must not replace the Wikidata query"]
    async fn qlever_answers_three_profiles_without_replacing_wdqs() {
        let mut napoleon_dated = 0;
        for qid in ["Q5582", "Q1048", "Q517"] {
            match probe_qlever(qid).await {
                Ok((dated, raw)) => {
                    println!("{qid} dated {dated} of {raw} bindings");
                    if qid == "Q517" {
                        napoleon_dated = dated;
                    }
                }
                Err(error) => println!("{qid} kept on WDQS: {error}"),
            }
        }
        let wdqs = events_for_person_query("Q517", 5);
        assert!(wdqs.contains("SERVICE"));
        assert!(wdqs.contains("wdt:P710"));
        assert!(
            napoleon_dated > 0,
            "QLever returned no dated Napoleon rows, so the WDQS query stays"
        );
    }

    #[test]
    fn qlever_query_does_not_use_the_wikibase_label_service() {
        let query = events_for_person_query_qlever("Q517", 20);
        assert!(!query.contains("SERVICE"));
        assert!(!query.contains("bd:serviceParam"));
        assert!(!query.contains("rdfs:label"));
        assert!(query.contains("wdt:P585"));
        assert!(query.contains("wd:Q517"));
    }

    #[test]
    fn signed_year_survives_binding_parse() {
        let payload = json!({"results": {"bindings": [{
            "event": {"type": "uri", "value": "http://www.wikidata.org/entity/Q1"},
            "eventLabel": {"type": "literal", "value": "Crossing of the Rubicon"},
            "date": {"type": "literal", "value": "-0049-01-10T00:00:00Z"},
            "evType": {"type": "literal", "value": "historical_fact"}
        }]}});
        let events = parse_sparql_bindings(&payload, None);
        assert_eq!(events[0].date, "-0049-01-10");
    }
}
