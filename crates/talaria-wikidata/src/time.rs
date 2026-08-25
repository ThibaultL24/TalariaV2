// crates/talaria-wikidata/src/time.rs
//! Typed Wikibase time values (precision, calendar, signed years including BCE).

const DEFAULT_PRECISION: i32 = 11;
const GREGORIAN: &str = "http://www.wikidata.org/entity/Q1985727";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WikibaseTime {
    pub year: i32,
    pub precision: i32,
    pub calendar: String,
}

pub fn parse_wikibase_time(
    time: &str,
    precision: Option<i32>,
    calendar: Option<&str>,
) -> Option<WikibaseTime> {
    let trimmed = time.trim();
    if trimmed.is_empty() {
        return None;
    }
    let (negative, rest) = if let Some(rest) = trimmed.strip_prefix('+') {
        (false, rest)
    } else if let Some(rest) = trimmed.strip_prefix('-') {
        (true, rest)
    } else {
        (false, trimmed)
    };
    let year_str = rest.split('-').next()?;
    if year_str.is_empty() {
        return None;
    }
    let mut year: i32 = year_str.parse().ok()?;
    if negative {
        year = -year;
    }
    Some(WikibaseTime {
        year,
        precision: precision.unwrap_or(DEFAULT_PRECISION),
        calendar: calendar.unwrap_or(GREGORIAN).to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bce_year_negative() {
        let t = parse_wikibase_time("-0044-03-15T00:00:00Z", Some(11), None).unwrap();
        assert_eq!(t.year, -44);
    }

    #[test]
    fn ce_year() {
        let t = parse_wikibase_time("+1769-08-15T00:00:00Z", Some(11), None).unwrap();
        assert_eq!(t.year, 1769);
    }

    #[test]
    fn year_more_than_four_digits() {
        let t = parse_wikibase_time("+12000-00-00T00:00:00Z", Some(9), None).unwrap();
        assert_eq!(t.year, 12000);
    }

    #[test]
    fn padded_eleven_digit_year() {
        let t = parse_wikibase_time("+00000001769-08-15T00:00:00Z", Some(11), None).unwrap();
        assert_eq!(t.year, 1769);
    }

    #[test]
    fn default_precision_and_gregorian_calendar() {
        let t = parse_wikibase_time("+1769-08-15T00:00:00Z", None, None).unwrap();
        assert_eq!(t.precision, DEFAULT_PRECISION);
        assert_eq!(t.calendar, GREGORIAN);
    }
}
