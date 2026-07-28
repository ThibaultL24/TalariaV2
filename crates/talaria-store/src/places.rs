// crates/talaria-store/src/places.rs
use sqlx::PgPool;

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
