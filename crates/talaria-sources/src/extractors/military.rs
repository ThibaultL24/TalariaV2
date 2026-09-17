// crates/talaria-sources/src/extractors/military.rs
//! Military campaign / battle page extractor — one page can yield multiple steps.

use crate::extractors::{CandidateExtractor, ExtractorInput, RawCandidate};
use crate::place_quality::is_plausible_place_label;

pub struct MilitaryCampaignExtractor;

impl CandidateExtractor for MilitaryCampaignExtractor {
    fn extractor_id(&self) -> &str {
        "military_campaign"
    }

    fn version(&self) -> &str {
        "military_campaign:v2"
    }

    fn extract(&self, input: &ExtractorInput) -> Vec<RawCandidate> {
        let mut out = Vec::new();
        let subject = input.effective_subject();
        let title = input.page_title.clone().unwrap_or_default();

        // Page-level battle/siege → at least one occurrence (place from title).
        if let Some((etype, place, object)) = classify_page_title(&title) {
            let year = first_year(&input.text, input.subject_death_year);
            let place_opt = if place.is_empty() { None } else { Some(place) };
            out.push(RawCandidate {
                event_type: etype.into(),
                predicate: predicate_for(etype).into(),
                subject_surface: subject.clone(),
                time_surface: year,
                place_surface: place_opt,
                object_surface: Some(object),
                participant_surfaces: vec![],
                clause_text: title.clone(),
                clause_index: 0,
                start_offset: 0,
                end_offset: title.len() as i32,
                cross_clause_join: false,
                extractor_id: self.extractor_id().into(),
                is_posthumous: false,
                lat: None,
                lon: None,
            });
        }

        for (i, unit) in crate::extractors::split_prose_units(&input.text)
            .into_iter()
            .enumerate()
        {
            let line = unit.as_str();
            let lower = line.to_lowercase();
            if subject_absent_from_action(&lower) || is_commemoration(&lower) {
                continue;
            }
            let year = first_year(line, input.subject_death_year);
            let hits = military_hits(line, &lower);
            if hits.is_empty() {
                continue;
            }
            let Some(year) = year else { continue };
            for (etype, pred, place) in hits {
                let Some(place) = place.filter(|p| is_plausible_place_label(p)) else {
                    continue;
                };
                out.push(RawCandidate {
                    event_type: etype.into(),
                    predicate: pred.into(),
                    subject_surface: subject.clone(),
                    time_surface: Some(year.clone()),
                    place_surface: Some(place),
                    object_surface: None,
                    participant_surfaces: vec![],
                    clause_text: line.trim().to_string(),
                    clause_index: i as i32,
                    start_offset: 0,
                    end_offset: line.len() as i32,
                    cross_clause_join: false,
                    extractor_id: self.extractor_id().into(),
                    is_posthumous: false,
                    lat: None,
                    lon: None,
                });
            }
        }
        out
    }
}

fn predicate_for(etype: &str) -> &'static str {
    match etype {
        "battle" => "fought_at",
        "siege" => "besieged",
        "treaty" => "signed",
        "imprisonment" => "captured_at",
        _ => "campaign_at",
    }
}

fn classify_page_title(title: &str) -> Option<(&'static str, String, String)> {
    let t = title.trim();
    let lower = t.to_lowercase();
    for prefix in ["battle of ", "bataille de ", "bataille d'"] {
        if let Some(rest) = lower.strip_prefix(prefix) {
            let place = title_case_place(&t[t.len() - rest.len()..]);
            return Some(("battle", place, t.to_string()));
        }
    }
    for prefix in ["siege of ", "siège de ", "siège d'", "siege de "] {
        if let Some(rest) = lower.strip_prefix(prefix) {
            let place = title_case_place(&t[t.len() - rest.len()..]);
            return Some(("siege", place, t.to_string()));
        }
    }
    if lower.contains("campaign") || lower.contains("campagne") {
        return Some(("military_campaign", String::new(), t.to_string()));
    }
    if lower.starts_with("treaty of ")
        || lower.starts_with("treaties of ")
        || lower.starts_with("traité de ")
        || lower.starts_with("traite de ")
    {
        let place = t
            .split_once(" of ")
            .or_else(|| t.split_once(" de "))
            .map(|(_, p)| p.to_string())
            .unwrap_or_else(|| t.to_string());
        return Some(("treaty", place, t.to_string()));
    }
    None
}

fn title_case_place(s: &str) -> String {
    s.split('(')
        .next()
        .unwrap_or(s)
        .trim()
        .trim_end_matches(['.', ','])
        .to_string()
}

fn first_year(text: &str, death_year: Option<i32>) -> Option<String> {
    let (lo, hi) = crate::lifespan_year_window(None, death_year);
    crate::first_year_in_window(text, lo, hi)
}

