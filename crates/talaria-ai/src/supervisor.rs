// crates/talaria-ai/src/supervisor.rs
//! Read-only supervision plans for Istoria and Poreia.
//! A plan may only name tasks the backend already allows. It never publishes
//! and never writes canonical events.

use serde_json::{json, Value};

pub const PROMPT_VERSION: &str = "supervisor:v2";
pub const MODE_READ_ONLY: &str = "read_only";

const MAX_TASKS: u32 = 8;
const MAX_DOCUMENTS: u32 = 40;
const MAX_TOKENS: u32 = 4_000;
pub const MAX_COST_USD_MICROS: u64 = 20_000;
pub const MAX_TASK_COST_USD_MICROS: u64 = 20_000;
pub const MAX_INGEST_COST_USD_MICROS: u64 = 100_000;
pub const MAX_QID_COST_USD_MICROS: u64 = 500_000;
/// Spend older than this window is not counted. The cap is not a lifetime ban.
pub const QID_BUDGET_WINDOW_DAYS: i64 = 30;
pub const QID_BUDGET_POLICY: &str = "rolling_30_days";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpendLedger {
    pub task_spent_usd_micros: u64,
    pub ingest_spent_usd_micros: u64,
    pub qid_spent_usd_micros: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BudgetDecision {
    Allowed { max_output_tokens: u32 },
    Refused { reason: &'static str },
}

/// The next call must fit in the per-call ceiling, the remaining task budget,
/// the remaining budget of this ingestion, and the remaining budget of the QID.
pub fn authorize_model_call(model: &str, input_tokens: u32, ledger: SpendLedger) -> BudgetDecision {
    let Some(rate) = listed_rate(model) else {
        return BudgetDecision::Refused {
            reason: "rate_unknown",
        };
    };
    let task_left = MAX_TASK_COST_USD_MICROS.saturating_sub(ledger.task_spent_usd_micros);
    let ingest_left = MAX_INGEST_COST_USD_MICROS.saturating_sub(ledger.ingest_spent_usd_micros);
    let qid_left = MAX_QID_COST_USD_MICROS.saturating_sub(ledger.qid_spent_usd_micros);
    let remaining = MAX_COST_USD_MICROS
        .min(task_left)
        .min(ingest_left)
        .min(qid_left);
    let input_cost = (u64::from(input_tokens) * rate.input_usd_micros_per_million) / 1_000_000;
    if input_cost >= remaining {
        return BudgetDecision::Refused {
            reason: "budget_exhausted",
        };
    }
    let Some(ceiling) = output_token_ceiling(model, input_tokens, remaining) else {
        return BudgetDecision::Refused {
            reason: "rate_unknown",
        };
    };
    if ceiling == 0 {
        return BudgetDecision::Refused {
            reason: "budget_exhausted",
        };
    }
    BudgetDecision::Allowed {
        max_output_tokens: ceiling,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Experience {
    Istoria,
    Poreia,
}

impl Experience {
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "istoria" => Some(Self::Istoria),
            "poreia" => Some(Self::Poreia),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Istoria => "istoria",
            Self::Poreia => "poreia",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskKind {
    ResolveWikidata,
    HarvestWikipedia,
    HarvestWikisource,
    HarvestCommons,
    HarvestWdqsBiography,
    RecordDocumentaryGap,
    ReviewHistoricalConsistency,
    DiscoverHeritageSites,
    ReviewPersonSiteRelation,
    CollectCommonsMedia,
    CollectCulturalOccurrences,
    FlagUnverifiedVisitFacts,
}

impl TaskKind {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "resolve_wikidata" => Some(Self::ResolveWikidata),
            "harvest_wikipedia" => Some(Self::HarvestWikipedia),
            "harvest_wikisource" => Some(Self::HarvestWikisource),
            "harvest_commons" => Some(Self::HarvestCommons),
            "harvest_wdqs_biography" => Some(Self::HarvestWdqsBiography),
            "record_documentary_gap" => Some(Self::RecordDocumentaryGap),
            "review_historical_consistency" => Some(Self::ReviewHistoricalConsistency),
            "discover_heritage_sites" => Some(Self::DiscoverHeritageSites),
            "review_person_site_relation" => Some(Self::ReviewPersonSiteRelation),
            "collect_commons_media" => Some(Self::CollectCommonsMedia),
            "collect_cultural_occurrences" => Some(Self::CollectCulturalOccurrences),
            "flag_unverified_visit_facts" => Some(Self::FlagUnverifiedVisitFacts),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::ResolveWikidata => "resolve_wikidata",
            Self::HarvestWikipedia => "harvest_wikipedia",
            Self::HarvestWikisource => "harvest_wikisource",
            Self::HarvestCommons => "harvest_commons",
            Self::HarvestWdqsBiography => "harvest_wdqs_biography",
            Self::RecordDocumentaryGap => "record_documentary_gap",
            Self::ReviewHistoricalConsistency => "review_historical_consistency",
            Self::DiscoverHeritageSites => "discover_heritage_sites",
            Self::ReviewPersonSiteRelation => "review_person_site_relation",
            Self::CollectCommonsMedia => "collect_commons_media",
            Self::CollectCulturalOccurrences => "collect_cultural_occurrences",
            Self::FlagUnverifiedVisitFacts => "flag_unverified_visit_facts",
        }
    }

    pub fn allowed_for(self, experience: Experience) -> bool {
        match experience {
            Experience::Istoria => matches!(
                self,
                Self::ResolveWikidata
                    | Self::HarvestWikipedia
                    | Self::HarvestWikisource
                    | Self::HarvestCommons
                    | Self::HarvestWdqsBiography
                    | Self::RecordDocumentaryGap
                    | Self::ReviewHistoricalConsistency
            ),
            Experience::Poreia => matches!(
                self,
                Self::ResolveWikidata
                    | Self::DiscoverHeritageSites
                    | Self::ReviewPersonSiteRelation
                    | Self::CollectCommonsMedia
                    | Self::CollectCulturalOccurrences
                    | Self::FlagUnverifiedVisitFacts
                    | Self::RecordDocumentaryGap
            ),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SubjectBrief {
    pub qid: String,
    pub label: String,
    pub birth_year: Option<i32>,
    pub death_year: Option<i32>,
    pub has_wikipedia: bool,
    pub wikipedia_documents: u32,
    pub wikisource_count: u32,
    pub commons_documents: u32,
    pub sourced_event_count: u32,
    pub published_site_count: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PlanTask {
    pub key: String,
    pub kind: TaskKind,
    pub priority: u8,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AgentPlan {
    pub experience: Experience,
    pub mode: &'static str,
    pub subject_qid: String,
    pub subject_label: String,
    pub provider: String,
    pub model: String,
    pub prompt_version: &'static str,
    pub tasks: Vec<PlanTask>,
    pub gaps: Vec<String>,
    pub refused: Vec<String>,
}

pub fn valid_qid(qid: &str) -> bool {
    let Some(rest) = qid.strip_prefix('Q') else {
        return false;
    };
    !rest.is_empty() && !rest.starts_with('0') && rest.bytes().all(|b| b.is_ascii_digit())
}

pub fn deterministic_plan(experience: Experience, brief: &SubjectBrief) -> AgentPlan {
    let mut gaps = Vec::new();
    let mut tasks = Vec::new();
    let push = |tasks: &mut Vec<PlanTask>, kind: TaskKind, priority: u8, reason: &str| {
        if tasks.len() as u32 >= MAX_TASKS {
            return;
        }
        tasks.push(PlanTask {
            key: format!("{}:{}", kind.as_str(), brief.qid),
            kind,
            priority,
            reason: reason.to_string(),
        });
    };
    push(
        &mut tasks,
        TaskKind::ResolveWikidata,
        1,
        "Confirm the QID before any homonym is accepted.",
    );
    match experience {
        Experience::Istoria => {
            if !brief.has_wikipedia && brief.wikipedia_documents == 0 {
                gaps.push("wikipedia_page_missing".into());
            }
            if brief.wikipedia_documents == 0 {
                push(
                    &mut tasks,
                    TaskKind::HarvestWikipedia,
                    1,
                    "Read the associated Wikipedia pages before prose extraction.",
                );
            }
            if brief.wikisource_count == 0 {
                gaps.push("wikisource_corpus_not_collected".into());
                push(
                    &mut tasks,
                    TaskKind::HarvestWikisource,
                    2,
                    "Page Wikisource inside the forty-document budget.",
                );
            }
            if brief.commons_documents == 0 {
                push(
                    &mut tasks,
                    TaskKind::HarvestCommons,
                    3,
                    "Collect Commons files only with author, license and attribution.",
                );
            }
            if brief.sourced_event_count == 0 {
                gaps.push("biography_without_sourced_events".into());
                push(
                    &mut tasks,
                    TaskKind::HarvestWdqsBiography,
                    1,
                    "Prefer dated, typed Wikidata statements over a generic historical fact.",
                );
            }
            if brief.birth_year.is_none() || brief.death_year.is_none() {
                gaps.push("lifespan_incomplete".into());
                push(
                    &mut tasks,
                    TaskKind::RecordDocumentaryGap,
                    2,
                    "Do not invent a missing birth or death year.",
                );
            }
            if brief.birth_year.is_some_and(|year| year < 0) {
                gaps.push("signed_year_must_stay_bce".into());
                push(
                    &mut tasks,
                    TaskKind::RecordDocumentaryGap,
                    1,
                    "Keep a negative year negative. Do not move it into the common era.",
                );
            }
        }
        Experience::Poreia => {
            if brief.published_site_count == 0 {
                gaps.push("no_published_site".into());
                push(
                    &mut tasks,
                    TaskKind::DiscoverHeritageSites,
                    1,
                    "Sites start in quarantine. Publication stays with the operator review.",
                );
            }
            push(
                &mut tasks,
                TaskKind::ReviewPersonSiteRelation,
                1,
                "A person-site link needs a source. A place named in a text is not a visit.",
            );
            push(
                &mut tasks,
                TaskKind::CollectCommonsMedia,
                2,
                "A photograph stays quarantined until author, license and attribution are present.",
            );
            push(
                &mut tasks,
                TaskKind::CollectCulturalOccurrences,
                2,
                "Exhibitions and performances stay separate from the historical timeline.",
            );
            push(&mut tasks, TaskKind::FlagUnverifiedVisitFacts, 1, "Hours, duration, price, access and open-now stay unverified unless a source records them.");
        }
    }
    AgentPlan {
        experience,
        mode: MODE_READ_ONLY,
        subject_qid: brief.qid.clone(),
        subject_label: brief.label.clone(),
        provider: "deterministic".into(),
        model: "none".into(),
        prompt_version: PROMPT_VERSION,
        tasks,
        gaps,
        refused: vec!["publish_site".into(), "write_canonical_event".into()],
    }
}

pub fn plan_json(plan: &AgentPlan) -> Value {
    json!({
        "experience": plan.experience.as_str(),
        "mode": plan.mode,
        "subject_qid": plan.subject_qid,
        "subject_label": plan.subject_label,
        "provider": plan.provider,
        "model": plan.model,
        "prompt_version": plan.prompt_version,
        "budget": {
            "max_tasks": MAX_TASKS,
            "max_documents": MAX_DOCUMENTS,
            "max_tokens": MAX_TOKENS,
            "max_output_tokens": output_token_ceiling("gpt-5.4", 0, MAX_COST_USD_MICROS).unwrap_or(0),
            "max_cost_usd_micros": MAX_COST_USD_MICROS,
            "max_task_cost_usd_micros": MAX_TASK_COST_USD_MICROS,
            "max_ingest_cost_usd_micros": MAX_INGEST_COST_USD_MICROS,
            "max_qid_cost_usd_micros": MAX_QID_COST_USD_MICROS,
            "qid_budget_window_days": QID_BUDGET_WINDOW_DAYS,
            "qid_budget_policy": QID_BUDGET_POLICY,
            "qid_budget_resets": "Spend older than the window is ignored. The cap is not a lifetime ban."
        },
        "tasks": plan.tasks.iter().map(|task| json!({
            "key": task.key,
            "kind": task.kind.as_str(),
            "priority": task.priority,
            "reason": task.reason,
            "writes": false,
            "publication": "forbidden"
        })).collect::<Vec<_>>(),
        "gaps": plan.gaps,
        "refused": plan.refused
    })
}

/// Keep model output inside the allow-list. Unknown, foreign, or mutating tasks are refused.
pub fn accept_model_tasks(
    experience: Experience,
    proposed: &Value,
) -> (Vec<TaskKind>, Vec<String>) {
    let mut kept = Vec::new();
    let mut refused = Vec::new();
    let Some(items) = proposed.get("tasks").and_then(Value::as_array) else {
        refused.push("model_tasks_missing".into());
        return (kept, refused);
    };
    for item in items {
        let kind = item.get("kind").and_then(Value::as_str).unwrap_or("");
        let writes = item.get("writes").and_then(Value::as_bool).unwrap_or(false);
        let publication = item
            .get("publication")
            .and_then(Value::as_str)
            .unwrap_or("forbidden");
        if writes || publication != "forbidden" {
            refused.push(format!("mutation:{kind}"));
            continue;
        }
        match TaskKind::parse(kind) {
            Some(task) if task.allowed_for(experience) && kept.len() < MAX_TASKS as usize => {
                if !kept.contains(&task) {
                    kept.push(task);
                }
            }
            Some(_) => refused.push(format!("wrong_experience:{kind}")),
            None => refused.push(format!("unknown:{kind}")),
        }
    }
    (kept, refused)
}

pub fn quote_is_documentary(quote: &str, source_text: &str) -> bool {
    let quote = quote.trim();
    !quote.is_empty()
        && source_text.contains(quote)
        && !quote.to_ascii_lowercase().contains("[synthetic]")
        && !quote.to_ascii_lowercase().starts_with("summary:")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ListedRate {
    pub input_usd_micros_per_million: u64,
    pub output_usd_micros_per_million: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UsagePrice {
    pub cost_usd_micros: Option<u64>,
    pub rate: Option<ListedRate>,
}

/// Official standard rates, in microdollars per million tokens.
/// An unlisted model has no price: a fallback rate is not reported as the cost.
pub fn listed_rate(model: &str) -> Option<ListedRate> {
    let (input, output) = match model {
        "gpt-5.4" | "gpt-5.4-2026-03-05" => (2_500_000, 15_000_000),
        "gpt-4o" => (2_500_000, 10_000_000),
        "gpt-4o-mini" => (150_000, 600_000),
        _ => return None,
    };
    Some(ListedRate {
        input_usd_micros_per_million: input,
        output_usd_micros_per_million: output,
    })
}

pub fn price_usage(model: &str, input_tokens: u32, output_tokens: u32) -> UsagePrice {
    if input_tokens == 0 && output_tokens == 0 {
        return UsagePrice {
            cost_usd_micros: Some(0),
            rate: listed_rate(model),
        };
    }
    let Some(rate) = listed_rate(model) else {
        return UsagePrice {
            cost_usd_micros: None,
            rate: None,
        };
    };
    let cost = u64::from(input_tokens) * rate.input_usd_micros_per_million
        + u64::from(output_tokens) * rate.output_usd_micros_per_million;
    UsagePrice {
        cost_usd_micros: Some((cost + 500_000) / 1_000_000),
        rate: Some(rate),
    }
}

/// Largest output the listed rate can produce without crossing the budget.
/// `None` means the model has no listed rate, so the call must not be made.
pub fn output_token_ceiling(model: &str, input_tokens: u32, budget_usd_micros: u64) -> Option<u32> {
    let rate = listed_rate(model)?;
    let input_cost = price_usage(model, input_tokens, 0).cost_usd_micros?;
    if input_cost >= budget_usd_micros {
        return Some(0);
    }
    let per_token = rate.output_usd_micros_per_million / 1_000_000;
    if per_token == 0 {
        return Some(0);
    }
    let room = (budget_usd_micros - input_cost) / per_token;
    Some(u32::try_from(room).unwrap_or(u32::MAX).min(MAX_TOKENS))
}

pub fn estimate_tokens(text: &str) -> u32 {
    u32::try_from(text.chars().count() / 4)
        .unwrap_or(u32::MAX)
        .max(1)
}

pub fn within_token_budget(prompt: &str) -> bool {
    estimate_tokens(prompt) <= MAX_TOKENS
}

/// Known subjects used by unit tests. Live planning reads the database instead.
pub fn reference_brief(qid: &str) -> Option<SubjectBrief> {
    match qid {
        "Q517" => Some(SubjectBrief {
            qid: qid.into(),
            label: "Napoléon Bonaparte".into(),
            birth_year: Some(1769),
            death_year: Some(1821),
            has_wikipedia: true,
            wikipedia_documents: 0,
            wikisource_count: 0,
            commons_documents: 0,
            sourced_event_count: 0,
            published_site_count: 0,
        }),
        "Q5582" => Some(SubjectBrief {
            qid: qid.into(),
            label: "Vincent van Gogh".into(),
            birth_year: Some(1853),
            death_year: Some(1890),
            has_wikipedia: true,
            wikipedia_documents: 0,
            wikisource_count: 0,
            commons_documents: 0,
            sourced_event_count: 0,
            published_site_count: 0,
        }),
        "Q1048" => Some(SubjectBrief {
            qid: qid.into(),
            label: "Jules César".into(),
            birth_year: Some(-100),
            death_year: Some(-44),
            has_wikipedia: true,
            wikipedia_documents: 0,
            wikisource_count: 0,
            commons_documents: 0,
            sourced_event_count: 0,
            published_site_count: 0,
        }),
        _ => None,
    }
}

pub fn policy_comparison(experience: Experience, qid: &str) -> Value {
    let Some(brief) = reference_brief(qid) else {
        return json!({"error": "unknown_reference_subject", "qid": qid});
    };
    let plan = deterministic_plan(experience, &brief);
    let without = match experience {
        Experience::Istoria => json!({
            "homonym": "fail closed when the top hits tie or are not a person",
            "wikisource_documents": "paged with a budget of 40",
            "typed_events": "a precise label replaces historical_fact",
            "bce_dates": "the sign stays on the year",
            "place_mention": "a generic fact does not count as presence",
            "quote": "a synthetic summary is rejected",
            "canonical_write": false
        }),
        Experience::Poreia => json!({
            "sites": "quarantine only, heritage enrichment stays inactive",
            "occurrences": "kept off the historical timeline",
            "hours": "flagged unverified",
            "person_site": "a named place is not a visit",
            "canonical_write": false
        }),
    };
    json!({
        "live_harvest": false,
        "cost_usd_micros": 0,
        "note": "Offline policy diff. No document was fetched and no model was called.",
        "subject_qid": qid,
        "without_supervision": without,
        "with_supervision": plan_json(&plan)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn istoria_plan_refuses_publication_and_keeps_bce() {
        let brief = reference_brief("Q1048").unwrap();
        let plan = deterministic_plan(Experience::Istoria, &brief);
        let body = plan_json(&plan);
        assert_eq!(body["mode"], "read_only");
        assert!(body["refused"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v == "write_canonical_event"));
        assert!(plan
            .gaps
            .iter()
            .any(|gap| gap == "signed_year_must_stay_bce"));
        assert!(plan
            .tasks
            .iter()
            .all(|task| task.kind.allowed_for(Experience::Istoria)));
    }

    #[test]
    fn poreia_plan_separates_occurrences_and_forbids_assumed_hours() {
        let brief = reference_brief("Q5582").unwrap();
        let plan = deterministic_plan(Experience::Poreia, &brief);
        assert!(plan
            .tasks
            .iter()
            .any(|task| task.kind == TaskKind::CollectCulturalOccurrences));
        assert!(plan
            .tasks
            .iter()
            .any(|task| task.kind == TaskKind::FlagUnverifiedVisitFacts));
        assert!(plan
            .tasks
            .iter()
            .all(|task| task.kind != TaskKind::HarvestWdqsBiography));
    }

    #[test]
    fn model_cannot_add_a_write_or_a_foreign_task() {
        let proposed = json!({"tasks": [
            {"kind": "discover_heritage_sites", "writes": false, "publication": "forbidden"},
            {"kind": "publish_site", "writes": true, "publication": "publish"},
            {"kind": "harvest_wdqs_biography", "writes": false, "publication": "forbidden"}
        ]});
        let (kept, refused) = accept_model_tasks(Experience::Poreia, &proposed);
        assert_eq!(kept, vec![TaskKind::DiscoverHeritageSites]);
        assert!(refused.iter().any(|item| item.contains("publish_site")));
        assert!(refused
            .iter()
            .any(|item| item.contains("harvest_wdqs_biography")));
    }

    #[test]
    fn a_synthetic_summary_is_not_a_documentary_extract() {
        assert!(quote_is_documentary(
            "born in Ajaccio",
            "He was born in Ajaccio."
        ));
        assert!(!quote_is_documentary(
            "[synthetic] born in Ajaccio",
            "He was born in Ajaccio."
        ));
        assert!(!quote_is_documentary(
            "born in Paris",
            "He was born in Ajaccio."
        ));
    }

    #[test]
    fn a_collected_corpus_is_not_fetched_again() {
        let mut brief = reference_brief("Q517").unwrap();
        brief.wikipedia_documents = 2;
        brief.wikisource_count = 3;
        brief.commons_documents = 1;
        brief.sourced_event_count = 4;
        brief.birth_year = Some(1769);
        brief.death_year = Some(1821);
        let plan = deterministic_plan(Experience::Istoria, &brief);
        assert!(plan
            .tasks
            .iter()
            .all(|task| task.kind == TaskKind::ResolveWikidata));
    }

    #[test]
    fn a_billed_call_cannot_be_priced_at_zero() {
        assert_eq!(price_usage("gpt-5.4", 0, 0).cost_usd_micros, Some(0));
        assert_eq!(price_usage("gpt-5.4", 31, 25).cost_usd_micros, Some(453));
        assert!(price_usage("unlisted-model", 31, 25)
            .cost_usd_micros
            .is_none());
        let ceiling = output_token_ceiling("gpt-5.4", 31, MAX_COST_USD_MICROS).unwrap();
        assert!(ceiling < MAX_TOKENS);
        assert!(
            price_usage("gpt-5.4", 31, ceiling).cost_usd_micros.unwrap() <= MAX_COST_USD_MICROS
        );
    }

    #[test]
    fn a_task_and_an_ingestion_share_a_cumulative_budget() {
        let fresh = authorize_model_call(
            "gpt-5.4",
            31,
            SpendLedger {
                task_spent_usd_micros: 0,
                ingest_spent_usd_micros: 0,
                qid_spent_usd_micros: 0,
            },
        );
        assert!(
            matches!(fresh, BudgetDecision::Allowed { max_output_tokens } if max_output_tokens > 0)
        );
        let task_spent = authorize_model_call(
            "gpt-5.4",
            31,
            SpendLedger {
                task_spent_usd_micros: MAX_TASK_COST_USD_MICROS,
                ingest_spent_usd_micros: 0,
                qid_spent_usd_micros: 0,
            },
        );
        assert_eq!(
            task_spent,
            BudgetDecision::Refused {
                reason: "budget_exhausted"
            }
        );
        let ingest_spent = authorize_model_call(
            "gpt-5.4",
            31,
            SpendLedger {
                task_spent_usd_micros: 0,
                ingest_spent_usd_micros: MAX_INGEST_COST_USD_MICROS,
                qid_spent_usd_micros: 0,
            },
        );
        assert_eq!(
            ingest_spent,
            BudgetDecision::Refused {
                reason: "budget_exhausted"
            }
        );
        let qid_spent = authorize_model_call(
            "gpt-5.4",
            31,
            SpendLedger {
                task_spent_usd_micros: 0,
                ingest_spent_usd_micros: 0,
                qid_spent_usd_micros: MAX_QID_COST_USD_MICROS,
            },
        );
        assert_eq!(
            qid_spent,
            BudgetDecision::Refused {
                reason: "budget_exhausted"
            }
        );
        assert_eq!(
            authorize_model_call(
                "unlisted-model",
                31,
                SpendLedger {
                    task_spent_usd_micros: 0,
                    ingest_spent_usd_micros: 0,
                    qid_spent_usd_micros: 0
                }
            ),
            BudgetDecision::Refused {
                reason: "rate_unknown"
            }
        );
        assert!(TaskKind::ReviewHistoricalConsistency.allowed_for(Experience::Istoria));
        assert!(!TaskKind::ReviewHistoricalConsistency.allowed_for(Experience::Poreia));
    }

    #[test]
    fn napoleon_comparison_stays_offline() {
        let body = policy_comparison(Experience::Istoria, "Q517");
        assert_eq!(body["live_harvest"], false);
        assert_eq!(body["cost_usd_micros"], 0);
        assert_eq!(body["with_supervision"]["mode"], "read_only");
    }
}
