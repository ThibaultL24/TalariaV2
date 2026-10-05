// crates/talaria-api/src/visit_live.rs
//! Live visit opportunities: Wikidata culture items + optional web search (Serper).

use chrono::{DateTime, NaiveDate, TimeZone, Utc};
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use talaria_sources::wdqs::{fetch_culture_about_person, WdqsCultureItem};
use talaria_store::{
    find_active_person_event_by_occurrence, find_active_person_event_by_title,
    find_nearby_visit_heritage, insert_person_event, upsert_visit_opportunity, PersonEventInsert,
    VisitOpportunityInsert,
};

const VISIT_HERITAGE_DEDUP_RADIUS_M: f64 = 80.0;
use uuid::Uuid;

/// Commemorative canonical event types (heritage map), inferred from Wikidata labels.
pub fn classify_heritage_event_type(label: &str) -> Option<&'static str> {
    let t = label.to_lowercase();
    if t.contains("musée") || t.contains("museum") {
        Some("museum")
    } else if t.contains("statue")
        || t.contains("équestre")
        || t.contains("equestrian")
        || t.contains("monument")
        || t.contains("colonne")
        || t.contains("column")
        || t.contains("buste")
        || t.contains("bust")
    {
        Some("statue")
    } else if t.contains("mémorial")
        || t.contains("memorial")
        || t.contains("tombe")
        || t.contains("tomb")
        || t.contains("sépulture")
    {
        Some("memorial")
    } else if t.contains("rue ") || t.contains("street naming") || t.contains("odonym") {
        Some("street_naming")
    } else {
        None
    }
}

pub fn infer_visit_kind(title: &str, snippet: &str) -> String {
    let text = format!("{title} {snippet}").to_lowercase();
    if text.contains("exhibition") || text.contains("exposition") || text.contains("expo ") {
        "exhibition".into()
    } else if text.contains("festival") {
        "festival".into()
    } else if text.contains("guided tour") || text.contains("visite guidée") {
        "guided_tour".into()
    } else if text.contains("concert")
        || text.contains("conférence")
        || text.contains("conference")
        || text.contains("colloque")
        || text.contains("rassemblement")
        || text.contains("symposium")
    {
        "event".into()
    } else {
        "other".into()
    }
}

pub async fn enrich_from_wikidata(
    pool: &PgPool,
    entity_id: Uuid,
    qid: &str,
    cap: u32,
) -> anyhow::Result<u32> {
    let (_, opportunities) =
        enrich_wikidata_culture_split(pool, entity_id, qid, cap, cap).await?;
    Ok(opportunities)
}

/// Heritage → `canonical_events` (statues, museums…); remainder → `visit_opportunities`.
pub async fn enrich_heritage_from_wikidata(
    pool: &PgPool,
    entity_id: Uuid,
    qid: &str,
    cap: u32,
) -> anyhow::Result<u32> {
    let (heritage, _) = enrich_wikidata_culture_split(pool, entity_id, qid, cap, 0).await?;
    Ok(heritage)
}

pub async fn enrich_wikidata_culture_split(
    pool: &PgPool,
    entity_id: Uuid,
    qid: &str,
    heritage_cap: u32,
    opportunity_cap: u32,
) -> anyhow::Result<(u32, u32)> {
    let items = fetch_culture_about_person(qid).await?;
    let mut heritage = 0u32;
    let mut opportunities = 0u32;
    for item in items {
        if let Some(event_type) = classify_heritage_event_type(&item.label) {
            if heritage >= heritage_cap {
                continue;
            }
            if upsert_wikidata_heritage(pool, entity_id, event_type, &item).await? {
                heritage += 1;
            }
        } else if opportunities < opportunity_cap {
            if upsert_wikidata_culture(pool, entity_id, &item).await? {
                opportunities += 1;
            }
        }
    }
    Ok((heritage, opportunities))
}

