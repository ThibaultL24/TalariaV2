//! Typed, bounded observability for Agora connector attempts.
//! Writes are fenced by the parent job lease; reads never expose raw source errors.
use chrono::{DateTime, Utc};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

#[derive(Debug, Clone, FromRow)]
pub struct ProviderAttempt {
    pub source_kind: String,
    pub attempt: i32,
    pub status: String,
    pub error_kind: Option<String>,
    pub discover_calls: i64,
    pub fetch_calls: i64,
    pub discovered: i64,
    pub persisted: i64,
    pub skipped: i64,
    pub matched: i64,
    pub timeouts: i64,
    pub errors: i64,
    pub elapsed_ms: i64,
    pub started_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, FromRow)]
pub struct DocumentCoverage {
    pub source_kind: String,
    pub persisted_unique: i64,
    pub matched_unique: i64,
}

#[derive(Debug, Clone)]
pub struct AttemptCounts {
    pub discover_calls: i64,
    pub fetch_calls: i64,
    pub discovered: i64,
    pub persisted: i64,
    pub skipped: i64,
    pub matched: i64,
    pub timeouts: i64,
    pub errors: i64,
    pub elapsed_ms: i64,
}

pub async fn begin_attempt(
    pool: &PgPool, job: Uuid, owner: Uuid, attempt: i32, source: &str,
) -> Result<(), sqlx::Error> {
    let row: Option<String> = sqlx::query_scalar(
        "INSERT INTO ingest_provider_attempt_metrics(job_id,source_kind,attempt)
         SELECT j.id,$4,$3 FROM ingest_jobs j
         WHERE j.id=$1 AND j.owner_token=$2 AND j.status='running'
           AND j.lease_until > now()
         ON CONFLICT (job_id,source_kind,attempt) DO NOTHING
         RETURNING source_kind"
    ).bind(job).bind(owner).bind(attempt).bind(source)
    .fetch_optional(pool).await?;
    if row.is_none() {
        return Err(sqlx::Error::RowNotFound);
    }
    Ok(())
}

pub async fn finish_attempt(
    pool: &PgPool, job: Uuid, owner: Uuid, attempt: i32,
    source: &str, status: &str, error_kind: Option<&str>, counts: &AttemptCounts,
) -> Result<(), sqlx::Error> {
    let row=sqlx::query(
        "UPDATE ingest_provider_attempt_metrics a SET
            status=$5,error_kind=$6,discover_calls=$7,fetch_calls=$8,
            discovered=$9,persisted=$10,skipped=$11,matched=$12,
            timeouts=$13,errors=$14,elapsed_ms=$15,finished_at=now()
         FROM ingest_jobs j
         WHERE a.job_id=$1 AND a.source_kind=$4 AND a.attempt=$3
           AND a.status='running'
           AND j.id=a.job_id AND j.owner_token=$2
           AND j.status='running' AND j.lease_until>now()"
    ).bind(job).bind(owner).bind(attempt).bind(source)
    .bind(status).bind(error_kind)
    .bind(counts.discover_calls).bind(counts.fetch_calls)
    .bind(counts.discovered).bind(counts.persisted)
    .bind(counts.skipped).bind(counts.matched)
    .bind(counts.timeouts).bind(counts.errors).bind(counts.elapsed_ms)
    .execute(pool).await?;
    if row.rows_affected()!=1 { return Err(sqlx::Error::RowNotFound); }
    Ok(())
}

pub async fn list_attempts(
    pool: &PgPool,job: Uuid, limit: i64, offset: i64,
) -> Result<Vec<ProviderAttempt>,sqlx::Error> {
    sqlx::query_as::<_,ProviderAttempt>(
        "SELECT source_kind,attempt,status,error_kind,discover_calls,fetch_calls,
                discovered,persisted,skipped,matched,timeouts,errors,elapsed_ms,
                started_at,finished_at
         FROM ingest_provider_attempt_metrics
         WHERE job_id=$1 ORDER BY source_kind,attempt
         LIMIT $2 OFFSET $3"
    ).bind(job).bind(limit.clamp(1,100)).bind(offset.max(0))
     .fetch_all(pool).await
}

pub async fn count_attempts(pool: &PgPool,job: Uuid) -> Result<i64,sqlx::Error> {
    sqlx::query_scalar(
        "SELECT count(*)::bigint FROM ingest_provider_attempt_metrics WHERE job_id=$1"
    ).bind(job).fetch_one(pool).await
}

/// Coverage refers to matching corpus *documents*, not verified historical claims.
/// Both counts are distinct to prevent N:M joins inflating the numerator.
pub async fn document_coverage(
    pool: &PgPool,job: Uuid,
) -> Result<Vec<DocumentCoverage>,sqlx::Error> {
    sqlx::query_as::<_,DocumentCoverage>(
        "SELECT d.source_kind,
          count(DISTINCT d.corpus_document_id)::bigint AS persisted_unique,
          count(DISTINCT d.corpus_document_id)
             FILTER (WHERE l.corpus_document_id IS NOT NULL)::bigint AS matched_unique
         FROM ingest_job_documents d
         JOIN ingest_jobs j ON j.id=d.job_id
         LEFT JOIN entity_document_links l
           ON l.corpus_document_id=d.corpus_document_id AND l.entity_id=j.entity_id
         WHERE d.job_id=$1
         GROUP BY d.source_kind
         ORDER BY d.source_kind"
    ).bind(job).fetch_all(pool).await
}

