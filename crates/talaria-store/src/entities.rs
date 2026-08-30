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
    let folded = format!("%{}%", fold_latin_accents(query));
    let predicate = person_match_sql(1, 3);
    let sql = format!(
        r#"
        SELECT
            e.id,
            e.qid,
            e.wikipedia_title,
            e.canonical_name,
            COUNT(ce.id)::bigint AS event_count
        FROM entities e
        LEFT JOIN canonical_events ce
          ON ce.entity_id = e.id
         AND ce.is_active
         AND ce.pipeline = 'person'
        WHERE (
            e.qid ILIKE $1
            OR {predicate}
        )
          AND char_length(coalesce(e.canonical_name, e.wikipedia_title)) BETWEEN 2 AND 120
          AND coalesce(e.canonical_name, e.wikipedia_title) !~ '[=]{{2}}'
          AND coalesce(e.canonical_name, e.wikipedia_title) NOT IN ('He', 'She', 'They')
        GROUP BY e.id
        ORDER BY event_count DESC, e.canonical_name ASC NULLS LAST
        LIMIT $2
        "#
    );
    let rows = sqlx::query_as::<_, EntityRow>(&sql)
        .bind(pattern)
        .bind(limit)
        .bind(folded)
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
        LEFT JOIN canonical_events ce
          ON ce.entity_id = e.id
         AND ce.is_active
         AND ce.pipeline = 'person'
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
        LEFT JOIN canonical_events ce
          ON ce.entity_id = e.id
         AND ce.is_active
         AND ce.pipeline = 'person'
        WHERE e.qid ILIKE $1
        GROUP BY e.id
        ORDER BY event_count DESC, e.created_at ASC
        LIMIT 1
        "#,
    )
    .bind(qid)
    .fetch_optional(pool)
    .await?;

    Ok(row)
}

