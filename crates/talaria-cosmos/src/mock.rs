// crates/talaria-cosmos/src/mock.rs
//! Dense Wikipedia-prose life-event extractor for demos without spaCy/COSMOS.
//! Uses person aliases + year + gazetteer place + verb cues; page title supplies
//! place context for Battle/Treaty/Congress pages.

use crate::{BatchInputItem, BatchOutputItem, ExtractedTuple};

/// Dense rule-based extractor over real biographical prose.
pub fn mock_extract(items: &[BatchInputItem]) -> Vec<BatchOutputItem> {
    let mut last_page = String::new();
    let mut carry_year: Option<String> = None;
    let mut carry_place: Option<String> = None;
    items
        .iter()
        .map(|item| {
            let page = item.page_title.clone().unwrap_or_default();
            if page != last_page {
                carry_year = None;
                carry_place = None;
                last_page = page.clone();
            }
            let tuples = mock_extract_text_with_carry(
                &item.text,
                item.page_title.as_deref(),
                carry_year.as_deref(),
                carry_place.as_deref(),
            );
            absorb_mock_carry(
                &item.text,
                item.page_title.as_deref(),
                &mut carry_year,
                &mut carry_place,
            );
            BatchOutputItem {
                id: item.id.clone(),
                tuples,
            }
        })
        .collect()
}

fn mock_extract_text(text: &str, page_title: Option<&str>) -> Vec<ExtractedTuple> {
    mock_extract_text_with_carry(text, page_title, None, None)
}

fn mock_extract_text_with_carry(
    text: &str,
    page_title: Option<&str>,
    carry_year: Option<&str>,
    carry_place: Option<&str>,
) -> Vec<ExtractedTuple> {
    let cleaned = strip_wiki_markup(text);
    let lower = cleaned.to_lowercase();
    let page = page_title.unwrap_or("").trim();
    let mut tuples = Vec::new();

    // 1) Explicit high-precision cue layouts (legacy + structured)
    for pattern in STRUCTURED_PATTERNS {
        if let Some(tuple) = try_structured(&cleaned, &lower, pattern) {
            push_unique(&mut tuples, tuple);
        }
    }

    // 2) Dense Wikipedia prose: person/alias + year + place + verb cue
    if let Some(tuple) = try_prose_dense(&cleaned, &lower, page, carry_year, carry_place) {
        push_unique(&mut tuples, tuple);
    }

    tuples
}

fn absorb_mock_carry(
    text: &str,
    page_title: Option<&str>,
    carry_year: &mut Option<String>,
    carry_place: &mut Option<String>,
) {
    let cleaned = strip_wiki_markup(text);
    let lower = cleaned.to_lowercase();
    let page = page_title.unwrap_or("").trim();
    let page_lower = page.to_lowercase();
    let Some((_, year_min, year_max)) = resolve_dense_subject(&cleaned, &page_lower) else {
        return;
    };
    if lower.contains("statue")
        || lower.contains("museum")
        || lower.contains("was unveiled")
        || lower.contains("memorial")
    {
        return;
    }
    if let Some(year) = find_year_in_window(&lower, year_min, year_max) {
        *carry_year = Some(year);
    }
    if let Some(place) = find_place(&lower, page) {
        *carry_place = Some(place);
    }
}

fn push_unique(tuples: &mut Vec<ExtractedTuple>, tuple: ExtractedTuple) {
    if !tuples.iter().any(|existing| {
        existing.verb == tuple.verb
            && existing.time == tuple.time
            && existing.place.eq_ignore_ascii_case(&tuple.place)
    }) {
        tuples.push(tuple);
    }
}

// --- structured patterns (strict layouts) ---------------------------------

struct Pattern {
    cue: &'static str,
    verb: &'static str,
    layout: Layout,
}

enum Layout {
    YearThenInPlace,
    PlaceThenInYear,
    ObjectYearPlace,
}

