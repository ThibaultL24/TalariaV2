//! Read-only operational analytics for authorized ingestion administrators.
//! No request payload, upstream error or raw historical content is returned.
use chrono::{DateTime, Utc};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

#[derive(Debug, Clone, FromRow)]
pub struct AdminJobRow {
    pub id: Uuid,
    pub lane: String,
    pub subject: String,
    pub qid: Option<String>,
    pub status: String,
    pub attempts: i32,
    pub created_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
    pub elapsed_ms: Option<i64>,
}

#[derive(Debug, Clone, FromRow)]
pub struct JobSummary {
    pub total: i64,
    pub queued: i64,
    pub running: i64,
    pub done: i64,
    pub failed: i64,
    pub retried: i64,
    pub p50_ms: Option<f64>,
    pub p95_ms: Option<f64>,
}

#[derive(Debug, Clone, FromRow)]
pub struct ProviderSummary {
    pub source_kind: String,
    pub attempts: i64,
    pub failed: i64,
    pub rate_limited: i64,
    pub timeouts: i64,
    pub fetch_calls: i64,
    pub persisted_operations: i64,
    pub p50_ms: Option<f64>,
    pub p95_ms: Option<f64>,
}

#[derive(Debug, Clone, FromRow)]
pub struct ProviderCoverage {
    pub source_kind: String,
    pub persisted_unique: i64,
    pub matched_unique: i64,
}

pub async fn jobs(
    pool: &PgPool, days: i32, lane: Option<&str>, status: Option<&str>,
    limit: i64, offset: i64,
) -> Result<(Vec<AdminJobRow>,i64), sqlx::Error> {
    let count: i64=sqlx::query_scalar(
        "SELECT count(*)::bigint FROM ingest_jobs
         WHERE created_at >= now()-make_interval(days => $1)
           AND ($2::text IS NULL OR lane=$2)
           AND ($3::text IS NULL OR status=$3)"
    ).bind(days).bind(lane).bind(status).fetch_one(pool).await?;
    let rows=sqlx::query_as::<_,AdminJobRow>(
        "SELECT id,lane,subject,qid,status,attempts,created_at,started_at,finished_at,
                CASE WHEN started_at IS NULL THEN NULL ELSE
                 greatest(0,extract(epoch FROM
                    (coalesce(finished_at,now())-started_at))*1000)::bigint
                END AS elapsed_ms
         FROM ingest_jobs
         WHERE created_at >= now()-make_interval(days => $1)
           AND ($2::text IS NULL OR lane=$2)
           AND ($3::text IS NULL OR status=$3)
         ORDER BY created_at DESC,id DESC
         LIMIT $4 OFFSET $5"
    ).bind(days).bind(lane).bind(status)
     .bind(limit.clamp(1,50)).bind(offset.max(0))
     .fetch_all(pool).await?;
    Ok((rows,count))
}

pub async fn job_summary(pool: &PgPool, days: i32) -> Result<JobSummary,sqlx::Error> {
    sqlx::query_as::<_,JobSummary>(
        "SELECT count(*)::bigint AS total,
          count(*) FILTER (WHERE status='queued')::bigint AS queued,
          count(*) FILTER (WHERE status='running')::bigint AS running,
          count(*) FILTER (WHERE status='done')::bigint AS done,
          count(*) FILTER (WHERE status='failed')::bigint AS failed,
          count(*) FILTER (WHERE attempts>1)::bigint AS retried,
          percentile_cont(0.5) WITHIN GROUP (ORDER BY
             extract(epoch FROM (finished_at-started_at))*1000)
             FILTER (WHERE finished_at IS NOT NULL AND started_at IS NOT NULL)
             AS p50_ms,
          percentile_cont(0.95) WITHIN GROUP (ORDER BY
             extract(epoch FROM (finished_at-started_at))*1000)
             FILTER (WHERE finished_at IS NOT NULL AND started_at IS NOT NULL)
             AS p95_ms
         FROM ingest_jobs
         WHERE created_at >= now()-make_interval(days => $1)"
    ).bind(days).fetch_one(pool).await
}

