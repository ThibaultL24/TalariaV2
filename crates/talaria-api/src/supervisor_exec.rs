// crates/talaria-api/src/supervisor_exec.rs
//! Typed documentary execution. The model names an allowed task. This module calls one existing connector.

use chrono::Utc;
use serde_json::Value;
use sqlx::PgPool;
use talaria_ai::TaskKind;
use talaria_sources::{
    connectors::{
        parse_p18_filenames, CommonsConnector, WikidataSourceConnector, WikidataSourceConnectorConfig,
        WikipediaConnector, WikipediaConnectorConfig, WikisourceConnector,
    },
    ResolvedSubject, SourceConnector,
};
use talaria_store::{upsert_supervised_document, SourcedSite, TaskEffect};
use uuid::Uuid;

const DOCUMENT_CAP: usize = 4;
const TEXT_CAP: usize = 16_000;

#[derive(Debug, Default)]
pub struct HarvestReport {
    pub effects: Vec<TaskEffect>,
    pub documents: u32,
    pub new_documents: u32,
    pub sources: Vec<String>,
    pub sourced_site: Option<SourcedSite>,
}

pub async fn execute_tasks(
    pool: &PgPool,
    experience: &str,
    qid: &str,
    label: &str,
    kinds: &[TaskKind],
) -> HarvestReport {
    let mut report = HarvestReport::default();
    let mut entity: Option<Value> = None;
    let mut wikidata_document: Option<Uuid> = None;
    for kind in kinds {
        if report.documents >= DOCUMENT_CAP as u32 {
            report.effects.push(refused(kind, qid, "document_budget_exhausted"));
            continue;
        }
        if !kind.allowed_for(parse_experience(experience)) {
            report.effects.push(refused(kind, qid, "wrong_experience"));
            continue;
        }
        let effect = match kind {
            TaskKind::ResolveWikidata | TaskKind::HarvestWdqsBiography => {
                fetch_wikidata(pool, qid, label, kind, &mut entity, &mut wikidata_document, &mut report).await
            }
            TaskKind::HarvestWikipedia => fetch_wikipedia(pool, qid, label, &mut report).await,
            TaskKind::HarvestWikisource => fetch_wikisource(pool, qid, label, entity.as_ref(), &mut report).await,
            TaskKind::HarvestCommons | TaskKind::CollectCommonsMedia => {
                fetch_commons(pool, qid, label, entity.as_ref(), kind, &mut report).await
            }
            TaskKind::DiscoverHeritageSites => {
                quarantine_heritage(pool, qid, label, &mut entity, &mut wikidata_document, &mut report).await
            }
            TaskKind::ReviewPersonSiteRelation | TaskKind::FlagUnverifiedVisitFacts | TaskKind::CollectCulturalOccurrences | TaskKind::RecordDocumentaryGap => {
                completed(kind, qid, false, serde_json::json!({"reason": "recorded_without_publication"}))
            }
        };
        report.effects.push(effect);
    }
    report
}

pub fn documented_places(entity: &Value) -> Vec<DocumentedPlace> {
    let mut places = Vec::new();
    let Some(claims) = entity.get("claims").and_then(Value::as_object) else {
        return places;
    };
    for (pid, role, kind) in [
        ("P119", "burial", "burial_site"),
        ("P138", "named_after", "memorial"),
        ("P547", "commemoration", "memorial"),
        ("P551", "residence", "residence"),
    ] {
        let Some(statements) = claims.get(pid).and_then(Value::as_array) else {
            continue;
        };
        for statement in statements {
            if statement.get("rank").and_then(Value::as_str) == Some("deprecated") {
                continue;
            }
            let statement_id = statement.get("id").and_then(Value::as_str).unwrap_or("");
            let target = statement
                .pointer("/mainsnak/datavalue/value/id")
                .and_then(Value::as_str)
                .unwrap_or("");
            if statement_id.is_empty() || !target.starts_with('Q') {
                continue;
            }
            places.push(DocumentedPlace {
                statement_id: statement_id.to_string(),
                target_qid: target.to_string(),
                relation_pid: pid.to_string(),
                relation_role: role.to_string(),
                kind: kind.to_string(),
            });
        }
    }
    places
}