const STRUCTURED_PATTERNS: &[Pattern] = &[
    Pattern { cue: " was born in ", verb: "born", layout: Layout::YearThenInPlace },
    Pattern { cue: " died in ", verb: "died", layout: Layout::YearThenInPlace },
    Pattern { cue: " studied at ", verb: "studied", layout: Layout::PlaceThenInYear },
    Pattern { cue: " fought at ", verb: "fought", layout: Layout::PlaceThenInYear },
    Pattern { cue: " was crowned in ", verb: "crowned", layout: Layout::YearThenInPlace },
    Pattern { cue: " married ", verb: "married", layout: Layout::ObjectYearPlace },
    Pattern { cue: " divorced ", verb: "divorced", layout: Layout::ObjectYearPlace },
    Pattern { cue: " was exiled to ", verb: "exiled", layout: Layout::PlaceThenInYear },
    Pattern { cue: " was exiled in ", verb: "exiled", layout: Layout::YearThenInPlace },
    Pattern { cue: " lived in ", verb: "lived", layout: Layout::PlaceThenInYear },
    Pattern { cue: " resided in ", verb: "lived", layout: Layout::PlaceThenInYear },
    Pattern { cue: " moved to ", verb: "moved", layout: Layout::PlaceThenInYear },
    Pattern { cue: " returned to ", verb: "moved", layout: Layout::PlaceThenInYear },
    Pattern { cue: " arrived in ", verb: "moved", layout: Layout::PlaceThenInYear },
    Pattern { cue: " visited ", verb: "visited", layout: Layout::PlaceThenInYear },
    Pattern { cue: " signed ", verb: "signed", layout: Layout::ObjectYearPlace },
    Pattern { cue: " negotiated ", verb: "signed", layout: Layout::ObjectYearPlace },
    Pattern { cue: " met ", verb: "met", layout: Layout::ObjectYearPlace },
    Pattern { cue: " invaded ", verb: "fought", layout: Layout::PlaceThenInYear },
    Pattern { cue: " was unveiled in ", verb: "unveiled", layout: Layout::YearThenInPlace },
    Pattern { cue: " enrolled at ", verb: "studied", layout: Layout::PlaceThenInYear },
    Pattern { cue: " studied in ", verb: "studied", layout: Layout::PlaceThenInYear },
    Pattern { cue: " sailed from ", verb: "sailed", layout: Layout::PlaceThenInYear },
    Pattern { cue: " landed at ", verb: "sailed", layout: Layout::PlaceThenInYear },
    Pattern { cue: " founded ", verb: "founded", layout: Layout::PlaceThenInYear },
];

fn try_structured(text: &str, lower: &str, pattern: &Pattern) -> Option<ExtractedTuple> {
    let cue_idx = lower.find(pattern.cue)?;
    let person = person_before(text, cue_idx).or_else(|| find_person_alias(text))?;
    let after = &text[cue_idx + pattern.cue.len()..];
    let (time, place) = match pattern.layout {
        Layout::YearThenInPlace => parse_year_then_in_place(after)?,
        Layout::PlaceThenInYear => parse_place_then_in_year(after)?,
        Layout::ObjectYearPlace => parse_object_year_place(after)?,
    };
    if !is_clean_place(&place) {
        return None;
    }
    Some(ExtractedTuple {
        person,
        time,
        place,
        verb: Some(pattern.verb.into()),
    })
}

// --- dense Wikipedia prose ------------------------------------------------

const PERSON_ALIASES: &[(&str, &str)] = &[
    ("napoleon bonaparte", "Napoleon"),
    ("napoléon bonaparte", "Napoleon"),
    ("napoleon i", "Napoleon"),
    ("emperor napoleon", "Napoleon"),
    ("general bonaparte", "Napoleon"),
    ("bonaparte", "Napoleon"),
    ("napoleon", "Napoleon"),
    ("napoléon", "Napoleon"),
    ("marie skłodowska-curie", "Marie Curie"),
    ("marie sklodowska-curie", "Marie Curie"),
    ("marie curie", "Marie Curie"),
    ("madame curie", "Marie Curie"),
    ("victor hugo", "Victor Hugo"),
    ("leonardo da vinci", "Leonardo da Vinci"),
    ("christopher columbus", "Christopher Columbus"),
    ("alan turing", "Alan Turing"),
    ("cleopatra vii", "Cleopatra"),
    ("cleopatra", "Cleopatra"),
    ("honoré de balzac", "Honoré de Balzac"),
    ("honore de balzac", "Honoré de Balzac"),
    ("de balzac", "Honoré de Balzac"),
    ("balzac", "Honoré de Balzac"),
];