async fn upsert_wikidata_heritage(
    pool: &PgPool,
    entity_id: Uuid,
    event_type: &str,
    item: &WdqsCultureItem,
) -> anyhow::Result<bool> {
    let (lat, lon) = match (item.lat, item.lon) {
        (Some(lat), Some(lon)) if lat.is_finite() && lon.is_finite() => (lat, lon),
        _ => return Ok(false),
    };
    let occurrence_key = format!("visit-wikidata:{}", item.item_qid);
    if find_active_person_event_by_occurrence(pool, entity_id, &occurrence_key)
        .await?
        .is_some()
    {
        return Ok(false);
    }
    if find_active_person_event_by_title(pool, entity_id, &item.label)
        .await?
        .is_some()
    {
        return Ok(false);
    }
    if find_nearby_visit_heritage(
        pool,
        entity_id,
        event_type,
        lat,
        lon,
        VISIT_HERITAGE_DEDUP_RADIUS_M,
    )
    .await?
    .is_some()
    {
        return Ok(false);
    }
    let year = item
        .start
        .as_deref()
        .and_then(|s| s.get(0..4))
        .and_then(|y| y.parse::<i32>().ok());
    let surface = year.map(|y| y.to_string());
    let time_json = if let Some(year) = year {
        serde_json::json!({
            "kind": "approx",
            "precision": "year",
            "start": year.to_string(),
            "surface": year.to_string(),
        })
    } else {
        serde_json::json!({"kind": "unknown", "precision": "year"})
    };
    let start_time = year.and_then(|y| {
        NaiveDate::from_ymd_opt(y, 1, 1)
            .and_then(|d| d.and_hms_opt(0, 0, 0))
            .map(|ndt| DateTime::<Utc>::from_naive_utc_and_offset(ndt, Utc))
    });
    let fingerprint = hex::encode(
        Sha256::digest(format!("visit-heritage|{entity_id}|{occurrence_key}").as_bytes()),
    );
    insert_person_event(
        pool,
        &PersonEventInsert {
            entity_id,
            event_type: event_type.into(),
            epistemic_status: "attested".into(),
            title: item.label.clone(),
            summary: None,
            start_time,
            time_json,
            place_label: item.place_label.clone(),
            lat: Some(lat),
            lon: Some(lon),
            confidence: 0.7,
            map_eligible: true,
            fingerprint,
            occurrence_key,
            occurrence_stem: surface,
            predicate: "commemorated_at".into(),
            place_identity_qid: None,
        },
    )
    .await?;
    Ok(true)
}

async fn upsert_wikidata_culture(
    pool: &PgPool,
    entity_id: Uuid,
    item: &WdqsCultureItem,
) -> anyhow::Result<bool> {
    let starts_at = parse_day_start(item.start.as_deref());
    let ends_at = parse_day_end(item.end.as_deref());
    let canonical_url = format!("https://www.wikidata.org/wiki/{}", item.item_qid);
    upsert_visit_opportunity(
        pool,
        &VisitOpportunityInsert {
            entity_id,
            kind: infer_visit_kind(&item.label, ""),
            title: item.label.clone(),
            summary: None,
            venue_label: item.place_label.clone(),
            lat: item.lat,
            lon: item.lon,
            starts_at,
            ends_at,
            canonical_url: Some(canonical_url),
            source_kind: "wikidata".into(),
            source_record_id: item.item_qid.clone(),
        },
    )
    .await?;
    Ok(true)
}

pub async fn enrich_from_serper(
    pool: &PgPool,
    entity_id: Uuid,
    label: &str,
    wiki_lang: &str,
    cap: u32,
) -> anyhow::Result<u32> {
    let key = std::env::var("SERPER_API_KEY")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    let Some(key) = key else {
        tracing::info!("SERPER_API_KEY unset — skipping web visit search");
        return Ok(0);
    };
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()?;
    let hl = if wiki_lang.starts_with("fr") { "fr" } else { "en" };
    let queries = if hl == "fr" {
        [
            format!("{label} exposition 2026"),
            format!("{label} conférence"),
            format!("{label} concert hommage"),
        ]
    } else {
        [
            format!("{label} exhibition 2026"),
            format!("{label} conference"),
            format!("{label} tribute concert"),
        ]
    };
    let mut written = 0u32;
    let mut seen_urls = std::collections::HashSet::new();
    for q in queries {
        if written >= cap {
            break;
        }
        let body = serde_json::json!({
            "q": q,
            "num": 8,
            "gl": if hl == "fr" { "fr" } else { "us" },
            "hl": hl,
        });
        let resp = client
            .post("https://google.serper.dev/search")
            .header("X-API-KEY", &key)
            .json(&body)
            .send()
            .await?;
        if !resp.status().is_success() {
            tracing::warn!(status = %resp.status(), query = %q, "serper search failed");
            continue;
        }
        let payload: serde_json::Value = resp.json().await?;
        let organic = payload
            .get("organic")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        for hit in organic {
            if written >= cap {
                break;
            }
            let title = hit.get("title").and_then(|v| v.as_str()).unwrap_or("").trim();
            let link = hit.get("link").and_then(|v| v.as_str()).unwrap_or("").trim();
            let snippet = hit.get("snippet").and_then(|v| v.as_str()).unwrap_or("");
            if title.is_empty() || link.is_empty() || !seen_urls.insert(link.to_string()) {
                continue;
            }
            if !looks_like_visit_hit(title, snippet) {
                continue;
            }
            let record_id = format!("serper:{}", link);
            upsert_visit_opportunity(
                pool,
                &VisitOpportunityInsert {
                    entity_id,
                    kind: infer_visit_kind(title, snippet),
                    title: title.to_string(),
                    summary: Some(snippet.to_string()),
                    venue_label: None,
                    lat: None,
                    lon: None,
                    starts_at: None,
                    ends_at: None,
                    canonical_url: Some(link.to_string()),
                    source_kind: "serper".into(),
                    source_record_id: record_id,
                },
            )
            .await?;
            written += 1;
        }
    }
    Ok(written)
}

