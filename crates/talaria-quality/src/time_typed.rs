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
    ("decembre", 12),
    ("décembre", 12),
    ("janvier", 1),
    ("fevrier", 2),
    ("février", 2),
    ("mars", 3),
    ("avril", 4),
    ("mai", 5),
    ("juin", 6),
    ("juillet", 7),
    ("aout", 8),
    ("août", 8),
    ("septembre", 9),
    ("octobre", 10),
    ("novembre", 11),
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
    if let Some(t) = short_time_reading(raw) {
        return t;
    }
    if let Some(ClauseTime::Range {
        start_year,
        end_year,
        start_month,
        start_day,
        end_month,
        end_day,
        surface,
    }) = calendar_clause(raw)
    {
        return TypedTime::Range {
            start_year,
            end_year,
            start_month,
            start_day,
            end_month,
            end_day,
            surface: Some(surface),
        };
    }
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
    // Work entirely in the lowercased domain to avoid UTF-8 boundary issues when
    // multi-byte chars have different lengths after lowercasing (e.g. Turkish İ → i).
    let lower = text.to_lowercase();
    for (name, month) in MONTHS {
        let mut from = 0usize;
        while let Some(rel) = lower.get(from..)?.find(name) {
            let mstart = from + rel;
            let mend = mstart + name.len();
            if !month_boundaries(&lower, mstart, mend) {
                from = mend;
                continue;
            }
            // Slice from `lower` (where positions came from), not `text`
            let after = lower
                .get(mend..)?
                .trim_start_matches(|c: char| c == ',' || c == ' ');
            let Some(year) = leading_year(after) else {
                from = mend;
                continue;
            };
            let before = lower.get(..mstart)?.trim_end();
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

/// How a clause supports a date. A quantity, a publication year, or a second
/// action does not become the event year.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClauseTime {
    Exact {
        year: i32,
        month: Option<u32>,
        day: Option<u32>,
        surface: String,
    },
    Range {
        start_year: i32,
        end_year: i32,
        start_month: Option<u32>,
        start_day: Option<u32>,
        end_month: Option<u32>,
        end_day: Option<u32>,
        surface: String,
    },
    /// The clause does not establish one historical date.
    Ambiguous {
        reason: &'static str,
    },
    Missing,
}

struct DatedPoint {
    year: i32,
    month: u32,
    day: u32,
    start: usize,
    end: usize,
}

fn short_time_reading(raw: &str) -> Option<TypedTime> {
    let lower = raw.trim().to_lowercase();
    if matches!(
        lower.as_str(),
        "unknown" | "undated" | "date inconnue" | "date unknown"
    ) {
        return Some(TypedTime::Unknown {
            surface: Some(raw.trim().to_string()),
        });
    }
    let open = lower.starts_with("c.")
        || lower.starts_with("circa")
        || lower.starts_with("vers ")
        || lower.starts_with("environ ")
        || lower.starts_with("after ")
        || lower.starts_with("apres ")
        || lower.starts_with("après ")
        || lower.starts_with("from ")
        || lower.starts_with("à partir de ")
        || lower.starts_with("a partir de ");
    if !open || raw.trim().len() > 32 || four_digit_years(raw).len() != 1 {
        return None;
    }
    let year = four_digit_years(raw).first().map(|(year, _, _)| *year)?;
    Some(TypedTime::Approx {
        year,
        surface: Some(raw.trim().to_string()),
    })
}

fn calendar_clause(text: &str) -> Option<ClauseTime> {
    let lower = text.to_lowercase();
    if lower.len() != text.len() {
        return None;
    }
    if let Some(span) = same_month_day_span(&lower) {
        return Some(span);
    }
    let dates = dated_points(&lower);
    if dates.len() < 2 {
        return None;
    }
    let first = &dates[0];
    let second = &dates[1];
    let between = lower.get(first.end..second.start)?.trim();
    if !range_connector(between) {
        return None;
    }
    Some(ClauseTime::Range {
        start_year: first.year,
        end_year: second.year,
        start_month: Some(first.month),
        start_day: Some(first.day),
        end_month: Some(second.month),
        end_day: Some(second.day),
        surface: text
            .get(first.start..second.end)
            .unwrap_or("")
            .trim()
            .to_string(),
    })
}

fn range_connector(between: &str) -> bool {
    let folded = between
        .trim_matches(|c: char| c == ',' || c == '.' || c == ';' || c == ':')
        .trim();
    matches!(
        folded,
        "to" | "until" | "au" | "à" | "a" | "-" | "–" | "—" | "jusqu'au" | "jusqu’au" | "jusqu'en"
            | "jusqu’en"
    )
}

fn same_month_day_span(lower: &str) -> Option<ClauseTime> {
    for (name, month) in MONTHS {
        let mut from = 0usize;
        while let Some(rel) = lower.get(from..)?.find(name) {
            let mstart = from + rel;
            let mend = mstart + name.len();
            if !month_boundaries(lower, mstart, mend) {
                from = mend;
                continue;
            }
            let after = lower
                .get(mend..)?
                .trim_start_matches(|c: char| c == ',' || c == ' ');
            let Some(year) = leading_year(after) else {
                from = mend;
                continue;
            };
            let before = lower.get(..mstart)?.trim_end();
            let Some((start_day, end_day, day_start)) = trailing_day_span(before) else {
                from = mend;
                continue;
            };
            let year_digits = year.to_string();
            let year_at = after.find(year_digits.as_str())?;
            let raw_after = lower.get(mend..)?;
            let skipped = raw_after.len() - after.len();
            let end = mend + skipped + year_at + year_digits.len();
            return Some(ClauseTime::Range {
                start_year: year,
                end_year: year,
                start_month: Some(*month),
                start_day: Some(start_day),
                end_month: Some(*month),
                end_day: Some(end_day),
                surface: lower
                    .get(day_start..end)
                    .unwrap_or("")
                    .trim()
                    .to_string(),
            });
        }
    }
    None
}

fn trailing_day_span(before: &str) -> Option<(u32, u32, usize)> {
    let parts: Vec<&str> = before.split_whitespace().collect();
    if parts.len() < 2 {
        return None;
    }
    let last = *parts.last()?;
    for sep in ["–", "—", "-"] {
        if let Some((left, right)) = last.split_once(sep) {
            let start_day = parse_day_token(left)?;
            let end_day = parse_day_token(right)?;
            if end_day < start_day {
                return None;
            }
            let start = before.rfind(last)?;
            return Some((start_day, end_day, start));
        }
    }
    let joiner = parts[parts.len() - 2];
    if joiner == "and" || joiner == "et" {
        let end_day = parse_day_token(last)?;
        let start_token = parts.get(parts.len().checked_sub(3)?)?;
        let start_day = parse_day_token(start_token)?;
        if end_day < start_day {
            return None;
        }
        let start = before.rfind(start_token)?;
        return Some((start_day, end_day, start));
    }
    None
}

fn parse_day_token(token: &str) -> Option<u32> {
    let day: u32 = token
        .trim_matches(|c: char| !c.is_ascii_digit())
        .parse()
        .ok()?;
    (1..=31).contains(&day).then_some(day)
}

fn dated_points(lower: &str) -> Vec<DatedPoint> {
    let mut found = Vec::new();
    for (name, month) in MONTHS {
        let mut from = 0usize;
        while let Some(rel) = lower.get(from..).and_then(|rest| rest.find(name)) {
            let mstart = from + rel;
            let mend = mstart + name.len();
            if !month_boundaries(lower, mstart, mend) {
                from = mend;
                continue;
            }
            let after = lower
                .get(mend..)
                .unwrap_or("")
                .trim_start_matches(|c: char| c == ',' || c == ' ');
            let Some(year) = leading_year(after) else {
                from = mend;
                continue;
            };
            let before = lower.get(..mstart).unwrap_or("").trim_end();
            let Some(day) = trailing_day(before) else {
                from = mend;
                continue;
            };
            let day_token = day.to_string();
            let trimmed = before.trim_end();
            if !trimmed.ends_with(day_token.as_str()) {
                from = mend;
                continue;
            }
            let day_at = trimmed.len() - day_token.len();
            let year_digits = year.to_string();
            let Some(year_at) = after.find(year_digits.as_str()) else {
                from = mend;
                continue;
            };
            let raw_after = lower.get(mend..).unwrap_or("");
            let skipped = raw_after.len() - after.len();
            let end = mend + skipped + year_at + year_digits.len();
            if !found.iter().any(|point: &DatedPoint| ranges_overlap(point.start, point.end, day_at, end))
            {
                found.push(DatedPoint {
                    year,
                    month: *month,
                    day,
                    start: day_at,
                    end,
                });
            }
            from = mend;
        }
    }
    found.sort_by_key(|point| point.start);
    found
}

fn ranges_overlap(a0: usize, a1: usize, b0: usize, b1: usize) -> bool {
    a0 < b1 && b0 < a1
}

pub fn read_clause_time(text: &str) -> ClauseTime {
    let masked = mask_thousands(text);
    if let Some(span) = calendar_clause(&masked) {
        return span;
    }
    let years = four_digit_years(&masked);
    let historical: Vec<(i32, usize, usize)> = if work_mentions_its_own_date(&masked) {
        years
            .into_iter()
            .filter(|(_, start, _)| !year_is_parenthetical(&masked, *start))
            .collect()
    } else {
        years
    };
    let dropped_publication =
        work_mentions_its_own_date(&masked) && four_digit_years(&masked).len() > historical.len();
    if let Some((start_year, end_year)) = year_span(&masked, &historical) {
        return ClauseTime::Range {
            start_year,
            end_year,
            start_month: None,
            start_day: None,
            end_month: None,
            end_day: None,
            surface: format!("{start_year}–{end_year}"),
        };
    }
    if let Some((year, month, day, surface)) = dated_span(&masked) {
        if historical.iter().any(|(other, _, _)| *other != year) {
            return ClauseTime::Ambiguous {
                reason: "another_action_in_the_same_passage",
            };
        }
        return ClauseTime::Exact {
            year,
            month: Some(month),
            day: Some(day),
            surface,
        };
    }
    match historical.as_slice() {
        [(year, _, _)] => ClauseTime::Exact {
            year: *year,
            month: None,
            day: None,
            surface: year.to_string(),
        },
        [] if dropped_publication => ClauseTime::Ambiguous {
            reason: "publication_or_cultural_year",
        },
        [] => {
            if let Some((year, surface)) = explicit_bce(&masked) {
                ClauseTime::Exact {
                    year,
                    month: None,
                    day: None,
                    surface,
                }
            } else {
                ClauseTime::Missing
            }
        }
        _ => ClauseTime::Ambiguous {
            reason: "several_years_in_one_passage",
        },
    }
}

fn mask_thousands(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::new();
    let mut index = 0;
    while index < chars.len() {
        if chars[index].is_ascii_digit() {
            let start = index;
            while index < chars.len() && chars[index].is_ascii_digit() {
                index += 1;
            }
            let mut end = index;
            while end < chars.len() && chars[end] == ',' {
                let after = end + 1;
                let mut cursor = after;
                while cursor < chars.len() && chars[cursor].is_ascii_digit() {
                    cursor += 1;
                }
                if cursor - after == 3 {
                    end = cursor;
                } else {
                    break;
                }
            }
            if end > index {
                out.extend(std::iter::repeat(' ').take(end - start));
                index = end;
            } else {
                out.extend(chars[start..index].iter().copied());
            }
        } else {
            out.push(chars[index]);
            index += 1;
        }
    }
    out
}

fn explicit_bce(text: &str) -> Option<(i32, String)> {
    let lower = text.to_lowercase();
    if lower.len() != text.len() {
        return None;
    }
    let surface = scan_bce_surface(&lower)?;
    let year = parse_typed_time(Some(&surface)).year_for_gates()?;
    if year >= 0 {
        return None;
    }
    Some((year, surface))
}

fn four_digit_years(text: &str) -> Vec<(i32, usize, usize)> {
    let bytes = text.as_bytes();
    let mut index = 0;
    let mut found = Vec::new();
    while index < bytes.len() {
        if bytes[index].is_ascii_digit() {
            let start = index;
            while index < bytes.len() && bytes[index].is_ascii_digit() {
                index += 1;
            }
            if index - start == 4 {
                if let Ok(year) = text[start..index].parse::<i32>() {
                    if (1000..=2100).contains(&year) {
                        found.push((year, start, index));
                    }
                }
            }
            continue;
        }
        index += 1;
    }
    found
}

fn year_is_parenthetical(text: &str, start: usize) -> bool {
    text[..start].chars().rev().find(|c| !c.is_whitespace()) == Some('(')
}

fn work_mentions_its_own_date(text: &str) -> bool {
    let folded = text.to_lowercase();
    [
        "osprey",
        "jeu ",
        "game",
        "published",
        "paru",
        " vol",
        "film",
        "roman",
    ]
    .iter()
    .any(|cue| folded.contains(cue))
}

fn year_span(text: &str, years: &[(i32, usize, usize)]) -> Option<(i32, i32)> {
    if years.len() < 2 {
        return None;
    }
    let (_, _, left_end) = years[0];
    let (right_year, right_start, _) = years[1];
    let between = &text[left_end..right_start];
    let folded = between.to_lowercase();
    let compact = folded.split_whitespace().collect::<Vec<_>>().join(" ");
    let short = compact.chars().count() <= 12
        && ["a", "à", "to", "-", "–", "—", "until"].contains(&compact.as_str());
    let long_range = folded.contains("jusqu") || folded.contains("until");
    if short || long_range {
        let start = years[0].0.min(right_year);
        let end = years[0].0.max(right_year);
        return Some((start, end));
    }
    None
}

fn dated_span(text: &str) -> Option<(i32, u32, u32, String)> {
    if let Some(iso) = scan_iso_date(text) {
        let (year, month, day) = parse_iso_ymd(&iso)?;
        return Some((year, month, day, iso));
    }
    parse_day_month_year(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_year_as_exact_without_month_day() {
        let t = parse_typed_time(Some("1769"));
        match t {
            TypedTime::Exact {
                year, month, day, ..
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
                year, month, day, ..
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
        let s = extract_time_surface("Napoleon Bonaparte was born on 15 August 1769 in Ajaccio.")
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
    fn an_ancient_birth_keeps_the_year_and_does_not_invent_a_day() {
        match read_clause_time("Aristotle was born in Stagira in 384 BC.") {
            ClauseTime::Exact { year, month, day, .. } => {
                assert_eq!(year, -384);
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

    #[test]
    fn start_time_range_year_projects_to_january_first() {
        let t = TypedTime::Range {
            start_year: 1804,
            end_year: 1815,
            start_month: None,
            start_day: None,
            end_month: None,
            end_day: None,
            surface: Some("1804–1815".into()),
        };
        let dt = start_time_from_typed(&t).expect("projection");
        assert_eq!(dt.format("%Y-%m-%d").to_string(), "1804-01-01");
    }

    #[test]
    fn a_siege_keeps_both_day_bounds() {
        let clause = "The siege of Mantua lasted from 4 June 1796 to 2 February 1797.";
        let ClauseTime::Range {
            start_year,
            end_year,
            start_month,
            start_day,
            end_month,
            end_day,
            ..
        } = read_clause_time(clause)
        else {
            panic!("Mantua must stay a range");
        };
        assert_eq!(
            (start_year, start_month, start_day, end_year, end_month, end_day),
            (1796, Some(6), Some(4), 1797, Some(2), Some(2))
        );
        let typed = parse_typed_time(Some("4 June 1796 to 2 February 1797"));
        let json = time_to_json(&typed);
        assert_eq!(json["kind"], "range");
        assert_eq!(json["precision"], "day");
        assert_eq!(json["start"], "1796-06-04");
        assert_eq!(json["end"], "1797-02-02");
        let projected = start_time_from_typed(&typed).expect("start");
        assert_eq!(projected.format("%Y-%m-%d").to_string(), "1796-06-04");
    }

    #[test]
    fn multi_day_battles_keep_their_span() {
        let arcole = read_clause_time(
            "The Battle of Arcole (15–17 November 1796) was fought southeast of Verona.",
        );
        assert!(matches!(
            arcole,
            ClauseTime::Range {
                start_year: 1796,
                end_year: 1796,
                start_month: Some(11),
                start_day: Some(15),
                end_month: Some(11),
                end_day: Some(17),
                ..
            }
        ));
        let dego = read_clause_time(
            "The Second Battle of Dego was fought on 14 and 15 April 1796 near Dego.",
        );
        assert!(matches!(
            dego,
            ClauseTime::Range {
                start_year: 1796,
                end_year: 1796,
                start_month: Some(4),
                start_day: Some(14),
                end_month: Some(4),
                end_day: Some(15),
                ..
            }
        ));
    }

    #[test]
    fn open_approximate_and_unknown_dates_stay_distinct() {
        assert!(matches!(
            parse_typed_time(Some("c. 1797")),
            TypedTime::Approx { year: 1797, .. }
        ));
        assert!(matches!(
            parse_typed_time(Some("from 1796")),
            TypedTime::Approx { year: 1796, .. }
        ));
        assert!(matches!(
            parse_typed_time(Some("date inconnue")),
            TypedTime::Unknown { .. }
        ));
        assert!(matches!(
            parse_typed_time(None),
            TypedTime::Unknown { surface: None }
        ));
        assert!(matches!(
            parse_typed_time(Some("5 August 1796")),
            TypedTime::Exact {
                year: 1796,
                month: Some(8),
                day: Some(5),
                ..
            }
        ));
    }

    #[test]
    fn start_time_approx_year_projects_to_january_first() {
        let t = TypedTime::Approx {
            year: 1799,
            surface: Some("c. 1799".into()),
        };
        let dt = start_time_from_typed(&t).expect("projection");
        assert_eq!(dt.format("%Y-%m-%d").to_string(), "1799-01-01");
    }

    #[test]
    fn date_precision_exact_with_day() {
        let t = TypedTime::Exact {
            year: 1769,
            month: Some(8),
            day: Some(15),
            surface: Some("15 August 1769".into()),
        };
        assert_eq!(t.date_precision(), "day");
    }

    #[test]
    fn date_precision_exact_with_month_only() {
        let t = TypedTime::Exact {
            year: 1805,
            month: Some(3),
            day: None,
            surface: Some("March 1805".into()),
        };
        assert_eq!(t.date_precision(), "month");
    }

    #[test]
    fn date_precision_exact_year_only() {
        let t = TypedTime::Exact {
            year: 1805,
            month: None,
            day: None,
            surface: Some("1805".into()),
        };
        assert_eq!(t.date_precision(), "year");
    }

    #[test]
    fn date_precision_range() {
        let t = TypedTime::Range {
            start_year: 1804,
            end_year: 1815,
            start_month: None,
            start_day: None,
            end_month: None,
            end_day: None,
            surface: Some("1804–1815".into()),
        };
        assert_eq!(t.date_precision(), "range");
    }

    #[test]
    fn date_precision_approx() {
        let t = TypedTime::Approx {
            year: 1799,
            surface: Some("c. 1799".into()),
        };
        assert_eq!(t.date_precision(), "approx");
    }

    #[test]
    fn date_precision_unknown() {
        let t = TypedTime::Unknown {
            surface: Some("sometime".into()),
        };
        assert_eq!(t.date_precision(), "unknown");
    }

    #[test]
    fn audit_clauses_keep_a_real_date_and_refuse_a_quantity() {
        match read_clause_time("The Battle of Lodi was fought on 10 May 1796 at Lodi.") {
            ClauseTime::Exact {
                year, month, day, ..
            } => {
                assert_eq!((year, month, day), (1796, Some(5), Some(10)));
            }
            other => panic!("{other:?}"),
        }
        match read_clause_time("The Battle of Mondovì was fought on 21 April 1796.") {
            ClauseTime::Exact {
                year, month, day, ..
            } => {
                assert_eq!((year, month, day), (1796, Some(4), Some(21)));
            }
            other => panic!("{other:?}"),
        }
        match read_clause_time("Messe du Sacre de Napoléon Ier, le 2 décembre 1804.") {
            ClauseTime::Exact {
                year, month, day, ..
            } => {
                assert_eq!((year, month, day), (1804, Some(12), Some(2)));
            }
            other => panic!("{other:?}"),
        }
        match read_clause_time("In 1814 Napoleon was exiled to Elba.") {
            ClauseTime::Exact {
                year, month, day, ..
            } => {
                assert_eq!(year, 1814);
                assert!(month.is_none() && day.is_none());
            }
            other => panic!("{other:?}"),
        }
        assert!(matches!(
            read_clause_time(
                "With 2,000 French casualties, Napoleon had captured 60,000 Austrian soldiers."
            ),
            ClauseTime::Missing
        ));
        assert!(matches!(
            read_clause_time("The Allies lost 838 men, 150 of whom were killed."),
            ClauseTime::Missing
        ));
        assert!(matches!(
            read_clause_time(
                "(1993), Jena 1806 : Napoleon destroys Prussia, Osprey Campaign Series."
            ),
            ClauseTime::Exact {
                year: 1806,
                month: None,
                day: None,
                ..
            }
        ));
        assert!(matches!(
            read_clause_time(
                "Napoleon Saga: Waterloo (2016), jeu de stratégie, campagne de Belgique en 1815."
            ),
            ClauseTime::Exact {
                year: 1815,
                month: None,
                day: None,
                ..
            }
        ));
        assert!(matches!(
            read_clause_time("À partir de 1786, il est atteint de paludisme jusqu'en 1796."),
            ClauseTime::Range {
                start_year: 1786,
                end_year: 1796,
                ..
            }
        ));
        assert!(matches!(
            read_clause_time("De 1805 à 1811, Napoléon reçoit des décorations étrangères."),
            ClauseTime::Range {
                start_year: 1805,
                end_year: 1811,
                ..
            }
        ));
        assert!(matches!(
            read_clause_time(
                "In December 1805, Napoleon abolished the calendar introduced in 1793."
            ),
            ClauseTime::Ambiguous {
                reason: "several_years_in_one_passage"
            }
        ));
        assert!(matches!(
            read_clause_time("After a series of battles, the allies forced a retreat at Arcis-sur-Aube (20–21 March)."),
            ClauseTime::Missing
        ));
        assert!(matches!(
            read_clause_time(
                "Après la victoire à la bataille de Wagram, un traité est signé au château."
            ),
            ClauseTime::Missing
        ));
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
            start_month,
            start_day,
            end_month,
            end_day,
            surface,
        } => {
            let precision = if start_day.is_some() && end_day.is_some() {
                "day"
            } else if start_month.is_some() || end_month.is_some() {
                "month"
            } else {
                "year"
            };
            serde_json::json!({
                "kind": "range",
                "start": format_exact_start(*start_year, *start_month, *start_day),
                "end": format_exact_start(*end_year, *end_month, *end_day),
                "precision": precision,
                "calendar": "gregorian",
                "surface": surface,
            })
        }
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

fn utc_ymd(year: i32, month: u32, day: u32) -> Option<chrono::DateTime<chrono::Utc>> {
    chrono::NaiveDate::from_ymd_opt(year, month, day)
        .and_then(|nd| nd.and_hms_opt(0, 0, 0))
        .map(|n| chrono::DateTime::from_naive_utc_and_offset(n, chrono::Utc))
}

/// Convert a `TypedTime` to a `DateTime<Utc>` suitable for `start_time` columns.
/// Year-only → 1 January; month-only → day 1 of that month; full day → that date.
pub fn start_time_from_typed(time: &TypedTime) -> Option<chrono::DateTime<chrono::Utc>> {
    match time {
        TypedTime::Exact {
            year, month, day, ..
        } => utc_ymd(*year, month.unwrap_or(1), day.unwrap_or(1)),
        TypedTime::Range {
            start_year,
            start_month,
            start_day,
            ..
        } => utc_ymd(*start_year, start_month.unwrap_or(1), start_day.unwrap_or(1)),
        TypedTime::Approx { year, .. } => utc_ymd(*year, 1, 1),
        TypedTime::Unknown { .. } => None,
    }
}