const VERB_CUES: &[(&str, &str)] = &[
    (r"was born|born in|birth of", "born"),
    (r"\bdied\b|death of|passed away", "died"),
    (r"married|wedding|marriage to|marriage with", "married"),
    (r"divorced|divorce|annulled|anulled", "divorced"),
    (r"studied|educated|enrolled|school at|to study|study in|study at|matriculat", "studied"),
    (
        r"fought|defeated|victory at|won at|battle of|besieged|invaded|captured|commanded|siege of",
        "fought",
    ),
    (
        r"signed|treaty of|negotiated|peace of|concordat|alliance with|diplomacy",
        "signed",
    ),
    (r"crowned|coronation|proclaimed emperor|became emperor|became consul|first consul", "crowned"),
    (r"exiled|exile to|banished|abdicated|abdication", "exiled"),
    (
        r"lived|resided|residence|stayed|settled|moved to|returned to|arrived in|arrived at|left for|departed",
        "lived",
    ),
    (r"visited|travelled|traveled|toured|went to", "visited"),
    (r"\bmet\b|received|audience|congress of", "met"),
    (r"imprisoned|detained|confined|captive", "imprisoned"),
    (r"published|napoleonic code|code civil|\bwrote\b", "published"),
    (r"appointed|promoted|commissioned|named general|elected", "appointed"),
    (r"set sail|sailed|embarked|voyage|expedition|landed", "sailed"),
    (r"founded|founding", "founded"),
    (r"worked at|worked in|laboratory", "worked"),
    (r"awarded|nobel|received the", "awarded"),
    (r"painted|painting", "painted"),
    (r"discovered|isolated|research at", "discovered"),
    (r"taught|teaching at", "taught"),
    (r"fled|escaped", "exiled"),
    (r"invented|designed|decoded", "invented"),
];

