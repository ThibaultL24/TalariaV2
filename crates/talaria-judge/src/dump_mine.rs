// crates/talaria-judge/src/dump_mine.rs
//! Keyword mining over dump sentences (anecdotes + extra life-event cues).

use crate::place::find_place_in_text;

pub const EXTRACTOR_ANECDOTE: &str = "dump:anecdote";
pub const EXTRACTOR_KEYWORDS: &str = "dump_keywords";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MinedCandidate {
    pub person: String,
    pub time: String,
    pub place: String,
    pub verb: String,
    pub extractor: &'static str,
}

/// Year/place inherited from earlier sentences on the same page.
#[derive(Debug, Clone, Default)]
pub struct MineCarry {
    pub year: Option<String>,
    pub year_value: Option<i32>,
    pub place: Option<String>,
}

impl MineCarry {
    pub fn absorb(&mut self, text: &str, page_title: &str) {
        let cleaned = text.trim();
        if cleaned.len() < 20 {
            return;
        }
        let lower = cleaned.to_ascii_lowercase();
        if has_any(&lower, COMMEMORATIVE) {
            return;
        }
        let Some(subject) = resolve_subject(&lower, page_title) else {
            return;
        };
        if let Some(found) = find_year_in_window(&lower, subject.year_min, subject.year_max) {
            self.year = Some(found.surface);
            self.year_value = Some(found.year);
        }
        if let Some(place) = resolve_place(cleaned, page_title) {
            self.place = Some(place);
        }
    }
}

struct Subject {
    wiki_title: &'static str,
    aliases: &'static [&'static str],
    page_needles: &'static [&'static str],
    year_min: i32,
    year_max: i32,
}

const SUBJECTS: &[Subject] = &[
    Subject {
        wiki_title: "Napoleon",
        aliases: &[
            "napoleon bonaparte",
            "napoléon bonaparte",
            "napoleon i",
            "emperor napoleon",
            "general bonaparte",
            "bonaparte",
            "napoleon",
            "napoléon",
        ],
        page_needles: &[
            "napoleon",
            "napoléon",
            "french consulate",
            "first french empire",
            "hundred days",
            "peninsular war",
            "continental system",
            "napoleonic",
            "brumaire",
            "waterloo",
            "austerlitz",
            "borodino",
            "jena",
            "wagram",
            "tilsit",
            "amiens",
            "marengo",
            "eylau",
            "friedland",
            "leipzig",
            "malmaison",
            "campo formio",
            "concordat",
            "toulon",
            "pyramids",
            "ligny",
        ],
        year_min: 1765,
        year_max: 1865,
    },
    Subject {
        wiki_title: "Marie Curie",
        aliases: &[
            "marie skłodowska-curie",
            "marie sklodowska-curie",
            "marie skłodowska",
            "marie curie",
            "madame curie",
        ],
        page_needles: &[
            "curie",
            "radium",
            "polonium",
            "institut curie",
            "espci",
            "skłodowska",
            "sklodowska",
        ],
        year_min: 1865,
        year_max: 1935,
    },
    Subject {
        wiki_title: "Victor Hugo",
        aliases: &["victor hugo", "hugo"],
        page_needles: &[
            "victor hugo",
            "les misérables",
            "les miserables",
            "notre-dame de paris",
            "hauteville",
            "hernani",
            "ruy blas",
            "toilers of the sea",
            "the man who laughs",
        ],
        year_min: 1800,
        year_max: 1885,
    },
    Subject {
        wiki_title: "Leonardo da Vinci",
        aliases: &["leonardo da vinci", "leonardo"],
        page_needles: &[
            "leonardo",
            "mona lisa",
            "last supper",
            "vitruvian",
            "codex atlanticus",
            "clos lucé",
            "clos luce",
        ],
        year_min: 1450,
        year_max: 1520,
    },
    Subject {
        wiki_title: "Christopher Columbus",
        aliases: &["christopher columbus", "columbus", "cristoforo colombo"],
        page_needles: &[
            "columbus",
            "voyages of christopher",
            "palos de la frontera",
            "santa maría",
            "santa maria (ship)",
            "la niña",
            "la pinta",
            "hispaniola",
        ],
        year_min: 1440,
        year_max: 1510,
    },
    Subject {
        wiki_title: "Alan Turing",
        aliases: &["alan turing", "turing"],
        page_needles: &[
            "turing",
            "bletchley",
            "enigma",
            "hut 8",
            "manchester mark",
            "automatic computing engine",
        ],
        year_min: 1910,
        year_max: 1960,
    },
    Subject {
        wiki_title: "Cleopatra",
        aliases: &["cleopatra vii", "cleopatra"],
        page_needles: &[
            "cleopatra",
            "ptolemaic",
            "actium",
            "caesarion",
            "donations of alexandria",
        ],
        year_min: -80,
        year_max: 30,
    },
    Subject {
        wiki_title: "Honoré de Balzac",
        aliases: &[
            "honoré de balzac",
            "honore de balzac",
            "honoré balzac",
            "honore balzac",
            "de balzac",
            "balzac",
        ],
        page_needles: &[
            "balzac",
            "comédie humaine",
            "comedie humaine",
            "père goriot",
            "pere goriot",
            "eugénie grandet",
            "eugenie grandet",
            "illusions perdues",
            "lost illusions",
            "cousine bette",
            "cousin bette",
            "château de saché",
            "chateau de sache",
        ],
        year_min: 1795,
        year_max: 1860,
    },
];

