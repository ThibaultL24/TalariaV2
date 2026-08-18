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
        page_needles: &["curie", "radium", "polonium"],
        year_min: 1865,
        year_max: 1935,
    },
    Subject {
        wiki_title: "Victor Hugo",
        aliases: &["victor hugo", "hugo"],
        page_needles: &["victor hugo", "les misérables", "les miserables", "notre-dame de paris"],
        year_min: 1800,
        year_max: 1885,
    },
    Subject {
        wiki_title: "Leonardo da Vinci",
        aliases: &["leonardo da vinci", "leonardo"],
        page_needles: &["leonardo", "mona lisa", "last supper", "vitruvian"],
        year_min: 1450,
        year_max: 1520,
    },
    Subject {
        wiki_title: "Christopher Columbus",
        aliases: &["christopher columbus", "columbus", "cristoforo colombo"],
        page_needles: &["columbus", "voyages of christopher"],
        year_min: 1440,
        year_max: 1510,
    },
    Subject {
        wiki_title: "Alan Turing",
        aliases: &["alan turing", "turing"],
        page_needles: &["turing", "bletchley", "enigma"],
        year_min: 1910,
        year_max: 1960,
    },
    Subject {
        wiki_title: "Cleopatra",
        aliases: &["cleopatra vii", "cleopatra"],
        page_needles: &["cleopatra", "ptolemaic", "actium"],
        year_min: -80,
        year_max: 30,
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
    ("fought", "fought"),
    ("battle of", "fought"),
    ("defeated", "fought"),
    ("signed", "signed"),
    ("treaty of", "signed"),
    ("crowned", "crowned"),
    ("exiled", "exiled"),
    ("lived in", "lived"),
    ("resided", "lived"),
    ("moved to", "moved"),
    ("returned to", "moved"),
    ("arrived in", "moved"),
    ("visited", "visited"),
    ("travelled", "visited"),
    ("traveled", "visited"),
    (" met ", "met"),
    ("published", "published"),
    ("wrote ", "published"),
    ("painted", "painted"),
    ("discovered", "discovered"),
    ("awarded", "awarded"),
    ("nobel", "awarded"),
    ("sailed", "visited"),
    ("landed", "visited"),
];

pub fn mine_sentence(text: &str, page_title: &str) -> Vec<MinedCandidate> {
    let cleaned = text.trim();
    if cleaned.len() < 28 {
        return vec![];
    }
    let lower = cleaned.to_ascii_lowercase();
    let Some(subject) = resolve_subject(&lower, page_title) else {
        return vec![];
    };
    let Some(year) = find_year_in_window(&lower, subject.year_min, subject.year_max) else {
        return vec![];
    };
    let Some(place) = find_place_in_text(cleaned).or_else(|| place_from_page_title(page_title))
    else {
        return vec![];
    };

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

    let Some(verb) = find_verb(&lower) else {
        return vec![];
    };
    vec![MinedCandidate {
        person: subject.wiki_title.into(),
        time: year,
        place,
        verb,
        extractor: EXTRACTOR_KEYWORDS,
    }]
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

fn find_year_in_window(lower: &str, min_year: i32, max_year: i32) -> Option<String> {
    let mut last = None;
    for found in extract_years(lower) {
        if (min_year..=max_year).contains(&found.year) {
            last = Some(found.surface);
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
    fn fixture_bios_yield_anecdotes_and_map_cues() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/dumps");
        let mut anecdotes = 0usize;
        let mut keywords = 0usize;
        for entry in std::fs::read_dir(&root).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().and_then(|e| e.to_str()) != Some("txt") {
                continue;
            }
            let title = path.file_stem().unwrap().to_string_lossy().into_owned();
            let text = std::fs::read_to_string(&path).unwrap();
            for sentence in text.split(['.', '\n']) {
                let sentence = sentence.trim();
                if sentence.len() < 28 {
                    continue;
                }
                for hit in mine_sentence(sentence, &title) {
                    if hit.extractor == EXTRACTOR_ANECDOTE {
                        anecdotes += 1;
                    } else {
                        keywords += 1;
                    }
                }
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
