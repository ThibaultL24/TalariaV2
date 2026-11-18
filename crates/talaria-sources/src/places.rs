// crates/talaria-sources/src/places.rs
//! ResolvePlaces — label/alias/QID → coordinates.
//!
//! This module provides offline place resolution via a gazetteer loaded from
//! `fixtures/gazetteer/historical_places.json`. The gazetteer includes:
//! - Coordinates (lat/lon) for map placement
//! - Wikidata QIDs for identity resolution (new in this expansion)
//! - Precision indicators (exact, approximate, centroid)
//!
//! The QID field enables `place_identity_qid` population without HTTP lookups,
//! significantly improving identity resolution fill rates.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::OnceLock;

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

#[derive(Debug, Clone, Deserialize)]
struct GazetteerEntry {
    label: String,
    lat: f64,
    lon: f64,
    precision: String,
    #[serde(default)]
    qid: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct GazetteerFile {
    #[allow(dead_code)]
    version: String,
    #[allow(dead_code)]
    description: String,
    places: Vec<GazetteerEntry>,
}

static GAZETTEER_ENTRIES: OnceLock<Vec<GazetteerEntry>> = OnceLock::new();
static GAZETTEER_MAP: OnceLock<HashMap<String, GazetteerEntry>> = OnceLock::new();

fn all_gazetteer_entries() -> &'static [GazetteerEntry] {
    GAZETTEER_ENTRIES
        .get_or_init(|| {
            let json_str = include_str!("../../../fixtures/gazetteer/historical_places.json");
            match serde_json::from_str::<GazetteerFile>(json_str) {
                Ok(file) => file.places,
                Err(error) => {
                    tracing::error!("Failed to parse gazetteer JSON: {}", error);
                    Vec::new()
                }
            }
        })
        .as_slice()
}

fn get_gazetteer_map() -> &'static HashMap<String, GazetteerEntry> {
    GAZETTEER_MAP.get_or_init(|| {
        let mut map = HashMap::with_capacity(all_gazetteer_entries().len());
        for entry in all_gazetteer_entries() {
            map.insert(entry.label.clone(), entry.clone());
        }
        map
    })
}

/// One gazetteer row. A label can occur more than once when two places share it.
#[derive(Debug, Clone, PartialEq)]
pub struct GazetteerIdentity {
    pub qid: Option<String>,
    pub label: String,
    pub lat: f64,
    pub lon: f64,
    pub precision: String,
    pub uncertainty_radius_m: Option<f64>,
}

/// Exact label match. A longer title that merely contains the name is not a match.
pub fn gazetteer_exact_identities(label: &str) -> Vec<GazetteerIdentity> {
    let key = fold_place_label(label);
    if key.is_empty() {
        return Vec::new();
    }
    all_gazetteer_entries()
        .iter()
        .filter(|entry| fold_place_label(&entry.label) == key)
        .map(gazetteer_identity)
        .collect()
}

/// Every gazetteer row that carries this QID. The name is not consulted.
pub fn gazetteer_identities_for_qid(qid: &str) -> Vec<GazetteerIdentity> {
    let wanted = canonical_qid(qid);
    if wanted.is_empty() {
        return Vec::new();
    }
    all_gazetteer_entries()
        .iter()
        .filter(|entry| entry.qid.as_deref().map(canonical_qid).as_deref() == Some(wanted.as_str()))
        .map(gazetteer_identity)
        .collect()
}

fn gazetteer_identity(entry: &GazetteerEntry) -> GazetteerIdentity {
    let precision = entry.precision.to_ascii_lowercase();
    GazetteerIdentity {
        qid: entry.qid.as_deref().map(canonical_qid),
        label: entry.label.clone(),
        lat: entry.lat,
        lon: entry.lon,
        precision: precision.clone(),
        uncertainty_radius_m: match precision.as_str() {
            "exact" | "point" => Some(500.0),
            "approximate" | "city" => Some(5_000.0),
            _ => None,
        },
    }
}

fn canonical_qid(value: &str) -> String {
    let bare = value.trim().trim_start_matches(['q', 'Q']);
    if bare.is_empty() {
        String::new()
    } else {
        format!("Q{bare}")
    }
}

