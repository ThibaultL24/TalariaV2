// crates/talaria-sources/src/person_profile.rs
//! Person-class ingest profiles (POC presets) — which pages and catalogs to search.

use serde::{Deserialize, Serialize};

use crate::kinds::SourceKind;
use crate::matching::subject_match_aliases;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PersonClass {
    Scientist,
    InventorEngineer,
    ArtistWriter,
    ArtistVisual,
    MusicianComposer,
    Philosopher,
    Explorer,
    Ruler,
    MilitaryLeader,
    ReligiousLeader,
    Athlete,
    Reformer,
    Unknown,
}

impl PersonClass {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Scientist => "scientist",
            Self::InventorEngineer => "inventor_engineer",
            Self::ArtistWriter => "artist_writer",
            Self::ArtistVisual => "artist_visual",
            Self::MusicianComposer => "musician_composer",
            Self::Philosopher => "philosopher",
            Self::Explorer => "explorer",
            Self::Ruler => "ruler",
            Self::MilitaryLeader => "military_leader",
            Self::ReligiousLeader => "religious_leader",
            Self::Athlete => "athlete",
            Self::Reformer => "reformer",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Clone)]
pub struct IngestProfile {
    pub class: PersonClass,
    pub expected_event_types: &'static [&'static str],
    pub wikipedia_boost: &'static [&'static str],
    pub wikipedia_deny: &'static [&'static str],
    pub catalog_terms: &'static [&'static str],
    pub scholarly_terms: &'static [&'static str],
    pub enable_wdqs_military: bool,
    pub enable_military_extractor: bool,
}

const CLASS_ORDER: &[PersonClass] = &[
    PersonClass::Scientist,
    PersonClass::InventorEngineer,
    PersonClass::ArtistWriter,
    PersonClass::ArtistVisual,
    PersonClass::MusicianComposer,
    PersonClass::Philosopher,
    PersonClass::Explorer,
    PersonClass::ReligiousLeader,
    PersonClass::Athlete,
    PersonClass::Reformer,
    PersonClass::MilitaryLeader,
    PersonClass::Ruler,
];

/// Every matching class for this person (polyvalent careers). Empty → unknown later.
pub fn infer_person_classes(occupations: &[String], lead: Option<&str>) -> Vec<PersonClass> {
    let mut found = Vec::new();
    for occ in occupations {
        if let Some(c) = class_from_text(occ) {
            if !found.contains(&c) {
                found.push(c);
            }
        }
    }
    if let Some(lead) = lead {
        if let Some(c) = class_from_text(lead) {
            if !found.contains(&c) {
                found.push(c);
            }
        }
    }
    CLASS_ORDER
        .iter()
        .copied()
        .filter(|c| found.contains(c))
        .collect()
}

/// Primary class for logging / catalog query (first in CLASS_ORDER).
pub fn infer_person_class(occupations: &[String], lead: Option<&str>) -> PersonClass {
    infer_person_classes(occupations, lead)
        .into_iter()
        .next()
        .unwrap_or(PersonClass::Unknown)
}

pub fn has_military_signal(occupations: &[String], lead: Option<&str>) -> bool {
    infer_person_classes(occupations, lead).contains(&PersonClass::MilitaryLeader)
}

fn military_event_type(event_type: &str) -> bool {
    matches!(
        event_type,
        "battle" | "siege" | "military_campaign" | "surrender" | "retreat"
    )
}

fn clause_attests_subject_service(clause: &str, subject: &str) -> bool {
    let c = clause.to_lowercase();
    let name_hit = subject
        .split_whitespace()
        .map(|w| w.trim_matches(|ch: char| !ch.is_alphabetic()).to_lowercase())
        .filter(|w| w.chars().count() >= 4)
        .any(|w| c.contains(&w));
    if !name_hit {
        return false;
    }
    [
        "fought",
        "enlisted",
        "served in",
        "served as",
        "soldier",
        "battled",
        "combattit",
        "s'engagea",
        "s engager",
        "militaire",
    ]
    .iter()
    .any(|v| c.contains(v))
}

/// Keep battle/siege only if this person has a military signal or the clause attests service.
pub fn keep_military_typed_event(
    event_type: &str,
    clause: &str,
    subject: &str,
    has_military_signal: bool,
) -> bool {
    if !military_event_type(event_type) {
        return true;
    }
    has_military_signal || clause_attests_subject_service(clause, subject)
}

