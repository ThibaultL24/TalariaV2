// crates/talaria-sources/src/plan.rs
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::budgets::IngestBudgets;
use crate::kinds::SourceKind;

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

pub const PLANNER_V1: &str = "plan_sources:v1";

/// Deterministic planner — no subject-specific hardcoding.
pub fn plan_sources(subject: &ResolvedSubject, budgets: IngestBudgets) -> SourcePlan {
    let mut sources = Vec::new();

    sources.push(PlannedSource {
        kind: SourceKind::Wikidata,
        reason: "identity, vital dates, structured statements, place coordinates".into(),
        priority: 10,
        max_documents: budgets.max_documents_per_source.min(20),
    });

    sources.push(PlannedSource {
        kind: SourceKind::Wikipedia,
        reason: "biographical prose, chronologies, linked event/place pages".into(),
        priority: 20,
        max_documents: budgets.max_documents_per_source,
    });

    sources.push(PlannedSource {
        kind: SourceKind::Wikisource,
        reason: "transcribed historical texts and correspondence".into(),
        priority: 40,
        max_documents: budgets.max_documents_per_source.min(15),
    });

    sources.push(PlannedSource {
        kind: SourceKind::WikimediaCommons,
        reason: "media captions and geotags (not events by themselves)".into(),
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

    if is_french {
        sources.push(PlannedSource {
            kind: SourceKind::Bnf,
            reason: "French authority records and alignments".into(),
            priority: 60,
            max_documents: 10,
        });
        sources.push(PlannedSource {
            kind: SourceKind::Gallica,
            reason: "digitized books, press, correspondence".into(),
            priority: 70,
            max_documents: 15,
        });
        sources.push(PlannedSource {
            kind: SourceKind::Persee,
            reason: "historiographic articles".into(),
            priority: 90,
            max_documents: 10,
        });
    }

    let military = subject.occupations.iter().any(|o| {
        let o = o.to_lowercase();
        o.contains("military") || o.contains("soldier") || o.contains("general") || o.contains("officer")
    });
    if military {
        // Linked Wikipedia battle/campaign pages are discovered via Wikipedia connector depth.
        sources.push(PlannedSource {
            kind: SourceKind::Wikipedia,
            reason: "military subject: expand battle/campaign/treaty linked pages".into(),
            priority: 25,
            max_documents: budgets.max_documents_per_source,
        });
    }

    sources.push(PlannedSource {
        kind: SourceKind::OpenLibrary,
        reason: "bibliographic works and editions".into(),
        priority: 80,
        max_documents: 10,
    });
    sources.push(PlannedSource {
        kind: SourceKind::InternetArchive,
        reason: "scans/OCR for published works".into(),
        priority: 85,
        max_documents: 10,
    });
    sources.push(PlannedSource {
        kind: SourceKind::Europeana,
        reason: "heritage objects and institutional provenance".into(),
        priority: 95,
        max_documents: 10,
    });

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
