// crates/talaria-quality/src/time_typed.rs
use crate::model::TypedTime;
use talaria_judge::parse_time_surface;

pub fn parse_typed_time(surface: Option<&str>) -> TypedTime {
    let Some(raw) = surface.map(str::trim).filter(|s| !s.is_empty()) else {
        return TypedTime::Unknown { surface: None };
    };
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

pub fn typed_time_year(time: &TypedTime) -> Option<i32> {
    time.year_for_gates()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_year_as_exact() {
        let t = parse_typed_time(Some("1769"));
        assert_eq!(t.year_for_gates(), Some(1769));
    }
}
