// crates/talaria-text/src/sections.rs
//! Split MediaWiki wikitext into == heading == sections (level-2+).

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WikiSectionSpan {
    pub ordinal: i32,
    pub title: String,
    pub wikitext: String,
}

/// Split wikitext on `== Heading ==` (and deeper) markers.
/// Lead (before first heading) is ordinal 0 with title `"Lead"`.
pub fn split_wiki_sections(wikitext: &str) -> Vec<WikiSectionSpan> {
    let mut sections = Vec::new();
    let mut current_title = "Lead".to_string();
    let mut current_body = String::new();
    let mut ordinal = 0i32;

    for line in wikitext.lines() {
        if let Some(title) = parse_heading(line) {
            let body = current_body.trim();
            if !body.is_empty() || ordinal == 0 {
                sections.push(WikiSectionSpan {
                    ordinal,
                    title: current_title.clone(),
                    wikitext: body.to_string(),
                });
                ordinal += 1;
            }
            current_title = title;
            current_body.clear();
            continue;
        }
        current_body.push_str(line);
        current_body.push('\n');
    }

    let body = current_body.trim();
    if !body.is_empty() || sections.is_empty() {
        sections.push(WikiSectionSpan {
            ordinal,
            title: current_title,
            wikitext: body.to_string(),
        });
    }

    sections
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
