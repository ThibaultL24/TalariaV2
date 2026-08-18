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

const GAZETTEER: &[(&str, f64, f64)] = &[
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
        // Multi-profile bios (Curie, Hugo, Leonardo, Columbus, Turing, Cleopatra)
        ("besançon", 47.2378, 6.0241),
        ("besancon", 47.2378, 6.0241),
        ("hauteville house", 49.457, -2.536),
        ("saint peter port", 49.4557, -2.535),
        ("guernsey", 49.4657, -2.5853),
        ("jersey", 49.2138, -2.1358),
        ("vianden", 49.935, 6.208),
        ("clos lucé", 47.4103, 0.9922),
        ("clos luce", 47.4103, 0.9922),
        ("amboise", 47.4131, 0.9827),
        ("vinci", 43.7869, 10.9237),
        ("cesena", 44.1391, 12.2431),
        ("palos de la frontera", 37.2278, -6.8933),
        ("palos", 37.2278, -6.8933),
        ("san salvador island", 24.077, -74.478),
        ("san salvador", 24.077, -74.478),
        ("hispaniola", 19.0, -70.6667),
        ("santo domingo", 18.4861, -69.9312),
        ("cuba", 23.1136, -82.3666),
        ("jamaica", 18.1096, -77.2975),
        ("valladolid", 41.6523, -4.7245),
        ("barcelona", 41.3851, 2.1734),
        ("canary islands", 28.2916, -16.6291),
        ("madeira", 32.7607, -16.9595),
        ("lisbon", 38.7223, -9.1393),
        ("stockholm", 59.3293, 18.0686),
        ("sceaux", 48.778, 2.295),
        ("passy", 48.8575, 2.2764),
        ("sorbonne", 48.849, 2.343),
        ("maida vale", 51.5274, -0.1899),
        ("sherborne", 50.949, -2.518),
        ("princeton", 40.3573, -74.6672),
        ("manchester", 53.4808, -2.2426),
        ("wilmslow", 53.328, -2.232),
        ("hampton", 51.415, -0.367),
        ("bletchley", 51.9973, -0.7406),
        ("tarsus", 36.9165, 34.8951),
        ("actium", 38.933, 20.733),
        ("pelusium", 31.042, 32.548),
        ("antioch", 36.202, 36.16),
        ("thebes", 25.7206, 32.6105),
        ("philae", 24.025, 32.884),
        ("cyrene", 32.825, 21.858),
        ("kraków", 50.0647, 19.945),
        ("krakow", 50.0647, 19.945),
        ("cracow", 50.0647, 19.945),
        ("zakopane", 49.2992, 19.9496),
        ("oslo", 59.9139, 10.7522),
        ("cádiz", 36.5271, -6.2886),
        ("cadiz", 36.5271, -6.2886),
        ("seville", 37.3891, -5.9845),
        ("sevilla", 37.3891, -5.9845),
        ("la gomera", 28.0916, -17.1133),
        ("gomera", 28.0916, -17.1133),
        ("la navidad", 19.75, -72.0),
        ("azores", 37.7412, -25.6756),
        ("anchiano", 43.802, 10.936),
        ("panthéon", 48.8462, 2.3464),
        ("pantheon", 48.8462, 2.3464),
        ("institut curie", 48.844, 2.345),
        ("espci", 48.8413, 2.3478),
        ("collège de france", 48.8493, 2.3456),
        ("college de france", 48.8493, 2.3456),
        ("place des vosges", 48.8556, 2.3655),
        ("école normale", 48.8417, 2.3445),
        ("ecole normale", 48.8417, 2.3445),
        ("invalides", 48.8583, 2.3126),
        ("vatican", 41.9029, 12.4534),
        ("pisa", 43.7228, 10.4017),
        ("bologna", 44.4949, 11.3426),
        ("urbino", 43.7262, 12.6366),
        ("padua", 45.4064, 11.8768),
        ("siena", 43.3188, 11.3308),
        ("ferrara", 44.8378, 11.6196),
        ("granada", 37.1773, -3.5986),
        ("córdoba", 37.8882, -4.7794),
        ("cordoba", 37.8882, -4.7794),
        ("toledo", 39.8628, -4.0273),
        ("bordeaux", 44.8378, -0.5792),
        ("nantes", 47.2184, -1.5536),
        ("dijon", 47.322, 5.0415),
        ("laon", 49.564, 3.62),
        ("reims", 49.2583, 4.0317),
        ("trafalgar", 36.043, -6.287),
        ("aboukir", 31.316, 30.061),
        ("gibraltar", 36.1408, -5.3536),
        ("hut 8", 51.9973, -0.7406),
        ("teddington", 51.427, -0.332),
        ("trinidad", 10.6918, -61.2225),
        ("puerto rico", 18.2208, -66.5901),
        ("venezuela", 10.4806, -66.9036),
        ("panama", 8.9824, -79.5199),
        ("cyprus", 35.1856, 33.3823),
        ("rhodes", 36.4349, 28.2176),
        ("athens", 37.9838, 23.7275),
        ("memphis", 29.8496, 31.2535),
];

fn gazetteer_lookup(key: &str) -> Option<(Option<f64>, Option<f64>)> {
    GAZETTEER
        .iter()
        .find(|(name, _, _)| *name == key)
        .map(|(_, lat, lon)| (Some(*lat), Some(*lon)))
}

/// Longest gazetteer hit in free text (word-boundary aware).
pub fn find_place_in_text(text: &str) -> Option<String> {
    let lower = text.to_ascii_lowercase();
    let bytes = lower.as_bytes();
    let mut best: Option<(&str, usize)> = None;
    for (name, _, _) in GAZETTEER {
        let Some(idx) = lower.find(*name) else {
            continue;
        };
        let before_ok = idx == 0 || !bytes[idx - 1].is_ascii_alphanumeric();
        let end = idx + name.len();
        let after_ok = end >= bytes.len() || !bytes[end].is_ascii_alphanumeric();
        if !before_ok || !after_ok {
            continue;
        }
        if name.len() > best.map(|(_, n)| n).unwrap_or(0) {
            best = Some((*name, name.len()));
        }
    }
    best.map(|(name, _)| title_case_place(name))
}

fn title_case_place(place: &str) -> String {
    let sep = if place.contains('-') { '-' } else { ' ' };
    place
        .split(['-', ' '])
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(&sep.to_string())
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

    #[test]
    fn finds_longest_place_in_prose() {
        let hit = find_place_in_text("She worked in a shed in Paris in 1898.").unwrap();
        assert_eq!(hit.to_lowercase(), "paris");
        let island = find_place_in_text("He landed at San Salvador in 1492.").unwrap();
        assert!(island.to_lowercase().contains("san salvador"));
        let krakow = find_place_in_text("She left Kraków in 1891.").unwrap();
        assert!(krakow.to_lowercase().contains("krak"));
    }
}
