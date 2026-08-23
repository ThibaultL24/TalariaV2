// crates/talaria-sources/src/places.rs
//! ResolvePlaces — label/alias/QID → coordinates.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaceResolution {
    pub label: String,
    pub method: String,
    pub wikidata_qid: Option<String>,
    pub lat: f64,
    pub lon: f64,
    pub precision: String,
    pub uncertainty_radius_m: Option<f64>,
    pub score: f32,
}

fn lookup_alias(key: &str) -> Option<PlaceResolution> {
    const ALIASES: &[(&str, f64, f64, &str)] = &[
        ("ajaccio", 41.9267, 8.7369, "exact"),
        ("paris", 48.8566, 2.3522, "exact"),
        ("waterloo", 50.6794, 4.4047, "exact"),
        ("austerlitz", 49.1533, 16.875, "exact"),
        ("slavkov u brna", 49.1533, 16.875, "exact"),
        ("waterloo battlefield", 50.6794, 4.4047, "exact"),
        ("leipzig", 51.3397, 12.3731, "exact"),
        ("borodino", 55.526, 35.821, "exact"),
        ("moscow", 55.7558, 37.6173, "exact"),
        ("elba", 42.777, 10.192, "exact"),
        ("saint helena", -15.965, -5.712, "exact"),
        ("st helena", -15.965, -5.712, "exact"),
        ("sainte-hélène", -15.965, -5.712, "exact"),
        ("fontainebleau", 48.4047, 2.7016, "exact"),
        ("malmaison", 48.8706, 2.1681, "exact"),
        ("toulon", 43.1242, 5.928, "exact"),
        ("brienne", 48.3933, 4.5228, "exact"),
        ("marengo", 44.888, 8.679, "exact"),
        ("jena", 50.9272, 11.586, "exact"),
        ("wagram", 48.25, 16.5667, "exact"),
        ("friedland", 54.443, 21.011, "exact"),
        ("tilsit", 55.0833, 21.8833, "exact"),
        ("cairo", 30.0444, 31.2357, "exact"),
        ("egypt", 30.0444, 31.2357, "centroid"),
        ("vienna", 48.2082, 16.3738, "exact"),
        ("schönbrunn", 48.1845, 16.3122, "exact"),
        ("madrid", 40.4168, -3.7038, "exact"),
        ("berlin", 52.52, 13.405, "exact"),
        ("milan", 45.4642, 9.19, "exact"),
        ("rome", 41.9028, 12.4964, "exact"),
        ("ulm", 48.4011, 9.9876, "exact"),
        ("eylau", 54.4, 20.6333, "exact"),
        ("smolensk", 54.7826, 32.0853, "exact"),
        ("dresden", 51.0504, 13.7373, "exact"),
        ("lüetzen", 51.2583, 12.1417, "exact"),
        ("lutzen", 51.2583, 12.1417, "exact"),
        ("bautzen", 51.1803, 14.4347, "exact"),
        ("ligny", 50.512, 4.574, "exact"),
        ("quatre bras", 50.571, 4.638, "exact"),
        ("arcole", 45.358, 11.278, "exact"),
        ("rivoli", 45.571, 10.837, "exact"),
        ("lodi", 45.314, 9.503, "exact"),
        ("mantua", 45.1564, 10.7914, "exact"),
        ("acre", 32.926, 35.083, "exact"),
        ("aboukir", 31.3167, 30.0667, "exact"),
        ("amiens", 49.8941, 2.2958, "exact"),
        ("lille", 50.6292, 3.0573, "exact"),
        ("honfleur", 49.4197, 0.2328, "exact"),
        ("colombey", 48.217, 4.883, "exact"),
        ("colombey-les-deux-eglises", 48.217, 4.883, "exact"),
        ("colombey-les-deux-églises", 48.217, 4.883, "exact"),
        ("strasbourg", 48.5734, 7.7521, "exact"),
        ("erfurt", 50.9787, 11.0328, "exact"),
        ("bayonne", 43.4929, -1.4748, "exact"),
        ("vitoria", 42.8499, -2.6729, "exact"),
        ("cannes", 43.5528, 7.0174, "exact"),
        ("grenoble", 45.1885, 5.7245, "exact"),
        ("lyon", 45.764, 4.8357, "exact"),
        ("auxerre", 47.7982, 3.5733, "exact"),
        ("boulogne", 50.7264, 1.6147, "exact"),
        ("corsica", 42.0396, 9.0129, "centroid"),
        ("trafalgar", 36.183, -6.0, "approximate"),
        ("copenhagen", 55.6761, 12.5683, "exact"),
        ("valmy", 49.0667, 4.7667, "exact"),
        ("fleurus", 50.4833, 4.55, "exact"),
        ("zurich", 47.3769, 8.5417, "exact"),
        ("hohenlinden", 48.15, 11.9833, "exact"),
        ("jena–auerstedt", 50.9272, 11.586, "exact"),
        ("jena-auerstedt", 50.9272, 11.586, "exact"),
        ("aspern-essling", 48.2167, 16.4667, "exact"),
        ("aspern–essling", 48.2167, 16.4667, "exact"),
        ("berezina", 54.4833, 28.3167, "approximate"),
        ("salamanca", 40.97, -5.663, "exact"),
        ("talavera", 39.963, -4.830, "exact"),
        ("albuera", 38.7167, -6.8167, "exact"),
        ("busaco", 40.3333, -8.3333, "exact"),
        ("bussaco", 40.3333, -8.3333, "exact"),
        ("fuentes de oñoro", 40.5833, -6.8167, "exact"),
        ("ciudad rodrigo", 40.6, -6.5333, "exact"),
        ("badajoz", 38.879, -6.97, "exact"),
        ("zaragoza", 41.6488, -0.8891, "exact"),
        ("saragossa", 41.6488, -0.8891, "exact"),
        ("gerona", 41.9794, 2.8214, "exact"),
        ("girona", 41.9794, 2.8214, "exact"),
        ("toulouse", 43.6047, 1.4442, "exact"),
        ("orthez", 43.488, -0.772, "exact"),
        ("nive", 43.4, -1.45, "approximate"),
        ("nivelle", 43.35, -1.6, "approximate"),
        ("san sebastián", 43.3183, -1.9812, "exact"),
        ("san sebastian", 43.3183, -1.9812, "exact"),
        ("pymont", 50.3755, -4.1427, "exact"),
        ("plymouth", 50.3755, -4.1427, "exact"),
        ("rochefort", 45.942, -0.9588, "exact"),
        ("longwood", -15.95, -5.35, "exact"),
        ("ajaccio", 41.9267, 8.7369, "exact"),
        ("notre-dame", 48.853, 2.3499, "exact"),
        ("tuileries", 48.863, 2.327, "exact"),
        ("invalides", 48.856, 2.312, "exact"),
        ("montenotte", 44.3667, 8.3, "exact"),
        ("millesimo", 44.3667, 8.2, "exact"),
        ("dego", 44.4167, 8.3167, "exact"),
        ("mondovì", 44.3833, 7.8167, "exact"),
        ("mondovi", 44.3833, 7.8167, "exact"),
        ("castiglione", 45.3833, 10.4833, "exact"),
        ("bassano", 45.7667, 11.7333, "exact"),
        ("caldiero", 45.4167, 11.1833, "exact"),
        ("novi", 44.7667, 8.7833, "exact"),
        ("trebbia", 45.0, 9.65, "approximate"),
        ("cassano", 45.5167, 9.5167, "exact"),
        ("magnano", 45.35, 11.0667, "exact"),
        ("pozzolo", 45.2, 10.8, "exact"),
        ("montebello", 44.9833, 9.0, "exact"),
        ("raab", 47.6833, 17.6333, "exact"),
        ("győr", 47.6833, 17.6333, "exact"),
        ("znaim", 48.855, 16.049, "exact"),
        ("znojmo", 48.855, 16.049, "exact"),
        ("hanau", 50.1333, 8.9167, "exact"),
        ("kulm", 50.7, 13.9, "approximate"),
        ("champaubert", 48.8833, 3.7667, "exact"),
        ("montmirail", 48.8667, 3.5333, "exact"),
        ("vauchamps", 48.8667, 3.6167, "exact"),
        ("craonne", 49.4333, 3.7167, "exact"),
        ("laon", 49.5667, 3.6167, "exact"),
        ("arcis-sur-aube", 48.5333, 4.1333, "exact"),
        ("brienne", 48.3933, 4.5228, "exact"),
        ("la rothière", 48.35, 4.55, "exact"),
        ("fère-champenoise", 48.75, 4.0, "exact"),
        ("reims", 49.2583, 4.0317, "exact"),
        ("wavre", 50.7167, 4.6, "exact"),
        ("valençay", 47.15, 1.55, "exact"),
        ("valence", 44.9334, 4.8924, "exact"),
        ("compiègne", 49.417, 2.826, "exact"),
        ("saint-cloud", 48.845, 2.216, "exact"),
        ("rambouillet", 48.644, 1.83, "exact"),
        ("versailles", 48.8049, 2.1204, "exact"),
        ("bladensburg", 38.939, -76.934, "exact"),
        ("halifax", 44.6488, -63.5752, "exact"),
        ("raszyn", 52.158, 20.926, "exact"),
        ("lyngør", 58.633, 9.133, "exact"),
        ("lyngor", 58.633, 9.133, "exact"),
        ("suvodol", 43.15, 21.35, "approximate"),
        ("castalla", 38.596, -0.671, "exact"),
        ("saorgio", 43.983, 7.55, "exact"),
        ("tarvis", 46.505, 13.578, "exact"),
        ("tarvisio", 46.505, 13.578, "exact"),
        ("la suffel", 48.65, 7.75, "approximate"),
        ("issy", 48.824, 2.273, "exact"),
        ("portoferraio", 42.813, 10.315, "exact"),
        ("ormea", 44.15, 7.916, "exact"),
        ("hoenheim", 48.622, 7.755, "exact"),
        ("souffelweyersheim", 48.635, 7.74, "exact"),
        ("catalonia", 41.591, 1.52, "centroid"),
        ("belgium", 50.5039, 4.4699, "centroid"),
        ("vitebsk", 55.1904, 30.2049, "exact"),
        ("abukir", 31.3167, 30.0667, "exact"),
        ("abu qir", 31.3167, 30.0667, "exact"),
        ("ostroleka", 53.083, 21.566, "exact"),
        ("ostrołęka", 53.083, 21.566, "exact"),
        ("lubeck", 53.8655, 10.6866, "exact"),
        ("lübeck", 53.8655, 10.6866, "exact"),
        ("corunna", 43.3623, -8.4115, "exact"),
        ("a coruña", 43.3623, -8.4115, "exact"),
        ("vimeiro", 39.178, -9.318, "exact"),
        ("toulon", 43.1242, 5.928, "exact"),
        ("warsaw", 52.2297, 21.0122, "exact"),
        ("stockholm", 59.3293, 18.0686, "exact"),
        ("besançon", 47.2378, 6.0241, "exact"),
        ("besancon", 47.2378, 6.0241, "exact"),
        ("guernsey", 49.4657, -2.5853, "centroid"),
        ("jersey", 49.2138, -2.1358, "centroid"),
        ("vinci", 43.7869, 10.9237, "exact"),
        ("amboise", 47.4131, 0.9827, "exact"),
        ("palos", 37.2278, -6.8933, "exact"),
        ("san salvador", 24.077, -74.478, "approximate"),
        ("hispaniola", 19.0, -70.6667, "centroid"),
        ("valladolid", 41.6523, -4.7245, "exact"),
        ("barcelona", 41.3851, 2.1734, "exact"),
        ("manchester", 53.4808, -2.2426, "exact"),
        ("wilmslow", 53.328, -2.232, "exact"),
        ("princeton", 40.3573, -74.6672, "exact"),
        ("bletchley", 51.9973, -0.7406, "exact"),
        ("actium", 38.933, 20.733, "approximate"),
        ("tarsus", 36.9165, 34.8951, "exact"),
        ("alexandria", 31.2001, 29.9187, "exact"),
        ("sceaux", 48.778, 2.295, "exact"),
        ("passy", 48.8575, 2.2764, "exact"),
        ("venice", 45.4408, 12.3155, "exact"),
        ("venise", 45.4408, 12.3155, "exact"),
        ("venedig", 45.4408, 12.3155, "exact"),
        ("genoa", 44.4056, 8.9463, "exact"),
        ("gênes", 44.4056, 8.9463, "exact"),
        ("marseille", 43.2965, 5.3698, "exact"),
        ("majorque", 39.6953, 3.0176, "centroid"),
        ("mallorca", 39.6953, 3.0176, "centroid"),
        ("nohant", 46.625, 1.96, "exact"),
        ("nohant-vic", 46.625, 1.96, "exact"),
        ("place des vosges", 48.8556, 2.3656, "exact"),
        ("varsovie", 52.2297, 21.0122, "exact"),
        ("warszawa", 52.2297, 21.0122, "exact"),
        ("perpignan", 42.6887, 2.8948, "exact"),
        ("cauterets", 42.889, -0.115, "exact"),
        ("valldemossa", 39.711, 2.622, "exact"),
        ("valdemosa", 39.711, 2.622, "exact"),
        ("florence", 43.7696, 11.2558, "exact"),
        ("firenze", 43.7696, 11.2558, "exact"),
        ("naples", 40.8518, 14.2681, "exact"),
        ("napoli", 40.8518, 14.2681, "exact"),
        ("edimbourg", 55.9533, -3.1883, "exact"),
        ("edinburgh", 55.9533, -3.1883, "exact"),
        ("cracovie", 50.0647, 19.945, "exact"),
        ("krakow", 50.0647, 19.945, "exact"),
        ("bruges", 51.2093, 3.2247, "exact"),
        ("avignon", 43.9493, 4.8055, "exact"),
        ("genève", 46.2044, 6.1432, "exact"),
        ("geneve", 46.2044, 6.1432, "exact"),
        ("geneva", 46.2044, 6.1432, "exact"),
        ("chamonix", 45.9237, 6.8694, "exact"),
        ("martigny", 46.1028, 7.0736, "exact"),
        ("fribourg", 46.8065, 7.1617, "exact"),
        ("palma", 39.5696, 2.6502, "exact"),
        ("barcelone", 41.3851, 2.1734, "exact"),
        ("gargilesse", 46.4, 1.6, "exact"),
        ("palaiseau", 48.7145, 2.2457, "exact"),
        ("la châtre", 46.582, 1.987, "exact"),
        ("la chatre", 46.582, 1.987, "exact"),
        ("rue meslay", 48.868, 2.36, "exact"),
        ("quai malaquais", 48.858, 2.335, "exact"),
        ("quai saint-michel", 48.853, 2.347, "exact"),
        ("pigalle", 48.882, 2.337, "exact"),
        ("hôtel danieli", 45.434, 12.342, "exact"),
        ("hotel danieli", 45.434, 12.342, "exact"),
        ("danieli", 45.434, 12.342, "exact"),
        ("square d'orléans", 48.877, 2.331, "exact"),
        ("square d'orleans", 48.877, 2.331, "exact"),
    ];
    for (alias, lat, lon, precision) in ALIASES {
        if *alias == key || key.contains(alias) {
            return Some(PlaceResolution {
                label: key.to_string(),
                method: "alias_gazetteer".into(),
                wikidata_qid: None,
                lat: *lat,
                lon: *lon,
                precision: (*precision).into(),
                uncertainty_radius_m: if *precision == "exact" {
                    Some(500.0)
                } else {
                    Some(5000.0)
                },
                score: 0.85,
            });
        }
    }
    None
}

