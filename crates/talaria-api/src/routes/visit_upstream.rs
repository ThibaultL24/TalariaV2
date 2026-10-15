// crates/talaria-api/src/routes/visit_upstream.rs
//! Live heritage lookup. Failures stay errors; they never become invented sites.

use serde_json::{json, Value};
use sqlx::PgPool;
use talaria_store::{
    catalog_heritage, merimee_hits_for_entity, CatalogReport, ClaimDirection, CommonsCandidate, PlaceClaim,
    PlaceProfile,
};

const WIKIDATA_API: &str = "https://www.wikidata.org/w/api.php";
const COMMONS_API: &str = "https://commons.wikimedia.org/w/api.php";
const SPARQL: &str = "https://query.wikidata.org/sparql";
const USER_AGENT: &str = "TalariaEngine/0.1 (visit heritage catalog)";

pub async fn fetch_catalog(
    pool: &PgPool,
    person_qid: &str,
    person_label: &str,
) -> anyhow::Result<(CatalogReport, Vec<String>)> {
    let http = reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .timeout(std::time::Duration::from_secs(25))
        .build()?;
    let mut errors = Vec::new();
    let person = match get_entities(&http, &[person_qid.to_string()]).await {
        Ok(entities) => entities,
        Err(err) => return Err(err),
    };
    let Some(person_entity) = person.get(person_qid) else {
        anyhow::bail!("wikidata_entity_missing");
    };
    let mut claims = outgoing_claims(person_entity);
    let mut place_ids: Vec<String> = claims.iter().map(|claim| claim.target_qid.clone()).collect();

    match incoming_sites(&http, person_qid).await {
        Ok(incoming) => {
            for (qid, pid) in incoming {
                claims.push(PlaceClaim { pid, target_qid: qid.clone(), direction: ClaimDirection::Incoming });
                place_ids.push(qid);
            }
        }
        Err(err) => errors.push(format!("wikidata_sparql:{err}")),
    }
    place_ids.sort();
    place_ids.dedup();
    place_ids.truncate(40);

    let places = match get_entities(&http, &place_ids).await {
        Ok(entities) => entities,
        Err(err) => {
            errors.push(format!("wikidata_places:{err}"));
            serde_json::Map::new()
        }
    };
    let profiles: Vec<PlaceProfile> = places
        .iter()
        .filter_map(|(qid, entity)| profile_from_entity(qid, entity))
        .collect();
    let merimee = match merimee_hits_for_entity(pool, person_label, Some(person_qid)).await {
        Ok(hits) => hits,
        Err(err) => {
            errors.push(format!("merimee_local:{err}"));
            Vec::new()
        }
    };
    let seeded = catalog_heritage(person_qid, person_label, &profiles, &claims, &merimee, &[]);
    let mut media = Vec::new();
    for site in seeded.accepted.iter().take(8) {
        let Some(qid) = &site.place_qid else { continue };
        let Some(profile) = profiles.iter().find(|profile| &profile.qid == qid) else { continue };
        let Some(file) = &profile.image_file else { continue };
        match commons_file(&http, file, qid, person_qid).await {
            Ok(Some(candidate)) => media.push(candidate),
            Ok(None) => {}
            Err(err) => {
                errors.push(format!("commons:{qid}:{err}"));
                break;
            }
        }
    }
    Ok((catalog_heritage(person_qid, person_label, &profiles, &claims, &merimee, &media), errors))
}

fn outgoing_claims(entity: &Value) -> Vec<PlaceClaim> {
    let mut claims = Vec::new();
    let Some(object) = entity.get("claims").and_then(Value::as_object) else {
        return claims;
    };
    for pid in ["P19", "P20", "P119", "P551", "P937", "P69"] {
        let Some(statements) = object.get(pid).and_then(Value::as_array) else { continue };
        for statement in statements {
            if statement.get("rank").and_then(Value::as_str) == Some("deprecated") {
                continue;
            }
            let Some(target) = statement
                .pointer("/mainsnak/datavalue/value/id")
                .and_then(Value::as_str)
            else {
                continue;
            };
            claims.push(PlaceClaim {
                pid: pid.into(),
                target_qid: target.into(),
                direction: ClaimDirection::Outgoing,
            });
        }
    }
    claims
}

fn profile_from_entity(qid: &str, entity: &Value) -> Option<PlaceProfile> {
    let label = entity
        .pointer("/labels/fr/value")
        .or_else(|| entity.pointer("/labels/en/value"))
        .and_then(Value::as_str)
        .unwrap_or(qid)
        .to_string();
    let instance_of = qid_claims(entity, "P31");
    let (lat, lon) = coordinate(entity);
    Some(PlaceProfile {
        qid: qid.into(),
        label,
        instance_of,
        lat,
        lon,
        image_file: entity
            .pointer("/claims/P18/0/mainsnak/datavalue/value")
            .and_then(Value::as_str)
            .map(str::to_string),
        revision_id: entity.get("lastrevid").and_then(Value::as_i64).map(|value| value.to_string()),
    })
}

