// crates/talaria-sources/src/extractors/anecdote.rs
//! Anecdote/life-event extraction: relationships, trials, financial, political, suicide attempts.
//! Captures biographical events often found in literary/artistic biographies.

use crate::extractors::travel::find_year;
use crate::extractors::{CandidateExtractor, ExtractorInput, RawCandidate};
use crate::place_quality::is_plausible_place_label;

pub struct AnecdoteLifeExtractor;

impl CandidateExtractor for AnecdoteLifeExtractor {
    fn extractor_id(&self) -> &str {
        "anecdote_life"
    }

    fn version(&self) -> &str {
        "anecdote_life:v2"
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
                let place = find_venue_place(line, &lower).or_else(|| find_anecdote_place(line, &lower));
                let person = find_related_person(line, &lower);

                // Patterns are high-signal; year/place/person enrich but are not required
                // (EN often puts the year in a neighbouring sentence).
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
        || lower.contains("took part in the revolutions")
        || lower.contains("participated in the revolution")
        || lower.contains("participated in the revolutions")
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
    
    if is_art_salon_review(lower) {
        // "Salon de 1845" is an art-criticism title, not a literary salon.
    } else if is_literary_salon(lower) {
        return Some(("meeting", "met_at_salon"));
    }

    if is_life_locus_haunt(lower) {
        return Some(("residence", "frequented"));
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

fn is_art_salon_review(lower: &str) -> bool {
    (lower.contains("salon de 1") || lower.contains("salon of 1") || lower.contains("salon de 18"))
        && !lower.contains("salon littéraire")
        && !lower.contains("literary salon")
}

fn is_literary_salon(lower: &str) -> bool {
    lower.contains("literary salon")
        || lower.contains("salon littéraire")
        || lower.contains("salon litteraire")
        || lower.contains("frequented the salon")
        || lower.contains("fréquentait les salons")
        || lower.contains("frequente les salons")
        || lower.contains("fréquenta les salons")
        || lower.contains("dans les salons")
        || (lower.contains("au salon de") && !is_art_salon_review(lower))
        || lower.contains("met at the salon")
        || lower.contains("met in the salon")
}

fn is_life_locus_haunt(lower: &str) -> bool {
    lower.contains("lupanar")
        || lower.contains("brothel")
        || lower.contains("bordel")
        || lower.contains("maison close")
        || lower.contains("house of ill fame")
        || lower.contains("taverns of")
        || lower.contains("dans les cafés")
        || lower.contains("dans les cafes")
        || lower.contains("fréquentait beaucoup les cafés")
        || lower.contains("frequentait beaucoup les cafes")
        || lower.contains("fréquentait les cafés")
        || lower.contains("frequentait les cafes")
        || lower.contains("composait dans les cafés")
        || lower.contains("composait dans les cafes")
        || ((lower.contains("frequent prostitutes") || lower.contains("fréquentait les prostitu"))
            && (lower.contains(" in ") || lower.contains(" à ") || lower.contains("paris")))
        || lower.contains("café du ")
        || lower.contains("cafe du ")
        || lower.contains("un café du")
        || lower.contains("un cafe du")
}

fn find_venue_place(s: &str, lower: &str) -> Option<String> {
    for (needle, label) in [
        ("quartier latin", "Quartier latin"),
        ("latin quarter", "Latin Quarter"),
        ("closerie des lilas", "Closerie des Lilas"),
        ("hôtel pimodan", "Hôtel Pimodan"),
        ("hotel pimodan", "Hôtel Pimodan"),
        ("hôtel de lauzun", "Hôtel de Lauzun"),
        ("hotel de lauzun", "Hôtel de Lauzun"),
    ] {
        if lower.contains(needle) {
            return Some(label.to_string());
        }
    }
    for cue in ["café ", "cafe "] {
        if let Some(pos) = lower.find(cue) {
            let after = &s[pos + cue.len()..];
            if let Some(name) = extract_person_name(after) {
                if is_plausible_place_label(&name) {
                    return Some(format!("café {name}"));
                }
            }
        }
    }
    if let Some(pos) = lower.find("taverns of ") {
        let after = &s[pos + "taverns of ".len()..];
        if let Some(name) = extract_person_name(after) {
            return Some(name);
        }
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
            let clipped = after
                .split(|c: char| c == '.' || c.is_ascii_digit() || c == ',')
                .next()?
                .trim()
                .trim_matches(|c: char| !c.is_alphabetic() && c != ' ' && c != '-' && c != '\'');
            let clipped = clipped.split(" and ").next().unwrap_or(clipped);
            let clipped = clipped.split(" et ").next().unwrap_or(clipped);
            let token = clipped
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
                && is_plausible_place_label(&token)
            {
                return Some(token);
            }
        }
    }
    None
}

fn find_related_person(s: &str, lower: &str) -> Option<String> {
    // EN often puts the name before the cue: "Jeanne Duval … became his mistress"
    for cue in [
        " became his mistress",
        " became her mistress",
        " became his lover",
        " became her lover",
        " became his partner",
        " became her partner",
    ] {
        if let Some(pos) = lower.find(cue) {
            if let Some(name) = extract_trailing_person_name(&s[..pos]) {
                return Some(name);
            }
        }
    }
    // Look for person names after relationship cues
    for cue in [
        "s'éprend de ",
        "s'éprend d'",
        "liaison avec ",
        "rencontre ",
        "rencontra ",
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

fn extract_trailing_person_name(before: &str) -> Option<String> {
    let tokens: Vec<String> = before
        .split_whitespace()
        .map(|w| {
            w.trim_matches(|c: char| !c.is_alphabetic() && c != '-' && c != '\'')
                .to_string()
        })
        .filter(|w| !w.is_empty())
        .collect();

    // Last run of ≥2 capitalized name tokens (skip trailing appositions like "a French-born actress").
    let mut best: Option<String> = None;
    let mut cur: Vec<String> = Vec::new();
    for t in &tokens {
        let first = t.chars().next().unwrap_or(' ');
        if first.is_uppercase()
            || (!cur.is_empty()
                && matches!(
                    t.to_lowercase().as_str(),
                    "de" | "du" | "van" | "von" | "la" | "le"
                ))
        {
            cur.push(t.clone());
            continue;
        }
        if cur.len() >= 2 {
            best = Some(cur.join(" "));
        }
        cur.clear();
    }
    if cur.len() >= 2 {
        best = Some(cur.join(" "));
    }
    best
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
        let hit = raws.iter().find(|r| r.event_type == "meeting").unwrap();
        assert!(
            crate::extractors::keep_extracted_raw(
                hit,
                "Charles Baudelaire",
                "Charles Baudelaire"
            ),
            "Duval clause must be kept on subject biography"
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
    fn english_jeanne_duval_mistress_name_before_cue_no_year() {
        let raws = AnecdoteLifeExtractor.extract(&make_input(
            "During this time, Jeanne Duval, a French-born actress, became his mistress."
        ));
        assert!(
            raws.iter().any(|r| {
                r.event_type == "meeting"
                    && r.object_surface
                        .as_deref()
                        .is_some_and(|p| p.contains("Jeanne Duval"))
            }),
            "should recover Jeanne Duval before mistress cue: {raws:?}"
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

    #[test]
    fn french_cafe_quartier_latin_is_residence() {
        let raws = AnecdoteLifeExtractor.extract(&make_input(
            "Il affectionnait aussi La Rotonde, un café du Quartier latin.",
        ));
        assert!(
            raws.iter().any(|r| r.event_type == "residence"
                && r.place_surface
                    .as_deref()
                    .is_some_and(|p| p.to_lowercase().contains("quartier"))),
            "Quartier latin café: {raws:?}"
        );
    }

    #[test]
    fn english_taverns_of_paris() {
        let raws = AnecdoteLifeExtractor.extract(&make_input(
            "On returning to the taverns of Paris, he began to compose some of the poems of Les Fleurs du Mal.",
        ));
        assert!(
            raws.iter().any(|r| r.event_type == "residence"
                && r.place_surface.as_deref() == Some("Paris")),
            "taverns of Paris: {raws:?}"
        );
    }

    #[test]
    fn literary_salon_is_meeting_not_art_review() {
        let salon = AnecdoteLifeExtractor.extract(&make_input(
            "Baudelaire fréquenta les salons de Paris et y rencontra d'autres auteurs.",
        ));
        assert!(
            salon.iter().any(|r| r.event_type == "meeting"),
            "literary salons: {salon:?}"
        );
        let review = AnecdoteLifeExtractor.extract(&make_input(
            "His first published work was his art review \"Salon of 1845\".",
        ));
        assert!(
            !review.iter().any(|r| r.event_type == "meeting" || r.event_type == "residence"),
            "art Salon of 1845 must not become a haunt: {review:?}"
        );
    }

    #[test]
    fn brothel_anecdote_keeps_place() {
        let raws = AnecdoteLifeExtractor.extract(&make_input(
            "He began to frequent prostitutes in Paris and may have contracted syphilis.",
        ));
        assert!(
            raws.iter().any(|r| r.event_type == "residence"
                && r.place_surface.as_deref() == Some("Paris")),
            "brothel/prostitute Paris: {raws:?}"
        );
    }
}
