// crates/talaria-sources/src/plan.rs
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::budgets::IngestBudgets;
use crate::kinds::SourceKind;
use crate::person_profile::{infer_person_class, profile_for, PersonClass};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolvedSubject {
    pub entity_id: Option<Uuid>,
    pub qid: Option<String>,
    pub label: String,
    pub languages: Vec<String>,
    pub birth_year: Option<i32>,
    pub death_year: Option<i32>,
    pub countries: Vec<String>,
    pub occupations: Vec<String>,
    pub known_identifiers: Vec<(String, String)>,
}

impl ResolvedSubject {
    pub fn person_class(&self) -> PersonClass {
        infer_person_class(&self.occupations, None)
    }

    pub fn catalog_query(&self, kind: SourceKind) -> String {
        crate::catalog_search_query(&self.label, &profile_for(self.person_class()), kind)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlannedSource {
    pub kind: SourceKind,
    pub reason: String,
    pub priority: u32,
    pub max_documents: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourcePlan {
    pub subject: ResolvedSubject,
    pub sources: Vec<PlannedSource>,
    pub budgets: IngestBudgets,
    pub planner_version: String,
}

pub const PLANNER_V1: &str = "plan_sources:v2_person_class";

/// Deterministic planner keyed by person class (POC presets), not a single military net.
#[allow(clippy::vec_init_then_push)]
pub fn plan_sources(subject: &ResolvedSubject, budgets: IngestBudgets) -> SourcePlan {
    let class = subject.person_class();
    let profile = profile_for(class);
    let mut sources = Vec::new();

    sources.push(PlannedSource {
        kind: SourceKind::Wikidata,
        reason: format!(
            "identity + structured statements for {}",
            class.as_str()
        ),
        priority: 10,
        max_documents: budgets.max_documents_per_source.min(20),
    });

    sources.push(PlannedSource {
        kind: SourceKind::Wikipedia,
        reason: format!("biography pages ranked for {}", class.as_str()),
        priority: 20,
        max_documents: budgets.max_documents_per_source,
    });

    sources.push(PlannedSource {
        kind: SourceKind::Wikisource,
        reason: format!("transcriptions / correspondence ({})", class.as_str()),
        priority: 40,
        max_documents: budgets.max_documents_per_source.min(15),
    });

    sources.push(PlannedSource {
        kind: SourceKind::WikimediaCommons,
        reason: format!("media captions/geotags ({})", class.as_str()),
        priority: 50,
        max_documents: budgets.max_documents_per_source.min(10),
    });

    let is_french = subject
        .languages
        .iter()
        .any(|l| l == "fr" || l.starts_with("fr"))
        || subject.countries.iter().any(|c| {
            let c = c.to_lowercase();
            c.contains("france") || c.contains("french")
        });
    let scholarly = matches!(
        class,
        PersonClass::Scientist
            | PersonClass::InventorEngineer
            | PersonClass::Philosopher
            | PersonClass::ArtistWriter
    );

    sources.push(PlannedSource {
        kind: SourceKind::OpenAlex,
        reason: format!("scholarly works query for {}", class.as_str()),
        priority: if scholarly { 55 } else { 62 },
        max_documents: budgets.max_documents_per_source.min(25),
    });

    sources.push(PlannedSource {
        kind: SourceKind::Bnf,
        reason: format!("BnF catalogue ({})", class.as_str()),
        priority: 60,
        max_documents: 15,
    });
    sources.push(PlannedSource {
        kind: SourceKind::Europeana,
        reason: format!("Europeana heritage ({})", class.as_str()),
        priority: 72,
        max_documents: 15,
    });
    sources.push(PlannedSource {
        kind: SourceKind::InternetArchive,
        reason: format!("Internet Archive texts ({})", class.as_str()),
        priority: 85,
        max_documents: 10,
    });
    sources.push(PlannedSource {
        kind: SourceKind::OpenLibrary,
        reason: format!("bibliographic editions ({})", class.as_str()),
        priority: 80,
        max_documents: 10,
    });

    if is_french || scholarly {
        sources.push(PlannedSource {
            kind: SourceKind::Gallica,
            reason: "digitized books, press, correspondence".into(),
            priority: 70,
            max_documents: 15,
        });
        sources.push(PlannedSource {
            kind: SourceKind::ThesesFr,
            reason: format!("French theses about {}", class.as_str()),
            priority: 65,
            max_documents: budgets.max_documents_per_source.min(25),
        });
        sources.push(PlannedSource {
            kind: SourceKind::Hal,
            reason: "French open archive scholarly works".into(),
            priority: 68,
            max_documents: 15,
        });
        sources.push(PlannedSource {
            kind: SourceKind::Persee,
            reason: "historiographic articles".into(),
            priority: 90,
            max_documents: 10,
        });
    }

    if profile.enable_military_extractor {
        sources.push(PlannedSource {
            kind: SourceKind::Wikipedia,
            reason: "military subject: expand battle/campaign/treaty linked pages".into(),
            priority: 25,
            max_documents: budgets.max_documents_per_source,
        });
    }

    // Deduplicate by kind keeping highest priority (lowest number).
    sources.sort_by_key(|s| s.priority);
    let mut seen = std::collections::HashSet::new();
    sources.retain(|s| seen.insert(s.kind.as_str().to_string()));

    SourcePlan {
        subject: subject.clone(),
        sources,
        budgets,
        planner_version: PLANNER_V1.into(),
    }
}
