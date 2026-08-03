// crates/talaria-text/src/wikitext.rs
pub fn wikitext_to_plain(input: &str) -> String {
    let mut text = input.to_string();
    text = strip_html_comments(&text);
    text = strip_block_tags(&text, "ref");
    text = strip_self_closing_tags(&text, "ref");
    text = remove_templates(&text);
    text = simplify_wiki_links(&text);
    text = strip_remaining_html_tags(&text);
    text = strip_category_lines(&text);
    text = text.replace("'''", "").replace("''", "");
    collapse_whitespace(&text)
}

fn strip_html_comments(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut rest = input;
    while let Some(start) = rest.find("<!--") {
        out.push_str(&rest[..start]);
        if let Some(end) = rest[start..].find("-->") {
            rest = &rest[start + end + 3..];
        } else {
            break;
        }
    }
    out.push_str(rest);
    out
}

fn strip_block_tags(input: &str, tag: &str) -> String {
    let open = format!("<{tag}");
    let close = format!("</{tag}>");
    let mut text = input.to_string();
    loop {
        let Some(start) = text.to_lowercase().find(&open) else {
            break;
        };
        let Some(rel_end) = text.to_lowercase()[start..].find(&close) else {
            text.replace_range(start.., " ");
            break;
        };
        let end = start + rel_end + close.len();
        text.replace_range(start..end, " ");
    }
    text
}

fn strip_self_closing_tags(input: &str, tag: &str) -> String {
    let open = format!("<{tag}");
    let mut text = input.to_string();
    loop {
        let lower = text.to_lowercase();
        let Some(start) = lower.find(&open) else {
            break;
        };
        let Some(rel_end) = text[start..].find('>') else {
            break;
        };
        let end = start + rel_end + 1;
        text.replace_range(start..end, " ");
    }
    text
}

fn remove_templates(input: &str) -> String {
    let mut text = input.to_string();
    loop {
        let Some(start) = text.find("{{") else {
            break;
        };
        let Some(end) = find_template_end(&text, start) else {
            text.replace_range(start..start + 2, " ");
            break;
        };
        text.replace_range(start..end, " ");
    }
    text
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

fn simplify_wiki_links(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut rest = input;
    while let Some(start) = rest.find("[[") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        let Some(end) = after.find("]]") else {
            out.push_str(rest);
            return out;
        };
        let inner = &after[..end];
        let target = inner.split('|').next_back().unwrap_or(inner).trim();
        let lower = target.to_lowercase();
        if !lower.starts_with("file:")
            && !lower.starts_with("image:")
            && !lower.starts_with("category:")
        {
            out.push_str(target);
            out.push(' ');
        }
        rest = &after[end + 2..];
    }
    out.push_str(rest);
    out
}

fn strip_remaining_html_tags(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut in_tag = false;
    for ch in input.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(ch),
            _ => {}
        }
    }
    out
}

fn strip_category_lines(input: &str) -> String {
    input
        .lines()
        .filter(|line| {
            let trimmed = line.trim();
            !trimmed.starts_with("[[Category:")
                && !trimmed.starts_with("[[category:")
                && !trimmed.is_empty()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn collapse_whitespace(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut prev_space = false;
    for ch in input.chars() {
        if ch.is_whitespace() {
            if !prev_space {
                out.push(' ');
                prev_space = true;
            }
        } else {
            out.push(ch);
            prev_space = false;
        }
    }
    out.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_templates_and_links() {
        let raw = "'''Ada Lovelace''' ({{birth date|1815|12|10}}) was a mathematician. See [[Analytical Engine|her engine]].";
        let plain = wikitext_to_plain(raw);
        assert!(plain.contains("Ada Lovelace"));
        assert!(plain.contains("mathematician"));
        assert!(plain.contains("her engine"));
        assert!(!plain.contains("{{"));
        assert!(!plain.contains("[["));
    }
}
