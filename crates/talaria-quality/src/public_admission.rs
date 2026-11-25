// crates/talaria-quality/src/public_admission.rs
//! What may leave review and enter the public biography.
//!
//! A technical accept is not enough. The passage has to show who acts, and a
//! catalog line, a relative's action, or a borrowed year stays in review.

/// Publish the fact, or keep the candidate in review with a stable reason.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PublicAdmission {
    Publish,
    Hold(&'static str),
}

/// Whether the passage supports this action by this person.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HistoricalStanding {
    Admit,
    Review(&'static str),
}

/// What date the passage actually supports. Independent of publication.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeStanding {
    Justified,
    /// No date was established. The fact may still be admitted.
    Unknown,
    /// A year was taken from another sentence and must not be stored.
    Borrowed,
}

/// Whether the place can become a map point. Independent of publication.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MapStanding {
    Eligible,
    OffMap(&'static str),
}

/// The three decisions for one extracted fact.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FactDecisions {
    pub historical: HistoricalStanding,
    pub time: TimeStanding,
    pub map: MapStanding,
}

/// One extracted fact, judged before it can become a canonical event.
pub struct PublicFact<'a> {
    pub subject: &'a str,
    pub quote: &'a str,
    pub summary: &'a str,
    pub event_type: &'a str,
    pub time_reading: &'a str,
    pub structured_source: bool,
    /// The place the extractor attached. It has to be the place of this passage.
    pub place: Option<&'a str>,
}

const KIN_CUES: &[&str] = &[
    "sa fille",
    "son fils",
    "sa soeur",
    "sa sœur",
    "son frere",
    "son frère",
    "sa femme",
    "son mari",
    "son epoux",
    "son époux",
    "his daughter",
    "her daughter",
    "his son",
    "her son",
    "his wife",
    "her husband",
    "his sister",
    "her sister",
    "his brother",
    "her brother",
];

const ACTION_VERBS: &[&str] = &[
    "compose",
    "composed",
    "composa",
    "écrit",
    "ecrit",
    "wrote",
    "written",
    "published",
    "publia",
    "publie",
    "fought",
    "signed",
    "signa",
    "commanded",
    "drowned",
    "painted",
    "peignit",
    "sculpta",
    "quitte",
    "quitta",
    "meurt",
    "mourut",
    "nait",
    "naquit",
    "installe",
    "joue",
    "joua",
    "intervient",
    "parait",
    "paraît",
    "epouse",
    "épouse",
    "married",
    "died",
    "born",
    "lived",
    "appeared",
];

const OPENERS: &[&str] = &[
    "the", "a", "an", "le", "la", "les", "un", "une", "en", "on", "in", "at", "de", "du",
    "des", "sa", "son", "his", "her", "their", "after", "après", "apres", "battle", "siege",
    "treaty", "armistice", "second", "general", "king",
];

const NOTICE_MARKERS: &[&str] = &[
    "gallica",
    "imprimerie",
    "impr.",
    "isbn",
    "ouvrage collectif",
    "deuxième édition",
    "deuxieme edition",
    "seconde édition",
    "seconde edition",
    "réédition",
    "reedition",
    "reprint",
    "catalogue",
    "catalog record",
    "notice bibliographique",
    "publiées par",
    "publiees par",
    "published by",
    "coll.",
];

const LOCALITIES: &[&str] = &[
    "paris", "lyon", "douai", "rouen", "bruxelles", "brussels", "genève", "geneve", "geneva",
    "guernsey", "guernesey", "ajaccio", "marseille", "rome", "london", "londres", "waterloo",
    "odéon", "odeon",
    "mantua", "mantoue", "cherasco", "arcole", "castiglione",
];

const TERRITORIES: &[&str] = &[
    "france", "england", "angleterre", "austria", "autriche", "russia", "russie", "spain",
    "espagne", "italy", "italie", "germany", "allemagne", "poland", "pologne", "belgium",
    "belgique", "europe", "prussia", "prusse", "royaume de france",
];

/// Separate the historical, temporal, and geographic decisions.
/// A missing date or a missing point does not change the historical one.
pub fn decide_fact(fact: &PublicFact<'_>, place: Option<&str>) -> FactDecisions {
    FactDecisions {
        historical: historical_standing(fact),
        time: time_standing(fact.time_reading),
        map: map_standing(place, fact.quote),
    }
}

pub fn admit_to_public_corpus(fact: &PublicFact<'_>) -> PublicAdmission {
    match historical_standing(fact) {
        HistoricalStanding::Admit => PublicAdmission::Publish,
        HistoricalStanding::Review(reason) => PublicAdmission::Hold(reason),
    }
}

