// crates/talaria-store/src/claim_graph.rs
//! Agora sources and argument graph on top of soft_claims / soft_claim_relations.

use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

pub const MAX_ARGUMENT_CHARS: usize = 4000;
pub const DEFAULT_ARGUMENT_LIMIT: i64 = 20;
pub const MAX_ARGUMENT_LIMIT: i64 = 50;

pub fn parse_argument_relation(raw: &str) -> Option<&'static str> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "supports" => Some("supports"),
        "contradicts" => Some("contradicts"),
        "qualifies" => Some("qualifies"),
        _ => None,
    }
}

pub fn normalize_argument_statement(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed.chars().count() > MAX_ARGUMENT_CHARS {
        return None;
    }
    Some(trimmed.to_string())
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ClaimSourceRow {
    pub claim_id: Uuid,
    pub corpus_document_id: Uuid,
    pub attached_by_user_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub title: String,
    pub source_kind: String,
    pub canonical_url: Option<String>,
    pub document_type: String,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ClaimCountRow {
    pub claim_id: Uuid,
    pub n: i64,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ArgumentRelationRow {
    pub relation_id: Uuid,
    pub relation: String,
    pub from_claim_id: Uuid,
    pub to_claim_id: Uuid,
    pub statement: String,
    pub claim_kind: String,
    pub origin: String,
    pub contribution_status: String,
    pub created_by_user_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub entity_id: Uuid,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct FragmentForEvidence {
    pub id: Uuid,
    pub text: String,
}

pub async fn attach_claim_source(
    pool: &PgPool,
    claim_id: Uuid,
    corpus_document_id: Uuid,
    user_id: Uuid,
) -> anyhow::Result<bool> {
    let result = sqlx::query(
        r#"
        INSERT INTO soft_claim_sources (claim_id, corpus_document_id, attached_by_user_id)
        VALUES ($1, $2, $3)
        ON CONFLICT (claim_id, corpus_document_id) DO NOTHING
        "#,
    )
    .bind(claim_id)
    .bind(corpus_document_id)
    .bind(user_id)
    .execute(pool)
    .await?;
    Ok(result.rows_affected() > 0)
}

pub async fn list_claim_sources(
    pool: &PgPool,
    claim_id: Uuid,
) -> anyhow::Result<Vec<ClaimSourceRow>> {
    let rows = sqlx::query_as::<_, ClaimSourceRow>(
        r#"
        SELECT s.claim_id, s.corpus_document_id, s.attached_by_user_id, s.created_at,
               d.title, d.source_kind, d.canonical_url, d.document_type
        FROM soft_claim_sources s
        JOIN corpus_documents d ON d.id = s.corpus_document_id
        WHERE s.claim_id = $1
        ORDER BY s.created_at ASC
        "#,
    )
    .bind(claim_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn count_sources_for_claims(
    pool: &PgPool,
    claim_ids: &[Uuid],
) -> anyhow::Result<Vec<ClaimCountRow>> {
    if claim_ids.is_empty() {
        return Ok(vec![]);
    }
    let rows = sqlx::query_as::<_, ClaimCountRow>(
        r#"
        SELECT claim_id, COUNT(*)::bigint AS n
        FROM soft_claim_sources
        WHERE claim_id = ANY($1)
        GROUP BY claim_id
        "#,
    )
    .bind(claim_ids)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn count_arguments_for_claims(
    pool: &PgPool,
    claim_ids: &[Uuid],
) -> anyhow::Result<Vec<ClaimCountRow>> {
    if claim_ids.is_empty() {
        return Ok(vec![]);
    }
    let rows = sqlx::query_as::<_, ClaimCountRow>(
        r#"
        SELECT to_claim_id AS claim_id, COUNT(*)::bigint AS n
        FROM soft_claim_relations
        WHERE to_claim_id = ANY($1)
          AND relation IN ('supports', 'contradicts', 'qualifies')
        GROUP BY to_claim_id
        "#,
    )
    .bind(claim_ids)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn insert_user_claim(
    pool: &PgPool,
    entity_id: Uuid,
    text: &str,
    user_id: Uuid,
) -> anyhow::Result<Uuid> {
    let id: Uuid = sqlx::query_scalar(
        r#"
        INSERT INTO soft_claims (
            entity_id, claim_kind, text, epistemic_status, relation_to_subject,
            confidence, origin, contribution_status, created_by_user_id
        )
        VALUES ($1, 'debate_stance', $2, 'proposed', 'historiography', 0, 'user', 'proposed', $3)
        RETURNING id
        "#,
    )
    .bind(entity_id)
    .bind(text)
    .bind(user_id)
    .fetch_one(pool)
    .await?;
    Ok(id)
}

pub async fn insert_linked_claim_evidence(
    pool: &PgPool,
    claim_id: Uuid,
    source_system: &str,
    locator: Option<&str>,
    quote: Option<&str>,
    corpus_document_id: Uuid,
    fragment_id: Option<Uuid>,
) -> anyhow::Result<Uuid> {
    let id: Uuid = sqlx::query_scalar(
        r#"
        INSERT INTO soft_claim_evidence (
            claim_id, source_system, locator, quote, sentence_id, confidence,
            corpus_document_id, fragment_id
        )
        VALUES ($1,$2,$3,$4,NULL,0,$5,$6)
        RETURNING id
        "#,
    )
    .bind(claim_id)
    .bind(source_system)
    .bind(locator)
    .bind(quote)
    .bind(corpus_document_id)
    .bind(fragment_id)
    .fetch_one(pool)
    .await?;
    Ok(id)
}

pub async fn list_arguments_for_claim(
    pool: &PgPool,
    target_claim_id: Uuid,
    limit: i64,
) -> anyhow::Result<Vec<ArgumentRelationRow>> {
    let rows = sqlx::query_as::<_, ArgumentRelationRow>(
        r#"
        SELECT r.id AS relation_id, r.relation, r.from_claim_id, r.to_claim_id,
               c.text AS statement, c.claim_kind, c.origin, c.contribution_status,
               c.created_by_user_id, c.created_at, c.entity_id
        FROM soft_claim_relations r
        JOIN soft_claims c ON c.id = r.from_claim_id
        WHERE r.to_claim_id = $1
          AND r.relation IN ('supports', 'contradicts', 'qualifies')
        ORDER BY c.created_at DESC
        LIMIT $2
        "#,
    )
    .bind(target_claim_id)
    .bind(limit)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn fragment_for_document(
    pool: &PgPool,
    fragment_id: Uuid,
    corpus_document_id: Uuid,
) -> anyhow::Result<Option<FragmentForEvidence>> {
    let row = sqlx::query_as::<_, FragmentForEvidence>(
        r#"
        SELECT f.id, f.text
        FROM document_fragments f
        JOIN corpus_document_snapshots cds ON cds.snapshot_id = f.snapshot_id
        WHERE f.id = $1 AND cds.corpus_document_id = $2
        LIMIT 1
        "#,
    )
    .bind(fragment_id)
    .bind(corpus_document_id)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        insert_claim, upsert_corpus_document, upsert_entity_with_kind, ClaimInsert,
        CorpusDocumentInsert,
    };

    async fn entity(pool: &PgPool) -> Uuid {
        upsert_entity_with_kind(pool, "en", "PR40 Person", "person")
            .await
            .unwrap()
    }

    async fn parent(pool: &PgPool, entity_id: Uuid) -> Uuid {
        insert_claim(
            pool,
            &ClaimInsert {
                entity_id,
                claim_kind: "theory".into(),
                text: "Parent theory".into(),
                epistemic_status: "attested".into(),
                relation_to_subject: "historiography".into(),
                event_time: None,
                place_label: None,
                confidence: 0.5,
                canonical_event_id: None,
                debate_type: None,
                evidence_layer: None,
            },
        )
        .await
        .unwrap()
    }

    fn doc(title: &str, ext: &str) -> CorpusDocumentInsert {
        CorpusDocumentInsert {
            source_kind: "hal".into(),
            external_id: ext.into(),
            canonical_url: Some("https://hal.example/doc".into()),
            document_type: "article".into(),
            title: title.into(),
            language: Some("fr".into()),
            abstract_text: None,
            academic_status: "unknown".into(),
            access_level: "unknown".into(),
            full_text_available: false,
            rights_uri: None,
            rights_holder: None,
            rights_normalized: "unknown".into(),
            publisher_or_institution: None,
            publication_time: serde_json::json!({"kind":"unknown"}),
            connector_version: "test".into(),
        }
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn attach_source_is_idempotent(pool: PgPool) {
        let entity_id = entity(&pool).await;
        let claim_id = parent(&pool, entity_id).await;
        let (doc_id, _) = upsert_corpus_document(&pool, &doc("Title", "ext-1"))
            .await
            .unwrap();
        let user_id = sqlx::query_scalar::<_, Uuid>(
            "INSERT INTO users (display_name) VALUES ('a') RETURNING id",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert!(attach_claim_source(&pool, claim_id, doc_id, user_id)
            .await
            .unwrap());
        assert!(!attach_claim_source(&pool, claim_id, doc_id, user_id)
            .await
            .unwrap());
        assert_eq!(list_claim_sources(&pool, claim_id).await.unwrap().len(), 1);
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn argument_relation_roundtrip(pool: PgPool) {
        let entity_id = entity(&pool).await;
        let target = parent(&pool, entity_id).await;
        let user_id = sqlx::query_scalar::<_, Uuid>(
            "INSERT INTO users (display_name) VALUES ('a') RETURNING id",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        let from = insert_user_claim(&pool, entity_id, "Support statement", user_id)
            .await
            .unwrap();
        crate::insert_claim_relation(&pool, from, target, "supports")
            .await
            .unwrap();
        let listed = list_arguments_for_claim(&pool, target, 20).await.unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].relation, "supports");
        assert_eq!(listed[0].origin, "user");
        assert_eq!(listed[0].contribution_status, "proposed");
    }
}