pub async fn enrich_from_openagenda(
    pool: &PgPool,
    entity_id: Uuid,
    label: &str,
    wiki_lang: &str,
    cap: u32,
) -> anyhow::Result<u32> {
    let key = std::env::var("OPENAGENDA_API_KEY")
        .or_else(|_| std::env::var("OPENAGENDA_PUBLIC_KEY"))
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    let Some(key) = key else {
        tracing::info!("OPENAGENDA_API_KEY unset — skipping OpenAgenda visit search");
        return Ok(0);
    };
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()?;
    let now = Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    let size = cap.min(50).max(1);
    let url = format!(
        "https://api.openagenda.com/v2/events?search={}&threshold=auto&size={}&sort=timings.asc&timings%5Bgte%5D={}",
        urlencoding_encode(label),
        size,
        urlencoding_encode(&now),
    );
    let resp = client.get(&url).header("key", &key).send().await?;
    if !resp.status().is_success() {
        tracing::warn!(status = %resp.status(), "openagenda search failed");
        return Ok(0);
    }
    let payload: serde_json::Value = resp.json().await?;
    let events = payload
        .get("events")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let langs = if wiki_lang.starts_with("fr") {
        ["fr", "en"]
    } else {
        ["en", "fr"]
    };
    let mut written = 0u32;
    for event in events {
        if written >= cap {
            break;
        }
        if let Some(row) = parse_openagenda_event(&event, &langs) {
            upsert_visit_opportunity(pool, &row.with_entity(entity_id)).await?;
            written += 1;
        }
    }
    Ok(written)
}

struct OpenAgendaRow {
    uid: String,
    title: String,
    summary: Option<String>,
    venue_label: Option<String>,
    lat: Option<f64>,
    lon: Option<f64>,
    starts_at: Option<DateTime<Utc>>,
    ends_at: Option<DateTime<Utc>>,
    canonical_url: Option<String>,
    kind: String,
}

impl OpenAgendaRow {
    fn with_entity(self, entity_id: Uuid) -> VisitOpportunityInsert {
        VisitOpportunityInsert {
            entity_id,
            kind: self.kind,
            title: self.title,
            summary: self.summary,
            venue_label: self.venue_label,
            lat: self.lat,
            lon: self.lon,
            starts_at: self.starts_at,
            ends_at: self.ends_at,
            canonical_url: self.canonical_url,
            source_kind: "openagenda".into(),
            source_record_id: format!("openagenda:{}", self.uid),
        }
    }
}

fn urlencoding_encode(value: &str) -> String {
    value
        .chars()
        .map(|c| match c {
            'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '.' | '~' => c.to_string(),
            _ => format!("%{:02X}", c as u8),
        })
        .collect()
}

pub fn parse_openagenda_event(event: &serde_json::Value, langs: &[&str]) -> Option<OpenAgendaRow> {
    let uid = event
        .get("uid")
        .and_then(|v| v.as_u64().or_else(|| v.as_str().and_then(|s| s.parse().ok())))
        .map(|n| n.to_string())?;
    let title = localized_field(event.get("title"), langs)
        .or_else(|| localized_field(event.get("title"), &["fr", "en"]))?;
    let summary = localized_field(event.get("description"), langs);
    let slug = event.get("slug").and_then(|v| v.as_str());
    let canonical_url = slug.map(|s| format!("https://openagenda.com/events/{s}"));
    let location = event.get("location").unwrap_or(&serde_json::Value::Null);
    let venue_label = location
        .get("name")
        .and_then(|v| localized_field(Some(v), langs))
        .or_else(|| location.get("city").and_then(|v| v.as_str()).map(str::to_string));
    let lat = location
        .get("latitude")
        .or_else(|| location.pointer("/coordinates/lat"))
        .and_then(|v| v.as_f64());
    let lon = location
        .get("longitude")
        .or_else(|| location.pointer("/coordinates/lng"))
        .and_then(|v| v.as_f64());
    let (starts_at, ends_at) = parse_openagenda_timings(event.get("timings"));
    let kind = infer_visit_kind(&title, summary.as_deref().unwrap_or(""));
    Some(OpenAgendaRow {
        uid,
        title,
        summary,
        venue_label,
        lat,
        lon,
        starts_at,
        ends_at,
        canonical_url,
        kind,
    })
}

