//! Authenticated, allowlisted operational analytics. Default is deny-all.
use super::{auth, AppState};
use axum::{
    extract::{Query, State},
    http::StatusCode,
    routing::get,
    Json, Router,
};
use axum_extra::extract::cookie::CookieJar;
use serde::Deserialize;
use serde_json::{json, Value};
use talaria_store::admin_ingest;
use uuid::Uuid;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/admin/ingestions/summary", get(summary))
        .route("/api/v1/admin/ingestions/jobs", get(jobs))
}

fn is_allowlisted(user_id: Uuid, raw: &str) -> bool {
    raw.split(',').filter_map(|id|Uuid::parse_str(id.trim()).ok())
        .any(|id|id==user_id)
}

/// Identity comes exclusively from an active, HttpOnly Talaria session.
/// No client-supplied header, query parameter or request body grants admin.
pub(crate) async fn require_admin(
    pool: &sqlx::PgPool, jar: &CookieJar,
) -> Result<(),(StatusCode,Json<Value>)> {
    let user=auth::require_user(pool,jar).await
        .map_err(auth::auth_error)?;
    let allowlist=std::env::var("TALARIA_ADMIN_USER_IDS").unwrap_or_default();
    if !is_allowlisted(user.id,&allowlist) {
        return Err((StatusCode::FORBIDDEN,Json(json!({"error":"admin_forbidden"}))));
    }
    Ok(())
}

#[derive(Debug,Deserialize)]
pub struct SummaryQuery {
    days: Option<i32>,
}

fn days_window(input: Option<i32>) -> Result<i32,(StatusCode,Json<Value>)> {
    let days=input.unwrap_or(30);
    if !(1..=90).contains(&days) {
        return Err((StatusCode::BAD_REQUEST,Json(json!({"error":"invalid_admin_days"}))));
    }
    Ok(days)
}

fn database_error(error: sqlx::Error) -> (StatusCode,Json<Value>) {
    tracing::error!(%error,"admin ingestion analytics failed");
    (StatusCode::INTERNAL_SERVER_ERROR,Json(json!({"error":"admin_metrics_unavailable"})))
}

async fn summary(
    State(state): State<AppState>,
    jar: CookieJar,
    Query(query): Query<SummaryQuery>,
) -> Result<Json<Value>,(StatusCode,Json<Value>)> {
    require_admin(&state.pool,&jar).await?;
    let days=days_window(query.days)?;
    let (jobs,provider_stats,coverage)=tokio::try_join!(
        admin_ingest::job_summary(&state.pool,days),
        admin_ingest::providers(&state.pool,days),
        admin_ingest::coverage(&state.pool,days)
    ).map_err(database_error)?;
    let stats=provider_stats.iter().map(|p|{
        let matched=coverage.iter().find(|c|c.source_kind==p.source_kind);
        let persisted=matched.map_or(0,|c|c.persisted_unique);
        let linked=matched.map_or(0,|c|c.matched_unique);
        json!({
            "provider":p.source_kind,
            "attempts":p.attempts,
            "failed":p.failed,
            "rate_limited":p.rate_limited,
            "timeouts":p.timeouts,
            "fetch_calls":p.fetch_calls,
            "persisted_operations":p.persisted_operations,
            "p50_ms":p.p50_ms,
            "p95_ms":p.p95_ms,
            "documents_persisted_unique":persisted,
            "documents_matched_unique":linked,
            "document_match_percent":percent(linked,persisted),
        })
    }).collect::<Vec<_>>();
    Ok(Json(json!({
        "days":days,
        "jobs":{
            "total":jobs.total,
            "queued":jobs.queued,
            "running":jobs.running,
            "done":jobs.done,
            "failed":jobs.failed,
            "retried":jobs.retried,
            "p50_ms":jobs.p50_ms,
            "p95_ms":jobs.p95_ms,
        },
        "coverage_meaning":"document_linkage_not_historical_truth",
        "providers":stats,
    })))
}

