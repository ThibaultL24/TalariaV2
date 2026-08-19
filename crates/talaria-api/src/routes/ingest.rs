// crates/talaria-api/src/routes/ingest.rs
//! Two explicit ingest lanes:
//! - **explorer** — Wikipedia/Wikidata/Wikisource/Commons life trace plus catalog-derived dated facts
//! - **agora** — bibliographic catalogs + historiography (theories, theses, opinions)

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;

use super::AppState;
use crate::corpus_ingest::{self, explorer_fact_providers, live_corpus_providers};
use crate::historiography;
use crate::lot_e::{run_lot_e_density_ingest, write_minimal_seed_list};
use talaria_sources::DensityTargets;

pub const LANE_EXPLORER: &str = "explorer";
pub const LANE_AGORA: &str = "agora";

#[derive(Debug, Clone)]
pub struct IngestJob {
    pub id: Uuid,
    pub lane: String,
    pub subject: String,
    pub qid: Option<String>,
    pub status: String,
    pub entity_id: Option<Uuid>,
    pub report: Option<Value>,
    pub error: Option<String>,
}

pub type IngestJobMap = Arc<Mutex<HashMap<Uuid, IngestJob>>>;

#[derive(Debug, Deserialize)]
pub struct StartIngestBody {
    pub subject: String,
    pub qid: Option<String>,
    #[serde(default = "default_live")]
    pub live: bool,
    /// Soft cap on seed titles processed (explorer lane only).
    #[serde(default)]
    pub max_titles: Option<u32>,
    /// Per-provider document cap (agora lane only).
    #[serde(default)]
    pub corpus_limit: Option<u32>,
}

fn default_live() -> bool {
    true
}

fn seed_slug(subject: &str) -> String {
    subject
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect::<String>()
        .trim_matches('_')
        .to_string()
}

fn resolve_seed_list(subject: &str) -> anyhow::Result<PathBuf> {
    let slug = seed_slug(subject);
    let curated = PathBuf::from(format!("fixtures/seeds/{slug}_wiki_titles.txt"));
    if curated.is_file() {
        return Ok(curated);
    }
    write_minimal_seed_list(subject)
}

fn parse_entity_id_from_report(report: &Value) -> Option<Uuid> {
    for nested_key in ["explorer", "corpus", "agora"] {
        if let Some(nested) = report.get(nested_key) {
            if let Some(id) = parse_entity_id_from_report(nested) {
                return Some(id);
            }
        }
    }
    report
        .pointer("/subject/entity_id")
        .or_else(|| report.get("entity_id"))
        .or_else(|| report.pointer("/comparison/entity_id"))
        .or_else(|| report.get("subject_entity_id"))
        .and_then(|v| v.as_str())
        .and_then(|s| Uuid::parse_str(s).ok())
}

async fn find_running_job(
    jobs: &IngestJobMap,
    lane: &str,
    subject: &str,
    qid: Option<&str>,
) -> Option<IngestJob> {
    let jobs = jobs.lock().await;
    jobs
        .values()
        .find(|job| {
            job.lane == lane
                && matches!(job.status.as_str(), "queued" | "running")
                && (qid.is_some_and(|q| job.qid.as_deref() == Some(q))
                    || job.subject.eq_ignore_ascii_case(subject))
        })
        .cloned()
}

fn job_started_response(job: &IngestJob, deduped: bool, extra: Value) -> Json<Value> {
    let mut body = json!({
        "job_id": job.id,
        "lane": job.lane,
        "status": job.status,
        "subject": job.subject,
        "qid": job.qid,
        "entity_id": job.entity_id,
        "deduped": deduped,
    });
    if let Some(obj) = body.as_object_mut() {
        if let Some(extra_obj) = extra.as_object() {
            for (k, v) in extra_obj {
                obj.insert(k.clone(), v.clone());
            }
        }
    }
    Json(body)
}

pub async fn start_explorer_ingest(
    State(state): State<AppState>,
    Json(body): Json<StartIngestBody>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    start_lane_ingest(state, body, LANE_EXPLORER).await
}

pub async fn start_agora_ingest(
    State(state): State<AppState>,
    Json(body): Json<StartIngestBody>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    start_lane_ingest(state, body, LANE_AGORA).await
}

/// Backward-compatible alias: explorer life-trace ingest.
pub async fn start_ingest(
    State(state): State<AppState>,
    Json(body): Json<StartIngestBody>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    start_explorer_ingest(State(state), Json(body)).await
}

