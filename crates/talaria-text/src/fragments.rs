// crates/talaria-text/src/fragments.rs
//! Split Wikipedia wikitext into infobox, section, and sentence fragments.

use crate::infobox::{extract_wikilinks, parse_infobox_fields, WikiLink};
use crate::sections::{split_wiki_sections, WikiSectionSpan};
use crate::sentences::split_sentences;
use crate::wikitext::wikitext_to_plain;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FragmentLink {
    pub surface: String,
    pub target_title: String,
    pub qid: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FragmentCitation {
    pub ref_name: Option<String>,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WikiContentFragment {
    pub kind: &'static str,
    pub parent_section_ordinal: Option<i32>,
    pub ordinal: i32,
    pub text: String,
    pub start_offset: i32,
    pub end_offset: i32,
    pub section_path: Vec<String>,
    pub internal_links: Vec<FragmentLink>,
    pub citations: Vec<FragmentCitation>,
}

struct LocatedRef {
    citation: FragmentCitation,
    start_char: i32,
}

pub fn extract_refs(wikitext: &str) -> Vec<FragmentCitation> {
    locate_refs(wikitext)
        .into_iter()
        .map(|r| r.citation)
        .collect()
}

pub fn fragment_wikitext(wikitext: &str) -> Vec<WikiContentFragment> {
    let mut out = Vec::new();
    let refs = locate_refs(wikitext);
    if let Some(infobox) = infobox_fragment(wikitext) {
        out.push(infobox);
    }
    for section in split_wiki_sections(wikitext) {
        out.push(section_fragment(&section, &refs));
        out.extend(sentence_fragments(&section, &refs));
    }
    out
}

fn infobox_fragment(wikitext: &str) -> Option<WikiContentFragment> {
    let start = find_ascii_ci(wikitext, 0, "{{infobox")?;
    let end = find_template_end(wikitext, start)?;
    let slice = &wikitext[start..end];
    let fields = parse_infobox_fields(wikitext);
    let text = fields
        .iter()
        .map(|f| format!("{}={}", f.key, f.value))
        .collect::<Vec<_>>()
        .join("\n");
    Some(WikiContentFragment {
        kind: "infobox",
        parent_section_ordinal: None,
        ordinal: 0,
        text,
        start_offset: char_offset(wikitext, start),
        end_offset: char_offset(wikitext, end),
        section_path: Vec::new(),
        internal_links: extract_wikilinks(slice).into_iter().map(to_fragment_link).collect(),
        citations: Vec::new(),
    })
}

fn section_fragment(section: &WikiSectionSpan, refs: &[LocatedRef]) -> WikiContentFragment {
    WikiContentFragment {
        kind: "section",
        parent_section_ordinal: None,
        ordinal: section.ordinal,
        text: section.wikitext.clone(),
        start_offset: section.start_offset,
        end_offset: section.end_offset,
        section_path: vec![section.title.clone()],
        internal_links: extract_wikilinks(&section.wikitext)
            .into_iter()
            .map(to_fragment_link)
            .collect(),
        citations: refs_in_range(refs, section.start_offset, section.end_offset),
    }
}

fn sentence_fragments(section: &WikiSectionSpan, refs: &[LocatedRef]) -> Vec<WikiContentFragment> {
    let sentences = split_sentences(&wikitext_to_plain(&section.wikitext));
    let links = extract_wikilinks(&section.wikitext);
    let located: Vec<(i32, i32)> = sentences
        .iter()
        .map(|s| {
            locate_plain_in_wiki(&section.wikitext, &s.text)
                .map(|(a, b)| (section.start_offset + a, section.start_offset + b))
                .unwrap_or((section.start_offset, section.end_offset))
        })
        .collect();

    sentences
        .into_iter()
        .enumerate()
        .map(|(i, sent)| {
            let (start_offset, end_offset) = located[i];
            let next_start = located
                .get(i + 1)
                .map(|(s, _)| *s)
                .unwrap_or(section.end_offset);
            let cite_end = next_start.max(end_offset);
            WikiContentFragment {
                kind: "sentence",
                parent_section_ordinal: Some(section.ordinal),
                ordinal: sent.ordinal,
                text: sent.text.clone(),
                start_offset,
                end_offset,
                section_path: vec![section.title.clone()],
                internal_links: links
                    .iter()
                    .filter(|l| sent.text.contains(&l.display) || sent.text.contains(&l.target))
                    .cloned()
                    .map(to_fragment_link)
                    .collect(),
                citations: refs_in_range(refs, start_offset, cite_end),
            }
        })
        .collect()
}

fn to_fragment_link(link: WikiLink) -> FragmentLink {
    FragmentLink {
        surface: link.display,
        target_title: link.target,
        qid: None,
    }
}

fn refs_in_range(refs: &[LocatedRef], start: i32, end: i32) -> Vec<FragmentCitation> {
    refs.iter()
        .filter(|r| r.start_char >= start && r.start_char < end)
        .map(|r| r.citation.clone())
        .collect()
}

fn locate_plain_in_wiki(wiki: &str, plain: &str) -> Option<(i32, i32)> {
    if let Some(b) = wiki.find(plain) {
        return Some((char_offset(wiki, b), char_offset(wiki, b + plain.len())));
    }
    let chars: Vec<char> = plain.chars().collect();
    if chars.len() < 12 {
        return None;
    }
    let max_prefix = chars.len().min(48);
    for len in (12..=max_prefix).rev() {
        let prefix: String = chars[..len].iter().collect();
        let Some(b) = wiki.find(&prefix) else {
            continue;
        };
        let suffix_len = 12.min(chars.len());
        let suffix: String = chars[chars.len() - suffix_len..].iter().collect();
        if let Some(rel) = wiki[b..].find(&suffix) {
            let end_b = b + rel + suffix.len();
            return Some((char_offset(wiki, b), char_offset(wiki, end_b)));
        }
        return Some((char_offset(wiki, b), char_offset(wiki, b + prefix.len())));
    }
    None
}

fn locate_refs(wikitext: &str) -> Vec<LocatedRef> {
    let mut out = Vec::new();
    let bytes = wikitext.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        if is_ref_open_at(wikitext, i) {
            let Some(gt) = wikitext[i + 4..].find('>').map(|rel| i + 4 + rel) else {
                break;
            };
            let attrs = &wikitext[i + 4..gt];
            if attrs.trim_end().ends_with('/') {
                i = gt + 1;
                continue;
            }
            let Some(close) = find_ascii_ci(wikitext, gt + 1, "</ref>") else {
                i = gt + 1;
                continue;
            };
            out.push(LocatedRef {
                citation: FragmentCitation {
                    ref_name: parse_ref_name(attrs),
                    text: wikitext[gt + 1..close].trim().to_string(),
                },
                start_char: char_offset(wikitext, i),
            });
            i = close + "</ref>".len();
            continue;
        }
        i += 1;
        while i < bytes.len() && (bytes[i] & 0b1100_0000) == 0b1000_0000 {
            i += 1;
        }
    }
    out
}

fn is_ref_open_at(hay: &str, i: usize) -> bool {
    let bytes = hay.as_bytes();
    if i + 4 > bytes.len() || !bytes[i..i + 4].eq_ignore_ascii_case(b"<ref") {
        return false;
    }
    matches!(
        bytes.get(i + 4).copied().unwrap_or(b'>'),
        b'>' | b'/' | b' ' | b'\t' | b'\n' | b'\r'
    )
}

fn parse_ref_name(attrs: &str) -> Option<String> {
    let lower = attrs.to_ascii_lowercase();
    let idx = lower.find("name")?;
    let after = attrs[idx + 4..].trim_start();
    if !after.starts_with('=') {
        return None;
    }
    let after = after[1..].trim_start();
    if let Some(rest) = after.strip_prefix('"') {
        return rest.find('"').map(|e| rest[..e].to_string());
    }
    if let Some(rest) = after.strip_prefix('\'') {
        return rest.find('\'').map(|e| rest[..e].to_string());
    }
    let name: String = after
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || matches!(*c, '_' | '-' | ':'))
        .collect();
    if name.is_empty() {
        None
    } else {
        Some(name)
    }
}

