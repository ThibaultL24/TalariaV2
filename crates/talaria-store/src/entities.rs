// crates/talaria-store/src/entities.rs
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct EntityRow {
    pub id: Uuid,
    pub qid: Option<String>,
    pub wikipedia_title: String,
    pub canonical_name: Option<String>,
    pub event_count: i64,
}

pub async fn upsert_entity_surface(
    pool: &PgPool,
    wiki_lang: &str,
    person_surface: &str,
) -> anyhow::Result<Uuid> {
    let id: Uuid = sqlx::query_scalar(
        r#"
        INSERT INTO entities (wikipedia_title, wiki_lang, canonical_name)
        VALUES ($1, $2, $3)
        ON CONFLICT (wiki_lang, wikipedia_title) DO UPDATE SET
            canonical_name = EXCLUDED.canonical_name
        RETURNING id
        "#,
    )
    .bind(person_surface)
    .bind(wiki_lang)
    .bind(person_surface)
    .fetch_one(pool)
    .await?;

    Ok(id)
}

pub async fn search_local_entities(
    pool: &PgPool,
    query: &str,
    limit: i64,
) -> anyhow::Result<Vec<EntityRow>> {
    let pattern = format!("%{query}%");
    let rows = sqlx::query_as::<_, EntityRow>(
        r#"
        SELECT
            e.id,
            e.qid,
            e.wikipedia_title,
            e.canonical_name,
            COUNT(ce.id)::bigint AS event_count
        FROM entities e
        LEFT JOIN canonical_events ce ON ce.entity_id = e.id
        WHERE e.canonical_name ILIKE $1
           OR e.wikipedia_title ILIKE $1
           OR e.qid ILIKE $1
        GROUP BY e.id
        ORDER BY event_count DESC, e.canonical_name ASC NULLS LAST
        LIMIT $2
        "#,
    )
    .bind(pattern)
    .bind(limit)
    .fetch_all(pool)
    .await?;

    Ok(rows)
}

pub async fn get_entity(pool: &PgPool, entity_id: Uuid) -> anyhow::Result<Option<EntityRow>> {
    let row = sqlx::query_as::<_, EntityRow>(
        r#"
        SELECT
            e.id,
            e.qid,
            e.wikipedia_title,
            e.canonical_name,
            COUNT(ce.id)::bigint AS event_count
        FROM entities e
        LEFT JOIN canonical_events ce ON ce.entity_id = e.id
        WHERE e.id = $1
        GROUP BY e.id
        "#,
    )
    .bind(entity_id)
    .fetch_optional(pool)
    .await?;

    Ok(row)
}

pub async fn find_entity_by_qid(pool: &PgPool, qid: &str) -> anyhow::Result<Option<EntityRow>> {
    let row = sqlx::query_as::<_, EntityRow>(
        r#"
        SELECT
            e.id,
            e.qid,
            e.wikipedia_title,
            e.canonical_name,
            COUNT(ce.id)::bigint AS event_count
        FROM entities e
        LEFT JOIN canonical_events ce ON ce.entity_id = e.id
        WHERE e.qid ILIKE $1
        GROUP BY e.id
        LIMIT 1
        "#,
    )
    .bind(qid)
    .fetch_optional(pool)
    .await?;

    Ok(row)
}

pub async fn update_entity_qid(
    pool: &PgPool,
    entity_id: Uuid,
    qid: &str,
) -> anyhow::Result<()> {
    sqlx::query("UPDATE entities SET qid = $2 WHERE id = $1")
        .bind(entity_id)
        .bind(qid)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn find_entity_by_wikipedia_title(
    pool: &PgPool,
    wiki_lang: &str,
    wikipedia_title: &str,
) -> anyhow::Result<Option<EntityRow>> {
    let row = sqlx::query_as::<_, EntityRow>(
        r#"
        SELECT
            e.id,
            e.qid,
            e.wikipedia_title,
            e.canonical_name,
            COUNT(ce.id)::bigint AS event_count
        FROM entities e
        LEFT JOIN canonical_events ce ON ce.entity_id = e.id
        WHERE e.wiki_lang = $1
          AND e.wikipedia_title = $2
        GROUP BY e.id
        LIMIT 1
        "#,
    )
    .bind(wiki_lang)
    .bind(wikipedia_title)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

/// Resolve entity by QID, then by Wikipedia sitelink; create surface if missing.
pub async fn upsert_entity_from_wikidata(
    pool: &PgPool,
    qid: &str,
    label: &str,
    wiki_lang: &str,
    wikipedia_title: &str,
) -> anyhow::Result<Uuid> {
    if let Some(existing) = find_entity_by_qid(pool, qid).await? {
        return Ok(existing.id);
    }
    if let Some(existing) =
        find_entity_by_wikipedia_title(pool, wiki_lang, wikipedia_title).await?
    {
        update_entity_qid(pool, existing.id, qid).await?;
        sqlx::query(
            r#"
            UPDATE entities
            SET canonical_name = COALESCE(canonical_name, $2)
            WHERE id = $1
            "#,
        )
        .bind(existing.id)
        .bind(label)
        .execute(pool)
        .await?;
        return Ok(existing.id);
    }

    let id = upsert_entity_surface(pool, wiki_lang, wikipedia_title).await?;
    update_entity_qid(pool, id, qid).await?;
    sqlx::query("UPDATE entities SET canonical_name = $2 WHERE id = $1")
        .bind(id)
        .bind(label)
        .execute(pool)
        .await?;
    Ok(id)
}