/// Global totals must not sum provider buckets: aliases can journal one document
/// under multiple sources. The entity match remains scoped to this job's identity.
pub async fn total_document_coverage(pool: &PgPool, job: Uuid) -> Result<(i64,i64),sqlx::Error> {
    sqlx::query_as("SELECT count(DISTINCT d.corpus_document_id)::bigint,
      count(DISTINCT d.corpus_document_id) FILTER (WHERE EXISTS (
        SELECT 1 FROM entity_document_links l WHERE l.entity_id=j.entity_id
        AND l.corpus_document_id=d.corpus_document_id))::bigint
      FROM ingest_job_documents d JOIN ingest_jobs j ON j.id=d.job_id WHERE d.job_id=$1")
      .bind(job).fetch_one(pool).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ingest_jobs;

    async fn admitted(pool: &PgPool) -> (Uuid,Uuid) {
        let (job,_) = ingest_jobs::admit(pool,"agora","Victor Hugo",Some("Q535"),None,
            &serde_json::json!({"subject":"Victor Hugo","live":true})).await.unwrap();
        let claim=ingest_jobs::claim_next(pool).await.unwrap().unwrap();
        assert_eq!(job.id,claim.id);
        (job.id,claim.owner_token.unwrap())
    }

    fn counts() -> AttemptCounts {
        AttemptCounts {
            discover_calls:2,fetch_calls:3,discovered:5,persisted:2,
            skipped:1,matched:1,timeouts:0,errors:1,elapsed_ms:125,
        }
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn typed_metrics_are_fenced_and_sorted(pool: PgPool) {
        let (job,owner)=admitted(&pool).await;
        begin_attempt(&pool,job,owner,1,"hal").await.unwrap();
        finish_attempt(&pool,job,owner,1,"hal","partial",Some("upstream"),&counts())
            .await.unwrap();
        let rows=list_attempts(&pool,job,100,0).await.unwrap();
        assert_eq!(rows.len(),1);
        assert_eq!(rows[0].source_kind,"hal");
        assert_eq!(rows[0].attempt,1);
        assert_eq!(rows[0].fetch_calls,3);
        assert_eq!(rows[0].matched,1);
        assert_eq!(rows[0].elapsed_ms,125);
        assert!(rows[0].finished_at.is_some());
        assert!(finish_attempt(&pool,job,owner,1,"hal","healthy",None,&counts())
            .await.is_err());

        sqlx::query("UPDATE ingest_jobs SET lease_until=now()-interval '1 second' WHERE id=$1")
            .bind(job).execute(&pool).await.unwrap();
        let reclaimed=ingest_jobs::claim_next(&pool).await.unwrap().unwrap();
        let next_owner=reclaimed.owner_token.unwrap();
        assert_ne!(owner,next_owner);
        assert!(begin_attempt(&pool,job,owner,2,"hal").await.is_err());
        begin_attempt(&pool,job,next_owner,2,"hal").await.unwrap();
        assert_eq!(list_attempts(&pool,job,100,0).await.unwrap().len(),2);
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn unique_document_coverage_does_not_infer_unverified_claims(pool: PgPool) {
        let (job,owner)=admitted(&pool).await;
        assert_eq!(document_coverage(&pool,job).await.unwrap().len(),0);
        let corpus_id:Uuid=sqlx::query_scalar(
            "INSERT INTO corpus_documents(source_kind,external_id,title,document_type)
             VALUES('hal','doc-1','Work','academic_article') RETURNING id"
        ).fetch_one(&pool).await.unwrap();
        crate::ingest_provider_checkpoints::record_document(
            &pool,job,owner,"hal","doc-1",corpus_id).await.unwrap();
        let rows=document_coverage(&pool,job).await.unwrap();
        assert_eq!(rows.len(),1);
        assert_eq!(rows[0].persisted_unique,1);
        assert_eq!(rows[0].matched_unique,0);

        let entity_id=crate::upsert_entity_with_kind(&pool,"en","Victor Hugo","person")
            .await.unwrap();
        sqlx::query("UPDATE ingest_jobs SET entity_id=$1 WHERE id=$2")
            .bind(entity_id).bind(job).execute(&pool).await.unwrap();
        // Several relation rows must not inflate the number of matched docs.
        for relation in ["about","mentioned_in"] {
            sqlx::query(
                "INSERT INTO entity_document_links
                  (entity_id,corpus_document_id,relation,match_version,score)
                 VALUES ($1,$2,$3,'test:v1',0.95)"
            ).bind(entity_id).bind(corpus_id).bind(relation)
             .execute(&pool).await.unwrap();
        }
        let updated=document_coverage(&pool,job).await.unwrap();
        assert_eq!(updated[0].persisted_unique,1);
        assert_eq!(updated[0].matched_unique,1);
    }
}
