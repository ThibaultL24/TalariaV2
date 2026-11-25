// crates/talaria-api/src/supervisor_exec.rs
//! Typed documentary execution. The model names an allowed task. This module calls one existing connector.

use chrono::Utc;
use serde_json::Value;
use sqlx::PgPool;
use talaria_ai::{qualify_passage, segment_passages, Passage, PassageDecision, TaskKind};
use talaria_quality::{
    apply_gates, time_to_json, EventCandidate, EvidencePtr, GateContext, GateDecision, TypedTime,
};
use talaria_sources::{
    connectors::{
        parse_p18_filenames, CommonsConnector, WikidataSourceConnector,
        WikidataSourceConnectorConfig, WikipediaConnector, WikipediaConnectorConfig,
        WikisourceConnector,
    },
    ResolvedSubject, SourceConnector,
};
use talaria_store::{
    ensure_subject_entity, insert_person_candidate, upsert_supervised_document,
    PersonCandidateInsert, SourcedSite, TaskEffect,
};
use uuid::Uuid;

const DOCUMENT_CAP: usize = 4;

#[derive(Debug, Default)]
pub struct HarvestQuality {
    pub passages: u32,
    pub attributable: u32,
    pub candidates: u32,
    pub admissible: u32,
    pub rejected: u32,
    pub duplicates: u32,
    pub attachment_errors: u32,
    pub documentation_kept: u32,
}

#[derive(Debug, Default)]
pub struct HarvestReport {
    pub effects: Vec<TaskEffect>,
    pub documents: u32,
    pub new_documents: u32,
    pub sources: Vec<String>,
    pub sourced_site: Option<SourcedSite>,
    pub quality: HarvestQuality,
}