fn historical_standing(fact: &PublicFact<'_>) -> HistoricalStanding {
    if is_documentary_notice(&fact.quote) || is_documentary_notice(&fact.summary) {
        return HistoricalStanding::Review("bibliographic_notice");
    }
    if let Some(reason) = unresolved_time(fact.time_reading) {
        return HistoricalStanding::Review(reason);
    }
    if let Some(reason) = borrowed_assembly(fact.quote, fact.place) {
        return HistoricalStanding::Review(reason);
    }
    if fact.place.is_some_and(place_is_sentence_fragment) {
        return HistoricalStanding::Review("place_is_a_sentence_fragment");
    }
    if post_was_not_exercised(&fact.quote) {
        return HistoricalStanding::Review("post_not_exercised");
    }
    if action_was_delegated(&fact.quote, fact.subject) {
        return HistoricalStanding::Review("action_is_delegated");
    }
    if fact.event_type == "death"
        && !death_cue(&fold(&fact.quote))
        && crate::biography_lead_date("death", fact.quote).is_none()
    {
        return HistoricalStanding::Review("passage_is_another_episode");
    }
    if fact.event_type == "residence" && birth_cue(&fold(&fact.quote)) {
        return HistoricalStanding::Review("residence_is_the_birth");
    }
    let passage = format!("{} {}", fact.summary, fact.quote);
    if someone_else_acts(&passage, fact.subject) {
        return HistoricalStanding::Review("actor_is_not_the_subject");
    }
    if is_parentage_only(&fact.quote) {
        return HistoricalStanding::Review("genealogical_context");
    }
    if is_posterity_naming(&fact.quote) || is_posterity_naming(&fact.summary) {
        return HistoricalStanding::Review("posterity_not_an_act");
    }
    if citation_without_an_act(&fact.quote, fact.subject)
        || citation_without_an_act(&fact.summary, fact.subject)
    {
        return HistoricalStanding::Review("bibliographic_notice");
    }
    if !fact.structured_source && fact.quote.split_whitespace().count() < 4 {
        return HistoricalStanding::Review("passage_too_thin");
    }
    if !fact.structured_source && !passage_has_an_actor(&fact.quote, fact.subject) {
        return HistoricalStanding::Review("actor_not_in_the_passage");
    }
    if !fact.structured_source && title_conflicts(fact.summary, fact.quote) {
        return HistoricalStanding::Review("title_not_in_the_passage");
    }
    if !fact.structured_source && place_missing_from_passage(fact.place, fact.quote, fact.event_type)
    {
        return HistoricalStanding::Review("place_not_in_the_passage");
    }
    if !fact.structured_source && mention_is_background(fact.summary, fact.quote) {
        return HistoricalStanding::Review("mention_is_not_the_action");
    }
    HistoricalStanding::Admit
}

const TITLE_STOP: &[&str] = &[
    "treaty", "traite", "battle", "bataille", "siege", "about", "after", "before", "since",
    "under", "their", "which", "where", "there", "these", "those", "first", "second", "third",
    "young", "later", "among", "named", "general", "french", "republic", "moment", "arrival",
    "arrivee", "office", "death", "birth", "residence", "meeting", "defence", "defense",
    "from", "with", "that", "this", "were", "into", "over", "upon", "near",
];

/// The proper names in a title have to occur in the passage that is supposed to prove it.
pub fn passage_supports_title(title: &str, quote: &str) -> bool {
    let names = distinctive_names(title);
    if names.is_empty() {
        return true;
    }
    let folded = fold(quote);
    names.iter().all(|name| folded.contains(name.as_str()))
}

fn distinctive_names(title: &str) -> Vec<String> {
    fold(title)
        .split(|ch: char| !ch.is_alphabetic())
        .filter(|token| token.len() >= 4 && !TITLE_STOP.contains(token))
        .map(str::to_string)
        .collect()
}

fn title_conflicts(summary: &str, quote: &str) -> bool {
    let summary = summary.trim();
    if summary.is_empty() || summary == quote.trim() {
        return false;
    }
    !passage_supports_title(summary, quote)
}

fn place_missing_from_passage(place: Option<&str>, quote: &str, event_type: &str) -> bool {
    if !matches!(
        event_type,
        "arrival" | "battle" | "siege" | "residence" | "diplomatic" | "meeting" | "exile"
            | "travel" | "office"
    ) {
        return false;
    }
    let Some(place) = place.map(str::trim).filter(|place| place.chars().count() >= 4) else {
        return false;
    };
    if crate::is_wikidata_qid(place) {
        return true;
    }
    !fold(quote).contains(&fold(place))
}

fn place_is_sentence_fragment(label: &str) -> bool {
    let folded = fold(label.trim());
    let first = folded.split_whitespace().next().unwrap_or("");
    matches!(
        first,
        "its" | "his" | "her" | "their" | "son" | "sa" | "leur" | "leurs" | "cette" | "this"
    )
}

fn post_was_not_exercised(quote: &str) -> bool {
    let folded = fold(quote);
    [
        "never took",
        "never held",
        "never occupied",
        "did not take",
        "n'occupe jamais",
        "n'occupe pas",
        "ne l'occupe pas",
        "jamais le poste",
    ]
    .iter()
    .any(|cue| folded.contains(cue))
}

fn action_was_delegated(quote: &str, subject: &str) -> bool {
    let folded = fold(quote);
    let sent = [" sent ", " dispatched ", " envoya ", " envoie "]
        .iter()
        .any(|cue| folded.contains(cue));
    let landed = folded.contains(" landed ") || folded.contains(" debarqu");
    if !(sent && landed) {
        return false;
    }
    let (_, surname) = subject_names(subject);
    if surname.is_empty() {
        return true;
    }
    let Some(index) = folded.find(" landed ").or_else(|| folded.find(" debarqu")) else {
        return true;
    };
    !folded[..index]
        .split_whitespace()
        .rev()
        .take(8)
        .any(|word| word.contains(&surname))
}

fn unresolved_time(reading: &str) -> Option<&'static str> {
    match reading {
        "another_action_in_the_same_passage" => Some("another_action_in_the_same_passage"),
        "several_years_in_one_passage" => Some("several_years_in_one_passage"),
        _ => None,
    }
}

fn date_only_line(folded: &str) -> bool {
    const MONTHS: &[&str] = &[
        "january", "february", "march", "april", "may", "june", "july", "august", "september",
        "october", "november", "december", "janvier", "fevrier", "février", "mars", "avril", "mai",
        "juin", "juillet", "aout", "août", "septembre", "octobre", "novembre", "decembre",
        "décembre",
    ];
    let words: Vec<&str> = folded
        .split(|ch: char| !ch.is_alphabetic())
        .filter(|word| !word.is_empty())
        .collect();
    !words.is_empty() && words.iter().all(|word| MONTHS.contains(word))
}

