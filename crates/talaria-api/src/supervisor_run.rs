// crates/talaria-api/src/supervisor_run.rs
//! CLI entry for read-only supervision. The model may only reorder allowed tasks.

use talaria_ai::{
    accept_model_tasks, deterministic_plan, plan_json, policy_comparison, reference_brief,
    valid_qid, Experience, PlanTask, SubjectBrief, PROMPT_VERSION,
};

pub async fn agent_plan(
    experience: &str,
    qid: &str,
    label: &str,
    live: bool,
) -> anyhow::Result<String> {
    let experience = Experience::parse(experience).ok_or_else(|| anyhow::anyhow!("invalid_experience"))?;
    anyhow::ensure!(valid_qid(qid), "invalid_qid");
    let brief = brief_for(qid, label);
    let mut plan = deterministic_plan(experience, &brief);
    let mut live_status = "not_requested".to_string();
    if live {
        let prompt = format!(
            "Return one JSON object with a tasks array. Each task has kind, writes=false, publication=forbidden. \
             Allowed Istoria kinds: resolve_wikidata, harvest_wikipedia, harvest_wikisource, harvest_commons, harvest_wdqs_biography, record_documentary_gap. \
             Allowed Poreia kinds: resolve_wikidata, discover_heritage_sites, review_person_site_relation, collect_commons_media, collect_cultural_occurrences, flag_unverified_visit_facts, record_documentary_gap. \
             Never publish and never write a canonical event. Subject {} {} experience {}.",
            brief.qid,
            brief.label,
            experience.as_str()
        );
        match crate::llm::complete_supervisor_plan(&prompt).await {
            Ok((provider, model, value)) => {
                let (kinds, refused) = accept_model_tasks(experience, &value);
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
                    plan.provider = provider;
                    plan.model = model;
                }
                live_status = "model_constrained".into();
            }
            Err(error) => {
                live_status = format!("fallback:{error}");
            }
        }
    }
    let mut body = plan_json(&plan);
    body["live_status"] = serde_json::json!(live_status);
    body["prompt_version"] = serde_json::json!(PROMPT_VERSION);
    if reference_brief(qid).is_some() {
        body["comparison"] = policy_comparison(experience, qid);
    }
    Ok(serde_json::to_string_pretty(&body)?)
}

fn brief_for(qid: &str, label: &str) -> SubjectBrief {
    let mut brief = reference_brief(qid).unwrap_or(SubjectBrief {
        qid: qid.to_string(),
        label: qid.to_string(),
        birth_year: None,
        death_year: None,
        has_wikipedia: false,
        wikisource_count: 0,
        sourced_event_count: 0,
        published_site_count: 0,
    });
    if !label.trim().is_empty() {
        brief.label = label.trim().to_string();
    }
    brief
}
