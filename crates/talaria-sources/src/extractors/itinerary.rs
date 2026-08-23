// crates/talaria-sources/src/extractors/itinerary.rs
//! Itinerary steps: every dated stop in a travel paragraph — never interpolated.

use crate::extractors::travel::find_year;
use crate::extractors::{CandidateExtractor, ExtractorInput, RawCandidate};
use crate::place_quality::is_plausible_place_label;

pub struct ItineraryExtractor;

const MOTION_CUES: &[&str] = &[
    "departed",
    "left for",
    "set out for",
    "sailed for",
    "embarked",
    "arrived",
    "entered ",
    "reached ",
    "landed",
    "disembarked",
    "passed through",
    "stopped at",
    "returned to",
    "traversent",
    "traverse ",
    "partit pour",
    "partent pour",
    "partent de",
    "parti pour",
    "partie pour",
    "arriva à",
    "arriva a",
    "arrivèrent à",
    "arrive à",
    "se rendit à",
    "se rendent à",
    "se rend à",
    "parviennent à",
    "parvint à",
    "parvenir à",
    "s'embarquent",
    "s’embarquent",
    "s'embarque",
    "s’embarque",
    "embarquent à",
    "débarquent",
    "débarqua",
    "débarqué",
    "visitent",
    "visita ",
    "revint à",
    "retourna à",
    "passa par",
    "passé par",
    "à l'hôtel",
    "à l’hôtel",
    "à l'hotel",
    "at the hotel",
];

const PLACE_CUES: &[&str] = &[
    "departed for ",
    "left for ",
    "set out for ",
    "sailed for ",
    "arrived at ",
    "arrived in ",
    "landed at ",
    "passed through ",
    "stopped at ",
    "returned to ",
    "reached ",
    "entered ",
    "embarked for ",
    "disembarked at ",
    "partit pour ",
    "partent pour ",
    "partent de ",
    "parti pour ",
    "partie pour ",
    "arriva à ",
    "arriva a ",
    "arrivèrent à ",
    "arrive à ",
    "se rendit à ",
    "se rendent à ",
    "se rend à ",
    "parviennent à ",
    "parvint à ",
    "parvenir à ",
    "s'embarquent à ",
    "s’embarquent à ",
    "s'embarque à ",
    "s’embarque à ",
    "embarquent à ",
    "débarquent à ",
    "débarqua à ",
    "débarqué à ",
    "visitent ",
    "visita ",
    "revint à ",
    "retourna à ",
    "passa par ",
    "passé par ",
    "à l'hôtel ",
    "à l’hôtel ",
    "à l'hotel ",
    "at the hotel ",
    "vers ",
    "via ",
];

impl CandidateExtractor for ItineraryExtractor {
    fn extractor_id(&self) -> &str {
        "itinerary"
    }

    fn version(&self) -> &str {
        "itinerary:v2"
    }

