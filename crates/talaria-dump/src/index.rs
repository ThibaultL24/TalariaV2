// crates/talaria-dump/src/index.rs
use serde::{Deserialize, Serialize};
use std::io::{BufRead, BufReader, Write};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DumpIndexEntry {
    pub offset: u64,
    pub page_id: u64,
    pub title: String,
}

/// Parse Wikimedia multistream index (tab-separated: byte offset, page id, namespace:title).
pub fn read_multistream_index(path: &Path) -> anyhow::Result<Vec<DumpIndexEntry>> {
    let file = std::fs::File::open(path)?;
    let reader = BufReader::new(file);
    let mut entries = Vec::new();

    for line in reader.lines() {
        let line = line?;
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let parts: Vec<&str> = line.split(':').collect();
        if parts.len() < 3 {
            continue;
        }
        let offset: u64 = parts[0].parse().unwrap_or(0);
        let page_id: u64 = parts[1].parse().unwrap_or(0);
        let title = parts[2..].join(":");
        if title.is_empty() {
            continue;
        }
        entries.push(DumpIndexEntry {
            offset,
            page_id,
            title,
        });
    }

    Ok(entries)
}

pub fn write_index_jsonl(path: &Path, entries: &[DumpIndexEntry]) -> anyhow::Result<()> {
    let file = std::fs::File::create(path)?;
    let mut writer = std::io::BufWriter::new(file);
    for entry in entries {
        serde_json::to_writer(&mut writer, entry)?;
        writer.write_all(b"\n")?;
    }
    Ok(())
}

pub fn read_index_jsonl(path: &Path) -> anyhow::Result<Vec<DumpIndexEntry>> {
    let file = std::fs::File::open(path)?;
    let reader = BufReader::new(file);
    let mut entries = Vec::new();
    for line in reader.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        entries.push(serde_json::from_str(&line)?);
    }
    Ok(entries)
}