fn death_cue(folded: &str) -> bool {
    ["died", "death", "meurt", "mourut", "dead", "deces", "décès", "cancer", "succombe", "buried"]
        .iter()
        .any(|cue| has_word(folded, cue))
}

fn birth_cue(folded: &str) -> bool {
    ["born", "naquit", "nait", "naît", "naissance"]
        .iter()
        .any(|cue| has_word(folded, cue))
}

fn has_word(folded: &str, cue: &str) -> bool {
    folded
        .split(|ch: char| !ch.is_alphabetic() && ch != '\'')
        .any(|word| word == cue)
}

fn mention_is_background(title: &str, quote: &str) -> bool {
    let folded = fold(quote);
    let names = distinctive_names(title);
    if names.is_empty() {
        return false;
    }
    let markers = [
        "since the ",
        "after the battle",
        "after the treaty",
        "following the ",
        "pendant la ",
        "lors de la ",
        "lors du ",
    ];
    let Some(marker_at) = markers.iter().filter_map(|marker| folded.find(marker)).min() else {
        return false;
    };
    let Some(name_at) = names.iter().filter_map(|name| folded.find(name.as_str())).min() else {
        return false;
    };
    name_at > marker_at
        && !folded.contains(" was signed")
        && !folded.contains(" signed the")
        && !folded.contains(" was fought")
}

/// The sentence that names the action. A date in another sentence of the same passage is not this fact's date.
pub fn clause_for_time(summary: &str, quote: &str) -> String {
    let name = action_name(quote).or_else(|| action_name(summary));
    if let Some(name) = name {
        if let Some(sentence) = sentences(quote)
            .into_iter()
            .find(|sentence| fold(sentence).contains(&fold(&name)))
        {
            return sentence.to_string();
        }
    }
    quote.to_string()
}

/// A title, a place, and a day taken from different sentences are not one fact.
fn borrowed_assembly(quote: &str, place: Option<&str>) -> Option<&'static str> {
    let name = action_name(quote)?;
    let folded_name = fold(&name);
    let Some(sentence) = sentences(quote)
        .into_iter()
        .find(|sentence| fold(sentence).contains(&folded_name))
    else {
        return Some("title_not_in_the_passage");
    };
    if let Some(place) = place.map(str::trim).filter(|place| place.chars().count() >= 4) {
        if !is_wikidata_qid_label(place) && !fold(sentence).contains(&fold(place)) {
            return Some("place_not_in_the_passage");
        }
    }
    let day_elsewhere = sentences(quote).into_iter().any(|other| {
        !std::ptr::eq(other, sentence) && sentence_has_day(other) && !sentence_has_day(sentence)
    });
    if day_elsewhere {
        return Some("date_not_in_the_passage");
    }
    None
}

fn action_name(text: &str) -> Option<String> {
    let lower = text.to_lowercase();
    for prefix in [
        "battle of ",
        "siege of ",
        "treaty of ",
        "bataille de ",
        "bataille d'",
        "siège de ",
        "siège d'",
        "siege de ",
        "siege d'",
    ] {
        let Some(at) = lower.find(prefix) else {
            continue;
        };
        let rest = &text[at..];
        let end = [" was ", " were ", " on ", " in ", " a ", ",", ".", ";", ":"]
            .iter()
            .filter_map(|mark| rest.find(mark))
            .min()
            .unwrap_or(rest.len());
        let name = rest[..end].trim();
        if (prefix.len() + 2..=80).contains(&name.chars().count()) {
            return Some(name.to_string());
        }
    }
    None
}

fn sentences(quote: &str) -> Vec<&str> {
    let mut spans = Vec::new();
    let mut start = 0usize;
    let mut offset = 0usize;
    let mut rest = quote;
    while let Some(index) = rest.find(". ") {
        let end = offset + index + 1;
        if end > start {
            let sentence = quote[start..end].trim();
            if !sentence.is_empty() {
                spans.push(sentence);
            }
        }
        offset += index + 2;
        start = offset;
        rest = quote.get(offset..).unwrap_or("");
    }
    if start < quote.len() {
        let sentence = quote[start..].trim();
        if !sentence.is_empty() {
            spans.push(sentence);
        }
    }
    spans
}

fn sentence_has_day(sentence: &str) -> bool {
    const MONTHS: &[&str] = &[
        "january", "february", "march", "april", "may", "june", "july", "august", "september",
        "october", "november", "december", "janvier", "février", "fevrier", "mars", "avril", "mai",
        "juin", "juillet", "août", "aout", "septembre", "octobre", "novembre", "décembre",
        "decembre",
    ];
    let folded = fold(sentence);
    let Some(month_at) = MONTHS.iter().filter_map(|month| folded.find(month)).min() else {
        return false;
    };
    let mut numbers = Vec::new();
    if let Some(token) = folded[..month_at]
        .split(|ch: char| !ch.is_ascii_digit())
        .filter(|token| !token.is_empty())
        .last()
    {
        numbers.push(token.to_string());
    }
    if let Some(token) = folded[month_at..]
        .split(|ch: char| !ch.is_ascii_digit())
        .find(|token| !token.is_empty())
    {
        numbers.push(token.to_string());
    }
    numbers.iter().any(|token| {
        token.parse::<u32>().ok().is_some_and(|number| (1..=31).contains(&number))
    })
}

fn is_wikidata_qid_label(label: &str) -> bool {
    let label = label.trim();
    label.len() >= 2 && label.starts_with('Q') && label[1..].bytes().all(|byte| byte.is_ascii_digit())
}