/// Places ordered longest-first for greedy match.
const GAZETTEER: &[&str] = &[
    "brienne-le-chateau",
    "boulogne-sur-mer",
    "saint-helena",
    "st helena",
    "saint helena",
    "preussisch eylau",
    "maloyaroslavets",
    "arcis-sur-aube",
    "campo formio",
    "quatre bras",
    "french invasion of russia",
    "hauteville house",
    "palos de la frontera",
    "institut curie",
    "collège de france",
    "college de france",
    "place des vosges",
    "école normale",
    "ecole normale",
    "bletchley park",
    "la gomera",
    "la navidad",
    "puerto rico",
    "san salvador",
    "notre-dame",
    "notre dame",
    "saint-cloud",
    "saint cloud",
    "portoferraio",
    "fontainebleau",
    "schönbrunn",
    "schonbrunn",
    "pressburg",
    "lunéville",
    "luneville",
    "clos luce",
    "clos lucé",
    "maida vale",
    "canary islands",
    "hut 8",
    "austerlitz",
    "waterloo",
    "borodino",
    "smolensk",
    "leipzig",
    "wagram",
    "aspern",
    "essling",
    "marengo",
    "friedland",
    "jena",
    "auerstedt",
    "toulon",
    "ajaccio",
    "corsica",
    "malmaison",
    "tuileries",
    "compiegne",
    "compiègne",
    "grenoble",
    "avignon",
    "antibes",
    "auxonne",
    "valence",
    "cairo",
    "egypt",
    "pyramids",
    "alexandria",
    "jaffa",
    "acre",
    "malta",
    "milan",
    "turin",
    "genoa",
    "venice",
    "florence",
    "naples",
    "rome",
    "vienna",
    "berlin",
    "warsaw",
    "kraków",
    "krakow",
    "cracow",
    "zakopane",
    "tilsit",
    "eylau",
    "ulm",
    "ratisbon",
    "regensburg",
    "munich",
    "dresden",
    "prague",
    "erfurt",
    "moscow",
    "berezina",
    "vilnius",
    "vilna",
    "minsk",
    "vyazma",
    "krasnoi",
    "liggy",
    "ligny",
    "wavre",
    "plancenoit",
    "charleroi",
    "brussels",
    "belgium",
    "elba",
    "longwood",
    "paris",
    "lyon",
    "nice",
    "spain",
    "portugal",
    "madrid",
    "lisbon",
    "russia",
    "prussia",
    "austria",
    "italy",
    "germany",
    "france",
    "england",
    "britain",
    "arcola",
    "lodi",
    "rivoli",
    "mantua",
    "amiens",
    "boulogne",
    "craonne",
    "montereau",
    "montmirail",
    "champaubert",
    "vauchamps",
    "lützen",
    "lutzen",
    "bautzen",
    "hanau",
    "kulm",
    "besançon",
    "besancon",
    "guernsey",
    "jersey",
    "amboise",
    "vinci",
    "anchiano",
    "palos",
    "hispaniola",
    "valladolid",
    "barcelona",
    "stockholm",
    "sceaux",
    "passy",
    "sorbonne",
    "sherborne",
    "princeton",
    "manchester",
    "wilmslow",
    "hampton",
    "bletchley",
    "tarsus",
    "actium",
    "pelusium",
    "antioch",
    "cádiz",
    "cadiz",
    "seville",
    "sevilla",
    "gomera",
    "azores",
    "panthéon",
    "pantheon",
    "espci",
    "invalides",
    "vatican",
    "pisa",
    "bologna",
    "urbino",
    "padua",
    "siena",
    "ferrara",
    "granada",
    "córdoba",
    "cordoba",
    "toledo",
    "bordeaux",
    "nantes",
    "dijon",
    "laon",
    "reims",
    "trafalgar",
    "aboukir",
    "gibraltar",
    "teddington",
    "trinidad",
    "venezuela",
    "panama",
    "cyprus",
    "rhodes",
    "athens",
    "memphis",
    "oslo",
    "tours",
    "saché",
    "sache",
    "vendôme",
    "vendome",
    "château de saché",
    "chateau de sache",
    "geneva",
    "berdychiv",
    "berdichev",
];

fn try_prose_dense(
    text: &str,
    lower: &str,
    page_title: &str,
    carry_year: Option<&str>,
    carry_place: Option<&str>,
) -> Option<ExtractedTuple> {
    let page_lower = page_title.to_lowercase();
    let (person, year_min, year_max) = resolve_dense_subject(text, &page_lower)?;
    let own_year = find_year_in_window(lower, year_min, year_max);
    let year = own_year.clone().or_else(|| {
        let surface = carry_year?;
        let value = parse_year_surface(surface)?;
        if (year_min..=year_max).contains(&value) {
            Some(surface.to_string())
        } else {
            None
        }
    })?;
    let own_place = find_place(lower, page_title);
    let place = own_place
        .clone()
        .or_else(|| carry_place.map(str::to_string))?;
    if (own_year.is_none() || own_place.is_none()) && !has_subject_hook(text, lower) {
        return None;
    }
    if !is_clean_place(&place) {
        return None;
    }
    let verb = find_verb_cue(lower).or_else(|| {
        if page_lower.starts_with("battle of ") {
            Some("fought".into())
        } else if page_lower.starts_with("treaty of ") || page_lower.contains("concordat") {
            Some("signed".into())
        } else if page_lower.starts_with("congress of ") {
            Some("met".into())
        } else {
            None
        }
    })?;

    if verb == "associated" {
        return None;
    }

    Some(ExtractedTuple {
        person,
        time: year,
        place,
        verb: Some(verb),
    })
}

