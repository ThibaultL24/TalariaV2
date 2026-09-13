// crates/talaria-store/src/places.rs
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct PlaceGeocodeRow {
    pub place_label: String,
    pub wikidata_qid: Option<String>,
    pub lat: Option<f64>,
    pub lon: Option<f64>,
}

pub async fn get_place_geocode(
    pool: &PgPool,
    wiki_lang: &str,
    place_label: &str,
) -> anyhow::Result<Option<PlaceGeocodeRow>> {
    let row = sqlx::query_as::<_, PlaceGeocodeRow>(
        r#"
        SELECT place_label, wikidata_qid, lat, lon
        FROM place_geocodes
        WHERE wiki_lang = $1 AND place_label = $2
        "#,
    )
    .bind(wiki_lang)
    .bind(place_label)
    .fetch_optional(pool)
    .await?;

    Ok(row)
}

pub async fn upsert_place_geocode(
    pool: &PgPool,
    wiki_lang: &str,
    place_label: &str,
    wikidata_qid: &str,
    lat: f64,
    lon: f64,
    raw_json: serde_json::Value,
) -> anyhow::Result<()> {
    sqlx::query(
        r#"
        INSERT INTO place_geocodes (place_label, wiki_lang, wikidata_qid, lat, lon, raw_json)
        VALUES ($1, $2, $3, $4, $5, $6)
        ON CONFLICT (wiki_lang, place_label) DO UPDATE SET
            wikidata_qid = EXCLUDED.wikidata_qid,
            lat = EXCLUDED.lat,
            lon = EXCLUDED.lon,
            raw_json = EXCLUDED.raw_json
        "#,
    )
    .bind(place_label)
    .bind(wiki_lang)
    .bind(wikidata_qid)
    .bind(lat)
    .bind(lon)
    .bind(raw_json)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn list_place_labels_needing_geocode(
    pool: &PgPool,
    wiki_lang: &str,
    limit: i64,
) -> anyhow::Result<Vec<String>> {
    let rows: Vec<(String,)> = sqlx::query_as(
        r#"
        SELECT DISTINCT ce.place_label
        FROM canonical_events ce
        LEFT JOIN place_geocodes pg
          ON pg.wiki_lang = $1 AND pg.place_label = ce.place_label
        WHERE ce.place_label IS NOT NULL
          AND pg.id IS NULL
        ORDER BY ce.place_label ASC
        LIMIT $2
        "#,
    )
    .bind(wiki_lang)
    .bind(limit)
    .fetch_all(pool)
    .await?;

    Ok(rows.into_iter().map(|(label,)| label).collect())
}

pub async fn apply_geocode_to_events(
    pool: &PgPool,
    wiki_lang: &str,
    place_label: &str,
    lat: f64,
    lon: f64,
) -> anyhow::Result<u64> {
    let result = sqlx::query(
        r#"
        UPDATE canonical_events
        SET geom = ST_SetSRID(ST_MakePoint($4, $3), 4326)::geography,
            map_eligible = true
        WHERE place_label = $2
          AND EXISTS (
            SELECT 1 FROM entities e
            WHERE e.id = canonical_events.entity_id AND e.wiki_lang = $1
          )
        "#,
    )
    .bind(wiki_lang)
    .bind(place_label)
    .bind(lat)
    .bind(lon)
    .execute(pool)
    .await?;

    Ok(result.rows_affected())
}

pub async fn apply_coords_to_event(
    pool: &PgPool,
    event_id: Uuid,
    lat: f64,
    lon: f64,
) -> anyhow::Result<()> {
    sqlx::query(
        r#"
        UPDATE canonical_events
        SET geom = ST_SetSRID(ST_MakePoint($2, $3), 4326)::geography,
            map_eligible = true
        WHERE id = $1 AND is_active
        "#,
    )
    .bind(event_id)
    .bind(lon)
    .bind(lat)
    .execute(pool)
    .await?;
    Ok(())
}

/// Apply place identity QID to an existing canonical event.
/// This is separate from coordinates — identity resolution establishes QID,
/// geocoding provides coordinates afterward.
pub async fn apply_place_identity_to_event(
    pool: &PgPool,
    event_id: Uuid,
    place_identity_qid: &str,
) -> anyhow::Result<()> {
    sqlx::query(
        r#"
        UPDATE canonical_events
        SET place_identity_qid = $2
        WHERE id = $1 AND is_active
        "#,
    )
    .bind(event_id)
    .bind(place_identity_qid)
    .execute(pool)
    .await?;
    Ok(())
}

/// Apply both place identity QID and coordinates to an existing event.
pub async fn apply_full_place_grounding(
    pool: &PgPool,
    event_id: Uuid,
    place_identity_qid: Option<&str>,
    lat: f64,
    lon: f64,
) -> anyhow::Result<()> {
    sqlx::query(
        r#"
        UPDATE canonical_events
        SET geom = ST_SetSRID(ST_MakePoint($3, $4), 4326)::geography,
            map_eligible = true,
            place_identity_qid = COALESCE($2, place_identity_qid)
        WHERE id = $1 AND is_active
        "#,
    )
    .bind(event_id)
    .bind(place_identity_qid)
    .bind(lon)
    .bind(lat)
    .execute(pool)
    .await?;
    Ok(())
}

/// Parameters for upserting a place resolution audit record.
#[derive(Debug, Clone)]
pub struct PlaceResolutionInsert {
    pub place_entity_id: Option<Uuid>,
    pub place_label: String,
    pub method: String,
    pub wikidata_qid: Option<String>,
    pub tgn_id: Option<String>,
    pub whg_id: Option<String>,
    pub geonames_id: Option<String>,
    pub lat: Option<f64>,
    pub lon: Option<f64>,
    pub score: Option<f32>,
    pub identity_source: Option<String>,
    pub raw_json: serde_json::Value,
}

/// Upsert a place resolution record (audit trail for place identity grounding).
/// Deduplicated by (place_label, method, wikidata_qid).
pub async fn upsert_place_resolution(
    pool: &PgPool,
    res: &PlaceResolutionInsert,
) -> anyhow::Result<Uuid> {
    let id: Uuid = sqlx::query_scalar(
        r#"
        INSERT INTO place_resolutions (
            place_entity_id, place_label, method, wikidata_qid,
            tgn_id, whg_id, geonames_id, lat, lon, score,
            identity_source, raw_json
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
        ON CONFLICT (place_label, method, (COALESCE(wikidata_qid, '')))
        DO UPDATE SET
            place_entity_id = COALESCE(EXCLUDED.place_entity_id, place_resolutions.place_entity_id),
            lat = COALESCE(EXCLUDED.lat, place_resolutions.lat),
            lon = COALESCE(EXCLUDED.lon, place_resolutions.lon),
            score = COALESCE(EXCLUDED.score, place_resolutions.score),
            identity_source = COALESCE(EXCLUDED.identity_source, place_resolutions.identity_source),
            raw_json = EXCLUDED.raw_json
        RETURNING id
        "#,
    )
    .bind(res.place_entity_id)
    .bind(&res.place_label)
    .bind(&res.method)
    .bind(&res.wikidata_qid)
    .bind(&res.tgn_id)
    .bind(&res.whg_id)
    .bind(&res.geonames_id)
    .bind(res.lat)
    .bind(res.lon)
    .bind(res.score)
    .bind(&res.identity_source)
    .bind(&res.raw_json)
    .fetch_one(pool)
    .await?;
    Ok(id)
}