pub async fn update_entity_qid(pool: &PgPool, entity_id: Uuid, qid: &str) -> anyhow::Result<()> {
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

/// Trim, uppercase, and validate `Q` + digits (e.g. `"q517"` → `"Q517"`).
pub fn normalize_qid(qid: &str) -> Option<String> {
    let trimmed = qid.trim();
    if trimmed.is_empty() {
        return None;
    }
    let upper = trimmed.to_uppercase();
    let digits = upper.strip_prefix('Q')?;
    if digits.is_empty() || !digits.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    Some(upper)
}

async fn insert_entity_alias(
    pool: &PgPool,
    entity_id: Uuid,
    surface: &str,
    language: &str,
) -> anyhow::Result<()> {
    let surface = surface.trim();
    if surface.is_empty() {
        return Ok(());
    }
    sqlx::query(
        r#"
        INSERT INTO entity_aliases (entity_id, surface, language)
        VALUES ($1, $2, $3)
        ON CONFLICT (language, surface, entity_id) DO NOTHING
        "#,
    )
    .bind(entity_id)
    .bind(surface)
    .bind(language)
    .execute(pool)
    .await?;
    Ok(())
}

fn typed_surface_is_distinct(
    typed_surface: &str,
    wikipedia_title: &str,
    wikidata_label: &str,
    canonical_name: Option<&str>,
) -> bool {
    let surface = typed_surface.trim();
    if surface.is_empty() {
        return false;
    }
    let matches = |other: &str| !other.trim().is_empty() && surface.eq_ignore_ascii_case(other.trim());
    !matches(wikipedia_title)
        && !matches(wikidata_label)
        && !canonical_name.is_some_and(|name| matches(name))
}

/// Resolve or create a person entity by Wikidata QID; record the user-typed surface as an alias.
pub async fn upsert_person_by_qid(
    pool: &PgPool,
    qid: &str,
    wikidata_label: &str,
    wiki_lang: &str,
    wikipedia_title: &str,
    typed_surface: &str,
) -> anyhow::Result<Uuid> {
    let qid = normalize_qid(qid).ok_or_else(|| anyhow::anyhow!("invalid qid: {qid}"))?;

    if let Some(existing) = find_entity_by_qid(pool, &qid).await? {
        if typed_surface_is_distinct(
            typed_surface,
            &existing.wikipedia_title,
            wikidata_label,
            existing.canonical_name.as_deref(),
        ) {
            insert_entity_alias(pool, existing.id, typed_surface, wiki_lang).await?;
        }
        return Ok(existing.id);
    }

    let id = upsert_entity_from_wikidata(
        pool,
        &qid,
        wikidata_label,
        wiki_lang,
        wikipedia_title,
    )
    .await?;
    if typed_surface_is_distinct(
        typed_surface,
        wikipedia_title,
        wikidata_label,
        Some(wikidata_label),
    ) {
        insert_entity_alias(pool, id, typed_surface, wiki_lang).await?;
    }
    Ok(id)
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
    if let Some(existing) = find_entity_by_wikipedia_title(pool, wiki_lang, wikipedia_title).await?
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

pub fn fold_latin_accents(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .map(|c| match c {
            'à' | 'á' | 'â' | 'ä' | 'ã' | 'å' | 'ā' => 'a',
            'è' | 'é' | 'ê' | 'ë' | 'ē' => 'e',
            'ì' | 'í' | 'î' | 'ï' | 'ī' => 'i',
            'ò' | 'ó' | 'ô' | 'ö' | 'õ' | 'ø' | 'ō' => 'o',
            'ù' | 'ú' | 'û' | 'ü' | 'ū' => 'u',
            'ý' | 'ÿ' => 'y',
            'ñ' => 'n',
            'ç' => 'c',
            'ł' => 'l',
            'ś' => 's',
            'ź' | 'ż' => 'z',
            _ => c,
        })
        .collect()
}

const ACCENT_FROM: &str = "àáâäãåāèéêëēìíîïīòóôöõøōùúûüūýÿñçłśźż";
const ACCENT_TO: &str = "aaaaaaaeeeeeiiiiioooooouuuuuyynclszz";

pub fn person_match_sql(pattern_n: usize, folded_n: usize) -> String {
    format!(
        "( \
            ${p}::text IS NULL \
            OR e.wikipedia_title ILIKE ${p} \
            OR e.canonical_name ILIKE ${p} \
            OR translate(lower(coalesce(e.canonical_name, '')), '{from}', '{to}') LIKE ${f} \
            OR translate(lower(e.wikipedia_title), '{from}', '{to}') LIKE ${f} \
            OR EXISTS ( \
                SELECT 1 FROM entity_aliases ea \
                WHERE ea.entity_id = e.id \
                  AND (ea.surface ILIKE ${p} \
                       OR translate(lower(ea.surface), '{from}', '{to}') LIKE ${f}) \
            ) \
        )",
        p = pattern_n,
        f = folded_n,
        from = ACCENT_FROM,
        to = ACCENT_TO,
    )
}

#[cfg(test)]
mod tests {
    use super::{fold_latin_accents, normalize_qid};

    #[test]
    fn normalize_qid_accepts_lowercase() {
        assert_eq!(normalize_qid("q517").as_deref(), Some("Q517"));
        assert_eq!(normalize_qid(" Q517 ").as_deref(), Some("Q517"));
    }

    #[test]
    fn normalize_qid_rejects_invalid() {
        assert_eq!(normalize_qid("LotD"), None);
        assert_eq!(normalize_qid(""), None);
        assert_eq!(normalize_qid("   "), None);
        assert_eq!(normalize_qid("Q"), None);
        assert_eq!(normalize_qid("Q517abc"), None);
    }

    #[test]
    fn folds_honore_de_balzac() {
        assert_eq!(fold_latin_accents("Honoré de Balzac"), "honore de balzac");
        assert_eq!(fold_latin_accents("Honore de Balzac"), "honore de balzac");
        assert_eq!(fold_latin_accents("Léopoldine"), "leopoldine");
    }

    #[test]
    fn person_lookup_sql_mentions_aliases_and_fold() {
        let sql = super::person_match_sql(2, 3);
        assert!(sql.contains("entity_aliases"));
        assert!(sql.contains("translate"));
    }

    #[test]
    fn local_search_counts_person_pipeline_only() {
        let src = include_str!("entities.rs");
        assert!(src.contains("ce.pipeline = 'person'"));
        assert!(src.contains("AND ce.is_active"));
    }
}
