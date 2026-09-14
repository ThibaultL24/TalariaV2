// crates/talaria-sources/src/extractors/education.rs
//! Education and life-event extraction: enrollment, graduation, burial, collapse.

use crate::extractors::travel::find_year;
use crate::extractors::{CandidateExtractor, ExtractorInput, RawCandidate};

pub struct EducationLifeExtractor;

impl CandidateExtractor for EducationLifeExtractor {
    fn extractor_id(&self) -> &str {
        "education_life"
    }

    fn version(&self) -> &str {
        "education_life:v1"
    }

    fn extract(&self, input: &ExtractorInput) -> Vec<RawCandidate> {
        let subject = input.effective_subject();
        let mut out = Vec::new();
        for (i, unit) in crate::extractors::split_prose_units(&input.text)
            .into_iter()
            .enumerate()
        {
            let line = unit.as_str();
            let lower = line.to_lowercase();
            
            if let Some((etype, pred)) = classify_education_life(&lower) {
                let year = find_year(line);
                let place = find_education_place(line, &lower);
                
                if year.is_none() && place.is_none() {
                    continue;
                }
                
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
        }
        out
    }
}

fn classify_education_life(lower: &str) -> Option<(&'static str, &'static str)> {
    // Education patterns (French + English)
    if lower.contains("inscrit à")
        || lower.contains("inscrit au")
        || lower.contains("inscrite à")
        || lower.contains("inscrite au")
        || lower.contains("s'inscrit")
        || lower.contains("est inscrit")
        || lower.contains("est inscrite")
        || lower.contains("enrolled at")
        || lower.contains("enrolled in")
    {
        return Some(("education", "enrolled_at"));
    }

    if lower.contains("suit les cours")
        || lower.contains("suivait les cours")
        || lower.contains("suivre les cours")
        || lower.contains("attends courses")
    {
        return Some(("education", "studied_at"));
    }

    if lower.contains("pensionnaire au collège")
        || lower.contains("pensionnaire au lycée")
        || lower.contains("pensionnaire à")
        || lower.contains("boarding student")
    {
        return Some(("education", "boarded_at"));
    }

    if lower.contains("au collège")
        || lower.contains("au lycée")
        || lower.contains("collège royal")
        || lower.contains("collège de")
        || lower.contains("lycée de")
        || lower.contains("lycée louis")
        || lower.contains("lycée saint")
        || lower.contains("lycée henri")
        || lower.contains("at school")
        || lower.contains("at college")
        || lower.contains("at the lycée")
        || lower.contains("at the college")
    {
        let has_edu_verb = lower.contains("suit")
            || lower.contains("inscrit")
            || lower.contains("étudie")
            || lower.contains("étudi")
            || lower.contains("redoubl")
            || lower.contains("enters")
            || lower.contains("attends")
            || lower.contains("enrolled");
        if has_edu_verb {
            return Some(("education", "studied_at"));
        }
    }

    if lower.contains("baccalauréat")
        || lower.contains("baccalaureat")
        || lower.contains("passe son bac")
        || lower.contains("obtient son bac")
        || lower.contains("reçu au bac")
    {
        return Some(("education", "graduated_from"));
    }

    if lower.contains("renvoyé du lycée")
        || lower.contains("renvoyé du collège")
        || lower.contains("renvoyée du")
        || lower.contains("expelled from")
    {
        return Some(("education", "expelled_from"));
    }

    if lower.contains("concours général")
        || lower.contains("prix de vers")
        || lower.contains("accessit")
    {
        return Some(("education", "received_prize"));
    }

    // Burial patterns
    if lower.contains("inhumé au")
        || lower.contains("inhumée au")
        || lower.contains("inhumé à")
        || lower.contains("inhumée à")
        || lower.contains("enterré au")
        || lower.contains("enterrée au")
        || lower.contains("enterré à")
        || lower.contains("enterrée à")
        || lower.contains("buried at")
        || lower.contains("buried in")
        || lower.contains("cimetière")
        || lower.contains("cemetery")
    {
        return Some(("burial", "buried_at"));
    }

    // Health/collapse events with place
    if lower.contains("perd connaissance")
        || lower.contains("perdit connaissance")
        || lower.contains("s'effondre")
        || lower.contains("collapsed")
        || lower.contains("loses consciousness")
    {
        return Some(("health_event", "collapsed_at"));
    }

    // Admission to institution (hospital, maison de santé)
    if (lower.contains("admis dans") || lower.contains("admise dans"))
        && (lower.contains("maison de santé")
            || lower.contains("hôpital")
            || lower.contains("hospital")
            || lower.contains("clinique"))
    {
        return Some(("residence", "admitted_to"));
    }

    None
}

fn find_education_place(s: &str, lower: &str) -> Option<String> {
    // Try specific institution patterns first
    for pattern in [
        "collège royal de ",
        "collège de ",
        "collège ",
        "lycée ",
        "lycée de ",
        "pension ",
        "école ",
        "université de ",
        "university of ",
        "school of ",
        "at the ",
        "at ",
    ] {
        if let Some(pos) = lower.find(pattern) {
            let after = &s[pos + pattern.len()..];
            if let Some(place) = take_institution_name(after) {
                return Some(place);
            }
        }
    }

    // French preposition patterns for places
    for cue in [
        " à la ",
        " au ",
        " à ",
        " dans la ",
        " dans le ",
        " dans ",
        " chez ",
        " en ",
    ] {
        if let Some(pos) = lower.rfind(cue) {
            let after = &s[pos + cue.len()..];
            let raw = after
                .split(|c: char| c == '.' || c.is_ascii_digit() || c == ',')
                .next()?
                .trim()
                .trim_matches(|c: char| !c.is_alphabetic() && c != ' ' && c != '-' && c != '\'');
            let token = raw
                .trim_end_matches(" en")
                .trim_end_matches(" in")
                .trim_start_matches("l'")
                .trim_start_matches("l'")
                .trim()
                .to_string();
            if token.len() >= 2 && !token.eq_ignore_ascii_case("en") && !token.eq_ignore_ascii_case("le") {
                return Some(token);
            }
        }
    }

    // Cemetery specific
    if lower.contains("cimetière") || lower.contains("cemetery") {
        for cue in ["cimetière du ", "cimetière de ", "cemetery of ", "cimetière "] {
            if let Some(pos) = lower.find(cue) {
                let after = &s[pos + cue.len()..];
                if let Some(name) = take_institution_name(after) {
                    return Some(format!("Cimetière {}", name));
                }
            }
        }
    }

    None
}

fn take_institution_name(after: &str) -> Option<String> {
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
        
        // Always take capitalized words
        if first.is_uppercase() {
            words.push(clean);
            if words.len() >= 4 {
                break;
            }
            continue;
        }
        
        // French articles/prepositions in institution names
        if !words.is_empty()
            && matches!(
                lower.as_str(),
                "de" | "du" | "des" | "le" | "la" | "les" | "l" | "d" | "-"
            )
        {
            words.push(clean);
            continue;
        }
        
        break;
    }
    