fn percent(numerator:i64,denominator:i64)->Option<f64>{
    if denominator<=0 { None }
    else {Some((10000.0 * numerator as f64 / denominator as f64).round()/100.0)}
}

#[derive(Debug,Deserialize)]
pub struct JobsQuery {
    days: Option<i32>,
    lane: Option<String>,
    status: Option<String>,
    limit: Option<i64>,
    offset: Option<i64>,
}

fn validate_jobs_query(q: &JobsQuery)->Result<(i32,i64,i64),(StatusCode,Json<Value>)>{
    let days=days_window(q.days)?;
    let limit=q.limit.unwrap_or(25);
    let offset=q.offset.unwrap_or(0);
    if !(1..=50).contains(&limit) || !(0..=5000).contains(&offset)
        || q.lane.as_deref().is_some_and(|v|!matches!(v,"agora"|"explorer"))
        || q.status.as_deref().is_some_and(|v|!matches!(v,"queued"|"running"|"done"|"failed"))
    {
        return Err((StatusCode::BAD_REQUEST,Json(json!({"error":"invalid_admin_job_filters"}))));
    }
    Ok((days,limit,offset))
}

async fn jobs(
    State(state): State<AppState>,jar: CookieJar,Query(query): Query<JobsQuery>,
) -> Result<Json<Value>,(StatusCode,Json<Value>)> {
    require_admin(&state.pool,&jar).await?;
    let (days,limit,offset)=validate_jobs_query(&query)?;
    let (rows,total)=admin_ingest::jobs(&state.pool,days,
        query.lane.as_deref(),query.status.as_deref(),limit,offset)
        .await.map_err(database_error)?;
    let items=rows.iter().map(|row|json!({
        "job_id":row.id,
        "lane":row.lane,
        "subject":row.subject,
        "qid":row.qid,
        "status":row.status,
        "attempts":row.attempts,
        "duration_ms":row.elapsed_ms,
        "created_at":row.created_at.to_rfc3339(),
        "started_at":row.started_at.map(|v|v.to_rfc3339()),
        "finished_at":row.finished_at.map(|v|v.to_rfc3339()),
    })).collect::<Vec<_>>();
    Ok(Json(json!({
        "days":days,"items":items,
        "pagination":{"limit":limit,"offset":offset,"total":total,
            "next_offset":if offset+limit<total {Some(offset+limit)} else {None}}
    })))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn admin_requires_exact_uuid_and_denies_missing_allowlist() {
        let admin_id=Uuid::new_v4();
        assert!(!is_allowlisted(admin_id,""));
        assert!(!is_allowlisted(admin_id,"not-an-id"));
        assert!(is_allowlisted(admin_id,&format!("bad,{},other",admin_id)));
        assert!(!is_allowlisted(Uuid::new_v4(),&admin_id.to_string()));
    }

    #[test]
    fn admin_filters_and_day_bounds_reject_unbounded_queries() {
        assert!(days_window(Some(0)).is_err());
        assert!(days_window(Some(91)).is_err());
        assert_eq!(days_window(None).unwrap(),30);
        let good=JobsQuery{days:None,lane:Some("agora".into()),
            status:None,limit:None,offset:None};
        assert_eq!(validate_jobs_query(&good).unwrap(),(30,25,0));
        let invalid=JobsQuery{lane:Some("fake".into()),..good};
        assert!(validate_jobs_query(&invalid).is_err());
        assert_eq!(percent(3,4),Some(75.0));
        assert_eq!(percent(0,0),None);
    }

    #[sqlx::test(migrations="../../migrations")]
    async fn no_cookie_denied_even_if_admin_id_is_configured(pool: sqlx::PgPool) {
        let error=require_admin(&pool,&CookieJar::new()).await.unwrap_err();
        assert_eq!(error.0,StatusCode::UNAUTHORIZED);
    }
}