pub async fn providers(pool: &PgPool, days: i32) -> Result<Vec<ProviderSummary>,sqlx::Error> {
    sqlx::query_as::<_,ProviderSummary>(
        "SELECT p.source_kind,
          count(*)::bigint AS attempts,
          count(*) FILTER (WHERE p.status IN ('failed','partial'))::bigint AS failed,
          count(*) FILTER (WHERE p.status='rate_limited')::bigint AS rate_limited,
          coalesce(sum(p.timeouts),0)::bigint AS timeouts,
          coalesce(sum(p.fetch_calls),0)::bigint AS fetch_calls,
          coalesce(sum(p.persisted),0)::bigint AS persisted_operations,
          percentile_cont(0.5) WITHIN GROUP (ORDER BY p.elapsed_ms)
              FILTER (WHERE p.status<>'running') AS p50_ms,
          percentile_cont(0.95) WITHIN GROUP (ORDER BY p.elapsed_ms)
              FILTER (WHERE p.status<>'running') AS p95_ms
         FROM ingest_provider_attempt_metrics p
         JOIN ingest_jobs j ON j.id=p.job_id
         WHERE j.created_at >= now()-make_interval(days => $1)
         GROUP BY p.source_kind ORDER BY p.source_kind"
    ).bind(days).fetch_all(pool).await
}

/// Count *unique job/document* pairs, and use EXISTS to avoid N:M link
/// multiplication. A link is not a verified historical claim.
pub async fn coverage(pool: &PgPool, days: i32) -> Result<Vec<ProviderCoverage>,sqlx::Error> {
    sqlx::query_as::<_,ProviderCoverage>(
        "WITH document_jobs AS (
          SELECT DISTINCT d.job_id,d.source_kind,d.corpus_document_id,j.entity_id
          FROM ingest_job_documents d
          JOIN ingest_jobs j ON j.id=d.job_id
          WHERE j.created_at >= now()-make_interval(days => $1)
         )
         SELECT d.source_kind,count(*)::bigint AS persisted_unique,
          count(*) FILTER (WHERE EXISTS (
             SELECT 1 FROM entity_document_links l
             WHERE l.entity_id=d.entity_id
               AND l.corpus_document_id=d.corpus_document_id
          ))::bigint AS matched_unique
         FROM document_jobs d GROUP BY d.source_kind ORDER BY d.source_kind"
    ).bind(days).fetch_all(pool).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ingest_jobs;

    #[sqlx::test(migrations="../../migrations")]
    async fn filters_pagination_and_summary_are_isolated(pool: PgPool) {
        let payload=serde_json::json!({"subject":"Victor Hugo","live":true});
        let (a,_)=ingest_jobs::admit(&pool,"agora","Victor Hugo",Some("Q535"),None,&payload).await.unwrap();
        let (b,_)=ingest_jobs::admit(&pool,"explorer","Marie Curie",Some("Q7186"),None,&payload).await.unwrap();
        assert_ne!(a.id,b.id);
        let (rows,count)=jobs(&pool,30,Some("agora"),Some("queued"),20,0).await.unwrap();
        assert_eq!(count,1);
        assert_eq!(rows.len(),1);
        assert_eq!(rows[0].id,a.id);
        assert_eq!(jobs(&pool,30,None,None,1,1).await.unwrap().0.len(),1);
        let stats=job_summary(&pool,30).await.unwrap();
        assert_eq!(stats.total,2);
        assert_eq!(stats.queued,2);
        assert_eq!(stats.p50_ms,None);
        assert!(providers(&pool,30).await.unwrap().is_empty());
    }

    #[sqlx::test(migrations="../../migrations")]
    async fn providers_report_failed_rate_limit_and_no_unverified_coverage(pool: PgPool) {
        let payload=serde_json::json!({"subject":"Victor Hugo","live":true});
        let (job,_)=ingest_jobs::admit(&pool,"agora","Victor Hugo",Some("Q535"),None,&payload).await.unwrap();
        let claimed=ingest_jobs::claim_next(&pool).await.unwrap().unwrap();
        let token=claimed.owner_token.unwrap();
        crate::ingest_provider_metrics::begin_attempt(&pool,job.id,token,1,"hal").await.unwrap();
        let counts=crate::ingest_provider_metrics::AttemptCounts {
            discover_calls:1,fetch_calls:4,discovered:4,persisted:2,
            skipped:2,matched:0,timeouts:1,errors:1,elapsed_ms:300,
        };
        crate::ingest_provider_metrics::finish_attempt(
            &pool,job.id,token,1,"hal","rate_limited",Some("rate_limit"),&counts
        ).await.unwrap();
        let p=providers(&pool,30).await.unwrap();
        assert_eq!(p.len(),1);
        assert_eq!(p[0].rate_limited,1);
        assert_eq!(p[0].fetch_calls,4);
        assert_eq!(p[0].p95_ms,Some(300.0));
        assert!(coverage(&pool,30).await.unwrap().is_empty());
    }
}
