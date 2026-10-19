use chrono::{DateTime, Utc};
use serde_json::Value;
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

pub const MAX_INGEST_ATTEMPTS: i32 = 3;
const LEASE_SECONDS: i32 = 90;

#[derive(Debug, Clone, FromRow)]
pub struct DurableIngestJob {
    pub id: Uuid,
    pub lane: String,
    pub subject: String,
    pub qid: Option<String>,
    pub identity_key: String,
    pub entity_id: Option<Uuid>,
    pub request_json: Value,
    pub progress_json: Value,
    pub report_json: Option<Value>,
    pub error: Option<String>,
    pub status: String,
    pub attempts: i32,
    pub owner_token: Option<Uuid>,
    pub lease_until: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
    pub updated_at: DateTime<Utc>,
}

const JOB_COLUMNS: &str = "id,lane,subject,qid,identity_key,entity_id,request_json,progress_json,report_json,error,status,attempts,owner_token,lease_until,created_at,started_at,finished_at,updated_at";

pub fn identity_key(subject: &str, qid: Option<&str>) -> String {
    match qid.map(str::trim).filter(|v| !v.is_empty()) {
        Some(qid) => format!("qid:{}", qid.to_ascii_uppercase()),
        None => format!("name:{}", subject.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase()),
    }
}

pub async fn find_active(
    pool: &PgPool, lane: &str, subject: &str, qid: Option<&str>,
) -> Result<Option<DurableIngestJob>, sqlx::Error> {
    let sql = format!(
        "SELECT {JOB_COLUMNS} FROM ingest_jobs WHERE lane=$1 AND identity_key=$2 AND status IN ('queued','running')"
    );
    sqlx::query_as::<_, DurableIngestJob>(&sql)
        .bind(lane)
        .bind(identity_key(subject,qid))
        .fetch_optional(pool)
        .await
}

/// A partial UNIQUE index handles concurrent inserts across all API replicas.
pub async fn admit(
    pool: &PgPool,
    lane: &str,
    subject: &str,
    qid: Option<&str>,
    entity_id: Option<Uuid>,
    request_json: &Value,
) -> Result<(DurableIngestJob, bool), sqlx::Error> {
    let key = identity_key(subject,qid);
    let id = Uuid::new_v4();
    let sql = format!(
        "INSERT INTO ingest_jobs (id,lane,subject,qid,identity_key,entity_id,request_json)
         VALUES ($1,$2,$3,$4,$5,$6,$7)
         ON CONFLICT DO NOTHING RETURNING {JOB_COLUMNS}"
    );
    let inserted = sqlx::query_as::<_, DurableIngestJob>(&sql)
        .bind(id).bind(lane).bind(subject).bind(qid)
        .bind(&key).bind(entity_id).bind(request_json)
        .fetch_optional(pool).await?;
    if let Some(job) = inserted {
        return Ok((job,true));
    }
    let existing = find_active(pool,lane,subject,qid).await?;
    match existing {
        Some(job) => Ok((job,false)),
        None => Err(sqlx::Error::RowNotFound), // Unlikely concurrent terminal transition; caller may retry.
    }
}

pub async fn get(pool: &PgPool, id: Uuid) -> Result<Option<DurableIngestJob>, sqlx::Error> {
    let sql = format!("SELECT {JOB_COLUMNS} FROM ingest_jobs WHERE id=$1");
    sqlx::query_as::<_, DurableIngestJob>(&sql).bind(id).fetch_optional(pool).await
}

/// Mark jobs whose third (and final) lease expired as failed. No automatic
/// fourth replay is possible until the user explicitly submits another job.
pub async fn exhaust_stale(pool: &PgPool) -> Result<u64, sqlx::Error> {
    let result = sqlx::query(
        "UPDATE ingest_jobs
         SET status='failed', error='worker_lease_expired_after_max_attempts',
             owner_token=NULL, lease_until=NULL, finished_at=now(),updated_at=now()
         WHERE status='running' AND lease_until < now() AND attempts >= $1"
    ).bind(MAX_INGEST_ATTEMPTS).execute(pool).await?;
    Ok(result.rows_affected())
}

/// Claim one queued or expired-lease job with SKIP LOCKED for multi-replica safety.
pub async fn claim_next(pool: &PgPool) -> Result<Option<DurableIngestJob>, sqlx::Error> {
    let token = Uuid::new_v4();
    let sql = format!(
        "WITH next AS (
           SELECT id FROM ingest_jobs
           WHERE (status='queued' OR (status='running' AND lease_until < now()))
             AND attempts < $1
           ORDER BY created_at,id
           FOR UPDATE SKIP LOCKED LIMIT 1
         )
         UPDATE ingest_jobs AS j SET
           status='running', owner_token=$2,
           lease_until=now()+($3 * interval '1 second'), attempts=j.attempts+1,
           started_at=coalesce(j.started_at,now()), updated_at=now(),
           progress_json=CASE WHEN j.status='queued' THEN jsonb_build_object('phase','starting')
                              ELSE jsonb_build_object('phase','restarting') END
         FROM next WHERE j.id=next.id
         RETURNING {JOB_COLUMNS}"
    );
    sqlx::query_as::<_, DurableIngestJob>(&sql)
        .bind(MAX_INGEST_ATTEMPTS).bind(token).bind(LEASE_SECONDS)
        .fetch_optional(pool).await
}