fn class_from_text(raw: &str) -> Option<PersonClass> {
    let t = raw.to_lowercase();
    if hit(
        &t,
        &[
            "physicist",
            "chemist",
            "scientist",
            "mathematician",
            "biologist",
            "physician",
            "physicien",
            "chimiste",
            "scientifique",
            "computer scientist",
            "cryptanalyst",
            "nobel",
        ],
    ) {
        return Some(PersonClass::Scientist);
    }
    if hit(&t, &["inventor", "engineer", "inventeur", "ingénieur", "ingenieur"]) {
        return Some(PersonClass::InventorEngineer);
    }
    if hit(
        &t,
        &[
            "writer",
            "poet",
            "novelist",
            "author",
            "écrivain",
            "ecrivain",
            "poète",
            "poete",
            "romancier",
            "journalist",
        ],
    ) {
        return Some(PersonClass::ArtistWriter);
    }
    if hit(
        &t,
        &[
            "painter",
            "sculptor",
            "architect",
            "peintre",
            "sculpteur",
            "architecte",
        ],
    ) {
        return Some(PersonClass::ArtistVisual);
    }
    if hit(&t, &["composer", "musician", "compositeur", "musicien"]) {
        return Some(PersonClass::MusicianComposer);
    }
    if hit(&t, &["philosopher", "philosophe", "thinker", "penseur"]) {
        return Some(PersonClass::Philosopher);
    }
    if hit(
        &t,
        &[
            "explorer",
            "navigator",
            "voyageur",
            "explorateur",
            "navigateur",
        ],
    ) {
        return Some(PersonClass::Explorer);
    }
    if hit(
        &t,
        &[
            "pharaoh",
            "queen of",
            "king of",
            "emperor",
            "empress",
            "monarch",
            "president",
            "prime minister",
            "pharaon",
            "reine",
            "roi",
            "empereur",
        ],
    ) {
        return Some(PersonClass::Ruler);
    }
    if hit(
        &t,
        &[
            "military",
            "soldier",
            "general",
            "admiral",
            "officer",
            "maréchal",
            "marechal",
            "militaire",
        ],
    ) {
        return Some(PersonClass::MilitaryLeader);
    }
    if hit(&t, &["saint", "pope", "cardinal", "theologian", "religieux"]) {
        return Some(PersonClass::ReligiousLeader);
    }
    if hit(&t, &["athlete", "sportsman", "footballer", "champion"]) {
        return Some(PersonClass::Athlete);
    }
    if hit(&t, &["revolutionary", "reformer", "révolutionnaire", "reformateur"]) {
        return Some(PersonClass::Reformer);
    }
    if t.contains("statesman") || t.contains("politician") || t.contains("homme d'état") {
        return Some(PersonClass::Ruler);
    }
    None
}

fn hit(text: &str, needles: &[&str]) -> bool {
    needles.iter().any(|n| text.contains(n))
}

