// crates/talaria-api/src/main.rs
mod cli;
mod cosmos;
mod geocode;
mod judge;
mod narrative_dossier;
mod routes;

use clap::Parser;
use cli::{Cli, Commands};
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

    match cli.command {
        Commands::Migrate => cli::run_migrate(&config).await?,
        Commands::Serve => routes::serve(config).await?,
        Commands::DumpIndex { index, limit } => cli::run_dump_index(&config, &index, limit).await?,
        Commands::ExtractPages {
            dump,
            index,
            limit,
            main_namespace,
            skip_existing,
        } => {
            cli::run_extract_pages(&config, dump, index, limit, main_namespace, skip_existing).await?
        }
        Commands::DataInit => {
            talaria_dump::ensure_data_dirs(&config)?;
            tracing::info!(root = %config.data_root.display(), "data directories ready");
        }
        Commands::SplitSentences { limit, skip_existing } => {
            cli::run_split_sentences(&config, limit, skip_existing).await?
        }
        Commands::CosmosExtract {
            batch_size,
            limit,
            skip_existing,
            mock,
        } => {
            cosmos::run_cosmos_extract(&config, batch_size, limit, skip_existing, mock).await?
        }
        Commands::JudgeCandidates { limit } => judge::run_judge_candidates(&config, limit).await?,
        Commands::GeocodePlaces { limit } => geocode::run_geocode_places(&config, limit).await?,
    }

    Ok(())
}