/// A later passage can reinforce an event only when it is still that event.
pub fn evidence_quote_fits_event(event_type: &str, _event_place: Option<&str>, quote: &str) -> bool {
    let spans = sentences(quote);
    let spans = if spans.is_empty() { vec![quote] } else { spans };
    if event_type == "death" {
        let lead = crate::biography_lead_date("death", quote).is_some();
        let death = lead || spans.iter().any(|sentence| death_cue(&fold(sentence)));
        let other = spans.iter().any(|sentence| {
            !death_cue(&fold(sentence))
                && other_episode(sentence)
                && crate::biography_lead_date("death", sentence).is_none()
        });
        return death && !other;
    }
    if event_type == "birth" {
        return spans.iter().any(|sentence| birth_cue(&fold(sentence)));
    }
    true
}

fn other_episode(sentence: &str) -> bool {
    let folded = fold(sentence);
    [" landed ", " debarqu", " fought ", " battle of ", " bataille "]
        .iter()
        .any(|cue| folded.contains(cue))
}

fn citation_without_an_act(text: &str, subject: &str) -> bool {
    let folded = fold(text);
    if ACTION_VERBS.iter().any(|verb| folded.contains(verb)) {
        return false;
    }
    if date_only_line(&folded) {
        return false;
    }
    let (given, surname) = subject_names(subject);
    let words: Vec<&str> = text.split_whitespace().collect();
    let mut index = 0;
    while index < words.len() {
        let token = clean_token(words[index]);
        if token.is_empty() || OPENERS.contains(&token.as_str()) {
            index += 1;
            continue;
        }
        break;
    }
    let Some(first) = words.get(index) else {
        return false;
    };
    if token_is_subject(&clean_token(first), &given, &surname) {
        return false;
    }
    first.chars().next().is_some_and(|ch| ch.is_uppercase())
}

fn is_posterity_naming(text: &str) -> bool {
    let folded = fold(text);
    folded.contains("porte son nom")
        || folded.contains("porte le nom")
        || folded.contains("named after her")
        || folded.contains("named after him")
        || folded.contains("bears her name")
        || folded.contains("bears his name")
}

fn time_standing(reading: &str) -> TimeStanding {
    match reading {
        "inferred_year" | "paragraph_context" => TimeStanding::Borrowed,
        "missing" | "inherited_not_used" => TimeStanding::Unknown,
        _ => TimeStanding::Justified,
    }
}

fn map_standing(place: Option<&str>, quote: &str) -> MapStanding {
    let Some(place) = place.map(str::trim).filter(|place| !place.is_empty()) else {
        return MapStanding::OffMap("no_place");
    };
    if is_political_territory(place) || quote_names_several_localities(quote) {
        return MapStanding::OffMap("place_is_not_an_event_point");
    }
    MapStanding::Eligible
}

/// Two cities in one sentence are not one map point.
pub fn locality_count(quote: &str) -> usize {
    let folded = fold(quote);
    LOCALITIES
        .iter()
        .filter(|name| contains_word(&folded, name))
        .count()
}

pub fn quote_names_several_localities(quote: &str) -> bool {
    locality_count(quote) >= 2
}

/// A country or kingdom name is not a precise place for a map point.
pub fn is_political_territory(label: &str) -> bool {
    let folded = fold(label.trim());
    if folded.starts_with("mainland ") || folded.ends_with(" mainland") {
        return true;
    }
    TERRITORIES.iter().any(|name| folded == *name)
}

/// Letters and digits only, so a final period does not create a second fact.
pub fn clause_identity(quote: &str) -> String {
    quote
        .chars()
        .filter(|ch| ch.is_alphanumeric())
        .flat_map(|ch| ch.to_lowercase())
        .collect()
}

fn someone_else_acts(text: &str, subject: &str) -> bool {
    let folded = fold(text);
    if KIN_CUES.iter().any(|cue| folded.contains(cue))
        && ACTION_VERBS.iter().any(|verb| folded.contains(verb))
    {
        return true;
    }
    if notice_names_another_maker(&folded, subject) || acted_under_someone_else(&folded, subject) {
        return true;
    }
    leading_name_acts(text, subject)
}

fn acted_under_someone_else(folded: &str, subject: &str) -> bool {
    let (given, surname) = subject_names(subject);
    for marker in ["fought by ", "commanded by ", "under general ", "signed by "] {
        let Some(index) = folded.find(marker) else {
            continue;
        };
        let after = &folded[index + marker.len()..];
        let window = after.split_whitespace().take(8).collect::<Vec<_>>().join(" ");
        let subject_acts = contains_word(&window, &surname)
            || (given.chars().count() >= 4 && contains_word(&window, &given));
        if !subject_acts {
            return true;
        }
    }
    false
}

fn notice_names_another_maker(folded: &str, subject: &str) -> bool {
    let (_, surname) = subject_names(subject);
    if surname.is_empty() {
        return false;
    }
    // The subject publishes the poem. Someone else only writes the music.
    if folded.contains("publication")
        && folded.contains(&surname)
        && (folded.contains("partition") || folded.contains("composed by"))
    {
        return false;
    }
    for marker in [
        "publiées par ",
        "publiees par ",
        "published by ",
        "composée par ",
        "composee par ",
        "composées par ",
        "composees par ",
        "composé par ",
        "composed by ",
    ] {
        if let Some(index) = folded.find(marker) {
            let after = folded[index + marker.len()..].trim();
            let maker = after.split_whitespace().take(3).collect::<Vec<_>>().join(" ");
            if !maker.is_empty() && !maker.contains(&surname) {
                return true;
            }
        }
    }
    false
}