fn resolve_dense_subject(text: &str, page_lower: &str) -> Option<(String, i32, i32)> {
    if let Some(person) = find_person_alias(text) {
        let window = subject_year_window(&person);
        return Some((person, window.0, window.1));
    }
    const PAGE_SUBJECTS: &[(&str, &str, i32, i32)] = &[
        ("napoleon", "Napoleon", 1765, 1865),
        ("napoléon", "Napoleon", 1765, 1865),
        ("marie curie", "Marie Curie", 1865, 1935),
        ("curie", "Marie Curie", 1865, 1935),
        ("victor hugo", "Victor Hugo", 1800, 1885),
        ("leonardo", "Leonardo da Vinci", 1450, 1520),
        ("columbus", "Christopher Columbus", 1440, 1510),
        ("turing", "Alan Turing", 1910, 1960),
        ("bletchley", "Alan Turing", 1910, 1960),
        ("cleopatra", "Cleopatra", -80, 30),
        ("balzac", "Honoré de Balzac", 1795, 1860),
        ("comédie humaine", "Honoré de Balzac", 1795, 1860),
        ("comedie humaine", "Honoré de Balzac", 1795, 1860),
        ("père goriot", "Honoré de Balzac", 1795, 1860),
        ("pere goriot", "Honoré de Balzac", 1795, 1860),
        ("eugénie grandet", "Honoré de Balzac", 1795, 1860),
        ("lost illusions", "Honoré de Balzac", 1795, 1860),
        ("illusions perdues", "Honoré de Balzac", 1795, 1860),
        ("cousin bette", "Honoré de Balzac", 1795, 1860),
        ("french consulate", "Napoleon", 1765, 1865),
        ("first french empire", "Napoleon", 1765, 1865),
        ("hundred days", "Napoleon", 1765, 1865),
        ("peninsular war", "Napoleon", 1765, 1865),
        ("continental system", "Napoleon", 1765, 1865),
        ("napoleonic", "Napoleon", 1765, 1865),
        ("les misérables", "Victor Hugo", 1800, 1885),
        ("les miserables", "Victor Hugo", 1800, 1885),
        ("notre-dame de paris", "Victor Hugo", 1800, 1885),
        ("hauteville", "Victor Hugo", 1800, 1885),
        ("hernani", "Victor Hugo", 1800, 1885),
        ("ruy blas", "Victor Hugo", 1800, 1885),
        ("toilers of the sea", "Victor Hugo", 1800, 1885),
        ("the man who laughs", "Victor Hugo", 1800, 1885),
        ("mona lisa", "Leonardo da Vinci", 1450, 1520),
        ("last supper", "Leonardo da Vinci", 1450, 1520),
        ("vitruvian", "Leonardo da Vinci", 1450, 1520),
        ("codex atlanticus", "Leonardo da Vinci", 1450, 1520),
        ("clos lucé", "Leonardo da Vinci", 1450, 1520),
        ("clos luce", "Leonardo da Vinci", 1450, 1520),
        ("voyages of christopher", "Christopher Columbus", 1440, 1510),
        ("palos de la frontera", "Christopher Columbus", 1440, 1510),
        ("hispaniola", "Christopher Columbus", 1440, 1510),
        ("hut 8", "Alan Turing", 1910, 1960),
        ("enigma", "Alan Turing", 1910, 1960),
        ("manchester mark", "Alan Turing", 1910, 1960),
        ("automatic computing engine", "Alan Turing", 1910, 1960),
        ("ptolemaic", "Cleopatra", -80, 30),
        ("actium", "Cleopatra", -80, 30),
        ("caesarion", "Cleopatra", -80, 30),
        ("donations of alexandria", "Cleopatra", -80, 30),
        ("radium", "Marie Curie", 1865, 1935),
        ("polonium", "Marie Curie", 1865, 1935),
        ("institut curie", "Marie Curie", 1865, 1935),
        ("espci", "Marie Curie", 1865, 1935),
    ];
    for (needle, title, min, max) in PAGE_SUBJECTS {
        if page_lower.contains(needle) {
            return Some(((*title).into(), *min, *max));
        }
    }
    if page_lower.starts_with("battle of ")
        || page_lower.starts_with("treaty of ")
        || page_lower.starts_with("congress of ")
        || page_lower.starts_with("coup of ")
    {
        return Some(("Napoleon".into(), 1765, 1865));
    }
    None
}

