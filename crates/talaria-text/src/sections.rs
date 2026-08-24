// crates/talaria-text/src/sections.rs
//! Split MediaWiki wikitext into == heading == sections (level-2+).

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WikiSectionSpan {
    pub ordinal: i32,
    pub title: String,
    pub wikitext: String,
    pub start_offset: i32,
    pub end_offset: i32,
}

/// Split wikitext on `== Heading ==` (and deeper) markers.
/// Lead (before first heading) is ordinal 0 with title `"Lead"`.
pub fn split_wiki_sections(wikitext: &str) -> Vec<WikiSectionSpan> {
    let mut sections = Vec::new();
    let mut current_title = "Lead".to_string();
    let mut ordinal = 0i32;
    let mut body_start = 0usize;
    let mut byte_pos = 0usize;

    for line in wikitext.lines() {
        let line_end = byte_pos + line.len();
        let nl_len = newline_len(wikitext, line_end);

        if let Some(title) = parse_heading(line) {
            let raw = wikitext.get(body_start..byte_pos).unwrap_or("");
            if !raw.trim().is_empty() || ordinal == 0 {
                push_section(
                    &mut sections,
                    wikitext,
                    ordinal,
                    current_title.clone(),
                    body_start,
                    byte_pos,
                );
                ordinal += 1;
            }
            current_title = title;
            body_start = line_end + nl_len;
            byte_pos = body_start;
            continue;
        }
        byte_pos = line_end + nl_len;
    }

    let raw = wikitext.get(body_start..).unwrap_or("");
    if !raw.trim().is_empty() || sections.is_empty() {
        push_section(
            &mut sections,
            wikitext,
            ordinal,
            current_title,
            body_start,
            wikitext.len(),
        );
    }

    sections
}

fn push_section(
    sections: &mut Vec<WikiSectionSpan>,
    wikitext: &str,
    ordinal: i32,
    title: String,
    body_start: usize,
    body_end: usize,
) {
    let raw = wikitext.get(body_start..body_end).unwrap_or("");
    let body = raw.trim();
    let leading = raw.len() - raw.trim_start().len();
    let start_byte = body_start + leading;
    let end_byte = start_byte + body.len();
    sections.push(WikiSectionSpan {
        ordinal,
        title,
        wikitext: body.to_string(),
        start_offset: char_offset(wikitext, start_byte),
        end_offset: char_offset(wikitext, end_byte),
    });
}

fn newline_len(s: &str, at: usize) -> usize {
    let rest = s.get(at..).unwrap_or("");
    if rest.starts_with("\r\n") {
        2
    } else if rest.starts_with('\n') {
        1
    } else {
        0
    }
}

fn char_offset(s: &str, byte: usize) -> i32 {
    s.get(..byte)
        .map(|prefix| prefix.chars().count() as i32)
        .unwrap_or_else(|| s.chars().count() as i32)
}

fn parse_heading(line: &str) -> Option<String> {
    let trimmed = line.trim();
    if !trimmed.starts_with("==") || !trimmed.ends_with("==") {
        return None;
    }
    // Require at least == x == (not a bare ====)
    let without = trimmed.trim_matches('=').trim();
    if without.is_empty() || without.contains("==") {
        return None;
    }
    // Must have been wrapped with ==
    let left = trimmed.chars().take_while(|c| *c == '=').count();
    let right = trimmed.chars().rev().take_while(|c| *c == '=').count();
    if left < 2 || right < 2 {
        return None;
    }
    Some(without.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_lead_and_naissance() {
        let raw = "Lead paragraph about someone.\n\n== Early life ==\nHe was born in Ajaccio.\n\n== Career ==\nHe became emperor.\n";
        let sections = split_wiki_sections(raw);
        assert_eq!(sections.len(), 3);
        assert_eq!(sections[0].title, "Lead");
        assert!(sections[0].wikitext.contains("Lead paragraph"));
        assert_eq!(sections[1].title, "Early life");
        assert_eq!(sections[2].title, "Career");
    }
}
