// crates/talaria-sources/src/budgets.rs
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IngestBudgets {
    pub max_depth: u32,
    pub max_documents_per_source: u32,
    pub max_linked_entities: u32,
    pub max_pages_per_collection: u32,
    pub max_external_calls: u32,
    pub max_download_bytes: u64,
    pub min_relevance: f32,
    pub time_budget_secs: u64,
}

impl Default for IngestBudgets {
    fn default() -> Self {
        Self {
            max_depth: 2,
            max_documents_per_source: 40,
            max_linked_entities: 80,
            max_pages_per_collection: 20,
            max_external_calls: 200,
            max_download_bytes: 50 * 1024 * 1024,
            min_relevance: 0.35,
            time_budget_secs: 300,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct BudgetCounters {
    pub external_calls: u32,
    pub download_bytes: u64,
    pub documents_per_source: std::collections::HashMap<String, u32>,
    pub linked_entities: u32,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum BudgetExhausted {
    #[error("max_documents_per_source exhausted for {0}")]
    DocumentsPerSource(String),
    #[error("max_external_calls exhausted")]
    ExternalCalls,
    #[error("max_download_bytes exhausted")]
    DownloadBytes,
    #[error("max_linked_entities exhausted")]
    LinkedEntities,
}

impl BudgetCounters {
    pub fn record_call(&mut self, budgets: &IngestBudgets) -> Result<(), BudgetExhausted> {
        self.external_calls += 1;
        if self.external_calls > budgets.max_external_calls {
            return Err(BudgetExhausted::ExternalCalls);
        }
        Ok(())
    }

    pub fn record_document(
        &mut self,
        source: &str,
        budgets: &IngestBudgets,
        bytes: u64,
    ) -> Result<(), BudgetExhausted> {
        let n = self.documents_per_source.entry(source.to_string()).or_insert(0);
        *n += 1;
        if *n > budgets.max_documents_per_source {
            return Err(BudgetExhausted::DocumentsPerSource(source.to_string()));
        }
        self.download_bytes += bytes;
        if self.download_bytes > budgets.max_download_bytes {
            return Err(BudgetExhausted::DownloadBytes);
        }
        Ok(())
    }
}