const ANECDOTE_CUES: &[&str] = &[
    "anecdote",
    "according to legend",
    "legend has it",
    "the story goes",
    "popular story",
    "apocryphal",
    "once told",
    "is said to have",
    "it is said that",
    "it is said ",
    "reputed to have",
    "tradition holds",
    "according to a popular",
    "famous anecdote",
    "an apocryphal",
    "folklore",
    "learned about",
    "learnt about",
    "learned of",
    "learnt of",
    "from a newspaper",
    "newspaper that he read",
    "newspaper that she read",
    "read in a café",
    "read in a cafe",
];

const COMMEMORATIVE: &[&str] = &[
    "statue",
    "museum",
    "memorial",
    "plaque",
    "was unveiled",
    "street named",
    "named after",
];

const VERB_CUES: &[(&str, &str)] = &[
    ("was born", "born"),
    ("born in", "born"),
    (" died", "died"),
    ("death of", "died"),
    ("married", "married"),
    ("studied", "studied"),
    ("educated", "studied"),
    ("to study", "studied"),
    ("study in", "studied"),
    ("study at", "studied"),
    ("enrolled", "studied"),
    ("matriculat", "studied"),
    ("fought", "fought"),
    ("battle of", "fought"),
    ("defeated", "fought"),
    ("captured", "fought"),
    ("campaign", "fought"),
    ("retreat", "fought"),
    ("signed", "signed"),
    ("treaty of", "signed"),
    ("crowned", "crowned"),
    ("exiled", "exiled"),
    ("fled", "exiled"),
    ("lived in", "lived"),
    ("resided", "lived"),
    ("settled in", "lived"),
    ("moved to", "moved"),
    ("returned to", "moved"),
    ("arrived in", "moved"),
    ("arrived at", "moved"),
    ("left for", "moved"),
    ("departed", "moved"),
    ("visited", "visited"),
    ("travelled", "visited"),
    ("traveled", "visited"),
    (" met ", "met"),
    ("published", "published"),
    ("wrote ", "published"),
    ("painted", "painted"),
    ("painting", "painted"),
    ("discovered", "discovered"),
    ("isolated", "discovered"),
    ("research at", "discovered"),
    ("awarded", "awarded"),
    ("nobel", "awarded"),
    ("received the", "awarded"),
    ("sailed", "sailed"),
    ("set sail", "sailed"),
    ("embarked", "sailed"),
    ("landed", "sailed"),
    ("voyage", "sailed"),
    ("expedition", "sailed"),
    ("founded", "founded"),
    ("worked at", "worked"),
    ("worked in", "worked"),
    ("worked as", "worked"),
    ("laboratory", "worked"),
    ("taught", "taught"),
    ("teaching at", "taught"),
    ("joined", "joined"),
    ("appointed", "appointed"),
    ("elected", "appointed"),
    ("commissioned", "appointed"),
    ("imprisoned", "imprisoned"),
    ("invented", "invented"),
    ("designed", "invented"),
    ("decoded", "worked"),
    ("buried", "died"),
];