fn subject_year_window(person: &str) -> (i32, i32) {
    match person {
        "Marie Curie" => (1865, 1935),
        "Victor Hugo" => (1800, 1885),
        "Leonardo da Vinci" => (1450, 1520),
        "Christopher Columbus" => (1440, 1510),
        "Alan Turing" => (1910, 1960),
        "Cleopatra" => (-80, 30),
        "Honoré de Balzac" => (1795, 1860),
        _ => (1765, 1865),
    }
}

fn is_clean_place(place: &str) -> bool {
    let trimmed = place.trim();
    if trimmed.chars().count() < 2 || trimmed.chars().count() > 40 {
        return false;
    }
    let lower = trimmed.to_lowercase();
    if lower.contains(" and ")
        || lower.contains(" was ")
        || lower.contains(" were ")
        || lower.contains(" the french")
        || lower.contains(" commissioned")
        || lower.contains(" officer")
    {
        return false;
    }
    true
}

fn find_year_in_window(lower: &str, min_year: i32, max_year: i32) -> Option<String> {
    let mut last = None;
    for (value, surface) in extract_year_surfaces(lower) {
        if (min_year..=max_year).contains(&value) {
            last = Some(surface);
        }
    }
    last
}

fn parse_year_surface(surface: &str) -> Option<i32> {
    let trimmed = surface.trim();
    let lower = trimmed.to_ascii_lowercase();
    if let Some(digits) = lower.strip_suffix(" bc") {
        return digits.trim().parse::<i32>().ok().map(|y| -y);
    }
    trimmed.parse().ok()
}

fn extract_year_surfaces(lower: &str) -> Vec<(i32, String)> {
    let bytes = lower.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i].is_ascii_digit() {
            let start = i;
            while i < bytes.len() && bytes[i].is_ascii_digit() {
                i += 1;
            }
            let digits = &lower[start..i];
            if digits.len() > 4 {
                continue;
            }
            let Ok(abs_year) = digits.parse::<i32>() else {
                continue;
            };
            let rest = lower[i..].trim_start();
            let is_bc = rest.starts_with("bc")
                || rest.starts_with("b.c")
                || rest.starts_with("bce")
                || rest.starts_with("b.c.e");
            if is_bc {
                if (1..=4000).contains(&abs_year) {
                    out.push((-abs_year, format!("{abs_year} BC")));
                }
                continue;
            }
            if digits.len() == 4 && (1000..=2100).contains(&abs_year) {
                out.push((abs_year, format!("{abs_year:04}")));
            }
        } else {
            i += 1;
        }
    }
    out
}

fn has_subject_hook(text: &str, lower: &str) -> bool {
    if find_person_alias(text).is_some() {
        return true;
    }
    for pronoun in ["she", "he", "they", "herself", "himself"] {
        if contains_word(lower, pronoun) {
            return true;
        }
    }
    false
}

fn contains_word(hay: &str, needle: &str) -> bool {
    let Some(idx) = hay.find(needle) else {
        return false;
    };
    let bytes = hay.as_bytes();
    let before_ok = idx == 0 || !bytes[idx - 1].is_ascii_alphanumeric();
    let end = idx + needle.len();
    let after_ok = end >= bytes.len() || !bytes[end].is_ascii_alphanumeric();
    before_ok && after_ok
}

fn find_person_alias(text: &str) -> Option<String> {
    let lower = text.to_lowercase();
    for (alias, wiki_title) in PERSON_ALIASES {
        if let Some(idx) = lower.find(alias) {
            let before_ok = idx == 0 || !lower.as_bytes()[idx - 1].is_ascii_alphanumeric();
            let end = idx + alias.len();
            let after_ok = end >= lower.len() || !lower.as_bytes()[end].is_ascii_alphanumeric();
            if before_ok && after_ok {
                return Some((*wiki_title).into());
            }
        }
    }
    None
}

