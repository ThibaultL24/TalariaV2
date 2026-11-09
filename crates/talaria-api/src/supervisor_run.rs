// crates/talaria-api/src/supervisor_run.rs
//! Plan from stored coverage, then execute only connector tasks the backend allows.

use std::time::Instant;

use talaria_ai::{
    accept_model_tasks, deterministic_plan, estimate_tokens, plan_json, priced_usage, valid_qid,
    within_token_budget, Experience, PlanTask, SubjectBrief, PROMPT_VERSION,
};
use talaria_core::AppConfig;
use talaria_store::{
    load_subject_coverage, record_supervisor_run, resume_failed_tasks, reusable_supervisor_report,
    SubjectCoverage, SupervisorCall, TaskEffect,
};

use crate::supervisor_exec::execute_tasks;

pub async fn agent_plan(
    config: &AppConfig,
    experience: &str,
    qid: &str,
    label: &str,
    live: bool,
) -> anyhow::Result<String> {
    let experience = parse_experience(experience)?;
    anyhow::ensure!(valid_qid(qid), "invalid_qid");
    let pool = talaria_store::connect(config).await?;
    talaria_store::run_migrations(&pool).await?;
    let coverage = load_subject_coverage(&pool, qid).await?;
    let brief = brief_from_coverage(qid, label, &coverage);
    let started = Instant::now();
    let (plan, usage) = rank_plan(experience, &brief, live).await;
    let mut body = plan_json(&plan);
    body["live_status"] = serde_json::json!(usage.live_status);
    body["prompt_version"] = serde_json::json!(PROMPT_VERSION);
    body["coverage"] = coverage_json(&coverage);
    body["usage"] = serde_json::json!({
        "input_tokens": usage.input_tokens,
        "output_tokens": usage.output_tokens,
        "cost_usd_micros": usage.cost_usd_micros,
        "estimated": usage.estimated,
        "duration_ms": started.elapsed().as_millis() as u64
    });
    Ok(serde_json::to_string_pretty(&body)?)
}

pub async fn agent_dispatch(
    config: &AppConfig,
    experience: &str,
    qid: &str,
    label: &str,
    execute: bool,
    live: bool,
) -> anyhow::Result<String> {
    let experience = parse_experience(experience)?;
    anyhow::ensure!(valid_qid(qid), "invalid_qid");
    let started = Instant::now();
    let pool = talaria_store::connect(config).await?;
    talaria_store::run_migrations(&pool).await?;
    let before = load_subject_coverage(&pool, qid).await?;
    let brief = brief_from_coverage(qid, label, &before);
    let (plan, usage) = rank_plan(experience, &brief, live).await;
    let over_budget = usage.cost_usd_micros > 20_000;
    let mut call = SupervisorCall {
        experience: experience.as_str().into(),
        subject_qid: qid.into(),
        subject_label: brief.label.clone(),
        provider: plan.provider.clone(),
        model: plan.model.clone(),
        prompt_version: PROMPT_VERSION.into(),
        plan_json: plan_json(&plan),
        execute,
        input_tokens: i32::try_from(usage.input_tokens).unwrap_or(i32::MAX),
        output_tokens: i32::try_from(usage.output_tokens).unwrap_or(i32::MAX),
        cost_usd_micros: i32::try_from(usage.cost_usd_micros).unwrap_or(i32::MAX),
        cost_estimated: usage.estimated,
        duration_ms: 0,
        effects: queued(&plan.tasks),
        sourced_site: None,
    };
    if call.effects.is_empty() {
        call.effects.push(TaskEffect {
            key: format!("resolve_wikidata:{qid}"),
            kind: "resolve_wikidata".into(),
            status: "queued",
            writes: false,
            detail: serde_json::json!({"reason": "no_open_task"}),
        });
    }
    if let Some(report) = reusable_supervisor_report(&pool, &call).await? {
        return Ok(serde_json::to_string_pretty(&report)?);
    }
    if execute && !over_budget {
        let harvest = execute_tasks(
            &pool,
            experience.as_str(),
            qid,
            &brief.label,
            &plan.tasks.iter().map(|task| task.kind).collect::<Vec<_>>(),
        )
        .await;
        call.effects = harvest.effects;
        call.sourced_site = harvest.sourced_site;
    } else if execute {
        call.effects = plan
            .tasks
            .iter()
            .map(|task| TaskEffect {
                key: task.key.clone(),
                kind: task.kind.as_str().into(),
                status: "refused",
                writes: false,
                detail: serde_json::json!({"reason": "budget_exhausted"}),
            })
            .collect();
    }
    call.duration_ms = i32::try_from(started.elapsed().as_millis()).unwrap_or(i32::MAX);
    let mut report = record_supervisor_run(&pool, &call).await?;
    if report["idempotent"] == true && execute {
        report = resume_failed_tasks(&pool, &call).await?;
    }
    let after = load_subject_coverage(&pool, qid).await?;
    report["documents_before"] = serde_json::json!(document_total(&before));
    report["documents_after"] = serde_json::json!(document_total(&after));
    report["new_documents"] = serde_json::json!(document_total(&after).saturating_sub(document_total(&before)));
    report["live_status"] = serde_json::json!(usage.live_status);
    Ok(serde_json::to_string_pretty(&report)?)
}

