// crates/talaria-api/src/cli.rs
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use talaria_core::AppConfig;
use talaria_dump::{build_extract_job, content_hash, read_multistream_index, run_page_extraction, write_index_jsonl};
use talaria_store::{
    connect, finish_dump_run, list_pages_for_sentence_split, replace_sentences_for_page,
    run_migrations, start_dump_run, store_extracted_page, SentenceRecord, WikiPageRecord,
};
use talaria_text::segment_wikitext;

#[derive(Parser)]
#[command(name = "talaria", about = "Talaria Engine — Wikipedia dump pipeline")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Apply SQL migrations
    Migrate,
    /// Start HTTP API
    Serve,
    /// Create JSONL index from Wikimedia multistream-index.txt
    DumpIndex {
        #[arg(long, help = "Path to *-multistream-index.txt")]
        index: PathBuf,
        #[arg(long, default_value = "0", help = "Max entries (0 = all)")]
        limit: usize,
    },
    /// Extract wiki pages from multistream XML bz2 into DB + raw files
    ExtractPages {
        #[arg(long, help = "Path to *-pages-articles-multistream.xml.bz2")]
        dump: PathBuf,
        #[arg(long, help = "Path to *-multistream-index.txt (optional)")]
        index: Option<PathBuf>,
        #[arg(long, default_value = "0", help = "Max pages (0 = all indexed)")]
        limit: usize,
        #[arg(long, default_value_t = true, help = "Only namespace 0 (main articles)")]
        main_namespace: bool,
        #[arg(long, help = "Skip pages already stored with same content hash")]
        skip_existing: bool,
    },
    /// Create TALARIA_DATA_ROOT/dumps and parquet dirs
    DataInit,
    /// Split stored wiki pages into sentences
    SplitSentences {
        #[arg(long, default_value = "0", help = "Max pages to process (0 = all pending)")]
        limit: i64,
        #[arg(long, help = "Skip pages that already have sentences")]
        skip_existing: bool,
    },
    /// Run COSMOS sidecar on sentences → phrase_candidates
    CosmosExtract {
        #[arg(long, default_value = "32", help = "Batch size sent to Python sidecar")]
        batch_size: usize,
        #[arg(long, default_value = "0", help = "Max sentences (0 = all pending)")]
        limit: i64,
        #[arg(long, help = "Skip sentences that already have phrase candidates")]
        skip_existing: bool,
        #[arg(long, help = "Use rule-based mock extractor (no spaCy/COSMOS)")]
        mock: bool,
    },
    /// Judge pending phrase candidates → canonical_events
    JudgeCandidates {
        #[arg(long, default_value = "0", help = "Max candidates (0 = all pending)")]
        limit: i64,
    },
    /// Geocode place labels via Wikidata → update canonical_events geom
    GeocodePlaces {
        #[arg(long, default_value = "0", help = "Max place labels (0 = all pending)")]
        limit: i64,
    },
    /// Quality pipeline: fixture extract → EventCandidate → gates → assemble
    QualityFixture {
        #[arg(long, help = "Subject / page title")]
        title: String,
        #[arg(long, help = "Path to fixture text file")]
        file: PathBuf,
        #[arg(long, default_value_t = true, help = "Assemble accepted candidates")]
        assemble: bool,
    },
    /// Deterministic Napoleon quality demo + adversarial gates + report
    QualityNapoleonDemo,
    /// Print quality pipeline execution report
    QualityReport,
    /// Append-only supersession of active quality death event
    QualitySupersedeDeath {
        #[arg(long)]
        subject: String,
        #[arg(long)]
        year: i32,
        #[arg(long)]
        place: String,
    },
    /// List source connectors and implementation status
    SourceRegistry {
        #[arg(long, help = "Include live Wikimedia clients as implemented")]
        live: bool,
    },
    /// Plan sources for a subject (deterministic, auditable JSON)
    PlanSources {
        #[arg(long)]
        subject: String,
        #[arg(long)]
        qid: Option<String>,
    },
    /// Multi-source quality ingest (fixture by default; --live for Wikimedia APIs)
    IngestQuality {
        #[arg(long)]
        subject: String,
        #[arg(long)]
        qid: Option<String>,
        #[arg(long, value_delimiter = ',', help = "Optional source filter, e.g. wikidata,wikipedia,open_library,gallica,europeana")]
        sources: Option<Vec<String>>,
        #[arg(long, default_value_t = true, action = clap::ArgAction::Set, help = "Use deterministic fixture corpus")]
        fixture: bool,
        #[arg(long, help = "Call live Wikimedia + catalog APIs (Open Library, Internet Archive, Gallica; Europeana if EUROPEANA_API_KEY)")]
        live: bool,
        /// Lot E: seed-driven dense Wikipedia exploration toward density targets
        #[arg(long, help = "Path to seed title list (enables Lot E density ingest when --live)")]
        seed_list: Option<PathBuf>,
        #[arg(long, default_value_t = 500)]
        target_timeline_events: u32,
        #[arg(long, default_value_t = 500)]
        target_map_events: u32,
        #[arg(long, default_value_t = 10_000)]
        max_documents: u32,
        #[arg(long, default_value_t = 3)]
        max_depth: u16,
        #[arg(long, default_value_t = 2_500)]
        max_documents_per_source: u32,
        #[arg(long, help = "Cap seed titles processed (0 = all)")]
        max_titles: Option<u32>,
        #[arg(long, default_value = "en", help = "Wikipedia language for Lot E seed fetch")]
        wiki_lang: String,
        #[arg(long, help = "Resume flag reserved for exploration queue (accepted, Lot E uses seed cursor)")]
        resume: bool,
    },
    /// Resolve unresolved quality places (offline gazetteer / aliases)
    ResolvePlaces {
        #[arg(long)]
        subject: String,
        #[arg(long, default_value_t = true, help = "Resolve all unresolved timeline-eligible events")]
        all_unresolved: bool,
        #[arg(long, help = "Allow live Wikidata P625 (optional; offline used first)")]
        live: bool,
    },
    /// Density / multi-source quality report for a subject
    DensityReport {
        #[arg(long)]
        subject: Option<String>,
        #[arg(long, help = "Include structured bottleneck reasons")]
        show_bottlenecks: bool,
        #[arg(long, help = "Include source / connector coverage")]
        show_source_coverage: bool,
        #[arg(long, help = "List unresolved place labels")]
        show_unresolved_places: bool,
    },
    /// Print connector implementation maturity
    SourceStatus,
    /// Exploration queue / seed coverage report
    ExplorationReport {
        #[arg(long)]
        subject: String,
    },
    /// Connector maturity report (alias of source-status + subject context)
    ConnectorReport {
        #[arg(long)]
        subject: Option<String>,
    },
    /// Ingest Wikidata JSON dump → entity QIDs + occupation/position profiles
    WikidataIngest {
        #[arg(long, help = "Path to wikidata-*-all.json[.bz2] or sample JSON")]
        dump: Option<PathBuf>,
        #[arg(long, default_value = "0", help = "Max humans to ingest (0 = all)")]
        limit: usize,
    },
    /// Extract soft claims from sentences (+ backfill life_events from canonical events)
    ClaimsExtract {
        #[arg(long, default_value = "0", help = "Max sentences (0 = all pending)")]
        limit: i64,
    },
    /// Mine dump sentences for anecdotes and extra life-event keywords
    DumpMine {
        #[arg(long, default_value = "0", help = "Max sentences (0 = all)")]
        limit: i64,
    },
}

