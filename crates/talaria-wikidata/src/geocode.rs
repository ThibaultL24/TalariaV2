// crates/talaria-wikidata/src/geocode.rs
use crate::client::WikidataClient;
use serde_json::json;

#[derive(Debug, Clone, PartialEq)]
pub struct GeocodedPlace {
    pub place_label: String,
    pub wikidata_qid: String,
    pub lat: f64,
    pub lon: f64,
    pub raw_json: serde_json::Value,
}

pub async fn geocode_place_label(
    client: &WikidataClient,
    label: &str,
    lang: &str,
) -> anyhow::Result<Option<GeocodedPlace>> {
    let trimmed = label.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }

    let Some(qid) = client.search_entity(trimmed, lang).await? else {
        tracing::debug!(label = trimmed, "wikidata search returned no results");
        return Ok(None);
    };

    let Some((lat, lon)) = client.fetch_coordinates(&qid).await? else {
        tracing::debug!(label = trimmed, qid, "wikidata entity has no coordinates");
        return Ok(None);
    };

    Ok(Some(GeocodedPlace {
        place_label: trimmed.to_string(),
        wikidata_qid: qid.clone(),
        lat,
        lon,
        raw_json: json!({ "qid": qid, "lat": lat, "lon": lon }),
    }))
}
