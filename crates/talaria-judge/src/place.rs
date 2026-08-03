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
        // Napoleon / French Revolutionary & Napoleonic campaigns
        ("ajaccio", 41.9267, 8.7369),
        ("brienne", 48.3933, 4.5228),
        ("brienne-le-chateau", 48.3933, 4.5228),
        ("toulon", 43.1242, 5.928),
        ("austerlitz", 49.1533, 16.875),
        ("waterloo", 50.6794, 4.4047),
        ("elba", 42.777, 10.192),
        ("saint helena", -15.965, -5.712),
        ("st helena", -15.965, -5.712),
        ("russia", 55.7558, 37.6173),
        ("milan", 45.4642, 9.19),
        ("cairo", 30.0444, 31.2357),
        ("egypt", 30.0444, 31.2357),
        ("marengo", 44.888, 8.679),
        ("jena", 50.9272, 11.586),
        ("wagram", 48.25, 16.5667),
        ("leipzig", 51.3397, 12.3731),
        ("corsica", 42.0396, 9.0129),
        ("fontainebleau", 48.4047, 2.7016),
        ("malmaison", 48.8706, 2.1681),
        ("notre-dame", 48.853, 2.3499),
        ("notre dame", 48.853, 2.3499),
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
