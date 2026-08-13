// crates/talaria-store/src/corpus.rs
//! Persistence for provider-agnostic corpus documents.

use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct CorpusDocumentInsert {
    pub source_kind: String,
    pub external_id: String,
    pub canonical_url: Option<String>,
    pub document_type: String,
    pub title: String,
    pub language: Option<String>,
    pub abstract_text: Option<String>,
    pub academic_status: String,
    pub access_level: String,
    pub full_text_available: bool,
    pub rights_uri: Option<String>,
    pub rights_holder: Option<String>,
    pub rights_normalized: String,
    pub publisher_or_institution: Option<String>,
    pub publication_time: serde_json::Value,
    pub connector_version: String,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct CorpusDocumentRow {
    pub id: Uuid,
    pub source_kind: String,
    pub external_id: String,
    pub canonical_url: Option<String>,
    pub document_type: String,
    pub title: String,
    pub language: Option<String>,
    pub abstract_text: Option<String>,
    pub academic_status: String,
    pub access_level: String,
    pub full_text_available: bool,
    pub rights_uri: Option<String>,
    pub rights_holder: Option<String>,
    pub rights_normalized: String,
    pub publisher_or_institution: Option<String>,
    pub publication_time: serde_json::Value,
    pub connector_version: String,
}

pub async fn upsert_corpus_document(
    pool: &PgPool,
    doc: &CorpusDocumentInsert,
) -> anyhow::Result<(Uuid, bool)> {
    let row: Option<(Uuid,)> = sqlx::query_as(
        r#"
        INSERT INTO corpus_documents (
            source_kind, external_id, canonical_url, document_type, title, language,
            abstract_text, academic_status, access_level, full_text_available,
            rights_uri, rights_holder, rights_normalized, publisher_or_institution,
            publication_time, connector_version, retrieved_at, updated_at
        )
        VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,NOW(),NOW())
        ON CONFLICT (source_kind, external_id) DO UPDATE SET
            canonical_url = EXCLUDED.canonical_url,
            document_type = EXCLUDED.document_type,
            title = EXCLUDED.title,
            language = EXCLUDED.language,
            abstract_text = EXCLUDED.abstract_text,
            academic_status = EXCLUDED.academic_status,
            access_level = EXCLUDED.access_level,
            full_text_available = EXCLUDED.full_text_available,
            rights_uri = EXCLUDED.rights_uri,
            rights_holder = EXCLUDED.rights_holder,
            rights_normalized = EXCLUDED.rights_normalized,
            publisher_or_institution = EXCLUDED.publisher_or_institution,
            publication_time = EXCLUDED.publication_time,
            connector_version = EXCLUDED.connector_version,
            retrieved_at = NOW(),
            updated_at = NOW()
        RETURNING id
        "#,
    )
    .bind(&doc.source_kind)
    .bind(&doc.external_id)
    .bind(&doc.canonical_url)
    .bind(&doc.document_type)
    .bind(&doc.title)
    .bind(&doc.language)
    .bind(&doc.abstract_text)
    .bind(&doc.academic_status)
    .bind(&doc.access_level)
    .bind(doc.full_text_available)
    .bind(&doc.rights_uri)
    .bind(&doc.rights_holder)
    .bind(&doc.rights_normalized)
    .bind(&doc.publisher_or_institution)
    .bind(&doc.publication_time)
    .bind(&doc.connector_version)
    .fetch_optional(pool)
    .await?;

    // ON CONFLICT DO UPDATE always returns a row; treat as upserted.
    if let Some((id,)) = row {
        return Ok((id, true));
    }
    let id: Uuid = sqlx::query_scalar(
        r#"SELECT id FROM corpus_documents WHERE source_kind = $1 AND external_id = $2"#,
    )
    .bind(&doc.source_kind)
    .bind(&doc.external_id)
    .fetch_one(pool)
    .await?;
    Ok((id, false))
}

pub async fn replace_document_identifiers(
    pool: &PgPool,
    corpus_document_id: Uuid,
    idents: &[(String, String, String)],
) -> anyhow::Result<()> {
    sqlx::query("DELETE FROM document_identifiers WHERE corpus_document_id = $1")
        .bind(corpus_document_id)
        .execute(pool)
        .await?;
    for (scheme, raw, norm) in idents {
        sqlx::query(
            r#"
            INSERT INTO document_identifiers (corpus_document_id, scheme, value_raw, value_normalized)
            VALUES ($1,$2,$3,$4)
            ON CONFLICT (corpus_document_id, scheme, value_normalized) DO NOTHING
            "#,
        )
        .bind(corpus_document_id)
        .bind(scheme)
        .bind(raw)
        .bind(norm)
        .execute(pool)
        .await?;
    }
    Ok(())
}

