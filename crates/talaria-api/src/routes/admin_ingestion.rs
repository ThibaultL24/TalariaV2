//! Admin-only, read-only ingestion diagnostics using existing SIWE sessions.
use super::AppState;
use axum::{
    extract::{Path, Query, Request, State},
    http::{header, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use axum_extra::extract::cookie::CookieJar;
use chrono::{DateTime, Duration, Utc};
use serde::Deserialize;
use serde_json::{json, Value};
use talaria_store::ingestion_observatory as store;
use uuid::Uuid;
type ApiError = (StatusCode, Json<Value>);
fn error(status: StatusCode, code: &str) -> ApiError {
    (status, Json(json!({"error":code})))
}
fn db_error(e: sqlx::Error) -> ApiError {
    tracing::error!(error=%e,"administration query failed");
    error(StatusCode::INTERNAL_SERVER_ERROR, "database_error")
}
pub(super) async fn require_admin(pool: &sqlx::PgPool, jar: &CookieJar) -> Result<(), ApiError> {
    let token = jar
        .get(crate::auth::SESSION_COOKIE)
        .ok_or_else(|| error(StatusCode::UNAUTHORIZED, "unauthenticated"))?;
    let user = talaria_store::get_active_session_user(
        pool,
        &talaria_store::hash_session_token(token.value()),
        Utc::now(),
    )
    .await
    .map_err(|_| error(StatusCode::INTERNAL_SERVER_ERROR, "database_error"))?
    .ok_or_else(|| error(StatusCode::UNAUTHORIZED, "unauthenticated"))?;
    let allowed: bool =
        sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM administration_users WHERE user_id=$1)")
            .bind(user.id)
            .fetch_one(pool)
            .await
            .map_err(db_error)?;
    if !allowed {
        return Err(error(StatusCode::FORBIDDEN, "administrator_required"));
    }
    Ok(())
}
async fn authorize(
    State(state): State<AppState>,
    jar: CookieJar,
    req: Request,
    next: Next,
) -> Response {
    let mut response = match require_admin(&state.pool, &jar).await {
        Ok(()) => next.run(req).await,
        Err(e) => e.into_response(),
    };
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        axum::http::HeaderValue::from_static("no-store"),
    );
    response
}
pub fn router(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/api/v1/admin/ingestion/overview", get(overview))
        .route("/api/v1/admin/ingestion/jobs", get(jobs))
        .route("/api/v1/admin/ingestion/providers", get(providers))
        .route("/api/v1/admin/ingestion/jobs/{job_id}", get(detail))
        .route(
            "/api/v1/ingest/{job_id}/metrics",
            get(super::ingest::get_ingest_metrics),
        )
        .route_layer(middleware::from_fn_with_state(state, authorize))
}
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Filters {
    from: Option<DateTime<Utc>>,
    until: Option<DateTime<Utc>>,
    lane: Option<String>,
    status: Option<String>,
    provider: Option<String>,
    qid: Option<String>,
    subject: Option<String>,
    limit: Option<i64>,
    offset: Option<i64>,
    sort: Option<String>,
}
impl Filters {
    fn validate(self) -> Result<(store::Filter, String), ApiError> {
        let until = self.until.unwrap_or_else(Utc::now);
        let from = self.from.unwrap_or(until - Duration::days(7));
        let limit = self.limit.unwrap_or(25);
        let offset = self.offset.unwrap_or(0);
        let sort = self.sort.unwrap_or_else(|| "slowest".into());
        let valid = from < until
            && until - from <= Duration::days(93)
            && (1..=100).contains(&limit)
            && (0..=10000).contains(&offset)
            && self
                .lane
                .as_deref()
                .is_none_or(|s| matches!(s, "explorer" | "agora"))
            && self
                .status
                .as_deref()
                .is_none_or(|s| matches!(s, "queued" | "running" | "done" | "failed"))
            && self.qid.as_deref().is_none_or(|s| {
                s.len() >= 2
                    && s.len() <= 20
                    && s.starts_with('Q')
                    && s.as_bytes()[1] != b'0'
                    && s[1..].bytes().all(|b| b.is_ascii_digit())
            })
            && self.provider.as_deref().is_none_or(|s| {
                (2..=64).contains(&s.len())
                    && s.bytes()
                        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
            })
            && self.subject.as_deref().is_none_or(|s| {
                (2..=240).contains(&s.chars().count()) && !s.chars().any(char::is_control)
            })
            && matches!(sort.as_str(), "slowest" | "calls" | "errors");
        if !valid {
            return Err(error(StatusCode::BAD_REQUEST, "invalid_filters"));
        }
        Ok((
            store::Filter {
                from,
                until,
                lane: self.lane,
                status: self.status,
                provider: self.provider,
                qid: self.qid,
                subject: self.subject,
                limit,
                offset,
            },
            sort,
        ))
    }
}
async fn overview(
    State(s): State<AppState>,
    Query(f): Query<Filters>,
) -> Result<Json<store::Overview>, ApiError> {
    let (f, _) = f.validate()?;
    Ok(Json(store::overview(&s.pool, &f).await.map_err(db_error)?))
}
async fn jobs(
    State(s): State<AppState>,
    Query(f): Query<Filters>,
) -> Result<Json<store::Page<store::Job>>, ApiError> {
    let (f, _) = f.validate()?;
    Ok(Json(store::jobs(&s.pool, &f).await.map_err(db_error)?))
}
async fn providers(
    State(s): State<AppState>,
    Query(f): Query<Filters>,
) -> Result<Json<store::Page<store::Provider>>, ApiError> {
    let (f, sort) = f.validate()?;
    Ok(Json(
        store::providers(&s.pool, &f, &sort)
            .await
            .map_err(db_error)?,
    ))
}
#[derive(serde::Serialize, sqlx::FromRow)]
struct Checkpoint {
    source_kind: String,
    connector_version: String,
    completed_at: DateTime<Utc>,
}
async fn detail(
    State(s): State<AppState>,
    Path(raw): Path<String>,
    Query(q): Query<super::ingest::IngestMetricsQuery>,
) -> Result<Json<Value>, ApiError> {
    let id = Uuid::parse_str(&raw).map_err(|_| error(StatusCode::BAD_REQUEST, "invalid_job_id"))?;
    let limit = q.limit.unwrap_or(50);
    let offset = q.offset.unwrap_or(0);
    if !(1..=100).contains(&limit) || !(0..=10000).contains(&offset) {
        return Err(error(StatusCode::BAD_REQUEST, "invalid_metrics_pagination"));
    }
    let job = store::job(&s.pool, id)
        .await
        .map_err(db_error)?
        .ok_or_else(|| error(StatusCode::NOT_FOUND, "job_not_found"))?;
    let checkpoints=sqlx::query_as::<_,Checkpoint>("SELECT source_kind,connector_version,completed_at FROM ingest_provider_checkpoints WHERE job_id=$1 ORDER BY source_kind LIMIT $2 OFFSET $3")
        .bind(id).bind(limit).bind(offset).fetch_all(&s.pool).await.map_err(db_error)?;
    let checkpoint_total: i64 =
        sqlx::query_scalar("SELECT count(*) FROM ingest_provider_checkpoints WHERE job_id=$1")
            .bind(id)
            .fetch_one(&s.pool)
            .await
            .map_err(db_error)?;
    let Json(metrics) = super::ingest::get_ingest_metrics(State(s), Path(id), Query(q)).await?;
    Ok(Json(
        json!({"job":job,"metrics":metrics,"checkpoints":{"items":checkpoints,"total":checkpoint_total,"limit":limit,"offset":offset}}),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn validates_period_lane_identity_and_pagination() {
        for input in [
            json!({"limit":0}),
            json!({"limit":101}),
            json!({"offset":-1}),
            json!({"offset":10001}),
            json!({"lane":"legacy"}),
            json!({"qid":"Q0"}),
            json!({"qid":"Q12 OR 1=1"}),
            json!({"provider":"HAL;"}),
            json!({"from":"2026-01-01T00:00:00Z","until":"2027-01-01T00:00:00Z"}),
            json!({"from":"2026-01-02T00:00:00Z","until":"2026-01-01T00:00:00Z"}),
        ] {
            assert!(serde_json::from_value::<Filters>(input)
                .unwrap()
                .validate()
                .is_err());
        }
        assert!(serde_json::from_value::<Filters>(
            json!({"lane":"explorer","qid":"Q517","limit":100})
        )
        .unwrap()
        .validate()
        .is_ok());
    }
    #[sqlx::test(migrations = "../../migrations")]
    async fn authorization_guards_all_routes_and_redacts_legacy_status(pool: sqlx::PgPool) {
        use axum::{
            body::{to_bytes, Body},
            http::Request,
        };
        use tower::ServiceExt;
        let state = AppState {
            pool: pool.clone(),
            offline_only: true,
            config: talaria_core::AppConfig::from_env().unwrap(),
            ingest_jobs: Default::default(),
        };
        let app = router(state.clone()).with_state(state.clone());
        let user: Uuid = sqlx::query_scalar("INSERT INTO users DEFAULT VALUES RETURNING id")
            .fetch_one(&pool)
            .await
            .unwrap();
        talaria_store::insert_session(
            &pool,
            user,
            &talaria_store::hash_session_token("test-session"),
            Utc::now() + Duration::hours(1),
        )
        .await
        .unwrap();
        let id = Uuid::new_v4();
        sqlx::query("INSERT INTO ingest_jobs(id,lane,subject,qid,identity_key,request_json,status,error,report_json) VALUES($1,'explorer','Test subject','Q1','qid:Q1','{}','failed','SECRET_CONNECTOR_TOKEN','{\"token\":\"SECRET_CONNECTOR_TOKEN\"}')").bind(id).execute(&pool).await.unwrap();
        let paths = vec![
            "/api/v1/admin/ingestion/overview".to_string(),
            "/api/v1/admin/ingestion/jobs".into(),
            "/api/v1/admin/ingestion/providers".into(),
            format!("/api/v1/admin/ingestion/jobs/{id}"),
            format!("/api/v1/ingest/{id}/metrics"),
        ];
        for path in &paths {
            let response = app
                .clone()
                .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
            let response = app
                .clone()
                .oneshot(
                    Request::builder()
                        .uri(path)
                        .header("cookie", "talaria_session=test-session")
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::FORBIDDEN);
        }
        sqlx::query("INSERT INTO administration_users(user_id) VALUES($1)")
            .bind(user)
            .execute(&pool)
            .await
            .unwrap();
        for path in &paths {
            let response = app
                .clone()
                .oneshot(
                    Request::builder()
                        .uri(path)
                        .header("cookie", "talaria_session=test-session")
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::OK, "{path}");
            assert_eq!(response.headers()["cache-control"], "no-store");
            let bytes = to_bytes(response.into_body(), 1_000_000).await.unwrap();
            assert!(!String::from_utf8_lossy(&bytes).contains("SECRET_CONNECTOR_TOKEN"));
        }
        for path in [
            "/api/v1/admin/ingestion/jobs?limit=101",
            "/api/v1/admin/ingestion/jobs?lane=legacy",
            "/api/v1/admin/ingestion/jobs/not-uuid",
        ] {
            let response = app
                .clone()
                .oneshot(
                    Request::builder()
                        .uri(path)
                        .header("cookie", "talaria_session=test-session")
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        }
        let Json(public) = super::super::ingest::get_ingest_job(State(state.clone()), Path(id))
            .await
            .unwrap();
        let Json(progress) = super::super::ingest::get_explorer_status(State(state), Path(id))
            .await
            .unwrap();
        assert!(!public.to_string().contains("SECRET_CONNECTOR_TOKEN"));
        assert!(!progress.to_string().contains("SECRET_CONNECTOR_TOKEN"));
        sqlx::query("DELETE FROM administration_users WHERE user_id=$1")
            .bind(user)
            .execute(&pool)
            .await
            .unwrap();
        let response = app
            .oneshot(
                Request::builder()
                    .uri(&paths[0])
                    .header("cookie", "talaria_session=test-session")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }
}