fn leading_name_acts(text: &str, subject: &str) -> bool {
    let (given, surname) = subject_names(subject);
    let words: Vec<&str> = text.split_whitespace().collect();
    let mut index = 0;
    while index < words.len() {
        let token = clean_token(words[index]);
        if token.is_empty() || OPENERS.contains(&token.as_str()) || token.chars().all(|ch| ch.is_ascii_digit())
        {
            index += 1;
            continue;
        }
        break;
    }
    let Some(first) = words.get(index) else {
        return false;
    };
    if !first.chars().next().is_some_and(|ch| ch.is_uppercase()) {
        return false;
    }
    let first_token = clean_token(first);
    if token_is_subject(&first_token, &given, &surname) {
        return false;
    }
    let second_raw = words.get(index + 1).copied().unwrap_or("");
    let third_raw = words.get(index + 2).copied().unwrap_or("");
    let second = clean_token(second_raw);
    let third = clean_token(third_raw);
    if token_is_subject(&second, &given, &surname) {
        return true;
    }
    if ACTION_VERBS.iter().any(|verb| second.starts_with(verb)) {
        return true;
    }
    if second_raw.chars().next().is_some_and(|ch| ch.is_uppercase())
        && ACTION_VERBS.iter().any(|verb| third.starts_with(verb))
    {
        return true;
    }
    let folded = fold(text);
    (folded.contains("monument") || folded.contains("statue"))
        && first.chars().next().is_some_and(|ch| ch.is_uppercase())
        && second_raw.chars().next().is_some_and(|ch| ch.is_uppercase())
}

fn passage_has_an_actor(quote: &str, subject: &str) -> bool {
    let folded = fold(quote);
    let (given, surname) = subject_names(subject);
    if !surname.is_empty() && contains_word(&folded, &surname) {
        return true;
    }
    if given.chars().count() >= 4 && contains_word(&folded, &given) {
        return true;
    }
    const CORE: &[&str] = &[
        "elle", "il", "she", "he", "they", "couple", "jeune", "young",
    ];
    CORE.iter().any(|word| contains_word(&folded, word))
}

fn is_documentary_notice(text: &str) -> bool {
    let folded = fold(text);
    NOTICE_MARKERS.iter().any(|marker| folded.contains(marker))
}

fn is_parentage_only(quote: &str) -> bool {
    let folded = fold(quote);
    let parentage = folded.contains("fille de")
        || folded.contains("fils de")
        || folded.contains("daughter of")
        || folded.contains("son of");
    parentage && !ACTION_VERBS.iter().any(|verb| folded.contains(verb))
}

fn subject_names(subject: &str) -> (String, String) {
    let given = subject.split_whitespace().next().unwrap_or("").to_string();
    let surname = subject.split_whitespace().last().unwrap_or("").to_string();
    (fold(&given), fold(&surname))
}

fn token_is_subject(token: &str, given: &str, surname: &str) -> bool {
    let token = fold(token);
    !token.is_empty() && (token == given || token == surname)
}

fn contains_word(folded: &str, word: &str) -> bool {
    folded
        .split(|ch: char| !ch.is_alphabetic() && ch != '-')
        .any(|token| token == word)
}

fn clean_token(word: &str) -> String {
    fold(word.trim_matches(|ch: char| !ch.is_alphabetic() && ch != '-' && ch != '\''))
}

