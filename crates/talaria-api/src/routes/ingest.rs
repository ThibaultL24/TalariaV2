// crates/talaria-api/src/routes/ingest.rs
//! On-demand density ingest triggered from the explorer search bar (Lot E).

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
use crate::lot_e::{run_lot_e_density_ingest, write_minimal_seed_list};
use talaria_sources::DensityTargets;

#[derive(Debug, Clone)]
pub struct IngestJob {
    pub id: Uuid,
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
    /// Soft cap on seed titles processed (0 = Lot E default budget).
    #[serde(default)]
    pub max_titles: Option<u32>,
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

pub async fn start_ingest(
    State(state): State<AppState>,
    Json(body): Json<StartIngestBody>,
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
            Json(json!({ "error": "live_required_for_density_ingest" })),
        ));
    }

    {
        let jobs = state.ingest_jobs.lock().await;
        if let Some(existing) = jobs.values().find(|job| {
            matches!(job.status.as_str(), "queued" | "running")
                && (body
                    .qid
                    .as_ref()
                    .is_some_and(|qid| job.qid.as_ref() == Some(qid))
                    || job.subject.eq_ignore_ascii_case(&subject))
        }) {
            return Ok(Json(json!({
                "job_id": existing.id,
                "status": existing.status,
                "subject": existing.subject,
                "qid": existing.qid,
                "entity_id": existing.entity_id,
                "deduped": true,
            })));
        }
    }

    let seed_list = resolve_seed_list(&subject).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("seed_list: {e}") })),
        )
    })?;
    let seed_list_display = seed_list.display().to_string();

    let job_id = Uuid::new_v4();
    let job = IngestJob {
        id: job_id,
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
    let wiki_lang = config.wiki_lang.clone();

    tokio::spawn(async move {
        {
            let mut jobs = jobs.lock().await;
            if let Some(job) = jobs.get_mut(&job_id) {
                job.status = "running".into();
            }
        }

        let targets = DensityTargets {
            target_timeline_events: 500,
            target_map_events: 500,
            max_documents: 400,
            max_linked_entities: 5_000,
            max_depth: 3,
            max_documents_per_source: 2_500,
        };

        let result = run_lot_e_density_ingest(
            &config,
            &subject_for_job,
            qid.as_deref(),
            &seed_list,
            targets,
            &wiki_lang,
            max_titles,
        )
        .await;

        let mut jobs = jobs.lock().await;
        let Some(job) = jobs.get_mut(&job_id) else {
            return;
        };
        match result {
            Ok(report_text) => {
                let report: Value = serde_json::from_str(&report_text)
                    .unwrap_or_else(|_| json!({ "raw": report_text }));
                job.entity_id = report
                    .pointer("/subject/entity_id")
                    .or_else(|| report.get("entity_id"))
                    .and_then(|v| v.as_str())
                    .and_then(|s| Uuid::parse_str(s).ok())
                    .or_else(|| {
                        report
                            .get("entity_id")
                            .and_then(|v| v.as_str())
                            .and_then(|s| Uuid::parse_str(s).ok())
                    });
                // Lot E report shapes vary — also try nested density subject.
                if job.entity_id.is_none() {
                    job.entity_id = report
                        .pointer("/comparison/entity_id")
                        .and_then(|v| v.as_str())
                        .and_then(|s| Uuid::parse_str(s).ok());
                }
                job.report = Some(report);
                job.status = "done".into();
            }
            Err(err) => {
                job.status = "failed".into();
                job.error = Some(err.to_string());
            }
        }
    });

    Ok(Json(json!({
        "job_id": job_id,
        "status": "queued",
        "subject": subject,
        "qid": body.qid,
        "seed_list": seed_list_display,
        "mode": "lot_e_density",
        "deduped": false,
    })))
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
        "status": job.status,
        "subject": job.subject,
        "qid": job.qid,
        "entity_id": job.entity_id,
        "error": job.error,
        "report": job.report,
    })))
}
