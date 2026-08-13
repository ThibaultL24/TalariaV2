// crates/talaria-sources/src/historiography.rs
//! Deterministic debate / controversy / theory hits from prose and bibliographic metadata.
//! Never produces canonical events.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DebateType {
    FactualDispute,
    InterpretationDispute,
    ChronologyDispute,
    CauseOfDeathDispute,
    MotiveDispute,
    LegitimacyDispute,
    LegendOrMyth,
    ConspiracyOrSpeculative,
    ArchivalGap,
}

impl DebateType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::FactualDispute => "factual_dispute",
            Self::InterpretationDispute => "interpretation_dispute",
            Self::ChronologyDispute => "chronology_dispute",
            Self::CauseOfDeathDispute => "cause_of_death_dispute",
            Self::MotiveDispute => "motive_dispute",
            Self::LegitimacyDispute => "legitimacy_dispute",
            Self::LegendOrMyth => "legend_or_myth",
            Self::ConspiracyOrSpeculative => "conspiracy_or_speculative",
            Self::ArchivalGap => "archival_gap",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvidenceLayer {
    EvidenceGap,
    CompetingReading,
    Interpretation,
    TheoryOrLegend,
}

impl EvidenceLayer {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::EvidenceGap => "evidence_gap",
            Self::CompetingReading => "competing_reading",
            Self::Interpretation => "interpretation",
            Self::TheoryOrLegend => "theory_or_legend",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventHint {
    Birth,
    Death,
    Battle,
    Exile,
    Office,
}

impl EventHint {
    pub fn event_type(self) -> &'static str {
        match self {
            Self::Birth => "birth",
            Self::Death => "death",
            Self::Battle => "battle",
            Self::Exile => "exile",
            Self::Office => "office",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoriographyHit {
    pub debate_type: DebateType,
    pub evidence_layer: EvidenceLayer,
    pub claim_kind: &'static str,
    pub epistemic_status: &'static str,
    pub quote: String,
    pub event_hint: Option<EventHint>,
}

const SECTION_MARKERS: &[&str] = &[
    "historiograph",
    "legacy",
    "reputation",
    "controvers",
    "death",
    "mort",
    "myth",
    "legend",
    "légende",
    "legende",
    "poison",
    "postérité",
    "posterite",
    "reception",
    "réception",
    "memory",
    "mémoire",
    "memoire",
    "assessment",
    "débat",
    "debat",
    "cause of death",
    "cause de la mort",
];

const SKIP_SECTIONS: &[&str] = &[
    "early life",
    "enfance",
    "campaign",
    "campagne",
    "see also",
    "references",
    "notes",
    "external links",
    "bibliography",
    "bibliographie",
];

pub fn is_historiography_section(title: &str) -> bool {
    let t = title.to_lowercase();
    if SKIP_SECTIONS.iter().any(|s| t.contains(s)) && !t.contains("death") && !t.contains("mort") {
        return false;
    }
    SECTION_MARKERS.iter().any(|m| t.contains(m))
}

pub fn scan_passage(text: &str) -> Vec<HistoriographyHit> {
    split_loose_sentences(text)
        .into_iter()
        .filter_map(|s| classify_sentence(&s))
        .collect()
}

pub fn scan_bibliographic(title: &str, abstract_text: Option<&str>) -> Vec<HistoriographyHit> {
    let mut blob = title.to_string();
    if let Some(a) = abstract_text.filter(|s| !s.trim().is_empty()) {
        blob.push_str(". ");
        blob.push_str(a);
    }
    let mut hits = scan_passage(&blob);
    if hits.is_empty() && bibliographic_looks_historiographic(title) {
        hits.push(HistoriographyHit {
            debate_type: DebateType::InterpretationDispute,
            evidence_layer: EvidenceLayer::Interpretation,
            claim_kind: "debate_stance",
            epistemic_status: "contested",
            quote: title.trim().to_string(),
            event_hint: None,
        });
    }
    hits
}

fn bibliographic_looks_historiographic(title: &str) -> bool {
    let t = title.to_lowercase();
    [
        "historiograph",
        "relecture",
        "révision",
        "revision",
        "controverse",
        "controversy",
        "débat",
        "debat",
        "myth",
        "légende",
        "memory of",
        "mémoire de",
        "reception of",
        "postérité",
    ]
    .iter()
    .any(|m| t.contains(m))
}

fn split_loose_sentences(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    for ch in text.chars() {
        cur.push(ch);
        if matches!(ch, '.' | '!' | '?' | '\n') && cur.trim().len() >= 24 {
            out.push(cur.trim().to_string());
            cur.clear();
        }
    }
    if cur.trim().len() >= 24 {
        out.push(cur.trim().to_string());
    }
    out
}

fn classify_sentence(sentence: &str) -> Option<HistoriographyHit> {
    let lower = sentence.to_lowercase();
    let (debate_type, layer, kind, status) = detect(&lower)?;
    Some(HistoriographyHit {
        debate_type,
        evidence_layer: layer,
        claim_kind: kind,
        epistemic_status: status,
        quote: sentence.trim().to_string(),
        event_hint: event_hint(&lower),
    })
}

fn detect(lower: &str) -> Option<(DebateType, EvidenceLayer, &'static str, &'static str)> {
    if has_any(
        lower,
        &[
            "no contemporary source",
            "archives destroyed",
            "lack of evidence",
            "insufficient evidence",
            "documentation is scarce",
            "sources are silent",
            "aucune source contemporaine",
            "lacune",
            "manque de preuves",
            "mal document",
        ],
    ) {
        return Some((
            DebateType::ArchivalGap,
            EvidenceLayer::EvidenceGap,
            "controversy",
            "uncertain",
        ));
    }
    if has_any(
        lower,
        &[
            "poison",
            "arsenic",
            "cause of death",
            "cause de la mort",
            "stomach cancer",
            "cancer de l'estomac",
            "assassinat",
        ],
    ) && has_any(
        lower,
        &[
            "theor",
            "hypothes",
            "disput",
            "controvers",
            "alleged",
            " allegedly",
            "debate",
            "débat",
            "selon certains",
        ],
    ) {
        return Some((
            DebateType::CauseOfDeathDispute,
            EvidenceLayer::TheoryOrLegend,
            "theory",
            "hypothesized",
        ));
    }
    if has_any(
        lower,
        &[
            "legend has it",
            "according to legend",
            "apocryphal",
            "myth",
            "légende",
            "legende",
            "is said to have",
        ],
    ) {
        return Some((
            DebateType::LegendOrMyth,
            EvidenceLayer::TheoryOrLegend,
            "theory",
            "hypothesized",
        ));
    }
    if has_any(
        lower,
        &["conspiracy", "complot", "secret plot", "cover-up", "cover up"],
    ) {
        return Some((
            DebateType::ConspiracyOrSpeculative,
            EvidenceLayer::TheoryOrLegend,
            "theory",
            "hypothesized",
        ));
    }
    if has_any(
        lower,
        &[
            "historians debate",
            "historians disagree",
            "no consensus",
            "remain disputed",
            "remains controversial",
            "les historiens débattent",
            "les historiens discutent",
            "controverse historiographique",
            "revisionist",
            "orthodox view",
        ],
    ) {
        return Some((
            DebateType::InterpretationDispute,
            EvidenceLayer::Interpretation,
            "debate_stance",
            "contested",
        ));
    }
    if has_any(
        lower,
        &[
            "legitimacy",
            "légitimité",
            "usurp",
            "coup d'état",
            "coup d'etat",
            "rightful",
        ],
    ) && has_any(
        lower,
        &["disput", "controvers", "debate", "débat", "question"],
    ) {
        return Some((
            DebateType::LegitimacyDispute,
            EvidenceLayer::Interpretation,
            "debate_stance",
            "contested",
        ));
    }
    if has_any(lower, &["motive", "intention", "why he", "pourquoi il"])
        && has_any(lower, &["disput", "unclear", "debate", "débat", "hypothes"])
    {
        return Some((
            DebateType::MotiveDispute,
            EvidenceLayer::Interpretation,
            "debate_stance",
            "contested",
        ));
    }
    if has_any(
        lower,
        &[
            "date is disputed",
            "year is disputed",
            "chronology",
            "dating remains",
            "date controvers",
        ],
    ) {
        return Some((
            DebateType::ChronologyDispute,
            EvidenceLayer::CompetingReading,
            "controversy",
            "disputed",
        ));
    }
    if has_any(
        lower,
        &[
            "some say",
            "others argue",
            "on the other hand",
            "however, some",
            "alternative account",
            "selon d'autres",
            "d'autres historiens",
        ],
    ) {
        return Some((
            DebateType::FactualDispute,
            EvidenceLayer::CompetingReading,
            "controversy",
            "disputed",
        ));
    }
    if has_any(
        lower,
        &[
            "controversial",
            "disputed",
            "contested",
            "controverse",
            "débattu",
            "debattu",
        ],
    ) {
        return Some((
            DebateType::FactualDispute,
            EvidenceLayer::CompetingReading,
            "controversy",
            "disputed",
        ));
    }
    None
}

fn event_hint(lower: &str) -> Option<EventHint> {
    if has_any(
        lower,
        &[
            "died",
            "death",
            "mort",
            "poison",
            "arsenic",
            "saint helena",
            "sainte-hélène",
            "sainte-helene",
        ],
    ) {
        return Some(EventHint::Death);
    }
    if has_any(lower, &["born", "birth", "naissance"]) {
        return Some(EventHint::Birth);
    }
    if has_any(lower, &["battle", "bataille", "waterloo", "austerlitz"]) {
        return Some(EventHint::Battle);
    }
    if has_any(lower, &["exile", "exil", "elba", "elbe"]) {
        return Some(EventHint::Exile);
    }
    if has_any(lower, &["crowned", "emperor", "empereur", "consul"]) {
        return Some(EventHint::Office);
    }
    None
}

fn has_any(haystack: &str, needles: &[&str]) -> bool {
    needles.iter().any(|n| haystack.contains(n))
}
