// crates/talaria-api/src/visit_enrich.rs
use std::path::PathBuf;

use chrono::{DateTime, NaiveDate, TimeZone, Utc};
use serde::Deserialize;
use sqlx::PgPool;
use talaria_sources::connectors::net::first_str;
use talaria_sources::{
    normalize_europeana_item, DiscoveryCursor, EuropeanaConfig, EuropeanaConnector,
    ResolvedSubject, SourceConnector,
};
use sha2::{Digest, Sha256};
use talaria_store::{
    find_active_person_event_by_occurrence, find_active_person_event_by_title,
    find_entity_by_qid, find_entity_by_wikipedia_title, find_nearby_visit_heritage, get_entity,
    insert_person_event, search_local_entities, upsert_visit_opportunity, PersonEventInsert,
    VisitOpportunityInsert,
};

const VISIT_HERITAGE_DEDUP_RADIUS_M: f64 = 80.0;
use uuid::Uuid;

#[derive(Debug, Deserialize)]
struct FixtureOpportunity {
    qid: Option<String>,
    kind: String,
    title: String,
    summary: Option<String>,
    venue_label: Option<String>,
    lat: Option<f64>,
    lon: Option<f64>,
    starts_at: Option<String>,
    ends_at: Option<String>,
    canonical_url: Option<String>,
    source_kind: Option<String>,
    source_record_id: String,
}

#[derive(Debug, Deserialize)]
struct FixtureHeritage {
    qid: String,
    event_type: String,
    title: String,
    summary: Option<String>,
    place_label: Option<String>,
    lat: f64,
    lon: f64,
    year: Option<i32>,
    source_record_id: String,
}

pub struct VisitEnrichOptions {
    pub entity_id: Option<Uuid>,
    pub subject: Option<String>,
    pub qid: Option<String>,
    pub wiki_lang: String,
    pub fixture: bool,
    pub live: bool,
    pub web_search: bool,
    pub fixture_path: PathBuf,
    pub europeana_fixture_dir: PathBuf,
    pub max_items: u32,
}