pub fn mine_sentence(text: &str, page_title: &str) -> Vec<MinedCandidate> {
    mine_sentence_with_carry(text, page_title, &MineCarry::default())
}

pub fn mine_sentence_with_carry(
    text: &str,
    page_title: &str,
    carry: &MineCarry,
) -> Vec<MinedCandidate> {
    let cleaned = text.trim();
    if cleaned.len() < 28 {
        return vec![];
    }
    let lower = cleaned.to_ascii_lowercase();
    let Some(subject) = resolve_subject(&lower, page_title) else {
        return vec![];
    };
    let own_year = find_year_in_window(&lower, subject.year_min, subject.year_max);
    let year = own_year
        .as_ref()
        .map(|found| found.surface.clone())
        .or_else(|| {
            let value = carry.year_value?;
            if (subject.year_min..=subject.year_max).contains(&value) {
                carry.year.clone()
            } else {
                None
            }
        });
    let Some(year) = year else {
        return vec![];
    };
    let own_place = resolve_place(cleaned, page_title);
    let Some(place) = own_place.clone().or_else(|| carry.place.clone()) else {
        return vec![];
    };
    let used_carry = own_year.is_none() || own_place.is_none();
    if used_carry && !has_subject_hook(&lower, subject) {
        return vec![];
    }

    let anecdote = has_any(&lower, ANECDOTE_CUES);
    if anecdote {
        return vec![MinedCandidate {
            person: subject.wiki_title.into(),
            time: year,
            place,
            verb: "anecdoted".into(),
            extractor: EXTRACTOR_ANECDOTE,
        }];
    }

    if has_any(&lower, COMMEMORATIVE) {
        return vec![];
    }

    let Some(mut verb) = find_verb(&lower) else {
        return vec![];
    };
    if verb == "died" && death_refers_to_other_person(&lower, subject.aliases) {
        verb = "grieved".into();
    }
    vec![MinedCandidate {
        person: subject.wiki_title.into(),
        time: year,
        place,
        verb,
        extractor: EXTRACTOR_KEYWORDS,
    }]
}

fn resolve_place(text: &str, page_title: &str) -> Option<String> {
    find_place_in_text(text).or_else(|| place_from_page_title(page_title))
}

fn resolve_subject(lower: &str, page_title: &str) -> Option<&'static Subject> {
    if let Some(subject) = SUBJECTS
        .iter()
        .find(|subject| subject.aliases.iter().any(|alias| contains_word(lower, alias)))
    {
        return Some(subject);
    }

    let page_lower = page_title.to_ascii_lowercase();
    if let Some(subject) = SUBJECTS.iter().find(|subject| {
        page_lower == subject.wiki_title.to_ascii_lowercase()
            || subject
                .page_needles
                .iter()
                .any(|needle| page_lower.contains(needle))
    }) {
        return Some(subject);
    }

    let campaign = page_lower.starts_with("battle of ")
        || page_lower.starts_with("treaty of ")
        || page_lower.starts_with("treaties of ")
        || page_lower.starts_with("congress of ")
        || page_lower.starts_with("siege of ");
    if !campaign {
        return None;
    }
    let years = extract_years(lower);
    SUBJECTS.iter().find(|subject| {
        years
            .iter()
            .any(|year| (subject.year_min..=subject.year_max).contains(&year.year))
    })
}

fn find_verb(lower: &str) -> Option<String> {
    VERB_CUES
        .iter()
        .find(|(cue, _)| lower.contains(cue))
        .map(|(_, verb)| (*verb).to_string())
}

