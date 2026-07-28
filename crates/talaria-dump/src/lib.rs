// crates/talaria-dump/src/lib.rs
pub mod extract;
pub mod index;
pub mod layout;
pub mod pipeline;

pub use extract::{
    content_hash, default_index_path, parse_dump_date, ParsedWikiPage, ExtractStats,
};
pub use index::{DumpIndexEntry, read_multistream_index, write_index_jsonl};
pub use layout::ensure_data_dirs;
pub use pipeline::{build_extract_job, run_page_extraction, PageExtractJob};