pub fn coordinates_of(entity: &Value) -> Option<(f64, f64)> {
    let value = entity.pointer("/claims/P625/0/mainsnak/datavalue/value")?;
    let lat = value.get("latitude")?.as_f64()?;
    let lon = value.get("longitude")?.as_f64()?;
    (lat.is_finite() && lon.is_finite()).then_some((lat, lon))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentedPlace {
    pub statement_id: String,
    pub target_qid: String,
    pub relation_pid: String,
    pub relation_role: String,
    pub kind: String,
}

async fn fetch_wikidata(
    pool: &PgPool,
    qid: &str,
    label: &str,
    kind: &TaskKind,
    entity: &mut Option<Value>,
    document_id: &mut Option<Uuid>,
    report: &mut HarvestReport,
) -> TaskEffect {
    match load_wikidata(pool, qid, label, entity, document_id, report).await {
        Ok(inserted) => completed(kind, qid, inserted, serde_json::json!({"source": "wikidata"})),
        Err(error) => failed(kind, qid, &error.to_string()),
    }
}

async fn load_wikidata(
    pool: &PgPool,
    qid: &str,
    label: &str,
    entity: &mut Option<Value>,
    document_id: &mut Option<Uuid>,
    report: &mut HarvestReport,
) -> anyhow::Result<bool> {
    if entity.is_some() {
        return Ok(false);
    }
    let connector = WikidataSourceConnector::new(WikidataSourceConnectorConfig::default())?;
    let page = connector.discover(&subject(qid, label, vec![]), None).await?;
    let Some(document) = page.documents.first() else {
        anyhow::bail!("wikidata_document_missing");
    };
    let fetched = connector.fetch(document).await?;
    let stored = store_fetch(pool, "wikidata", qid, &fetched).await?;
    *entity = Some(fetched.raw_metadata);
    *document_id = Some(stored.0);
    remember(report, "wikidata", stored.1);
    Ok(stored.1)
}

async fn fetch_wikipedia(pool: &PgPool, qid: &str, label: &str, report: &mut HarvestReport) -> TaskEffect {
    let connector = match WikipediaConnector::new(WikipediaConnectorConfig {
        languages: vec!["en".into()],
        max_linked_pages: 0,
    }) {
        Ok(connector) => connector,
        Err(error) => return failed(&TaskKind::HarvestWikipedia, qid, &error.to_string()),
    };
    let page = match connector.discover(&subject(qid, label, vec![]), None).await {
        Ok(page) => page,
        Err(error) => return failed(&TaskKind::HarvestWikipedia, qid, &error.to_string()),
    };
    let Some(document) = page.documents.first() else {
        return completed(&TaskKind::HarvestWikipedia, qid, false, serde_json::json!({"reason": "no_page"}));
    };
    match connector.fetch(document).await {
        Ok(fetched) => match store_fetch(pool, "wikipedia", qid, &fetched).await {
            Ok(stored) => {
                remember(report, "wikipedia", stored.1);
                completed(&TaskKind::HarvestWikipedia, qid, stored.1, serde_json::json!({"source": "wikipedia", "title": document.title}))
            }
            Err(error) => failed(&TaskKind::HarvestWikipedia, qid, &error.to_string()),
        },
        Err(error) => failed(&TaskKind::HarvestWikipedia, qid, &error.to_string()),
    }
}

async fn fetch_wikisource(
    pool: &PgPool,
    qid: &str,
    label: &str,
    entity: Option<&Value>,
    report: &mut HarvestReport,
) -> TaskEffect {
    let mut connector = match WikisourceConnector::new() {
        Ok(connector) => connector,
        Err(error) => return failed(&TaskKind::HarvestWikisource, qid, &error.to_string()),
    };
    connector.max_docs = 1;
    let identifiers = sitelink(entity, "frwikisource");
    let page = match connector
        .discover(&subject(qid, label, identifiers), None)
        .await
    {
        Ok(page) => page,
        Err(error) => return failed(&TaskKind::HarvestWikisource, qid, &error.to_string()),
    };
    let Some(document) = page.documents.first() else {
        return completed(&TaskKind::HarvestWikisource, qid, false, serde_json::json!({"reason": "no_wikisource_page"}));
    };
    match connector.fetch(document).await {
        Ok(fetched) => match store_fetch(pool, "wikisource", qid, &fetched).await {
            Ok(stored) => {
                remember(report, "wikisource", stored.1);
                completed(&TaskKind::HarvestWikisource, qid, stored.1, serde_json::json!({"source": "wikisource", "title": document.title}))
            }
            Err(error) => failed(&TaskKind::HarvestWikisource, qid, &error.to_string()),
        },
        Err(error) => failed(&TaskKind::HarvestWikisource, qid, &error.to_string()),
    }
}

async fn fetch_commons(
    pool: &PgPool,
    qid: &str,
    _label: &str,
    entity: Option<&Value>,
    kind: &TaskKind,
    report: &mut HarvestReport,
) -> TaskEffect {
    let Some(entity) = entity else {
        return completed(kind, qid, false, serde_json::json!({"reason": "wikidata_entity_required"}));
    };
    let Some(file) = parse_p18_filenames(entity).into_iter().next() else {
        return completed(kind, qid, false, serde_json::json!({"reason": "no_commons_file"}));
    };
    let connector = match CommonsConnector::new() {
        Ok(connector) => connector,
        Err(error) => return failed(kind, qid, &error.to_string()),
    };
    let document = CommonsConnector::document_from_p18(&file);
    match connector.fetch(&document).await {
        Ok(fetched) => match store_fetch(pool, "commons", qid, &fetched).await {
            Ok(stored) => {
                remember(report, "commons", stored.1);
                completed(kind, qid, stored.1, serde_json::json!({"source": "commons", "file": file}))
            }
            Err(error) => failed(kind, qid, &error.to_string()),
        },
        Err(error) => failed(kind, qid, &error.to_string()),
    }
}

async fn quarantine_heritage(
    pool: &PgPool,
    qid: &str,
    label: &str,
    entity: &mut Option<Value>,
    document_id: &mut Option<Uuid>,
    report: &mut HarvestReport,
) -> TaskEffect {
    let inserted = match load_wikidata(pool, qid, label, entity, document_id, report).await {
        Ok(inserted) => inserted,
        Err(error) => return failed(&TaskKind::DiscoverHeritageSites, qid, &error.to_string()),
    };
    let Some(person) = entity.as_ref() else {
        return refused(&TaskKind::DiscoverHeritageSites, qid, "wikidata_entity_required");
    };
    let Some(place) = documented_places(person).into_iter().next() else {
        return completed(&TaskKind::DiscoverHeritageSites, qid, inserted, serde_json::json!({"reason": "no_documented_place"}));
    };
    let connector = match WikidataSourceConnector::new(WikidataSourceConnectorConfig::default()) {
        Ok(connector) => connector,
        Err(error) => return failed(&TaskKind::DiscoverHeritageSites, qid, &error.to_string()),
    };
    let site_subject = subject(&place.target_qid, &place.target_qid, vec![]);
    let page = match connector.discover(&site_subject, None).await {
        Ok(page) => page,
        Err(error) => return failed(&TaskKind::DiscoverHeritageSites, qid, &error.to_string()),
    };
    let Some(document) = page.documents.first() else {
        return refused(&TaskKind::DiscoverHeritageSites, qid, "site_entity_missing");
    };
    let fetched = match connector.fetch(document).await {
        Ok(fetched) => fetched,
        Err(error) => return failed(&TaskKind::DiscoverHeritageSites, qid, &error.to_string()),
    };
    let Some((lat, lon)) = coordinates_of(&fetched.raw_metadata) else {
        return completed(&TaskKind::DiscoverHeritageSites, qid, inserted, serde_json::json!({"reason": "coordinates_missing", "site_qid": place.target_qid}));
    };
    let Some(document_id) = *document_id else {
        return refused(&TaskKind::DiscoverHeritageSites, qid, "source_document_required");
    };
    let site_label = fetched
        .raw_metadata
        .pointer("/labels/en/value")
        .or_else(|| fetched.raw_metadata.pointer("/labels/fr/value"))
        .and_then(Value::as_str)
        .unwrap_or(&place.target_qid)
        .to_string();
    let revision = fetched.revision_id.as_deref().and_then(|value| value.parse().ok());
    report.sourced_site = Some(SourcedSite {
        qid: place.target_qid,
        label: site_label,
        kind: place.kind,
        lat,
        lon,
        source_url: format!("https://www.wikidata.org/wiki/{qid}"),
        retrieved_at: Utc::now(),
        revision_id: revision,
        relation_role: place.relation_role,
        relation_pid: place.relation_pid,
        statement_id: place.statement_id,
        document_id,
    });
    completed(&TaskKind::DiscoverHeritageSites, qid, false, serde_json::json!({"source": "wikidata", "publication": "quarantine"}))
}

async fn store_fetch(
    pool: &PgPool,
    source_type: &str,
    qid: &str,
    fetched: &talaria_sources::FetchedDocument,
) -> anyhow::Result<(Uuid, bool)> {
    let url = fetched
        .discovered
        .canonical_url
        .clone()
        .filter(|url| talaria_store::verifiable_source_url(url))
        .ok_or_else(|| anyhow::anyhow!("unverifiable_source_url"))?;
    let mut text = fetched.text.clone();
    if text.chars().count() > TEXT_CAP {
        text = text.chars().take(TEXT_CAP).collect();
    }
    upsert_supervised_document(
        pool,
        source_type,
        &url,
        &fetched.discovered.title,
        fetched.discovered.language.as_deref().unwrap_or("en"),
        qid,
        fetched.revision_id.as_deref(),
        fetched.license.as_deref(),
        &text,
    )
    .await
}

fn remember(report: &mut HarvestReport, source: &str, inserted: bool) {
    report.documents += 1;
    if inserted {
        report.new_documents += 1;
    }
    if !report.sources.iter().any(|item| item == source) {
        report.sources.push(source.to_string());
    }
}

fn subject(qid: &str, label: &str, known_identifiers: Vec<(String, String)>) -> ResolvedSubject {
    ResolvedSubject {
        entity_id: None,
        qid: Some(qid.to_string()),
        label: label.to_string(),
        languages: vec!["en".into(), "fr".into()],
        birth_year: None,
        death_year: None,
        countries: vec![],
        occupations: vec![],
        known_identifiers,
    }
}

fn sitelink(entity: Option<&Value>, wiki: &str) -> Vec<(String, String)> {
    entity
        .and_then(|value| value.pointer(&format!("/sitelinks/{wiki}/title")))
        .and_then(Value::as_str)
        .map(|title| vec![(wiki.to_string(), title.to_string())])
        .unwrap_or_default()
}

fn parse_experience(value: &str) -> talaria_ai::Experience {
    talaria_ai::Experience::parse(value).unwrap_or(talaria_ai::Experience::Istoria)
}

fn completed(kind: &TaskKind, qid: &str, writes: bool, detail: Value) -> TaskEffect {
    TaskEffect {
        key: format!("{}:{qid}", kind.as_str()),
        kind: kind.as_str().into(),
        status: "completed",
        writes,
        detail,
    }
}

fn refused(kind: &TaskKind, qid: &str, reason: &str) -> TaskEffect {
    TaskEffect {
        key: format!("{}:{qid}", kind.as_str()),
        kind: kind.as_str().into(),
        status: "refused",
        writes: false,
        detail: serde_json::json!({"reason": reason}),
    }
}

fn failed(kind: &TaskKind, qid: &str, error: &str) -> TaskEffect {
    TaskEffect {
        key: format!("{}:{qid}", kind.as_str()),
        kind: kind.as_str().into(),
        status: "failed",
        writes: false,
        detail: serde_json::json!({"error": error}),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_place_without_a_statement_id_is_not_a_relation() {
        let entity = json!({"claims": {"P119": [{"rank": "normal", "mainsnak": {"datavalue": {"value": {"id": "Q90"}}}}]}});
        assert!(documented_places(&entity).is_empty());
    }

    #[test]
    fn a_burial_statement_keeps_its_target_and_role() {
        let entity = json!({"claims": {"P119": [{
            "id": "Q5582$burial",
            "rank": "normal",
            "mainsnak": {"datavalue": {"value": {"id": "Q170481"}}}
        }]}});
        let places = documented_places(&entity);
        assert_eq!(places[0].target_qid, "Q170481");
        assert_eq!(places[0].relation_role, "burial");
        assert_eq!(places[0].kind, "burial_site");
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn istoria_and_poreia_fetch_real_documents_without_publishing(pool: sqlx::PgPool) {
        let before = sqlx::query_scalar::<_, i64>("SELECT count(*)::bigint FROM canonical_events")
            .fetch_one(&pool)
            .await
            .unwrap();
        let istoria = execute_tasks(
            &pool,
            "istoria",
            "Q517",
            "Napoleon",
            &[TaskKind::ResolveWikidata, TaskKind::HarvestWikipedia, TaskKind::HarvestWikisource],
        )
        .await;
        assert!(istoria.documents >= 1, "{istoria:?}");
        assert!(istoria.effects.iter().all(|effect| effect.status != "failed"), "{istoria:?}");
        let again = execute_tasks(
            &pool,
            "istoria",
            "Q517",
            "Napoleon",
            &[TaskKind::ResolveWikidata, TaskKind::HarvestWikipedia],
        )
        .await;
        assert_eq!(again.new_documents, 0);
        let poreia = execute_tasks(
            &pool,
            "poreia",
            "Q5582",
            "Vincent van Gogh",
            &[TaskKind::DiscoverHeritageSites, TaskKind::CollectCommonsMedia],
        )
        .await;
        assert!(poreia.documents >= 1, "{poreia:?}");
        assert!(poreia.effects.iter().all(|effect| effect.status != "failed"), "{poreia:?}");
        let caesar = execute_tasks(
            &pool,
            "istoria",
            "Q1048",
            "Julius Caesar",
            &[TaskKind::ResolveWikidata],
        )
        .await;
        assert!(caesar.documents >= 1, "{caesar:?}");
        let text = sqlx::query_scalar::<_, String>(
            "SELECT payload->>'text' FROM raw_documents WHERE source_identifier = 'Q1048'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert!(text.contains('-'), "a BCE statement must keep its sign: {text}");
        if let Some(site) = poreia.sourced_site {
            let report = talaria_store::record_supervisor_run(
                &pool,
                &talaria_store::SupervisorCall {
                    experience: "poreia".into(),
                    subject_qid: "Q5582".into(),
                    subject_label: "Vincent van Gogh".into(),
                    provider: "deterministic".into(),
                    model: "none".into(),
                    prompt_version: "supervisor:v2".into(),
                    plan_json: serde_json::json!({"tasks": [{"key": "discover_heritage_sites:Q5582"}]}),
                    execute: true,
                    input_tokens: 0,
                    output_tokens: 0,
                    cost_usd_micros: 0,
                    cost_estimated: false,
                    duration_ms: 1,
                    effects: poreia.effects.iter().filter(|effect| effect.kind == "discover_heritage_sites").cloned().collect(),
                    sourced_site: Some(site),
                },
            )
            .await
            .unwrap();
            assert_eq!(report["published_sites"], 0);
            assert!(report["tasks"][0]["writes"] == true);
        }
        let canonical = sqlx::query_scalar::<_, i64>("SELECT count(*)::bigint FROM canonical_events")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(canonical, before);
        let published = sqlx::query_scalar::<_, i64>(
            "SELECT count(*)::bigint FROM visit_sites WHERE publication_status = 'published'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(published, 0);
    }

    #[tokio::test]
    async fn a_configured_model_call_has_a_nonzero_cost() {
        if !crate::llm::is_configured() {
            return;
        }
        let completion = crate::llm::complete_supervisor_plan(
            "Return one JSON object {\"tasks\":[{\"kind\":\"resolve_wikidata\",\"writes\":false,\"publication\":\"forbidden\"}]}",
        )
        .await
        .unwrap();
        let (cost, _) = talaria_ai::priced_usage(
            &completion.model,
            completion.input_tokens,
            completion.output_tokens,
        );
        assert!(completion.input_tokens + completion.output_tokens > 0);
        assert!(cost >= 1);
        assert!(completion.duration_ms > 0);
    }

    #[test]
    fn coordinates_come_from_the_place_statement() {
        let entity = json!({"claims": {"P625": [{"mainsnak": {"datavalue": {"value": {"latitude": 48.85, "longitude": 2.31}}}}]}});
        assert_eq!(coordinates_of(&entity), Some((48.85, 2.31)));
    }
}
