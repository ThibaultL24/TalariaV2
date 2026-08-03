// crates/talaria-sources/src/lib.rs
//! Multi-source discovery & fetch for the quality pipeline (Lot A + Wikimedia Lot B).
//!
//! Extractors never write canonical events — they only produce documents/candidates
//! consumed by talaria-quality gates + assemble.

mod budgets;
mod connector;
mod kinds;
mod plan;
mod registry;
mod types;

pub mod connectors;
pub mod extractors;

pub use budgets::{BudgetExhausted, IngestBudgets};
pub use connector::{
    ConnectorError, ConnectorHealth, DiscoveryCursor, DiscoveryPage, FetchedDocument,
    SourceConnector,
};
pub use kinds::{DocumentType, DiscoveryMethod, SourceAccessMode, SourceCapabilities, SourceKind};
pub use plan::{plan_sources, PlannedSource, ResolvedSubject, SourcePlan};
pub use registry::{ConnectorRegistration, SourceRegistry};
pub use types::{DiscoveredDocument, ExternalEntityRef, SourceMetadata, TypedTimeLite};
pub use budgets::BudgetCounters;
