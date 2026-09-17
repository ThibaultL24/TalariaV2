// crates/talaria-quality/src/context_year.rs
//! Context-aware approximate year inference for undated bio anecdotes.
//!
//! When an event from a high-confidence bio anecdote type lacks an explicit year,
//! we try to infer an approximate year from:
//! 1. Section headings containing years (e.g., "### 1864-1866", "### Dernières années")
//! 2. Nearby sentences in the same paragraph with years
//! 3. Lifespan bounds (birth to death range)
//!
//! Inferred years are marked as approximate (TypedTime::Approx), not exact.

use crate::model::TypedTime;

/// Event types eligible for context-aware year inference.
/// These are high-confidence bio anecdote types from CORE subject pages.
const APPROX_ELIGIBLE_TYPES: &[&str] = &[
    "meeting",
    "trial",
    "legal_event",
    "health_event",
    "education",
    "burial",
    "political_event",
    "financial_event",
    "scandal",
    "censorship",
    "residence",
    "arrival",
    "departure",
    "travel",
    "exile",
    "battle",
    "siege",
    "work",
    "imprisonment",
];

/// Check if an event type is eligible for approximate year inference.
pub fn is_approx_eligible_type(event_type: &str) -> bool {
    APPROX_ELIGIBLE_TYPES.contains(&event_type)
}

/// Scan text for years in the range [min_year, max_year].
/// Returns all years found, sorted.
pub fn scan_years_in_range(text: &str, min_year: i32, max_year: i32) -> Vec<i32> {
    let mut years = Vec::new();
    for word in text.split(|c: char| !c.is_ascii_digit()) {
        if word.len() == 4 {
            if let Ok(y) = word.parse::<i32>() {
                if y >= min_year && y <= max_year {
                    years.push(y);
                }
            }
        }
    }
    years.sort();
    years.dedup();
    years
}

/// Extract years from a section heading.
/// Handles patterns like "### 1864-1866", "### Dernières années (1864-1867)".
pub fn years_from_section_heading(heading: &str, min_year: i32, max_year: i32) -> Option<(i32, Option<i32>)> {
    let years = scan_years_in_range(heading, min_year, max_year);
    match years.as_slice() {
        [single] => Some((*single, None)),
        [start, end] if end > start => Some((*start, Some(*end))),
        [start, ..] => Some((*start, None)),
        [] => None,
    }
}

/// Find nearby years from surrounding text (paragraph context).
/// Returns the most likely year based on proximity and frequency.
pub fn year_from_nearby_context(
    clause_text: &str,
    full_paragraph: &str,
    birth_year: Option<i32>,
    death_year: Option<i32>,
) -> Option<i32> {
    let min_year = birth_year.unwrap_or(1000);
    let max_year = death_year.unwrap_or(2100);
    
    // First, check if the clause itself has any year we might have missed
    let clause_years = scan_years_in_range(clause_text, min_year, max_year);
    if clause_years.len() == 1 {
        return Some(clause_years[0]);
    }
    
    // Look in the full paragraph for contextual years
    let para_years = scan_years_in_range(full_paragraph, min_year, max_year);
    if para_years.is_empty() {
        return None;
    }
    
    // If only one year in the paragraph, use it
    if para_years.len() == 1 {
        return Some(para_years[0]);
    }
    
    // If multiple years, prefer the one closest to the clause position
    // This is a heuristic: years mentioned earlier in context often apply to later sentences
    // Return the first year that's within the subject's lifespan
    for &y in &para_years {
        if y >= min_year && y <= max_year {
            return Some(y);
        }
    }
    
    None
}