    // Remove trailing articles
    while words.last().is_some_and(|w| {
        matches!(
            w.to_lowercase().as_str(),
            "de" | "du" | "des" | "le" | "la" | "les"
        )
    }) {
        words.pop();
    }
    
    let result = words.join(" ");
    if result.len() >= 2 {
        Some(result)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::extractors::ExtractorInput;

    #[test]
    fn french_college_enrollment() {
        let raws = EducationLifeExtractor.extract(&ExtractorInput {
            text: "En 1831, le jeune Baudelaire est inscrit à la pension Delorme et suit les cours de sixième au collège royal de Lyon.".into(),
            page_title: Some("Charles Baudelaire".into()),
            subject_label: Some("Charles Baudelaire".into()),
            document_type: "article".into(),
            subject_death_year: Some(1867),
            ..Default::default()
        });
        assert!(!raws.is_empty(), "should extract education events");
        assert!(
            raws.iter().any(|r| r.event_type == "education" && r.time_surface.as_deref() == Some("1831")),
            "should find 1831 education: {raws:?}"
        );
    }

    #[test]
    fn french_lycee_boarding() {
        let raws = EducationLifeExtractor.extract(&ExtractorInput {
            text: "Alors âgé de quatorze ans, Charles est inscrit comme pensionnaire au collège Louis-le-Grand.".into(),
            page_title: Some("Charles Baudelaire".into()),
            subject_label: Some("Charles Baudelaire".into()),
            document_type: "article".into(),
            subject_death_year: Some(1867),
            ..Default::default()
        });
        assert!(
            raws.iter().any(|r| r.event_type == "education" 
                && r.place_surface.as_deref().is_some_and(|p| p.contains("Louis-le-Grand"))),
            "should find Louis-le-Grand: {raws:?}"
        );
    }

    #[test]
    fn french_expelled_from_school() {
        let raws = EducationLifeExtractor.extract(&ExtractorInput {
            text: "Renvoyé du lycée Louis-le-Grand en avril 1839.".into(),
            page_title: Some("Charles Baudelaire".into()),
            subject_label: Some("Charles Baudelaire".into()),
            document_type: "article".into(),
            subject_death_year: Some(1867),
            ..Default::default()
        });
        assert!(
            raws.iter().any(|r| r.event_type == "education" 
                && r.predicate == "expelled_from"
                && r.time_surface.as_deref() == Some("1839")),
            "should find expulsion in 1839: {raws:?}"
        );
    }

    #[test]
    fn french_baccalaureat() {
        let raws = EducationLifeExtractor.extract(&ExtractorInput {
            text: "Il passe son baccalauréat au lycée Saint-Louis en fin d'année.".into(),
            page_title: Some("Charles Baudelaire".into()),
            subject_label: Some("Charles Baudelaire".into()),
            document_type: "article".into(),
            subject_death_year: Some(1867),
            ..Default::default()
        });
        assert!(
            raws.iter().any(|r| r.event_type == "education" 
                && r.predicate == "graduated_from"
                && r.place_surface.as_deref().is_some_and(|p| p.contains("Saint-Louis"))),
            "should find baccalauréat at Saint-Louis: {raws:?}"
        );
    }

    #[test]
    fn french_burial_at_cemetery() {
        let raws = EducationLifeExtractor.extract(&ExtractorInput {
            text: "Le même jour, il est inhumé au cimetière du Montparnasse.".into(),
            page_title: Some("Charles Baudelaire".into()),
            subject_label: Some("Charles Baudelaire".into()),
            document_type: "article".into(),
            subject_death_year: Some(1867),
            ..Default::default()
        });
        assert!(
            raws.iter().any(|r| r.event_type == "burial" 
                && r.place_surface.as_deref().is_some_and(|p| p.contains("Montparnasse"))),
            "should find burial at Montparnasse: {raws:?}"
        );
    }

    #[test]
    fn french_collapse_at_church() {
        let raws = EducationLifeExtractor.extract(&ExtractorInput {
            text: "Lors d'une visite à l'église Saint-Loup de Namur, le 15 mars 1866, Baudelaire perd connaissance.".into(),
            page_title: Some("Charles Baudelaire".into()),
            subject_label: Some("Charles Baudelaire".into()),
            document_type: "article".into(),
            subject_death_year: Some(1867),
            ..Default::default()
        });
        assert!(
            raws.iter().any(|r| r.event_type == "health_event" 
                && r.time_surface.as_deref() == Some("1866")
                && r.place_surface.as_deref().is_some_and(|p| p.contains("Namur"))),
            "should find collapse at Namur in 1866: {raws:?}"
        );
    }

    #[test]
    fn hospital_admission() {
        let raws = EducationLifeExtractor.extract(&ExtractorInput {
            text: "En juillet 1866, il est admis dans la maison de santé du docteur Duval.".into(),
            page_title: Some("Charles Baudelaire".into()),
            subject_label: Some("Charles Baudelaire".into()),
            document_type: "article".into(),
            subject_death_year: Some(1867),
            ..Default::default()
        });
        assert!(
            raws.iter().any(|r| r.event_type == "residence" 
                && r.predicate == "admitted_to"
                && r.time_surface.as_deref() == Some("1866")),
            "should find hospital admission in 1866: {raws:?}"
        );
    }
}
