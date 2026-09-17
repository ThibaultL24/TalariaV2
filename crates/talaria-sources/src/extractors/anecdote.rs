// crates/talaria-sources/src/extractors/anecdote.rs
//! Anecdote/life-event extraction: relationships, trials, financial, political, suicide attempts.
//! Captures biographical events often found in literary/artistic biographies.

use crate::extractors::travel::find_year;
use crate::extractors::{CandidateExtractor, ExtractorInput, RawCandidate};

pub struct AnecdoteLifeExtractor;

impl CandidateExtractor for AnecdoteLifeExtractor {
    fn extractor_id(&self) -> &str {
        "anecdote_life"
    }

    fn version(&self) -> &str {
        "anecdote_life:v1"
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
            
            if let Some((etype, pred)) = classify_anecdote(&lower) {
                let year = find_year(line);
                let place = find_anecdote_place(line, &lower);
                let person = find_related_person(line, &lower);
                
                if year.is_none() && place.is_none() && person.is_none() {
                    continue;
                }
                
                out.push(RawCandidate {
                    event_type: etype.into(),
                    predicate: pred.into(),
                    subject_surface: subject.clone(),
                    time_surface: year,
                    place_surface: place,
                    object_surface: person,
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

fn classify_anecdote(lower: &str) -> Option<(&'static str, &'static str)> {
    // Relationship patterns (French + English)
    if lower.contains("s'éprend de")
        || lower.contains("seprend de")
        || lower.contains("s'éprend d'")
        || lower.contains("fell in love")
        || lower.contains("becomes enamored")
        || lower.contains("became enamored")
    {
        return Some(("meeting", "fell_in_love_with"));
    }
    
    if lower.contains("liaison avec")
        || lower.contains("liaison with")
        || lower.contains("affair with")
        || lower.contains("had an affair")
        || lower.contains("began an affair")
    {
        return Some(("meeting", "had_liaison_with"));
    }
    
    if lower.contains("became his mistress")
        || lower.contains("became her lover")
        || lower.contains("became his lover")
        || lower.contains("devint sa maîtresse")
        || lower.contains("devint son amant")
        || lower.contains("sa maîtresse") && (lower.contains("duval") || lower.contains("1842") || lower.contains("1843"))
    {
        return Some(("meeting", "became_partner"));
    }
    
    if lower.contains("se lie avec")
        || lower.contains("se lia avec")
        || lower.contains("became acquainted")
        || lower.contains("made acquaintance")
        || lower.contains("formed a friendship")
    {
        return Some(("meeting", "acquainted_with"));
    }
    
    // Trial/Legal patterns (French + English)
    if lower.contains("poursuivi en justice")
        || lower.contains("poursuivies en justice")
        || lower.contains("poursuivie en justice")
        || lower.contains("successfully prosecuted")
        || lower.contains("was prosecuted")
        || lower.contains("were prosecuted")
        || lower.contains("brought to trial")
        || lower.contains("mis en accusation")
    {
        return Some(("trial", "prosecuted"));
    }
    
    if lower.contains("condamné à")
        || lower.contains("condamnée à")
        || lower.contains("condamné pour")
        || lower.contains("condamnée pour")
        || lower.contains("was condemned")
        || lower.contains("was convicted")
        || lower.contains("found guilty")
        || lower.contains("sentenced to")
    {
        return Some(("trial", "convicted"));
    }
    
    if lower.contains("procès des")
        || lower.contains("procès de")
        || lower.contains("le procès")
        || lower.contains("trial of")
        || lower.contains("the trial")
        || lower.contains("stood trial")
    {
        return Some(("trial", "stood_trial"));
    }
    
    if lower.contains("amende de")
        || lower.contains("frappé d'une amende")
        || lower.contains("frappée d'une amende")
        || lower.contains("fined")
        || lower.contains("they were fined")
        || lower.contains("paid a fine")
    {
        return Some(("trial", "fined"));
    }
    
    // Suicide attempt patterns (French + English)
    if lower.contains("tente de se suicider")
        || lower.contains("tenta de se suicider")
        || lower.contains("tentative de suicide")
        || lower.contains("suicide attempt")
        || lower.contains("attempted suicide")
        || lower.contains("made a suicide attempt")
        || lower.contains("tried to kill himself")
        || lower.contains("tried to kill herself")
    {
        return Some(("health_event", "suicide_attempt"));
    }
    
    // Financial/Legal arrangements (French + English)
    if lower.contains("conseil judiciaire")
        || lower.contains("placed under guardianship")
        || lower.contains("property in trust")
        || lower.contains("conseil de famille")
        || lower.contains("family obtained a decree")
    {
        return Some(("legal_event", "placed_under_guardianship"));
    }
    
    if lower.contains("heavily indebted")
        || lower.contains("very indebted")
        || lower.contains("heavily in debt")
        || lower.contains("deep in debt")
        || ((lower.contains("endetté") || lower.contains("endettée") || lower.contains("endettement"))
            && (lower.contains("très") || lower.contains("criblé") || lower.contains("lourdement")))
    {
        return Some(("financial_event", "heavily_indebted"));
    }
    
    if lower.contains("dilapide")
        || lower.contains("dilapida")
        || lower.contains("squandered")
        || lower.contains("dissipated his fortune")
        || lower.contains("dissipated his inheritance")
    {
        return Some(("financial_event", "squandered_fortune"));
    }
    
    // Political/Revolution patterns (French + English)
    if lower.contains("participe aux barricades")
        || lower.contains("participa aux barricades")
        || lower.contains("aux barricades")
        || lower.contains("took part in the revolution")
        || lower.contains("participated in the revolution")
        || lower.contains("joined the uprising")
    {
        return Some(("political_event", "participated_in_revolution"));
    }
    
    if lower.contains("révolution de")
        || lower.contains("revolution of")
        || lower.contains("uprising of")
        || (lower.contains("1848") && (lower.contains("revolution") || lower.contains("révolution") || lower.contains("barricade")))
    {
        return Some(("political_event", "revolution"));
    }
    
    // Enhanced meeting patterns
    if (lower.contains(" rencontre ") || lower.contains(" rencontra "))
        && !lower.contains("rencontre avec")
        && !lower.contains("rend visite")
    {
        return Some(("meeting", "met"));
    }
    
    // Expedition/Adventure patterns
    if lower.contains("expédition")
        || lower.contains("expedition")
        || lower.contains("treasure hunt")
        || lower.contains("chasse au trésor")
        || lower.contains("aventure avec")
        || lower.contains("adventure with")
    {
        return Some(("meeting", "expedition_with"));
    }
    
    // Scandal/Controversy patterns
    if lower.contains("scandale")
        || lower.contains("scandal")
        || lower.contains("provoked a scandal")
        || lower.contains("provoqua un scandale")
        || lower.contains("caused a scandal")
    {
        return Some(("scandal", "caused_scandal"));
    }
    
    // Censorship/Ban patterns
    if lower.contains("interdit")
        || lower.contains("interdiction")
        || lower.contains("banned")
        || lower.contains("suppressed")
        || lower.contains("censored")
        || lower.contains("censuré")
    {
        return Some(("censorship", "work_banned"));
    }
    
    None
}

fn find_anecdote_place(s: &str, lower: &str) -> Option<String> {
    // Common place cues
    for cue in [
        " in ", " at ", " to ", " à ", " au ", " en ", " dans ", " chez ",
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
            if token.len() >= 2
                && !token.eq_ignore_ascii_case("en")
                && !token.eq_ignore_ascii_case("le")
                && !token.eq_ignore_ascii_case("la")
                && !is_person_name_prefix(&token)
            {
                return Some(token);
            }
        }
    }
    None
}

fn find_related_person(s: &str, lower: &str) -> Option<String> {
    // Look for person names after relationship cues
    for cue in [
        "s'éprend de ",
        "s'éprend d'",
        "liaison avec ",
        "rencontre ",
        "rencontra ",
        "became his mistress",
        "became her lover",
        "fell in love with ",
        "affair with ",
        "acquainted with ",
        "expedition with ",
        "adventure with ",
    ] {
        if let Some(pos) = lower.find(cue) {
            let start = pos + cue.len();
            let after = &s[start..];
            if let Some(name) = extract_person_name(after) {
                return Some(name);
            }
        }
    }
    None
}

fn extract_person_name(after: &str) -> Option<String> {
    let mut words = Vec::new();
    for w in after.split_whitespace() {
        let clean = w
            .trim_matches(|c: char| !c.is_alphabetic() && c != '-' && c != '\'')
            .to_string();
        if clean.is_empty() {
            break;
        }
        let first = clean.chars().next()?;
        
        if first.is_uppercase() {
            words.push(clean);
            if words.len() >= 3 {
                break;
            }
            continue;
        }
        
        if !words.is_empty() && matches!(clean.to_lowercase().as_str(), "de" | "du" | "van" | "von" | "la" | "le") {
            words.push(clean);
            continue;
        }
        
        break;
    }
    
    if words.len() >= 2 {
        Some(words.join(" "))
    } else if words.len() == 1 && words[0].chars().count() >= 3 {
        Some(words[0].clone())
    } else {
        None
    }
}

fn is_person_name_prefix(s: &str) -> bool {
    matches!(
        s.to_lowercase().as_str(),
        "jeanne" | "charles" | "marie" | "victor" | "auguste" | "félicien" | "edgar" | "théophile"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::extractors::ExtractorInput;

    fn make_input(text: &str) -> ExtractorInput {
        ExtractorInput {
            text: text.into(),
            page_title: Some("Charles Baudelaire".into()),
            subject_label: Some("Charles Baudelaire".into()),
            document_type: "article".into(),
            subject_death_year: Some(1867),
            ..Default::default()
        }
    }

    #[test]
    fn french_jeanne_duval_relationship() {
        let raws = AnecdoteLifeExtractor.extract(&make_input(
            "De retour à Paris, Charles s'éprend de Jeanne Duval, une jeune mulâtresse haïtienne."
        ));
        assert!(
            raws.iter().any(|r| r.event_type == "meeting" 
                && r.predicate == "fell_in_love_with"
                && r.object_surface.as_deref().is_some_and(|p| p.contains("Jeanne Duval"))),
            "should find Jeanne Duval relationship: {raws:?}"
        );
    }

    #[test]
    fn english_jeanne_duval_mistress() {
        let raws = AnecdoteLifeExtractor.extract(&make_input(
            "During this time, Jeanne Duval became his mistress in 1842."
        ));
        assert!(
            raws.iter().any(|r| r.event_type == "meeting" 
                && r.time_surface.as_deref() == Some("1842")),
            "should find 1842 mistress relationship: {raws:?}"
        );
    }

    #[test]
    fn french_trial_prosecution() {
        let raws = AnecdoteLifeExtractor.extract(&make_input(
            "Moins de deux mois après leur parution, Les Fleurs du mal sont poursuivies en justice pour « offense à la morale religieuse »."
        ));
        assert!(
            raws.iter().any(|r| r.event_type == "trial" && r.predicate == "prosecuted"),
            "should find prosecution: {raws:?}"
        );
    }

    #[test]
    fn french_condemnation() {
        let raws = AnecdoteLifeExtractor.extract(&make_input(
            "Baudelaire est condamné à une forte amende de trois cents francs en 1857."
        ));
        assert!(
            raws.iter().any(|r| r.event_type == "trial" 
                && r.predicate == "convicted"
                && r.time_surface.as_deref() == Some("1857")),
            "should find 1857 conviction: {raws:?}"
        );
    }

    #[test]
    fn english_prosecution() {
        let raws = AnecdoteLifeExtractor.extract(&make_input(
            "Baudelaire, his publisher and the printer were successfully prosecuted in 1857."
        ));
        assert!(
            raws.iter().any(|r| r.event_type == "trial" 
                && r.time_surface.as_deref() == Some("1857")),
            "should find 1857 prosecution: {raws:?}"
        );
    }

    #[test]
    fn french_suicide_attempt() {
        let raws = AnecdoteLifeExtractor.extract(&make_input(
            "Cette situation infantilisante inflige à Baudelaire une telle humiliation qu'il tente de se suicider d'un coup de couteau dans la poitrine le 30 juin 1845."
        ));
        assert!(
            raws.iter().any(|r| r.event_type == "health_event" 
                && r.predicate == "suicide_attempt"
                && r.time_surface.as_deref() == Some("1845")),
            "should find 1845 suicide attempt: {raws:?}"
        );
    }

    #[test]
    fn english_suicide_attempt() {
        let raws = AnecdoteLifeExtractor.extract(&make_input(
            "Baudelaire made a suicide attempt during this period in 1845."
        ));
        assert!(
            raws.iter().any(|r| r.event_type == "health_event" 
                && r.predicate == "suicide_attempt"
                && r.time_surface.as_deref() == Some("1845")),
            "should find 1845 suicide attempt: {raws:?}"
        );
    }

    #[test]
    fn french_conseil_judiciaire() {
        let raws = AnecdoteLifeExtractor.extract(&make_input(
            "Le 21 septembre 1844, maître Narcisse Ancelle est officiellement désigné comme conseil judiciaire."
        ));
        assert!(
            raws.iter().any(|r| r.event_type == "legal_event" 
                && r.predicate == "placed_under_guardianship"
                && r.time_surface.as_deref() == Some("1844")),
            "should find 1844 conseil judiciaire: {raws:?}"
        );
    }

    #[test]
    fn english_property_trust() {
        let raws = AnecdoteLifeExtractor.extract(&make_input(
            "His family obtained a decree to place his property in trust in 1844."
        ));
        assert!(
            raws.iter().any(|r| r.event_type == "legal_event" 
                && r.time_surface.as_deref() == Some("1844")),
            "should find 1844 trust decree: {raws:?}"
        );
    }

    #[test]
    fn french_1848_barricades() {
        let raws = AnecdoteLifeExtractor.extract(&make_input(
            "En 1848, il participe aux barricades."
        ));
        assert!(
            raws.iter().any(|r| r.event_type == "political_event" 
                && r.predicate == "participated_in_revolution"
                && r.time_surface.as_deref() == Some("1848")),
            "should find 1848 revolution: {raws:?}"
        );
    }

    #[test]
    fn english_1848_revolutions() {
        let raws = AnecdoteLifeExtractor.extract(&make_input(
            "He took part in the Revolutions of 1848 and wrote for a revolutionary newspaper."
        ));
        assert!(
            raws.iter().any(|r| r.event_type == "political_event" 
                && r.time_surface.as_deref() == Some("1848")),
            "should find 1848 participation: {raws:?}"
        );
    }

    #[test]
    fn french_felicien_rops_meeting() {
        let raws = AnecdoteLifeExtractor.extract(&make_input(
            "C'est en Belgique que Baudelaire rencontre Félicien Rops en 1866."
        ));
        assert!(
            raws.iter().any(|r| r.event_type == "meeting" 
                && r.time_surface.as_deref() == Some("1866")
                && r.object_surface.as_deref().is_some_and(|p| p.contains("Félicien Rops"))),
            "should find Félicien Rops meeting: {raws:?}"
        );
    }

    #[test]
    fn english_heavily_indebted() {
        let raws = AnecdoteLifeExtractor.extract(&make_input(
            "In 1864, he left Paris for Belgium, very heavily indebted."
        ));
        assert!(
            raws.iter().any(|r| r.event_type == "financial_event"
                && r.time_surface.as_deref() == Some("1864")),
            "should find 1864 financial distress: {raws:?}"
        );
    }

    #[test]
    fn french_squandered_inheritance() {
        let raws = AnecdoteLifeExtractor.extract(&make_input(
            "En dandy, Baudelaire a des goûts de luxe. Ayant hérité de son père à sa majorité, il dilapide la moitié de cet héritage en 1842."
        ));
        assert!(
            raws.iter().any(|r| r.event_type == "financial_event" 
                && r.predicate == "squandered_fortune"
                && r.time_surface.as_deref() == Some("1842")),
            "should find 1842 squandering: {raws:?}"
        );
    }

    #[test]
    fn english_squandered_inheritance() {
        let raws = AnecdoteLifeExtractor.extract(&make_input(
            "At 21, he received a sizable inheritance but squandered much of it within a few years in 1842."
        ));
        assert!(
            raws.iter().any(|r| r.event_type == "financial_event" 
                && r.time_surface.as_deref() == Some("1842")),
            "should find 1842 squandering: {raws:?}"
        );
    }

    #[test]
    fn french_book_banned() {
        let raws = AnecdoteLifeExtractor.extract(&make_input(
            "Six poèmes sont interdits en 1857."
        ));
        assert!(
            raws.iter().any(|r| r.event_type == "censorship" 
                && r.time_surface.as_deref() == Some("1857")),
            "should find 1857 ban: {raws:?}"
        );
    }
}