async fn start_lane_ingest(
    state: AppState,
    body: StartIngestBody,
    lane: &str,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let subject = body.subject.trim().to_string();
    if subject.len() < 2 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "subject_too_short" })),
        ));
    }
    if body.live && state.offline_only {
        return Err((
            StatusCode::FORBIDDEN,
            Json(json!({ "error": "offline_only" })),
        ));
    }
    if !body.live {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "live_required_for_ingest" })),
        ));
    }

    if let Some(existing) =
        find_running_job(&state.ingest_jobs, lane, &subject, body.qid.as_deref()).await
    {
        return Ok(job_started_response(
            &existing,
            true,
            json!({
                "mode": lane,
                "purpose": lane_purpose(lane),
            }),
        ));
    }

    let job_id = Uuid::new_v4();
    let job = IngestJob {
        id: job_id,
        lane: lane.into(),
        subject: subject.clone(),
        qid: body.qid.clone(),
        status: "queued".into(),
        entity_id: None,
        report: None,
        error: None,
    };
    state.ingest_jobs.lock().await.insert(job_id, job);

    let config = state.config.clone();
    let jobs = state.ingest_jobs.clone();
    let qid = body.qid.clone();
    let subject_for_job = subject.clone();
    let max_titles = body.max_titles.filter(|n| *n > 0);
    let corpus_limit = body.corpus_limit.filter(|n| *n > 0).unwrap_or(15);
    let wiki_lang = config.wiki_lang.clone();
    let lane_owned = lane.to_string();

    let mut start_extra = json!({
        "mode": lane,
        "purpose": lane_purpose(lane),
    });

    let seed_list_display = if lane == LANE_EXPLORER {
        let seed_list = resolve_seed_list(&subject).map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": format!("seed_list: {e}") })),
            )
        })?;
        let display = seed_list.display().to_string();
        start_extra["seed_list"] = json!(display);
        Some(seed_list)
    } else {
        None
    };

    tokio::spawn(async move {
        {
            let mut jobs = jobs.lock().await;
            if let Some(job) = jobs.get_mut(&job_id) {
                job.status = "running".into();
            }
        }

        let result = if lane_owned == LANE_EXPLORER {
            let seed_list = seed_list_display.expect("explorer seed list");
            run_explorer_lane(
                &config,
                &subject_for_job,
                qid.as_deref(),
                &seed_list,
                &wiki_lang,
                max_titles,
            )
            .await
        } else {
            run_agora_lane(
                &config,
                &subject_for_job,
                qid.as_deref(),
                corpus_limit,
            )
            .await
        };

        let mut jobs = jobs.lock().await;
        let Some(job) = jobs.get_mut(&job_id) else {
            return;
        };
        match result {
            Ok(report) => {
                job.entity_id = parse_entity_id_from_report(&report);
                job.report = Some(report);
                job.status = "done".into();
            }
            Err(err) => {
                job.status = "failed".into();
                job.error = Some(err.to_string());
            }
        }
    });

    let mut start_body = json!({
        "job_id": job_id,
        "lane": lane,
        "status": "queued",
        "subject": subject,
        "qid": body.qid,
        "deduped": false,
        "mode": lane,
        "purpose": lane_purpose(lane),
    });
    if lane == LANE_EXPLORER {
        if let Some(obj) = start_body.as_object_mut() {
            if let Some(extra_obj) = start_extra.as_object() {
                for (k, v) in extra_obj {
                    obj.insert(k.clone(), v.clone());
                }
            }
        }
    }

    Ok(Json(start_body))
}

async fn run_explorer_lane(
    config: &talaria_core::AppConfig,
    subject: &str,
    qid: Option<&str>,
    seed_list: &std::path::Path,
    wiki_lang: &str,
    max_titles: Option<u32>,
) -> anyhow::Result<Value> {
    let targets = DensityTargets {
        target_timeline_events: 500,
        target_map_events: 500,
        max_documents: 400,
        max_linked_entities: 5_000,
        max_depth: 3,
        max_documents_per_source: 2_500,
    };
    let lot_e_text = run_lot_e_density_ingest(
        config,
        subject,
        qid,
        seed_list,
        targets,
        wiki_lang,
        max_titles,
    )
    .await?;
    let wikipedia_wikidata = parse_json_report(&lot_e_text);

    let catalog_facts = match crate::ingest::run_ingest_quality(
        config,
        subject,
        qid,
        Some(explorer_fact_providers()),
        false,
        true,
    )
    .await
    {
        Ok(text) => parse_json_report(&text),
        Err(error) => {
            tracing::warn!(error = %error, "explorer catalog fact ingest failed");
            json!({ "error": error.to_string() })
        }
    };

    let entity_id = parse_entity_id_from_report(&wikipedia_wikidata)
        .or_else(|| parse_entity_id_from_report(&catalog_facts));

    Ok(json!({
        "lane": LANE_EXPLORER,
        "purpose": lane_purpose(LANE_EXPLORER),
        "wikipedia_wikidata": wikipedia_wikidata,
        "catalog_facts": catalog_facts,
        "subject": {
            "entity_id": entity_id.map(|id| id.to_string()),
            "label": subject,
            "qid": qid,
        },
    }))
}

