// crates/talaria-judge/src/time.rs
use chrono::{DateTime, NaiveDate, TimeZone, Utc};
use serde_json::json;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedTime {
    pub surface: String,
    pub precision: String,
    pub year: i32,
    pub start: DateTime<Utc>,
}

pub fn parse_time_surface(surface: &str) -> Option<ParsedTime> {
    let trimmed = surface.trim().trim_end_matches('.');
    let lower = trimmed.to_ascii_lowercase();
    let is_bc = lower.contains("bc") || lower.contains("b.c");
    let digits: String = trimmed.chars().filter(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() || digits.len() > 4 {
        return None;
    }
    let abs_year: i32 = digits.parse().ok()?;

    if is_bc {
        if !(1..=4000).contains(&abs_year) {
            return None;
        }
        let year = -abs_year;
        // Chrono uses astronomical year numbering: 1 BC = year 0, 31 BC = -30.
        let chrono_year = year + 1;
        let start =
            Utc.from_utc_datetime(&NaiveDate::from_ymd_opt(chrono_year, 6, 15)?.and_hms_opt(0, 0, 0)?);
        return Some(ParsedTime {
            surface: trimmed.to_string(),
            precision: "year".into(),
            year,
            start,
        });
    }

    if digits.len() == 4 && (1000..=2100).contains(&abs_year) {
        let start =
            Utc.from_utc_datetime(&NaiveDate::from_ymd_opt(abs_year, 6, 15)?.and_hms_opt(0, 0, 0)?);
        return Some(ParsedTime {
            surface: trimmed.to_string(),
            precision: "year".into(),
            year: abs_year,
            start,
        });
    }

    None
}

impl ParsedTime {
    pub fn to_json(&self) -> serde_json::Value {
        json!({
            "surface": self.surface,
            "precision": self.precision,
            "year": self.year,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Datelike;

    #[test]
    fn parses_four_digit_year() {
        let parsed = parse_time_surface("1912").unwrap();
        assert_eq!(parsed.year, 1912);
        assert_eq!(parsed.precision, "year");
    }

    #[test]
    fn parses_bc_year() {
        let parsed = parse_time_surface("48 BC").unwrap();
        assert_eq!(parsed.year, -48);
        assert!(parsed.start.year() <= 0);
    }
}