/// Infer an approximate year from lifespan alone.
/// Only used as a last resort for events that clearly happened during life.
/// Returns a midpoint year if the range is reasonable.
pub fn year_from_lifespan_midpoint(
    event_type: &str,
    birth_year: Option<i32>,
    death_year: Option<i32>,
) -> Option<i32> {
    let (birth, death) = match (birth_year, death_year) {
        (Some(b), Some(d)) if d > b => (b, d),
        _ => return None,
    };
    
    // Only for events that definitely happened during life
    // Not for posthumous events like burial (which can be pinpointed differently)
    match event_type {
        "burial" => Some(death), // Burial year is death year
        "education" => {
            // Education typically 6-25 years old
            let edu_start = birth + 6;
            let edu_end = (birth + 25).min(death);
            Some((edu_start + edu_end) / 2)
        }
        _ => {
            // For other events, we're too uncertain to infer from lifespan alone
            // Only use lifespan if we have a narrow range (< 20 years)
            if death - birth <= 20 {
                Some((birth + death) / 2)
            } else {
                None
            }
        }
    }
}

/// Narrow a full document to text near the clause (avoids pinning every anecdote
/// to the first lifespan year in the article).
pub fn local_context_window<'a>(full: &'a str, clause: &str, radius: usize) -> &'a str {
    let clause = clause.trim();
    if clause.is_empty() || full.is_empty() {
        return full;
    }
    let Some(pos) = full.find(clause) else {
        // Fall back to a prefix window — better than whole-doc first-year bias.
        return full.get(..radius.min(full.len())).unwrap_or(full);
    };
    let start = pos.saturating_sub(radius);
    let end = (pos + clause.len().saturating_add(radius)).min(full.len());
    // Expand to char boundaries
    let start = floor_char_boundary(full, start);
    let end = ceil_char_boundary(full, end);
    &full[start..end]
}

fn floor_char_boundary(s: &str, mut i: usize) -> usize {
    if i >= s.len() {
        return s.len();
    }
    while i > 0 && !s.is_char_boundary(i) {
        i -= 1;
    }
    i
}

fn ceil_char_boundary(s: &str, mut i: usize) -> usize {
    if i >= s.len() {
        return s.len();
    }
    while i < s.len() && !s.is_char_boundary(i) {
        i += 1;
    }
    i
}

/// Main entry point: infer an approximate year for an undated event.
/// Returns (year, inference_source) if inference succeeded.
pub fn infer_approximate_year(
    event_type: &str,
    clause_text: &str,
    paragraph_context: Option<&str>,
    section_heading: Option<&str>,
    birth_year: Option<i32>,
    death_year: Option<i32>,
) -> Option<(i32, &'static str)> {
    // Only infer for eligible event types
    if !is_approx_eligible_type(event_type) {
        return None;
    }
    
    let min_year = birth_year.unwrap_or(1000);
    let max_year = death_year.map(|d| d + 5).unwrap_or(2100); // Allow 5 years after death for burial
    
    // 1. Try section heading first (most reliable context)
    if let Some(heading) = section_heading {
        if let Some((start, end)) = years_from_section_heading(heading, min_year, max_year) {
            let year = end.map(|e| (start + e) / 2).unwrap_or(start);
            return Some((year, "section_heading"));
        }
    }
    
    // 2. Try nearby paragraph context
    if let Some(para) = paragraph_context {
        if let Some(y) = year_from_nearby_context(clause_text, para, birth_year, death_year) {
            return Some((y, "paragraph_context"));
        }
    }
    
    // 3. For burial specifically, use death year
    if event_type == "burial" {
        if let Some(dy) = death_year {
            return Some((dy, "death_year_for_burial"));
        }
    }
    
    // 4. For education, try lifespan inference (very conservative)
    if event_type == "education" {
        if let Some(y) = year_from_lifespan_midpoint(event_type, birth_year, death_year) {
            return Some((y, "lifespan_education"));
        }
    }
    
    None
}

