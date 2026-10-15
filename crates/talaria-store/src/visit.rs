// crates/talaria-store/src/visit.rs
//! Visit projections. Biography rows are read, never updated.

use crate::media::{upsert_media_asset, MediaAssetInsert};
use crate::visit_catalog::{specific_site_label, AcceptedSite};
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use uuid::Uuid;

pub const ITINERARY_GENERATION_VERSION: &str = "visit-itinerary-v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VisitTemporalClass {
    Lived,
    Memory,
    Tourist,
}

impl VisitTemporalClass {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Lived => "lived",
            Self::Memory => "memory",
            Self::Tourist => "tourist",
        }
    }
}

/// Map a lived biographical event onto a heritage relation.
/// Commemorations are refused here so they cannot be stored as deeds.
pub fn relation_for_lived_event(event_type: &str) -> Option<(&'static str, &'static str)> {
    match event_type {
        "birth" => Some(("birthplace", "other")),
        "residence" | "exile" => Some(("former_residence", "residence")),
        "death" | "burial" => Some(("burial_site", "burial")),
        "education" | "office" | "employment" => Some(("workplace", "other")),
        "battle" | "siege" | "meeting" | "travel" | "arrival" | "departure" | "diplomatic" => {
            Some(("historical_event_site", "monument"))
        }
        "commemoration" | "memorial" | "statue" | "museum" | "street_naming" | "publication"
        | "award" => None,
        _ => None,
    }
}

pub fn site_content_hash(label: &str, qid: Option<&str>, category: &str, lat: f64, lon: f64) -> String {
    let mut hasher = Sha256::new();
    hasher.update(label.trim().to_lowercase().as_bytes());
    hasher.update(b"|");
    hasher.update(qid.unwrap_or("").as_bytes());
    hasher.update(b"|");
    hasher.update(category.as_bytes());
    hasher.update(format!("|{lat:.5}|{lon:.5}").as_bytes());
    hex::encode(hasher.finalize())
}

pub fn fingerprint(parts: &[&str]) -> String {
    let mut hasher = Sha256::new();
    for (index, part) in parts.iter().enumerate() {
        if index > 0 {
            hasher.update(b"|");
        }
        hasher.update(part.as_bytes());
    }
    hex::encode(hasher.finalize())
}

#[derive(Debug, Clone, Copy, PartialEq, sqlx::FromRow)]
pub struct SitePoint {
    pub id: Uuid,
    pub lat: f64,
    pub lon: f64,
}

/// Greedy nearest-neighbor order. Distances are straight-line kilometres, not travel times.
pub fn order_stops(origin: (f64, f64), sites: &[SitePoint]) -> Vec<(Uuid, f64)> {
    let mut remaining = sites.to_vec();
    let mut cursor = origin;
    let mut ordered = Vec::with_capacity(remaining.len());
    while !remaining.is_empty() {
        let (index, km) = remaining
            .iter()
            .enumerate()
            .map(|(index, site)| (index, haversine_km(cursor, (site.lat, site.lon))))
            .min_by(|left, right| left.1.total_cmp(&right.1))
            .expect("remaining is non-empty");
        let next = remaining.remove(index);
        ordered.push((next.id, km));
        cursor = (next.lat, next.lon);
    }
    ordered
}

pub fn haversine_km(a: (f64, f64), b: (f64, f64)) -> f64 {
    const EARTH_KM: f64 = 6371.0;
    let lat1 = a.0.to_radians();
    let lat2 = b.0.to_radians();
    let dlat = (b.0 - a.0).to_radians();
    let dlon = (b.1 - a.1).to_radians();
    let h = (dlat / 2.0).sin().powi(2) + lat1.cos() * lat2.cos() * (dlon / 2.0).sin().powi(2);
    2.0 * EARTH_KM * h.sqrt().asin()
}

/// A historical fact never implies that a place is open today.
pub fn visitability_from_history() -> &'static str {
    "unknown"
}