fn find_year(lower: &str) -> Option<String> {
    let bytes = lower.as_bytes();
    let mut i = 0;
    let mut last = None;
    while i + 4 <= bytes.len() {
        if bytes[i..i + 4].iter().all(|b| b.is_ascii_digit()) {
            let boundary_before = i == 0 || !bytes[i - 1].is_ascii_digit();
            let boundary_after = i + 4 == bytes.len() || !bytes[i + 4].is_ascii_digit();
            if boundary_before && boundary_after {
                let y: i32 = std::str::from_utf8(&bytes[i..i + 4])
                    .ok()?
                    .parse()
                    .ok()?;
                if (1000..=2100).contains(&y) {
                    last = Some(format!("{y:04}"));
                }
            }
            i += 4;
        } else {
            i += 1;
        }
    }
    last
}

fn find_place(lower: &str, page_title: &str) -> Option<String> {
    find_gazetteer_place(lower)
        .or_else(|| place_from_page_title(page_title))
}

fn find_gazetteer_place(lower: &str) -> Option<String> {
    for place in GAZETTEER {
        if let Some(idx) = lower.find(place) {
            let before_ok = idx == 0 || !lower.as_bytes()[idx - 1].is_ascii_alphanumeric();
            let end = idx + place.len();
            let after_ok = end >= lower.len() || !lower.as_bytes()[end].is_ascii_alphanumeric();
            if before_ok && after_ok {
                return Some(title_case_place(place));
            }
        }
    }
    None
}

fn place_from_page_title(page_title: &str) -> Option<String> {
    let title = page_title.trim();
    for prefix in ["Battle of ", "Treaty of ", "Treaties of ", "Congress of ", "Coup of ", "Siege of "] {
        if let Some(rest) = title.strip_prefix(prefix) {
            let name = rest.split('(').next()?.trim();
            if name.len() >= 2 {
                return Some(name.to_string());
            }
        }
    }
    let stripped = title.split('(').next()?.trim();
    let hit = find_gazetteer_place(&stripped.to_lowercase())?;
    let stripped_lower = stripped.to_lowercase();
    let hit_lower = hit.to_lowercase();
    if stripped_lower == hit_lower || (stripped_lower.starts_with(&hit_lower) && hit.len() >= 8) {
        return Some(hit);
    }
    None
}

fn find_verb_cue(lower: &str) -> Option<String> {
    for (pat, verb) in VERB_CUES {
        if regex_simple_or(lower, pat) {
            return Some((*verb).into());
        }
    }
    None
}

/// Tiny alternation matcher for `a|b|c` and optional `\b` anchors (no full regex crate).
fn regex_simple_or(hay: &str, pattern: &str) -> bool {
    for alt in pattern.split('|') {
        let alt = alt.trim();
        let alt = alt.strip_prefix(r"\b").unwrap_or(alt);
        let alt = alt.strip_suffix(r"\b").unwrap_or(alt);
        if alt.is_empty() {
            continue;
        }
        if hay.contains(alt) {
            return true;
        }
    }
    false
}

fn title_case_place(place: &str) -> String {
    place
        .split(|c: char| c == '-' || c == ' ')
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(f) => f.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(if place.contains('-') { "-" } else { " " })
}

// --- shared parsers -------------------------------------------------------

fn person_before(text: &str, cue_idx: usize) -> Option<String> {
    if let Some(alias) = find_person_alias(&text[..cue_idx]) {
        return Some(alias);
    }
    let before = text[..cue_idx].trim();
    let lowered = before.to_lowercase();
    let name_src = if let Some(idx) = lowered.rfind(" of ") {
        before[idx + 4..].trim()
    } else if let Some(idx) = lowered.rfind(". ") {
        before[idx + 2..].trim()
    } else {
        before
    };
    let person = strip_wiki_markup(name_src)
        .trim()
        .trim_matches(|c: char| c == '"' || c == '\'')
        .to_string();
    if person.chars().count() < 3 {
        return None;
    }
    Some(person)
}

fn parse_year_then_in_place(after: &str) -> Option<(String, String)> {
    let after = after.trim();
    let mut parts = after.splitn(2, " in ");
    let year = find_year(&parts.next()?.to_lowercase())?;
    let place = clean_place(parts.next()?.trim())?;
    Some((year, place))
}

fn parse_place_then_in_year(after: &str) -> Option<(String, String)> {
    let after = after.trim();
    let in_idx = after.to_lowercase().rfind(" in ")?;
    let place = clean_place(after[..in_idx].trim())?;
    let year = find_year(&after[in_idx + 4..].to_lowercase())?;
    Some((year, place))
}

