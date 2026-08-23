// crates/talaria-text/src/infobox.rs
//! Parse person infobox templates and wikilinks from MediaWiki source.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InfoboxField {
    pub key: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WikiLink {
    pub target: String,
    pub display: String,
}

/// First Infobox / infobox template body as `key → raw value` pairs.
pub fn parse_infobox_fields(wikitext: &str) -> Vec<InfoboxField> {
    let Some(body) = first_infobox_body(wikitext) else {
        return vec![];
    };
    parse_template_fields(&body)
}

/// `[[Target]]` / `[[Target|label]]` excluding files, images, categories.
pub fn extract_wikilinks(wikitext: &str) -> Vec<WikiLink> {
    let mut out = Vec::new();
    let mut rest = wikitext;
    while let Some(start) = rest.find("[[") {
        let after = &rest[start + 2..];
        let Some(end) = after.find("]]") else {
            break;
        };
        let inner = after[..end].trim();
        rest = &after[end + 2..];
        if inner.is_empty() {
            continue;
        }
        let lower = inner.to_lowercase();
        if lower.starts_with("file:")
            || lower.starts_with("image:")
            || lower.starts_with("category:")
            || lower.starts_with("catégorie:")
            || lower.starts_with("categorie:")
        {
            continue;
        }
        let mut parts = inner.splitn(2, '|');
        let target = parts.next().unwrap_or(inner).trim();
        let display = parts.next().unwrap_or(target).trim();
        if target.is_empty() {
            continue;
        }
        out.push(WikiLink {
            target: target.to_string(),
            display: display.to_string(),
        });
    }
    out
}

