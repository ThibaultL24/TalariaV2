//! Read-only administrative projections. Cohort = jobs created in [from, until).
//! Provider operation counters and unique document coverage have separate populations.
use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::{FromRow, PgPool, Postgres, QueryBuilder};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct Filter {
    pub from: DateTime<Utc>,
    pub until: DateTime<Utc>,
    pub lane: Option<String>,
    pub status: Option<String>,
    pub provider: Option<String>,
    pub qid: Option<String>,
    pub subject: Option<String>,
    pub limit: i64,
    pub offset: i64,
}
fn cohort(f: &Filter) -> QueryBuilder<'_, Postgres> {
    let mut q =
        QueryBuilder::new("WITH cohort AS (SELECT j.* FROM ingest_jobs j WHERE j.created_at >= ");
    q.push_bind(f.from)
        .push(" AND j.created_at < ")
        .push_bind(f.until);
    if let Some(v) = &f.lane {
        q.push(" AND j.lane = ").push_bind(v);
    }
    if let Some(v) = &f.status {
        q.push(" AND j.status = ").push_bind(v);
    }
    if let Some(v) = &f.qid {
        q.push(" AND j.qid = ").push_bind(v);
    }
    if let Some(v) = &f.subject {
        q.push(" AND lower(j.subject) = lower(")
            .push_bind(v)
            .push(")");
    }
    if let Some(v) = &f.provider {
        q.push(" AND EXISTS (SELECT 1 FROM ingest_provider_attempt_metrics a WHERE a.job_id=j.id AND a.source_kind=").push_bind(v).push(")");
    }
    q.push(") ");
    q
}
#[derive(Debug, Serialize, FromRow)]
pub struct Job {
    pub id: Uuid,
    pub lane: String,
    pub subject: String,
    pub qid: Option<String>,
    pub entity_id: Option<Uuid>,
    pub status: String,
    pub attempts: i32,
    pub resumes: i32,
    pub lease_expired: bool,
    pub created_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
    pub duration_ms: Option<f64>,
}
const JOB_FIELDS: &str = "id,lane,subject,qid,entity_id,status,attempts,greatest(attempts-1,0) AS resumes, coalesce(status='running' AND lease_until<=now(),false) AS lease_expired,created_at,started_at,finished_at, extract(epoch FROM (coalesce(finished_at,now())-started_at))::float8*1000 AS duration_ms";
#[derive(Debug, Serialize)]
pub struct Page<T> {
    pub items: Vec<T>,
    pub total: i64,
    pub limit: i64,
    pub offset: i64,
}
pub async fn jobs(pool: &PgPool, f: &Filter) -> Result<Page<Job>, sqlx::Error> {
    let mut tx = pool.begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ, READ ONLY")
        .execute(&mut *tx)
        .await?;
    sqlx::query("SET LOCAL statement_timeout = '10s'")
        .execute(&mut *tx)
        .await?;
    let total = cohort(f)
        .push("SELECT count(*) FROM cohort")
        .build_query_scalar()
        .fetch_one(&mut *tx)
        .await?;
    let items = cohort(f)
        .push("SELECT ")
        .push(JOB_FIELDS)
        .push(" FROM cohort ORDER BY created_at DESC,id DESC LIMIT ")
        .push_bind(f.limit)
        .push(" OFFSET ")
        .push_bind(f.offset)
        .build_query_as()
        .fetch_all(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(Page {
        items,
        total,
        limit: f.limit,
        offset: f.offset,
    })
}
pub async fn job(pool: &PgPool, id: Uuid) -> Result<Option<Job>, sqlx::Error> {
    QueryBuilder::new("SELECT ")
        .push(JOB_FIELDS)
        .push(" FROM ingest_jobs WHERE id=")
        .push_bind(id)
        .build_query_as()
        .fetch_optional(pool)
        .await
}
#[derive(Debug, Serialize, FromRow)]
pub struct Summary {
    pub queued: i64,
    pub running: i64,
    pub done: i64,
    pub failed: i64,
    pub resumes: i64,
    pub expired_leases: i64,
    pub mean_ms: Option<f64>,
    pub p50_ms: Option<f64>,
    pub p95_ms: Option<f64>,
    pub success_percent: Option<f64>,
    pub failure_percent: Option<f64>,
    pub documents_persisted_unique: i64,
    pub documents_matched_unique: i64,
    pub instrumented_agora_jobs: i64,
}
#[derive(Debug, Serialize, FromRow)]
pub struct Volume {
    pub day: String,
    pub jobs: i64,
}
#[derive(Debug, Serialize)]
pub struct Overview {
    pub summary: Summary,
    pub volume: Vec<Volume>,
}
const COVERAGE: &str = ", coverage AS (
 SELECT count(DISTINCT d.corpus_document_id)::bigint AS documents_persisted_unique,
 count(DISTINCT d.corpus_document_id) FILTER (WHERE EXISTS (
 SELECT 1 FROM entity_document_links l WHERE l.entity_id=j.entity_id AND l.corpus_document_id=d.corpus_document_id))::bigint AS documents_matched_unique
 FROM cohort j JOIN ingest_job_documents d ON d.job_id=j.id";
pub async fn overview(pool: &PgPool, f: &Filter) -> Result<Overview, sqlx::Error> {
    let mut q = cohort(f);
    q.push(COVERAGE);
    if let Some(v) = &f.provider {
        q.push(" WHERE d.source_kind=").push_bind(v);
    }
    q.push(") SELECT count(*) FILTER (WHERE status='queued') AS queued,
    count(*) FILTER (WHERE status='running') AS running,
    count(*) FILTER (WHERE status='done') AS done,
    count(*) FILTER (WHERE status='failed') AS failed,
    coalesce(sum(greatest(attempts-1,0)),0)::bigint AS resumes,
    count(*) FILTER (WHERE status='running' AND lease_until<=now()) AS expired_leases,
    avg(extract(epoch FROM (finished_at-started_at))*1000) FILTER (WHERE status IN ('done','failed'))::float8 AS mean_ms,
    percentile_cont(0.5) WITHIN GROUP (ORDER BY extract(epoch FROM (finished_at-started_at))*1000) FILTER (WHERE status IN ('done','failed')) AS p50_ms,
    percentile_cont(0.95) WITHIN GROUP (ORDER BY extract(epoch FROM (finished_at-started_at))*1000) FILTER (WHERE status IN ('done','failed')) AS p95_ms,
    100.0*count(*) FILTER (WHERE status='done')::float8/nullif(count(*) FILTER (WHERE status IN ('done','failed')),0) AS success_percent,
    100.0*count(*) FILTER (WHERE status='failed')::float8/nullif(count(*) FILTER (WHERE status IN ('done','failed')),0) AS failure_percent,
    (SELECT documents_persisted_unique FROM coverage) AS documents_persisted_unique,
    (SELECT documents_matched_unique FROM coverage) AS documents_matched_unique,
    count(*) FILTER (WHERE lane='agora' AND EXISTS (SELECT 1 FROM ingest_provider_attempt_metrics a WHERE a.job_id=cohort.id)) AS instrumented_agora_jobs
    FROM cohort");
    let mut tx = pool.begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ, READ ONLY")
        .execute(&mut *tx)
        .await?;
    sqlx::query("SET LOCAL statement_timeout = '10s'")
        .execute(&mut *tx)
        .await?;
    let summary = q.build_query_as().fetch_one(&mut *tx).await?;
    let volume=cohort(f).push("SELECT to_char(created_at AT TIME ZONE 'UTC','YYYY-MM-DD') AS day,count(*) AS jobs FROM cohort GROUP BY day ORDER BY day")
        .build_query_as().fetch_all(&mut *tx).await?;
    tx.commit().await?;
    Ok(Overview { summary, volume })
}
#[derive(Debug, Serialize, FromRow)]
pub struct Provider {
    pub provider: String,
    pub attempts: i64,
    pub completed_attempts: i64,
    pub mean_ms: Option<f64>,
    pub p50_ms: Option<f64>,
    pub p95_ms: Option<f64>,
    pub elapsed_ms: i64,
    pub discover_calls: i64,
    pub fetch_calls: i64,
    pub discovered: i64,
    pub persisted: i64,
    pub timeouts: i64,
    pub errors: i64,
    pub rate_limited_attempts: i64,
    pub resumes: i64,
    pub interrupted_attempts: i64,
    pub failure_percent: Option<f64>,
    pub documents_persisted_unique: i64,
    pub documents_matched_unique: i64,
    pub match_rate_percent: Option<f64>,
}
pub async fn providers(
    pool: &PgPool,
    f: &Filter,
    sort: &str,
) -> Result<Page<Provider>, sqlx::Error> {
    let mut q = cohort(f);
    q.push(", a AS (SELECT a.*,j.attempts AS job_attempts,j.status AS job_status,j.lease_until FROM ingest_provider_attempt_metrics a JOIN cohort j ON j.id=a.job_id");
    if let Some(v) = &f.provider {
        q.push(" WHERE a.source_kind=").push_bind(v);
    }
    q.push("), coverage AS (SELECT d.source_kind,count(DISTINCT d.corpus_document_id)::bigint AS persisted_unique,
      count(DISTINCT d.corpus_document_id) FILTER (WHERE EXISTS (SELECT 1 FROM entity_document_links l WHERE l.entity_id=j.entity_id AND l.corpus_document_id=d.corpus_document_id))::bigint AS matched_unique
      FROM ingest_job_documents d JOIN cohort j ON j.id=d.job_id GROUP BY d.source_kind), stats AS (
      SELECT source_kind AS provider,count(*) AS attempts,count(*) FILTER (WHERE finished_at IS NOT NULL) AS completed_attempts,
      avg(elapsed_ms) FILTER (WHERE finished_at IS NOT NULL)::float8 AS mean_ms,
      percentile_cont(0.5) WITHIN GROUP (ORDER BY elapsed_ms) FILTER (WHERE finished_at IS NOT NULL) AS p50_ms,
      percentile_cont(0.95) WITHIN GROUP (ORDER BY elapsed_ms) FILTER (WHERE finished_at IS NOT NULL) AS p95_ms,
      sum(elapsed_ms)::bigint AS elapsed_ms,sum(discover_calls)::bigint AS discover_calls,sum(fetch_calls)::bigint AS fetch_calls,
      sum(discovered)::bigint AS discovered,sum(persisted)::bigint AS persisted,sum(timeouts)::bigint AS timeouts,sum(errors)::bigint AS errors,
      count(*) FILTER (WHERE error_kind='rate_limit') AS rate_limited_attempts,
      count(*) FILTER (WHERE EXISTS (SELECT 1 FROM ingest_provider_attempt_metrics prior WHERE prior.job_id=a.job_id AND prior.source_kind=a.source_kind AND prior.attempt<a.attempt)) AS resumes,
      count(*) FILTER (WHERE status='running' AND (attempt<job_attempts OR job_status<>'running' OR lease_until<=now())) AS interrupted_attempts,
      100.0*count(*) FILTER (WHERE status IN ('partial','failed','rate_limited'))::float8/nullif(count(*) FILTER (WHERE finished_at IS NOT NULL AND status<>'not_configured'),0) AS failure_percent
      FROM a GROUP BY source_kind) SELECT stats.*,coalesce(c.persisted_unique,0) AS documents_persisted_unique,
      coalesce(c.matched_unique,0) AS documents_matched_unique,100.0*c.matched_unique::float8/nullif(c.persisted_unique,0) AS match_rate_percent
      FROM stats LEFT JOIN coverage c ON c.source_kind=stats.provider ORDER BY ");
    q.push(match sort {
        "errors" => "failure_percent DESC NULLS LAST",
        "calls" => "(discover_calls+fetch_calls) DESC",
        _ => "p95_ms DESC NULLS LAST",
    });
    q.push(",provider LIMIT ")
        .push_bind(f.limit)
        .push(" OFFSET ")
        .push_bind(f.offset);
    let mut tx = pool.begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ, READ ONLY")
        .execute(&mut *tx)
        .await?;
    sqlx::query("SET LOCAL statement_timeout = '10s'")
        .execute(&mut *tx)
        .await?;
    let items = q.build_query_as().fetch_all(&mut *tx).await?;
    let mut total_q = cohort(f);
    total_q.push("SELECT count(DISTINCT a.source_kind) FROM ingest_provider_attempt_metrics a JOIN cohort j ON j.id=a.job_id");
    if let Some(v) = &f.provider {
        total_q.push(" WHERE a.source_kind=").push_bind(v);
    }
    let total = total_q.build_query_scalar().fetch_one(&mut *tx).await?;
    tx.commit().await?;
    Ok(Page {
        items,
        total,
        limit: f.limit,
        offset: f.offset,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;
    fn filter() -> Filter {
        Filter {
            from: "2026-01-01T00:00:00Z".parse().unwrap(),
            until: "2026-01-08T00:00:00Z".parse().unwrap(),
            lane: None,
            status: None,
            provider: None,
            qid: None,
            subject: None,
            limit: 1,
            offset: 0,
        }
    }
    async fn fixture(pool: &PgPool, qid: &str, seconds: i32, attempts: i32) -> Uuid {
        let id = Uuid::new_v4();
        sqlx::query("INSERT INTO ingest_jobs(id,lane,subject,qid,identity_key,request_json,status,attempts,created_at,started_at,finished_at,error,report_json)
         VALUES($1,'agora','Homonyme',$2,'qid:'||$2,'{}','done',$3,'2026-01-02','2026-01-02','2026-01-02'::timestamptz+($4*interval '1 second'),'secret-token','{\"token\":\"private\"}')")
         .bind(id).bind(qid).bind(attempts).bind(seconds).execute(pool).await.unwrap();
        id
    }
    #[sqlx::test(migrations = "../../migrations")]
    async fn aggregates_retries_percentiles_distinct_documents_and_identity(pool: PgPool) {
        let first = fixture(&pool, "Q1", 10, 3).await;
        let second = fixture(&pool, "Q2", 30, 1).await;
        let entity:Uuid=sqlx::query_scalar("INSERT INTO entities(wikipedia_title,canonical_name) VALUES('Homonyme','Homonyme') RETURNING id").fetch_one(&pool).await.unwrap();
        sqlx::query("UPDATE ingest_jobs SET entity_id=$1 WHERE id=$2")
            .bind(entity)
            .bind(first)
            .execute(&pool)
            .await
            .unwrap();
        let doc:Uuid=sqlx::query_scalar("INSERT INTO corpus_documents(source_kind,external_id,title,document_type) VALUES('hal','observatory-doc','Test','article') RETURNING id").fetch_one(&pool).await.unwrap();
        for (job, source, external) in [
            (first, "hal", "one"),
            (first, "hal", "alias"),
            (second, "hal", "one"),
            (first, "gallica", "alias"),
        ] {
            sqlx::query("INSERT INTO ingest_job_documents(job_id,source_kind,external_id,corpus_document_id) VALUES($1,$2,$3,$4)")
                .bind(job).bind(source).bind(external).bind(doc).execute(&pool).await.unwrap();
        }
        for relation in ["about", "mentioned_in"] {
            sqlx::query("INSERT INTO entity_document_links(entity_id,corpus_document_id,relation,match_version,score) VALUES($1,$2,$3,'test',0.9)").bind(entity).bind(doc).bind(relation).execute(&pool).await.unwrap();
        }
        for (attempt, status, elapsed) in [(1, "partial", 1000_i64), (2, "healthy", 3000)] {
            sqlx::query("INSERT INTO ingest_provider_attempt_metrics(job_id,source_kind,attempt,status,discover_calls,fetch_calls,persisted,elapsed_ms,finished_at) VALUES($1,'hal',$2,$3,2,3,1,$4,now())").bind(first).bind(attempt).bind(status).bind(elapsed).execute(&pool).await.unwrap();
        }
        sqlx::query("INSERT INTO ingest_provider_attempt_metrics(job_id,source_kind,attempt) VALUES($1,'gallica',1)").bind(first).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO canonical_events(entity_id,title,time_json,pipeline) VALUES($1,'Historical fixture','{\"kind\":\"unknown\"}','person')").bind(entity).execute(&pool).await.unwrap();
        assert_eq!(
            crate::ingest_provider_metrics::total_document_coverage(&pool, first)
                .await
                .unwrap(),
            (1, 1)
        );
        let before: serde_json::Value = sqlx::query_scalar(
            "SELECT coalesce(jsonb_agg(to_jsonb(e)),'[]') FROM canonical_events e",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        let f = filter();
        let result = overview(&pool, &f).await.unwrap();
        assert_eq!((result.summary.done, result.summary.resumes), (2, 2));
        assert_eq!(result.summary.p50_ms, Some(20000.));
        assert_eq!(result.summary.p95_ms, Some(29000.));
        assert_eq!(result.summary.documents_persisted_unique, 1);
        assert_eq!(result.summary.documents_matched_unique, 1);
        assert_eq!(result.volume[0].jobs, 2);
        let providers = providers(&pool, &f, "slowest").await.unwrap();
        assert_eq!(providers.total, 2);
        let hal = &providers.items[0];
        assert_eq!(hal.provider, "hal");
        assert_eq!(hal.resumes, 1);
        assert_eq!(hal.p50_ms, Some(2000.));
        assert_eq!(hal.p95_ms, Some(2900.));
        assert_eq!(hal.persisted, 2);
        assert_eq!(hal.documents_persisted_unique, 1);
        assert_eq!(hal.failure_percent, Some(50.));
        let page = jobs(&pool, &f).await.unwrap();
        assert_eq!(page.items.len(), 1);
        assert_eq!(page.total, 2);
        let mut f = f.clone();
        f.offset = 50;
        assert!(jobs(&pool, &f).await.unwrap().items.is_empty());
        assert_eq!(jobs(&pool, &f).await.unwrap().total, 2);
        f.offset = 0;
        f.qid = Some("Q2".into());
        let page = jobs(&pool, &f).await.unwrap();
        assert_eq!(page.total, 1);
        assert_eq!(page.items[0].id, second);
        assert_eq!(
            overview(&pool, &f)
                .await
                .unwrap()
                .summary
                .documents_matched_unique,
            0
        );
        f.qid = None;
        f.status = Some("failed".into());
        assert_eq!(jobs(&pool, &f).await.unwrap().total, 0);
        f.status = None;
        f.provider = Some("gallica".into());
        let interrupted = super::providers(&pool, &f, "errors").await.unwrap();
        assert_eq!(interrupted.items[0].interrupted_attempts, 1);
        assert_eq!(interrupted.items[0].p95_ms, None);
        f.provider = None;
        f.qid = None;
        f.lane = Some("explorer".into());
        assert_eq!(overview(&pool, &f).await.unwrap().summary.done, 0);
        f.lane = None;
        f.until = f.from + Duration::hours(1);
        let empty = overview(&pool, &f).await.unwrap();
        assert_eq!(empty.summary.p50_ms, None);
        assert_eq!(empty.summary.success_percent, None);
        let plan:Vec<(String,)>=sqlx::query_as("EXPLAIN (ANALYZE,BUFFERS) SELECT id FROM ingest_jobs WHERE created_at>='2026-01-01' AND created_at<'2026-01-08' ORDER BY created_at DESC,id DESC LIMIT 25").fetch_all(&pool).await.unwrap();
        eprintln!(
            "Period page EXPLAIN (small fixture):\n{}",
            plan.iter()
                .map(|line| line.0.as_str())
                .collect::<Vec<_>>()
                .join("\n")
        );
        let after: serde_json::Value = sqlx::query_scalar(
            "SELECT coalesce(jsonb_agg(to_jsonb(e)),'[]') FROM canonical_events e",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(before, after);
        let safe = serde_json::to_string(&job(&pool, first).await.unwrap()).unwrap();
        assert!(!safe.contains("secret-token"));
        assert!(!safe.contains("private"));
    }
}
