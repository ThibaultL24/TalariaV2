// crates/talaria-store/src/profiles.rs
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct PeriodRow {
    pub id: Uuid,
    pub slug: String,
    pub label: String,
    pub start_year: Option<i32>,
    pub end_year: Option<i32>,
    pub kind: String,
    pub wikidata_qid: Option<String>,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct EntityProfileRow {
    pub id: Uuid,
    pub entity_id: Uuid,
    pub profile_qid: Option<String>,
    pub profile_slug: String,
    pub profile_label: String,
    pub kind: String,
    pub confidence: f64,
    pub source_system: String,
}

pub async fn upsert_period(
    pool: &PgPool,
    slug: &str,
    label: &str,
    start_year: Option<i32>,
    end_year: Option<i32>,
    kind: &str,
    wikidata_qid: Option<&str>,
) -> anyhow::Result<Uuid> {
    let id: Uuid = sqlx::query_scalar(
        r#"
        INSERT INTO periods (slug, label, start_year, end_year, kind, wikidata_qid)
        VALUES ($1, $2, $3, $4, $5, $6)
        ON CONFLICT (slug) DO UPDATE SET
            label = EXCLUDED.label,
            start_year = EXCLUDED.start_year,
            end_year = EXCLUDED.end_year,
            kind = EXCLUDED.kind,
            wikidata_qid = COALESCE(EXCLUDED.wikidata_qid, periods.wikidata_qid)
        RETURNING id
        "#,
    )
    .bind(slug)
    .bind(label)
    .bind(start_year)
    .bind(end_year)
    .bind(kind)
    .bind(wikidata_qid)
    .fetch_one(pool)
    .await?;
    Ok(id)
}

pub async fn list_periods(pool: &PgPool) -> anyhow::Result<Vec<PeriodRow>> {
    let rows = sqlx::query_as::<_, PeriodRow>(
        r#"
        SELECT id, slug, label, start_year, end_year, kind, wikidata_qid
        FROM periods
        ORDER BY start_year NULLS LAST, label ASC
        "#,
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn get_period_by_slug(pool: &PgPool, slug: &str) -> anyhow::Result<Option<PeriodRow>> {
    let row = sqlx::query_as::<_, PeriodRow>(
        r#"
        SELECT id, slug, label, start_year, end_year, kind, wikidata_qid
        FROM periods
        WHERE slug = $1
        "#,
    )
    .bind(slug)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

pub async fn upsert_entity_profile(
    pool: &PgPool,
    entity_id: Uuid,
    profile_slug: &str,
    profile_label: &str,
    kind: &str,
    profile_qid: Option<&str>,
    confidence: f64,
    source_system: &str,
) -> anyhow::Result<Uuid> {
    let id: Uuid = sqlx::query_scalar(
        r#"
        INSERT INTO entity_profiles (
            entity_id, profile_qid, profile_slug, profile_label, kind, confidence, source_system
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7)
        ON CONFLICT (entity_id, profile_slug, kind) DO UPDATE SET
            profile_label = EXCLUDED.profile_label,
            profile_qid = COALESCE(EXCLUDED.profile_qid, entity_profiles.profile_qid),
            confidence = EXCLUDED.confidence,
            source_system = EXCLUDED.source_system
        RETURNING id
        "#,
    )
    .bind(entity_id)
    .bind(profile_qid)
    .bind(profile_slug)
    .bind(profile_label)
    .bind(kind)
    .bind(confidence)
    .bind(source_system)
    .fetch_one(pool)
    .await?;
    Ok(id)
}

pub async fn list_entity_profiles(
    pool: &PgPool,
    entity_id: Uuid,
) -> anyhow::Result<Vec<EntityProfileRow>> {
    let rows = sqlx::query_as::<_, EntityProfileRow>(
        r#"
        SELECT id, entity_id, profile_qid, profile_slug, profile_label, kind, confidence, source_system
        FROM entity_profiles
        WHERE entity_id = $1
        ORDER BY kind ASC, profile_label ASC
        "#,
    )
    .bind(entity_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn list_profile_catalog(pool: &PgPool) -> anyhow::Result<Vec<(String, String, i64)>> {
    let rows = sqlx::query_as::<_, (String, String, i64)>(
        r#"
        SELECT profile_slug, MIN(profile_label) AS profile_label, COUNT(*)::bigint AS n
        FROM entity_profiles
        GROUP BY profile_slug
        ORDER BY n DESC, profile_slug ASC
        "#,
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn link_entity_period(
    pool: &PgPool,
    entity_id: Uuid,
    period_id: Uuid,
) -> anyhow::Result<()> {
    sqlx::query(
        r#"
        INSERT INTO entity_periods (entity_id, period_id)
        VALUES ($1, $2)
        ON CONFLICT DO NOTHING
        "#,
    )
    .bind(entity_id)
    .bind(period_id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn seed_default_periods(pool: &PgPool) -> anyhow::Result<usize> {
    let centuries: &[(i32, &str, &str)] = &[
        (15, "15th-century", "15th century"),
        (16, "16th-century", "16th century"),
        (17, "17th-century", "17th century"),
        (18, "18th-century", "18th century"),
        (19, "19th-century", "19th century"),
        (20, "20th-century", "20th century"),
        (21, "21st-century", "21st century"),
    ];
    let mut n = 0;
    for (century, slug, label) in centuries {
        let start = (century - 1) * 100 + 1;
        let end = century * 100;
        upsert_period(pool, slug, label, Some(start), Some(end), "century", None).await?;
        n += 1;
    }

    let eras = [
        ("antiquity", "Antiquity", Some(-800i32), Some(500), "era"),
        ("medieval", "Medieval period", Some(500), Some(1500), "era"),
        (
            "early-modern",
            "Early modern period",
            Some(1500),
            Some(1800),
            "era",
        ),
        (
            "contemporary",
            "Contemporary period",
            Some(1900),
            Some(2100),
            "era",
        ),
    ];
    for (slug, label, start, end, kind) in eras {
        upsert_period(pool, slug, label, start, end, kind, None).await?;
        n += 1;
    }
    Ok(n)
}

pub async fn link_entity_to_centuries(
    pool: &PgPool,
    entity_id: Uuid,
    birth_year: Option<i32>,
    death_year: Option<i32>,
) -> anyhow::Result<usize> {
    let Some(start) = birth_year.or(death_year) else {
        return Ok(0);
    };
    let end = death_year.unwrap_or(start);
    let rows = sqlx::query_as::<_, PeriodRow>(
        r#"
        SELECT id, slug, label, start_year, end_year, kind, wikidata_qid
        FROM periods
        WHERE kind = 'century'
          AND start_year IS NOT NULL
          AND end_year IS NOT NULL
          AND end_year >= $1
          AND start_year <= $2
        "#,
    )
    .bind(start)
    .bind(end)
    .fetch_all(pool)
    .await?;

    let mut n = 0;
    for period in rows {
        link_entity_period(pool, entity_id, period.id).await?;
        n += 1;
    }
    Ok(n)
}
