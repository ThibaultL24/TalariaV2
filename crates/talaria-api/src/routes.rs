// crates/talaria-api/src/routes.rs
mod entities;
mod events;

use axum::{routing::get, Json, Router};
use entities::{get_entity, search as search_entities};
use events::{detail, evidence, geojson, timeline};
use serde_json::{json, Value};
use std::net::SocketAddr;
use std::path::Path;
use talaria_core::AppConfig;
use talaria_store::{connect, run_migrations, DbPool};
use tower_http::cors::CorsLayer;
use tower_http::services::{ServeDir, ServeFile};
use tower_http::trace::TraceLayer;

pub async fn serve(config: AppConfig) -> anyhow::Result<()> {
    let pool = connect(&config).await?;
    run_migrations(&pool).await?;

    let app = Router::new()
        .route("/health", get(health))
        .route("/up", get(health))
        .route("/api/v1/status", get(status))
        .route("/api/v1/entities/search", get(search_entities))
        .route("/api/v1/entities/{entity_id}", get(get_entity))
        .route("/api/v1/timeline", get(timeline))
        .route("/api/v1/events/geojson", get(geojson))
        .route("/api/v1/events/{event_id}", get(detail))
        .route("/api/v1/events/{event_id}/evidence", get(evidence))
        .with_state(AppState { pool })
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http());

    let app = attach_web_ui(app);

    let addr: SocketAddr = config.bind_addr.parse()?;
    tracing::info!(%addr, "listening");
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

#[derive(Clone)]
pub struct AppState {
    pub pool: DbPool,
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

    Json(json!({
        "engine": "talaria-engine",
        "version": env!("CARGO_PKG_VERSION"),
        "counts": {
            "wiki_pages": pages,
            "sentences": sentences,
            "phrase_candidates": candidates,
            "canonical_events": events
        }
    }))
}

fn attach_web_ui(app: Router) -> Router {
    let dist = Path::new("web/dist");
    let index = dist.join("index.html");
    if dist.is_dir() && index.is_file() {
        app.fallback_service(
            ServeDir::new(dist).not_found_service(ServeFile::new(index)),
        )
    } else {
        app
    }
}
