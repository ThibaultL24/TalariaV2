// crates/talaria-api/src/main.rs
mod cli;
mod cosmos;
mod geocode;
mod ingest;
mod judge;
mod lot_e;
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
            seed_list,
            target_timeline_events,
            target_map_events,
            max_documents,
            max_depth,
            max_documents_per_source,
            max_titles,
            wiki_lang,
            resume: _,
        } => {
            if live && seed_list.is_some() {
                let seeds = seed_list.unwrap_or_else(lot_e::default_napoleon_seed);
                let targets = talaria_sources::DensityTargets {
                    target_timeline_events,
                    target_map_events,
                    max_documents,
                    max_linked_entities: 5_000,
                    max_depth,
                    max_documents_per_source,
                };
                let _ = lot_e::run_lot_e_density_ingest(
                    &config,
                    &subject,
                    qid.as_deref(),
                    &seeds,
                    targets,
                    &wiki_lang,
                    max_titles.filter(|n| *n > 0),
                )
                .await?;
            } else {
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
        }
        Commands::ResolvePlaces {
            subject,
            all_unresolved,
            live: _,
        } => {
            let report = lot_e::run_resolve_places(&config, &subject, all_unresolved).await?;
            println!("{report}");
        }
        Commands::DensityReport {
            subject,
            show_bottlenecks,
            show_source_coverage,
            show_unresolved_places,
        } => {
            let report = lot_e::run_density_report(
                &config,
                subject.as_deref(),
                show_bottlenecks,
                show_source_coverage,
                show_unresolved_places,
            )
            .await?;
            println!("{report}");
        }
        Commands::SourceStatus | Commands::ConnectorReport { subject: _ } => {
            println!("{}", lot_e::connector_status_json());
        }
        Commands::ExplorationReport { subject } => {
            let report = lot_e::run_exploration_report(&config, &subject).await?;
            println!("{report}");
        }
    }

    Ok(())
}
