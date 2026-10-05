// crates/talaria-api/src/main.rs
#![allow(clippy::too_many_arguments)]
#![allow(clippy::collapsible_if)]
#![allow(clippy::redundant_closure)]
#![allow(clippy::useless_format)]
#![allow(clippy::if_same_then_else)]
#![allow(clippy::needless_borrows_for_generic_args)]
#![allow(clippy::unnecessary_cast)]
#![allow(dead_code)]

mod agora_stance;
mod claim_extract;
mod cli;
mod cli_dispatch;
mod cli_helpers;
mod corpus_ingest;
mod cosmos;
mod dump_mine;
mod display_i18n;
mod dump_cosmos;
mod dump_events;
mod dump_ingest;
mod geocode;
mod historiography;
mod ingest;
mod intuition;
mod judge;
mod llm;
mod lot_e;
mod lot_e_reports;
mod narrative_dossier;
mod person_ingest;
mod place_conflict;
mod quality;
mod rebuild;
mod routes;
mod visit_audit;
mod visit_enrich;
mod visit_live;
mod wiki_persist;
mod wikidata_ingest;

use clap::Parser;
use cli::Cli;
use talaria_core::AppConfig;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let cli = Cli::parse();
    let config = AppConfig::from_env()?;
    cli_dispatch::dispatch(cli, config).await
}