/// Create an approximate TypedTime from an inferred year.
pub fn approx_typed_time(year: i32, inference_source: &str) -> TypedTime {
    TypedTime::Approx {
        year,
        surface: Some(format!("~{} (inferred from {})", year, inference_source)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scans_years_correctly() {
        let years = scan_years_in_range("In 1857, the trial happened after events in 1842.", 1800, 1900);
        assert_eq!(years, vec![1842, 1857]);
    }

    #[test]
    fn extracts_year_from_section_heading_single() {
        let (start, end) = years_from_section_heading("### 1866", 1800, 1900).unwrap();
        assert_eq!(start, 1866);
        assert!(end.is_none());
    }

    #[test]
    fn extracts_year_from_section_heading_range() {
        let (start, end) = years_from_section_heading("### 1864-1866", 1800, 1900).unwrap();
        assert_eq!(start, 1864);
        assert_eq!(end, Some(1866));
    }

    #[test]
    fn extracts_year_from_section_heading_with_text() {
        let (start, end) = years_from_section_heading("### Dernières années (1864-1867)", 1800, 1900).unwrap();
        assert_eq!(start, 1864);
        assert_eq!(end, Some(1867));
    }

    #[test]
    fn infers_trial_year_from_paragraph() {
        let clause = "Baudelaire was successfully prosecuted for creating an offense against public morals.";
        let paragraph = "In 1857, Les Fleurs du mal was published. Baudelaire was successfully prosecuted for creating an offense against public morals.";
        let result = infer_approximate_year("trial", clause, Some(paragraph), None, Some(1821), Some(1867));
        assert_eq!(result, Some((1857, "paragraph_context")));
    }

    #[test]
    fn infers_burial_year_from_death_year() {
        let result = infer_approximate_year("burial", "He was buried at Montparnasse.", None, None, Some(1821), Some(1867));
        assert_eq!(result, Some((1867, "death_year_for_burial")));
    }

    #[test]
    fn infers_from_section_heading_over_paragraph() {
        let clause = "He met Félicien Rops.";
        let paragraph = "In 1857, Les Fleurs du mal was published. He met Félicien Rops.";
        let heading = "### 1866";
        let result = infer_approximate_year("meeting", clause, Some(paragraph), Some(heading), Some(1821), Some(1867));
        // Section heading should win over paragraph
        assert_eq!(result, Some((1866, "section_heading")));
    }

    #[test]
    fn education_inference_uses_lifespan() {
        let result = infer_approximate_year("education", "He was educated in Lyon.", None, None, Some(1821), Some(1867));
        // Should infer ~1836 (midpoint of 1827-1846)
        assert!(result.is_some());
        let (year, source) = result.unwrap();
        assert!(year >= 1827 && year <= 1846, "year {} should be in education range", year);
        assert_eq!(source, "lifespan_education");
    }

    #[test]
    fn non_eligible_types_not_inferred() {
        let result = infer_approximate_year("publication", "He published a book.", Some("In 1857, he was active."), None, Some(1821), Some(1867));
        assert!(result.is_none());
    }

    #[test]
    fn creates_approx_typed_time() {
        let t = approx_typed_time(1857, "paragraph_context");
        match t {
            TypedTime::Approx { year, surface } => {
                assert_eq!(year, 1857);
                assert!(surface.unwrap().contains("inferred"));
            }
            _ => panic!("expected Approx"),
        }
    }

    #[test]
    fn respects_lifespan_bounds() {
        // Year outside lifespan should not be picked
        let clause = "Something happened.";
        let paragraph = "In 1999, unrelated event. In 1857, relevant event.";
        let result = infer_approximate_year("trial", clause, Some(paragraph), None, Some(1821), Some(1867));
        assert_eq!(result, Some((1857, "paragraph_context")));
    }

    #[test]
    fn local_window_keeps_nearby_years_not_article_start() {
        let filler = "x".repeat(500);
        let full = format!(
            "Charles Baudelaire was born in 1821 in Paris. {filler} \
             In 1857 Les Fleurs du mal was published. \
             Baudelaire was successfully prosecuted for creating an offense against public morals. \
             He died in 1867."
        );
        let clause = "Baudelaire was successfully prosecuted for creating an offense against public morals.";
        let window = local_context_window(&full, clause, 120);
        assert!(window.contains("1857"), "window={window}");
        assert!(
            !window.contains("1821"),
            "birth year should be outside the local window: {window}"
        );
        let result = infer_approximate_year(
            "trial",
            clause,
            Some(window),
            None,
            Some(1821),
            Some(1867),
        );
        assert_eq!(result, Some((1857, "paragraph_context")));
    }
}