const BATTLE_PREFIXES: &[&str] = &[
    "battle of ",
    "bataille de ",
    "bataille d'",
    "victory at ",
    "victoire de ",
    "victoire à ",
    "victoire a ",
];

const SIEGE_PREFIXES: &[&str] = &[
    "siege of ",
    "siège de ",
    "siège d'",
    "siege de ",
];

fn subject_absent_from_action(lower: &str) -> bool {
    lower.contains("did not take part")
        || lower.contains("did not participate")
        || lower.contains("took no part")
        || lower.contains("ne prit pas part")
        || lower.contains("n'y prit pas part")
        || lower.contains("ne participa pas")
        || lower.contains("around the time")
        || lower.contains("au moment où")
        || lower.contains("au moment ou")
}

fn is_commemoration(lower: &str) -> bool {
    lower.contains("celebration")
        || lower.contains("festival")
        || lower.contains("panegyric")
        || lower.contains("radio")
        || lower.contains("mistère")
        || lower.contains("mystery of the siege")
        || lower.contains("fêtes johanniques")
        || lower.contains("fetes johanniques")
}

fn military_hits(line: &str, lower: &str) -> Vec<(&'static str, &'static str, Option<String>)> {
    let mut hits = Vec::new();
    push_prefix_hits(line, lower, SIEGE_PREFIXES, "siege", "besieged", &mut hits);
    push_prefix_hits(line, lower, BATTLE_PREFIXES, "battle", "fought_at", &mut hits);

    if hits.is_empty() {
        if let Some((etype, pred)) = classify_loose_cue(lower) {
            let place = place_from_line(line);
            hits.push((etype, pred, place));
        }
    }
    hits
}

fn push_prefix_hits(
    line: &str,
    lower: &str,
    prefixes: &[&str],
    etype: &'static str,
    pred: &'static str,
    hits: &mut Vec<(&'static str, &'static str, Option<String>)>,
) {
    for prefix in prefixes {
        let mut from = 0;
        while let Some(rel) = lower[from..].find(prefix) {
            let pos = from + rel;
            let after = &line[pos + prefix.len()..];
            if let Some(token) = take_place_token(after) {
                hits.push((etype, pred, Some(token)));
            }
            from = pos + prefix.len();
        }
    }
}

fn classify_loose_cue(lower: &str) -> Option<(&'static str, &'static str)> {
    if lower.contains("besieged")
        || lower.contains("assiège")
        || lower.contains("assiege")
        || lower.contains("relief of")
        || lower.contains("raising of the siege")
        || lower.contains("levée du siège")
        || lower.contains("levee du siege")
    {
        return Some(("siege", "besieged"));
    }
    if lower.contains("fought at")
        || lower.contains("combattit")
        || lower.contains("defeated")
        || (lower.contains("sortied") && (lower.contains("camp") || lower.contains("attack")))
    {
        return Some(("battle", "fought_at"));
    }
    if lower.contains("captured at")
        || lower.contains("taken prisoner")
        || lower.contains("capturée")
        || lower.contains("capturee")
        || lower.contains("capturé à")
        || lower.contains("capture a")
    {
        return Some(("imprisonment", "captured_at"));
    }
    if lower.contains("retreated") || lower.contains("retreat from") || lower.contains("recula") {
        return Some(("retreat", "retreated_from"));
    }
    if lower.contains("surrender") || lower.contains("capitulat") {
        return Some(("surrender", "surrendered_at"));
    }
    if lower.contains("headquarters") || lower.contains("quartier général") {
        return Some(("headquarters", "hq_at"));
    }
    if (lower.contains("campaign") || lower.contains("campagne"))
        && (lower.contains("began") || lower.contains("opened") || lower.contains("commenc"))
    {
        return Some(("military_campaign", "campaign_at"));
    }
    None
}

fn place_from_line(line: &str) -> Option<String> {
    let lower = line.to_lowercase();
    for cue in [
        "relieve ",
        "relief of ",
        " near ",
        " près de ",
        " pres de ",
        " at ",
        " in ",
        " à ",
        " au ",
        " aux ",
    ] {
        if let Some(pos) = lower.find(cue) {
            if let Some(token) = take_place_token(&line[pos + cue.len()..]) {
                return Some(token);
            }
        }
    }
    None
}