fn qid_claims(entity: &Value, pid: &str) -> Vec<String> {
    entity
        .pointer(&format!("/claims/{pid}"))
        .and_then(Value::as_array)
        .map(|statements| {
            statements
                .iter()
                .filter_map(|statement| statement.pointer("/mainsnak/datavalue/value/id").and_then(Value::as_str).map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

fn coordinate(entity: &Value) -> (Option<f64>, Option<f64>) {
    let value = entity.pointer("/claims/P625/0/mainsnak/datavalue/value");
    (
        value.and_then(|item| item.get("latitude")).and_then(Value::as_f64),
        value.and_then(|item| item.get("longitude")).and_then(Value::as_f64),
    )
}

async fn get_entities(http: &reqwest::Client, ids: &[String]) -> anyhow::Result<serde_json::Map<String, Value>> {
    if ids.is_empty() {
        return Ok(serde_json::Map::new());
    }
    let response = http
        .get(WIKIDATA_API)
        .query(&[
            ("action", "wbgetentities"),
            ("ids", &ids.join("|")),
            ("props", "claims|labels|info"),
            ("languages", "fr|en"),
            ("format", "json"),
        ])
        .send()
        .await?
        .error_for_status()?;
    let body: Value = response.json().await?;
    Ok(body
        .get("entities")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default())
}

async fn incoming_sites(http: &reqwest::Client, person_qid: &str) -> anyhow::Result<Vec<(String, String)>> {
    let query = format!(
        "SELECT ?site ?pid WHERE {{ VALUES ?pid {{ wdt:P547 wdt:P138 wdt:P825 }} ?site ?pid wd:{person_qid} . }} LIMIT 25"
    );
    let response = http
        .get(SPARQL)
        .header("Accept", "application/sparql-results+json")
        .query(&[("query", query.as_str())])
        .send()
        .await?
        .error_for_status()?;
    let body: Value = response.json().await?;
    let mut found = Vec::new();
    let Some(bindings) = body.pointer("/results/bindings").and_then(Value::as_array) else {
        return Ok(found);
    };
    for binding in bindings {
        let Some(site) = binding.pointer("/site/value").and_then(Value::as_str) else { continue };
        let Some(qid) = site.rsplit('/').next() else { continue };
        let pid = binding
            .pointer("/pid/value")
            .and_then(Value::as_str)
            .and_then(|value| value.rsplit('/').next())
            .unwrap_or("P547");
        found.push((qid.to_string(), pid.to_string()));
    }
    Ok(found)
}

async fn commons_file(
    http: &reqwest::Client,
    file: &str,
    site_qid: &str,
    person_qid: &str,
) -> anyhow::Result<Option<CommonsCandidate>> {
    let title = if file.starts_with("File:") { file.to_string() } else { format!("File:{file}") };
    let response = http
        .get(COMMONS_API)
        .query(&[
            ("action", "query"),
            ("titles", title.as_str()),
            ("prop", "imageinfo"),
            ("iiprop", "url|extmetadata|mime"),
            ("iiurlwidth", "960"),
            ("format", "json"),
        ])
        .send()
        .await?
        .error_for_status()?;
    let body: Value = response.json().await?;
    let page = body
        .pointer("/query/pages")
        .and_then(Value::as_object)
        .and_then(|pages| pages.values().next())
        .cloned()
        .unwrap_or(json!({}));
    let info = page.pointer("/imageinfo/0").cloned().unwrap_or(Value::Null);
    let author = ext(&info, "Artist").or_else(|| ext(&info, "Credit"));
    let license = ext(&info, "LicenseShortName");
    let thumb = info.get("thumburl").and_then(Value::as_str).map(str::to_string);
    Ok(Some(CommonsCandidate {
        commons_file: title,
        depicts_qids: vec![],
        author,
        license,
        thumb_url: thumb,
        site_qid: site_qid.into(),
        person_qid: person_qid.into(),
        is_site_p18: true,
    }))
}

fn ext(info: &Value, key: &str) -> Option<String> {
    let raw = info.pointer(&format!("/extmetadata/{key}/value")).and_then(Value::as_str)?;
    let cleaned = raw.replace('<', " ").replace('>', " ");
    let text = cleaned.split_whitespace().collect::<Vec<_>>().join(" ");
    if text.is_empty() { None } else { Some(text) }
}

#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn live_catalog_counts_are_observed() {
        let pool = match std::env::var("DATABASE_URL") {
            Ok(url) => sqlx::PgPool::connect(&url).await.expect("database"),
            Err(_) => {
                println!("skip live_catalog_counts_are_observed: DATABASE_URL unset");
                return;
            }
        };
        for (qid, label) in [("Q535", "Victor Hugo"), ("Q517", "Napoleon"), ("Q762", "Leonardo da Vinci")] {
            match super::fetch_catalog(&pool, qid, label).await {
                Ok((report, errors)) => {
                    let geolocated = report.accepted.iter().filter(|site| site.lat.is_some() && site.lon.is_some()).count();
                    let photos = report.accepted.iter().filter(|site| site.media.is_some()).count();
                    let links = report.accepted.iter().map(|site| site.relations.len()).sum::<usize>();
                    println!(
                        "{label} {qid} candidates={} accepted={} geolocated={} photos={} links={} duplicates={} rejected={} upstream_errors={}",
                        report.candidates,
                        report.accepted.len(),
                        geolocated,
                        photos,
                        links,
                        report.duplicates,
                        report.rejected.len(),
                        errors.len(),
                    );
                    println!("  sites: {}", report.accepted.iter().map(|site| site.label.as_str()).collect::<Vec<_>>().join(" | "));
                    if let Some(error) = errors.first() {
                        println!("  first_upstream: {error}");
                    }
                }
                Err(err) => println!("{label} {qid} FETCH_FAILED {err}"),
            }
        }
    }
}
