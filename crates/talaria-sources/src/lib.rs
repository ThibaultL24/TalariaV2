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
mod person_profile;
mod place_quality;
mod places;
mod plan;
mod registry;
mod seeds;
mod types;
mod wiki_fragments;

pub mod connectors;
pub mod extractors;
pub mod historiography;
pub mod wdqs;

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
    is_historiography_section, scan_bibliographic, scan_passage, DebateType, EventHint,
    EvidenceLayer, HistoriographyHit,
};
pub use identifiers::{normalize_identifier, normalize_person_name};
pub use kinds::{
    AcademicStatus, AccessLevel, AuthorityTier, ContributionRole, DiscoveryMethod, DocumentType,
    IdentifierScheme, SourceAccessMode, SourceCapabilities, SourceKind,
};
pub use matching::{
    match_resolved_subject_to_document, match_subject_to_document, subject_match_aliases,
};
pub use person_profile::{
    catalog_search_buckets, catalog_search_query, filter_wiki_titles_for_classes,
    filter_wiki_titles_for_profile, has_military_signal, infer_person_class, infer_person_classes,
    keep_military_typed_event, profile_for, rank_wikipedia_title, rank_wikipedia_title_for_classes,
    IngestProfile, PersonClass,
};
pub use place_quality::is_plausible_place_label;
pub use places::{
    place_hint_from_title, place_query_variants, resolve_place_offline, PlaceResolution,
};
pub use plan::{plan_sources, PlannedSource, ResolvedSubject, SourcePlan};
pub use registry::{ConnectorRegistration, SourceRegistry};
pub use seeds::{
    dated_wikilink_titles, first_year_in_window, is_followable_map_title, is_high_value_link_title,
    is_life_trace_link_title, is_noise_wiki_title, lifespan_year_window, load_seed_titles,
    merge_seed_titles, merge_seed_titles_for, subject_surname,
};
pub use types::{DiscoveredDocument, ExternalEntityRef, SourceMetadata, TypedTimeLite};
pub use wiki_fragments::{fragment_inserts, fragment_inserts_with_titles};

pub use connectors::{
    normalize_bnf_notice, normalize_europeana_item, normalize_ia_item, normalize_openalex_work,
    normalize_these_detail, BnfConfig, BnfConnector, CorpusConnectors, EuropeanaConfig,
    EuropeanaConnector, InternetArchiveConfig, InternetArchiveConnector, OpenAlexConfig,
    OpenAlexConnector, ThesesFrConfig, ThesesFrConnector,
};