fn take_place_token(after: &str) -> Option<String> {
    const STOP: &[&str] = &[
        "in", "at", "on", "en", "et", "and", "by", "which", "who", "from", "during", "after",
        "before", "when", "where", "the", "a", "an", "le", "la", "les", "des", "du",
        "january", "february", "march", "april", "may", "june", "july", "august", "september",
        "october", "november", "december", "janvier", "février", "fevrier", "mars", "avril",
        "mai", "juin", "juillet", "août", "aout", "septembre", "octobre", "novembre", "décembre",
        "decembre",
    ];
    let mut words = Vec::new();
    for w in after.split_whitespace() {
        let clean = w
            .trim_matches(|c: char| !c.is_alphabetic() && c != '-' && c != '\'')
            .to_string();
        if clean.is_empty() {
            break;
        }
        let l = clean.to_lowercase();
        if STOP.contains(&l.as_str()) {
            if words.is_empty() && matches!(l.as_str(), "la" | "le" | "les" | "the") {
                words.push(clean);
                continue;
            }
            break;
        }
        let first = clean.chars().next()?;
        if first.is_uppercase()
            || (!words.is_empty() && matches!(l.as_str(), "de" | "du" | "d"))
        {
            words.push(clean);
            if words.len() >= 4 {
                break;
            }
            continue;
        }
        break;
    }
    let token = words.join(" ");
    if token.len() >= 2 {
        Some(token)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::extractors::ExtractorInput;

    fn joan_input(text: &str) -> ExtractorInput {
        ExtractorInput {
            text: text.into(),
            page_title: Some("Joan of Arc".into()),
            subject_label: Some("Joan of Arc".into()),
            document_type: "article".into(),
            subject_death_year: Some(1431),
            ..Default::default()
        }
    }

    #[test]
    fn splits_paragraph_into_dated_sieges() {
        let text = "Joan of Arc (1412 – 30 May 1431) is a patron saint of France.\n\
             After Charles's coronation, Joan participated in the unsuccessful siege of Paris in September 1429 and the failed siege of La Charité in November.\n\
             In early 1430, Joan organized a company of volunteers to relieve Compiègne, which had been besieged by the Burgundians.";
        let raws = MilitaryCampaignExtractor.extract(&joan_input(text));
        assert!(
            raws.iter().any(|r| r.event_type == "siege"
                && r.time_surface.as_deref() == Some("1429")
                && r.place_surface.as_deref().is_some_and(|p| p.contains("Paris"))),
            "Paris 1429: {raws:?}"
        );
        assert!(
            raws.iter().any(|r| r.event_type == "siege"
                && r.place_surface
                    .as_deref()
                    .is_some_and(|p| p.contains("Charité") || p.contains("Charite"))),
            "La Charité: {raws:?}"
        );
        assert!(
            raws.iter().any(|r| r.place_surface
                .as_deref()
                .is_some_and(|p| p.contains("Compiègne") || p.contains("Compiegne"))),
            "Compiègne: {raws:?}"
        );
    }

    #[test]
    fn french_siege_and_capture() {
        let text = "En octobre 1429, Jeanne participe au siège de Saint-Pierre-le-Moûtier.\n\
             Capturée par les Bourguignons à Compiègne en 1430, elle est vendue aux Anglais.";
        let raws = MilitaryCampaignExtractor.extract(&ExtractorInput {
            text: text.into(),
            page_title: Some("Jeanne d'Arc".into()),
            subject_label: Some("Jeanne d'Arc".into()),
            document_type: "article".into(),
            subject_death_year: Some(1431),
            ..Default::default()
        });
        assert!(
            raws.iter().any(|r| r.event_type == "siege"
                && r.time_surface.as_deref() == Some("1429")
                && r.place_surface
                    .as_deref()
                    .is_some_and(|p| p.contains("Saint-Pierre"))),
            "FR siege: {raws:?}"
        );
        assert!(
            raws.iter().any(|r| r.event_type == "imprisonment"
                && r.time_surface.as_deref() == Some("1430")
                && r.place_surface
                    .as_deref()
                    .is_some_and(|p| p.contains("Compiègne") || p.contains("Compiegne"))),
            "capture: {raws:?}"
        );
    }

    #[test]
    fn skips_battle_joan_did_not_fight() {
        let text = "Après la victoire de Patay (où Jeanne d'Arc ne prit pas part aux combats), le 18 juin 1429, Charles partit vers Reims.";
        let raws = MilitaryCampaignExtractor.extract(&ExtractorInput {
            text: text.into(),
            page_title: Some("Jeanne d'Arc".into()),
            subject_label: Some("Jeanne d'Arc".into()),
            document_type: "article".into(),
            subject_death_year: Some(1431),
            ..Default::default()
        });
        assert!(
            !raws
                .iter()
                .any(|r| r.place_surface.as_deref().is_some_and(|p| p.contains("Patay"))),
            "must not pin Patay: {raws:?}"
        );
    }

    #[test]
    fn french_battle_page_title() {
        let raws = MilitaryCampaignExtractor.extract(&ExtractorInput {
            text: "La bataille de Patay a lieu le 18 juin 1429.".into(),
            page_title: Some("Bataille de Patay".into()),
            subject_label: Some("Jeanne d'Arc".into()),
            document_type: "article".into(),
            subject_death_year: Some(1431),
            ..Default::default()
        });
        assert!(
            raws.iter().any(|r| r.event_type == "battle"
                && r.place_surface.as_deref().is_some_and(|p| p.contains("Patay"))),
            "{raws:?}"
        );
    }
}
