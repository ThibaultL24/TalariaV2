// crates/talaria-api/src/main.rs
mod cli;
mod cosmos;
mod geocode;
mod ingest;
mod judge;
mod narrative_dossier;
mod quality;
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
        Commands::QualityFixture {
            title,
            file,
            assemble,
        } => {
            let text = std::fs::read_to_string(&file)?;
            let stats = quality::run_quality_fixture(&config, &title, &text, assemble).await?;
            tracing::info!(?stats, "quality fixture complete");
        }
        Commands::QualityNapoleonDemo => {
            let report = quality::run_quality_napoleon_demo(&config).await?;
            println!("---\n{report}");
        }
        Commands::QualityReport => {
            quality::run_quality_report(&config).await?;
        }
        Commands::QualitySupersedeDeath {
            subject,
            year,
            place,
        } => {
            let id = quality::run_quality_supersede_death(&config, &subject, year, &place).await?;
            tracing::info!(%id, "supersession done");
        }
        Commands::SourceRegistry { live } => ingest::run_source_registry(live).await?,
        Commands::PlanSources { subject, qid } => {
            ingest::run_plan_sources(&subject, qid.as_deref()).await?
        }
        Commands::IngestQuality {
            subject,
            qid,
            sources,
            fixture,
            live,
        } => {
            let report = ingest::run_ingest_quality(
                &config,
                &subject,
                qid.as_deref(),
                sources,
                fixture,
                live,
            )
            .await?;
            println!("---\n{report}");
        }
        Commands::DensityReport { subject } => {
            let pool = talaria_store::connect(&config).await?;
            talaria_store::run_migrations(&pool).await?;
            let sid = if let Some(label) = subject {
                Some(
                    talaria_store::upsert_entity_with_kind(&pool, &config.wiki_lang, &label, "person")
                        .await?,
                )
            } else {
                None
            };
            let counts = talaria_store::density_report_counts(&pool, sid).await?;
            println!("{}", serde_json::to_string_pretty(&serde_json::json!({
                "documents_discovered": counts.documents_discovered,
                "documents_snapshotted": counts.documents_snapshotted,
                "fragments": counts.fragments,
                "candidates": counts.candidates,
                "rejected": counts.rejected,
                "needs_review": counts.needs_review,
                "claims": counts.claims,
                "accepted_events": counts.accepted_events,
                "timeline_eligible": counts.timeline_eligible,
                "map_eligible": counts.map_eligible,
                "events_without_place": counts.events_without_place,
                "multi_source_events": counts.multi_source_events,
            }))?);
        }
    }

    Ok(())
}