/// False means the lease is lost; the old worker MUST stop processing.
pub async fn heartbeat(
    pool: &PgPool, id: Uuid, token: Uuid,
    entity_id: Option<Uuid>, progress: &Value,
) -> Result<bool, sqlx::Error> {
    let result = sqlx::query(
        "UPDATE ingest_jobs SET
           lease_until=now()+($3 * interval '1 second'),updated_at=now(),
           entity_id=coalesce($4,entity_id),progress_json=$5
         WHERE id=$1 AND owner_token=$2 AND status='running'
           AND lease_until > now()"
    ).bind(id).bind(token).bind(LEASE_SECONDS)
      .bind(entity_id).bind(progress).execute(pool).await?;
    Ok(result.rows_affected()==1)
}

/// The completed state is persisted only by the current lease owner.
pub async fn finish(
    pool: &PgPool, id: Uuid, token: Uuid, entity_id: Option<Uuid>,
    progress: &Value, report: Option<&Value>, error: Option<&str>,
) -> Result<bool, sqlx::Error> {
    let status = if error.is_some() { "failed" } else { "done" };
    let result = sqlx::query(
        "UPDATE ingest_jobs SET status=$3,entity_id=coalesce($4,entity_id),
           progress_json=$5,report_json=$6,error=$7,owner_token=NULL,
           lease_until=NULL,finished_at=now(),updated_at=now()
         WHERE id=$1 AND owner_token=$2 AND status='running'
           AND lease_until > now()"
    ).bind(id).bind(token).bind(status).bind(entity_id).bind(progress)
      .bind(report).bind(error).execute(pool).await?;
    Ok(result.rows_affected()==1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_uses_qid_before_any_name_matching() {
        assert_eq!(identity_key("Alexandre Dumas",Some("q187608")), "qid:Q187608");
        assert_eq!(identity_key("Alexandre Dumas",Some("Q193129")), "qid:Q193129");
        assert_eq!(identity_key("  Victor   Hugo ",None),"name:victor hugo");
        assert_ne!(identity_key("Victor Hugo",Some("Q535")),identity_key("Victor Hugo",None));
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn concurrent_admissions_produce_a_single_active_job(pool: PgPool) {
        let payload=serde_json::json!({"subject":"Victor Hugo","live":true});
        let (a,b)=tokio::join!(
            admit(&pool,"explorer","Victor Hugo",Some("Q535"),None,&payload),
            admit(&pool,"explorer","Victor Hugo",Some("Q535"),None,&payload)
        );
        let (a,a_new)=a.unwrap();
        let (b,b_new)=b.unwrap();
        assert_eq!(a.id,b.id);
        assert_ne!(a_new,b_new);
        assert_eq!(sqlx::query_scalar::<_,i64>("SELECT COUNT(*) FROM ingest_jobs")
            .fetch_one(&pool).await.unwrap(),1);
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn cross_worker_lease_ownership_and_retry(pool: PgPool) {
        let payload=serde_json::json!({"subject":"Victor Hugo","live":true});
        let (job,created)=admit(&pool,"explorer","Victor Hugo",Some("Q535"),None,&payload).await.unwrap();
        assert!(created);
        let first=claim_next(&pool).await.unwrap().unwrap();
        assert_eq!(first.id,job.id);
        let token=first.owner_token.unwrap();
        assert!(claim_next(&pool).await.unwrap().is_none());
        assert!(!heartbeat(&pool,job.id,Uuid::new_v4(),None,&serde_json::json!({})).await.unwrap());
        sqlx::query("UPDATE ingest_jobs SET lease_until=now()-interval '1 second' WHERE id=$1")
            .bind(job.id).execute(&pool).await.unwrap();
        let second=claim_next(&pool).await.unwrap().unwrap();
        assert_eq!(second.attempts,2);
        assert_ne!(second.owner_token,Some(token));
        assert!(!finish(&pool,job.id,token,None,&serde_json::json!({}),None,None).await.unwrap());
        assert!(finish(&pool,job.id,second.owner_token.unwrap(),None,&serde_json::json!({"phase":"done"}),Some(&payload),None).await.unwrap());
        assert_eq!(get(&pool,job.id).await.unwrap().unwrap().status,"done");
        let (next,created)=admit(&pool,"explorer","Victor Hugo",Some("Q535"),None,&payload).await.unwrap();
        assert!(created);
        assert_ne!(next.id,job.id);
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn homonyms_are_isolated_across_lanes_and_qids(pool: PgPool) {
        let data=serde_json::json!({"subject":"Alexandre Dumas"});
        for (lane,qid) in [("explorer","Q187608"),("explorer","Q193129"),("agora","Q187608")] {
            assert!(admit(&pool,lane,"Alexandre Dumas",Some(qid),None,&data).await.unwrap().1);
        }
        assert_eq!(sqlx::query_scalar::<_,i64>("SELECT COUNT(*) FROM ingest_jobs").fetch_one(&pool).await.unwrap(),3);
    }
}
