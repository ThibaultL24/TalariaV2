// crates/talaria-sources/src/person_profile.rs
//! Person-class ingest profiles (POC presets) — which pages and catalogs to search.

use serde::{Deserialize, Serialize};

use crate::kinds::SourceKind;

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

pub fn infer_person_class(occupations: &[String], lead: Option<&str>) -> PersonClass {
    let mut found = Vec::new();
    for occ in occupations {
        if let Some(c) = class_from_text(occ) {
            found.push(c);
        }
    }
    if let Some(lead) = lead {
        if let Some(c) = class_from_text(lead) {
            found.push(c);
        }
    }
    primary_class(&found)
}

fn primary_class(found: &[PersonClass]) -> PersonClass {
    const ORDER: &[PersonClass] = &[
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
    for want in ORDER {
        if found.contains(want) {
            return *want;
        }
    }
    PersonClass::Unknown
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
            wikipedia_boost: &["règne", "regne", "treaty", "traité", "traite", "reform", "réforme", "cour"],
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
    let lower = title.to_lowercase();
    if profile.wikipedia_deny.iter().any(|d| lower.contains(d)) {
        return 0.15;
    }
    let mut score: f32 = 0.55;
    if profile.wikipedia_boost.iter().any(|b| lower.contains(b)) {
        score += 0.40;
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

pub fn catalog_search_query(label: &str, profile: &IngestProfile, kind: SourceKind) -> String {
    let label = label.replace('"', "").trim().to_string();
    let terms = match kind {
        SourceKind::OpenAlex | SourceKind::ThesesFr | SourceKind::Hal | SourceKind::Crossref => {
            profile.scholarly_terms
        }
        _ => {
            if profile.catalog_terms.is_empty() {
                profile.scholarly_terms
            } else {
                profile.catalog_terms
            }
        }
    };
    let or_terms = terms.join(" OR ");
    match kind {
        SourceKind::Bnf => {
            if or_terms.is_empty() {
                format!("bib.anywhere all \"{label}\"")
            } else {
                format!("bib.anywhere all \"{label}\" and bib.anywhere all \"{or_terms}\"")
            }
        }
        SourceKind::Europeana | SourceKind::Gallica => {
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
            if or_terms.is_empty() {
                format!("(titrePrincipal:({label}) OR sujetsRameauLibelle:({label}))")
            } else {
                format!(
                    "(titrePrincipal:({label}) OR sujetsRameauLibelle:({label}) OR resumes.fr:({or_terms}))"
                )
            }
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

/// Keep the subject page; drop denied titles (battles for scientists, etc.).
pub fn filter_wiki_titles_for_profile(
    subject: &str,
    titles: Vec<String>,
    profile: &IngestProfile,
    death_year: Option<i32>,
) -> Vec<String> {
    let mut kept: Vec<(bool, f32, String)> = Vec::new();
    for title in titles {
        let is_subject = title.eq_ignore_ascii_case(subject);
        let score = rank_wikipedia_title(&title, profile, death_year);
        if !is_subject && score < 0.30 {
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
