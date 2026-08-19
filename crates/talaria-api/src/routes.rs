// crates/talaria-api/src/routes.rs
mod documents;
mod entities;
mod events;
mod facets;
mod ingest;

use axum::{
    routing::{get, post},
    Json, Router,
};
use documents::{
    get_document, list_document_fragments, list_entity_bibliography, list_entity_documents,
};
use entities::{get_entity, list_claims, search as search_entities};
use events::{detail, evidence, geojson, timeline};
use facets::{list_periods, list_profiles};
use ingest::{
    get_ingest_job, start_agora_ingest, start_explorer_ingest, start_ingest, IngestJobMap,
};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::Path;
use std::sync::Arc;
use talaria_core::AppConfig;
use talaria_store::{connect, run_migrations, seed_default_periods, DbPool};
use tokio::sync::Mutex;
use tower_http::cors::CorsLayer;
use tower_http::services::{ServeDir, ServeFile};
use tower_http::trace::TraceLayer;

pub async fn serve(config: AppConfig) -> anyhow::Result<()> {
    let pool = connect(&config).await?;
    run_migrations(&pool).await?;
    let seeded = seed_default_periods(&pool).await.unwrap_or(0);
    tracing::info!(periods = seeded, "default periods ready");

    let ingest_jobs: IngestJobMap = Arc::new(Mutex::new(HashMap::new()));

    let offline_only = config.offline_only;
    let addr: SocketAddr = config.bind_addr.parse()?;

    let app = Router::new()
        .route("/health", get(health))
        .route("/up", get(health))
        .route("/api/v1/status", get(status))
        .route("/api/v1/entities/search", get(search_entities))
        .route("/api/v1/entities/{entity_id}", get(get_entity))
        .route("/api/v1/entities/{entity_id}/claims", get(list_claims))
        .route(
            "/api/v1/entities/{entity_id}/documents",
            get(list_entity_documents),
        )
        .route(
            "/api/v1/entities/{entity_id}/bibliography",
            get(list_entity_bibliography),
        )
        .route("/api/v1/documents/{document_id}", get(get_document))
        .route(
            "/api/v1/documents/{document_id}/fragments",
            get(list_document_fragments),
        )
        .route("/api/v1/periods", get(list_periods))
        .route("/api/v1/profiles", get(list_profiles))
        .route("/api/v1/timeline", get(timeline))
        .route("/api/v1/events/geojson", get(geojson))
        .route("/api/v1/events/{event_id}", get(detail))
        .route("/api/v1/events/{event_id}/evidence", get(evidence))
        .route("/api/v1/ingest", post(start_ingest))
        .route("/api/v1/ingest/explorer", post(start_explorer_ingest))
        .route("/api/v1/ingest/agora", post(start_agora_ingest))
        .route("/api/v1/ingest/{job_id}", get(get_ingest_job))
        .with_state(AppState {
            pool,
            offline_only,
            config,
            ingest_jobs,
        })
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http());

    let app = attach_web_ui(app);

    tracing::info!(%addr, offline_only, "listening");
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

#[derive(Clone)]
pub struct AppState {
    pub pool: DbPool,
    pub offline_only: bool,
    pub config: AppConfig,
    pub ingest_jobs: IngestJobMap,
}

async fn health() -> Json<Value> {
    Json(json!({ "status": "ok", "engine": "talaria" }))
}

async fn status(axum::extract::State(state): axum::extract::State<AppState>) -> Json<Value> {
    let pages: i64 = sqlx::query_scalar("SELECT COUNT(*)::bigint FROM wiki_pages")
        .fetch_one(&state.pool)
        .await
        .unwrap_or(0);
    let sentences: i64 = sqlx::query_scalar("SELECT COUNT(*)::bigint FROM sentences")
        .fetch_one(&state.pool)
        .await
        .unwrap_or(0);
    let candidates: i64 = sqlx::query_scalar("SELECT COUNT(*)::bigint FROM phrase_candidates")
        .fetch_one(&state.pool)
        .await
        .unwrap_or(0);
    let events: i64 = sqlx::query_scalar("SELECT COUNT(*)::bigint FROM canonical_events")
        .fetch_one(&state.pool)
        .await
        .unwrap_or(0);
    let profiles: i64 = sqlx::query_scalar("SELECT COUNT(*)::bigint FROM entity_profiles")
        .fetch_one(&state.pool)
        .await
        .unwrap_or(0);
    let claims: i64 = sqlx::query_scalar("SELECT COUNT(*)::bigint FROM soft_claims")
        .fetch_one(&state.pool)
        .await
        .unwrap_or(0);

    Json(json!({
        "engine": "talaria-engine",
        "version": env!("CARGO_PKG_VERSION"),
        "offline_only": state.offline_only,
        "counts": {
            "wiki_pages": pages,
            "sentences": sentences,
            "phrase_candidates": candidates,
            "canonical_events": events,
            "entity_profiles": profiles,
            "claims": claims
        }
    }))
}

fn attach_web_ui(app: Router) -> Router {
    let dist = Path::new("web/dist");
    let index = dist.join("index.html");
    if dist.is_dir() && index.is_file() {
        app.fallback_service(ServeDir::new(dist).not_found_service(ServeFile::new(index)))
    } else {
        app
    }
}
