// crates/talaria-quality/src/time_typed.rs
use crate::model::TypedTime;
use talaria_judge::parse_time_surface;

const MONTHS: &[(&str, u32)] = &[
    ("january", 1),
    ("february", 2),
    ("march", 3),
    ("april", 4),
    ("may", 5),
    ("june", 6),
    ("july", 7),
    ("august", 8),
    ("september", 9),
    ("october", 10),
    ("november", 11),
    ("december", 12),
    ("jan", 1),
    ("feb", 2),
    ("mar", 3),
    ("apr", 4),
    ("jun", 6),
    ("jul", 7),
    ("aug", 8),
    ("sep", 9),
    ("sept", 9),
    ("oct", 10),
    ("nov", 11),
    ("dec", 12),
];

pub fn parse_typed_time(surface: Option<&str>) -> TypedTime {
    let Some(raw) = surface.map(str::trim).filter(|s| !s.is_empty()) else {
        return TypedTime::Unknown { surface: None };
    };
    if let Some(t) = parse_calendar_date(raw) {
        return t;
    }
    match parse_time_surface(raw) {
        Some(p) => TypedTime::Exact {
            year: p.year,
            month: None,
            day: None,
            surface: Some(p.surface),
        },
        None => TypedTime::Unknown {
            surface: Some(raw.to_string()),
        },
    }
}

/// Best time surface in a clause: day-month-year if present, else a 4-digit year.
pub fn extract_time_surface(text: &str) -> Option<String> {
    if let Some(span) = scan_day_month_year(text) {
        return Some(span);
    }
    if let Some(span) = scan_iso_date(text) {
        return Some(span);
    }
    scan_year(text)
}

pub fn typed_time_year(time: &TypedTime) -> Option<i32> {
    time.year_for_gates()
}

fn parse_calendar_date(raw: &str) -> Option<TypedTime> {
    let trimmed = raw.trim().trim_end_matches('.');
    if let Some((year, month, day)) = parse_iso_ymd(trimmed) {
        return Some(TypedTime::Exact {
            year,
            month: Some(month),
            day: Some(day),
            surface: Some(trimmed.to_string()),
        });
    }
    if let Some((year, month, day, _)) = parse_day_month_year(trimmed) {
        return Some(TypedTime::Exact {
            year,
            month: Some(month),
            day: Some(day),
            surface: Some(trimmed.to_string()),
        });
    }
    None
}

fn scan_iso_date(text: &str) -> Option<String> {
    // Iterate by char boundary to avoid splitting multi-byte characters.
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    for &(byte_pos, _) in &chars {
        let remaining = &text[byte_pos..];
        if remaining.len() < 10 {
            break;
        }
        // Take exactly 10 bytes — valid only if that lands on a char boundary.
        let end = byte_pos + 10;
        if end > text.len() || !text.is_char_boundary(end) {
            continue;
        }
        let slice = &text[byte_pos..end];
        if parse_iso_ymd(slice).is_some() {
            return Some(slice.to_string());
        }
    }
    None
}

fn parse_iso_ymd(s: &str) -> Option<(i32, u32, u32)> {
    if s.len() != 10 || s.as_bytes()[4] != b'-' || s.as_bytes()[7] != b'-' {
        return None;
    }
    let year: i32 = s[0..4].parse().ok()?;
    let month: u32 = s[5..7].parse().ok()?;
    let day: u32 = s[8..10].parse().ok()?;
    if !(1000..=2100).contains(&year) || !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    Some((year, month, day))
}

fn scan_day_month_year(text: &str) -> Option<String> {
    parse_day_month_year(text).map(|(_, _, _, span)| span)
}

fn parse_day_month_year(text: &str) -> Option<(i32, u32, u32, String)> {
    let lower = text.to_lowercase();
    for (name, month) in MONTHS {
        let mut from = 0usize;
        while let Some(rel) = lower[from..].find(name) {
            let mstart = from + rel;
            let mend = mstart + name.len();
            if !month_boundaries(&lower, mstart, mend) {
                from = mend;
                continue;
            }
            let after = text[mend..].trim_start_matches(|c: char| c == ',' || c == ' ');
            let Some(year) = leading_year(after) else {
                from = mend;
                continue;
            };
            let before = text[..mstart].trim_end();
            let Some(day) = trailing_day(before) else {
                from = mend;
                continue;
            };
            let span = format!("{day} {name} {year}");
            return Some((year, *month, day, span));
        }
    }
    None
}