/// Offline alias table (mirrors DB seed) for tests and fast path.
pub fn resolve_place_offline(label: &str) -> Option<PlaceResolution> {
    let raw = label.trim();
    if raw.is_empty() {
        return None;
    }
    let key = raw.to_lowercase();
    if let Some(mut r) = lookup_alias(&key) {
        r.label = raw.to_string();
        return Some(r);
    }
    // Strip parenthetical qualifiers: "Mantua (1796–1797)" → "Mantua"
    if let Some(base) = key.split('(').next().map(str::trim).filter(|s| !s.is_empty()) {
        if base != key {
            if let Some(mut r) = lookup_alias(base) {
                r.label = raw.to_string();
                return Some(r);
            }
        }
    }
    // Compound battle locales: "Jena–Auerstedt" / "Aspern-Essling" → try left then right
    for sep in ['–', '-', '/', ','] {
        if let Some((left, right)) = key.split_once(sep) {
            let left = left.trim();
            let right = right.trim();
            if let Some(mut r) = lookup_alias(left) {
                r.label = raw.to_string();
                return Some(r);
            }
            if let Some(mut r) = lookup_alias(right) {
                r.label = raw.to_string();
                return Some(r);
            }
        }
    }
    None
}

/// Extract place hint from "Battle of X" / "Siege of X" titles.
pub fn place_hint_from_title(title: &str) -> Option<String> {
    let lower = title.to_lowercase();
    for prefix in [
        "battle of ",
        "siege of ",
        "treaty of ",
        "treaties of ",
        "bataille de ",
        "siège de ",
        "traite de ",
        "traité de ",
    ] {
        if let Some(rest) = lower.strip_prefix(prefix) {
            let place = title[title.len() - rest.len()..]
                .split('(')
                .next()?
                .trim()
                .to_string();
            if !place.is_empty() {
                return Some(place);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_austerlitz() {
        let r = resolve_place_offline("Austerlitz").unwrap();
        assert!((r.lat - 49.1533).abs() < 0.01);
        assert!((r.lon - 16.875).abs() < 0.01);
        // GeoJSON order reminder: lon, lat
        assert!(r.lon < r.lat || r.lat < 60.0);
    }

    #[test]
    fn battle_title_place_hint() {
        assert_eq!(
            place_hint_from_title("Battle of Waterloo").as_deref(),
            Some("Waterloo")
        );
    }
}
