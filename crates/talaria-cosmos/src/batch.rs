// crates/talaria-cosmos/src/batch.rs
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::process::Command;
use talaria_core::AppConfig;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchInputItem {
    pub id: String,
    pub text: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BatchOutputItem {
    pub id: String,
    pub tuples: Vec<ExtractedTuple>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ExtractedTuple {
    pub person: String,
    pub time: String,
    pub place: String,
    #[serde(default)]
    pub verb: Option<String>,
}

pub fn run_cosmos_batch(
    config: &AppConfig,
    batch_script: &Path,
    items: &[BatchInputItem],
) -> anyhow::Result<Vec<BatchOutputItem>> {
    if items.is_empty() {
        return Ok(Vec::new());
    }

    let dir = tempfile::tempdir()?;
    let input_path = dir.path().join("cosmos_in.json");
    let output_path = dir.path().join("cosmos_out.json");

    std::fs::write(&input_path, serde_json::to_string(items)?)?;

    let status = Command::new(&config.cosmos_python)
        .arg(batch_script)
        .arg("--input")
        .arg(&input_path)
        .arg("--output")
        .arg(&output_path)
        .status()?;

    if !status.success() {
        anyhow::bail!("COSMOS batch script failed with status {status}");
    }

    let raw = std::fs::read_to_string(&output_path)?;
    Ok(serde_json::from_str(&raw)?)
}
