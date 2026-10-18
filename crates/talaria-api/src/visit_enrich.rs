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
    find_entity_by_qid, find_entity_by_wikipedia_title, get_entity, search_local_entities,
    upsert_entity_from_wikidata, upsert_visit_opportunity, VisitOpportunityInsert,
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
    if options.fixture {
        ensure_fixture_people(pool, options).await?;
    }
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
        written += seed_demo_sites(pool, entity_id, fixture_qid.as_deref(), &visit_dir).await?;
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
        if !options.fixture {
            written += seed_demo_sites(
                pool,
                entity_id,
                fixture_qid.as_deref(),
                heritage_path
                    .parent()
                    .unwrap_or(std::path::Path::new("fixtures/visit")),
            )
            .await?;
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

#[derive(Debug, Deserialize)]
struct SitesDemoFile {
    people: Vec<FixturePerson>,
    sites: Vec<FixtureSite>,
}

#[derive(Debug, Deserialize)]
struct FixturePerson {
    qid: String,
    label: String,
    wiki_title: String,
    wiki_lang: String,
}

#[derive(Debug, Deserialize)]
struct FixtureSite {
    person_qid: String,
    site_qid: String,
    label: String,
    kind: String,
    relation_pid: String,
    lat: f64,
    lon: f64,
}

fn sites_demo_path(visit_dir: &std::path::Path) -> PathBuf {
    let dir = if visit_dir.is_file() {
        visit_dir.parent().unwrap_or(visit_dir)
    } else {
        visit_dir
    };
    let sibling = dir.join("sites_demo.json");
    if sibling.is_file() {
        return sibling;
    }
    crate::cli_helpers::resolve_repo_path(PathBuf::from("fixtures/visit/sites_demo.json"))
}

async fn ensure_fixture_people(pool: &PgPool, options: &VisitEnrichOptions) -> anyhow::Result<()> {
    let path = sites_demo_path(&options.fixture_path);
    if !path.is_file() {
        return Ok(());
    }
    let file: SitesDemoFile = serde_json::from_str(&std::fs::read_to_string(&path)?)?;
    for person in file.people {
        if !fixture_person_selected(&person, options) {
            continue;
        }
        if find_entity_by_qid(pool, &person.qid).await?.is_some() {
            continue;
        }
        upsert_entity_from_wikidata(
            pool,
            &person.qid,
            &person.label,
            &person.wiki_lang,
            &person.wiki_title,
        )
        .await?;
    }
    Ok(())
}

fn fixture_person_selected(person: &FixturePerson, options: &VisitEnrichOptions) -> bool {
    if let Some(qid) = options
        .qid
        .as_deref()
        .map(str::trim)
        .filter(|qid| !qid.is_empty())
    {
        return person.qid == qid;
    }
    let Some(subject) = options
        .subject
        .as_deref()
        .map(str::trim)
        .filter(|subject| !subject.is_empty())
    else {
        return false;
    };
    person.label.eq_ignore_ascii_case(subject) || person.wiki_title.eq_ignore_ascii_case(subject)
}

async fn seed_demo_sites(
    pool: &PgPool,
    entity_id: Uuid,
    person_qid: Option<&str>,
    visit_dir: &std::path::Path,
) -> anyhow::Result<u32> {
    let path = sites_demo_path(visit_dir);
    if !path.is_file() {
        return Ok(0);
    }
    let file: SitesDemoFile = serde_json::from_str(&std::fs::read_to_string(&path)?)?;
    let mut written = 0u32;
    for site in file.sites {
        if !fixture_matches_qid(Some(site.person_qid.as_str()), person_qid) {
            continue;
        }
        if !valid_demo_site(&site) {
            anyhow::bail!(
                "invalid demo site {} for {}",
                site.site_qid,
                site.person_qid
            );
        }
        let site_id: Uuid = sqlx::query_scalar(
            r#"
            INSERT INTO visit_sites(wikidata_qid, label, kind, geom, retrieved_at)
            VALUES ($1, $2, $3, ST_SetSRID(ST_MakePoint($4, $5), 4326)::geography, NOW())
            ON CONFLICT (wikidata_qid) DO UPDATE
              SET label = EXCLUDED.label,
                  kind = EXCLUDED.kind,
                  geom = EXCLUDED.geom,
                  updated_at = NOW(),
                  retrieved_at = NOW()
            RETURNING id
            "#,
        )
        .bind(&site.site_qid)
        .bind(site.label.trim())
        .bind(&site.kind)
        .bind(site.lon)
        .bind(site.lat)
        .fetch_one(pool)
        .await?;
        let source_url = format!("https://www.wikidata.org/wiki/{}", site.site_qid);
        sqlx::query(
            r#"
            INSERT INTO visit_person_sites(entity_id, site_id, relation_pid, source_url, retrieved_at)
            VALUES ($1, $2, $3, $4, NOW())
            ON CONFLICT (entity_id, site_id, relation_pid)
            DO UPDATE SET retrieved_at = NOW()
            "#,
        )
        .bind(entity_id)
        .bind(site_id)
        .bind(&site.relation_pid)
        .bind(&source_url)
        .execute(pool)
        .await?;
        written += 1;
    }
    tracing::info!(entity_id = %entity_id, count = written, path = %path.display(), "visit demo sites");
    Ok(written)
}

fn valid_demo_site(site: &FixtureSite) -> bool {
    const KINDS: &[&str] = &[
        "building",
        "museum",
        "monument",
        "memorial",
        "burial_site",
        "residence",
    ];
    const PIDS: &[&str] = &["P547", "P138", "P119", "P551"];
    valid_qid(&site.person_qid)
        && valid_qid(&site.site_qid)
        && KINDS.contains(&site.kind.as_str())
        && PIDS.contains(&site.relation_pid.as_str())
        && !site.label.trim().is_empty()
        && site.lat.is_finite()
        && site.lon.is_finite()
        && (-90.0..=90.0).contains(&site.lat)
        && (-180.0..=180.0).contains(&site.lon)
}

fn valid_qid(qid: &str) -> bool {
    qid.strip_prefix('Q').is_some_and(|rest| {
        !rest.is_empty() && !rest.starts_with('0') && rest.bytes().all(|byte| byte.is_ascii_digit())
    })
}

fn fixture_matches_qid(row_qid: Option<&str>, entity_qid: Option<&str>) -> bool {
    match row_qid {
        None => true,
        Some(row) => entity_qid.map(|e| e == row).unwrap_or(false),
    }
}

// Legacy fixtures are intentionally excluded from canonical_events.
// Import real heritage through /visit/discover and visit_sites.
async fn seed_heritage_fixture(
    _pool: &PgPool,
    _entity_id: Uuid,
    _row: &FixtureHeritage,
) -> anyhow::Result<bool> {
    Ok(false)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn demo_sites_use_real_wikidata_links() {
        let path = crate::cli_helpers::resolve_repo_path(PathBuf::from(
            "fixtures/visit/sites_demo.json",
        ));
        let file: SitesDemoFile =
            serde_json::from_str(&std::fs::read_to_string(path).expect("sites fixture")).unwrap();
        assert!(file.sites.len() >= 20);
        assert!(file.sites.iter().all(valid_demo_site));
        assert!(file
            .sites
            .iter()
            .any(|site| site.person_qid == "Q517" && site.kind == "monument"));
        assert!(file.people.iter().any(|person| person.qid == "Q517"));
    }
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