pub fn profile_for(class: PersonClass) -> IngestProfile {
    match class {
        PersonClass::Scientist => IngestProfile {
            class,
            expected_event_types: &["discovery", "publication", "award", "education", "office"],
            wikipedia_boost: &[
                "university", "université", "laboratory", "laboratoire", "nobel", "prix",
                "publication", "discovery", "découverte", "academy", "académie", "institute",
            ],
            wikipedia_deny: &[
                "battle of", "bataille de", "guerre_", "campagne_", "siege of", "ordre de bataille",
                "coalition",
            ],
            catalog_terms: &["laboratoire", "nobel", "radioactiv", "physique", "chimie"],
            scholarly_terms: &["biography", "correspondence", "laboratory", "discovery"],
            enable_wdqs_military: false,
            enable_military_extractor: false,
        },
        PersonClass::InventorEngineer => IngestProfile {
            class,
            expected_event_types: &["invention", "patent", "publication", "office"],
            wikipedia_boost: &["invention", "patent", "brevet", "laboratory", "prototype"],
            wikipedia_deny: &["battle of", "bataille de", "guerre_"],
            catalog_terms: &["invention", "brevet", "patent"],
            scholarly_terms: &["invention", "patent", "engineering"],
            enable_wdqs_military: false,
            enable_military_extractor: false,
        },
        PersonClass::ArtistWriter => IngestProfile {
            class,
            expected_event_types: &["publication", "exile", "office", "residence"],
            wikipedia_boost: &[
                "œuvre", "oeuvres", "bibliography", "bibliographie", "poem", "publication",
                "correspondence", "correspondance",
            ],
            wikipedia_deny: &[
                "battle of", "bataille de", "coalition", "campagne_", "guerres_",
            ],
            catalog_terms: &["roman", "poésie", "poesie", "théâtre", "theatre", "correspondance"],
            scholarly_terms: &["bibliography", "reception", "censorship"],
            enable_wdqs_military: false,
            enable_military_extractor: false,
        },
        PersonClass::ArtistVisual => IngestProfile {
            class,
            expected_event_types: &["publication", "residence", "office"],
            wikipedia_boost: &[
                "œuvre", "oeuvres", "exhibition", "exposition", "museum", "musée", "atelier",
                "commission", "collection",
            ],
            wikipedia_deny: &["battle of", "bataille de", "guerres_", "campagne_"],
            catalog_terms: &["peinture", "sculpture", "atelier", "musée", "musee"],
            scholarly_terms: &["catalogue raisonné", "exhibition"],
            enable_wdqs_military: false,
            enable_military_extractor: false,
        },
        PersonClass::MusicianComposer => IngestProfile {
            class,
            expected_event_types: &["publication", "office"],
            wikipedia_boost: &["œuvre", "oeuvres", "opera", "opéra", "symphony", "concert", "conservatoire"],
            wikipedia_deny: &["battle of", "bataille de", "guerres_"],
            catalog_terms: &["opéra", "opera", "partition", "concert"],
            scholarly_terms: &["composition", "premiere"],
            enable_wdqs_military: false,
            enable_military_extractor: false,
        },
        PersonClass::Philosopher => IngestProfile {
            class,
            expected_event_types: &["publication", "education"],
            wikipedia_boost: &["œuvre", "oeuvres", "concept", "school", "école", "doctrine"],
            wikipedia_deny: &["battle of", "bataille de", "guerre_"],
            catalog_terms: &["philosophie", "doctrine"],
            scholarly_terms: &["philosophy", "reception"],
            enable_wdqs_military: false,
            enable_military_extractor: false,
        },
        PersonClass::Explorer => IngestProfile {
            class,
            expected_event_types: &["arrival", "departure", "voyage", "historical_fact"],
            wikipedia_boost: &[
                "voyage", "expedition", "expédition", "navigation", "route", "discovery",
                "découverte",
            ],
            wikipedia_deny: &["première guerre mondiale", "seconde guerre mondiale", "world war"],
            catalog_terms: &["voyage", "expédition", "expedition", "navigation"],
            scholarly_terms: &["voyage", "navigation", "discovery"],
            enable_wdqs_military: false,
            enable_military_extractor: false,
        },
        PersonClass::Ruler => IngestProfile {
            class,
            expected_event_types: &["office", "diplomatic", "treaty", "residence"],
            wikipedia_boost: &[
                "règne", "regne", "treaty", "traité", "traite", "reform", "réforme", "cour",
                "presidency", "président", "president", "office",
            ],
            wikipedia_deny: &["comics", "fictional"],
            catalog_terms: &["règne", "regne", "traité", "traite", "cour"],
            scholarly_terms: &["reign", "diplomacy", "court"],
            enable_wdqs_military: false,
            enable_military_extractor: false,
        },
        PersonClass::MilitaryLeader => IngestProfile {
            class,
            expected_event_types: &["battle", "siege", "treaty", "exile", "office"],
            wikipedia_boost: &[
                "battle of", "bataille", "siege", "siège", "campaign", "campagne", "treaty",
                "traité", "guerre", "président", "president", "présidence", "presidence",
                "gouvernement", "résistance", "resistance", "ministre", "discours", "rpf",
                "élysée", "elysee",
            ],
            wikipedia_deny: &["comics", "fictional"],
            catalog_terms: &["bataille", "campagne", "guerre", "armée"],
            scholarly_terms: &["campaign", "battle", "napoleonic"],
            enable_wdqs_military: true,
            enable_military_extractor: true,
        },
        PersonClass::ReligiousLeader => IngestProfile {
            class,
            expected_event_types: &["office", "residence"],
            wikipedia_boost: &["doctrine", "canonisation", "pèlerinage", "pelerinage", "sermon"],
            wikipedia_deny: &["battle of", "bataille de", "comics"],
            catalog_terms: &["sermon", "théologie", "theologie"],
            scholarly_terms: &["theology", "canonization"],
            enable_wdqs_military: false,
            enable_military_extractor: false,
        },
        PersonClass::Athlete => IngestProfile {
            class,
            expected_event_types: &["office"],
            wikipedia_boost: &["career", "carrière", "championnat", "record", "club"],
            wikipedia_deny: &["liste des", "calendrier_"],
            catalog_terms: &["sport", "championnat"],
            scholarly_terms: &["biography"],
            enable_wdqs_military: false,
            enable_military_extractor: false,
        },
        PersonClass::Reformer => IngestProfile {
            class,
            expected_event_types: &["office", "diplomatic", "publication"],
            wikipedia_boost: &["révolution", "revolution", "réforme", "reforme", "manifeste", "parti"],
            wikipedia_deny: &["comics"],
            catalog_terms: &["révolution", "revolution", "réforme", "reforme"],
            scholarly_terms: &["revolution", "reform"],
            enable_wdqs_military: false,
            enable_military_extractor: false,
        },
        PersonClass::Unknown => IngestProfile {
            class,
            expected_event_types: &["birth", "death", "residence"],
            wikipedia_boost: &["early life", "biography", "timeline"],
            wikipedia_deny: &["comics", "fictional character"],
            catalog_terms: &[],
            scholarly_terms: &["biography", "historiography"],
            enable_wdqs_military: false,
            enable_military_extractor: false,
        },
    }
}