    fn extract(&self, input: &ExtractorInput) -> Vec<RawCandidate> {
        let subject = input.effective_subject();
        let mut out = Vec::new();
        for (i, paragraph) in split_paragraphs(&input.text).into_iter().enumerate() {
            if !is_travel_paragraph(&paragraph) {
                continue;
            }
            let year = find_year(&paragraph);
            let Some(year) = year else { continue };
            let places = collect_stops(&paragraph, &input.known_places);
            let multi = places.len() > 1;
            for (j, place) in places.into_iter().enumerate() {
                let etype = classify_stop(&paragraph, &place, j == 0 && !multi);
                out.push(RawCandidate {
                    event_type: etype.0.into(),
                    predicate: etype.1.into(),
                    subject_surface: subject.clone(),
                    time_surface: Some(year.clone()),
                    place_surface: Some(place),
                    object_surface: None,
                    participant_surfaces: vec![],
                    clause_text: paragraph.trim().to_string(),
                    clause_index: i as i32,
                    start_offset: 0,
                    end_offset: paragraph.len() as i32,
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

fn split_paragraphs(text: &str) -> Vec<String> {
    let mut parts: Vec<String> = text
        .split("\n\n")
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect();
    if parts.is_empty() && !text.trim().is_empty() {
        parts.push(text.trim().to_string());
    }
    parts
}

fn is_travel_paragraph(text: &str) -> bool {
    let lower = text.to_lowercase();
    MOTION_CUES.iter().any(|c| lower.contains(c))
}

fn collect_stops(paragraph: &str, known: &[String]) -> Vec<String> {
    let mut places = Vec::new();
    let lower = paragraph.to_lowercase();
    for cue in PLACE_CUES {
        let mut search = 0usize;
        while let Some(rel) = lower[search..].find(cue) {
            let pos = search + rel;
            if let Some(token) = take_place_token(&paragraph[pos + cue.len()..]) {
                push_place(&mut places, token);
            }
            search = pos + cue.len();
        }
    }
    let mut known_hits: Vec<(usize, String)> = known
        .iter()
        .filter(|p| !p.trim().is_empty())
        .filter(|p| lower.contains(&p.to_lowercase()))
        .map(|p| (p.chars().count(), p.clone()))
        .collect();
    known_hits.sort_by(|a, b| b.0.cmp(&a.0));
    for (_, p) in known_hits {
        push_place(&mut places, p);
    }
    drop_country_if_cities(&mut places);
    places
}

fn take_place_token(after: &str) -> Option<String> {
    let mut words = Vec::new();
    for w in after.split_whitespace() {
        let clean = w
            .trim_matches(|c: char| !c.is_alphabetic() && c != '-' && c != '\'')
            .to_string();
        if clean.is_empty() {
            break;
        }
        let lower = clean.to_lowercase();
        let first = clean.chars().next()?;
        if first.is_uppercase() {
            words.push(clean);
            if words.len() >= 3 {
                break;
            }
            continue;
        }
        if !words.is_empty()
            && matches!(lower.as_str(), "de" | "du" | "des" | "sur" | "la" | "le" | "les")
        {
            words.push(clean);
            continue;
        }
        break;
    }
    while words.last().is_some_and(|w| {
        matches!(
            w.to_lowercase().as_str(),
            "de" | "du" | "des" | "le" | "la" | "les"
        )
    }) {
        words.pop();
    }
    let token = words.join(" ");
    if token.len() >= 2 {
        Some(token)
    } else {
        None
    }
}

fn push_place(out: &mut Vec<String>, token: String) {
    let token = token.trim().to_string();
    if !is_plausible_place_label(&token) {
        return;
    }
    if is_country_or_region(&token) && out.iter().any(|p| !is_country_or_region(p)) {
        return;
    }
    if out
        .iter()
        .any(|p| p.eq_ignore_ascii_case(&token) || p.to_lowercase().contains(&token.to_lowercase()))
    {
        return;
    }
    out.push(token);
}

fn drop_country_if_cities(places: &mut Vec<String>) {
    if places.iter().any(|p| !is_country_or_region(p)) {
        places.retain(|p| !is_country_or_region(p));
    }
}

pub fn is_country_or_region(label: &str) -> bool {
    matches!(
        label.to_lowercase().as_str(),
        "italie"
            | "italy"
            | "france"
            | "espagne"
            | "spain"
            | "suisse"
            | "switzerland"
            | "allemagne"
            | "germany"
            | "angleterre"
            | "england"
            | "europe"
            | "autriche"
            | "austria"
            | "pologne"
            | "poland"
            | "russie"
            | "russia"
            | "égypte"
            | "egypte"
            | "egypt"
            | "grèce"
            | "grece"
            | "greece"
            | "portugal"
            | "belgique"
            | "belgium"
            | "pays-bas"
            | "netherlands"
    )
}

fn classify_stop(paragraph: &str, place: &str, single_first: bool) -> (&'static str, &'static str) {
    let lower = paragraph.to_lowercase();
    let place_l = place.to_lowercase();
    if lower.contains(&format!("partent de {place_l}"))
        || lower.contains(&format!("departed {place_l}"))
        || lower.contains(&format!("left {place_l}"))
    {
        return ("departure", "departed_from");
    }
    if lower.contains(&format!("partent pour {place_l}"))
        || lower.contains(&format!("partit pour {place_l}"))
        || lower.contains(&format!("left for {place_l}"))
    {
        return ("departure", "departed_for");
    }
    if lower.contains(&format!("passa par {place_l}"))
        || lower.contains(&format!("via {place_l}"))
        || lower.contains(&format!("passed through {place_l}"))
    {
        return ("passage", "passed_through");
    }
    if single_first
        && (lower.contains("partit pour")
            || lower.contains("partent pour")
            || lower.contains("left for")
            || lower.contains("departed"))
    {
        return ("departure", "departed_for");
    }
    ("arrival", "arrived_in")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::extractors::ExtractorInput;

    #[test]
    fn french_departure_to_venice() {
        let raws = ItineraryExtractor.extract(&ExtractorInput {
            text: "En décembre 1833, la romancière partit pour Venise.".into(),
            page_title: Some("George Sand".into()),
            subject_label: Some("George Sand".into()),
            document_type: "article".into(),
            subject_death_year: Some(1876),
            wikitext: None,
            known_places: vec![],
        });
        assert_eq!(raws[0].event_type, "departure");
        assert_eq!(raws[0].place_surface.as_deref(), Some("Venise"));
        assert_eq!(raws[0].time_surface.as_deref(), Some("1833"));
    }

    #[test]
    fn english_arrival_for_another_person() {
        let raws = ItineraryExtractor.extract(&ExtractorInput {
            text: "In 1482 Leonardo arrived in Milan.".into(),
            page_title: Some("Leonardo da Vinci".into()),
            subject_label: Some("Leonardo da Vinci".into()),
            document_type: "article".into(),
            subject_death_year: Some(1519),
            wikitext: None,
            known_places: vec![],
        });
        assert_eq!(raws[0].event_type, "arrival");
        assert_eq!(raws[0].place_surface.as_deref(), Some("Milan"));
        assert_eq!(raws[0].time_surface.as_deref(), Some("1482"));
    }

    #[test]
    fn travel_paragraph_emits_each_stop_with_shared_year() {
        let raws = ItineraryExtractor.extract(&ExtractorInput {
            text: "Ils partent de Paris le 12 décembre 1833, s'embarquent à Marseille, débarquent à Gênes, visitent Florence et parviennent à Venise le 31 décembre.".into(),
            page_title: Some("George Sand".into()),
            subject_label: Some("George Sand".into()),
            document_type: "article".into(),
            subject_death_year: Some(1876),
            wikitext: None,
            known_places: vec![],
        });
        let places: Vec<_> = raws
            .iter()
            .filter_map(|r| r.place_surface.as_deref())
            .collect();
        for expected in ["Paris", "Marseille", "Gênes", "Florence", "Venise"] {
            assert!(
                places.iter().any(|p| *p == expected),
                "missing {expected} in {places:?} from {raws:?}"
            );
        }
        assert!(raws.iter().all(|r| r.time_surface.as_deref() == Some("1833")));
    }

    #[test]
    fn wiki_linked_places_become_stops_in_a_travel_sentence() {
        let raws = ItineraryExtractor.extract(&ExtractorInput {
            text: "En 1836 ils traversent la Suisse vers Genève et Chamonix.".into(),
            page_title: Some("George Sand".into()),
            subject_label: Some("George Sand".into()),
            document_type: "article".into(),
            subject_death_year: Some(1876),
            wikitext: None,
            known_places: vec!["Genève".into(), "Chamonix".into(), "Suisse".into()],
        });
        let places: Vec<_> = raws
            .iter()
            .filter_map(|r| r.place_surface.as_deref())
            .collect();
        assert!(places.contains(&"Genève"), "{places:?}");
        assert!(places.contains(&"Chamonix"), "{places:?}");
        assert!(
            !places.contains(&"Suisse"),
            "country dropped when cities exist: {places:?}"
        );
        assert!(raws.iter().all(|r| r.time_surface.as_deref() == Some("1836")));
    }
}