fn find_year_in_window(lower: &str, min_year: i32, max_year: i32) -> Option<FoundYear> {
    let mut last = None;
    for found in extract_years(lower) {
        if (min_year..=max_year).contains(&found.year) {
            last = Some(found);
        }
    }
    last
}

struct FoundYear {
    year: i32,
    surface: String,
}

fn extract_years(lower: &str) -> Vec<FoundYear> {
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
                    out.push(FoundYear {
                        year: -abs_year,
                        surface: format!("{abs_year} BC"),
                    });
                }
                continue;
            }
            if digits.len() == 4 && (1000..=2100).contains(&abs_year) {
                out.push(FoundYear {
                    year: abs_year,
                    surface: format!("{abs_year:04}"),
                });
            }
        } else {
            i += 1;
        }
    }
    out
}

fn place_from_page_title(page_title: &str) -> Option<String> {
    let title = page_title.trim();
    for prefix in [
        "Battle of ",
        "Treaty of ",
        "Treaties of ",
        "Congress of ",
        "Siege of ",
        "Coup of ",
    ] {
        if let Some(rest) = title.strip_prefix(prefix) {
            let name = rest.split('(').next()?.trim();
            if name.len() >= 2 {
                return Some(name.to_string());
            }
        }
    }
    let stripped = title.split('(').next()?.trim();
    let hit = find_place_in_text(stripped)?;
    let stripped_lower = stripped.to_ascii_lowercase();
    let hit_lower = hit.to_ascii_lowercase();
    if stripped_lower == hit_lower || (stripped_lower.starts_with(&hit_lower) && hit.len() >= 8) {
        return Some(hit);
    }
    None
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

fn has_any(hay: &str, needles: &[&str]) -> bool {
    needles.iter().any(|n| hay.contains(n))
}

fn has_subject_hook(lower: &str, subject: &Subject) -> bool {
    if subject
        .aliases
        .iter()
        .any(|alias| contains_word(lower, alias))
    {
        return true;
    }
    ["she", "he", "they", "herself", "himself"]
        .iter()
        .any(|pronoun| contains_word(lower, pronoun))
}

pub fn split_heading_chunks(text: &str) -> Vec<String> {
    let mut chunks = Vec::new();
    let mut rest = text;
    while let Some((idx, len)) = find_wiki_heading(rest) {
        let before = rest[..idx].trim();
        if !before.is_empty() {
            chunks.push(before.to_string());
        }
        rest = rest[idx + len..].trim_start();
    }
    let tail = rest.trim();
    if !tail.is_empty() {
        chunks.push(tail.to_string());
    }
    if chunks.is_empty() {
        vec![text.trim().to_string()]
    } else {
        chunks
    }
}

fn find_wiki_heading(text: &str) -> Option<(usize, usize)> {
    let bytes = text.as_bytes();
    let mut i = 0;
    while i + 4 < bytes.len() {
        if bytes[i] != b'=' {
            i += 1;
            continue;
        }
        let mut j = i;
        while j < bytes.len() && bytes[j] == b'=' {
            j += 1;
        }
        if j - i < 2 {
            i += 1;
            continue;
        }
        let mut k = j;
        while k < bytes.len() && bytes[k] != b'=' {
            k += 1;
        }
        let mut end = k;
        while end < bytes.len() && bytes[end] == b'=' {
            end += 1;
        }
        if end - k >= 2 && k > j {
            return Some((i, end - i));
        }
        i += 1;
    }
    None
}

pub fn death_refers_to_other_person(sentence_lc: &str, subject_aliases: &[&str]) -> bool {
    const RELATIVES: &[&str] = &[
        "daughter",
        "son",
        "wife",
        "husband",
        "father",
        "mother",
        "sister",
        "brother",
        "child",
        "infant",
        "fiancée",
        "fiancee",
    ];
    const DEATH_CUES: &[&str] = &[
        " died",
        "drowned",
        "was buried",
        "funeral",
        "'s death",
        "death of",
    ];
    if subject_died_explicitly(sentence_lc, subject_aliases) {
        return false;
    }
    let has_relative = RELATIVES.iter().any(|rel| contains_word(sentence_lc, rel));
    let has_death = DEATH_CUES.iter().any(|cue| sentence_lc.contains(cue));
    if has_relative && has_death {
        return true;
    }
    if sentence_lc.contains("'s death") {
        return true;
    }
    false
}

fn subject_died_explicitly(sentence_lc: &str, subject_aliases: &[&str]) -> bool {
    subject_aliases.iter().any(|alias| {
        sentence_lc.contains(&format!("{alias} died"))
            || sentence_lc.contains(&format!("{alias} drowned"))
            || sentence_lc.contains(&format!("death of {alias}"))
            || sentence_lc.contains(&format!("{alias}'s death"))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mines_napoleon_anecdote_not_statue() {
        let hits = mine_sentence(
            "According to legend, Napoleon slept only four hours a night while campaigning near Austerlitz in 1805.",
            "Napoleon",
        );
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].extractor, EXTRACTOR_ANECDOTE);
        assert_eq!(hits[0].person, "Napoleon");
        assert_eq!(hits[0].time, "1805");
        assert!(hits[0].place.to_lowercase().contains("austerlitz"));
    }

    #[test]
    fn skips_commemorative_keywords() {
        let hits = mine_sentence(
            "A statue of Napoleon was unveiled in 1865 in Paris after his death.",
            "Napoleon",
        );
        assert!(hits.is_empty());
    }

    #[test]
    fn mines_curie_life_event() {
        let hits = mine_sentence(
            "Marie Curie was born in 1867 in Warsaw and later worked with radium.",
            "Marie Curie",
        );
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].extractor, EXTRACTOR_KEYWORDS);
        assert_eq!(hits[0].person, "Marie Curie");
        assert_eq!(hits[0].time, "1867");
        assert_eq!(hits[0].place.to_lowercase(), "warsaw");
    }

    #[test]
    fn mines_study_in_without_studied() {
        let hits = mine_sentence(
            "In 1891, Curie followed her sister to study in Paris.",
            "Marie Curie",
        );
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].time, "1891");
        assert_eq!(hits[0].place.to_lowercase(), "paris");
        assert_eq!(hits[0].verb, "studied");
    }

    #[test]
    fn carries_year_into_next_sentence() {
        let mut carry = MineCarry::default();
        carry.absorb("In 1891 she left Warsaw for France.", "Marie Curie");
        assert_eq!(carry.year.as_deref(), Some("1891"));
        assert_eq!(carry.place.as_deref().map(str::to_lowercase).as_deref(), Some("warsaw"));

        let hits = mine_sentence_with_carry(
            "She enrolled at the Sorbonne in Paris to continue her research.",
            "Marie Curie",
            &carry,
        );
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].time, "1891");
        let place = hits[0].place.to_lowercase();
        assert!(place.contains("paris") || place.contains("sorbonne"));
        assert_eq!(hits[0].verb, "studied");
    }

    #[test]
    fn carry_requires_pronoun_or_alias() {
        let mut carry = MineCarry::default();
        carry.absorb("In 1891 she left Warsaw for France.", "Marie Curie");
        let hits = mine_sentence_with_carry(
            "Wladyslaw Sklodowski taught mathematics at a gymnasium in Warsaw.",
            "Marie Curie",
            &carry,
        );
        assert!(hits.is_empty());
    }

    #[test]
    fn skips_statue_even_with_carry() {
        let mut carry = MineCarry::default();
        carry.absorb("In 1805 Napoleon fought near Austerlitz.", "Napoleon");
        let hits = mine_sentence_with_carry(
            "A statue of Napoleon was unveiled in Paris after his death.",
            "Napoleon",
            &carry,
        );
        assert!(hits.is_empty());
    }

    #[test]
    fn page_title_place_does_not_steal_vinci_from_leonardo() {
        let hits = mine_sentence(
            "In 1503 Leonardo painted a portrait in Florence.",
            "Leonardo da Vinci",
        );
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].place.to_lowercase(), "florence");
    }

    #[test]
    fn mines_balzac_birth() {
        let hits = mine_sentence(
            "Honoré de Balzac was born in 1799 in Tours.",
            "Honoré de Balzac",
        );
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].person, "Honoré de Balzac");
        assert_eq!(hits[0].time, "1799");
        assert_eq!(hits[0].place.to_lowercase(), "tours");
    }

    #[test]
    fn mines_cleopatra_bc_anecdote() {
        let hits = mine_sentence(
            "The story goes that Cleopatra had herself delivered in a carpet to Caesar in Alexandria in 48 BC.",
            "Cleopatra",
        );
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].extractor, EXTRACTOR_ANECDOTE);
        assert_eq!(hits[0].time, "48 BC");
        assert!(hits[0].place.to_lowercase().contains("alexandria"));
    }

    #[test]
    fn does_not_mine_daughters_drowning_as_subject_death() {
        let hits = mine_sentence(
            "On 4 September 1843, she drowned in the Seine at Villequier when the boat she was in overturned. Her young husband died trying to save her.",
            "Victor Hugo",
        );
        assert!(
            hits.iter().all(|hit| hit.verb != "died"),
            "daughter drowning must not use verb died, got {hits:?}"
        );
    }

    #[test]
    fn mines_newspaper_cafe_as_anecdote_not_death() {
        let hits = mine_sentence(
            "Hugo was travelling in 1843 in the south of France when he first learned about Léopoldine's death from a newspaper that he read in a café.",
            "Victor Hugo",
        );
        assert_eq!(hits.len(), 1, "expected one anecdote, got {hits:?}");
        assert_eq!(hits[0].extractor, EXTRACTOR_ANECDOTE);
        assert_ne!(hits[0].verb, "died");
    }

    #[test]
    fn heading_chunks_break_glued_wiki_sections() {
        let text = "Hugo was unable to attend her funeral in Villequier, where their daughter Léopoldine was buried. ==== Children ==== Adèle and Victor Hugo had their first child, Léopold, in 1823, but the boy died in infancy.";
        let chunks = split_heading_chunks(text);
        assert_eq!(chunks.len(), 2);
        assert!(chunks[0].contains("Villequier"));
        assert!(chunks[1].contains("1823"));
        assert!(!chunks[1].contains("Villequier"));
    }

    #[test]
    fn mines_balzac_sardinia_travel() {
        let mut carry = MineCarry::default();
        carry.absorb("As of April 1828 Balzac owed money to his mother in Paris.", "Honoré de Balzac");
        let hits = mine_sentence_with_carry(
            "He traveled to Sardinia in the hopes of reprocessing the slag from the Roman mines there.",
            "Honoré de Balzac",
            &carry,
        );
        assert_eq!(hits.len(), 1, "expected Sardinia travel, got {hits:?}");
        assert_eq!(hits[0].verb, "visited");
        assert!(hits[0].place.to_lowercase().contains("sardinia"));
        assert_ne!(hits[0].verb, "died");
    }

    #[test]
    fn fixture_bios_yield_anecdotes_and_map_cues() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/dumps");
        let mut anecdotes = 0usize;
        let mut keywords = 0usize;
        for entry in std::fs::read_dir(&root).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().and_then(|e| e.to_str()) != Some("txt") {
                continue;
            }
            let title = path.file_stem().unwrap().to_string_lossy().into_owned();
            let text = std::fs::read_to_string(&path).unwrap();
            let mut carry = MineCarry::default();
            for sentence in text.split(['.', '\n']) {
                let sentence = sentence.trim();
                if sentence.len() < 28 {
                    carry.absorb(sentence, &title);
                    continue;
                }
                for hit in mine_sentence_with_carry(sentence, &title, &carry) {
                    if hit.extractor == EXTRACTOR_ANECDOTE {
                        anecdotes += 1;
                    } else {
                        keywords += 1;
                    }
                }
                carry.absorb(sentence, &title);
            }
        }
        assert!(
            anecdotes >= 20,
            "expected dated/placed anecdotes in fixtures, got {anecdotes}"
        );
        assert!(
            keywords >= 20,
            "expected extra life-event keywords in fixtures, got {keywords}"
        );
    }
}
