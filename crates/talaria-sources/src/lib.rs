// crates/talaria-sources/src/lib.rs
//! Multi-source discovery & fetch for the quality pipeline.

mod budgets;
mod connector;
mod density;
mod kinds;
mod place_quality;
mod places;
mod plan;
mod registry;
mod seeds;
mod types;

pub mod connectors;
pub mod extractors;

pub use budgets::{BudgetCounters, BudgetExhausted, IngestBudgets};
pub use connector::{
    ConnectorError, ConnectorHealth, DiscoveryCursor, DiscoveryPage, FetchedDocument,
    SourceConnector,
};
pub use density::{DensityProgress, DensityTargets};
pub use kinds::{DocumentType, DiscoveryMethod, SourceAccessMode, SourceCapabilities, SourceKind};
pub use places::{place_hint_from_title, resolve_place_offline, PlaceResolution};
pub use place_quality::is_plausible_place_label;
pub use plan::{plan_sources, PlannedSource, ResolvedSubject, SourcePlan};
pub use registry::{ConnectorRegistration, SourceRegistry};
pub use seeds::{is_high_value_link_title, load_seed_titles};
pub use types::{DiscoveredDocument, ExternalEntityRef, SourceMetadata, TypedTimeLite};