fn fold(text: &str) -> String {
    text.to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fact<'a>(
        subject: &'a str,
        quote: &'a str,
        reading: &'a str,
    ) -> PublicFact<'a> {
        PublicFact {
            subject,
            quote,
            summary: quote,
            event_type: "work",
            time_reading: reading,
            structured_source: false,
            place: None,
        }
    }

    #[test]
    fn a_composer_does_not_erase_the_subjects_poem() {
        let billet = "la première publication connue d'un poème de Marceline Desbordes-Valmore est « Le Billet », avec une partition composée par Joseph-Henri Mees publiée en 1807";
        assert_eq!(
            admit_to_public_corpus(&fact("Marceline Desbordes-Valmore", billet, "missing")),
            PublicAdmission::Publish
        );
        let full = "la première publication connue d'un poème de Marceline Desbordes-Valmore est « Le Billet », avec une partition composée par Joseph-Henri Mees publiée en 1807 ;";
        let short: String = full.chars().take(120).collect();
        let summary = format!("{short}…");
        assert_eq!(
            admit_to_public_corpus(&PublicFact {
                subject: "Marceline Desbordes-Valmore",
                quote: full,
                summary: &summary,
                event_type: "publication",
                time_reading: "missing",
                structured_source: false,
                place: None,
            }),
            PublicAdmission::Publish,
            "{summary}"
        );
        let louise = "parmi les dix romances composées par Louise de Caumont Dadde on trouve cinq mises en musique de poèmes de Marceline Desbordes-Valmore";
        assert_eq!(
            admit_to_public_corpus(&fact("Marceline Desbordes-Valmore", louise, "missing")),
            PublicAdmission::Hold("actor_is_not_the_subject")
        );
    }

    #[test]
    fn a_daughters_poems_are_not_the_subjects_publication() {
        let full = "Sa fille Hyacinthe compose des poèmes et des contes.";
        assert_eq!(
            admit_to_public_corpus(&fact(
                "Marceline Desbordes-Valmore",
                full,
                "missing"
            )),
            PublicAdmission::Hold("actor_is_not_the_subject")
        );
        let short = PublicFact {
            subject: "Marceline Desbordes-Valmore",
            quote: "compose des poèmes et des contes",
            summary: full,
            event_type: "work",
            time_reading: "missing",
            structured_source: false,
            place: None,
        };
        assert_eq!(
            admit_to_public_corpus(&short),
            PublicAdmission::Hold("actor_is_not_the_subject")
        );
    }

    #[test]
    fn a_borrowed_year_is_not_stored_but_the_fact_can_remain() {
        let quote = "the young girl somehow made her way back to France";
        let borrowed = fact("Marceline Desbordes-Valmore", quote, "inferred_year");
        let decisions = decide_fact(&borrowed, Some("France"));
        assert_eq!(decisions.historical, HistoricalStanding::Admit);
        assert_eq!(decisions.time, TimeStanding::Borrowed);
        assert_eq!(decisions.map, MapStanding::OffMap("place_is_not_an_event_point"));
        assert_eq!(admit_to_public_corpus(&borrowed), PublicAdmission::Publish);
        let undated = decide_fact(
            &fact("Marceline Desbordes-Valmore", quote, "missing"),
            Some("France"),
        );
        assert_eq!(undated.historical, decisions.historical);
        assert_eq!(undated.time, TimeStanding::Unknown);
        assert_eq!(undated.map, decisions.map);
    }

    #[test]
    fn a_missing_date_or_point_does_not_change_the_historical_decision() {
        let letter = "Marceline écrit à Prosper Lanchantin une lettre dont la date n'est pas connue.";
        let dated = fact("Marceline Desbordes-Valmore", letter, "explicit_year");
        let undated = fact("Marceline Desbordes-Valmore", letter, "missing");
        assert_eq!(
            decide_fact(&dated, Some("Lyon")).historical,
            decide_fact(&undated, None).historical
        );
        assert_eq!(decide_fact(&undated, None).time, TimeStanding::Unknown);
        assert_eq!(decide_fact(&undated, None).map, MapStanding::OffMap("no_place"));
        assert_eq!(
            decide_fact(&undated, Some("Lyon")).map,
            MapStanding::Eligible
        );
        assert_eq!(admit_to_public_corpus(&undated), PublicAdmission::Publish);
    }

    #[test]
    fn a_gallica_edition_is_a_notice_not_a_life_event() {
        let quote = "deuxième édition (Poésies de Mme Desbordes-Valmore publiées par Gustave Revilliod) sur Gallica, Genève, Jules Fick, 1873";
        assert_eq!(
            admit_to_public_corpus(&fact("Marceline Desbordes-Valmore", quote, "explicit_year")),
            PublicAdmission::Hold("bibliographic_notice")
        );
    }

    #[test]
    fn a_later_reprint_and_a_statue_are_not_her_actions() {
        let statue = "Édouard Houssin, Monument à Marceline Desbordes-Valmore, 1896, statue en bronze, Douai.";
        assert_eq!(
            admit_to_public_corpus(&fact("Marceline Desbordes-Valmore", statue, "explicit_year")),
            PublicAdmission::Hold("actor_is_not_the_subject")
        );
        let book = "Auguste Bleton, Marceline Desbordes-Valmore à Lyon, Lyon, Impr. de A. Rey, 1896";
        assert_eq!(
            admit_to_public_corpus(&fact("Marceline Desbordes-Valmore", book, "explicit_year")),
            PublicAdmission::Hold("bibliographic_notice")
        );
    }

    #[test]
    fn the_same_lyon_sentence_is_one_clause() {
        assert_eq!(
            clause_identity("Le couple s'installe ensuite à Lyon"),
            clause_identity("Le couple s'installe ensuite à Lyon.")
        );
    }

    #[test]
    fn a_dated_publication_in_the_sentence_can_be_public() {
        let quote = "En 1820 paraissent les Poésies de Mme Desbordes-Valmore.";
        assert_eq!(
            admit_to_public_corpus(&fact("Marceline Desbordes-Valmore", quote, "explicit_year")),
            PublicAdmission::Publish
        );
    }

    #[test]
    fn another_composers_settings_stay_in_review() {
        let louise = "parmi les dix romances composées par Louise de Caumont Dadde on trouve cinq mises en musique de poèmes de Marceline Desbordes-Valmore";
        assert_eq!(
            admit_to_public_corpus(&fact("Marceline Desbordes-Valmore", louise, "missing")),
            PublicAdmission::Hold("actor_is_not_the_subject")
        );
        let woollett = "Henri Woollett compose la suite Marceline ou la Vie d'une femme";
        assert_eq!(
            admit_to_public_corpus(&fact("Marceline Desbordes-Valmore", woollett, "explicit_year")),
            PublicAdmission::Hold("actor_is_not_the_subject")
        );
    }

    #[test]
    fn parentage_and_a_fragment_stay_in_review() {
        let parents = "Marceline Desbordes est la fille de Catherine Lucas et de Félix Desbordes, un peintre en armoiries";
        assert_eq!(
            admit_to_public_corpus(&fact("Marceline Desbordes-Valmore", parents, "missing")),
            PublicAdmission::Hold("genealogical_context")
        );
        assert_eq!(
            admit_to_public_corpus(&fact("Marceline Desbordes-Valmore", "et à Paris", "missing")),
            PublicAdmission::Hold("passage_too_thin")
        );
    }

    #[test]
    fn one_career_sentence_is_not_several_cities() {
        let quote = "Desbordes-Valmore appeared as an actress and singer in Douai, Rouen, the Opéra-Comique in Paris, and the Théâtre de la Monnaie in Brussels";
        assert!(quote_names_several_localities(quote));
        assert!(!quote_names_several_localities(
            "Le couple s'installe ensuite à Lyon"
        ));
    }

    #[test]
    fn napoleon_keeps_a_command_and_loses_another_generals_battle() {
        let arcole = "The Battle of Arcole (15–17 November 1796) was fought southeast of Verona. The battle saw a bold maneuver by Napoleon Bonaparte's French Army of Italy.";
        assert_eq!(
            admit_to_public_corpus(&fact("Napoleon", arcole, "explicit_day")),
            PublicAdmission::Publish
        );
        let ceva = "The Battle of Ceva was fought by the French under General Pierre Augereau.";
        assert_eq!(
            admit_to_public_corpus(&fact("Napoleon", ceva, "explicit_day")),
            PublicAdmission::Hold("actor_is_not_the_subject")
        );
        let murat = "The Battle of Tolentino was fought by the Napoleonic King of Naples Joachim Murat.";
        assert_eq!(
            admit_to_public_corpus(&fact("Napoleon", murat, "missing")),
            PublicAdmission::Hold("actor_is_not_the_subject")
        );
    }

    #[test]
    fn hugo_and_curie_keep_their_own_work_and_lose_family_and_reprints() {
        assert_eq!(
            admit_to_public_corpus(&fact(
                "Victor Hugo",
                "In 1855 Victor Hugo lived in Guernsey.",
                "explicit_year"
            )),
            PublicAdmission::Publish
        );
        assert_eq!(
            admit_to_public_corpus(&fact(
                "Victor Hugo",
                "Léopoldine Hugo drowned in the Seine in 1843.",
                "explicit_year"
            )),
            PublicAdmission::Hold("actor_is_not_the_subject")
        );
        assert_eq!(
            admit_to_public_corpus(&fact(
                "Victor Hugo",
                "Victor Hugo published Les Misérables in 1862.",
                "explicit_year"
            )),
            PublicAdmission::Publish
        );
        assert_eq!(
            admit_to_public_corpus(&fact(
                "Victor Hugo",
                "Édition illustrée des Misérables, Paris, Hetzel, sur Gallica.",
                "explicit_year"
            )),
            PublicAdmission::Hold("bibliographic_notice")
        );
        assert_eq!(
            admit_to_public_corpus(&fact(
                "Marie Curie",
                "Marie Curie and Pierre Curie announced the discovery of polonium in 1898.",
                "explicit_year"
            )),
            PublicAdmission::Publish
        );
        assert_eq!(
            admit_to_public_corpus(&fact(
                "Marie Curie",
                "A reprint of Marie Curie's doctoral thesis was issued in Paris in 1903.",
                "explicit_year"
            )),
            PublicAdmission::Hold("bibliographic_notice")
        );
    }

    #[test]
    fn a_bibliography_line_is_not_her_act() {
        let notice = "Jeanine Moulin, Marceline Desbordes-Valmore, Paris, Seghers, coll";
        let medal = "David d'Angers, Marceline Desbordes-Valmore, médaillon en plâtre, Paris, musée de la vie romantique";
        assert_eq!(
            admit_to_public_corpus(&fact("Marceline Desbordes-Valmore", notice, "explicit_year")),
            PublicAdmission::Hold("bibliographic_notice")
        );
        assert_eq!(
            admit_to_public_corpus(&fact("Marceline Desbordes-Valmore", medal, "missing")),
            PublicAdmission::Hold("bibliographic_notice")
        );
    }

    #[test]
    fn a_street_named_after_her_is_not_her_act() {
        let street = "Une nouvelle rue de Grenoble dans le quartier Flaubert porte son nom : rue Marceline Desbordes-Valmore";
        assert_eq!(
            admit_to_public_corpus(&fact("Marceline Desbordes-Valmore", street, "missing")),
            PublicAdmission::Hold("posterity_not_an_act")
        );
        assert_eq!(
            admit_to_public_corpus(&fact(
                "Marceline Desbordes-Valmore",
                "Le couple s'installe ensuite à Lyon.",
                "missing"
            )),
            PublicAdmission::Publish
        );
        assert_eq!(
            admit_to_public_corpus(&fact(
                "Marceline Desbordes-Valmore",
                "Mais la famille Debonne refusant une union avec une ancienne comédienne, Marceline Desbordes quitte définitivement son amant et reprend le chemin du théâtre, à l'Odéon puis à la Monnaie à Bruxelles",
                "missing"
            )),
            PublicAdmission::Publish
        );
    }

    #[test]
    fn an_assembled_fact_must_be_one_fact() {
        let prisoners = "Bonaparte returned with 30,000 prisoners, 540 cannons, 170 standards, the result of 67 actions and 18 pitched battles.";
        assert_eq!(
            admit_to_public_corpus(&PublicFact {
                subject: "Napoleon Bonaparte",
                quote: prisoners,
                summary: "Treaty of Campo Formio (1797)",
                event_type: "arrival",
                time_reading: "explicit_year",
                structured_source: false,
                place: Some("Paris"),
            }),
            PublicAdmission::Hold("title_not_in_the_passage")
        );
        let basel = "Napoleon decided to intervene in Spain, an ally of the Republic since the treaty of Basel in 1795.";
        assert_eq!(
            admit_to_public_corpus(&PublicFact {
                subject: "Napoleon Bonaparte",
                quote: basel,
                summary: "Treaty of Basel (1795)",
                event_type: "arrival",
                time_reading: "explicit_year",
                structured_source: false,
                place: Some("Lisbon"),
            }),
            PublicAdmission::Hold("place_not_in_the_passage")
        );
        assert_eq!(
            admit_to_public_corpus(&PublicFact {
                subject: "Napoleon Bonaparte",
                quote: basel,
                summary: "Treaty of Basel (1795)",
                event_type: "diplomatic",
                time_reading: "explicit_year",
                structured_source: false,
                place: Some("Basel"),
            }),
            PublicAdmission::Hold("mention_is_not_the_action")
        );
        let defence = "He had seen the Swiss Guard massacred, and believed artillery would be the key to its defence.";
        assert_eq!(
            admit_to_public_corpus(&PublicFact {
                subject: "Napoleon Bonaparte",
                quote: defence,
                summary: defence,
                event_type: "office",
                time_reading: "explicit_year",
                structured_source: false,
                place: Some("Its Defence"),
            }),
            PublicAdmission::Hold("place_is_a_sentence_fragment")
        );
        let constantinople = "The request was eventually granted, but he never took up the post.";
        assert_eq!(
            admit_to_public_corpus(&PublicFact {
                subject: "Napoleon Bonaparte",
                quote: constantinople,
                summary: "diplomatic mission",
                event_type: "diplomatic",
                time_reading: "explicit_year",
                structured_source: false,
                place: Some("Constantinople"),
            }),
            PublicAdmission::Hold("post_not_exercised")
        );
        let leclerc = "Bonaparte sent the Saint-Domingue expedition under General Charles Leclerc, and they landed there in February 1802.";
        assert_eq!(
            admit_to_public_corpus(&PublicFact {
                subject: "Napoleon Bonaparte",
                quote: leclerc,
                summary: leclerc,
                event_type: "meeting",
                time_reading: "explicit_year",
                structured_source: false,
                place: Some("Saint-Domingue"),
            }),
            PublicAdmission::Hold("action_is_delegated")
        );
        let alexandria = "The fleet landed at Alexandria and fought at Shubra Khit.";
        assert_eq!(
            admit_to_public_corpus(&PublicFact {
                subject: "Napoleon Bonaparte",
                quote: alexandria,
                summary: "Died in 1821",
                event_type: "death",
                time_reading: "explicit_year",
                structured_source: false,
                place: Some("Longwood House"),
            }),
            PublicAdmission::Hold("passage_is_another_episode")
        );
        assert!(!evidence_quote_fits_event("death", Some("Longwood House"), alexandria));
        let campaign = "Bonaparte's army captured 150,000 prisoners. Bonaparte finalized peace terms with Austria by the Treaty of Campo Formio. Bonaparte returned to Paris on 5 December 1797 as a hero.";
        assert_eq!(
            admit_to_public_corpus(&PublicFact {
                subject: "Napoleon Bonaparte",
                quote: campaign,
                summary: "Treaty of Campo Formio (1797)",
                event_type: "arrival",
                time_reading: "explicit_day",
                structured_source: false,
                place: Some("Paris"),
            }),
            PublicAdmission::Hold("place_not_in_the_passage")
        );
        let spain = "Napoleon decided to intervene since the treaty of Basel in 1795. Marshal Junot entered Lisbon on November 30.";
        assert_eq!(
            admit_to_public_corpus(&PublicFact {
                subject: "Napoleon Bonaparte",
                quote: spain,
                summary: "treaty of Basel (1795)",
                event_type: "arrival",
                time_reading: "missing",
                structured_source: false,
                place: Some("Lisbon"),
            }),
            PublicAdmission::Hold("place_not_in_the_passage")
        );
        let mixed = "He decided on a military expedition to seize Egypt. Bonaparte reached Malta on 9 June 1798.";
        assert_eq!(
            admit_to_public_corpus(&PublicFact {
                subject: "Napoleon Bonaparte",
                quote: mixed,
                summary: "arrival · 1798 · Hospitaller Malta",
                event_type: "arrival",
                time_reading: "another_action_in_the_same_passage",
                structured_source: false,
                place: Some("Hospitaller Malta"),
            }),
            PublicAdmission::Hold("another_action_in_the_same_passage")
        );
        let lead = "Napoleon Bonaparte (born Napoleone di Buonaparte; 15 August 1769 – 5 May 1821) was Emperor of the French";
        assert_eq!(
            admit_to_public_corpus(&PublicFact {
                subject: "Napoleon Bonaparte",
                quote: lead,
                summary: "15 August 1769",
                event_type: "birth",
                time_reading: "explicit_day",
                structured_source: false,
                place: Some("Ajaccio"),
            }),
            PublicAdmission::Publish
        );
        assert_eq!(
            admit_to_public_corpus(&PublicFact {
                subject: "Napoleon Bonaparte",
                quote: lead,
                summary: "5 May 1821",
                event_type: "death",
                time_reading: "explicit_day",
                structured_source: false,
                place: Some("Longwood House"),
            }),
            PublicAdmission::Publish
        );
        assert!(evidence_quote_fits_event("death", Some("Longwood House"), lead));
        let landing = "He died at Longwood House in 1821. The fleet landed at Alexandria and fought at Shubra Khit.";
        assert!(!evidence_quote_fits_event("death", Some("Longwood House"), landing));
        let birth = "Carlo and Letizia Bonaparte's son Napoleon was born in Ajaccio on 15 August 1769.";
        assert_eq!(
            admit_to_public_corpus(&PublicFact {
                subject: "Napoleon Bonaparte",
                quote: birth,
                summary: birth,
                event_type: "residence",
                time_reading: "explicit_day",
                structured_source: false,
                place: Some("Ajaccio"),
            }),
            PublicAdmission::Hold("residence_is_the_birth")
        );
        assert_eq!(
            admit_to_public_corpus(&PublicFact {
                subject: "Napoleon Bonaparte",
                quote: birth,
                summary: birth,
                event_type: "birth",
                time_reading: "explicit_day",
                structured_source: false,
                place: Some("Ajaccio"),
            }),
            PublicAdmission::Publish
        );
    }

    #[test]
    fn an_ancient_birth_sentence_is_not_a_notice() {
        assert_eq!(
            admit_to_public_corpus(&fact(
                "Aristotle",
                "Aristotle was born in Stagira in 384 BC.",
                "explicit_year"
            )),
            PublicAdmission::Publish
        );
    }
}