pub fn rank_wikipedia_title(title: &str, profile: &IngestProfile, death_year: Option<i32>) -> f32 {
    rank_wikipedia_title_ex(
        title,
        profile,
        death_year,
        profile.enable_military_extractor,
    )
}

/// POC-style keep score: topical boosts, deny unless this person is military, WW2 penalty if dead before 1900.
pub fn rank_wikipedia_title_for_classes(
    title: &str,
    classes: &[PersonClass],
    death_year: Option<i32>,
    has_military_signal: bool,
) -> f32 {
    if classes.is_empty() {
        return rank_wikipedia_title_ex(
            title,
            &profile_for(PersonClass::Unknown),
            death_year,
            has_military_signal,
        );
    }
    classes
        .iter()
        .map(|c| {
            rank_wikipedia_title_ex(title, &profile_for(*c), death_year, has_military_signal)
        })
        .fold(0.0f32, f32::max)
}

fn rank_wikipedia_title_ex(
    title: &str,
    profile: &IngestProfile,
    death_year: Option<i32>,
    allow_military_pages: bool,
) -> f32 {
    let lower = title.to_lowercase();
    let battleish = lower.contains("battle of")
        || lower.contains("bataille de")
        || lower.contains("siege of")
        || lower.contains("siège de")
        || lower.contains("siege de");
    if profile.wikipedia_deny.iter().any(|d| lower.contains(d))
        && !(allow_military_pages && battleish)
    {
        return 0.15;
    }
    // Baseline below POC keep threshold (0.55) so untopical links drop.
    let mut score: f32 = 0.40;
    if profile.wikipedia_boost.iter().any(|b| lower.contains(b)) {
        score += 0.40;
    }
    if allow_military_pages && battleish {
        score += 0.30;
    }
    if let Some(death) = death_year {
        if death < 1900
            && (lower.contains("world war")
                || lower.contains("guerre mondiale")
                || lower.contains("1914")
                || lower.contains("1939"))
        {
            score -= 0.50;
        }
    }
    score.clamp(0.0, 1.0)
}

fn profile_terms(profile: &IngestProfile, kind: &SourceKind) -> &'static [&'static str] {
    match kind {
        SourceKind::OpenAlex
        | SourceKind::ThesesFr
        | SourceKind::Hal
        | SourceKind::Crossref
        | SourceKind::Persee => profile.scholarly_terms,
        _ => {
            if profile.catalog_terms.is_empty() {
                profile.scholarly_terms
            } else {
                profile.catalog_terms
            }
        }
    }
}

fn escape_cql_phrase(label: &str) -> String {
    label.replace('"', "").trim().to_string()
}

fn or_cql_subject_clauses(terms: &[&str]) -> String {
    terms
        .iter()
        .map(|t| format!("dc.subject all \"{t}\""))
        .collect::<Vec<_>>()
        .join(" or ")
}

fn or_hal_field_clauses(terms: &[&str]) -> String {
    terms
        .iter()
        .flat_map(|t| [format!("title_t:{t}"), format!("keyword_s:{t}")])
        .collect::<Vec<_>>()
        .join(" OR ")
}

/// Profile-aware search buckets for connectors that benefit from multi-query discovery.
pub fn catalog_search_buckets(label: &str, profile: &IngestProfile, kind: SourceKind) -> Vec<String> {
    let label = escape_cql_phrase(label);
    if label.len() < 3 {
        return vec![];
    }
    let terms = profile_terms(profile, &kind);
    match kind {
        SourceKind::Gallica => {
            let mut buckets = vec![format!("dc.title all \"{label}\"")];
            if !terms.is_empty() {
                buckets.push(format!(
                    "gallica all \"{label}\" and ({})",
                    or_cql_subject_clauses(terms)
                ));
            }
            buckets.push(format!(
                "(dc.creator all \"{label}\" or dc.contributor all \"{label}\")"
            ));
            buckets
        }
        SourceKind::Hal => {
            let mut buckets = vec![format!("text:\"{label}\"")];
            if !terms.is_empty() {
                buckets.push(format!(
                    "text:\"{label}\" AND ({})",
                    or_hal_field_clauses(terms)
                ));
            }
            buckets.push(format!("authFullName_s:\"{label}\""));
            buckets
        }
        SourceKind::Persee => {
            let mut buckets = vec![label.clone()];
            for term in terms.iter().take(3) {
                buckets.push(format!("{label} {term}"));
            }
            buckets
        }
        _ => vec![catalog_search_query(&label, profile, kind)],
    }
}