pub async fn run_visit_enrich(pool: &PgPool, options: &VisitEnrichOptions) -> anyhow::Result<u32> {
    let (entity_id, label, entity_qid) = resolve_entity(pool, options).await?;
    let fixture_qid = entity_qid
        .or_else(|| options.qid.clone())
        .map(|q| q.trim().to_string())
        .filter(|q| !q.is_empty());
    let mut written = 0u32;

    if options.fixture {
        let visit_dir = if options.fixture_path.is_file() {
            options
                .fixture_path
                .parent()
                .unwrap_or(&options.fixture_path)
                .to_path_buf()
        } else {
            options.fixture_path.clone()
        };
        let opp_path = if options.fixture_path.is_file() {
            options.fixture_path.clone()
        } else {
            visit_dir.join("opportunities_sample.json")
        };
        if opp_path.is_file() {
            let raw = std::fs::read_to_string(&opp_path).map_err(|e| {
                anyhow::anyhow!("fixture not found at {} ({e})", opp_path.display())
            })?;
            let rows: Vec<FixtureOpportunity> = serde_json::from_str(&raw)?;
            for row in rows {
                if !fixture_matches_qid(row.qid.as_deref(), fixture_qid.as_deref()) {
                    continue;
                }
                upsert_fixture(pool, entity_id, &row).await?;
                written += 1;
            }
        }
        let heritage_path = visit_dir.join("heritage_demo.json");
        if heritage_path.is_file() {
            let raw = std::fs::read_to_string(&heritage_path)?;
            let rows: Vec<FixtureHeritage> = serde_json::from_str(&raw)?;
            for row in rows {
                if !fixture_matches_qid(Some(row.qid.as_str()), fixture_qid.as_deref()) {
                    continue;
                }
                if seed_heritage_fixture(pool, entity_id, &row).await? {
                    written += 1;
                }
            }
            tracing::info!(
                entity_id=%entity_id,
                path=%heritage_path.display(),
                "visit heritage fixture enrich"
            );
        }
        tracing::info!(
            entity_id=%entity_id,
            path=%opp_path.display(),
            count=written,
            "visit fixture enrich"
        );
    }

    if options.live {
        let subject = ResolvedSubject {
            entity_id: Some(entity_id),
            qid: options.qid.clone(),
            label: label.clone(),
            languages: vec![options.wiki_lang.clone()],
            birth_year: None,
            death_year: None,
            countries: vec![],
            occupations: vec![],
            known_identifiers: vec![],
        };
        let api_key = std::env::var("EUROPEANA_API_KEY").ok();
        let connector = if api_key
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .is_some()
        {
            EuropeanaConnector::new(EuropeanaConfig {
                api_key,
                ..EuropeanaConfig::default()
            })
            .map_err(|e| anyhow::anyhow!("{e}"))?
        } else {
            EuropeanaConnector::from_fixture_dir(&options.europeana_fixture_dir)
                .map_err(|e| anyhow::anyhow!("{e}"))?
        };
        let cap = options.max_items.max(1).min(200);
        let mut cursor = Some(DiscoveryCursor {
            offset: 0,
            token: None,
        });
        while written < cap {
            let page = connector
                .discover(&subject, cursor)
                .await
                .map_err(|e| anyhow::anyhow!("{e}"))?;
            if page.documents.is_empty() {
                break;
            }
            for doc in &page.documents {
                if written >= cap {
                    break;
                }
                let fetched = match connector.fetch(doc).await {
                    Ok(f) => f,
                    Err(e) => {
                        tracing::warn!(
                            external_id = %doc.external_id,
                            err = %e,
                            "visit europeana fetch skip"
                        );
                        continue;
                    }
                };
                let detail = fetched
                    .raw_metadata
                    .get("provider")
                    .cloned()
                    .unwrap_or_else(|| fetched.raw_metadata.clone());
                let normalized = match normalize_europeana_item(&detail) {
                    Ok(n) => n,
                    Err(e) => {
                        tracing::warn!(
                            external_id = %doc.external_id,
                            err = %e,
                            "visit europeana parse skip"
                        );
                        continue;
                    }
                };
                let (lat, lon) = europeana_coords(&detail);
                let (starts_at, ends_at) = europeana_window(&detail);
                upsert_visit_opportunity(
                    pool,
                    &VisitOpportunityInsert {
                        entity_id,
                        kind: map_europeana_kind(&detail),
                        title: normalized.title.clone(),
                        summary: normalized.abstract_text.clone(),
                        venue_label: normalized.publisher_or_institution.clone(),
                        lat,
                        lon,
                        starts_at,
                        ends_at,
                        canonical_url: normalized
                            .canonical_url
                            .clone()
                            .or(doc.canonical_url.clone()),
                        source_kind: "europeana".into(),
                        source_record_id: doc.external_id.clone(),
                    },
                )
                .await?;
                written += 1;
            }
            cursor = page.next_cursor;
            if cursor.is_none() {
                break;
            }
        }
        tracing::info!(entity_id=%entity_id, count=written, "visit europeana enrich");
        let wd_qid = fixture_qid.as_deref().or(options.qid.as_deref());
        if let Some(qid) = wd_qid {
            match crate::visit_live::enrich_wikidata_culture_split(
                pool,
                entity_id,
                qid,
                options.max_items.min(120),
                options.max_items.min(40),
            )
            .await
            {
                Ok((heritage_n, opportunity_n)) => {
                    written += heritage_n + opportunity_n;
                    tracing::info!(
                        entity_id = %entity_id,
                        qid = %qid,
                        heritage = heritage_n,
                        opportunities = opportunity_n,
                        "visit wikidata culture enrich"
                    );
                }
                Err(e) => {
                    tracing::warn!(
                        entity_id = %entity_id,
                        qid = %qid,
                        err = %e,
                        "visit wikidata culture enrich skipped"
                    );
                }
            }
        }
        let heritage_path =
            crate::cli_helpers::resolve_repo_path(
                PathBuf::from("fixtures/visit/heritage_demo.json"),
            );
        if heritage_path.is_file() {
            let raw = std::fs::read_to_string(&heritage_path)?;
            let rows: Vec<FixtureHeritage> = serde_json::from_str(&raw)?;
            for row in rows {
                if !fixture_matches_qid(Some(row.qid.as_str()), fixture_qid.as_deref()) {
                    continue;
                }
                if seed_heritage_fixture(pool, entity_id, &row).await? {
                    written += 1;
                }
            }
        }
        if options.web_search {
            let n = crate::visit_live::enrich_from_serper(
                pool,
                entity_id,
                &label,
                &options.wiki_lang,
                options.max_items.min(40),
            )
            .await?;
            written += n;
            tracing::info!(entity_id=%entity_id, count=n, "visit serper web enrich");
        }
        let n = crate::visit_live::enrich_from_openagenda(
            pool,
            entity_id,
            &label,
            &options.wiki_lang,
            options.max_items.min(50),
        )
        .await?;
        written += n;
        tracing::info!(entity_id=%entity_id, count=n, "visit openagenda enrich");
    }

    if !options.fixture && !options.live {
        anyhow::bail!("pass --fixture and/or --live");
    }

    Ok(written)
}