pub async fn run_migrate(config: &AppConfig) -> anyhow::Result<()> {
    let pool = connect(config).await?;
    run_migrations(&pool).await?;
    tracing::info!("migrations applied");
    Ok(())
}

pub async fn run_dump_index(
    config: &AppConfig,
    index_path: &std::path::Path,
    limit: usize,
) -> anyhow::Result<()> {
    talaria_dump::ensure_data_dirs(config)?;
    tracing::info!(path = %index_path.display(), "reading multistream index");
    let mut entries = read_multistream_index(index_path)?;
    if limit > 0 && entries.len() > limit {
        entries.truncate(limit);
    }
    let out = config.dumps_dir().join(
        index_path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string()
            + ".jsonl",
    );
    write_index_jsonl(&out, &entries)?;
    tracing::info!(count = entries.len(), out = %out.display(), "index written");
    Ok(())
}

pub async fn run_extract_pages(
    config: &AppConfig,
    dump_path: PathBuf,
    index_path: Option<PathBuf>,
    limit: usize,
    main_namespace: bool,
    skip_existing: bool,
) -> anyhow::Result<()> {
    talaria_dump::ensure_data_dirs(config)?;
    let pool = connect(config).await?;
    run_migrations(&pool).await?;

    let job = build_extract_job(config, dump_path.clone(), index_path, limit, main_namespace)?;
    let run_id = start_dump_run(
        &pool,
        job.dump_path.to_string_lossy().as_ref(),
        &job.wiki_lang,
    )
    .await?;

    tracing::info!(
        dump = %job.dump_path.display(),
        index = %job.index_path.display(),
        limit = job.limit,
        main_namespace = job.main_namespace_only,
        "starting page extraction"
    );

    let wiki_lang = job.wiki_lang.clone();
    let dump_date = job.dump_date;
    let (pages, stats) = tokio::task::spawn_blocking(move || run_page_extraction(&job)).await??;

    let mut stored = 0usize;
    let mut skipped = 0usize;

    for page in pages {
        let raw_path = config.page_file(&wiki_lang, page.page_id);
        if let Some(parent) = raw_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&raw_path, &page.text)?;

        let record = WikiPageRecord {
            page_id: page.page_id as i64,
            title: page.title,
            wiki_lang: wiki_lang.clone(),
            revision_id: page.revision_id.map(|id| id as i64),
            content_hash: content_hash(&page.text),
            dump_date,
            raw_path: raw_path.to_string_lossy().into_owned(),
        };

        if store_extracted_page(&pool, &record, skip_existing).await? {
            stored += 1;
        } else {
            skipped += 1;
        }
    }

    finish_dump_run(&pool, run_id, stored as i32, "completed").await?;

    tracing::info!(
        blocks_read = stats.blocks_read,
        pages_seen = stats.pages_seen,
        pages_matched = stats.pages_matched,
        stored,
        skipped,
        "page extraction complete"
    );

    Ok(())
}

