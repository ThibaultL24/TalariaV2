// crates/talaria-sources/src/connectors/wikisource.rs
use std::collections::HashMap;

use serde_json::Value;

/// Wikisource FR connector shell — discover/fetch wired in Task 2.
pub struct WikisourceConnector {
    pub http: reqwest::Client,
    pub max_docs: u32,
}

/// Map MediaWiki siteinfo namespace canonical `"*"` names to ids; skip empty main ns.
pub fn parse_siteinfo_namespaces(json: &Value) -> HashMap<String, i64> {
    let mut map = HashMap::new();
    let Some(namespaces) = json.pointer("/query/namespaces").and_then(Value::as_object) else {
        return map;
    };
    for ns in namespaces.values() {
        let name = ns.get("*").and_then(Value::as_str).unwrap_or("");
        if name.is_empty() {
            continue;
        }
        if let Some(id) = ns.get("id").and_then(Value::as_i64) {
            map.insert(name.to_string(), id);
        }
    }
    map
}

/// Extract page titles from a MediaWiki search API response.
pub fn parse_search_titles(json: &Value) -> Vec<String> {
    json.pointer("/query/search")
        .and_then(Value::as_array)
        .map(|hits| {
            hits.iter()
                .filter_map(|hit| hit.get("title").and_then(Value::as_str))
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn fold_accents(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            'à' | 'â' | 'ä' | 'á' | 'ã' | 'À' | 'Â' | 'Ä' | 'Á' | 'Ã' => 'a',
            'è' | 'é' | 'ê' | 'ë' | 'È' | 'É' | 'Ê' | 'Ë' => 'e',
            'ì' | 'í' | 'î' | 'ï' | 'Ì' | 'Í' | 'Î' | 'Ï' => 'i',
            'ò' | 'ó' | 'ô' | 'ö' | 'Ò' | 'Ó' | 'Ô' | 'Ö' => 'o',
            'ù' | 'ú' | 'û' | 'ü' | 'Ù' | 'Ú' | 'Û' | 'Ü' => 'u',
            'ç' | 'Ç' => 'c',
            other => other.to_ascii_lowercase(),
        })
        .collect()
}

fn contains_any(haystack: &str, needles: &[&str]) -> bool {
    let folded = fold_accents(haystack);
    needles.iter().any(|needle| folded.contains(needle))
}

/// Classify Wikisource document genre from title/categories (not event extraction).
pub fn classify_genre(title: &str, _wikitext: &str, categories: &[String]) -> &'static str {
    let combined = {
        let cats = categories.join(" ");
        if cats.is_empty() {
            title.to_string()
        } else {
            format!("{title} {cats}")
        }
    };

    if contains_any(&combined, &["lettre", "correspondance"]) {
        "letter"
    } else if contains_any(&combined, &["discours"]) {
        "speech"
    } else if contains_any(&combined, &["traite"]) {
        "treaty"
    } else if contains_any(&combined, &["loi", "code"]) {
        "law"
    } else if contains_any(&combined, &["memoire", "memoires"]) {
        "memoir"
    } else if contains_any(&combined, &["journal"]) {
        "periodical"
    } else {
        "narrative"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn siteinfo_finds_page_ns_without_hardcoded_number() {
        let json = serde_json::json!({"query":{"namespaces":{
            "0": {"id": 0, "*": ""},
            "104": {"id": 104, "*": "Page"},
            "114": {"id": 114, "*": "Livre"}
        }}});
        let map = parse_siteinfo_namespaces(&json);
        assert_eq!(map.get("Page").copied(), Some(104));
        assert_eq!(map.get("Livre").copied(), Some(114));
        assert!(!map.contains_key(""));
    }

    #[test]
    fn parse_search_titles_extracts_from_query_search() {
        let json = serde_json::json!({
            "query": {
                "search": [
                    {"title": "Foo"},
                    {"title": "Bar"}
                ]
            }
        });
        assert_eq!(
            parse_search_titles(&json),
            vec!["Foo".to_string(), "Bar".to_string()]
        );
    }

    #[test]
    fn genre_letter() {
        assert_eq!(classify_genre("Lettre à Joséphine", "", &[]), "letter");
    }

    #[test]
    fn genre_speech() {
        assert_eq!(
            classify_genre("Discours aux états généraux", "", &[]),
            "speech"
        );
    }

    #[test]
    fn genre_treaty_accented() {
        assert_eq!(
            classify_genre("Traité de Campoformio", "", &[]),
            "treaty"
        );
    }

    #[test]
    fn genre_treaty_unaccented() {
        assert_eq!(classify_genre("Traite de Campoformio", "", &[]), "treaty");
    }

    #[test]
    fn genre_law() {
        assert_eq!(classify_genre("Code civil", "", &[]), "law");
        assert_eq!(classify_genre("Loi sur les successions", "", &[]), "law");
    }

    #[test]
    fn genre_memoir_accented() {
        assert_eq!(
            classify_genre("Mémoires sur la Révolution", "", &[]),
            "memoir"
        );
    }

    #[test]
    fn genre_memoir_unaccented() {
        assert_eq!(
            classify_genre("Memoires sur la Revolution", "", &[]),
            "memoir"
        );
    }

    #[test]
    fn genre_periodical() {
        assert_eq!(
            classify_genre("Journal des débats", "", &[]),
            "periodical"
        );
    }

    #[test]
    fn genre_narrative_default() {
        assert_eq!(
            classify_genre("Histoire de France", "", &[]),
            "narrative"
        );
    }

    #[test]
    fn genre_from_category() {
        assert_eq!(
            classify_genre(
                "Correspondance secrète",
                "",
                &["Lettres de Napoléon".into()]
            ),
            "letter"
        );
    }
}
