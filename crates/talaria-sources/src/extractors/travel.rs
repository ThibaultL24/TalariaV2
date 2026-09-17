// crates/talaria-sources/src/extractors/travel.rs
use crate::extractors::{CandidateExtractor, ExtractorInput, RawCandidate};

pub struct TravelResidenceExtractor;

impl CandidateExtractor for TravelResidenceExtractor {
    fn extractor_id(&self) -> &str {
        "travel_residence"
    }

    fn version(&self) -> &str {
        "travel_residence:v2"
    }

    fn extract(&self, input: &ExtractorInput) -> Vec<RawCandidate> {
        let subject = input.effective_subject();
        let mut out = Vec::new();
        for (i, unit) in crate::extractors::split_prose_units(&input.text).into_iter().enumerate() {
            let line = unit.as_str();
            let lower = line.to_lowercase();
            let (etype, pred) = if lower.contains("departed")
                || lower.contains("left for")
                || lower.contains("set sail")
                || lower.contains("set off")
                || lower.contains("embarked for")
                || lower.contains("embarked on")
                || lower.contains("partit pour")
                || lower.contains("partent pour")
                || lower.contains("parti pour")
                || lower.contains("partie pour")
                || lower.contains("part pour")
                || lower.contains("quitte ")
                || lower.contains("quitta ")
                || lower.contains("quittent ")
            {
                ("departure", "departed_for")
            } else if lower.contains("arrived")
                || lower.contains("reached")
                || lower.contains("landed in")
                || lower.contains("landed at")
                || lower.contains("disembarked")
                || lower.contains("arriva")
                || lower.contains("arrive en métropole")
                || lower.contains("arrive en metropole")
                || lower.contains("fait escale")
                || lower.contains("fit escale")
                || lower.contains("faire escale")
            {
                ("arrival", "arrived_in")
            } else if lower.contains("traveled to")
                || lower.contains("travelled to")
                || lower.contains("journeyed to")
                || lower.contains("sailed to")
                || lower.contains("voyage to")
                || lower.contains("voyage à")
                || lower.contains("trip to")
                || lower.contains("went to")
            {
                ("travel", "traveled_to")
            } else if lower.contains("lived in")
                || lower.contains("lived at")
                || lower.contains("resided in")
                || lower.contains("resided at")
                || lower.contains("settled in")
                || lower.contains("settled at")
                || lower.contains("moved to")
                || lower.contains("relocated to")
                || lower.contains("vécut")
                || lower.contains("vecut")
                || lower.contains("habita")
                || lower.contains("s'installa")
                || lower.contains("s'installe")
                || lower.contains("séjourna")
                || lower.contains("sejourna")
                || lower.contains("séjourne")
                || lower.contains("sejourne")
                || lower.contains("se fixe")
                || lower.contains("se fixa")
                || lower.contains("loge à")
                || lower.contains("logea à")
                || lower.contains("logeait à")
                || lower.contains("domicilié")
                || lower.contains("domiciliée")
            {
                ("residence", "resided_in")
            } else if lower.contains("stayed in")
                || lower.contains("stayed at")
                || lower.contains("spent time in")
            {
                ("residence", "stayed_at")
            } else if lower.contains("revient à")
                || lower.contains("revint à")
                || lower.contains("retourne à")
                || lower.contains("retourna à")
                || lower.contains("returns to")
                || lower.contains("returned to")
                || lower.contains("went back to")
                || lower.contains("came back to")
                || lower.contains("on le ramène")
                || lower.contains("ramené à")
                || lower.contains("ramenée à")
            {
                ("arrival", "returned_to")
            } else if lower.contains("visited")
                || lower.contains("rend visite")
                || lower.contains("rendit visite")
                || lower.contains("rend plusieurs visites")
                || lower.contains("pays a visit")
                || lower.contains("paid a visit")
            {
                ("meeting", "visited")
            } else if lower.contains("frequent")
                || lower.contains("fréquente")
                || lower.contains("fréquenta")
                || lower.contains("frequented")
            {
                ("residence", "frequented")
            } else if lower.contains("exiled")
                || lower.contains("banished")
                || lower.contains("s'exila")
                || lower.contains("exilé")
            {
                ("exile", "exiled_to")
            } else {
                continue;
            };
            let year = find_year(line);
            let place = find_place(line);
            out.push(RawCandidate {
                event_type: etype.into(),
                predicate: pred.into(),
                subject_surface: subject.clone(),
                time_surface: year,
                place_surface: place,
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
        out
    }
}

pub(crate) fn find_year(s: &str) -> Option<String> {
    for w in s.split(|c: char| !c.is_ascii_digit()) {
        if w.len() == 4 {
            if let Ok(y) = w.parse::<i32>() {
                if (1000..=2100).contains(&y) {
                    return Some(y.to_string());
                }
            }
        }
    }
    None
}

pub(crate) fn find_place(s: &str) -> Option<String> {
    let lower = s.to_lowercase();
    for cue in [" in ", " at ", " to ", " for ", " à ", " au ", " aux ", " en ", " pour "] {
        if let Some(pos) = lower.rfind(cue) {
            let after = &s[pos + cue.len()..];
            let raw = after
                .split(|c: char| c == '.' || c.is_ascii_digit() || c == ',')
                .next()?
                .trim()
                .trim_matches(|c: char| !c.is_alphabetic() && c != ' ' && c != '-');
            let token = raw
                .trim_end_matches(" en")
                .trim_end_matches(" in")
                .trim_start_matches("l'")
                .trim()
                .to_string();
            if token.len() >= 2 && !token.eq_ignore_ascii_case("en") {
                return Some(token);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::extractors::ExtractorInput;

    #[test]
    fn french_residence_line_yields_place() {
        let raws = TravelResidenceExtractor.extract(&ExtractorInput {
            text: "Il vécut à Honfleur en 1859.".into(),
            page_title: Some("Charles Baudelaire".into()),
            subject_label: Some("Charles Baudelaire".into()),
            document_type: "article".into(),
            subject_death_year: Some(1867),
            ..Default::default()
        });
        assert_eq!(raws[0].event_type, "residence");
        assert_eq!(raws[0].time_surface.as_deref(), Some("1859"));
        assert_eq!(raws[0].place_surface.as_deref(), Some("Honfleur"));
    }

    #[test]
    fn french_installs_and_stays_are_residences() {
        let raws = TravelResidenceExtractor.extract(&ExtractorInput {
            text: "Elle s'installe à Nohant en 1831. En 1838 elle séjourne à Majorque.".into(),
            page_title: Some("George Sand".into()),
            subject_label: Some("George Sand".into()),
            document_type: "article".into(),
            subject_death_year: Some(1876),
            ..Default::default()
        });
        assert!(
            raws.iter().any(|r| r.place_surface.as_deref() == Some("Nohant")
                && r.time_surface.as_deref() == Some("1831")),
            "{raws:?}"
        );
        assert!(
            raws.iter().any(|r| r.place_surface.as_deref() == Some("Majorque")
                && r.time_surface.as_deref() == Some("1838")),
            "{raws:?}"
        );
    }

    #[test]
    fn travel_sentence_not_swallowed_by_publication_in_same_paragraph() {
        let travel = TravelResidenceExtractor.extract(&ExtractorInput {
            text: "Elle s'installe à Paris en 1831. Elle publia Indiana à Paris en 1832.".into(),
            page_title: Some("George Sand".into()),
            subject_label: Some("George Sand".into()),
            document_type: "article".into(),
            subject_death_year: Some(1876),
            ..Default::default()
        });
        assert_eq!(travel.len(), 1);
        assert_eq!(travel[0].event_type, "residence");
        assert_eq!(travel[0].time_surface.as_deref(), Some("1831"));
    }

    #[test]
    fn french_settles_at_brussels() {
        let raws = TravelResidenceExtractor.extract(&ExtractorInput {
            text: "Il se fixe à Bruxelles en 1864.".into(),
            page_title: Some("Charles Baudelaire".into()),
            subject_label: Some("Charles Baudelaire".into()),
            document_type: "article".into(),
            subject_death_year: Some(1867),
            ..Default::default()
        });
        assert!(
            raws.iter().any(|r| r.event_type == "residence"
                && r.place_surface.as_deref() == Some("Bruxelles")
                && r.time_surface.as_deref() == Some("1864")),
            "should find Bruxelles residence: {raws:?}"
        );
    }

    #[test]
    fn french_return_to_paris() {
        let raws = TravelResidenceExtractor.extract(&ExtractorInput {
            text: "En juillet 1866, on le ramène à Paris.".into(),
            page_title: Some("Charles Baudelaire".into()),
            subject_label: Some("Charles Baudelaire".into()),
            document_type: "article".into(),
            subject_death_year: Some(1867),
            ..Default::default()
        });
        assert!(
            raws.iter().any(|r| r.event_type == "arrival"
                && r.predicate == "returned_to"
                && r.place_surface.as_deref() == Some("Paris")
                && r.time_surface.as_deref() == Some("1866")),
            "should find return to Paris: {raws:?}"
        );
    }

    #[test]
    fn french_leaves_bordeaux() {
        let raws = TravelResidenceExtractor.extract(&ExtractorInput {
            text: "Le Paquebot des Mers du Sud quitte Bordeaux le 9 juin 1841.".into(),
            page_title: Some("Charles Baudelaire".into()),
            subject_label: Some("Charles Baudelaire".into()),
            document_type: "article".into(),
            subject_death_year: Some(1867),
            ..Default::default()
        });
        assert!(
            raws.iter().any(|r| r.event_type == "departure"
                && r.time_surface.as_deref() == Some("1841")),
            "should find departure in 1841: {raws:?}"
        );
    }

    #[test]
    fn french_stopover_mauritius() {
        let raws = TravelResidenceExtractor.extract(&ExtractorInput {
            text: "En septembre 1841, une violente tempête oblige le capitaine à faire escale à Port-Louis.".into(),
            page_title: Some("Charles Baudelaire".into()),
            subject_label: Some("Charles Baudelaire".into()),
            document_type: "article".into(),
            subject_death_year: Some(1867),
            ..Default::default()
        });
        assert!(
            raws.iter().any(|r| r.event_type == "arrival"
                && r.place_surface.as_deref() == Some("Port-Louis")
                && r.time_surface.as_deref() == Some("1841")),
            "should find stopover at Port-Louis: {raws:?}"
        );
    }

    #[test]
    fn french_visit_victor_hugo() {
        let raws = TravelResidenceExtractor.extract(&ExtractorInput {
            text: "Il rend plusieurs visites à Victor Hugo, exilé politique volontaire.".into(),
            page_title: Some("Charles Baudelaire".into()),
            subject_label: Some("Charles Baudelaire".into()),
            document_type: "article".into(),
            subject_death_year: Some(1867),
            ..Default::default()
        });
        assert!(
            raws.iter().any(|r| r.event_type == "meeting" && r.predicate == "visited"),
            "should find visit/meeting: {raws:?}"
        );
    }

    // English pattern tests
    #[test]
    fn english_traveled_to() {
        let raws = TravelResidenceExtractor.extract(&ExtractorInput {
            text: "In 1841, Baudelaire traveled to Mauritius on a merchant ship.".into(),
            page_title: Some("Charles Baudelaire".into()),
            subject_label: Some("Charles Baudelaire".into()),
            document_type: "article".into(),
            subject_death_year: Some(1867),
            ..Default::default()
        });
        assert!(
            raws.iter().any(|r| r.event_type == "travel"
                && r.time_surface.as_deref() == Some("1841")),
            "should find travel in 1841: {raws:?}"
        );
    }

    #[test]
    fn english_sailed_to() {
        let raws = TravelResidenceExtractor.extract(&ExtractorInput {
            text: "He sailed to Réunion Island in 1841.".into(),
            page_title: Some("Charles Baudelaire".into()),
            subject_label: Some("Charles Baudelaire".into()),
            document_type: "article".into(),
            subject_death_year: Some(1867),
            ..Default::default()
        });
        assert!(
            raws.iter().any(|r| r.event_type == "travel"
                && r.time_surface.as_deref() == Some("1841")),
            "should find travel in 1841: {raws:?}"
        );
    }

    #[test]
    fn english_moved_to() {
        let raws = TravelResidenceExtractor.extract(&ExtractorInput {
            text: "In 1864, Baudelaire moved to Brussels.".into(),
            page_title: Some("Charles Baudelaire".into()),
            subject_label: Some("Charles Baudelaire".into()),
            document_type: "article".into(),
            subject_death_year: Some(1867),
            ..Default::default()
        });
        assert!(
            raws.iter().any(|r| r.event_type == "residence"
                && r.place_surface.as_deref() == Some("Brussels")
                && r.time_surface.as_deref() == Some("1864")),
            "should find Brussels residence: {raws:?}"
        );
    }

    #[test]
    fn english_returned_to() {
        let raws = TravelResidenceExtractor.extract(&ExtractorInput {
            text: "He returned to Paris in 1842.".into(),
            page_title: Some("Charles Baudelaire".into()),
            subject_label: Some("Charles Baudelaire".into()),
            document_type: "article".into(),
            subject_death_year: Some(1867),
            ..Default::default()
        });
        assert!(
            raws.iter().any(|r| r.event_type == "arrival"
                && r.predicate == "returned_to"
                && r.place_surface.as_deref() == Some("Paris")
                && r.time_surface.as_deref() == Some("1842")),
            "should find return to Paris: {raws:?}"
        );
    }

    #[test]
    fn english_visited() {
        let raws = TravelResidenceExtractor.extract(&ExtractorInput {
            text: "He visited Belgium in 1864.".into(),
            page_title: Some("Charles Baudelaire".into()),
            subject_label: Some("Charles Baudelaire".into()),
            document_type: "article".into(),
            subject_death_year: Some(1867),
            ..Default::default()
        });
        assert!(
            raws.iter().any(|r| r.event_type == "meeting"
                && r.time_surface.as_deref() == Some("1864")),
            "should find visit in 1864: {raws:?}"
        );
    }

    #[test]
    fn english_set_sail() {
        let raws = TravelResidenceExtractor.extract(&ExtractorInput {
            text: "The ship set sail for Calcutta in June 1841.".into(),
            page_title: Some("Charles Baudelaire".into()),
            subject_label: Some("Charles Baudelaire".into()),
            document_type: "article".into(),
            subject_death_year: Some(1867),
            ..Default::default()
        });
        assert!(
            raws.iter().any(|r| r.event_type == "departure"
                && r.time_surface.as_deref() == Some("1841")),
            "should find departure in 1841: {raws:?}"
        );
    }

    #[test]
    fn english_arrived_in() {
        let raws = TravelResidenceExtractor.extract(&ExtractorInput {
            text: "He arrived in Paris, 1842.".into(),
            page_title: Some("Charles Baudelaire".into()),
            subject_label: Some("Charles Baudelaire".into()),
            document_type: "article".into(),
            subject_death_year: Some(1867),
            ..Default::default()
        });
        assert!(
            raws.iter().any(|r| r.event_type == "arrival"
                && r.place_surface.as_deref() == Some("Paris")
                && r.time_surface.as_deref() == Some("1842")),
            "should find arrival in Paris: {raws:?}"
        );
    }

    #[test]
    fn english_settled_in() {
        let raws = TravelResidenceExtractor.extract(&ExtractorInput {
            text: "He settled at Brussels, 1864.".into(),
            page_title: Some("Charles Baudelaire".into()),
            subject_label: Some("Charles Baudelaire".into()),
            document_type: "article".into(),
            subject_death_year: Some(1867),
            ..Default::default()
        });
        assert!(
            raws.iter().any(|r| r.event_type == "residence"
                && r.place_surface.as_deref() == Some("Brussels")
                && r.time_surface.as_deref() == Some("1864")),
            "should find residence in Brussels: {raws:?}"
        );
    }
}
