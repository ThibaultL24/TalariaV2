// crates/talaria-quality/src/places.rs

pub struct PlaceQuery {
    pub surface: String,
    pub search_keys: Vec<String>,
}

pub fn place_query(surface: &str) -> PlaceQuery {
    let trimmed = surface.trim();
    let mut search_keys = vec![trimmed.to_string()];

    if let Some(remainder) = trimmed.strip_prefix("The ") {
        if !remainder.is_empty() {
            search_keys.push(remainder.to_string());
        }
    } else if let Some(remainder) = trimmed.strip_prefix("the ") {
        if !remainder.is_empty() {
            search_keys.push(remainder.to_string());
        }
    }

    PlaceQuery {
        surface: trimmed.to_string(),
        search_keys,
    }
}

#[cfg(test)]
mod tests {
    use super::place_query;

    #[test]
    fn the_hague_keeps_surface() {
        let q = place_query("The Hague");
        assert_eq!(q.surface, "The Hague");
        assert!(q.search_keys.iter().any(|k| k == "The Hague"));
    }

    #[test]
    fn the_united_states_adds_key_without_article() {
        let q = place_query("the United States");
        assert_eq!(q.surface, "the United States");
        assert!(q.search_keys.iter().any(|k| k == "United States"));
    }
}