fn parse_object_year_place(after: &str) -> Option<(String, String)> {
    let after = after.trim();
    let lower = after.to_lowercase();
    let first_in = lower.find(" in ")?;
    let rest = &after[first_in + 4..];
    let rest_lower = rest.to_lowercase();
    if let Some(second_in) = rest_lower.find(" in ") {
        let year = find_year(&rest[..second_in].to_lowercase())?;
        let place = clean_place(rest[second_in + 4..].trim())?;
        return Some((year, place));
    }
    parse_place_then_in_year(after)
}

fn clean_place(surface: &str) -> Option<String> {
    let place = surface
        .trim()
        .trim_end_matches(|c: char| c == '.' || c == ',' || c == ';' || c == ')')
        .trim()
        .to_string();
    if place.chars().count() < 2 || place.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    Some(place)
}

fn strip_wiki_markup(input: &str) -> String {
    input.replace("'''", "").replace("''", "").trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_born_in_pattern() {
        let tuples = mock_extract_text("Alan Turing was born in 1912 in London.", None);
        assert_eq!(tuples.len(), 1);
        assert_eq!(tuples[0].person, "Alan Turing");
        assert_eq!(tuples[0].time, "1912");
        assert_eq!(tuples[0].place, "London");
    }

    #[test]
    fn dense_prose_napoleon_siege_of_toulon() {
        let text = "He rose rapidly through the ranks after winning the siege of Toulon in 1793 and fighting the War of the First Coalition.";
        let tuples = mock_extract_text(text, Some("Napoleon"));
        assert!(!tuples.is_empty());
        assert_eq!(tuples[0].person, "Napoleon");
        assert_eq!(tuples[0].time, "1793");
        assert!(tuples[0].place.to_lowercase().contains("toulon"));
    }

    #[test]
    fn battle_page_supplies_place_and_person() {
        let text = "The armies clashed on 18 June 1815 near the ridge.";
        let tuples = mock_extract_text(text, Some("Battle of Waterloo"));
        assert_eq!(tuples.len(), 1);
        assert_eq!(tuples[0].person, "Napoleon");
        assert_eq!(tuples[0].time, "1815");
        assert!(tuples[0].place.to_lowercase().contains("waterloo"));
        assert_eq!(tuples[0].verb.as_deref(), Some("fought"));
    }

    #[test]
    fn diplomatic_treaty_cue() {
        let text = "Napoleon signed the Treaty of Tilsit with Russia in 1807 at Tilsit.";
        let tuples = mock_extract_text(text, Some("Treaties of Tilsit"));
        assert!(!tuples.is_empty());
        assert_eq!(tuples[0].verb.as_deref(), Some("signed"));
        assert_eq!(tuples[0].time, "1807");
    }

    #[test]
    fn dense_prose_curie_warsaw() {
        let text = "Marie Curie was born in 1867 in Warsaw and later moved to Paris.";
        let tuples = mock_extract_text(text, Some("Marie Curie"));
        assert!(!tuples.is_empty());
        assert_eq!(tuples[0].person, "Marie Curie");
        assert_eq!(tuples[0].time, "1867");
        assert!(tuples[0].place.to_lowercase().contains("warsaw"));
    }

    #[test]
    fn carries_year_across_sentences_on_same_page() {
        let items = vec![
            BatchInputItem {
                id: "1".into(),
                text: "In 1891 she left Warsaw.".into(),
                page_title: Some("Marie Curie".into()),
            },
            BatchInputItem {
                id: "2".into(),
                text: "She enrolled at the Sorbonne in Paris.".into(),
                page_title: Some("Marie Curie".into()),
            },
        ];
        let out = mock_extract(&items);
        assert!(!out[1].tuples.is_empty());
        assert_eq!(out[1].tuples[0].person, "Marie Curie");
        assert_eq!(out[1].tuples[0].time, "1891");
        let place = out[1].tuples[0].place.to_lowercase();
        assert!(place.contains("paris") || place.contains("sorbonne"));
    }
}