struct ModelUsage {
    live_status: String,
    input_tokens: u32,
    output_tokens: u32,
    cost_usd_micros: u64,
    estimated: bool,
}

async fn rank_plan(experience: Experience, brief: &SubjectBrief, live: bool) -> (talaria_ai::AgentPlan, ModelUsage) {
    let mut plan = deterministic_plan(experience, brief);
    if !live {
        return (plan, ModelUsage {
            live_status: "not_requested".into(),
            input_tokens: 0,
            output_tokens: 0,
            cost_usd_micros: 0,
            estimated: false,
        });
    }
    let prompt = format!(
        "Return one JSON object with a tasks array. Each task has kind, writes=false, publication=forbidden. \
         Choose only among these kinds: {}. Subject {} {} has wikipedia documents {}, wikisource {}, commons {}, sourced events {}, published sites {}.",
        plan.tasks.iter().map(|task| task.kind.as_str()).collect::<Vec<_>>().join(", "),
        brief.qid,
        brief.label,
        brief.wikipedia_documents,
        brief.wikisource_count,
        brief.commons_documents,
        brief.sourced_event_count,
        brief.published_site_count
    );
    if !within_token_budget(&prompt) {
        return (plan, ModelUsage {
            live_status: "budget_refused".into(),
            input_tokens: estimate_tokens(&prompt),
            output_tokens: 0,
            cost_usd_micros: 0,
            estimated: false,
        });
    }
    match crate::llm::complete_supervisor_plan(&prompt).await {
        Ok(completion) => {
            let (cost, estimated_price) = priced_usage(&completion.model, completion.input_tokens, completion.output_tokens);
            let (kinds, refused) = accept_model_tasks(experience, &completion.value);
            plan.refused.extend(refused);
            if !kinds.is_empty() {
                plan.tasks = kinds
                    .into_iter()
                    .map(|kind| PlanTask {
                        key: format!("{}:{}", kind.as_str(), brief.qid),
                        kind,
                        priority: 2,
                        reason: "Kept from the model because it is an allowed read-only task.".into(),
                    })
                    .collect();
                plan.provider = completion.provider;
                plan.model = completion.model;
            }
            (plan, ModelUsage {
                live_status: "model_constrained".into(),
                input_tokens: completion.input_tokens,
                output_tokens: completion.output_tokens,
                cost_usd_micros: cost,
                estimated: completion.tokens_estimated || estimated_price,
            })
        }
        Err(error) => (plan, ModelUsage {
            live_status: format!("fallback:{error}"),
            input_tokens: 0,
            output_tokens: 0,
            cost_usd_micros: 0,
            estimated: false,
        }),
    }
}

fn brief_from_coverage(qid: &str, label: &str, coverage: &SubjectCoverage) -> SubjectBrief {
    SubjectBrief {
        qid: qid.to_string(),
        label: if label.trim().is_empty() { coverage.label.clone() } else { label.trim().to_string() },
        birth_year: coverage.birth_year,
        death_year: coverage.death_year,
        has_wikipedia: coverage.has_wikipedia,
        wikipedia_documents: coverage.wikipedia_documents,
        wikisource_count: coverage.wikisource_count,
        commons_documents: coverage.commons_documents,
        sourced_event_count: coverage.sourced_event_count,
        published_site_count: coverage.published_site_count,
    }
}

fn coverage_json(coverage: &SubjectCoverage) -> serde_json::Value {
    serde_json::json!({
        "label": coverage.label,
        "birth_year": coverage.birth_year,
        "death_year": coverage.death_year,
        "wikipedia_documents": coverage.wikipedia_documents,
        "wikisource_documents": coverage.wikisource_count,
        "commons_documents": coverage.commons_documents,
        "sourced_events": coverage.sourced_event_count,
        "published_sites": coverage.published_site_count
    })
}

fn document_total(coverage: &SubjectCoverage) -> u32 {
    coverage.wikipedia_documents + coverage.wikisource_count + coverage.commons_documents
}

fn queued(tasks: &[PlanTask]) -> Vec<TaskEffect> {
    tasks
        .iter()
        .map(|task| TaskEffect {
            key: task.key.clone(),
            kind: task.kind.as_str().into(),
            status: "queued",
            writes: false,
            detail: serde_json::json!({"reason": "read_only"}),
        })
        .collect()
}

fn parse_experience(value: &str) -> anyhow::Result<Experience> {
    Experience::parse(value).ok_or_else(|| anyhow::anyhow!("invalid_experience"))
}
