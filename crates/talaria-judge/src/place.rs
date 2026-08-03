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
        ("borodino", 55.526, 35.821),
        ("smolensk", 54.7826, 32.0853),
        ("friedland", 54.443, 21.011),
        ("tilsit", 55.0833, 21.8833),
        ("amiens", 49.8943, 2.2957),
        ("ulm", 48.4011, 9.9876),
        ("auerstedt", 51.1, 11.583),
        ("jena–auerstedt", 50.9272, 11.586),
        ("jena-auerstedt", 50.9272, 11.586),
        ("the pyramids", 29.9792, 31.1342),
        ("pyramids", 29.9792, 31.1342),
        ("eylau", 54.4, 20.633),
        ("preussisch eylau", 54.4, 20.633),
        ("aspern", 48.216, 16.466),
        ("essling", 48.21, 16.48),
        ("warsaw", 52.2297, 21.0122),
        ("berlin", 52.52, 13.405),
        ("dresden", 51.0504, 13.7373),
        ("prague", 50.0755, 14.4378),
        ("munich", 48.1351, 11.582),
        ("erfurt", 50.9848, 11.0299),
        ("berezina", 54.48, 28.5),
        ("vilna", 54.6872, 25.2797),
        ("vilnius", 54.6872, 25.2797),
        ("liggy", 50.512, 4.266),
        ("ligny", 50.512, 4.266),
        ("wavre", 50.717, 4.611),
        ("brussels", 50.8503, 4.3517),
        ("belgium", 50.8503, 4.3517),
        ("spain", 40.4168, -3.7038),
        ("portugal", 38.7223, -9.1393),
        ("lisbon", 38.7223, -9.1393),
        ("italy", 41.9028, 12.4964),
        ("austria", 48.2082, 16.3738),
        ("prussia", 52.52, 13.405),
        ("germany", 52.52, 13.405),
        ("france", 48.8566, 2.3522),
        ("england", 51.5074, -0.1278),
        ("britain", 51.5074, -0.1278),
        ("malta", 35.8989, 14.5146),
        ("alexandria", 31.2001, 29.9187),
        ("acre", 32.9281, 35.082),
        ("jaffa", 32.0504, 34.7522),
        ("genoa", 44.4056, 8.9463),
        ("turin", 45.0703, 7.6869),
        ("venice", 45.4408, 12.3155),
        ("florence", 43.7696, 11.2558),
        ("naples", 40.8518, 14.2681),
        ("arcola", 45.35, 11.283),
        ("lodi", 45.314, 9.503),
        ("rivoli", 45.566, 10.833),
        ("mantua", 45.1564, 10.7914),
        ("portoferraio", 42.812, 10.316),
        ("longwood", -15.95, -5.72),
        ("tuileries", 48.8636, 2.3272),
        ("saint-cloud", 48.844, 2.219),
        ("saint cloud", 48.844, 2.219),
        ("boulogne", 50.7264, 1.6147),
        ("boulogne-sur-mer", 50.7264, 1.6147),
        ("lyon", 45.764, 4.8357),
        ("grenoble", 45.1885, 5.7245),
        ("nice", 43.7102, 7.262),
        ("antibes", 43.5804, 7.1251),
        ("auxonne", 47.193, 5.388),
        ("valence", 44.9334, 4.892),
        ("avignon", 43.9493, 4.8055),
        ("campo formio", 45.95, 13.3),
        ("lunéville", 48.592, 6.489),
        ("luneville", 48.592, 6.489),
        ("pressburg", 48.1486, 17.1077),
        ("schönbrunn", 48.1845, 16.3122),
        ("schonbrunn", 48.1845, 16.3122),
        ("ratisbon", 49.0134, 12.1016),
        ("regensburg", 49.0134, 12.1016),
        ("lützen", 51.258, 12.141),
        ("lutzen", 51.258, 12.141),
        ("bautzen", 51.1814, 14.427),
        ("hanau", 50.1333, 8.9167),
        ("kulm", 50.7, 13.85),
        ("quatre bras", 50.571, 4.453),
        ("plancenoit", 50.666, 4.417),
        ("charleroi", 50.4108, 4.4446),
        ("arcis-sur-aube", 48.536, 4.14),
        ("craonne", 49.44, 3.72),
        ("montereau", 48.387, 2.957),
        ("montmirail", 48.875, 3.54),
        ("champaubert", 48.88, 3.775),
        ("vauchamps", 48.88, 3.62),
        ("maloyaroslavets", 55.014, 36.478),
        ("vyazma", 55.21, 34.285),
        ("krasnoi", 54.556, 31.43),
        ("minsk", 53.9, 27.5667),
        ("compiegne", 49.4179, 2.8261),
        ("compiègne", 49.4179, 2.8261),
        ("18 brumaire", 48.8566, 2.3522),
        ("erfurt", 50.9848, 11.0299),
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
