// crates/talaria-judge/src/place.rs
use serde_json::json;

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedPlace {
    pub label: String,
    pub lat: Option<f64>,
    pub lon: Option<f64>,
}

pub fn parse_place_surface(surface: &str) -> ParsedPlace {
    let label = surface.trim().trim_end_matches('.').to_string();
    let key = label.to_lowercase();
    let (lat, lon) = gazetteer_lookup(&key).unwrap_or((None, None));
    ParsedPlace {
        label,
        lat,
        lon,
    }
}

impl ParsedPlace {
    pub fn map_eligible(&self) -> bool {
        self.lat.is_some() && self.lon.is_some()
    }

    pub fn to_json(&self) -> serde_json::Value {
        json!({
            "label": self.label,
            "lat": self.lat,
            "lon": self.lon,
        })
    }
}

fn gazetteer_lookup(key: &str) -> Option<(Option<f64>, Option<f64>)> {
    const PLACES: &[(&str, f64, f64)] = &[
        ("london", 51.5074, -0.1278),
        ("paris", 48.8566, 2.3522),
        ("berlin", 52.52, 13.405),
        ("cambridge", 52.2053, 0.1218),
        ("oxford", 51.752, -1.2577),
        ("new york", 40.7128, -74.006),
        ("washington", 38.9072, -77.0369),
        ("boston", 42.3601, -71.0589),
        ("vienna", 48.2082, 16.3738),
        ("rome", 41.9028, 12.4964),
        ("madrid", 40.4168, -3.7038),
        ("moscow", 55.7558, 37.6173),
        ("beijing", 39.9042, 116.4074),
        ("tokyo", 35.6762, 139.6503),
        ("sydney", -33.8688, 151.2093),
        ("bletchley park", 51.9973, -0.7406),
    ];

    PLACES
        .iter()
        .find(|(name, _, _)| *name == key)
        .map(|(_, lat, lon)| (Some(*lat), Some(*lon)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_london_coordinates() {
        let place = parse_place_surface("London");
        assert!(place.map_eligible());
        assert_eq!(place.label, "London");
    }
}