fn month_boundaries(lower: &str, start: usize, end: usize) -> bool {
    let b = start == 0 || !lower.as_bytes()[start - 1].is_ascii_alphabetic();
    let a = end >= lower.len() || !lower.as_bytes()[end].is_ascii_alphabetic();
    b && a
}

fn leading_year(s: &str) -> Option<i32> {
    let digits: String = s.chars().take(4).filter(|c| c.is_ascii_digit()).collect();
    if digits.len() != 4 {
        return None;
    }
    let y: i32 = digits.parse().ok()?;
    (1000..=2100).contains(&y).then_some(y)
}

fn trailing_day(before: &str) -> Option<u32> {
    let tok = before
        .rsplit(|c: char| !c.is_ascii_digit())
        .find(|s| !s.is_empty())?;
    let day: u32 = tok.parse().ok()?;
    (1..=31).contains(&day).then_some(day)
}

fn scan_year(text: &str) -> Option<String> {
    let lower = text.to_lowercase();
    if let Some(s) = scan_bce_surface(&lower) {
        return Some(s);
    }
    for word in text.split(|c: char| !c.is_ascii_digit()) {
        if word.len() == 4 {
            if let Ok(y) = word.parse::<i32>() {
                if (1..=2100).contains(&y) {
                    return Some(y.to_string());
                }
            }
        }
    }
    None
}

