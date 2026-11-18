// crates/talaria-api/src/person_ingest/collect.rs
//! Wikipedia fetch, follow-queue helpers, and seed titles.

use std::collections::HashSet;

use serde_json::Value;
use talaria_sources::is_followable_map_title;
use talaria_sources::wdqs::WdqsEvent;

/// Wikipedia languages to fetch for the subject article.
/// Uses the requested locale only, plus English fallback when the primary is not English
/// (avoids mixing FR extracts into an EN ingest).
pub fn wiki_langs(requested: &str) -> Vec<String> {
    let primary = requested.trim().to_ascii_lowercase();
    if primary.is_empty() {
        return vec!["en".to_string()];
    }
    let mut langs = vec![primary.clone()];
    if primary != "en" {
        langs.push("en".to_string());
    }
    langs
}

pub fn follow_budget(max_documents: u32) -> u32 {
    max_documents.max(8).min(400)
}

pub fn follow_titles_from_page_links(links: &[String], cap: u32) -> Vec<String> {
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    for title in links {
        if out.len() as u32 >= cap {
            break;
        }
        if !should_pin_follow_title(title) {
            continue;
        }
        let key = title.to_lowercase();
        if seen.insert(key) {
            out.push(title.clone());
        }
    }
    out
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

pub async fn fetch_wiki_extract(
    lang: &str,
    title: &str,
) -> anyhow::Result<(String, String, Vec<String>)> {
    let client = reqwest::Client::builder()
        .user_agent("TalariaEngine/0.1 (person-ingest)")
        .timeout(std::time::Duration::from_secs(45))
        .build()?;
    let url = format!("https://{lang}.wikipedia.org/w/api.php");
    let body: Value = client
        .get(&url)
        .query(&[
            ("action", "query"),
            ("prop", "extracts|links"),
            ("explaintext", "1"),
            ("exlimit", "1"),
            ("plnamespace", "0"),
            ("pllimit", "500"),
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
    let links = page
        .get("links")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|row| row.get("title")?.as_str())
                .map(|s| s.to_string())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    Ok((resolved, extract, links))
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

/// A Wikipedia heading is life, surrounding context, later reputation, or page apparatus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SectionKind {
    Life,
    Context,
    Posterity,
    Apparatus,
}

impl SectionKind {
    /// Only the life of the subject becomes biographical events.
    pub fn admits_events(self) -> bool {
        matches!(self, Self::Life)
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Life => "life",
            Self::Context => "context",
            Self::Posterity => "posterity",
            Self::Apparatus => "apparatus",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WikiSection {
    pub heading: String,
    pub anchor: String,
    pub index: i32,
    pub plain: String,
    pub kind: SectionKind,
}

#[derive(Debug, Clone)]
pub struct WikiArticle {
    pub title: String,
    pub revision: Option<i64>,
    pub sections: Vec<WikiSection>,
}

#[derive(Debug, Clone)]
pub struct TocHeading {
    pub line: String,
    pub anchor: String,
    pub byte_offset: usize,
    pub index: i32,
}

/// Classify a heading without knowing who the article is about.
pub fn section_kind(heading: &str) -> SectionKind {
    let folded = fold_name(heading);
    const APPARATUS: &[&str] = &[
        "reference",
        "references",
        "notes",
        "note",
        "bibliography",
        "bibliographie",
        "see also",
        "voir aussi",
        "external links",
        "external link",
        "liens externes",
        "further reading",
        "sources",
        "notes et references",
        "articles connexes",
        "works cited",
        "citations",
        "footnotes",
        "liens internes",
    ];
    const POSTERITY: &[&str] = &[
        "popular culture",
        "in popular culture",
        "culture populaire",
        "dans la culture populaire",
        "cultural depictions",
        "in fiction",
        "legacy",
        "posterite",
        "memoire",
        "historiography",
        "historiographie",
        "depictions",
        "filmographie",
        "hommages",
    ];
    const CONTEXT: &[&str] = &[
        "background",
        "historical background",
        "contexte",
        "contexte historique",
        "context",
    ];
    if heading_is(&folded, APPARATUS) {
        SectionKind::Apparatus
    } else if heading_is(&folded, POSTERITY) {
        SectionKind::Posterity
    } else if heading_is(&folded, CONTEXT) {
        SectionKind::Context
    } else {
        SectionKind::Life
    }
}

fn heading_is(folded: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| {
        folded == *needle
            || folded.starts_with(&format!("{needle} "))
            || folded.starts_with(&format!("{needle}:"))
            || folded.ends_with(&format!(" {needle}"))
    })
}

/// Split one article at the parser's byte offsets. The lead is the text before the first heading.
pub fn article_sections(wikitext: &str, toc: &[TocHeading]) -> Vec<WikiSection> {
    let mut bounds: Vec<(usize, Option<&TocHeading>)> = vec![(0, None)];
    for heading in toc {
        let offset = floor_char_boundary(wikitext, heading.byte_offset);
        if bounds.last().is_none_or(|last| last.0 != offset) {
            bounds.push((offset, Some(heading)));
        }
    }
    bounds.sort_by_key(|item| item.0);
    let mut sections = Vec::new();
    for window in bounds.windows(2) {
        let start = window[0].0;
        let end = window[1].0;
        push_section(&mut sections, wikitext, start, end, window[0].1);
    }
    if let Some((start, heading)) = bounds.last() {
        push_section(&mut sections, wikitext, *start, wikitext.len(), *heading);
    }
    sections
}

fn push_section(
    sections: &mut Vec<WikiSection>,
    wikitext: &str,
    start: usize,
    end: usize,
    heading: Option<&TocHeading>,
) {
    let Some(slice) = wikitext.get(start..end.min(wikitext.len())) else {
        return;
    };
    let plain = plain_from_wikitext(slice);
    if plain.chars().count() < 24 {
        return;
    }
    let (heading, anchor, index) = match heading {
        Some(item) => (strip_tags(&item.line), item.anchor.clone(), item.index),
        None => ("Introduction".to_string(), "lead".to_string(), 0),
    };
    if heading.is_empty() {
        return;
    }
    let kind = if anchor == "lead" {
        SectionKind::Life
    } else {
        section_kind(&heading)
    };
    sections.push(WikiSection {
        heading,
        anchor,
        index,
        plain,
        kind,
    });
}

fn floor_char_boundary(text: &str, offset: usize) -> usize {
    if offset >= text.len() {
        return text.len();
    }
    if text.is_char_boundary(offset) {
        offset
    } else {
        text.char_indices()
            .map(|(index, _)| index)
            .take_while(|index| *index <= offset)
            .last()
            .unwrap_or(0)
    }
}

/// Drop templates, notes, and file links. Keep the words a reader would see.
pub fn plain_from_wikitext(input: &str) -> String {
    let without_comments = strip_delimited(input, "<!--", "-->");
    let without_refs = strip_refs(&without_comments);
    let without_templates = strip_delimited(&without_refs, "{{", "}}");
    let linked = replace_wiki_links(&without_templates);
    let unquoted = linked
        .replace("'''''", "")
        .replace("'''", "")
        .replace("''", "");
    let mut lines = Vec::new();
    for line in unquoted.lines() {
        let cleaned = strip_tags(line).trim_matches('=').trim().to_string();
        if cleaned.chars().count() < 2 {
            continue;
        }
        lines.push(cleaned);
    }
    lines.join("\n")
}

fn strip_refs(input: &str) -> String {
    let lower = input.to_ascii_lowercase();
    let mut out = String::new();
    let mut rest = input;
    let mut lower_rest = lower.as_str();
    while let Some(start) = lower_rest.find("<ref") {
        out.push_str(&rest[..start]);
        let after = &rest[start..];
        let after_lower = &lower_rest[start..];
        let Some(end_rel) = after_lower.find('>') else {
            break;
        };
        let self_closing = after_lower[..end_rel].ends_with('/');
        if self_closing {
            rest = &after[end_rel + 1..];
            lower_rest = &after_lower[end_rel + 1..];
            continue;
        }
        if let Some(close) = after_lower[end_rel..].find("</ref>") {
            let next = end_rel + close + "</ref>".len();
            rest = &after[next..];
            lower_rest = &after_lower[next..];
            continue;
        }
        rest = &after[end_rel + 1..];
        lower_rest = &after_lower[end_rel + 1..];
    }
    out.push_str(rest);
    out
}

fn strip_delimited(input: &str, open: &str, close: &str) -> String {
    let mut out = String::new();
    let mut rest = input;
    while let Some(start) = rest.find(open) {
        out.push_str(&rest[..start]);
        let after = &rest[start + open.len()..];
        match nested_close(after, open, close) {
            Some(end_rel) => rest = &after[end_rel + close.len()..],
            None => {
                rest = after;
                break;
            }
        }
    }
    out.push_str(rest);
    out
}

fn nested_close(input: &str, open: &str, close: &str) -> Option<usize> {
    let mut depth = 1;
    let mut index = 0;
    while index < input.len() {
        if input[index..].starts_with(open) {
            depth += 1;
            index += open.len();
            continue;
        }
        if input[index..].starts_with(close) {
            depth -= 1;
            if depth == 0 {
                return Some(index);
            }
            index += close.len();
            continue;
        }
        index += input[index..].chars().next()?.len_utf8();
    }
    None
}

fn replace_wiki_links(input: &str) -> String {
    let mut out = String::new();
    let mut rest = input;
    while let Some(start) = rest.find("[[") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        let Some(end) = after.find("]]") else {
            out.push_str(rest);
            return out;
        };
        out.push_str(&link_label(&after[..end]));
        rest = &after[end + 2..];
    }
    out.push_str(rest);
    out
}

fn link_label(inner: &str) -> String {
    let lower = inner.to_ascii_lowercase();
    if lower.starts_with("file:")
        || lower.starts_with("image:")
        || lower.starts_with("fichier:")
        || lower.starts_with("media:")
    {
        return String::new();
    }
    inner
        .split('|')
        .next_back()
        .unwrap_or(inner)
        .split('#')
        .next()
        .unwrap_or(inner)
        .trim()
        .to_string()
}

fn strip_tags(input: &str) -> String {
    let mut out = String::new();
    let mut in_tag = false;
    for ch in input.chars() {
        if ch == '<' {
            in_tag = true;
            continue;
        }
        if ch == '>' {
            in_tag = false;
            continue;
        }
        if !in_tag {
            out.push(ch);
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The article, its current revision, and one plain-text block per heading.
pub async fn fetch_wiki_article(lang: &str, title: &str) -> anyhow::Result<WikiArticle> {
    let client = reqwest::Client::builder()
        .user_agent("TalariaEngine/0.1 (encyclopedia)")
        .timeout(std::time::Duration::from_secs(45))
        .build()?;
    let url = format!("https://{lang}.wikipedia.org/w/api.php");
    let body: Value = client
        .get(&url)
        .query(&[
            ("action", "parse"),
            ("page", title),
            ("prop", "wikitext|sections|revid"),
            ("redirects", "1"),
            ("format", "json"),
            ("formatversion", "2"),
        ])
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    if let Some(code) = body.pointer("/error/code").and_then(|value| value.as_str()) {
        anyhow::bail!("wikipedia parse failed: {code}");
    }
    let resolved = body
        .pointer("/parse/title")
        .and_then(|value| value.as_str())
        .unwrap_or(title)
        .to_string();
    let revision = body
        .pointer("/parse/revid")
        .and_then(|value| value.as_i64());
    let wikitext = body
        .pointer("/parse/wikitext")
        .and_then(|value| {
            value
                .as_str()
                .or_else(|| value.get("*").and_then(|item| item.as_str()))
        })
        .unwrap_or("");
    if wikitext.trim().is_empty() {
        anyhow::bail!("wikipedia wikitext empty for {title}");
    }
    let toc = body
        .pointer("/parse/sections")
        .and_then(|value| value.as_array())
        .map(|rows| {
            rows.iter()
                .filter_map(|row| {
                    let line = row.get("line")?.as_str()?.to_string();
                    let anchor = row
                        .get("anchor")
                        .and_then(|value| value.as_str())
                        .unwrap_or(&line)
                        .to_string();
                    let byte_offset = row
                        .get("byteoffset")
                        .and_then(|value| value.as_u64())
                        .unwrap_or(0) as usize;
                    let index = row
                        .get("index")
                        .and_then(|value| value.as_i64())
                        .or_else(|| {
                            row.get("index")
                                .and_then(|value| value.as_str())
                                .and_then(|text| text.parse::<i64>().ok())
                        })
                        .unwrap_or(0) as i32;
                    Some(TocHeading {
                        line,
                        anchor,
                        byte_offset,
                        index,
                    })
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    Ok(WikiArticle {
        title: resolved,
        revision,
        sections: article_sections(wikitext, &toc),
    })
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
    fn page_links_keep_battles_and_drop_lists() {
        let titles = follow_titles_from_page_links(
            &[
                "Battle of the Dunes".into(),
                "List of battles of Louis XIV".into(),
                "Treaty of Nijmegen".into(),
                "Victor Hugo".into(),
            ],
            40,
        );
        assert!(titles.contains(&"Battle of the Dunes".into()));
        assert!(titles.contains(&"Treaty of Nijmegen".into()));
        assert!(!titles.iter().any(|t| t.starts_with("List of")));
        assert!(!titles.iter().any(|t| t == "Victor Hugo"));
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
    fn wiki_langs_respects_primary_without_forced_french() {
        assert_eq!(wiki_langs("en"), vec!["en".to_string()]);
        assert_eq!(wiki_langs("fr"), vec!["fr".to_string(), "en".to_string()]);
    }

    #[test]
    fn napoleon_seed_list_has_dozens_of_pin_titles() {
        let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/seeds/napoleon_wiki_titles.txt");
        let titles = talaria_sources::load_seed_titles(&path).expect("napoleon seed list");
        let pins = titles.iter().filter(|t| should_pin_follow_title(t)).count();
        assert!(
            pins >= 80,
            "expected a dense Napoleon battle/treaty seed list, got {pins}"
        );
    }

    #[test]
    fn an_article_keeps_the_life_and_drops_the_apparatus() {
        let wikitext = "Born in 1769 in Ajaccio, he left the island as a child.\n\
== Early life ==\n\
He studied at Brienne from 1779.\n\
== References ==\n\
The works cited in this article are listed apart from the life itself.\n\
== In popular culture ==\n\
Films have returned to this life many times since the nineteenth century.";
        let toc = vec![
            TocHeading {
                line: "Early life".into(),
                anchor: "Early_life".into(),
                byte_offset: wikitext.find("== Early life ==").unwrap(),
                index: 1,
            },
            TocHeading {
                line: "References".into(),
                anchor: "References".into(),
                byte_offset: wikitext.find("== References ==").unwrap(),
                index: 2,
            },
            TocHeading {
                line: "In popular culture".into(),
                anchor: "In_popular_culture".into(),
                byte_offset: wikitext.find("== In popular culture ==").unwrap(),
                index: 3,
            },
        ];
        let sections = article_sections(wikitext, &toc);
        assert_eq!(sections[0].anchor, "lead");
        assert!(sections[0].plain.contains("1769"));
        assert!(sections[0].kind.admits_events());
        let early = sections
            .iter()
            .find(|section| section.anchor == "Early_life")
            .unwrap();
        assert!(early.plain.contains("Brienne"));
        assert!(early.kind.admits_events());
        assert!(sections
            .iter()
            .any(|section| section.kind == SectionKind::Apparatus));
        assert!(sections
            .iter()
            .any(|section| section.kind == SectionKind::Posterity));
        assert!(!sections
            .iter()
            .any(|section| section.kind != SectionKind::Life && section.kind.admits_events()));
    }

    #[test]
    fn wikitext_keeps_the_visible_words() {
        let plain = plain_from_wikitext(
            "He won at [[Battle of Lodi|Lodi]] in 1796.<ref>A note.</ref> {{sfn|Roberts|2014}} [[File:Map.jpg|thumb]]",
        );
        assert!(plain.contains("Lodi"));
        assert!(plain.contains("1796"));
        assert!(!plain.contains("Roberts"));
        assert!(!plain.contains("File:"));
        assert!(!plain.contains("A note"));
    }
}