#[derive(Debug, PartialEq, Eq)]
pub enum TemporalError {
    MissingYear,
    UnverifiedFuture,
    MissingProvenance,
}

pub fn activity_status(year: Option<i32>, now_year: i32) -> Result<&'static str, TemporalError> {
    match year {
        None => Err(TemporalError::MissingYear),
        Some(year) if year > now_year => Ok("scheduled"),
        Some(_) => Ok("past"),
    }
}

pub fn commemoration_status(
    year: Option<i32>,
    now_year: i32,
    future_verified: bool,
) -> Result<&'static str, TemporalError> {
    let Some(year) = year else {
        return Err(TemporalError::MissingYear);
    };
    if year > now_year {
        if future_verified {
            return Ok("future_verified");
        }
        return Err(TemporalError::UnverifiedFuture);
    }
    Ok("past")
}

pub fn require_provenance(value: &serde_json::Value) -> Result<(), TemporalError> {
    let empty = value.as_object().is_none_or(|object| object.is_empty());
    if empty {
        Err(TemporalError::MissingProvenance)
    } else {
        Ok(())
    }
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct LivedPlaceRow {
    pub event_id: Uuid,
    pub event_type: String,
    pub title: String,
    pub place_label: String,
    pub lat: f64,
    pub lon: f64,
}

pub async fn list_lived_places(pool: &PgPool, entity_id: Uuid) -> anyhow::Result<Vec<LivedPlaceRow>> {
    let rows = sqlx::query_as::<_, LivedPlaceRow>(
        r#"
        SELECT ce.id AS event_id, ce.event_type, ce.title, ce.place_label,
               ST_Y(ce.geom::geometry) AS lat, ST_X(ce.geom::geometry) AS lon
        FROM canonical_events ce
        WHERE ce.entity_id = $1
          AND ce.is_active
          AND ce.pipeline = 'person'
          AND ce.place_label IS NOT NULL
          AND ce.geom IS NOT NULL
          AND ce.event_type <> 'commemoration'
        "#,
    )
    .bind(entity_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn upsert_heritage_site(
    pool: &PgPool,
    label: &str,
    place_qid: Option<&str>,
    category: &str,
    lat: f64,
    lon: f64,
    provenance: serde_json::Value,
) -> anyhow::Result<Uuid> {
    upsert_catalog_row(
        pool,
        label,
        place_qid,
        None,
        category,
        Some(lat),
        Some(lon),
        None,
        provenance,
        "site",
        "published",
        None,
    )
    .await
}

async fn upsert_catalog_row(
    pool: &PgPool,
    label: &str,
    place_qid: Option<&str>,
    merimee_ref: Option<&str>,
    category: &str,
    lat: Option<f64>,
    lon: Option<f64>,
    summary: Option<&str>,
    provenance: serde_json::Value,
    grain: &str,
    publication: &str,
    media_asset_id: Option<Uuid>,
) -> anyhow::Result<Uuid> {
    if let Some(qid) = place_qid {
        if let Some(existing) = sqlx::query_scalar::<_, Uuid>("SELECT id FROM heritage_sites WHERE place_qid = $1")
            .bind(qid)
            .fetch_optional(pool)
            .await?
        {
            return Ok(existing);
        }
    }
    if let Some(reference) = merimee_ref {
        if let Some(existing) = sqlx::query_scalar::<_, Uuid>(
            "SELECT id FROM heritage_sites WHERE external_ids->>'merimee' = $1",
        )
        .bind(reference)
        .fetch_optional(pool)
        .await?
        {
            return Ok(existing);
        }
    }
    let hash = site_content_hash(label, place_qid.or(merimee_ref), category, lat.unwrap_or(0.0), lon.unwrap_or(0.0));
    let external_ids = serde_json::json!({
        "wikidata": place_qid,
        "merimee": merimee_ref,
    });
    let id: Uuid = sqlx::query_scalar(
        r#"
        INSERT INTO heritage_sites (
            place_label, place_qid, category, lat, lon, geom, visitability,
            practical_json, provenance_json, content_hash, identity_grain,
            publication_status, external_ids, summary, media_asset_id
        )
        VALUES (
            $1, $2, $3, $4, $5,
            CASE WHEN $4::float8 IS NULL OR $5::float8 IS NULL THEN NULL
                 ELSE ST_SetSRID(ST_MakePoint($5, $4), 4326)::geography END,
            'unknown', '{}'::jsonb, $6, $7, $8, $9, $10, $11, $12
        )
        ON CONFLICT (content_hash) DO UPDATE SET fetched_at = NOW()
        RETURNING id
        "#,
    )
    .bind(label)
    .bind(place_qid)
    .bind(category)
    .bind(lat)
    .bind(lon)
    .bind(provenance)
    .bind(hash)
    .bind(grain)
    .bind(publication)
    .bind(external_ids)
    .bind(summary)
    .bind(media_asset_id)
    .fetch_one(pool)
    .await?;
    Ok(id)
}

pub async fn persist_catalog(pool: &PgPool, entity_id: Uuid, sites: &[AcceptedSite]) -> anyhow::Result<i64> {
    let mut links = 0_i64;
    for site in sites {
        let media_id = if let Some(media) = &site.media {
            Some(
                upsert_media_asset(
                    pool,
                    &MediaAssetInsert {
                        commons_file: media.commons_file.clone(),
                        mid: None,
                        sha1: None,
                        mime: Some("image".into()),
                        license: Some(media.license.clone()),
                        attribution_text: media.attribution_text.clone(),
                        thumb_url: media.thumb_url.clone(),
                        depicts_qids: media.depicts_qids.clone(),
                        revision_id: None,
                        rights_normalized: "open".into(),
                        entity_id: Some(entity_id),
                        corpus_document_id: None,
                    },
                )
                .await?,
            )
        } else {
            None
        };
        let provenance = site
            .relations
            .first()
            .map(|relation| relation.provenance.clone())
            .unwrap_or_else(|| serde_json::json!({}));
        let site_id = upsert_catalog_row(
            pool,
            &site.label,
            site.place_qid.as_deref(),
            site.merimee_ref.as_deref(),
            &site.category,
            site.lat,
            site.lon,
            site.summary.as_deref(),
            provenance,
            "site",
            "published",
            media_id,
        )
        .await?;
        if media_id.is_some() {
            sqlx::query("UPDATE heritage_sites SET media_asset_id = $2 WHERE id = $1 AND media_asset_id IS NULL")
                .bind(site_id)
                .bind(media_id)
                .execute(pool)
                .await?;
        }
        for relation in &site.relations {
            link_heritage(
                pool,
                entity_id,
                site_id,
                &relation.relation,
                &relation.temporal_class,
                None,
                relation.provenance.clone(),
            )
            .await?;
            links += 1;
        }
    }
    Ok(links)
}

pub async fn link_heritage(
    pool: &PgPool,
    entity_id: Uuid,
    site_id: Uuid,
    relation: &str,
    temporal_class: &str,
    source_event_id: Option<Uuid>,
    provenance: serde_json::Value,
) -> anyhow::Result<()> {
    sqlx::query(
        r#"
        INSERT INTO entity_heritage_links (
            entity_id, site_id, relation, temporal_class, source_event_id, provenance_json
        )
        VALUES ($1, $2, $3, $4, $5, $6)
        ON CONFLICT (entity_id, site_id, relation) DO NOTHING
        "#,
    )
    .bind(entity_id)
    .bind(site_id)
    .bind(relation)
    .bind(temporal_class)
    .bind(source_event_id)
    .bind(provenance)
    .execute(pool)
    .await?;
    Ok(())
}

/// Project geolocated lived events into Visit sites. Does not write canonical rows.
pub async fn project_lived_heritage(pool: &PgPool, entity_id: Uuid) -> anyhow::Result<i64> {
    let places = list_lived_places(pool, entity_id).await?;
    let mut linked = 0_i64;
    for place in places {
        let Some((relation, category)) = relation_for_lived_event(&place.event_type) else {
            continue;
        };
        if !specific_site_label(&place.place_label) {
            continue;
        }
        let provenance = serde_json::json!({
            "kind": "canonical_event",
            "event_id": place.event_id,
            "event_type": place.event_type,
        });
        let site_id = upsert_heritage_site(
            pool,
            &place.place_label,
            None,
            category,
            place.lat,
            place.lon,
            provenance.clone(),
        )
        .await?;
        link_heritage(
            pool,
            entity_id,
            site_id,
            relation,
            VisitTemporalClass::Lived.as_str(),
            Some(place.event_id),
            provenance,
        )
        .await?;
        linked += 1;
    }
    Ok(linked)
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct HeritageSiteRow {
    pub id: Uuid,
    pub place_label: String,
    pub place_qid: Option<String>,
    pub category: String,
    pub relation: String,
    pub temporal_class: String,
    pub lat: Option<f64>,
    pub lon: Option<f64>,
    pub visitability: String,
    pub official_url: Option<String>,
    pub practical_json: serde_json::Value,
    pub practical_verified_at: Option<chrono::DateTime<chrono::Utc>>,
    pub provenance_json: serde_json::Value,
    pub summary: Option<String>,
    pub image_thumb_url: Option<String>,
    pub image_license: Option<String>,
    pub image_credit: Option<String>,
    pub image_commons_file: Option<String>,
    pub merimee_ref: Option<String>,
    pub merimee_protection_kind: Option<String>,
    pub merimee_commune: Option<String>,
    pub merimee_historique: Option<String>,
}

pub async fn list_heritage_sites(
    pool: &PgPool,
    entity_id: Uuid,
    category: Option<&str>,
    bbox: Option<[f64; 4]>,
    limit: i64,
    offset: i64,
) -> anyhow::Result<Vec<HeritageSiteRow>> {
    let [west, south, east, north] = bbox.unwrap_or([0.0, 0.0, 0.0, 0.0]);
    let use_bbox = bbox.is_some();
    let rows = sqlx::query_as::<_, HeritageSiteRow>(
        r#"
        SELECT s.id, s.place_label, s.place_qid, s.category, l.relation, l.temporal_class,
               s.lat, s.lon, s.visitability, s.official_url, s.practical_json,
               s.practical_verified_at, l.provenance_json, COALESCE(s.summary, ce.summary) AS summary,
               m.thumb_url AS image_thumb_url, m.license AS image_license,
               m.attribution_text AS image_credit, m.commons_file AS image_commons_file,
               COALESCE(s.external_ids->>'merimee', mn.reference) AS merimee_ref,
               mn.protection_kind AS merimee_protection_kind,
               mn.commune AS merimee_commune,
               mn.historique AS merimee_historique
        FROM entity_heritage_links l
        JOIN heritage_sites s ON s.id = l.site_id
        LEFT JOIN canonical_events ce ON ce.id = l.source_event_id
        LEFT JOIN media_assets m ON m.id = s.media_asset_id
        LEFT JOIN merimee_notices mn ON mn.reference = s.external_ids->>'merimee'
            OR mn.heritage_site_id = s.id
        WHERE l.entity_id = $1
          AND s.publication_status = 'published'
          AND s.identity_grain = 'site'
          AND ($2::text IS NULL OR s.category = $2)
          AND (
            NOT $3::bool
            OR (
              s.geom IS NOT NULL
              AND ST_Intersects(
                s.geom,
                ST_MakeEnvelope($4, $5, $6, $7, 4326)::geography
              )
            )
          )
        ORDER BY l.relation, s.place_label
        LIMIT $8 OFFSET $9
        "#,
    )
    .bind(entity_id)
    .bind(category)
    .bind(use_bbox)
    .bind(west)
    .bind(south)
    .bind(east)
    .bind(north)
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ActivityRow {
    pub id: Uuid,
    pub site_id: Option<Uuid>,
    pub kind: String,
    pub title: String,
    pub time_json: serde_json::Value,
    pub occurrence_status: String,
    pub provenance_json: serde_json::Value,
}

pub async fn list_activities(
    pool: &PgPool,
    entity_id: Uuid,
    limit: i64,
    offset: i64,
) -> anyhow::Result<Vec<ActivityRow>> {
    let rows = sqlx::query_as::<_, ActivityRow>(
        r#"
        SELECT id, site_id, kind, title, time_json, occurrence_status, provenance_json
        FROM cultural_activities
        WHERE entity_id = $1
        ORDER BY created_at, id
        LIMIT $2 OFFSET $3
        "#,
    )
    .bind(entity_id)
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn insert_activity(
    pool: &PgPool,
    entity_id: Uuid,
    site_id: Option<Uuid>,
    kind: &str,
    title: &str,
    time_json: serde_json::Value,
    occurrence_status: &str,
    provenance: serde_json::Value,
) -> anyhow::Result<Uuid> {
    let year = time_json
        .get("year")
        .and_then(|value| value.as_i64())
        .map(|year| year.to_string())
        .unwrap_or_default();
    let site = site_id.map(|id| id.to_string()).unwrap_or_default();
    let key = fingerprint(&[
        &entity_id.to_string(),
        kind,
        title,
        &year,
        &site,
    ]);
    let id: Uuid = sqlx::query_scalar(
        r#"
        INSERT INTO cultural_activities (
            entity_id, site_id, kind, title, time_json, occurrence_status, provenance_json, fingerprint
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
        ON CONFLICT (fingerprint) DO UPDATE SET provenance_json = EXCLUDED.provenance_json
        RETURNING id
        "#,
    )
    .bind(entity_id)
    .bind(site_id)
    .bind(kind)
    .bind(title)
    .bind(time_json)
    .bind(occurrence_status)
    .bind(provenance)
    .bind(key)
    .fetch_one(pool)
    .await?;
    Ok(id)
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct CommemorationRow {
    pub id: Uuid,
    pub site_id: Option<Uuid>,
    pub title: String,
    pub time_json: serde_json::Value,
    pub occurrence_status: String,
    pub place_persistence: String,
    pub provenance_json: serde_json::Value,
}

pub async fn list_commemorations(
    pool: &PgPool,
    entity_id: Uuid,
    limit: i64,
    offset: i64,
) -> anyhow::Result<Vec<CommemorationRow>> {
    let rows = sqlx::query_as::<_, CommemorationRow>(
        r#"
        SELECT id, site_id, title, time_json, occurrence_status, place_persistence, provenance_json
        FROM commemorations
        WHERE honoree_entity_id = $1
        ORDER BY created_at, id
        LIMIT $2 OFFSET $3
        "#,
    )
    .bind(entity_id)
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn insert_commemoration(
    pool: &PgPool,
    entity_id: Uuid,
    site_id: Option<Uuid>,
    title: &str,
    time_json: serde_json::Value,
    occurrence_status: &str,
    place_persistence: &str,
    provenance: serde_json::Value,
) -> anyhow::Result<Uuid> {
    let year = time_json
        .get("year")
        .and_then(|value| value.as_i64())
        .map(|year| year.to_string())
        .unwrap_or_default();
    let key = fingerprint(&[&entity_id.to_string(), title, &year, occurrence_status]);
    let id: Uuid = sqlx::query_scalar(
        r#"
        INSERT INTO commemorations (
            honoree_entity_id, site_id, title, time_json, occurrence_status,
            place_persistence, provenance_json, fingerprint
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
        ON CONFLICT (fingerprint) DO UPDATE SET provenance_json = EXCLUDED.provenance_json
        RETURNING id
        "#,
    )
    .bind(entity_id)
    .bind(site_id)
    .bind(title)
    .bind(time_json)
    .bind(occurrence_status)
    .bind(place_persistence)
    .bind(provenance)
    .bind(key)
    .fetch_one(pool)
    .await?;
    Ok(id)
}

pub async fn load_site_points(pool: &PgPool, entity_id: Uuid, ids: &[Uuid]) -> anyhow::Result<Vec<SitePoint>> {
    let rows = sqlx::query_as::<_, SitePoint>(
        r#"
        SELECT s.id, s.lat, s.lon
        FROM heritage_sites s
        JOIN entity_heritage_links l ON l.site_id = s.id
        WHERE l.entity_id = $1
          AND s.lat IS NOT NULL
          AND s.lon IS NOT NULL
          AND (cardinality($2::uuid[]) = 0 OR s.id = ANY($2))
        "#,
    )
    .bind(entity_id)
    .bind(ids)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn insert_itinerary(
    pool: &PgPool,
    entity_id: Uuid,
    mode: &str,
    origin: Option<(f64, f64)>,
    stops: &[(Uuid, f64)],
) -> anyhow::Result<Uuid> {
    let (lat, lon) = match origin {
        Some((lat, lon)) => (Some(lat), Some(lon)),
        None => (None, None),
    };
    let id: Uuid = sqlx::query_scalar(
        r#"
        INSERT INTO visit_itineraries (
            entity_id, mode, origin_lat, origin_lon, routing_status, generation_version
        )
        VALUES ($1, $2, $3, $4, 'unavailable', $5)
        RETURNING id
        "#,
    )
    .bind(entity_id)
    .bind(mode)
    .bind(lat)
    .bind(lon)
    .bind(ITINERARY_GENERATION_VERSION)
    .fetch_one(pool)
    .await?;
    for (ordinal, (site_id, km)) in stops.iter().enumerate() {
        sqlx::query(
            r#"
            INSERT INTO visit_itinerary_stops (itinerary_id, ordinal, site_id, straight_line_km)
            VALUES ($1, $2, $3, $4)
            "#,
        )
        .bind(id)
        .bind(ordinal as i32)
        .bind(site_id)
        .bind(km)
        .execute(pool)
        .await?;
    }
    Ok(id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lived_residence_is_not_a_commemoration() {
        assert_eq!(
            relation_for_lived_event("residence"),
            Some(("former_residence", "residence"))
        );
        assert_eq!(relation_for_lived_event("commemoration"), None);
        assert_eq!(visitability_from_history(), "unknown");
    }

    #[test]
    fn future_homage_requires_verification() {
        assert_eq!(
            commemoration_status(Some(2005), 2026, false),
            Ok("past")
        );
        assert_eq!(
            commemoration_status(Some(2030), 2026, false),
            Err(TemporalError::UnverifiedFuture)
        );
        assert_eq!(
            commemoration_status(Some(2030), 2026, true),
            Ok("future_verified")
        );
        assert_eq!(activity_status(Some(1802), 2026), Ok("past"));
        assert!(require_provenance(&serde_json::json!({})).is_err());
        assert!(require_provenance(&serde_json::json!({"source":"wikidata"})).is_ok());
    }

    #[test]
    fn stops_follow_straight_line_order_without_invented_duration() {
        let paris = Uuid::from_u128(1);
        let lyon = Uuid::from_u128(2);
        let ordered = order_stops(
            (48.86, 2.35),
            &[
                SitePoint { id: lyon, lat: 45.75, lon: 4.85 },
                SitePoint { id: paris, lat: 48.85, lon: 2.34 },
            ],
        );
        assert_eq!(ordered[0].0, paris);
        assert!(ordered[0].1 < ordered[1].1);
    }

    #[test]
    fn site_hash_is_stable() {
        let left = site_content_hash("Paris", Some("Q90"), "monument", 48.8566, 2.3522);
        let right = site_content_hash(" paris ", Some("Q90"), "monument", 48.8566, 2.3522);
        assert_eq!(left, right);
    }
}