/// Birth / death / residence surfaces from a person infobox (language-agnostic keys).
pub fn infobox_life_facts(wikitext: &str) -> InfoboxLifeFacts {
    let fields = parse_infobox_fields(wikitext);
    let mut facts = InfoboxLifeFacts::default();
    for field in &fields {
        let key = normalize_infobox_key(&field.key);
        let value = field.value.as_str();
        if is_birth_place_key(&key) {
            facts.birth_place = first_place_in_value(value);
        } else if is_death_place_key(&key) {
            facts.death_place = first_place_in_value(value);
        } else if is_residence_key(&key) {
            facts.residences.extend(places_in_value(value));
        } else if is_birth_date_key(&key) {
            facts.birth_year = year_in_value(value);
            if facts.birth_place.is_none() {
                facts.birth_place = first_place_in_value(value);
            }
        } else if is_death_date_key(&key) {
            facts.death_year = year_in_value(value);
            if facts.death_place.is_none() {
                facts.death_place = first_place_in_value(value);
            }
        }
    }
    facts.residences.sort();
    facts.residences.dedup();
    facts
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InfoboxLifeFacts {
    pub birth_place: Option<String>,
    pub death_place: Option<String>,
    pub birth_year: Option<String>,
    pub death_year: Option<String>,
    pub residences: Vec<String>,
}

fn first_infobox_body(wikitext: &str) -> Option<String> {
    let lower = wikitext.to_lowercase();
    let start = lower.find("{{infobox")?;
    let end = find_template_end(wikitext, start)?;
    let inner = &wikitext[start + 2..end.saturating_sub(2)];
    Some(inner.to_string())
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

fn parse_template_fields(body: &str) -> Vec<InfoboxField> {
    let mut fields = Vec::new();
    let mut current = String::new();
    let mut depth_brace = 0i32;
    let mut depth_link = 0i32;
    for ch in body.chars() {
        match ch {
            '{' => {
                depth_brace += 1;
                current.push(ch);
            }
            '}' => {
                depth_brace = (depth_brace - 1).max(0);
                current.push(ch);
            }
            '[' => {
                depth_link += 1;
                current.push(ch);
            }
            ']' => {
                depth_link = (depth_link - 1).max(0);
                current.push(ch);
            }
            '|' if depth_brace == 0 && depth_link == 0 => {
                push_field(&mut fields, &current);
                current.clear();
            }
            _ => current.push(ch),
        }
    }
    push_field(&mut fields, &current);
    fields
}

fn push_field(fields: &mut Vec<InfoboxField>, raw: &str) {
    let raw = raw.trim();
    if raw.is_empty() || !raw.contains('=') {
        return;
    }
    let mut parts = raw.splitn(2, '=');
    let key = parts.next().unwrap_or("").trim();
    let value = parts.next().unwrap_or("").trim();
    if key.is_empty() || key.chars().any(|c| c.is_whitespace() && key.len() > 40) {
        return;
    }
    if key.to_lowercase().starts_with("infobox") {
        return;
    }
    fields.push(InfoboxField {
        key: key.to_string(),
        value: value.to_string(),
    });
}

fn normalize_infobox_key(key: &str) -> String {
    key.to_lowercase()
        .replace('_', " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn is_birth_place_key(key: &str) -> bool {
    matches!(
        key,
        "birth place" | "birthplace" | "lieu de naissance" | "lieu naissance" | "place of birth"
    ) || key.contains("lieu de naissance")
}

fn is_death_place_key(key: &str) -> bool {
    matches!(
        key,
        "death place"
            | "deathplace"
            | "lieu de décès"
            | "lieu de deces"
            | "lieu décès"
            | "lieu deces"
            | "place of death"
    ) || key.contains("lieu de décès")
        || key.contains("lieu de deces")
}

fn is_residence_key(key: &str) -> bool {
    matches!(
        key,
        "residence" | "residences" | "résidence" | "résidences" | "resting place" | "lieu de repos"
    ) || key.contains("résidence")
        || key.contains("residence")
}

fn is_birth_date_key(key: &str) -> bool {
    matches!(key, "birth date" | "birthdate" | "date de naissance" | "naissance")
}

fn is_death_date_key(key: &str) -> bool {
    matches!(
        key,
        "death date" | "deathdate" | "date de décès" | "date de deces" | "décès" | "deces"
    )
}

fn year_in_value(value: &str) -> Option<String> {
    // {{birth date|YYYY|M|D}} / {{Date de naissance|D|month|YYYY}}
    let lower = value.to_lowercase();
    if lower.contains("birth date") || lower.contains("death date") {
        for part in value.split('|') {
            let p = part.trim().trim_end_matches('}');
            if p.len() == 4 && p.chars().all(|c| c.is_ascii_digit()) {
                return Some(p.to_string());
            }
        }
    }
    for w in value.split(|c: char| !c.is_ascii_digit()) {
        if w.len() == 4 {
            if let Ok(y) = w.parse::<i32>() {
                if (1000..=2100).contains(&y) {
                    return Some(y.to_string());
                }
            }
        }
    }
    None
}

fn places_in_value(value: &str) -> Vec<String> {
    let links = extract_wikilinks(value);
    if !links.is_empty() {
        return links
            .into_iter()
            .map(|l| l.display)
            .filter(|s| s.chars().count() >= 2)
            .collect();
    }
    first_place_in_value(value).into_iter().collect()
}

fn first_place_in_value(value: &str) -> Option<String> {
    let links = extract_wikilinks(value);
    if let Some(link) = links.into_iter().next() {
        return Some(link.display);
    }
    let lower = value.to_lowercase();
    for cue in [" à ", "|à ", " at ", " in "] {
        if let Some(pos) = lower.find(cue) {
            let after = value[pos + cue.len()..].trim();
            let token = after
                .split(|c: char| c == '|' || c == '}' || c == ',' || c == '<')
                .next()
                .unwrap_or(after)
                .trim()
                .trim_matches(|c: char| !c.is_alphabetic() && c != ' ' && c != '-' && c != '\'')
                .to_string();
            if token.len() >= 2 {
                return Some(token);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    const FR_INFOBOX: &str = r#"
{{Infobox Biographie2
| nom               = Marie Curie
| naissance         = {{Date de naissance|7|novembre|1867|à Varsovie}}
| lieu de naissance = [[Varsovie]]
| décès             = {{Date de décès|4|juillet|1934|à Passy}}
| lieu de décès     = [[Passy (Haute-Savoie)|Passy]]
| résidence         = [[Paris]]
}}
Lead [[Sorbonne]] text.
"#;

    const EN_INFOBOX: &str = r#"
{{Infobox scientist
| name        = Ada Lovelace
| birth_date  = {{birth date|1815|12|10}}
| birth_place = [[London]]
| death_date  = {{death date and age|1852|11|27|1815|12|10}}
| death_place = [[Marylebone]], London
}}
"#;

    #[test]
    fn french_infobox_birth_death_and_residence() {
        let facts = infobox_life_facts(FR_INFOBOX);
        assert_eq!(facts.birth_place.as_deref(), Some("Varsovie"));
        assert_eq!(facts.death_place.as_deref(), Some("Passy"));
        assert_eq!(facts.birth_year.as_deref(), Some("1867"));
        assert_eq!(facts.death_year.as_deref(), Some("1934"));
        assert!(facts.residences.iter().any(|r| r == "Paris"));
    }

    #[test]
    fn english_infobox_birth_place_and_year() {
        let facts = infobox_life_facts(EN_INFOBOX);
        assert_eq!(facts.birth_place.as_deref(), Some("London"));
        assert_eq!(facts.death_place.as_deref(), Some("Marylebone"));
        assert_eq!(facts.birth_year.as_deref(), Some("1815"));
        assert_eq!(facts.death_year.as_deref(), Some("1852"));
    }

    #[test]
    fn wikilinks_skip_files_and_keep_place_targets() {
        let links = extract_wikilinks(
            "See [[Venise|Venice]] and [[File:Map.png]] then [[hôtel Danieli]].",
        );
        let targets: Vec<_> = links.iter().map(|l| l.target.as_str()).collect();
        assert!(targets.contains(&"Venise"));
        assert!(targets.contains(&"hôtel Danieli"));
        assert!(!targets.iter().any(|t| t.to_lowercase().starts_with("file:")));
        assert_eq!(
            links.iter().find(|l| l.target == "Venise").unwrap().display,
            "Venice"
        );
    }
}