fn fold_place_label(value: &str) -> String {
    value
        .trim()
        .chars()
        .map(|character| match character {
            'à' | 'á' | 'â' | 'ä' => 'a',
            'è' | 'é' | 'ê' | 'ë' => 'e',
            'ì' | 'í' | 'î' | 'ï' => 'i',
            'ò' | 'ó' | 'ô' | 'ö' => 'o',
            'ù' | 'ú' | 'û' | 'ü' => 'u',
            'ç' => 'c',
            'œ' => 'o',
            other => other,
        })
        .collect::<String>()
        .to_ascii_lowercase()
}

fn lookup_alias(key: &str) -> Option<PlaceResolution> {
    let gazetteer = get_gazetteer_map();

    if let Some(entry) = gazetteer.get(key) {
        return Some(PlaceResolution {
            label: key.to_string(),
            method: "alias_gazetteer".into(),
            wikidata_qid: entry.qid.clone(),
            lat: entry.lat,
            lon: entry.lon,
            precision: entry.precision.clone(),
            uncertainty_radius_m: if entry.precision == "exact" {
                Some(500.0)
            } else {
                Some(5000.0)
            },
            score: if entry.qid.is_some() { 0.90 } else { 0.85 },
        });
    }

    let mut best: Option<&GazetteerEntry> = None;
    for (alias, entry) in gazetteer.iter() {
        if key.contains(alias.as_str()) {
            if best.map_or(true, |prev| alias.len() > prev.label.len()) {
                best = Some(entry);
            }
        }
    }

    best.map(|entry| PlaceResolution {
        label: key.to_string(),
        method: "alias_gazetteer".into(),
        wikidata_qid: entry.qid.clone(),
        lat: entry.lat,
        lon: entry.lon,
        precision: entry.precision.clone(),
        uncertainty_radius_m: if entry.precision == "exact" {
            Some(500.0)
        } else {
            Some(5000.0)
        },
        score: if entry.qid.is_some() { 0.90 } else { 0.85 },
    })
}

/// Count of entries in the offline gazetteer.
pub fn gazetteer_entry_count() -> usize {
    get_gazetteer_map().len()
}

/// Count of entries in the offline gazetteer that have a Wikidata QID.
pub fn gazetteer_qid_count() -> usize {
    get_gazetteer_map()
        .values()
        .filter(|e| e.qid.is_some())
        .count()
}

// Legacy const array preserved for reference; actual lookups use the JSON gazetteer above.
#[allow(dead_code)]
const LEGACY_ALIASES: &[(&str, f64, f64, &str)] = &[
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
    ("douai", 50.3708, 3.0799, "exact"),
    ("bruxelles", 50.8503, 4.3517, "exact"),
    ("brussels", 50.8503, 4.3517, "exact"),
    ("stagira", 40.5917, 23.7944, "approximate"),
    ("stagire", 40.5917, 23.7944, "approximate"),
    ("stageira", 40.5917, 23.7944, "approximate"),
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
    ("london", 51.5074, -0.1278, "exact"),
    ("londres", 51.5074, -0.1278, "exact"),
    ("chicago", 41.8781, -87.6298, "exact"),
    ("montreal", 45.5017, -73.5673, "exact"),
    ("montréal", 45.5017, -73.5673, "exact"),
    ("beirut", 33.8938, 35.5018, "exact"),
    ("beyrouth", 33.8938, 35.5018, "exact"),
    ("ho chi minh", 10.8231, 106.6297, "exact"),
    ("saigon", 10.8231, 106.6297, "exact"),
    ("eiffel tower", 48.8584, 2.2945, "exact"),
    ("tour eiffel", 48.8584, 2.2945, "exact"),
    ("rue dauphine", 48.8554, 2.3395, "exact"),
    ("royal institution", 51.5098, -0.1325, "exact"),
    ("sancellemoz", 45.923, 6.710, "exact"),
    ("haute-savoie", 45.9, 6.15, "centroid"),
    ("sorbonne", 48.8493, 2.3459, "exact"),
    ("institut du radium", 48.8447, 2.345, "exact"),
    ("radium institute", 48.8447, 2.345, "exact"),
    ("école normale supérieure", 48.8417, 2.3444, "exact"),
    ("ecole normale superieure", 48.8417, 2.3444, "exact"),
    ("panthéon", 48.8462, 2.3461, "exact"),
    ("pantheon", 48.8462, 2.3461, "exact"),
    ("new york", 40.7128, -74.006, "exact"),
    ("philadelphia", 39.9526, -75.1652, "exact"),
    ("washington", 38.9072, -77.0369, "exact"),
];

