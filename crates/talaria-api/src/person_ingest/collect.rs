// crates/talaria-api/src/person_ingest/collect.rs
//! Wikipedia fetch, follow-queue helpers, and seed titles.

use std::collections::HashSet;

use serde_json::Value;
use talaria_sources::is_followable_map_title;
use talaria_sources::wdqs::WdqsEvent;

pub fn wiki_langs(requested: &str) -> Vec<String> {
    let mut langs = vec![requested.trim().to_ascii_lowercase()];
    for extra in ["en", "fr"] {
        if !langs.iter().any(|l| l == extra) {
            langs.push(extra.to_string());
        }
    }
    langs
}

pub fn follow_budget(max_documents: u32) -> u32 {
    max_documents.max(8).min(400)
}

pub fn follow_titles_from_wdqs(events: &[WdqsEvent], cap: u32) -> Vec<String> {
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    for ev in events {
        if out.len() as u32 >= cap {
            break;
        }
        if !is_followable_map_title(&ev.label) {
            continue;
        }
        let key = ev.label.to_lowercase();
        if seen.insert(key) {
            out.push(ev.label.clone());
        }
    }
    out
}

pub fn fold_name(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            'é' | 'è' | 'ê' | 'ë' | 'É' | 'È' | 'Ê' | 'Ë' => 'e',
            'à' | 'á' | 'â' | 'ä' | 'À' | 'Á' | 'Â' => 'a',
            'î' | 'ï' | 'Î' | 'Ï' => 'i',
            'ô' | 'ö' | 'Ô' => 'o',
            'ù' | 'ú' | 'û' | 'Ù' => 'u',
            'ç' | 'Ç' => 'c',
            other => other.to_ascii_lowercase(),
        })
        .collect()
}

pub fn subject_aliases(subject: &str) -> Vec<String> {
    let mut out = vec![fold_name(subject)];
    for part in subject.split_whitespace() {
        let folded = fold_name(part);
        if folded.len() >= 4 && !out.iter().any(|a| a == &folded) {
            out.push(folded);
        }
    }
    out
}

pub fn subject_mentioned(text: &str, subject: &str) -> bool {
    let hay = fold_name(text);
    subject_aliases(subject).iter().any(|a| hay.contains(a))
}

fn is_overview_title(title: &str) -> bool {
    let l = title.to_lowercase();
    l.starts_with("list of")
        || l.starts_with("liste des")
        || l.starts_with("liste de")
        || l.starts_with("timeline of")
        || l.starts_with("military career")
        || l.starts_with("early life")
        || l.starts_with("scientific career")
        || l.ends_with(" wars")
        || l.ends_with(" war")
}

pub fn should_pin_follow_title(title: &str) -> bool {
    is_followable_map_title(title) && !is_overview_title(title)
}

pub async fn fetch_wiki_extract(lang: &str, title: &str) -> anyhow::Result<(String, String)> {
    let client = reqwest::Client::builder()
        .user_agent("TalariaEngine/0.1 (person-ingest)")
        .timeout(std::time::Duration::from_secs(45))
        .build()?;
    let url = format!("https://{lang}.wikipedia.org/w/api.php");
    let body: Value = client
        .get(&url)
        .query(&[
            ("action", "query"),
            ("prop", "extracts"),
            ("explaintext", "1"),
            ("exlimit", "1"),
            ("titles", title),
            ("format", "json"),
            ("redirects", "1"),
        ])
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    let pages = body
        .pointer("/query/pages")
        .and_then(|v| v.as_object())
        .ok_or_else(|| anyhow::anyhow!("wikipedia extract missing pages"))?;
    let page = pages
        .values()
        .next()
        .ok_or_else(|| anyhow::anyhow!("wikipedia extract empty"))?;
    if page.get("missing").is_some() {
        anyhow::bail!("wikipedia page missing: {title}");
    }
    let resolved = page
        .get("title")
        .and_then(|v| v.as_str())
        .unwrap_or(title)
        .to_string();
    let extract = page
        .get("extract")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    if extract.trim().is_empty() {
        anyhow::bail!("wikipedia extract empty for {title}");
    }
    Ok((resolved, extract))
}

pub async fn fetch_wiki_page(
    lang: &str,
    title: &str,
) -> anyhow::Result<(String, String, Option<(f64, f64)>)> {
    let client = reqwest::Client::builder()
        .user_agent("TalariaEngine/0.1 (person-ingest)")
        .timeout(std::time::Duration::from_secs(45))
        .build()?;
    let url = format!("https://{lang}.wikipedia.org/w/api.php");
    let body: Value = client
        .get(&url)
        .query(&[
            ("action", "query"),
            ("prop", "extracts|coordinates"),
            ("explaintext", "1"),
            ("exlimit", "1"),
            ("colimit", "1"),
            ("titles", title),
            ("format", "json"),
            ("redirects", "1"),
        ])
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    let pages = body
        .pointer("/query/pages")
        .and_then(|v| v.as_object())
        .ok_or_else(|| anyhow::anyhow!("wikipedia page missing pages"))?;
    let page = pages
        .values()
        .next()
        .ok_or_else(|| anyhow::anyhow!("wikipedia page empty"))?;
    if page.get("missing").is_some() {
        anyhow::bail!("wikipedia page missing: {title}");
    }
    let resolved = page
        .get("title")
        .and_then(|v| v.as_str())
        .unwrap_or(title)
        .to_string();
    let extract = page
        .get("extract")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    if extract.trim().is_empty() {
        anyhow::bail!("wikipedia extract empty for {title}");
    }
    let coords = page
        .get("coordinates")
        .and_then(|v| v.as_array())
        .and_then(|a| a.first())
        .and_then(|c| Some((c.get("lat")?.as_f64()?, c.get("lon")?.as_f64()?)));
    Ok((resolved, extract, coords))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn follow_budget_uses_requested_document_cap() {
        assert_eq!(follow_budget(400), 400);
        assert_eq!(follow_budget(0), 8);
        assert_eq!(follow_budget(9_000), 400);
        assert!(follow_budget(400) > 80);
    }

    #[test]
    fn battle_pages_are_pinned_lists_are_not() {
        assert!(should_pin_follow_title("Battle of Waterloo"));
        assert!(should_pin_follow_title("Bataille d'Austerlitz"));
        assert!(should_pin_follow_title("Treaty of Tilsit"));
        assert!(!should_pin_follow_title(
            "List of battles of the Napoleonic Wars"
        ));
        assert!(!should_pin_follow_title("Napoleonic Wars"));
        assert!(!should_pin_follow_title("Military career of Napoleon"));
    }

    #[test]
    fn napoleon_seed_list_has_dozens_of_pin_titles() {
        let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/seeds/napoleon_wiki_titles.txt");
        let titles = talaria_sources::load_seed_titles(&path).expect("napoleon seed list");
        let pins = titles
            .iter()
            .filter(|t| should_pin_follow_title(t))
            .count();
        assert!(
            pins >= 80,
            "expected a dense Napoleon battle/treaty seed list, got {pins}"
        );
    }
}
