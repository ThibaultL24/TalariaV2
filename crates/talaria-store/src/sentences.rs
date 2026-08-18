// crates/talaria-store/src/sentences.rs
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct SentenceRecord {
    pub ordinal: i32,
    pub text: String,
    pub char_start: Option<i32>,
    pub char_end: Option<i32>,
}

pub async fn page_has_sentences(pool: &PgPool, wiki_page_id: Uuid) -> anyhow::Result<bool> {
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM sentences WHERE wiki_page_id = $1)",
    )
    .bind(wiki_page_id)
    .fetch_one(pool)
    .await?;

    Ok(exists)
}

pub async fn replace_sentences_for_page(
    pool: &PgPool,
    wiki_page_id: Uuid,
    sentences: &[SentenceRecord],
) -> anyhow::Result<usize> {
    let mut tx = pool.begin().await?;
    sqlx::query("DELETE FROM sentences WHERE wiki_page_id = $1")
        .bind(wiki_page_id)
        .execute(&mut *tx)
        .await?;

    for sentence in sentences {
        sqlx::query(
            r#"
            INSERT INTO sentences (wiki_page_id, ordinal, text, char_start, char_end)
            VALUES ($1, $2, $3, $4, $5)
            "#,
        )
        .bind(wiki_page_id)
        .bind(sentence.ordinal)
        .bind(&sentence.text)
        .bind(sentence.char_start)
        .bind(sentence.char_end)
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;
    Ok(sentences.len())
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct SentenceRow {
    pub id: Uuid,
    pub text: String,
    pub page_title: String,
}

pub async fn list_sentences_for_extraction(
    pool: &PgPool,
    wiki_lang: &str,
    limit: i64,
    skip_with_candidates: bool,
) -> anyhow::Result<Vec<SentenceRow>> {
    let rows = sqlx::query_as::<_, SentenceRow>(
        r#"
        SELECT s.id, s.text, wp.title AS page_title
        FROM sentences s
        INNER JOIN wiki_pages wp ON wp.id = s.wiki_page_id
        WHERE wp.wiki_lang = $1
          AND (
            $2
            OR NOT EXISTS (
              SELECT 1 FROM phrase_candidates pc WHERE pc.sentence_id = s.id
            )
          )
        ORDER BY wp.title ASC, s.ordinal ASC
        LIMIT $3
        "#,
    )
    .bind(wiki_lang)
    .bind(!skip_with_candidates)
    .bind(limit)
    .fetch_all(pool)
    .await?;

    Ok(rows)
}

/// All dump sentences, including those already mined by COSMOS/mock.
pub async fn list_sentences_for_dump_mine(
    pool: &PgPool,
    wiki_lang: &str,
    limit: i64,
) -> anyhow::Result<Vec<SentenceRow>> {
    let rows = sqlx::query_as::<_, SentenceRow>(
        r#"
        SELECT s.id, s.text, wp.title AS page_title
        FROM sentences s
        INNER JOIN wiki_pages wp ON wp.id = s.wiki_page_id
        WHERE wp.wiki_lang = $1
        ORDER BY wp.title ASC, s.ordinal ASC
        LIMIT $2
        "#,
    )
    .bind(wiki_lang)
    .bind(limit)
    .fetch_all(pool)
    .await?;

    Ok(rows)
}
