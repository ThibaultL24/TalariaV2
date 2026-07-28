// crates/talaria-store/src/wiki_pages.rs
use chrono::NaiveDate;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct WikiPageRecord {
    pub page_id: i64,
    pub title: String,
    pub wiki_lang: String,
    pub revision_id: Option<i64>,
    pub content_hash: String,
    pub dump_date: Option<NaiveDate>,
    pub raw_path: String,
}

pub async fn find_content_hash(
    pool: &PgPool,
    wiki_lang: &str,
    title: &str,
) -> anyhow::Result<Option<String>> {
    let hash: Option<String> = sqlx::query_scalar(
        "SELECT content_hash FROM wiki_pages WHERE wiki_lang = $1 AND title = $2",
    )
    .bind(wiki_lang)
    .bind(title)
    .fetch_optional(pool)
    .await?;

    Ok(hash)
}

pub async fn upsert_wiki_page(pool: &PgPool, record: &WikiPageRecord) -> anyhow::Result<Uuid> {
    let id: Uuid = sqlx::query_scalar(
        r#"
        INSERT INTO wiki_pages (page_id, title, wiki_lang, revision_id, content_hash, dump_date, raw_path)
        VALUES ($1, $2, $3, $4, $5, $6, $7)
        ON CONFLICT (wiki_lang, title) DO UPDATE SET
            page_id = EXCLUDED.page_id,
            revision_id = EXCLUDED.revision_id,
            content_hash = EXCLUDED.content_hash,
            dump_date = EXCLUDED.dump_date,
            raw_path = EXCLUDED.raw_path
        RETURNING id
        "#,
    )
    .bind(record.page_id)
    .bind(&record.title)
    .bind(&record.wiki_lang)
    .bind(record.revision_id)
    .bind(&record.content_hash)
    .bind(record.dump_date)
    .bind(&record.raw_path)
    .fetch_one(pool)
    .await?;

    Ok(id)
}

pub async fn store_extracted_page(
    pool: &PgPool,
    record: &WikiPageRecord,
    skip_existing: bool,
) -> anyhow::Result<bool> {
    if skip_existing {
        if let Some(existing) = find_content_hash(pool, &record.wiki_lang, &record.title).await? {
            if existing == record.content_hash {
                return Ok(false);
            }
        }
    }

    upsert_wiki_page(pool, record).await?;
    Ok(true)
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct WikiPageRow {
    pub id: Uuid,
    pub title: String,
    pub raw_path: Option<String>,
}

pub async fn list_pages_for_sentence_split(
    pool: &PgPool,
    wiki_lang: &str,
    limit: i64,
    skip_with_sentences: bool,
) -> anyhow::Result<Vec<WikiPageRow>> {
    let rows = sqlx::query_as::<_, WikiPageRow>(
        r#"
        SELECT id, title, raw_path
        FROM wiki_pages
        WHERE wiki_lang = $1
          AND raw_path IS NOT NULL
          AND (
            $2
            OR NOT EXISTS (
              SELECT 1 FROM sentences s WHERE s.wiki_page_id = wiki_pages.id
            )
          )
        ORDER BY created_at ASC
        LIMIT $3
        "#,
    )
    .bind(wiki_lang)
    .bind(!skip_with_sentences)
    .bind(limit)
    .fetch_all(pool)
    .await?;

    Ok(rows)
}