async fn upsert_fixture(
    pool: &PgPool,
    entity_id: Uuid,
    row: &FixtureOpportunity,
) -> anyhow::Result<()> {
    let starts_at = parse_ts(row.starts_at.as_deref())?;
    let ends_at = parse_ts(row.ends_at.as_deref())?;
    upsert_visit_opportunity(
        pool,
        &VisitOpportunityInsert {
            entity_id,
            kind: row.kind.clone(),
            title: row.title.clone(),
            summary: row.summary.clone(),
            venue_label: row.venue_label.clone(),
            lat: row.lat,
            lon: row.lon,
            starts_at,
            ends_at,
            canonical_url: row.canonical_url.clone(),
            source_kind: row
                .source_kind
                .clone()
                .unwrap_or_else(|| "fixture".into()),
            source_record_id: row.source_record_id.clone(),
        },
    )
    .await?;
    Ok(())
}

fn fixture_matches_qid(row_qid: Option<&str>, entity_qid: Option<&str>) -> bool {
    match row_qid {
        None => true,
        Some(row) => entity_qid.map(|e| e == row).unwrap_or(false),
    }
}

async fn seed_heritage_fixture(
    pool: &PgPool,
    entity_id: Uuid,
    row: &FixtureHeritage,
) -> anyhow::Result<bool> {
    let occurrence_key = format!("visit-fixture:{}", row.source_record_id);
    if find_active_person_event_by_occurrence(pool, entity_id, &occurrence_key)
        .await?
        .is_some()
    {
        return Ok(false);
    }
    if find_active_person_event_by_title(pool, entity_id, &row.title)
        .await?
        .is_some()
    {
        return Ok(false);
    }
    if find_nearby_visit_heritage(
        pool,
        entity_id,
        &row.event_type,
        row.lat,
        row.lon,
        VISIT_HERITAGE_DEDUP_RADIUS_M,
    )
    .await?
    .is_some()
    {
        return Ok(false);
    }
    let surface = row.year.map(|y| y.to_string());
    let time_json = if let Some(year) = row.year {
        serde_json::json!({
            "kind": "approx",
            "precision": "year",
            "start": year.to_string(),
            "surface": year.to_string(),
        })
    } else {
        serde_json::json!({"kind": "unknown", "precision": "year"})
    };
    let start_time = row.year.and_then(|y| {
        chrono::NaiveDate::from_ymd_opt(y, 1, 1)
            .and_then(|d| d.and_hms_opt(0, 0, 0))
            .map(|ndt| chrono::DateTime::<Utc>::from_naive_utc_and_offset(ndt, Utc))
    });
    let fingerprint = hex::encode(
        Sha256::digest(format!("visit-heritage|{entity_id}|{occurrence_key}").as_bytes()),
    );
    insert_person_event(
        pool,
        &PersonEventInsert {
            entity_id,
            event_type: row.event_type.clone(),
            epistemic_status: "attested".into(),
            title: row.title.clone(),
            summary: row.summary.clone(),
            start_time,
            time_json,
            place_label: row.place_label.clone(),
            lat: Some(row.lat),
            lon: Some(row.lon),
            confidence: 0.75,
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

pub async fn resolve_entity(
    pool: &PgPool,
    options: &VisitEnrichOptions,
) -> anyhow::Result<(Uuid, String, Option<String>)> {
    if let Some(id) = options.entity_id {
        let entity = get_entity(pool, id).await?;
        let Some(entity) = entity else {
            anyhow::bail!("entity not found: {id}");
        };
        let label = entity
            .canonical_name
            .clone()
            .unwrap_or(entity.wikipedia_title.clone());
        return Ok((id, label, entity.qid));
    }
    let subject = options
        .subject
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| anyhow::anyhow!("--entity or --subject required"))?;
    if let Some(qid) = options.qid.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        if let Some(e) = find_entity_by_qid(pool, qid).await? {
            let label = e
                .canonical_name
                .clone()
                .unwrap_or(e.wikipedia_title.clone());
            return Ok((e.id, label, e.qid));
        }
    }
    if let Some(e) =
        find_entity_by_wikipedia_title(pool, &options.wiki_lang, subject).await?
    {
        let label = e
            .canonical_name
            .clone()
            .unwrap_or(e.wikipedia_title.clone());
        return Ok((e.id, label, e.qid));
    }
    let hits = search_local_entities(pool, subject, 1).await?;
    let Some(e) = hits.into_iter().next() else {
        anyhow::bail!("no entity matching {subject:?}");
    };
    let label = e
        .canonical_name
        .clone()
        .unwrap_or(e.wikipedia_title.clone());
    Ok((e.id, label, e.qid))
}

fn parse_ts(value: Option<&str>) -> anyhow::Result<Option<DateTime<Utc>>> {
    let Some(value) = value.map(str::trim).filter(|s| !s.is_empty()) else {
        return Ok(None);
    };
    Ok(Some(
        DateTime::parse_from_rfc3339(value)
            .map_err(|e| anyhow::anyhow!("invalid timestamp {value}: {e}"))?
            .with_timezone(&Utc),
    ))
}

fn europeana_coords(raw: &serde_json::Value) -> (Option<f64>, Option<f64>) {
    let item = raw.get("object").unwrap_or(raw);
    let lat = first_str(item, &["edmPlaceLatitude", "latitude"])
        .and_then(|s| s.parse().ok());
    let lon = first_str(item, &["edmPlaceLongitude", "longitude"])
        .and_then(|s| s.parse().ok());
    (lat, lon)
}

fn europeana_window(raw: &serde_json::Value) -> (Option<DateTime<Utc>>, Option<DateTime<Utc>>) {
    let item = raw.get("object").unwrap_or(raw);
    let year = first_str(item, &["year", "temporal"]);
    let Some(year) = year.and_then(|y| y.chars().take(4).collect::<String>().parse::<i32>().ok())
    else {
        return (None, None);
    };
    let start = NaiveDate::from_ymd_opt(year, 1, 1)
        .and_then(|d| d.and_hms_opt(0, 0, 0))
        .map(|ndt| Utc.from_utc_datetime(&ndt));
    let end = NaiveDate::from_ymd_opt(year, 12, 31)
        .and_then(|d| d.and_hms_opt(23, 59, 59))
        .map(|ndt| Utc.from_utc_datetime(&ndt));
    (start, end)
}

fn map_europeana_kind(raw: &serde_json::Value) -> String {
    let item = raw.get("object").unwrap_or(raw);
    let ty = first_str(item, &["type", "dcType"]).unwrap_or_default().to_lowercase();
    if ty.contains("event") {
        "event".into()
    } else if ty.contains("festival") {
        "festival".into()
    } else {
        "exhibition".into()
    }
}