async fn run_agora_lane(
    config: &talaria_core::AppConfig,
    subject: &str,
    qid: Option<&str>,
    corpus_limit: u32,
) -> anyhow::Result<Value> {
    let providers = live_corpus_providers();
    let corpus_report = corpus_ingest::run_corpus_ingest(
        config,
        subject,
        qid,
        &providers,
        corpus_limit,
        false,
        None,
        true,
    )
    .await?;
    let corpus_json: Value = serde_json::from_str(&corpus_report)
        .unwrap_or_else(|_| json!({ "raw": corpus_report }));

    let hist_report = historiography::run_historiography_extract(config, subject, None).await?;
    let hist_json: Value = serde_json::from_str(&hist_report)
        .unwrap_or_else(|_| json!({ "raw": hist_report }));

    Ok(json!({
        "lane": LANE_AGORA,
        "purpose": lane_purpose(LANE_AGORA),
        "corpus": corpus_json,
        "historiography": hist_json,
        "entity_id": corpus_json.get("subject_entity_id")
            .or_else(|| hist_json.get("entity_id")),
    }))
}

fn parse_json_report(text: &str) -> Value {
    serde_json::from_str(text).unwrap_or_else(|_| json!({ "raw": text }))
}

fn lane_purpose(lane: &str) -> &'static str {
    match lane {
        LANE_EXPLORER => {
            "Dated life facts and anecdotes with places — geographic trace for the map and timeline"
        }
        LANE_AGORA => {
            "Theories, controversies, opinions, analyses, and academic works about the person"
        }
        _ => "unknown ingest lane",
    }
}

pub async fn get_ingest_job(
    State(state): State<AppState>,
    Path(job_id): Path<Uuid>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let jobs = state.ingest_jobs.lock().await;
    let Some(job) = jobs.get(&job_id) else {
        return Err((
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "job_not_found" })),
        ));
    };
    Ok(Json(json!({
        "job_id": job.id,
        "lane": job.lane,
        "purpose": lane_purpose(&job.lane),
        "status": job.status,
        "subject": job.subject,
        "qid": job.qid,
        "entity_id": job.entity_id,
        "error": job.error,
        "report": job.report,
    })))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entity_id_reads_subject_pointer() {
        let report = json!({
            "subject": { "entity_id": "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa" }
        });
        assert_eq!(
            parse_entity_id_from_report(&report).unwrap().to_string(),
            "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa"
        );
    }

    #[test]
    fn agora_lane_uses_every_live_catalog() {
        let providers = live_corpus_providers();
        for name in [
            "hal",
            "persee",
            "gallica",
            "theses_fr",
            "open_alex",
            "bnf",
            "open_library",
            "internet_archive",
            "europeana",
        ] {
            assert!(
                providers.iter().any(|p| p == name),
                "agora missing catalog {name}"
            );
        }
        assert!(!providers.iter().any(|p| p == "wikisource"));
        assert!(!providers.iter().any(|p| p == "wikimedia_commons"));
    }

    #[test]
    fn explorer_lane_collects_sister_wikis() {
        let providers = explorer_fact_providers();
        assert!(providers.iter().any(|p| p == "wikisource"));
        assert!(providers.iter().any(|p| p == "wikimedia_commons"));
        assert!(providers.iter().any(|p| p == "hal"));
    }

    #[test]
    fn explorer_and_agora_purposes_stay_separated() {
        let explorer = lane_purpose(LANE_EXPLORER).to_lowercase();
        let agora = lane_purpose(LANE_AGORA).to_lowercase();
        assert!(explorer.contains("life") || explorer.contains("map"));
        assert!(agora.contains("theor") || agora.contains("academic"));
        assert!(!explorer.contains("controvers"));
        assert!(!agora.contains("timeline"));
    }
}
