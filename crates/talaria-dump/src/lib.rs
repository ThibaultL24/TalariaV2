// crates/talaria-dump/src/lib.rs
pub mod extract;
pub mod generic;
pub mod index;
pub mod layout;
pub mod pipeline;
pub mod readers;

pub use extract::{
    content_hash, default_index_path, parse_dump_date, ExtractStats, ParsedWikiPage,
};
pub use generic::{count_records, hash_dump_file, DumpReader, DumpRecord, DumpTime};
pub use index::{read_multistream_index, write_index_jsonl, DumpIndexEntry};
pub use layout::ensure_data_dirs;
pub use pipeline::{build_extract_job, run_page_extraction, PageExtractJob};
pub use readers::JsonlDumpReader;
