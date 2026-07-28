// crates/talaria-dump/src/layout.rs
use std::path::Path;
use talaria_core::AppConfig;

pub fn ensure_data_dirs(config: &AppConfig) -> anyhow::Result<()> {
    for dir in [
        config.data_root.as_path(),
        &config.dumps_dir(),
        &config.parquet_dir(),
        &config.pages_dir(),
    ] {
        std::fs::create_dir_all(dir)?;
    }
    Ok(())
}

pub fn dump_index_path(dump_xml_bz2: &Path) -> std::path::PathBuf {
    let name = dump_xml_bz2
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("dump.xml.bz2");
    dump_xml_bz2
        .with_file_name(format!("{name}.index.jsonl"))
}