pub async fn execute_tasks(
    pool: &PgPool,
    experience: &str,
    qid: &str,
    label: &str,
    kinds: &[TaskKind],
    birth_year: Option<i32>,
    death_year: Option<i32>,
) -> HarvestReport {
    let mut report = HarvestReport::default();
    let mut entity: Option<Value> = None;
    let mut wikidata_document: Option<Uuid> = None;
    for kind in kinds {
        if *kind != TaskKind::ReviewHistoricalConsistency && report.documents >= DOCUMENT_CAP as u32
        {
            report
                .effects
                .push(refused(kind, qid, "document_budget_exhausted"));
            continue;
        }
        if !kind.allowed_for(parse_experience(experience)) {
            report.effects.push(refused(kind, qid, "wrong_experience"));
            continue;
        }
        let effect = match kind {
            TaskKind::ResolveWikidata | TaskKind::HarvestWdqsBiography => {
                fetch_wikidata(
                    pool,
                    qid,
                    label,
                    kind,
                    &mut entity,
                    &mut wikidata_document,
                    &mut report,
                )
                .await
            }
            TaskKind::HarvestWikipedia => fetch_wikipedia(pool, qid, label, &mut report).await,
            TaskKind::HarvestWikisource => {
                fetch_wikisource(
                    pool,
                    qid,
                    label,
                    entity.as_ref(),
                    birth_year,
                    death_year,
                    &mut report,
                )
                .await
            }
            TaskKind::HarvestCommons | TaskKind::CollectCommonsMedia => {
                fetch_commons(pool, qid, label, entity.as_ref(), kind, &mut report).await
            }
            TaskKind::DiscoverHeritageSites => {
                quarantine_heritage(
                    pool,
                    qid,
                    label,
                    &mut entity,
                    &mut wikidata_document,
                    &mut report,
                )
                .await
            }
            TaskKind::ReviewHistoricalConsistency => {
                crate::historical_review::review_task(pool, qid).await
            }
            TaskKind::ReviewPersonSiteRelation
            | TaskKind::FlagUnverifiedVisitFacts
            | TaskKind::CollectCulturalOccurrences
            | TaskKind::RecordDocumentaryGap => completed(
                kind,
                qid,
                false,
                serde_json::json!({
                    "reason": "recorded_without_publication",
                    "canonical_write": false,
                    "integration": "review_historical_record"
                }),
            ),
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
        Ok(inserted) => completed(
            kind,
            qid,
            inserted,
            serde_json::json!({"source": "wikidata"}),
        ),
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
    let page = connector
        .discover(&subject(qid, label, vec![]), None)
        .await?;
    let Some(document) = page.documents.first() else {
        anyhow::bail!("wikidata_document_missing");
    };
    let fetched = connector.fetch(document).await?;
    let stored = store_fetch(pool, "wikidata", qid, &fetched, report).await?;
    *entity = Some(fetched.raw_metadata);
    *document_id = Some(stored.0);
    remember(report, "wikidata", stored.1);
    Ok(stored.1)
}

async fn fetch_wikipedia(
    pool: &PgPool,
    qid: &str,
    label: &str,
    report: &mut HarvestReport,
) -> TaskEffect {
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
        return completed(
            &TaskKind::HarvestWikipedia,
            qid,
            false,
            serde_json::json!({"reason": "no_page"}),
        );
    };
    match connector.fetch(document).await {
        Ok(fetched) => match store_fetch(pool, "wikipedia", qid, &fetched, report).await {
            Ok(stored) => {
                remember(report, "wikipedia", stored.1);
                completed(
                    &TaskKind::HarvestWikipedia,
                    qid,
                    stored.1,
                    serde_json::json!({"source": "wikipedia", "title": document.title}),
                )
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
    birth_year: Option<i32>,
    death_year: Option<i32>,
    report: &mut HarvestReport,
) -> TaskEffect {
    let mut connector = match WikisourceConnector::new() {
        Ok(connector) => connector,
        Err(error) => return failed(&TaskKind::HarvestWikisource, qid, &error.to_string()),
    };
    connector.max_docs = 4;
    let identifiers = sitelink(entity, "frwikisource");
    let page = match connector
        .discover(&subject(qid, label, identifiers), None)
        .await
    {
        Ok(page) => page,
        Err(error) => return failed(&TaskKind::HarvestWikisource, qid, &error.to_string()),
    };
    let mut titles = Vec::new();
    let mut wrote = false;
    for document in page.documents.iter().take(2) {
        if talaria_sources::connectors::is_author_index(&document.title) {
            continue;
        }
        match connector.fetch(document).await {
            Ok(fetched) => match store_fetch(pool, "wikisource", qid, &fetched, report).await {
                Ok((document_id, inserted, passages)) => {
                    remember(report, "wikisource", inserted);
                    wrote |= inserted;
                    consider_passages(
                        pool,
                        qid,
                        label,
                        birth_year,
                        death_year,
                        document_id,
                        &passages,
                        report,
                    )
                    .await;
                    titles.push(document.title.clone());
                }
                Err(error) => return failed(&TaskKind::HarvestWikisource, qid, &error.to_string()),
            },
            Err(error) => return failed(&TaskKind::HarvestWikisource, qid, &error.to_string()),
        }
    }
    if titles.is_empty() {
        return completed(
            &TaskKind::HarvestWikisource,
            qid,
            false,
            serde_json::json!({"reason": "no_wikisource_work"}),
        );
    }
    completed(
        &TaskKind::HarvestWikisource,
        qid,
        wrote,
        serde_json::json!({"source": "wikisource", "titles": titles, "quality": quality_json(&report.quality)}),
    )
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
        return completed(
            kind,
            qid,
            false,
            serde_json::json!({"reason": "wikidata_entity_required"}),
        );
    };
    let Some(file) = parse_p18_filenames(entity).into_iter().next() else {
        return completed(
            kind,
            qid,
            false,
            serde_json::json!({"reason": "no_commons_file"}),
        );
    };
    let connector = match CommonsConnector::new() {
        Ok(connector) => connector,
        Err(error) => return failed(kind, qid, &error.to_string()),
    };
    let document = CommonsConnector::document_from_p18(&file);
    match connector.fetch(&document).await {
        Ok(fetched) => match store_fetch(pool, "commons", qid, &fetched, report).await {
            Ok(stored) => {
                remember(report, "commons", stored.1);
                completed(
                    kind,
                    qid,
                    stored.1,
                    serde_json::json!({"source": "commons", "file": file}),
                )
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
        return refused(
            &TaskKind::DiscoverHeritageSites,
            qid,
            "wikidata_entity_required",
        );
    };
    let Some(place) = documented_places(person).into_iter().next() else {
        return completed(
            &TaskKind::DiscoverHeritageSites,
            qid,
            inserted,
            serde_json::json!({"reason": "no_documented_place"}),
        );
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
        return completed(
            &TaskKind::DiscoverHeritageSites,
            qid,
            inserted,
            serde_json::json!({"reason": "coordinates_missing", "site_qid": place.target_qid}),
        );
    };
    let Some(document_id) = *document_id else {
        return refused(
            &TaskKind::DiscoverHeritageSites,
            qid,
            "source_document_required",
        );
    };
    let site_label = fetched
        .raw_metadata
        .pointer("/labels/en/value")
        .or_else(|| fetched.raw_metadata.pointer("/labels/fr/value"))
        .and_then(Value::as_str)
        .unwrap_or(&place.target_qid)
        .to_string();
    let revision = fetched
        .revision_id
        .as_deref()
        .and_then(|value| value.parse().ok());
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
    completed(
        &TaskKind::DiscoverHeritageSites,
        qid,
        false,
        serde_json::json!({"source": "wikidata", "publication": "quarantine"}),
    )
}

async fn store_fetch(
    pool: &PgPool,
    source_type: &str,
    qid: &str,
    fetched: &talaria_sources::FetchedDocument,
    report: &mut HarvestReport,
) -> anyhow::Result<(Uuid, bool, Vec<Passage>)> {
    let url = fetched
        .discovered
        .canonical_url
        .clone()
        .filter(|url| talaria_store::verifiable_source_url(url))
        .ok_or_else(|| anyhow::anyhow!("unverifiable_source_url"))?;
    let passages = segment_passages(&url, &fetched.text);
    report.quality.passages += u32::try_from(passages.len()).unwrap_or(u32::MAX);
    let payload = serde_json::json!(passages
        .iter()
        .map(|passage| serde_json::json!({
            "fragment_id": passage.fragment_id,
            "start": passage.start,
            "end": passage.end,
            "text": passage.text,
        }))
        .collect::<Vec<_>>());
    let (document_id, inserted) = upsert_supervised_document(
        pool,
        source_type,
        &url,
        &fetched.discovered.title,
        fetched.discovered.language.as_deref().unwrap_or("en"),
        qid,
        fetched.revision_id.as_deref(),
        fetched.license.as_deref(),
        &fetched.text,
        &payload,
    )
    .await?;
    Ok((document_id, inserted, passages))
}

fn quality_json(quality: &HarvestQuality) -> Value {
    serde_json::json!({
        "passages": quality.passages,
        "attributable": quality.attributable,
        "candidates": quality.candidates,
        "admissible": quality.admissible,
        "rejected": quality.rejected,
        "duplicates": quality.duplicates,
        "attachment_errors": quality.attachment_errors,
        "documentation_kept": quality.documentation_kept,
        "canonical_events": 0
    })
}

async fn consider_passages(
    pool: &PgPool,
    qid: &str,
    label: &str,
    birth_year: Option<i32>,
    death_year: Option<i32>,
    document_id: Uuid,
    passages: &[Passage],
    report: &mut HarvestReport,
) {
    for passage in passages {
        match qualify_passage(label, passage) {
            PassageDecision::AttachmentError => report.quality.attachment_errors += 1,
            PassageDecision::Documentation { .. } => report.quality.documentation_kept += 1,
            PassageDecision::Candidate(found) => {
                report.quality.attributable += 1;
                match submit_candidate(
                    pool,
                    qid,
                    label,
                    birth_year,
                    death_year,
                    document_id,
                    passage,
                    &found,
                )
                .await
                {
                    Ok(Admission::Admissible) => {
                        report.quality.candidates += 1;
                        report.quality.admissible += 1;
                    }
                    Ok(Admission::Rejected) => {
                        report.quality.candidates += 1;
                        report.quality.rejected += 1;
                    }
                    Ok(Admission::Duplicate) => report.quality.duplicates += 1,
                    Err(_) => report.quality.documentation_kept += 1,
                }
            }
        }
    }
}

enum Admission {
    Admissible,
    Rejected,
    Duplicate,
}

async fn submit_candidate(
    pool: &PgPool,
    qid: &str,
    label: &str,
    birth_year: Option<i32>,
    death_year: Option<i32>,
    document_id: Uuid,
    passage: &Passage,
    found: &talaria_ai::QualifiedPassage,
) -> anyhow::Result<Admission> {
    let fingerprint = format!(
        "supervisor:{}:{}:{}",
        passage.fragment_id, found.event_type, found.year
    );
    let existing =
        sqlx::query_scalar::<_, Uuid>("SELECT id FROM event_candidates WHERE fingerprint = $1")
            .bind(&fingerprint)
            .fetch_optional(pool)
            .await?;
    if existing.is_some() {
        return Ok(Admission::Duplicate);
    }
    let entity_id = ensure_subject_entity(pool, qid, label).await?;
    let time = TypedTime::Approx {
        year: found.year,
        surface: Some(found.year.to_string()),
    };
    let candidate = EventCandidate {
        id: Uuid::nil(),
        snapshot_id: Uuid::nil(),
        fragment_id: Uuid::nil(),
        clause_index: 0,
        subject_surface: label.to_string(),
        subject_entity_id: Some(entity_id),
        event_type: found.event_type.clone(),
        predicate: found.predicate.clone(),
        time: time.clone(),
        place_mentions: vec![],
        object_mentions: vec![],
        participant_mentions: vec![],
        place_entity_id: None,
        place_label: found.place.clone(),
        evidence_ptrs: vec![EvidencePtr {
            fragment_id: Uuid::nil(),
            clause_index: 0,
            start_offset: i32::try_from(passage.start).unwrap_or(0),
            end_offset: i32::try_from(passage.end).unwrap_or(0),
            quoted_text: found.quote.clone(),
        }],
        extractor_version: "supervisor:passage:v1".into(),
        fingerprint: fingerprint.clone(),
        status: talaria_quality::CandidateStatus::Pending,
        rejection_codes: vec![],
    };
    let decision = apply_gates(
        &candidate,
        &GateContext {
            subject_birth_year: birth_year,
            subject_death_year: death_year,
            ..GateContext::default()
        },
    );
    let (status, admission) = match &decision {
        GateDecision::Reject(_) => ("rejected", Admission::Rejected),
        GateDecision::Accept | GateDecision::NeedsReview => ("needs_review", Admission::Admissible),
    };
    insert_person_candidate(
        pool,
        &PersonCandidateInsert {
            subject_surface: label.to_string(),
            subject_entity_id: entity_id,
            event_type: found.event_type.clone(),
            predicate: found.predicate.clone(),
            time_json: time_to_json(&time),
            place_label: found.place.clone(),
            evidence_ptrs: serde_json::json!([{
                "fragment_id": passage.fragment_id,
                "start": passage.start,
                "end": passage.end,
                "quoted_text": found.quote,
                "source": "wikisource"
            }]),
            extractor_version: "supervisor:passage:v1".into(),
            fingerprint,
            occurrence_key: format!(
                "{qid}:{}:{}:{}",
                found.event_type, found.year, passage.fragment_id
            ),
            primary_object: None,
            action_role: Some(found.predicate.clone()),
            status: status.into(),
            rejection_codes: decision.codes(),
            judgment_json: serde_json::json!({
                "gate": match &decision {
                    GateDecision::Accept => "accept",
                    GateDecision::NeedsReview => "needs_review",
                    GateDecision::Reject(_) => "reject",
                },
                "canonical_write": false
            }),
            raw_document_id: document_id,
        },
    )
    .await?;
    Ok(admission)
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
    async fn a_precise_passage_stays_a_candidate(pool: sqlx::PgPool) {
        let text = "Le couronnement de Napoléon a lieu à Paris en 1804, devant le pape.";
        let passages = segment_passages("https://fr.wikisource.org/wiki/Lettre_de_test", text);
        let (document_id, _) = upsert_supervised_document(
            &pool,
            "wikisource",
            "https://fr.wikisource.org/wiki/Lettre_de_test",
            "Lettre",
            "fr",
            "Q517",
            None,
            Some("CC-BY-SA"),
            text,
            &json!([]),
        )
        .await
        .unwrap();
        let mut report = HarvestReport::default();
        consider_passages(
            &pool,
            "Q517",
            "Napoléon",
            Some(1769),
            Some(1821),
            document_id,
            &passages,
            &mut report,
        )
        .await;
        assert_eq!(report.quality.admissible, 1);
        assert_eq!(report.quality.candidates, 1);
        let status = sqlx::query_scalar::<_, String>("SELECT status FROM event_candidates")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(status, "needs_review");
        let canonical =
            sqlx::query_scalar::<_, i64>("SELECT count(*)::bigint FROM canonical_events")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(canonical, 0);
        let weak = segment_passages(
            "https://fr.wikisource.org/wiki/Lettre_faible",
            "Napoléon écrit à Joseph le 12 mai 1804, sans nommer un événement.",
        );
        let (weak_id, _) = upsert_supervised_document(
            &pool,
            "wikisource",
            "https://fr.wikisource.org/wiki/Lettre_faible",
            "Lettre faible",
            "fr",
            "Q517",
            None,
            Some("CC-BY-SA"),
            "Napoléon écrit à Joseph le 12 mai 1804, sans nommer un événement.",
            &json!([]),
        )
        .await
        .unwrap();
        consider_passages(
            &pool,
            "Q517",
            "Napoléon",
            Some(1769),
            Some(1821),
            weak_id,
            &weak,
            &mut report,
        )
        .await;
        assert_eq!(report.quality.candidates, 1);
        assert!(report.quality.documentation_kept >= 1);
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
            &[
                TaskKind::ResolveWikidata,
                TaskKind::HarvestWikipedia,
                TaskKind::HarvestWikisource,
            ],
            Some(1769),
            Some(1821),
        )
        .await;
        assert!(istoria.documents >= 1, "{istoria:?}");
        assert!(
            istoria
                .effects
                .iter()
                .all(|effect| effect.status != "failed"),
            "{istoria:?}"
        );
        let wikisource = istoria
            .effects
            .iter()
            .find(|effect| effect.kind == "harvest_wikisource")
            .unwrap();
        let titles = wikisource.detail["titles"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        assert!(!titles.is_empty(), "{wikisource:?}");
        assert!(
            titles
                .iter()
                .all(|title| !title.as_str().unwrap_or("").starts_with("Auteur:")),
            "{wikisource:?}"
        );
        assert!(istoria.quality.passages > 0, "{istoria:?}");
        assert!(
            istoria.quality.documentation_kept
                + istoria.quality.candidates
                + istoria.quality.attachment_errors
                > 0,
            "{istoria:?}"
        );
        println!(
            "istoria titles {titles:?} passages {} candidates {} admissible {} kept {} attachment {}",
            istoria.quality.passages, istoria.quality.candidates, istoria.quality.admissible,
            istoria.quality.documentation_kept, istoria.quality.attachment_errors
        );
        let again = execute_tasks(
            &pool,
            "istoria",
            "Q517",
            "Napoleon",
            &[TaskKind::ResolveWikidata, TaskKind::HarvestWikipedia],
            Some(1769),
            Some(1821),
        )
        .await;
        assert_eq!(again.new_documents, 0);
        let poreia = execute_tasks(
            &pool,
            "poreia",
            "Q5582",
            "Vincent van Gogh",
            &[
                TaskKind::DiscoverHeritageSites,
                TaskKind::CollectCommonsMedia,
            ],
            Some(1853),
            Some(1890),
        )
        .await;
        assert!(poreia.documents >= 1, "{poreia:?}");
        assert!(
            poreia
                .effects
                .iter()
                .all(|effect| effect.status != "failed"),
            "{poreia:?}"
        );
        let caesar = execute_tasks(
            &pool,
            "istoria",
            "Q1048",
            "Julius Caesar",
            &[TaskKind::ResolveWikidata],
            Some(-100),
            Some(-44),
        )
        .await;
        assert!(caesar.documents >= 1, "{caesar:?}");
        let text = sqlx::query_scalar::<_, String>(
            r#"
            SELECT coalesce(string_agg(passage->>'text', ' '), '')
            FROM raw_documents, jsonb_array_elements(payload->'passages') AS passage
            WHERE source_identifier = 'Q1048'
            "#,
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert!(
            text.contains('-'),
            "a BCE statement must keep its sign: {text}"
        );
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
        let canonical =
            sqlx::query_scalar::<_, i64>("SELECT count(*)::bigint FROM canonical_events")
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
        let model = crate::llm::model();
        let Some(ceiling) =
            talaria_ai::output_token_ceiling(&model, 32, talaria_ai::MAX_COST_USD_MICROS)
        else {
            let price = talaria_ai::price_usage(&model, 31, 25);
            assert!(price.cost_usd_micros.is_none());
            return;
        };
        let completion = crate::llm::complete_supervisor_plan(
            "Return one JSON object {\"tasks\":[{\"kind\":\"resolve_wikidata\",\"writes\":false,\"publication\":\"forbidden\"}]}",
            ceiling,
        )
        .await
        .unwrap();
        let price = talaria_ai::price_usage(
            &completion.model,
            completion.input_tokens,
            completion.output_tokens,
        );
        assert!(completion.input_tokens + completion.output_tokens > 0);
        assert!(
            price.rate.is_some(),
            "listed model {} has no rate",
            completion.model
        );
        assert!(price.cost_usd_micros.unwrap() >= 1);
        assert!(price.cost_usd_micros.unwrap() <= talaria_ai::MAX_COST_USD_MICROS);
        println!(
            "model {} in {} out {} cost_usd_micros {} ceiling {}",
            completion.model,
            completion.input_tokens,
            completion.output_tokens,
            price.cost_usd_micros.unwrap(),
            ceiling
        );
        assert!(completion.duration_ms > 0);
    }

    #[test]
    fn coordinates_come_from_the_place_statement() {
        let entity = json!({"claims": {"P625": [{"mainsnak": {"datavalue": {"value": {"latitude": 48.85, "longitude": 2.31}}}}]}});
        assert_eq!(coordinates_of(&entity), Some((48.85, 2.31)));
    }
}