pub async fn replace_document_contributions(
    pool: &PgPool,
    corpus_document_id: Uuid,
    rows: &[ContributionInsert],
) -> anyhow::Result<()> {
    sqlx::query("DELETE FROM document_contributions WHERE corpus_document_id = $1")
        .bind(corpus_document_id)
        .execute(pool)
        .await?;
    for row in rows {
        sqlx::query(
            r#"
            INSERT INTO document_contributions (
                corpus_document_id, role, agent_name, name_normalized,
                identifier_scheme, identifier_value, ordinal
            )
            VALUES ($1,$2,$3,$4,$5,$6,$7)
            "#,
        )
        .bind(corpus_document_id)
        .bind(&row.role)
        .bind(&row.agent_name)
        .bind(&row.name_normalized)
        .bind(&row.identifier_scheme)
        .bind(&row.identifier_value)
        .bind(row.ordinal)
        .execute(pool)
        .await?;
    }
    Ok(())
}

#[derive(Debug, Clone)]
pub struct ContributionInsert {
    pub role: String,
    pub agent_name: String,
    pub name_normalized: String,
    pub identifier_scheme: Option<String>,
    pub identifier_value: Option<String>,
    pub ordinal: i32,
}

#[derive(Debug, Clone)]
pub struct SubjectInsert {
    pub scheme: String,
    pub label: String,
    pub identifier: Option<String>,
}

pub async fn replace_document_subjects(
    pool: &PgPool,
    corpus_document_id: Uuid,
    rows: &[SubjectInsert],
) -> anyhow::Result<()> {
    sqlx::query("DELETE FROM document_subjects WHERE corpus_document_id = $1")
        .bind(corpus_document_id)
        .execute(pool)
        .await?;
    for row in rows {
        sqlx::query(
            r#"
            INSERT INTO document_subjects (corpus_document_id, scheme, label, identifier)
            VALUES ($1,$2,$3,$4)
            "#,
        )
        .bind(corpus_document_id)
        .bind(&row.scheme)
        .bind(&row.label)
        .bind(&row.identifier)
        .execute(pool)
        .await?;
    }
    Ok(())
}

pub async fn link_corpus_snapshot(
    pool: &PgPool,
    corpus_document_id: Uuid,
    snapshot_id: Uuid,
    revision_token: Option<&str>,
    content_hash: &str,
) -> anyhow::Result<()> {
    sqlx::query(
        r#"
        INSERT INTO corpus_document_snapshots (
            corpus_document_id, snapshot_id, revision_token, content_hash
        )
        VALUES ($1,$2,$3,$4)
        ON CONFLICT (corpus_document_id, snapshot_id) DO NOTHING
        "#,
    )
    .bind(corpus_document_id)
    .bind(snapshot_id)
    .bind(revision_token)
    .bind(content_hash)
    .execute(pool)
    .await?;
    Ok(())
}

#[derive(Debug, Clone)]
pub struct EntityDocumentLinkInsert {
    pub entity_id: Uuid,
    pub corpus_document_id: Uuid,
    pub relation: String,
    pub match_version: String,
    pub score: f32,
    pub components: serde_json::Value,
    pub evidence_summary: Option<String>,
}

pub async fn upsert_entity_document_link(
    pool: &PgPool,
    link: &EntityDocumentLinkInsert,
) -> anyhow::Result<Uuid> {
    let id: Uuid = sqlx::query_scalar(
        r#"
        INSERT INTO entity_document_links (
            entity_id, corpus_document_id, relation, match_version,
            score, components_json, evidence_summary
        )
        VALUES ($1,$2,$3,$4,$5,$6,$7)
        ON CONFLICT (entity_id, corpus_document_id, relation, match_version) DO UPDATE SET
            score = EXCLUDED.score,
            components_json = EXCLUDED.components_json,
            evidence_summary = EXCLUDED.evidence_summary
        RETURNING id
        "#,
    )
    .bind(link.entity_id)
    .bind(link.corpus_document_id)
    .bind(&link.relation)
    .bind(&link.match_version)
    .bind(link.score)
    .bind(&link.components)
    .bind(&link.evidence_summary)
    .fetch_one(pool)
    .await?;
    Ok(id)
}

