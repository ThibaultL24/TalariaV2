// crates/talaria-api/src/supervisor_run.rs
//! Plan from stored coverage, then execute only connector tasks the backend allows.

use std::time::Instant;

use talaria_ai::{
    accept_model_tasks, deterministic_plan, estimate_tokens, plan_json, price_usage, valid_qid,
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
    let ledger = talaria_ai::SpendLedger {
        task_spent_usd_micros: 0,
        ingest_spent_usd_micros: 0,
        qid_spent_usd_micros: talaria_store::supervisor_subject_spend_usd_micros(
            &pool,
            qid,
            talaria_ai::QID_BUDGET_WINDOW_DAYS,
        )
        .await?,
    };
    let (plan, usage) = rank_plan(experience, &brief, live, ledger).await;
    let mut body = plan_json(&plan);
    body["live_status"] = serde_json::json!(usage.live_status);
    body["prompt_version"] = serde_json::json!(PROMPT_VERSION);
    body["coverage"] = coverage_json(&coverage);
    body["usage"] = serde_json::json!({
        "input_tokens": usage.input_tokens,
        "output_tokens": usage.output_tokens,
        "cost_usd_micros": usage.cost_usd_micros,
        "input_usd_micros_per_million": usage.input_rate,
        "output_usd_micros_per_million": usage.output_rate,
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
    let ledger = talaria_ai::SpendLedger {
        task_spent_usd_micros: 0,
        ingest_spent_usd_micros: 0,
        qid_spent_usd_micros: talaria_store::supervisor_subject_spend_usd_micros(
            &pool,
            qid,
            talaria_ai::QID_BUDGET_WINDOW_DAYS,
        )
        .await?,
    };
    let (plan, usage) = rank_plan(experience, &brief, live, ledger).await;
    let over_budget = usage
        .cost_usd_micros
        .is_none_or(|cost| cost > talaria_ai::MAX_COST_USD_MICROS);
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
        cost_usd_micros: i32::try_from(usage.cost_usd_micros.unwrap_or(0)).unwrap_or(i32::MAX),
        cost_estimated: usage.estimated || usage.cost_usd_micros.is_none(),
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
    let mut quality = serde_json::json!({
        "passages": 0, "attributable": 0, "candidates": 0, "admissible": 0,
        "rejected": 0, "duplicates": 0, "attachment_errors": 0, "documentation_kept": 0
    });
    if execute && !over_budget {
        let harvest = execute_tasks(
            &pool,
            experience.as_str(),
            qid,
            &brief.label,
            &plan.tasks.iter().map(|task| task.kind).collect::<Vec<_>>(),
            brief.birth_year,
            brief.death_year,
        )
        .await;
        quality = serde_json::json!({
            "passages": harvest.quality.passages,
            "attributable": harvest.quality.attributable,
            "candidates": harvest.quality.candidates,
            "admissible": harvest.quality.admissible,
            "rejected": harvest.quality.rejected,
            "duplicates": harvest.quality.duplicates,
            "attachment_errors": harvest.quality.attachment_errors,
            "documentation_kept": harvest.quality.documentation_kept
        });
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
    report["new_documents"] =
        serde_json::json!(document_total(&after).saturating_sub(document_total(&before)));
    report["quality"] = quality;
    report["live_status"] = serde_json::json!(usage.live_status);
    Ok(serde_json::to_string_pretty(&report)?)
}

struct ModelUsage {
    live_status: String,
    input_tokens: u32,
    output_tokens: u32,
    cost_usd_micros: Option<u64>,
    input_rate: Option<u64>,
    output_rate: Option<u64>,
    estimated: bool,
}

fn usage_from(
    live_status: &str,
    input: u32,
    output: u32,
    cost: Option<u64>,
    rate: Option<talaria_ai::ListedRate>,
    estimated: bool,
) -> ModelUsage {
    ModelUsage {
        live_status: live_status.into(),
        input_tokens: input,
        output_tokens: output,
        cost_usd_micros: cost,
        input_rate: rate.map(|rate| rate.input_usd_micros_per_million),
        output_rate: rate.map(|rate| rate.output_usd_micros_per_million),
        estimated,
    }
}

async fn rank_plan(
    experience: Experience,
    brief: &SubjectBrief,
    live: bool,
    ledger: talaria_ai::SpendLedger,
) -> (talaria_ai::AgentPlan, ModelUsage) {
    let mut plan = deterministic_plan(experience, brief);
    if !live {
        return (
            plan,
            usage_from("not_requested", 0, 0, Some(0), None, false),
        );
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
    let input_estimate = estimate_tokens(&prompt);
    if !within_token_budget(&prompt) {
        return (
            plan,
            usage_from("budget_refused", input_estimate, 0, Some(0), None, false),
        );
    }
    let model = crate::llm::model();
    match talaria_ai::authorize_model_call(&model, input_estimate, ledger) {
        talaria_ai::BudgetDecision::Refused { reason } => {
            let cost = if reason == "rate_unknown" {
                None
            } else {
                Some(0)
            };
            (
                plan,
                usage_from(
                    reason,
                    input_estimate,
                    0,
                    cost,
                    talaria_ai::listed_rate(&model),
                    false,
                ),
            )
        }
        talaria_ai::BudgetDecision::Allowed { max_output_tokens } => {
            match crate::llm::complete_supervisor_plan(&prompt, max_output_tokens).await {
                Ok(completion) => {
                    let price = price_usage(
                        &completion.model,
                        completion.input_tokens,
                        completion.output_tokens,
                    );
                    let (kinds, refused) = accept_model_tasks(experience, &completion.value);
                    plan.refused.extend(refused);
                    if !kinds.is_empty() {
                        plan.tasks = kinds
                            .into_iter()
                            .map(|kind| PlanTask {
                                key: format!("{}:{}", kind.as_str(), brief.qid),
                                kind,
                                priority: 2,
                                reason:
                                    "Kept from the model because it is an allowed read-only task."
                                        .into(),
                            })
                            .collect();
                        plan.provider = completion.provider;
                        plan.model = completion.model;
                    }
                    let status = if price.cost_usd_micros.is_none() {
                        "rate_unknown"
                    } else {
                        "model_constrained"
                    };
                    (
                        plan,
                        usage_from(
                            status,
                            completion.input_tokens,
                            completion.output_tokens,
                            price.cost_usd_micros,
                            price.rate,
                            completion.tokens_estimated,
                        ),
                    )
                }
                Err(error) => (
                    plan,
                    usage_from(&format!("fallback:{error}"), 0, 0, Some(0), None, false),
                ),
            }
        }
    }
}

fn brief_from_coverage(qid: &str, label: &str, coverage: &SubjectCoverage) -> SubjectBrief {
    SubjectBrief {
        qid: qid.to_string(),
        label: if label.trim().is_empty() {
            coverage.label.clone()
        } else {
            label.trim().to_string()
        },
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
