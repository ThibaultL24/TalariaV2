// crates/talaria-dump/src/pipeline.rs
use crate::extract::{
    default_index_path, extract_pages_from_dump, parse_dump_date, ExtractOptions, ExtractStats,
    ParsedWikiPage,
};
use crate::index::{read_index_jsonl, read_multistream_index};
use crate::layout::dump_index_path;
use std::path::{Path, PathBuf};
use talaria_core::AppConfig;

#[derive(Debug, Clone)]
pub struct PageExtractJob {
    pub dump_path: PathBuf,
    pub index_path: PathBuf,
    pub wiki_lang: String,
    pub dump_date: Option<chrono::NaiveDate>,
    pub main_namespace_only: bool,
    pub limit: usize,
}

pub fn resolve_index_path(dump_path: &Path, index_path: Option<PathBuf>) -> PathBuf {
    index_path.unwrap_or_else(|| default_index_path(dump_path))
}

pub fn load_index_entries(index_path: &Path, dump_path: &Path) -> anyhow::Result<Vec<crate::index::DumpIndexEntry>> {
    let jsonl = dump_index_path(dump_path);
    if jsonl.exists() {
        return read_index_jsonl(&jsonl);
    }
    read_multistream_index(index_path)
}

pub fn build_extract_job(
    config: &AppConfig,
    dump_path: PathBuf,
    index_path: Option<PathBuf>,
    limit: usize,
    main_namespace_only: bool,
) -> anyhow::Result<PageExtractJob> {
    let index_path = resolve_index_path(&dump_path, index_path);
    if !dump_path.exists() {
        anyhow::bail!("dump file not found: {}", dump_path.display());
    }
    if !index_path.exists() {
        anyhow::bail!("index file not found: {}", index_path.display());
    }

    let dump_date = parse_dump_date(&dump_path);

    Ok(PageExtractJob {
        dump_path,
        index_path,
        wiki_lang: config.wiki_lang.clone(),
        dump_date,
        main_namespace_only,
        limit,
    })
}

pub fn run_page_extraction(job: &PageExtractJob) -> anyhow::Result<(Vec<ParsedWikiPage>, ExtractStats)> {
    let mut index_entries = load_index_entries(&job.index_path, &job.dump_path)?;
    if job.limit > 0 && index_entries.len() > job.limit {
        index_entries.truncate(job.limit);
    }

    let options = ExtractOptions {
        dump_path: job.dump_path.clone(),
        index_entries,
        main_namespace_only: job.main_namespace_only,
        limit: job.limit,
    };

    extract_pages_from_dump(&options)
}