pub fn place_query_variants(label: &str) -> Vec<String> {
    let mut out = Vec::new();
    let push = |out: &mut Vec<String>, s: &str| {
        let t = s.trim().to_lowercase();
        if t.len() >= 3 && !out.iter().any(|x| x == &t) {
            out.push(t);
        }
    };
    push(&mut out, label);
    let low = label.trim().to_lowercase();
    let stripped = ["the ", "a ", "an ", "le ", "la ", "les ", "el "]
        .iter()
        .find_map(|p| low.strip_prefix(p))
        .unwrap_or(low.as_str())
        .trim();
    push(&mut out, stripped);
    for part in stripped.split([',', ';']) {
        push(&mut out, part);
    }
    for sep in [" in ", " at ", " à ", " en "] {
        if let Some((_, right)) = stripped.rsplit_once(sep) {
            push(&mut out, right);
        }
    }
    out
}

/// Offline alias table (mirrors DB seed) for tests and fast path.
pub fn resolve_place_offline(label: &str) -> Option<PlaceResolution> {
    let raw = label.trim();
    if raw.is_empty() {
        return None;
    }
    for variant in place_query_variants(raw) {
        if let Some(mut r) = lookup_alias(&variant) {
            r.label = raw.to_string();
            return Some(r);
        }
    }
    let key = raw.to_lowercase();
    // Strip parenthetical qualifiers: "Mantua (1796–1797)" → "Mantua"
    if let Some(base) = key
        .split('(')
        .next()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
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

/// Locality heuristic for a new ingestion. It is not historical proof,
/// and distance alone must not correct an event already published.
const SAME_LOCALITY_M: f64 = 20_000.0;

/// Coordinates for a new event. An uncertain name match does not receive a point.
#[derive(Debug, Clone, PartialEq)]
pub struct EventPlacePoint {
    pub lat: Option<f64>,
    pub lon: Option<f64>,
    pub qid: Option<String>,
    pub reason: &'static str,
}

pub fn event_place_point(role: &str, label: &str, given: Option<(f64, f64)>) -> EventPlacePoint {
    if talaria_quality::is_political_territory(label) {
        return EventPlacePoint {
            lat: None,
            lon: None,
            qid: None,
            reason: "political_territory_is_not_a_point",
        };
    }
    decide_event_place(role, &gazetteer_exact_identities(label), given)
}

fn decide_event_place(
    role: &str,
    identities: &[GazetteerIdentity],
    given: Option<(f64, f64)>,
) -> EventPlacePoint {
    if role != "locus" {
        return EventPlacePoint {
            lat: None,
            lon: None,
            qid: None,
            reason: "place_is_not_the_event_locus",
        };
    }
    let mut exact: Vec<&GazetteerIdentity> = identities
        .iter()
        .filter(|identity| identity.precision == "exact" || identity.precision == "point")
        .collect();
    exact.sort_by(|left, right| left.qid.cmp(&right.qid));
    exact.dedup_by(|left, right| left.qid == right.qid);
    if exact.len() > 1 {
        return EventPlacePoint {
            lat: None,
            lon: None,
            qid: None,
            reason: "place_identity_ambiguous",
        };
    }
    if let Some(identity) = exact.first() {
        if let Some((lat, lon)) = given {
            if distance_m(lat, lon, identity.lat, identity.lon) <= SAME_LOCALITY_M {
                return EventPlacePoint {
                    lat: Some(lat),
                    lon: Some(lon),
                    qid: identity.qid.clone(),
                    reason: "given_point_matches_identity",
                };
            }
        }
        return EventPlacePoint {
            lat: Some(identity.lat),
            lon: Some(identity.lon),
            qid: identity.qid.clone(),
            reason: "unique_exact_identity",
        };
    }
    let cities: Vec<&GazetteerIdentity> = identities
        .iter()
        .filter(|identity| identity.precision == "city")
        .collect();
    if cities.len() == 1 {
        let identity = cities[0];
        return EventPlacePoint {
            lat: Some(identity.lat),
            lon: Some(identity.lon),
            qid: identity.qid.clone(),
            reason: "named_city",
        };
    }
    if identities
        .iter()
        .any(|identity| identity.precision == "approximate" || identity.precision == "centroid")
    {
        return EventPlacePoint {
            lat: None,
            lon: None,
            qid: None,
            reason: "identity_precision_insufficient",
        };
    }
    if let Some((lat, lon)) = given {
        return EventPlacePoint {
            lat: Some(lat),
            lon: Some(lon),
            qid: None,
            reason: "page_coordinates_without_gazetteer",
        };
    }
    EventPlacePoint {
        lat: None,
        lon: None,
        qid: None,
        reason: "no_reliable_place_identity",
    }
}

fn distance_m(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    let radius = 6_371_000.0;
    let p1 = lat1.to_radians();
    let p2 = lat2.to_radians();
    let dlat = (lat2 - lat1).to_radians();
    let dlon = (lon2 - lon1).to_radians();
    let h = (dlat / 2.0).sin().powi(2) + p1.cos() * p2.cos() * (dlon / 2.0).sin().powi(2);
    2.0 * radius * h.sqrt().asin()
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
        "bataille d'",
        "siège de ",
        "siège d'",
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

    #[test]
    fn an_exact_label_keeps_its_qid_and_radius() {
        let valmy = gazetteer_exact_identities("Valmy");
        assert_eq!(valmy.len(), 1);
        assert_eq!(valmy[0].qid.as_deref(), Some("Q649178"));
        assert_eq!(valmy[0].uncertainty_radius_m, Some(500.0));
        assert!(gazetteer_exact_identities("Battle of Valmy").is_empty());
        assert_eq!(
            gazetteer_identities_for_qid("q649178")[0].precision,
            "exact"
        );
    }

    #[test]
    fn a_distant_homonym_uses_the_exact_identity_and_a_short_gap_stays() {
        let mondovi = event_place_point("locus", "Mondovì", Some((59.60, 29.67)));
        assert_eq!(mondovi.qid.as_deref(), Some("Q50775"));
        assert!((mondovi.lat.unwrap() - 44.3833).abs() < 0.01);
        let friedland = event_place_point("locus", "Friedland", Some((41.94, 2.81)));
        assert_eq!(friedland.qid.as_deref(), Some("Q13431"));
        assert!(friedland.lat.unwrap() > 50.0);
        let eylau = event_place_point("locus", "Eylau", Some((42.03, 2.74)));
        assert_eq!(eylau.qid.as_deref(), Some("Q13422"));
        assert!(eylau.lat.unwrap() > 50.0);
        let vitoria = event_place_point("locus", "Vitoria", Some((51.29, 13.53)));
        assert_eq!(vitoria.qid.as_deref(), Some("Q8714"));
        assert!(vitoria.lon.unwrap() < 0.0);
        let ratisbon = event_place_point("locus", "Ratisbon", Some((49.02, 12.09)));
        assert_eq!(ratisbon.reason, "given_point_matches_identity");
        assert!((ratisbon.lat.unwrap() - 49.02).abs() < 0.001);
        let salamanca = event_place_point("locus", "Salamanca", Some((40.965, -5.664)));
        assert_eq!(salamanca.reason, "given_point_matches_identity");
        let title = event_place_point("honorific", "Valmy", Some((-33.45, -70.66)));
        assert!(title.lat.is_none());
        assert_eq!(title.reason, "place_is_not_the_event_locus");
        assert!(
            event_place_point("locus", "Battle of Valmy", Some((49.0, 4.7)))
                .qid
                .is_none()
        );
    }

    #[test]
    fn valid_places_are_not_dropped_and_homonyms_stay_unlocated() {
        let absent = event_place_point("locus", "Quarktown", Some((47.19, 5.39)));
        assert_eq!(absent.reason, "page_coordinates_without_gazetteer");
        assert!((absent.lat.unwrap() - 47.19).abs() < 0.001);
        let homonym = decide_event_place(
            "locus",
            &[
                GazetteerIdentity {
                    qid: Some("Q1".into()),
                    label: "Friedland".into(),
                    lat: 54.4,
                    lon: 21.0,
                    precision: "exact".into(),
                    uncertainty_radius_m: Some(500.0),
                },
                GazetteerIdentity {
                    qid: Some("Q2".into()),
                    label: "Friedland".into(),
                    lat: 53.6,
                    lon: 13.5,
                    precision: "exact".into(),
                    uncertainty_radius_m: Some(500.0),
                },
            ],
            Some((54.4, 21.0)),
        );
        assert!(homonym.lat.is_none());
        assert_eq!(homonym.reason, "place_identity_ambiguous");
        let approximate = event_place_point("locus", "Trafalgar", Some((36.1, -6.0)));
        assert!(approximate.lat.is_none());
        assert_eq!(approximate.reason, "identity_precision_insufficient");
        let named = event_place_point("mention", "Lodi", Some((45.314, 9.503)));
        assert!(named.lat.is_none());
        assert_eq!(named.reason, "place_is_not_the_event_locus");
        let douai = event_place_point("locus", "Douai", None);
        assert_eq!(douai.reason, "unique_exact_identity");
        assert!((douai.lat.unwrap() - 50.3708).abs() < 0.01);
        let france = event_place_point("locus", "France", Some((46.2, 2.2)));
        assert!(france.lat.is_none());
        let stagira = event_place_point("locus", "Stagira", None);
        assert_eq!(stagira.reason, "named_city");
        let guernsey = event_place_point("locus", "Guernsey", None);
        assert_eq!(guernsey.reason, "named_city");
    }

    #[test]
    fn the_four_correction_identities_are_unique_and_exact() {
        for (qid, lat, lon) in [
            ("Q50775", 44.3833, 7.8167),
            ("Q13431", 54.443, 21.011),
            ("Q13422", 54.4, 20.6333),
            ("Q6258", 45.314, 9.503),
        ] {
            let rows = gazetteer_identities_for_qid(qid);
            assert!(!rows.is_empty(), "{qid}");
            assert!(rows
                .iter()
                .all(|row| row.precision == "exact" && row.qid.as_deref() == Some(qid)));
            assert!(rows
                .iter()
                .all(|row| (row.lat - lat).abs() < 0.001 && (row.lon - lon).abs() < 0.001));
        }
    }

    #[test]
    fn curie_life_and_commemoration_places_resolve() {
        assert!(resolve_place_offline("London").is_some());
        assert!(resolve_place_offline("Chicago, United States").is_some());
        assert!(resolve_place_offline("Saint-Laurent in Montreal").is_some());
        assert!(resolve_place_offline("Eiffel Tower").is_some());
        assert!(resolve_place_offline("Rue Dauphine").is_some());
        assert!(resolve_place_offline("Royal Institution").is_some());
        let death =
            resolve_place_offline("the Sancellemoz sanatorium in Passy, Haute-Savoie, France")
                .unwrap();
        assert!(
            death.lat > 45.0 && death.lat < 47.0,
            "death must be Haute-Savoie not Paris Passy, got {}",
            death.lat
        );
        assert!(resolve_place_offline("her house").is_none());
        assert!(resolve_place_offline("the institute").is_none());
    }

    #[test]
    fn gazetteer_loaded_from_json() {
        let count = gazetteer_entry_count();
        assert!(
            count > 100,
            "Expected at least 100 gazetteer entries, got {count}"
        );
    }

    #[test]
    fn gazetteer_has_qids() {
        let qid_count = gazetteer_qid_count();
        assert!(
            qid_count > 100,
            "Expected at least 100 entries with QIDs, got {qid_count}"
        );
    }

    #[test]
    fn napoleon_battle_sites_have_qids() {
        let battles = [
            ("Waterloo", "Q134583"),
            ("Austerlitz", "Q153433"),
            ("Borodino", "Q200679"),
            ("Marengo", "Q178181"),
            ("Leipzig", "Q2079"),
            ("Jena", "Q3150"),
            ("Wagram", "Q48828"),
        ];

        for (name, expected_qid) in battles {
            let r = resolve_place_offline(name).unwrap_or_else(|| panic!("{name} should resolve"));
            assert!(
                r.wikidata_qid.is_some(),
                "{name} should have a QID, got None"
            );
            assert_eq!(
                r.wikidata_qid.as_deref(),
                Some(expected_qid),
                "{name} should have QID {expected_qid}"
            );
        }
    }

    #[test]
    fn major_countries_have_qids() {
        let countries = [
            ("France", "Q142"),
            ("Austria", "Q40"),
            ("Russia", "Q159"),
            ("Spain", "Q29"),
            ("Italy", "Q38"),
            ("Germany", "Q183"),
            ("Poland", "Q36"),
        ];

        for (name, expected_qid) in countries {
            let r = resolve_place_offline(name).unwrap_or_else(|| panic!("{name} should resolve"));
            assert_eq!(
                r.wikidata_qid.as_deref(),
                Some(expected_qid),
                "{name} should have QID {expected_qid}"
            );
        }
    }

    #[test]
    fn a_country_name_does_not_become_a_map_point() {
        let point = event_place_point("locus", "France", Some((46.2, 2.2)));
        assert!(point.lat.is_none());
        assert_eq!(point.reason, "political_territory_is_not_a_point");
        let paris = event_place_point("locus", "Paris", None);
        assert!(paris.lat.is_some());
    }

    #[test]
    fn major_capitals_have_qids() {
        let capitals = [
            ("Paris", "Q90"),
            ("Vienna", "Q1741"),
            ("Berlin", "Q64"),
            ("Moscow", "Q649"),
            ("London", "Q84"),
            ("Madrid", "Q2807"),
            ("Rome", "Q220"),
        ];

        for (name, expected_qid) in capitals {
            let r = resolve_place_offline(name).unwrap_or_else(|| panic!("{name} should resolve"));
            assert_eq!(
                r.wikidata_qid.as_deref(),
                Some(expected_qid),
                "{name} should have QID {expected_qid}"
            );
        }
    }

    #[test]
    fn saint_helena_variants_resolve() {
        for variant in ["Saint Helena", "St Helena", "sainte-hélène"] {
            let r = resolve_place_offline(variant)
                .unwrap_or_else(|| panic!("{variant} should resolve"));
            assert_eq!(r.wikidata_qid.as_deref(), Some("Q34497"));
        }
    }

    #[test]
    fn elba_exile_resolves_with_qid() {
        let r = resolve_place_offline("Elba").unwrap();
        assert_eq!(r.wikidata_qid.as_deref(), Some("Q13334"));
    }

    #[test]
    fn entries_with_qid_have_higher_score() {
        let paris = resolve_place_offline("Paris").unwrap();
        assert!(
            paris.score >= 0.90,
            "Entry with QID should have score >= 0.90"
        );
    }

    #[test]
    fn compound_battles_resolve() {
        let r = resolve_place_offline("Jena-Auerstedt").unwrap();
        assert!(r.wikidata_qid.is_some());

        let r2 = resolve_place_offline("Aspern-Essling").unwrap();
        assert_eq!(r2.wikidata_qid.as_deref(), Some("Q159821"));
    }

    #[test]
    fn treaty_locations_have_qids() {
        let treaties = [
            ("Tilsit", "Q5705"),
            ("Campo Formio", "Q245038"),
            ("Lunéville", "Q203875"),
            ("Pressburg", "Q1780"),
            ("Amiens", "Q41688"),
        ];

        for (name, expected_qid) in treaties {
            let r = resolve_place_offline(name).unwrap_or_else(|| panic!("{name} should resolve"));
            assert_eq!(
                r.wikidata_qid.as_deref(),
                Some(expected_qid),
                "{name} should have QID {expected_qid}"
            );
        }
    }
}