fn localized_field(value: Option<&serde_json::Value>, langs: &[&str]) -> Option<String> {
    let Some(value) = value else {
        return None;
    };
    if let Some(s) = value.as_str() {
        let t = s.trim();
        return (!t.is_empty()).then(|| t.to_string());
    }
    let obj = value.as_object()?;
    for lang in langs {
        if let Some(s) = obj.get(*lang).and_then(|v| v.as_str()) {
            let t = s.trim();
            if !t.is_empty() {
                return Some(t.to_string());
            }
        }
    }
    obj.values()
        .find_map(|v| v.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn parse_openagenda_timings(value: Option<&serde_json::Value>) -> (Option<DateTime<Utc>>, Option<DateTime<Utc>>) {
    let Some(arr) = value.and_then(|v| v.as_array()) else {
        return (None, None);
    };
    let Some(first) = arr.first() else {
        return (None, None);
    };
    let start = first
        .get("begin")
        .or_else(|| first.get("start"))
        .and_then(|v| v.as_str())
        .and_then(parse_rfc3339_loose);
    let end = first
        .get("end")
        .and_then(|v| v.as_str())
        .and_then(parse_rfc3339_loose);
    (start, end)
}

fn parse_rfc3339_loose(raw: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(raw)
        .map(|dt| dt.with_timezone(&Utc))
        .ok()
        .or_else(|| {
            chrono::DateTime::parse_from_str(raw, "%Y-%m-%dT%H:%M:%S%.3f%z")
                .map(|dt| dt.with_timezone(&Utc))
                .ok()
        })
}

fn looks_like_visit_hit(title: &str, snippet: &str) -> bool {
    let t = format!("{title} {snippet}").to_lowercase();
    [
        "exhibition",
        "exposition",
        "expo",
        "museum",
        "musée",
        "concert",
        "conférence",
        "conference",
        "festival",
        "colloque",
        "symposium",
        "visite",
        "tour",
        "memorial",
        "hommage",
        "tribute",
        "gathering",
        "rassemblement",
    ]
    .iter()
    .any(|k| t.contains(k))
}

fn parse_day_start(day: Option<&str>) -> Option<DateTime<Utc>> {
    day.and_then(|d| {
        NaiveDate::parse_from_str(d, "%Y-%m-%d")
            .ok()
            .and_then(|nd| nd.and_hms_opt(0, 0, 0))
            .map(|ndt| Utc.from_utc_datetime(&ndt))
    })
}

fn parse_day_end(day: Option<&str>) -> Option<DateTime<Utc>> {
    day.and_then(|d| {
        NaiveDate::parse_from_str(d, "%Y-%m-%d")
            .ok()
            .and_then(|nd| nd.and_hms_opt(23, 59, 59))
            .map(|ndt| Utc.from_utc_datetime(&ndt))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_statue_and_museum_labels() {
        assert_eq!(
            classify_heritage_event_type("Statue équestre de Napoléon Ier"),
            Some("statue")
        );
        assert_eq!(
            classify_heritage_event_type("Musée de l'Armée"),
            Some("museum")
        );
        assert_eq!(classify_heritage_event_type("Random conference 2026"), None);
    }

    #[test]
    fn infers_exhibition_and_concert() {
        assert_eq!(infer_visit_kind("Napoleon exhibition", ""), "exhibition");
        assert_eq!(infer_visit_kind("Concert hommage", ""), "event");
    }

    #[test]
    fn filters_non_visit_snippets() {
        assert!(!looks_like_visit_hit("Biography of Napoleon", "born in 1769"));
        assert!(looks_like_visit_hit("Exposition Napoléon", "au musée"));
    }

    #[test]
    fn parses_openagenda_event_json() {
        let raw = serde_json::json!({
            "uid": 42,
            "slug": "expo-napoleon",
            "title": { "fr": "Exposition Napoléon" },
            "description": { "fr": "Conférences et visites guidées" },
            "location": { "name": { "fr": "Musée" }, "latitude": 48.85, "longitude": 2.35 },
            "timings": [{ "begin": "2026-06-01T10:00:00.000Z", "end": "2026-08-31T18:00:00.000Z" }]
        });
        let row = parse_openagenda_event(&raw, &["fr", "en"]).expect("parse");
        assert_eq!(row.uid, "42");
        assert_eq!(row.title, "Exposition Napoléon");
        assert_eq!(row.lat, Some(48.85));
        assert!(row.starts_at.is_some());
    }
}
