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
    let digits: String = trimmed.chars().filter(|c| c.is_ascii_digit()).collect();

    if digits.len() == 4 {
        let year: i32 = digits.parse().ok()?;
        if !(1000..=2100).contains(&year) {
            return None;
        }
        let start = Utc.from_utc_datetime(&NaiveDate::from_ymd_opt(year, 6, 15)?.and_hms_opt(0, 0, 0)?);
        return Some(ParsedTime {
            surface: trimmed.to_string(),
            precision: "year".into(),
            year,
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

    #[test]
    fn parses_four_digit_year() {
        let parsed = parse_time_surface("1912").unwrap();
        assert_eq!(parsed.year, 1912);
        assert_eq!(parsed.precision, "year");
    }
}