pub async fn mark_discovered_corpus_document(
    pool: &PgPool,
    discovered_id: Uuid,
    corpus_document_id: Uuid,
) -> anyhow::Result<()> {
    sqlx::query(
        r#"
        UPDATE discovered_documents
        SET corpus_document_id = $2
        WHERE id = $1
        "#,
    )
    .bind(discovered_id)
    .bind(corpus_document_id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn get_corpus_document(
    pool: &PgPool,
    document_id: Uuid,
) -> anyhow::Result<Option<CorpusDocumentRow>> {
    let row = sqlx::query_as::<_, CorpusDocumentRow>(
        r#"
        SELECT id, source_kind, external_id, canonical_url, document_type, title, language,
               abstract_text, academic_status, access_level, full_text_available,
               rights_uri, rights_holder, rights_normalized, publisher_or_institution,
               publication_time, connector_version
        FROM corpus_documents WHERE id = $1
        "#,
    )
    .bind(document_id)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct DocumentIdentifierRow {
    pub scheme: String,
    pub value_raw: String,
    pub value_normalized: String,
}

pub async fn list_document_identifiers(
    pool: &PgPool,
    corpus_document_id: Uuid,
) -> anyhow::Result<Vec<DocumentIdentifierRow>> {
    Ok(sqlx::query_as(
        r#"
        SELECT scheme, value_raw, value_normalized
        FROM document_identifiers
        WHERE corpus_document_id = $1
        ORDER BY scheme, value_normalized
        "#,
    )
    .bind(corpus_document_id)
    .fetch_all(pool)
    .await?)
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct DocumentContributionRow {
    pub role: String,
    pub agent_name: String,
    pub identifier_scheme: Option<String>,
    pub identifier_value: Option<String>,
    pub ordinal: i32,
}

pub async fn list_document_contributions(
    pool: &PgPool,
    corpus_document_id: Uuid,
) -> anyhow::Result<Vec<DocumentContributionRow>> {
    Ok(sqlx::query_as(
        r#"
        SELECT role, agent_name, identifier_scheme, identifier_value, ordinal
        FROM document_contributions
        WHERE corpus_document_id = $1
        ORDER BY role, ordinal, agent_name
        "#,
    )
    .bind(corpus_document_id)
    .fetch_all(pool)
    .await?)
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct EntityLinkedDocumentRow {
    pub id: Uuid,
    pub source_kind: String,
    pub external_id: String,
    pub title: String,
    pub document_type: String,
    pub academic_status: String,
    pub access_level: String,
    pub full_text_available: bool,
    pub language: Option<String>,
    pub canonical_url: Option<String>,
    pub relation: String,
    pub score: f32,
    pub match_version: String,
    pub components_json: serde_json::Value,
    pub evidence_summary: Option<String>,
    pub publication_time: serde_json::Value,
}

#[derive(Debug, Clone)]
pub struct EntityDocumentsFilter<'a> {
    pub relation: Option<&'a str>,
    pub document_types: &'a [String],
    pub providers: &'a [String],
    pub academic_status: Option<&'a str>,
    pub access: Option<&'a str>,
    pub language: Option<&'a str>,
    pub limit: i64,
    pub cursor_score: Option<f32>,
    pub cursor_id: Option<Uuid>,
}

pub async fn list_entity_documents(
    pool: &PgPool,
    entity_id: Uuid,
    filter: &EntityDocumentsFilter<'_>,
) -> anyhow::Result<Vec<EntityLinkedDocumentRow>> {
    // Keyset on (score DESC, id DESC) — deterministic, no unbounded OFFSET.
    let rows = sqlx::query_as::<_, EntityLinkedDocumentRow>(
        r#"
        SELECT
            d.id, d.source_kind, d.external_id, d.title, d.document_type,
            d.academic_status, d.access_level, d.full_text_available, d.language,
            d.canonical_url, l.relation, l.score, l.match_version,
            l.components_json, l.evidence_summary, d.publication_time
        FROM entity_document_links l
        JOIN corpus_documents d ON d.id = l.corpus_document_id
        WHERE l.entity_id = $1
          AND ($2::text IS NULL OR l.relation = $2)
          AND ($3::text IS NULL OR d.academic_status = $3)
          AND ($4::text IS NULL OR d.access_level = $4)
          AND ($5::text IS NULL OR d.language = $5)
          AND (cardinality($6::text[]) = 0 OR d.document_type = ANY($6))
          AND (cardinality($7::text[]) = 0 OR d.source_kind = ANY($7))
          AND (
                $8::real IS NULL
                OR l.score < $8
                OR (l.score = $8 AND ($9::uuid IS NULL OR d.id < $9))
              )
        ORDER BY l.score DESC, d.id DESC
        LIMIT $10
        "#,
    )
    .bind(entity_id)
    .bind(filter.relation)
    .bind(filter.academic_status)
    .bind(filter.access)
    .bind(filter.language)
    .bind(filter.document_types)
    .bind(filter.providers)
    .bind(filter.cursor_score)
    .bind(filter.cursor_id)
    .bind(filter.limit)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn count_corpus_snapshots(
    pool: &PgPool,
    corpus_document_id: Uuid,
) -> anyhow::Result<i64> {
    let n: i64 = sqlx::query_scalar(
        "SELECT COUNT(*)::bigint FROM corpus_document_snapshots WHERE corpus_document_id = $1",
    )
    .bind(corpus_document_id)
    .fetch_one(pool)
    .await?;
    Ok(n)
}