pub fn catalog_search_query(label: &str, profile: &IngestProfile, kind: SourceKind) -> String {
    let label = escape_cql_phrase(label);
    let terms = profile_terms(profile, &kind);
    let or_terms = terms.join(" OR ");
    match kind {
        SourceKind::Bnf => {
            if or_terms.is_empty() {
                format!("bib.anywhere all \"{label}\"")
            } else {
                format!("bib.anywhere all \"{label}\" and bib.anywhere all \"{or_terms}\"")
            }
        }
        SourceKind::Gallica => {
            if terms.is_empty() {
                format!("dc.title all \"{label}\"")
            } else {
                format!(
                    "gallica all \"{label}\" and ({})",
                    or_cql_subject_clauses(terms)
                )
            }
        }
        SourceKind::Hal => {
            if terms.is_empty() {
                format!("text:\"{label}\"")
            } else {
                format!(
                    "text:\"{label}\" AND ({})",
                    or_hal_field_clauses(terms)
                )
            }
        }
        SourceKind::Persee => {
            if terms.is_empty() {
                label
            } else {
                format!("{label} {}", terms[0])
            }
        }
        SourceKind::Europeana => {
            if or_terms.is_empty() {
                format!("\"{label}\"")
            } else {
                format!("\"{label}\" AND ({or_terms})")
            }
        }
        SourceKind::OpenAlex => {
            if or_terms.is_empty() {
                format!("\"{label}\"")
            } else {
                format!("\"{label}\" {or_terms}")
            }
        }
        SourceKind::InternetArchive => {
            if or_terms.is_empty() {
                format!("title:(\"{label}\") AND mediatype:texts")
            } else {
                format!("title:(\"{label}\") AND mediatype:texts AND ({or_terms})")
            }
        }
        SourceKind::ThesesFr => {
            let clauses: Vec<String> = subject_match_aliases(&label)
                .into_iter()
                .flat_map(|n| {
                    [
                        format!("titrePrincipal:({n})"),
                        format!("sujetsRameauLibelle:({n})"),
                    ]
                })
                .collect();
            format!("({})", clauses.join(" OR "))
        }
        SourceKind::WikimediaCommons | SourceKind::Wikisource => {
            if or_terms.is_empty() {
                label
            } else {
                format!("{label} {or_terms}")
            }
        }
        _ => {
            if or_terms.is_empty() {
                format!("\"{label}\"")
            } else {
                format!("\"{label}\" ({or_terms})")
            }
        }
    }
}

/// Keep the subject page; drop denied / untopical titles (POC keep ≥ 0.55).
pub fn filter_wiki_titles_for_profile(
    subject: &str,
    titles: Vec<String>,
    profile: &IngestProfile,
    death_year: Option<i32>,
) -> Vec<String> {
    filter_wiki_titles_scored(
        subject,
        titles,
        |title| rank_wikipedia_title(title, profile, death_year),
    )
}

pub fn filter_wiki_titles_for_classes(
    subject: &str,
    titles: Vec<String>,
    classes: &[PersonClass],
    death_year: Option<i32>,
    has_military_signal: bool,
) -> Vec<String> {
    filter_wiki_titles_scored(subject, titles, |title| {
        rank_wikipedia_title_for_classes(title, classes, death_year, has_military_signal)
    })
}

fn filter_wiki_titles_scored(
    subject: &str,
    titles: Vec<String>,
    score_fn: impl Fn(&str) -> f32,
) -> Vec<String> {
    let mut kept: Vec<(bool, f32, String)> = Vec::new();
    for title in titles {
        let is_subject = title.eq_ignore_ascii_case(subject);
        let score = score_fn(&title);
        if !is_subject && score < 0.55 {
            continue;
        }
        kept.push((is_subject, score, title));
    }
    kept.sort_by(|a, b| {
        b.0.cmp(&a.0)
            .then_with(|| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal))
            .then_with(|| a.2.cmp(&b.2))
    });
    kept.into_iter().map(|(_, _, t)| t).collect()
}