pub async fn run_split_sentences(
    config: &AppConfig,
    limit: i64,
    skip_existing: bool,
) -> anyhow::Result<()> {
    let pool = connect(config).await?;
    run_migrations(&pool).await?;

    let page_limit = if limit > 0 { limit } else { i64::MAX };
    let pages =
        list_pages_for_sentence_split(&pool, &config.wiki_lang, page_limit, skip_existing).await?;

    tracing::info!(
        pages = pages.len(),
        skip_existing,
        "splitting wiki pages into sentences"
    );

    let mut pages_processed = 0usize;
    let mut sentences_stored = 0usize;
    let mut pages_skipped = 0usize;

    for page in pages {
        let Some(raw_path) = page.raw_path else {
            pages_skipped += 1;
            continue;
        };

        let wikitext = match std::fs::read_to_string(&raw_path) {
            Ok(text) => text,
            Err(err) => {
                tracing::warn!(title = %page.title, path = %raw_path, %err, "failed to read wiki file");
                pages_skipped += 1;
                continue;
            }
        };

        let wikitext_for_sections = wikitext.clone();
        let spans = tokio::task::spawn_blocking(move || segment_wikitext(&wikitext)).await?;

        let records: Vec<SentenceRecord> = spans
            .into_iter()
            .map(|span| SentenceRecord {
                ordinal: span.ordinal,
                text: span.text,
                char_start: Some(span.char_start),
                char_end: Some(span.char_end),
            })
            .collect();

        if records.is_empty() {
            tracing::debug!(title = %page.title, "no sentences extracted");
            pages_skipped += 1;
            continue;
        }

        let count = replace_sentences_for_page(&pool, page.id, &records).await?;

        let section_spans =
            tokio::task::spawn_blocking(move || talaria_text::split_wiki_sections(&wikitext_for_sections))
                .await?;
        let section_records: Vec<talaria_store::WikiSectionRecord> = section_spans
            .into_iter()
            .map(|section| {
                let plain = talaria_text::wikitext_to_plain(&section.wikitext);
                talaria_store::WikiSectionRecord {
                    ordinal: section.ordinal,
                    title: section.title,
                    text: plain,
                }
            })
            .filter(|section| !section.text.trim().is_empty())
            .collect();
        if !section_records.is_empty() {
            let _ = talaria_store::replace_sections_for_page(&pool, page.id, &section_records).await?;
        }

        pages_processed += 1;
        sentences_stored += count;

        if pages_processed.is_multiple_of(100) {
            tracing::info!(pages_processed, sentences_stored, "sentence split progress");
        }
    }

    tracing::info!(
        pages_processed,
        sentences_stored,
        pages_skipped,
        "sentence split complete"
    );

    Ok(())
}
