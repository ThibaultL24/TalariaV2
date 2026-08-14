// crates/talaria-dump/src/readers/jsonl.rs
use crate::generic::{DumpReader, DumpRecord};
use anyhow::{anyhow, Context};
use std::fs::File;
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};

const READER_ID: &str = "jsonl";
const READER_VERSION: &str = "1";

pub struct JsonlDumpReader {
    path: PathBuf,
    inner: Box<dyn BufRead + Send>,
    line_no: u64,
    byte_offset: u64,
    records_emitted: u64,
    invalid_records: u64,
}

impl JsonlDumpReader {
    pub fn open(path: impl AsRef<Path>) -> anyhow::Result<Self> {
        let path = path.as_ref().to_path_buf();
        let inner = open_buf(&path)?;
        Ok(Self {
            path,
            inner,
            line_no: 0,
            byte_offset: 0,
            records_emitted: 0,
            invalid_records: 0,
        })
    }

    pub fn invalid_records(&self) -> u64 {
        self.invalid_records
    }

    pub fn records_emitted(&self) -> u64 {
        self.records_emitted
    }

    fn reopen(&mut self) -> anyhow::Result<()> {
        self.inner = open_buf(&self.path)?;
        self.line_no = 0;
        self.byte_offset = 0;
        Ok(())
    }
}

fn open_buf(path: &Path) -> anyhow::Result<Box<dyn BufRead + Send>> {
    let file = File::open(path).with_context(|| format!("open dump {}", path.display()))?;
    let reader: Box<dyn Read + Send> = if is_gzip(path) {
        Box::new(flate2::read::GzDecoder::new(file))
    } else {
        Box::new(file)
    };
    Ok(Box::new(BufReader::new(reader)))
}

fn is_gzip(path: &Path) -> bool {
    path.extension().and_then(|e| e.to_str()) == Some("gz")
}

fn parse_line(line: &str) -> anyhow::Result<DumpRecord> {
    let record: DumpRecord = serde_json::from_str(line).map_err(|e| anyhow!(e))?;
    if !record.is_usable() {
        anyhow::bail!("dump record missing external_id or text");
    }
    Ok(record)
}

impl DumpReader for JsonlDumpReader {
    fn reader_id(&self) -> &'static str {
        READER_ID
    }

    fn version(&self) -> &'static str {
        READER_VERSION
    }

    fn next_record(&mut self) -> anyhow::Result<Option<DumpRecord>> {
        let mut line = String::new();
        loop {
            line.clear();
            let n = self.inner.read_line(&mut line)?;
            if n == 0 {
                return Ok(None);
            }
            self.line_no += 1;
            self.byte_offset += n as u64;
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            match parse_line(trimmed) {
                Ok(record) => {
                    self.records_emitted += 1;
                    return Ok(Some(record));
                }
                Err(_) => {
                    self.invalid_records += 1;
                }
            }
        }
    }

    fn checkpoint(&self) -> serde_json::Value {
        serde_json::json!({
            "line_no": self.line_no,
            "byte_offset": self.byte_offset,
            "records_emitted": self.records_emitted,
            "invalid_records": self.invalid_records,
        })
    }

    fn restore(&mut self, cursor: &serde_json::Value) -> anyhow::Result<()> {
        let line_no = cursor
            .get("line_no")
            .and_then(|v| v.as_u64())
            .ok_or_else(|| anyhow!("cursor missing line_no"))?;
        self.reopen()?;
        let mut buf = String::new();
        while self.line_no < line_no {
            buf.clear();
            let n = self.inner.read_line(&mut buf)?;
            if n == 0 {
                break;
            }
            self.line_no += 1;
            self.byte_offset += n as u64;
        }
        self.records_emitted = cursor
            .get("records_emitted")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        self.invalid_records = cursor
            .get("invalid_records")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generic::count_records;
    use flate2::write::GzEncoder;
    use flate2::Compression;
    use std::io::Write;

    fn fixture_path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/dumps/mini_events.jsonl")
    }

    fn drain_ids(reader: &mut dyn DumpReader) -> Vec<String> {
        let mut ids = Vec::new();
        while let Some(record) = reader.next_record().unwrap() {
            ids.push(record.external_id);
        }
        ids
    }

    #[test]
    fn streams_mini_fixture_without_loading_all_up_front() {
        let mut reader = JsonlDumpReader::open(fixture_path()).unwrap();
        assert_eq!(reader.reader_id(), "jsonl");
        assert_eq!(reader.version(), "1");

        let first = reader.next_record().unwrap().unwrap();
        assert_eq!(first.external_id, "fixture:napoleon-bio");
        assert!(first.text.contains("Ajaccio"));
        assert_eq!(reader.records_emitted(), 1);

        let n = count_records(&mut reader).unwrap();
        assert_eq!(n, 7);
        assert_eq!(reader.records_emitted(), 8);
        assert_eq!(reader.next_record().unwrap(), None);
    }

    #[test]
    fn invalid_record_is_isolated() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("mixed.jsonl");
        std::fs::write(
            &path,
            concat!(
                "{\"external_id\":\"a\",\"text\":\"Ada was born in 1815 in London.\"}\n",
                "this is not json\n",
                "{\"external_id\":\"\",\"text\":\"missing id\"}\n",
                "{\"external_id\":\"b\",\"text\":\"\"}\n",
                "{\"external_id\":\"c\",\"text\":\"Marie Curie worked in Paris in 1898.\"}\n",
            ),
        )
        .unwrap();

        let mut reader = JsonlDumpReader::open(&path).unwrap();
        let ids = drain_ids(&mut reader);
        assert_eq!(ids, vec!["a", "c"]);
        assert_eq!(reader.invalid_records(), 3);
    }

    #[test]
    fn restore_continues_after_checkpoint() {
        let mut reader = JsonlDumpReader::open(fixture_path()).unwrap();
        let first = reader.next_record().unwrap().unwrap();
        let second = reader.next_record().unwrap().unwrap();
        let cursor = reader.checkpoint();
        assert_eq!(first.external_id, "fixture:napoleon-bio");
        assert_eq!(second.external_id, "fixture:waterloo");

        let mut resumed = JsonlDumpReader::open(fixture_path()).unwrap();
        resumed.restore(&cursor).unwrap();
        let third = resumed.next_record().unwrap().unwrap();
        assert_eq!(third.external_id, "fixture:amiens");
        assert_eq!(resumed.records_emitted(), 3);
    }

    #[test]
    fn gzip_jsonl_streams_same_records() {
        let raw = std::fs::read(fixture_path()).unwrap();
        let dir = tempfile::tempdir().unwrap();
        let gz_path = dir.path().join("mini_events.jsonl.gz");
        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(&raw).unwrap();
        std::fs::write(&gz_path, encoder.finish().unwrap()).unwrap();

        let mut plain = JsonlDumpReader::open(fixture_path()).unwrap();
        let mut gzip = JsonlDumpReader::open(&gz_path).unwrap();
        assert_eq!(drain_ids(&mut plain), drain_ids(&mut gzip));
    }
}
