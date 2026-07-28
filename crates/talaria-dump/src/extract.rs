// crates/talaria-dump/src/extract.rs
use crate::index::DumpIndexEntry;
use bzip2::read::BzDecoder;
use quick_xml::events::Event;
use quick_xml::Reader;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashSet};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedWikiPage {
    pub page_id: u64,
    pub title: String,
    pub namespace: i32,
    pub revision_id: Option<u64>,
    pub text: String,
}

#[derive(Debug, Clone, Default)]
pub struct ExtractStats {
    pub blocks_read: usize,
    pub pages_seen: usize,
    pub pages_matched: usize,
    pub pages_written: usize,
}

#[derive(Debug, Clone)]
pub struct ExtractOptions {
    pub dump_path: PathBuf,
    pub index_entries: Vec<DumpIndexEntry>,
    pub main_namespace_only: bool,
    pub limit: usize,
}

pub fn default_index_path(dump_path: &Path) -> PathBuf {
    let name = dump_path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("dump.xml.bz2");
    if let Some(stem) = name.strip_suffix(".xml.bz2") {
        dump_path.with_file_name(format!("{stem}-index.txt"))
    } else {
        dump_path.with_file_name(format!("{name}-index.txt"))
    }
}

pub fn parse_dump_date(dump_path: &Path) -> Option<chrono::NaiveDate> {
    let name = dump_path.file_name()?.to_str()?;
    let mut parts = name.split('-');
    parts.next()?;
    let date = parts.next()?;
    chrono::NaiveDate::parse_from_str(date, "%Y%m%d").ok()
}

pub fn group_index_by_offset(entries: &[DumpIndexEntry]) -> BTreeMap<u64, Vec<DumpIndexEntry>> {
    let mut groups = BTreeMap::new();
    for entry in entries {
        groups
            .entry(entry.offset)
            .or_insert_with(Vec::new)
            .push(entry.clone());
    }
    groups
}

pub fn content_hash(text: &str) -> String {
    hex::encode(Sha256::digest(text.as_bytes()))
}

pub fn read_bz2_block(dump_path: &Path, offset: u64) -> anyhow::Result<String> {
    let mut file = File::open(dump_path)?;
    file.seek(SeekFrom::Start(offset))?;
    let mut decoder = BzDecoder::new(&mut file);
    let mut xml = String::new();
    decoder.read_to_string(&mut xml)?;
    Ok(xml)
}

pub fn parse_pages_from_xml(xml: &str) -> anyhow::Result<Vec<ParsedWikiPage>> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut pages = Vec::new();
    let mut buf = Vec::new();

    let mut in_page = false;
    let mut in_revision = false;
    let mut current_tag = String::new();
    let mut page_id: Option<u64> = None;
    let mut title = String::new();
    let mut namespace: i32 = 0;
    let mut revision_id: Option<u64> = None;
    let mut text = String::new();

    loop {
        match reader.read_event_into(&mut buf)? {
            Event::Start(e) => {
                current_tag = String::from_utf8_lossy(e.name().as_ref()).into_owned();
                match current_tag.as_str() {
                    "page" => {
                        in_page = true;
                        page_id = None;
                        title.clear();
                        namespace = 0;
                        revision_id = None;
                        text.clear();
                    }
                    "revision" if in_page => in_revision = true,
                    _ => {}
                }
            }
            Event::End(e) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).into_owned();
                match name.as_str() {
                    "page" if in_page => {
                        if let Some(id) = page_id {
                            pages.push(ParsedWikiPage {
                                page_id: id,
                                title: title.clone(),
                                namespace,
                                revision_id,
                                text: text.clone(),
                            });
                        }
                        in_page = false;
                        in_revision = false;
                    }
                    "revision" => in_revision = false,
                    _ => {}
                }
            }
            Event::Text(e) if in_page => {
                let value = e.unescape()?.into_owned();
                match current_tag.as_str() {
                    "id" if !in_revision => page_id = value.parse().ok(),
                    "id" if in_revision => revision_id = value.parse().ok(),
                    "title" => title = value,
                    "ns" => namespace = value.parse().unwrap_or(0),
                    "text" => text = value,
                    _ => {}
                }
            }
            Event::CData(e) if in_page && current_tag == "text" => {
                text = String::from_utf8_lossy(e.as_ref()).into_owned();
            }
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }

    Ok(pages)
}

pub fn extract_pages_from_dump(options: &ExtractOptions) -> anyhow::Result<(Vec<ParsedWikiPage>, ExtractStats)> {
    let groups = group_index_by_offset(&options.index_entries);
    let mut stats = ExtractStats::default();
    let mut extracted = Vec::new();
    let mut remaining = options.limit;

    for (offset, entries) in groups {
        if options.limit > 0 && remaining == 0 {
            break;
        }

        stats.blocks_read += 1;
        let xml = read_bz2_block(&options.dump_path, offset)?;
        let block_pages = parse_pages_from_xml(&xml)?;
        stats.pages_seen += block_pages.len();

        let wanted_ids: HashSet<u64> = entries.iter().map(|e| e.page_id).collect();
        for page in block_pages {
            if !wanted_ids.contains(&page.page_id) {
                continue;
            }
            if options.main_namespace_only && page.namespace != 0 {
                continue;
            }
            stats.pages_matched += 1;
            extracted.push(page);
            stats.pages_written += 1;

            if options.limit > 0 {
                remaining -= 1;
                if remaining == 0 {
                    break;
                }
            }
        }
    }

    Ok((extracted, stats))
}

#[cfg(test)]
mod tests {
    use super::*;
    use bzip2::write::BzEncoder;
    use bzip2::Compression;
    use std::io::Write;

    const SAMPLE_XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<mediawiki>
  <page>
    <title>Ada Lovelace</title>
    <ns>0</ns>
    <id>123</id>
    <revision>
      <id>456</id>
      <text>Ada was born in 1815.</text>
    </revision>
  </page>
  <page>
    <title>Talk:Ada Lovelace</title>
    <ns>1</ns>
    <id>124</id>
    <revision>
      <id>457</id>
      <text>Discussion page</text>
    </revision>
  </page>
</mediawiki>"#;

    #[test]
    fn parse_pages_from_xml_extracts_main_and_talk() {
        let pages = parse_pages_from_xml(SAMPLE_XML).unwrap();
        assert_eq!(pages.len(), 2);
        assert_eq!(pages[0].page_id, 123);
        assert_eq!(pages[0].title, "Ada Lovelace");
        assert_eq!(pages[0].namespace, 0);
        assert_eq!(pages[0].text, "Ada was born in 1815.");
    }

    #[test]
    fn read_bz2_block_reads_one_member() {
        let dir = tempfile::tempdir().unwrap();
        let dump_path = dir.path().join("sample.xml.bz2");
        let mut encoder = BzEncoder::new(Vec::new(), Compression::best());
        encoder.write_all(SAMPLE_XML.as_bytes()).unwrap();
        let compressed = encoder.finish().unwrap();
        std::fs::write(&dump_path, compressed).unwrap();

        let xml = read_bz2_block(&dump_path, 0).unwrap();
        let pages = parse_pages_from_xml(&xml).unwrap();
        assert_eq!(pages.len(), 2);
    }

    #[test]
    fn parse_dump_date_from_filename() {
        let path = Path::new("/data/enwiki-20250601-pages-articles-multistream.xml.bz2");
        let date = parse_dump_date(path).unwrap();
        assert_eq!(date, chrono::NaiveDate::from_ymd_opt(2025, 6, 1).unwrap());
    }
}
