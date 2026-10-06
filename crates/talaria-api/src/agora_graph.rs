// crates/talaria-api/src/agora_graph.rs
//! Structured Agora sources and arguments. Off-chain; never publishes to Intuition.

use crate::agora_stance::is_stance_claim;
use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::PgPool;
use std::collections::HashMap;
use talaria_store::{
    attach_claim_source, fragment_for_document, get_claim, get_comment, get_corpus_document,
    insert_claim_relation, insert_linked_claim_evidence, insert_user_claim,
    list_arguments_for_claim, list_claim_evidence_for_claims, list_claim_sources,
    list_comment_authors, normalize_argument_statement, parse_argument_relation,
    ArgumentRelationRow, ClaimSourceRow, DEFAULT_ARGUMENT_LIMIT, MAX_ARGUMENT_LIMIT,
};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgoraGraphError {
    Unauthenticated,
    ClaimNotFound,
    ClaimNotDiscussable,
    CommentNotFound,
    SourceNotFound,
    SourceAlreadyAttached,
    MissingSource,
    MissingEvidence,
    InvalidRelation,
    InvalidArgument,
    DatabaseError,
}

impl AgoraGraphError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Unauthenticated => "unauthenticated",
            Self::ClaimNotFound => "claim_not_found",
            Self::ClaimNotDiscussable => "claim_not_discussable",
            Self::CommentNotFound => "comment_not_found",
            Self::SourceNotFound => "source_not_found",
            Self::SourceAlreadyAttached => "source_already_attached",
            Self::MissingSource => "missing_source",
            Self::MissingEvidence => "missing_evidence",
            Self::InvalidRelation => "invalid_relation",
            Self::InvalidArgument => "invalid_argument",
            Self::DatabaseError => "database_error",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ClaimSourceDto {
    pub corpus_document_id: Uuid,
    pub title: String,
    pub source_kind: String,
    pub canonical_url: Option<String>,
    pub document_type: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ArgumentEvidenceDto {
    pub id: Uuid,
    pub quote: Option<String>,
    pub locator: Option<String>,
    pub source_system: String,
    pub corpus_document_id: Option<Uuid>,
    pub fragment_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ArgumentDto {
    pub id: Uuid,
    pub target_claim_id: Uuid,
    pub relation: String,
    pub statement: String,
    pub origin: String,
    pub contribution_status: String,
    pub created_at: DateTime<Utc>,
    pub author: Option<talaria_store::CommentAuthorRow>,
    pub sources: Vec<ClaimSourceDto>,
    pub evidence: Vec<ArgumentEvidenceDto>,
}

fn map_db<E>(_: E) -> AgoraGraphError {
    AgoraGraphError::DatabaseError
}

async fn require_discussable(
    pool: &PgPool,
    claim_id: Uuid,
) -> Result<talaria_store::ClaimRow, AgoraGraphError> {
    let claim = get_claim(pool, claim_id)
        .await
        .map_err(map_db)?
        .ok_or(AgoraGraphError::ClaimNotFound)?;
    if !is_stance_claim(&claim.claim_kind) {
        return Err(AgoraGraphError::ClaimNotDiscussable);
    }
    Ok(claim)
}

fn source_dto(row: &ClaimSourceRow) -> ClaimSourceDto {
    ClaimSourceDto {
        corpus_document_id: row.corpus_document_id,
        title: row.title.clone(),
        source_kind: row.source_kind.clone(),
        canonical_url: row.canonical_url.clone(),
        document_type: row.document_type.clone(),
        created_at: row.created_at,
    }
}

pub async fn list_sources(
    pool: &PgPool,
    claim_id: Uuid,
) -> Result<Vec<ClaimSourceDto>, AgoraGraphError> {
    require_discussable(pool, claim_id).await?;
    let rows = list_claim_sources(pool, claim_id).await.map_err(map_db)?;
    Ok(rows.iter().map(source_dto).collect())
}

pub async fn add_source(
    pool: &PgPool,
    user_id: Uuid,
    claim_id: Uuid,
    corpus_document_id: Uuid,
) -> Result<(ClaimSourceDto, bool), AgoraGraphError> {
    require_discussable(pool, claim_id).await?;
    get_corpus_document(pool, corpus_document_id)
        .await
        .map_err(map_db)?
        .ok_or(AgoraGraphError::SourceNotFound)?;
    let created = attach_claim_source(pool, claim_id, corpus_document_id, user_id)
        .await
        .map_err(map_db)?;
    let rows = list_claim_sources(pool, claim_id).await.map_err(map_db)?;
    let dto = rows
        .iter()
        .find(|row| row.corpus_document_id == corpus_document_id)
        .map(source_dto)
        .ok_or(AgoraGraphError::DatabaseError)?;
    Ok((dto, created))
}

pub async fn list_arguments(
    pool: &PgPool,
    claim_id: Uuid,
    limit: Option<i64>,
) -> Result<Vec<ArgumentDto>, AgoraGraphError> {
    require_discussable(pool, claim_id).await?;
    let limit = limit
        .unwrap_or(DEFAULT_ARGUMENT_LIMIT)
        .clamp(1, MAX_ARGUMENT_LIMIT);
    let rows = list_arguments_for_claim(pool, claim_id, limit)
        .await
        .map_err(map_db)?;
    assemble_arguments(pool, rows).await
}

async fn assemble_arguments(
    pool: &PgPool,
    rows: Vec<ArgumentRelationRow>,
) -> Result<Vec<ArgumentDto>, AgoraGraphError> {
    if rows.is_empty() {
        return Ok(vec![]);
    }
    let ids: Vec<Uuid> = rows.iter().map(|r| r.from_claim_id).collect();
    let mut sources: HashMap<Uuid, Vec<ClaimSourceDto>> = HashMap::new();
    for id in &ids {
        let listed = list_claim_sources(pool, *id).await.map_err(map_db)?;
        sources.insert(*id, listed.iter().map(source_dto).collect());
    }
    let evidence_rows = list_claim_evidence_for_claims(pool, &ids)
        .await
        .map_err(map_db)?;
    let mut evidence: HashMap<Uuid, Vec<ArgumentEvidenceDto>> = HashMap::new();
    for row in evidence_rows {
        evidence.entry(row.claim_id).or_default().push(ArgumentEvidenceDto {
            id: row.id,
            quote: row.quote.clone(),
            locator: row.locator.clone(),
            source_system: row.source_system.clone(),
            corpus_document_id: row.corpus_document_id,
            fragment_id: row.fragment_id,
        });
    }
    let author_ids: Vec<Uuid> = rows.iter().filter_map(|r| r.created_by_user_id).collect();
    let authors = list_comment_authors(pool, &author_ids)
        .await
        .map_err(map_db)?;
    let mut author_map = HashMap::new();
    for author in authors {
        author_map.insert(author.id, author);
    }
    Ok(rows
        .into_iter()
        .map(|row| ArgumentDto {
            id: row.from_claim_id,
            target_claim_id: row.to_claim_id,
            relation: row.relation,
            statement: row.statement,
            origin: row.origin,
            contribution_status: row.contribution_status,
            created_at: row.created_at,
            author: row
                .created_by_user_id
                .and_then(|id| author_map.get(&id).cloned()),
            sources: sources.get(&row.from_claim_id).cloned().unwrap_or_default(),
            evidence: evidence.remove(&row.from_claim_id).unwrap_or_default(),
        })
        .collect())
}

pub struct CreateArgumentInput {
    pub relation: String,
    pub statement: String,
    pub corpus_document_id: Option<Uuid>,
    pub fragment_id: Option<Uuid>,
    pub quote: Option<String>,
}

pub async fn create_argument(
    pool: &PgPool,
    user_id: Uuid,
    target_claim_id: Uuid,
    input: CreateArgumentInput,
) -> Result<ArgumentDto, AgoraGraphError> {
    let parent = require_discussable(pool, target_claim_id).await?;
    let relation =
        parse_argument_relation(&input.relation).ok_or(AgoraGraphError::InvalidRelation)?;
    let statement =
        normalize_argument_statement(&input.statement).ok_or(AgoraGraphError::InvalidArgument)?;
    let doc_id = input.corpus_document_id.ok_or(AgoraGraphError::MissingSource)?;
    let document = get_corpus_document(pool, doc_id)
        .await
        .map_err(map_db)?
        .ok_or(AgoraGraphError::SourceNotFound)?;

    let (quote, fragment_id) = if let Some(fid) = input.fragment_id {
        let frag = fragment_for_document(pool, fid, doc_id)
            .await
            .map_err(map_db)?
            .ok_or(AgoraGraphError::MissingEvidence)?;
        (frag.text, Some(frag.id))
    } else {
        let quote = input
            .quote
            .as_deref()
            .and_then(normalize_argument_statement)
            .ok_or(AgoraGraphError::MissingEvidence)?;
        (quote, None)
    };

    let from_id = insert_user_claim(pool, parent.entity_id, &statement, user_id)
        .await
        .map_err(map_db)?;
    if from_id == target_claim_id {
        return Err(AgoraGraphError::InvalidArgument);
    }
    attach_claim_source(pool, from_id, doc_id, user_id)
        .await
        .map_err(map_db)?;
    insert_linked_claim_evidence(
        pool,
        from_id,
        &document.source_kind,
        document.canonical_url.as_deref(),
        Some(&quote),
        doc_id,
        fragment_id,
    )
    .await
    .map_err(map_db)?;
    insert_claim_relation(pool, from_id, target_claim_id, relation)
        .await
        .map_err(map_db)?;

    list_arguments(pool, target_claim_id, Some(MAX_ARGUMENT_LIMIT))
        .await?
        .into_iter()
        .find(|item| item.id == from_id)
        .ok_or(AgoraGraphError::DatabaseError)
}

pub async fn promote_comment(
    pool: &PgPool,
    user_id: Uuid,
    comment_id: Uuid,
    input: CreateArgumentInput,
) -> Result<ArgumentDto, AgoraGraphError> {
    let comment = get_comment(pool, comment_id)
        .await
        .map_err(map_db)?
        .ok_or(AgoraGraphError::CommentNotFound)?;
    if comment.user_id != user_id || comment.status != "active" {
        return Err(AgoraGraphError::CommentNotFound);
    }
    let mut input = input;
    if input.statement.trim().is_empty() {
        input.statement = comment.body.clone();
    }
    create_argument(pool, user_id, comment.claim_id, input).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use talaria_store::{
        insert_claim, insert_comment, resolve_or_create_evm_user, upsert_corpus_document,
        upsert_entity_with_kind, ClaimInsert, CorpusDocumentInsert, NormalizedEvmAddress,
    };

    async fn user(pool: &PgPool) -> talaria_store::UserRow {
        resolve_or_create_evm_user(
            pool,
            &NormalizedEvmAddress::parse("0xab5801a7d398351b8be11c439e05c5b3259aec9b").unwrap(),
        )
        .await
        .unwrap()
    }

    async fn theory(pool: &PgPool) -> Uuid {
        let entity = upsert_entity_with_kind(pool, "en", "PR40 Person", "person")
            .await
            .unwrap();
        insert_claim(
            pool,
            &ClaimInsert {
                entity_id: entity,
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

    fn doc(ext: &str) -> CorpusDocumentInsert {
        CorpusDocumentInsert {
            source_kind: "hal".into(),
            external_id: ext.into(),
            canonical_url: Some("https://hal.example/doc".into()),
            document_type: "article".into(),
            title: "Indexed article".into(),
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

    fn input(relation: &str, doc_id: Uuid) -> CreateArgumentInput {
        CreateArgumentInput {
            relation: relation.into(),
            statement: "La dégradation observée".into(),
            corpus_document_id: Some(doc_id),
            fragment_id: None,
            quote: Some("citation from the article".into()),
        }
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn add_source_valid_and_idempotent(pool: PgPool) {
        let u = user(&pool).await;
        let claim = theory(&pool).await;
        let (doc_id, _) = upsert_corpus_document(&pool, &doc("a")).await.unwrap();
        let (_, created) = add_source(&pool, u.id, claim, doc_id).await.unwrap();
        assert!(created);
        let (_, again) = add_source(&pool, u.id, claim, doc_id).await.unwrap();
        assert!(!again);
        assert_eq!(list_sources(&pool, claim).await.unwrap().len(), 1);
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn source_nonexistent_rejected(pool: PgPool) {
        let u = user(&pool).await;
        let claim = theory(&pool).await;
        let err = add_source(&pool, u.id, claim, Uuid::new_v4())
            .await
            .unwrap_err();
        assert_eq!(err, AgoraGraphError::SourceNotFound);
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn argument_without_source_or_evidence(pool: PgPool) {
        let u = user(&pool).await;
        let claim = theory(&pool).await;
        let (doc_id, _) = upsert_corpus_document(&pool, &doc("b")).await.unwrap();
        let mut missing_src = input("supports", doc_id);
        missing_src.corpus_document_id = None;
        assert_eq!(
            create_argument(&pool, u.id, claim, missing_src)
                .await
                .unwrap_err(),
            AgoraGraphError::MissingSource
        );
        let mut missing_ev = input("supports", doc_id);
        missing_ev.quote = None;
        assert_eq!(
            create_argument(&pool, u.id, claim, missing_ev)
                .await
                .unwrap_err(),
            AgoraGraphError::MissingEvidence
        );
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn invalid_relation_and_non_agora(pool: PgPool) {
        let u = user(&pool).await;
        let claim = theory(&pool).await;
        let (doc_id, _) = upsert_corpus_document(&pool, &doc("c")).await.unwrap();
        assert_eq!(
            create_argument(&pool, u.id, claim, input("debates", doc_id))
                .await
                .unwrap_err(),
            AgoraGraphError::InvalidRelation
        );
        let entity = upsert_entity_with_kind(&pool, "en", "Other", "person")
            .await
            .unwrap();
        let life = insert_claim(
            &pool,
            &ClaimInsert {
                entity_id: entity,
                claim_kind: "life_event".into(),
                text: "Born".into(),
                epistemic_status: "attested".into(),
                relation_to_subject: "direct".into(),
                event_time: None,
                place_label: None,
                confidence: 0.5,
                canonical_event_id: None,
                debate_type: None,
                evidence_layer: None,
            },
        )
        .await
        .unwrap();
        assert_eq!(
            create_argument(&pool, u.id, life, input("supports", doc_id))
                .await
                .unwrap_err(),
            AgoraGraphError::ClaimNotDiscussable
        );
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn supports_contradicts_qualifies(pool: PgPool) {
        let u = user(&pool).await;
        let claim = theory(&pool).await;
        let (doc_id, _) = upsert_corpus_document(&pool, &doc("d")).await.unwrap();
        for rel in ["supports", "contradicts", "qualifies"] {
            let dto = create_argument(&pool, u.id, claim, input(rel, doc_id))
                .await
                .unwrap();
            assert_eq!(dto.relation, rel);
            assert_eq!(dto.origin, "user");
            assert_eq!(dto.contribution_status, "proposed");
            assert_eq!(dto.sources.len(), 1);
            assert_eq!(dto.evidence.len(), 1);
        }
        assert_eq!(list_arguments(&pool, claim, None).await.unwrap().len(), 3);
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn promote_comment_keeps_comment(pool: PgPool) {
        let u = user(&pool).await;
        let claim = theory(&pool).await;
        let (doc_id, _) = upsert_corpus_document(&pool, &doc("e")).await.unwrap();
        let comment = insert_comment(&pool, u.id, claim, None, "Promote me")
            .await
            .unwrap();
        let dto = promote_comment(
            &pool,
            u.id,
            comment.id,
            CreateArgumentInput {
                relation: "supports".into(),
                statement: String::new(),
                corpus_document_id: Some(doc_id),
                fragment_id: None,
                quote: Some("quote".into()),
            },
        )
        .await
        .unwrap();
        assert_eq!(dto.statement, "Promote me");
        let still = get_comment(&pool, comment.id).await.unwrap().unwrap();
        assert_eq!(still.body, "Promote me");
        assert_eq!(still.status, "active");
        assert_ne!(dto.id, comment.id);
        assert_eq!(dto.origin, "user");
        assert_eq!(dto.contribution_status, "proposed");
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn layers_stay_separate(pool: PgPool) {
        let u = user(&pool).await;
        let claim = theory(&pool).await;
        let (doc_id, _) = upsert_corpus_document(&pool, &doc("layer")).await.unwrap();
        let comment = insert_comment(&pool, u.id, claim, None, "A comment is not an argument")
            .await
            .unwrap();
        let argument = create_argument(&pool, u.id, claim, input("supports", doc_id))
            .await
            .unwrap();
        assert_ne!(comment.id, argument.id);
        assert_ne!(comment.body.as_str(), argument.statement.as_str());
        let listed = list_arguments(&pool, claim, None).await.unwrap();
        assert!(listed.iter().all(|row| row.id != comment.id));
        let sources = list_sources(&pool, claim).await.unwrap();
        assert!(sources
            .iter()
            .all(|row| row.corpus_document_id != comment.id));
        assert_eq!(argument.origin, "user");
        let still_comment = get_comment(&pool, comment.id).await.unwrap().unwrap();
        assert_eq!(still_comment.status, "active");
    }
}
