// crates/talaria-core/src/config.rs
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub database_url: String,
    pub data_root: PathBuf,
    pub bind_addr: String,
    pub wiki_lang: String,
    pub cosmos_python: String,
    pub cosmos_script: PathBuf,
    pub cosmos_batch_script: PathBuf,
    /// When true, skip live MediaWiki/Wikidata HTTP in request paths (dossier, etc.).
    pub offline_only: bool,
}

impl AppConfig {
    pub fn from_env() -> anyhow::Result<Self> {
        Ok(Self {
            database_url: std::env::var("DATABASE_URL")
                .map_err(|_| anyhow::anyhow!("DATABASE_URL is required"))?,
            data_root: PathBuf::from(
                std::env::var("TALARIA_DATA_ROOT").unwrap_or_else(|_| "./data".into()),
            ),
            bind_addr: std::env::var("TALARIA_BIND").unwrap_or_else(|_| "0.0.0.0:8080".into()),
            wiki_lang: std::env::var("WIKI_LANG").unwrap_or_else(|_| "en".into()),
            cosmos_python: std::env::var("COSMOS_PYTHON").unwrap_or_else(|_| "python3".into()),
            cosmos_script: PathBuf::from(
                std::env::var("COSMOS_SCRIPT")
                    .unwrap_or_else(|_| "sidecar/cosmos/preprocessing/tuple_extraction.py".into()),
            ),
            cosmos_batch_script: PathBuf::from(
                std::env::var("COSMOS_BATCH_SCRIPT")
                    .unwrap_or_else(|_| "sidecar/cosmos_batch.py".into()),
            ),
            offline_only: env_truthy("TALARIA_OFFLINE_ONLY"),
        })
    }

    pub fn dumps_dir(&self) -> PathBuf {
        self.data_root.join("dumps")
    }

    pub fn parquet_dir(&self) -> PathBuf {
        self.data_root.join("parquet")
    }

    pub fn pages_dir(&self) -> PathBuf {
        self.data_root.join("pages")
    }

    pub fn wikidata_dir(&self) -> PathBuf {
        self.data_root.join("wikidata")
    }

    pub fn page_file(&self, wiki_lang: &str, page_id: u64) -> PathBuf {
        self.pages_dir()
            .join(wiki_lang)
            .join(format!("{page_id}.wiki"))
    }
}

fn env_truthy(key: &str) -> bool {
    matches!(
        std::env::var(key).ok().as_deref(),
        Some("1" | "true" | "TRUE" | "yes" | "YES" | "on" | "ON")
    )
}
