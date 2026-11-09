// crates/talaria-ai/src/supervisor.rs
//! Read-only supervision plans for Istoria and Poreia.
//! A plan may only name tasks the backend already allows. It never publishes
//! and never writes canonical events.

use serde_json::{json, Value};

pub const PROMPT_VERSION: &str = "supervisor:v1";
pub const MODE_READ_ONLY: &str = "read_only";

const MAX_TASKS: u32 = 8;
const MAX_DOCUMENTS: u32 = 40;
const MAX_TOKENS: u32 = 4_000;

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
    pub wikisource_count: u32,
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
    push(&mut tasks, TaskKind::ResolveWikidata, 1, "Confirm the QID before any homonym is accepted.");
    match experience {
        Experience::Istoria => {
            if !brief.has_wikipedia {
                gaps.push("wikipedia_page_missing".into());
            }
            push(&mut tasks, TaskKind::HarvestWikipedia, 1, "Read the associated Wikipedia pages before prose extraction.");
            if brief.wikisource_count == 0 {
                gaps.push("wikisource_corpus_not_collected".into());
            }
            push(&mut tasks, TaskKind::HarvestWikisource, 2, "Page Wikisource inside the forty-document budget.");
            push(&mut tasks, TaskKind::HarvestCommons, 3, "Collect Commons files only with author, license and attribution.");
            if brief.sourced_event_count == 0 {
                gaps.push("biography_without_sourced_events".into());
            }
            push(&mut tasks, TaskKind::HarvestWdqsBiography, 1, "Prefer dated, typed Wikidata statements over a generic historical fact.");
            if brief.birth_year.is_none() || brief.death_year.is_none() {
                gaps.push("lifespan_incomplete".into());
                push(&mut tasks, TaskKind::RecordDocumentaryGap, 2, "Do not invent a missing birth or death year.");
            }
            if brief.birth_year.is_some_and(|year| year < 0) {
                gaps.push("signed_year_must_stay_bce".into());
                push(&mut tasks, TaskKind::RecordDocumentaryGap, 1, "Keep a negative year negative. Do not move it into the common era.");
            }
        }
        Experience::Poreia => {
            if brief.published_site_count == 0 {
                gaps.push("no_published_site".into());
            }
            push(&mut tasks, TaskKind::DiscoverHeritageSites, 1, "Sites start in quarantine. Publication stays with the operator review.");
            push(&mut tasks, TaskKind::ReviewPersonSiteRelation, 1, "A person-site link needs a source. A place named in a text is not a visit.");
            push(&mut tasks, TaskKind::CollectCommonsMedia, 2, "A photograph stays quarantined until author, license and attribution are present.");
            push(&mut tasks, TaskKind::CollectCulturalOccurrences, 2, "Exhibitions and performances stay separate from the historical timeline.");
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
        refused: vec![
            "publish_site".into(),
            "write_canonical_event".into(),
        ],
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
            "max_cost_usd_micros": 0
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
pub fn accept_model_tasks(experience: Experience, proposed: &Value) -> (Vec<TaskKind>, Vec<String>) {
    let mut kept = Vec::new();
    let mut refused = Vec::new();
    let Some(items) = proposed.get("tasks").and_then(Value::as_array) else {
        refused.push("model_tasks_missing".into());
        return (kept, refused);
    };
    for item in items {
        let kind = item.get("kind").and_then(Value::as_str).unwrap_or("");
        let writes = item.get("writes").and_then(Value::as_bool).unwrap_or(false);
        let publication = item.get("publication").and_then(Value::as_str).unwrap_or("forbidden");
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

/// Known subjects used to compare policy with and without a model. No network and no invented facts.
pub fn reference_brief(qid: &str) -> Option<SubjectBrief> {
    match qid {
        "Q517" => Some(SubjectBrief {
            qid: qid.into(),
            label: "Napoléon Bonaparte".into(),
            birth_year: Some(1769),
            death_year: Some(1821),
            has_wikipedia: true,
            wikisource_count: 0,
            sourced_event_count: 0,
            published_site_count: 0,
        }),
        "Q5582" => Some(SubjectBrief {
            qid: qid.into(),
            label: "Vincent van Gogh".into(),
            birth_year: Some(1853),
            death_year: Some(1890),
            has_wikipedia: true,
            wikisource_count: 0,
            sourced_event_count: 0,
            published_site_count: 0,
        }),
        "Q1048" => Some(SubjectBrief {
            qid: qid.into(),
            label: "Jules César".into(),
            birth_year: Some(-100),
            death_year: Some(-44),
            has_wikipedia: true,
            wikisource_count: 0,
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
        assert!(body["refused"].as_array().unwrap().iter().any(|v| v == "write_canonical_event"));
        assert!(plan.gaps.iter().any(|gap| gap == "signed_year_must_stay_bce"));
        assert!(plan.tasks.iter().all(|task| task.kind.allowed_for(Experience::Istoria)));
    }

    #[test]
    fn poreia_plan_separates_occurrences_and_forbids_assumed_hours() {
        let brief = reference_brief("Q5582").unwrap();
        let plan = deterministic_plan(Experience::Poreia, &brief);
        assert!(plan.tasks.iter().any(|task| task.kind == TaskKind::CollectCulturalOccurrences));
        assert!(plan.tasks.iter().any(|task| task.kind == TaskKind::FlagUnverifiedVisitFacts));
        assert!(plan.tasks.iter().all(|task| task.kind != TaskKind::HarvestWdqsBiography));
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
        assert!(refused.iter().any(|item| item.contains("harvest_wdqs_biography")));
    }

    #[test]
    fn a_synthetic_summary_is_not_a_documentary_extract() {
        assert!(quote_is_documentary("born in Ajaccio", "He was born in Ajaccio."));
        assert!(!quote_is_documentary("[synthetic] born in Ajaccio", "He was born in Ajaccio."));
        assert!(!quote_is_documentary("born in Paris", "He was born in Ajaccio."));
    }

    #[test]
    fn napoleon_comparison_stays_offline() {
        let body = policy_comparison(Experience::Istoria, "Q517");
        assert_eq!(body["live_harvest"], false);
        assert_eq!(body["cost_usd_micros"], 0);
        assert_eq!(body["with_supervision"]["mode"], "read_only");
    }
}
