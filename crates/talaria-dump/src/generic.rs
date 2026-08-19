// crates/talaria-dump/src/generic.rs
//! Format-agnostic dump streaming. Wikipedia XML is one reader, not the core.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::io::Read;
use std::path::Path;

/// Document-level time. Same serde shape as `talaria_quality::TypedTime`
/// so later lots can map without a dump → quality dependency.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DumpTime {
    Exact {
        year: i32,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        month: Option<u32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        day: Option<u32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        surface: Option<String>,
    },
    Range {
        start_year: i32,
        end_year: i32,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        surface: Option<String>,
    },
    Approx {
        year: i32,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        surface: Option<String>,
    },
    Unknown {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        surface: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DumpRecord {
    pub external_id: String,
    #[serde(default)]
    pub title: Option<String>,
    pub text: String,
    #[serde(default)]
    pub language: Option<String>,
    #[serde(default)]
    pub canonical_url: Option<String>,
    #[serde(default = "default_document_type")]
    pub document_type: String,
    #[serde(default)]
    pub published: Option<DumpTime>,
    #[serde(default)]
    pub contributors: Vec<String>,
    #[serde(default)]
    pub external_ids: Vec<(String, String)>,
    #[serde(default)]
    pub license: Option<String>,
    #[serde(default = "default_metadata")]
    pub provider_metadata: serde_json::Value,
}

fn default_document_type() -> String {
    "unspecified".to_string()
}

fn default_metadata() -> serde_json::Value {
    serde_json::json!({})
}

impl DumpRecord {
    pub fn is_usable(&self) -> bool {
        !self.external_id.trim().is_empty() && !self.text.trim().is_empty()
    }
}

pub trait DumpReader: Send {
    fn reader_id(&self) -> &'static str;
    fn version(&self) -> &'static str;
    fn next_record(&mut self) -> anyhow::Result<Option<DumpRecord>>;
    fn checkpoint(&self) -> serde_json::Value;
    fn restore(&mut self, cursor: &serde_json::Value) -> anyhow::Result<()>;
}

/// Drain a reader without collecting records in the reader itself.
pub fn count_records(reader: &mut dyn DumpReader) -> anyhow::Result<usize> {
    let mut n = 0;
    while reader.next_record()?.is_some() {
        n += 1;
    }
    Ok(n)
}

/// SHA-256 of dump bytes (streaming; gzip hashes compressed on-disk bytes).
pub fn hash_dump_file(path: &Path) -> anyhow::Result<String> {
    let mut file = std::fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 8192];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hex::encode(hasher.finalize()))
}
