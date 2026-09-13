// crates/talaria-api/src/person_ingest/typing.rs
//! TypedTime + place resolution for person ingest.
//!
//! Grounding chain (P0 contract):
//!   mention → place identity (aliases / existing entities / TGN / WHG) → geocode that identity.
//!
//! Coordinates come only from Wikidata P625 / offline gazetteer / wiki page coords.
//! TGN and WHG are identity layers (connectors stubbed); they resolve place names to
//! authoritative identifiers which can then be geocoded via P625.

use talaria_quality::{place_query, TypedTime};
use talaria_sources::{
    resolve_place_offline, CompositeIdentityResolver, PlaceIdentity, PlaceIdentityResolver,
};
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

/// Resolve place identity WITHOUT coordinates.
/// This is the first step in the grounding chain: mention → identity.
/// Returns a PlaceIdentity if the place can be identified (possibly with a QID).
pub fn resolve_place_identity(label: Option<&str>) -> Option<PlaceIdentity> {
    let label = label.map(str::trim).filter(|s| !s.is_empty())?;

    // If already a QID, return identity directly
    if talaria_quality::is_wikidata_qid(label) {
        return Some(PlaceIdentity::from_wikidata(label, label));
    }

    // Try composite resolver: alias gazetteer → TGN (stub) → WHG (stub)
    let resolver = CompositeIdentityResolver::new();
    if let Some(identity) = resolver.resolve(label) {
        return Some(identity);
    }

    // Fallback: return unresolved identity (label only, no QID)
    None
}

/// Geocode a place identity (second step in grounding chain).
/// Coordinates come only from Wikidata P625 / offline gazetteer.
/// This function assumes identity was already resolved; it just fetches coords.
pub async fn geocode_identity(
    identity: &PlaceIdentity,
) -> Option<talaria_sources::PlaceResolution> {
    // If identity has a QID, fetch coords via P625
    if let Some(qid) = &identity.wikidata_qid {
        if let Ok(client) = talaria_wikidata::WikidataClient::new() {
            if let Ok(Some((lat, lon))) = client.fetch_coordinates(qid).await {
                return Some(talaria_sources::PlaceResolution {
                    label: identity.label.clone(),
                    method: "identity_then_p625".into(),
                    wikidata_qid: Some(qid.clone()),
                    lat,
                    lon,
                    precision: "wikidata_p625".into(),
                    uncertainty_radius_m: Some(5000.0),
                    score: identity.confidence,
                });
            }
        }
    }

    // If identity came from alias gazetteer, it already has coords in PlaceResolution
    if identity.identity_source == "alias_gazetteer" {
        return resolve_place_offline(&identity.label);
    }

    None
}

/// Combined place resolution: identity first, then geocode.
/// This is the main entry point for place grounding in person_ingest.
pub async fn geocode_place(label: Option<&str>) -> Option<talaria_sources::PlaceResolution> {
    let label = label.map(str::trim).filter(|s| !s.is_empty())?;

    // Step 1: Resolve place identity
    if let Some(identity) = resolve_place_identity(Some(label)) {
        // Step 2: Geocode the resolved identity
        if let Some(res) = geocode_identity(&identity).await {
            return Some(res);
        }
    }

    // Direct QID path (already a QID, not a mention)
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

    // Fallback: traditional place_query path for unresolved identities
    let q = place_query(label);
    let keys: Vec<&String> = q
        .search_keys
        .iter()
        .filter(|key| !talaria_sources::extractors::is_country_or_region(key))
        .collect();
    if keys.is_empty() {
        return None;
    }

    // Try offline gazetteer
    for key in &keys {
        if let Some(res) = resolve_place_offline(key) {
            return Some(res);
        }
    }

    // Try Wikidata search → P625 (identity resolution + geocode in one step)
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