fn find_ascii_ci(hay: &str, from: usize, needle: &str) -> Option<usize> {
    let h = hay.as_bytes();
    let n = needle.as_bytes();
    if from > h.len() || n.is_empty() || from + n.len() > h.len() {
        return None;
    }
    h[from..]
        .windows(n.len())
        .position(|w| w.eq_ignore_ascii_case(n))
        .map(|p| from + p)
}

fn find_template_end(text: &str, start: usize) -> Option<usize> {
    let bytes = text.as_bytes();
    let mut i = start + 2;
    let mut depth = 1;
    while i + 1 < bytes.len() {
        if bytes[i] == b'{' && bytes[i + 1] == b'{' {
            depth += 1;
            i += 2;
            continue;
        }
        if bytes[i] == b'}' && bytes[i + 1] == b'}' {
            depth -= 1;
            i += 2;
            if depth == 0 {
                return Some(i);
            }
            continue;
        }
        i += 1;
    }
    None
}

fn char_offset(s: &str, byte: usize) -> i32 {
    s.get(..byte)
        .map(|prefix| prefix.chars().count() as i32)
        .unwrap_or_else(|| s.chars().count() as i32)
}

#[cfg(test)]
mod tests {
    use super::*;

    const NAP: &str = r#"{{Infobox Biographie2
|nom=Napoléon
|lieu naissance=[[Ajaccio]]
}}
Napoléon fut couronné à [[Paris]] le 2 décembre 1804.<ref>Tulard</ref>

== Consulat et Empire ==
Il régna jusqu'en 1814.
"#;

    #[test]
    fn fragments_napoleon_fr_fixture() {
        let frags = fragment_wikitext(NAP);
        assert!(frags.iter().any(|f| f.kind == "infobox"));
        assert!(frags.iter().any(|f| f.kind == "section" && f.section_path == ["Lead"]));
        assert!(frags.iter().any(|f| f.kind == "section" && f.section_path.iter().any(|s| s.contains("Consulat"))));
        let crown = frags.iter().find(|f| f.kind == "sentence" && f.text.contains("1804")).expect("crowning sentence");
        assert!(crown.internal_links.iter().any(|l| l.target_title == "Paris"));
        assert!(crown.citations.iter().any(|c| c.text.contains("Tulard")));
        assert!(frags.iter().filter(|f| f.kind == "sentence").count() >= 2);
    }
}
