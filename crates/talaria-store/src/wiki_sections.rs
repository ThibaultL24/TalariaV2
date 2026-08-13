// crates/talaria-store/src/wiki_sections.rs
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct WikiSectionRecord {
    pub ordinal: i32,
    pub title: String,
    pub text: String,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct WikiSectionRow {
    pub id: Uuid,
    pub wiki_page_id: Uuid,
    pub ordinal: i32,
    pub title: String,
    pub text: String,
    pub page_title: String,
    pub wiki_lang: String,
    pub revision_id: Option<i64>,
}

pub async fn replace_sections_for_page(
    pool: &PgPool,
    wiki_page_id: Uuid,
    sections: &[WikiSectionRecord],
) -> anyhow::Result<usize> {
    let mut tx = pool.begin().await?;
    sqlx::query("DELETE FROM wiki_sections WHERE wiki_page_id = $1")
        .bind(wiki_page_id)
        .execute(&mut *tx)
        .await?;

    for section in sections {
        sqlx::query(
            r#"
            INSERT INTO wiki_sections (wiki_page_id, ordinal, title, text)
            VALUES ($1, $2, $3, $4)
            "#,
        )
        .bind(wiki_page_id)
        .bind(section.ordinal)
        .bind(&section.title)
        .bind(&section.text)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    Ok(sections.len())
}

pub async fn list_sections_for_title(
    pool: &PgPool,
    wiki_lang: &str,
    title: &str,
) -> anyhow::Result<Vec<WikiSectionRow>> {
    let rows = sqlx::query_as::<_, WikiSectionRow>(
        r#"
        SELECT
            ws.id,
            ws.wiki_page_id,
            ws.ordinal,
            ws.title,
            ws.text,
            wp.title AS page_title,
            wp.wiki_lang,
            wp.revision_id
        FROM wiki_sections ws
        INNER JOIN wiki_pages wp ON wp.id = ws.wiki_page_id
        WHERE wp.wiki_lang = $1
          AND wp.title = $2
        ORDER BY ws.ordinal ASC
        "#,
    )
    .bind(wiki_lang)
    .bind(title)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn list_pages_for_section_split(
    pool: &PgPool,
    wiki_lang: &str,
    limit: i64,
    skip_existing: bool,
) -> anyhow::Result<Vec<crate::wiki_pages::WikiPageRow>> {
    let rows = sqlx::query_as::<_, crate::wiki_pages::WikiPageRow>(
        r#"
        SELECT id, title, raw_path
        FROM wiki_pages
        WHERE wiki_lang = $1
          AND raw_path IS NOT NULL
          AND raw_path <> ''
          AND (
            NOT $2
            OR NOT EXISTS (
              SELECT 1 FROM wiki_sections ws WHERE ws.wiki_page_id = wiki_pages.id
            )
          )
        ORDER BY title ASC
        LIMIT $3
        "#,
    )
    .bind(wiki_lang)
    .bind(skip_existing)
    .bind(limit)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct HistoriographySectionRow {
    pub title: String,
    pub text: String,
    pub page_title: String,
    pub wiki_lang: String,
    pub revision_id: Option<i64>,
}

pub async fn list_sections_matching_page(
    pool: &PgPool,
    wiki_lang: &str,
    page_title: &str,
) -> anyhow::Result<Vec<HistoriographySectionRow>> {
    let rows = sqlx::query_as::<_, HistoriographySectionRow>(
        r#"
        SELECT
            ws.title,
            ws.text,
            wp.title AS page_title,
            wp.wiki_lang,
            wp.revision_id
        FROM wiki_sections ws
        INNER JOIN wiki_pages wp ON wp.id = ws.wiki_page_id
        WHERE wp.wiki_lang = $1
          AND wp.title ILIKE $2
        ORDER BY wp.title ASC, ws.ordinal ASC
        "#,
    )
    .bind(wiki_lang)
    .bind(page_title)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}
