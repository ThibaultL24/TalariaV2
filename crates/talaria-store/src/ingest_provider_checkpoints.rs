//! Source-level checkpoints for a single durable Agora ingest job.
//! All writes are fenced by the currently valid worker lease.
use serde_json::Value;
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

#[derive(Debug, Clone, FromRow)]
pub struct CompletedProvider {
    pub source_kind: String,
    pub connector_version: String,
    pub metrics_json: Value,
}

#[derive(Debug, Clone, FromRow)]
pub struct CompletedDocument {
    pub source_kind: String,
    pub external_id: String,
    pub corpus_document_id: Uuid,
}

pub async fn completed_providers(
    pool: &PgPool, job_id: Uuid,
) -> Result<Vec<CompletedProvider>, sqlx::Error> {
    sqlx::query_as::<_,CompletedProvider>(
        "SELECT source_kind,connector_version,metrics_json
         FROM ingest_provider_checkpoints WHERE job_id=$1 ORDER BY source_kind"
    ).bind(job_id).fetch_all(pool).await
}

pub async fn completed_documents(
    pool: &PgPool, job_id: Uuid,
) -> Result<Vec<CompletedDocument>, sqlx::Error> {
    sqlx::query_as::<_,CompletedDocument>(
        "SELECT source_kind,external_id,corpus_document_id
         FROM ingest_job_documents WHERE job_id=$1 ORDER BY source_kind,external_id"
    ).bind(job_id).fetch_all(pool).await
}

pub async fn ensure_lease(
    pool: &PgPool, job_id: Uuid, owner: Uuid,
) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar::<_,bool>(
        "SELECT EXISTS(SELECT 1 FROM ingest_jobs
         WHERE id=$1 AND owner_token=$2 AND status='running' AND lease_until>now())"
    ).bind(job_id).bind(owner).fetch_one(pool).await
}

/// Fenced ledger write: if the lease is lost, it fails instead of allowing
/// an old worker to claim a document saved by a replacement worker.
pub async fn record_document(
    pool: &PgPool, job_id: Uuid, owner: Uuid, source: &str,
    external_id: &str, corpus_id: Uuid,
) -> Result<bool, sqlx::Error> {
    let inserted: Option<Uuid> = sqlx::query_scalar(
        "INSERT INTO ingest_job_documents (job_id,source_kind,external_id,corpus_document_id)
         SELECT j.id,$3,$4,$5 FROM ingest_jobs j
         WHERE j.id=$1 AND j.owner_token=$2 AND j.status='running' AND j.lease_until>now()
         ON CONFLICT (job_id,source_kind,external_id) DO NOTHING RETURNING corpus_document_id"
    ).bind(job_id).bind(owner).bind(source).bind(external_id)
      .bind(corpus_id).fetch_optional(pool).await?;
    if inserted.is_some() { return Ok(true); }
    if !ensure_lease(pool,job_id,owner).await? {
        return Err(sqlx::Error::RowNotFound);
    }
    Ok(false)
}

/// Only a fully successful provider is checkpointed. A failure or rate limit
/// must leave it pending for the next job attempt.
pub async fn complete_provider(
    pool: &PgPool, job_id: Uuid, owner: Uuid,
    source: &str, version: &str, metrics: &Value,
) -> Result<(), sqlx::Error> {
    let inserted: Option<Uuid> = sqlx::query_scalar(
        "INSERT INTO ingest_provider_checkpoints
           (job_id,source_kind,connector_version,metrics_json)
         SELECT j.id,$3,$4,$5 FROM ingest_jobs j
         WHERE j.id=$1 AND j.owner_token=$2 AND j.status='running' AND j.lease_until>now()
         ON CONFLICT (job_id,source_kind)
         DO UPDATE SET connector_version=excluded.connector_version,
                       metrics_json=excluded.metrics_json,
                       completed_at=now()
         RETURNING job_id"
    ).bind(job_id).bind(owner).bind(source).bind(version)
      .bind(metrics).fetch_optional(pool).await?;
    if inserted.is_none() && !ensure_lease(pool,job_id,owner).await? {
        return Err(sqlx::Error::RowNotFound);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ingest_jobs;

    async fn claimed(pool: &PgPool) -> (Uuid,Uuid) {
        let (job,_) = ingest_jobs::admit(pool,"agora","Victor Hugo",Some("Q535"),None,
            &serde_json::json!({"subject":"Victor Hugo","live":true}))
            .await.unwrap();
        let owner=ingest_jobs::claim_next(pool).await.unwrap().unwrap();
        assert_eq!(owner.id,job.id);
        (job.id,owner.owner_token.unwrap())
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn repeated_documents_and_provider_checkpoints_are_idempotent(pool: PgPool) {
        let (job,owner)=claimed(&pool).await;
        let source="hal";
        let corpus_id:Uuid=sqlx::query_scalar(
            "INSERT INTO corpus_documents(source_kind,external_id,title,document_type)
             VALUES('hal','work-1','Work 1','academic_article') RETURNING id"
        ).fetch_one(&pool).await.unwrap();
        assert!(record_document(&pool,job,owner,source,"work-1",corpus_id).await.unwrap());
        assert!(!record_document(&pool,job,owner,source,"work-1",corpus_id).await.unwrap());
        let metrics=serde_json::json!({"status":"healthy","documents_persisted":1});
        complete_provider(&pool,job,owner,source,"hal:v1",&metrics).await.unwrap();
        complete_provider(&pool,job,owner,source,"hal:v1",&metrics).await.unwrap();
        assert_eq!(completed_documents(&pool,job).await.unwrap().len(),1);
        assert_eq!(completed_providers(&pool,job).await.unwrap().len(),1);
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn completed_provider_can_refresh_connector_version(pool: PgPool) {
        let (job,owner)=claimed(&pool).await;
        complete_provider(&pool,job,owner,"hal","hal:v1",&serde_json::json!({"status":"healthy"})).await.unwrap();
        complete_provider(&pool,job,owner,"hal","hal:v2",&serde_json::json!({"status":"healthy","documents_persisted":2})).await.unwrap();
        let data=completed_providers(&pool,job).await.unwrap();
        assert_eq!(data.len(),1);
        assert_eq!(data[0].connector_version,"hal:v2");
        assert_eq!(data[0].metrics_json["documents_persisted"],2);
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn old_owner_cannot_record_work_after_lease_reclaimed(pool: PgPool) {
        let (job,old)=claimed(&pool).await;
        sqlx::query("UPDATE ingest_jobs SET lease_until=now()-interval '1 second' WHERE id=$1")
            .bind(job).execute(&pool).await.unwrap();
        let replacement=ingest_jobs::claim_next(&pool).await.unwrap().unwrap();
        let new_owner=replacement.owner_token.unwrap();
        assert_ne!(old,new_owner);
        let metrics=serde_json::json!({"status":"healthy"});
        assert!(complete_provider(&pool,job,old,"hal","1",&metrics).await.is_err());
        assert!(record_document(&pool,job,old,"hal","x",Uuid::new_v4()).await.is_err());
        complete_provider(&pool,job,new_owner,"hal","1",&metrics).await.unwrap();
        assert_eq!(completed_providers(&pool,job).await.unwrap().len(),1);
    }
}
