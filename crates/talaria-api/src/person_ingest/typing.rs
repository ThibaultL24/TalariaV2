// crates/talaria-api/src/person_ingest/typing.rs
//! TypedTime + place resolution for person ingest.

use talaria_quality::{place_query, TypedTime};
use talaria_sources::resolve_place_offline;
use talaria_store::apply_coords_to_event;
use uuid::Uuid;

pub fn typed_time_from_year(year: Option<i32>) -> TypedTime {
    match year {
        Some(y) => TypedTime::Exact {
            year: y,
            month: None,
            day: None,
            surface: Some(y.to_string()),
        },
        None => TypedTime::Unknown { surface: None },
    }
}

pub async fn geocode_place(label: Option<&str>) -> Option<talaria_sources::PlaceResolution> {
    let label = label.map(str::trim).filter(|s| !s.is_empty())?;
    if talaria_quality::is_wikidata_qid(label) {
        if let Ok(client) = talaria_wikidata::WikidataClient::new() {
            if let Ok(Some((lat, lon))) = client.fetch_coordinates(label).await {
                return Some(talaria_sources::PlaceResolution {
                    label: label.to_string(),
                    method: "wikidata_qid_p625".into(),
                    wikidata_qid: Some(label.to_string()),
                    lat,
                    lon,
                    precision: "wikidata_p625".into(),
                    uncertainty_radius_m: Some(5000.0),
                    score: 0.75,
                });
            }
        }
    }
    let q = place_query(label);
    let keys: Vec<&String> = q
        .search_keys
        .iter()
        .filter(|key| !talaria_sources::extractors::is_country_or_region(key))
        .collect();
    if keys.is_empty() {
        return None;
    }
    for key in &keys {
        if let Some(res) = resolve_place_offline(key) {
            return Some(res);
        }
    }
    for key in &keys {
        let Some(hit) = crate::lot_e::resolve_label_coords(key).await else {
            continue;
        };
        return Some(talaria_sources::PlaceResolution {
            label: q.surface.clone(),
            method: "wikidata_p625".into(),
            wikidata_qid: None,
            lat: hit.lat,
            lon: hit.lon,
            precision: hit.precision,
            uncertainty_radius_m: hit.uncertainty,
            score: 0.7,
        });
    }
    None
}

pub async fn resolve_coords(label: Option<&str>, given: Option<(f64, f64)>) -> Option<(f64, f64)> {
    if given.is_some() {
        return given;
    }
    geocode_place(label).await.map(|g| (g.lat, g.lon))
}

pub async fn backfill_person_geocodes(pool: &sqlx::PgPool, entity_id: Uuid) -> anyhow::Result<()> {
    let rows: Vec<(Uuid, Option<String>)> = sqlx::query_as(
        r#"
        SELECT id, place_label FROM canonical_events
        WHERE entity_id = $1 AND pipeline = 'person' AND is_active
          AND place_label IS NOT NULL AND btrim(place_label) <> ''
        "#,
    )
    .bind(entity_id)
    .fetch_all(pool)
    .await?;
    for (id, label) in rows {
        let Some(label) = label else { continue };
        if let Some(geo) = geocode_place(Some(&label)).await {
            apply_coords_to_event(pool, id, geo.lat, geo.lon).await?;
        }
    }
    Ok(())
}
