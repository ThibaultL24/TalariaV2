// crates/talaria-sources/src/lib.rs
//! Multi-source discovery & fetch for the quality pipeline.
#![allow(clippy::too_many_arguments)]
#![allow(clippy::useless_format)]
#![allow(clippy::if_same_then_else)]
#![allow(clippy::items_after_test_module)]
#![allow(dead_code)]

mod budgets;
mod connector;
mod corpus;
mod density;
mod identifiers;
mod kinds;
mod matching;
mod place_quality;
mod places;
mod plan;
mod registry;
mod seeds;
mod types;

pub mod connectors;
pub mod extractors;
pub mod historiography;

pub use budgets::{BudgetCounters, BudgetExhausted, IngestBudgets};
pub use connector::{
    ConnectorError, ConnectorHealth, DiscoveryCursor, DiscoveryPage, FetchedDocument,
    SourceConnector,
};
pub use corpus::{
    EntityDocumentMatch, MatchComponent, NormalizedContribution, NormalizedCorpusDocument,
    NormalizedIdentifier, NormalizedSubject, SUBJECT_MATCH_V1,
};
pub use density::{DensityProgress, DensityTargets};
pub use historiography::{
    is_historiography_section, scan_bibliographic, scan_passage, DebateType, EvidenceLayer,
    EventHint, HistoriographyHit,
};
pub use identifiers::{normalize_identifier, normalize_person_name};
pub use kinds::{
    AcademicStatus, AccessLevel, AuthorityTier, ContributionRole, DiscoveryMethod, DocumentType,
    IdentifierScheme, SourceAccessMode, SourceCapabilities, SourceKind,
};
pub use matching::match_subject_to_document;
pub use place_quality::is_plausible_place_label;
pub use places::{place_hint_from_title, resolve_place_offline, PlaceResolution};
pub use plan::{plan_sources, PlannedSource, ResolvedSubject, SourcePlan};
pub use registry::{ConnectorRegistration, SourceRegistry};
pub use seeds::{is_high_value_link_title, load_seed_titles};
pub use types::{DiscoveredDocument, ExternalEntityRef, SourceMetadata, TypedTimeLite};

pub use connectors::{normalize_these_detail, ThesesFrConfig, ThesesFrConnector};