fn scan_bce_surface(lower: &str) -> Option<String> {
    let bytes = lower.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i].is_ascii_digit() {
            let start = i;
            while i < bytes.len() && bytes[i].is_ascii_digit() {
                i += 1;
            }
            let digits = &lower[start..i];
            if digits.len() <= 4 && !digits.is_empty() {
                let rest = lower[i..].trim_start();
                if rest.starts_with("bc")
                    || rest.starts_with("b.c")
                    || rest.starts_with("bce")
                    || rest.starts_with("av. j")
                    || rest.starts_with("av j")
                {
                    return Some(format!("{} BC", digits.trim_start_matches('0')));
                }
            }
            continue;
        }
        i += 1;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_year_as_exact_without_month_day() {
        let t = parse_typed_time(Some("1769"));
        match t {
            TypedTime::Exact {
                year,
                month,
                day,
                ..
            } => {
                assert_eq!(year, 1769);
                assert!(month.is_none());
                assert!(day.is_none());
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn parses_day_month_year() {
        let t = parse_typed_time(Some("15 August 1769"));
        match t {
            TypedTime::Exact {
                year,
                month,
                day,
                ..
            } => {
                assert_eq!(year, 1769);
                assert_eq!(month, Some(8));
                assert_eq!(day, Some(15));
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn extracts_full_date_from_clause() {
        let s = extract_time_surface(
            "Napoleon Bonaparte was born on 15 August 1769 in Ajaccio.",
        )
        .unwrap();
        let t = parse_typed_time(Some(&s));
        assert_eq!(t.year_for_gates(), Some(1769));
        match t {
            TypedTime::Exact { month, day, .. } => {
                assert_eq!(month, Some(8));
                assert_eq!(day, Some(15));
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn year_only_clause_has_no_month() {
        let s = extract_time_surface("In 1814 Napoleon was exiled to Elba.").unwrap();
        assert_eq!(s, "1814");
        match parse_typed_time(Some(&s)) {
            TypedTime::Exact { month, day, .. } => {
                assert!(month.is_none());
                assert!(day.is_none());
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn extracts_bce_year() {
        let s = extract_time_surface("Cleopatra died in 30 BC in Alexandria.").unwrap();
        assert!(s.to_lowercase().contains("bc"), "{s}");
        let t = parse_typed_time(Some(&s));
        assert_eq!(t.year_for_gates(), Some(-30), "{t:?}");
    }

    #[test]
    fn time_to_json_exact_year_uses_kind_exact_not_year() {
        let t = TypedTime::Exact {
            year: 1805,
            month: None,
            day: None,
            surface: Some("1805".into()),
        };
        let v = time_to_json(&t);
        assert_eq!(v["kind"], "exact");
        assert_eq!(v["precision"], "year");
        assert_eq!(v["start"], "1805");
        assert!(v.get("end").unwrap().is_null() || v["end"] == serde_json::Value::Null);
        assert_eq!(v["calendar"], "gregorian");
        assert_eq!(v["surface"], "1805");
    }

    #[test]
    fn time_to_json_never_uses_precision_as_kind() {
        let t = TypedTime::Exact {
            year: 1805,
            month: Some(3),
            day: None,
            surface: Some("March 1805".into()),
        };
        let v = time_to_json(&t);
        assert_eq!(v["kind"], "exact");
        assert_eq!(v["precision"], "month");
        assert_eq!(v["start"], "1805-03");
    }

    #[test]
    fn start_time_year_projects_to_january_first_not_june() {
        let t = TypedTime::Exact {
            year: 1805,
            month: None,
            day: None,
            surface: Some("1805".into()),
        };
        let dt = start_time_from_typed(&t).expect("projection");
        assert_eq!(dt.format("%Y-%m-%d").to_string(), "1805-01-01");
    }

    #[test]
    fn start_time_month_projects_to_day_one() {
        let t = TypedTime::Exact {
            year: 1805,
            month: Some(3),
            day: None,
            surface: Some("March 1805".into()),
        };
        let dt = start_time_from_typed(&t).expect("projection");
        assert_eq!(dt.format("%Y-%m-%d").to_string(), "1805-03-01");
    }
}

/// Serialise a `TypedTime` to a JSON value for storage in `canonical_events.time_json`.
pub fn time_to_json(time: &TypedTime) -> serde_json::Value {
    match time {
        TypedTime::Exact {
            year,
            month,
            day,
            surface,
        } => {
            let precision = if day.is_some() {
                "day"
            } else if month.is_some() {
                "month"
            } else {
                "year"
            };
            serde_json::json!({
                "kind": "exact",
                "start": format_exact_start(*year, *month, *day),
                "end": serde_json::Value::Null,
                "precision": precision,
                "calendar": "gregorian",
                "surface": surface,
            })
        }
        TypedTime::Range {
            start_year,
            end_year,
            surface,
        } => serde_json::json!({
            "kind": "range",
            "start": start_year.to_string(),
            "end": end_year.to_string(),
            "precision": "year",
            "calendar": "gregorian",
            "surface": surface,
        }),
        TypedTime::Approx { year, surface } => serde_json::json!({
            "kind": "approx",
            "start": year.to_string(),
            "end": serde_json::Value::Null,
            "precision": "year",
            "calendar": "gregorian",
            "surface": surface,
        }),
        TypedTime::Unknown { surface } => serde_json::json!({
            "kind": "unknown",
            "start": serde_json::Value::Null,
            "end": serde_json::Value::Null,
            "calendar": "gregorian",
            "surface": surface,
        }),
    }
}

fn format_exact_start(year: i32, month: Option<u32>, day: Option<u32>) -> String {
    match (month, day) {
        (Some(m), Some(d)) => format!("{year}-{m:02}-{d:02}"),
        (Some(m), None) => format!("{year}-{m:02}"),
        _ => year.to_string(),
    }
}

/// Convert a `TypedTime` to a `DateTime<Utc>` suitable for `start_time` columns.
/// Year-only → 1 January; month-only → day 1 of that month; full day → that date.
pub fn start_time_from_typed(time: &TypedTime) -> Option<chrono::DateTime<chrono::Utc>> {
    match time {
        TypedTime::Exact { year, month, day, .. } => {
            let m = month.unwrap_or(1);
            let d = day.unwrap_or(1);
            chrono::NaiveDate::from_ymd_opt(*year, m, d)
                .and_then(|nd| nd.and_hms_opt(0, 0, 0))
                .map(|n| chrono::DateTime::from_naive_utc_and_offset(n, chrono::Utc))
        }
        _ => time
            .year_for_gates()
            .and_then(|y| parse_time_surface(&y.to_string()).map(|p| p.start)),
    }
}
