// crates/talaria-api/src/person_ingest/typing.rs
//! TypedTime + place resolution for person ingest.
//!
//! Grounding chain (P0 contract):
//!   mention → place identity (aliases / existing entities / TGN / WHG) → geocode that identity.
//!
//! Coordinates come only from Wikidata P625 / offline gazetteer / wiki page coords.
//! TGN and WHG are identity layers (fully async); they resolve place names to
//! authoritative identifiers which can then be geocoded via P625.

use talaria_quality::{
    approx_typed_time, infer_approximate_year, is_approx_eligible_type, place_query, TypedTime,
};
use talaria_sources::{
    resolve_place_offline, CompositeIdentityResolver, PlaceIdentity, PlaceIdentityResolver,
};
use talaria_store::{apply_full_place_grounding, upsert_place_resolution, PlaceResolutionInsert};
use uuid::Uuid;

/// Result of place grounding: coordinates + identity QID.
/// Used to pass both pieces to event persistence.
#[derive(Debug, Clone)]
pub struct PlaceGroundingResult {
    pub coords: Option<(f64, f64)>,
    pub identity_qid: Option<String>,
    pub identity_source: Option<String>,
    /// Full identity for audit trail (if resolved).
    pub identity: Option<PlaceIdentity>,
}

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

/// Context for inferring approximate years from surrounding text.
#[derive(Default)]
pub struct YearInferenceContext<'a> {
    pub clause_text: &'a str,
    pub paragraph_context: Option<&'a str>,
    pub section_heading: Option<&'a str>,
    pub birth_year: Option<i32>,
    pub death_year: Option<i32>,
}

/// Create TypedTime with context-aware approximate year inference.
/// If the event has no explicit year but is an eligible anecdote type,
/// try to infer an approximate year from surrounding context.
pub fn typed_time_from_year_with_context(
    year: Option<i32>,
    event_type: &str,
    ctx: &YearInferenceContext<'_>,
) -> TypedTime {
    // If we have an explicit year, use it
    if let Some(y) = year {
        return TypedTime::Exact {
            year: y,
            month: None,
            day: None,
            surface: Some(y.to_string()),
        };
    }

    // Try to infer an approximate year from context
    if is_approx_eligible_type(event_type) {
        if let Some((inferred_year, source)) = infer_approximate_year(
            event_type,
            ctx.clause_text,
            ctx.paragraph_context,
            ctx.section_heading,
            ctx.birth_year,
            ctx.death_year,
        ) {
            return approx_typed_time(inferred_year, source);
        }
    }

    // Fall back to unknown
    TypedTime::Unknown { surface: None }
}

/// Resolve place identity WITHOUT coordinates (async).
/// This is the first step in the grounding chain: mention → identity.
/// Returns a PlaceIdentity if the place can be identified (possibly with a QID).
///
/// IMPORTANT: This function is async to support TGN SPARQL and WHG REST identity resolution.
/// Never use block_on to call this from sync context — it will panic inside async runtime.
pub async fn resolve_place_identity(label: Option<&str>) -> Option<PlaceIdentity> {
    let label = label.map(str::trim).filter(|s| !s.is_empty())?;

    // If already a QID, return identity directly
    if talaria_quality::is_wikidata_qid(label) {
        return Some(PlaceIdentity::from_wikidata(label, label));
    }

    // Try composite resolver: alias gazetteer → TGN → WHG
    let resolver = CompositeIdentityResolver::new();
    if let Some(identity) = resolver.resolve(label).await {
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

    // Step 1: Resolve place identity (async — TGN SPARQL, WHG REST, or alias gazetteer)
    if let Some(identity) = resolve_place_identity(Some(label)).await {
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

/// Full place grounding: identity resolution + geocoding in one call.
/// Returns both coordinates and the Wikidata QID for `place_identity_qid`.
/// This is the main entry point for person_ingest place grounding.
pub async fn ground_place_full(
    label: Option<&str>,
    given_coords: Option<(f64, f64)>,
) -> PlaceGroundingResult {
    let label = match label.map(str::trim).filter(|s| !s.is_empty()) {
        Some(l) => l,
        None => {
            return PlaceGroundingResult {
                coords: given_coords,
                identity_qid: None,
                identity_source: None,
                identity: None,
            };
        }
    };

    // Step 1: Resolve place identity (async — TGN SPARQL, WHG REST, or alias gazetteer)
    let identity = resolve_place_identity(Some(label)).await;
    let identity_qid = identity.as_ref().and_then(|i| i.wikidata_qid.clone());
    let identity_source = identity.as_ref().map(|i| i.identity_source.clone());

    // Step 2: Get coordinates (given coords take precedence, then geocode identity)
    let coords = if given_coords.is_some() {
        given_coords
    } else if let Some(ref ident) = identity {
        geocode_identity(ident).await.map(|r| (r.lat, r.lon))
    } else {
        // Fallback: try traditional geocode_place path
        geocode_place(Some(label)).await.map(|r| (r.lat, r.lon))
    };

    // If we still don't have a QID but geocode_place found one, use it
    let final_qid = if identity_qid.is_some() {
        identity_qid
    } else if let Some(res) = geocode_place(Some(label)).await {
        res.wikidata_qid
    } else {
        None
    };

    PlaceGroundingResult {
        coords,
        identity_qid: final_qid,
        identity_source,
        identity,
    }
}

/// Resolve coordinates for an event, with given coords taking precedence.
/// Also returns identity QID if resolved.
pub async fn resolve_coords_with_identity(
    label: Option<&str>,
    given: Option<(f64, f64)>,
) -> (Option<(f64, f64)>, Option<String>) {
    let result = ground_place_full(label, given).await;
    (result.coords, result.identity_qid)
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
        let result = ground_place_full(Some(&label), None).await;
        if let Some((lat, lon)) = result.coords {
            apply_full_place_grounding(pool, id, result.identity_qid.as_deref(), lat, lon).await?;
        }
    }
    Ok(())
}

/// Persist place resolution to the audit trail table.
pub async fn persist_place_resolution(
    pool: &sqlx::PgPool,
    label: &str,
    identity: &PlaceIdentity,
    coords: Option<(f64, f64)>,
) -> anyhow::Result<Uuid> {
    upsert_place_resolution(
        pool,
        &PlaceResolutionInsert {
            place_entity_id: None,
            place_label: label.to_string(),
            method: identity.identity_source.clone(),
            wikidata_qid: identity.wikidata_qid.clone(),
            tgn_id: identity.tgn_id.clone(),
            whg_id: identity.whg_id.clone(),
            geonames_id: identity.geonames_id.clone(),
            lat: coords.map(|(lat, _)| lat),
            lon: coords.map(|(_, lon)| lon),
            score: Some(identity.confidence),
            identity_source: Some(identity.identity_source.clone()),
            raw_json: serde_json::json!({
                "label": identity.label,
                "confidence": identity.confidence,
            }),
        },
    )
    .await
}
