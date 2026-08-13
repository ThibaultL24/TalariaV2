// crates/talaria-sources/src/connectors/mod.rs
mod fixture;
mod stub;
mod theses_fr;
mod wikidata;
mod wikipedia;

pub use fixture::FixtureConnector;
pub use stub::StubConnector;
pub use theses_fr::{
    normalize_these_detail, ThesesFrConfig, ThesesFrConnector,
    CONNECTOR_VERSION as THESES_FR_VERSION,
};
pub use wikidata::{WikidataSourceConnector, WikidataSourceConnectorConfig};
pub use wikipedia::{WikipediaConnector, WikipediaConnectorConfig};

use std::sync::Arc;

use crate::kinds::{AuthorityTier, DocumentType, SourceAccessMode, SourceCapabilities, SourceKind};
use crate::registry::{ConnectorRegistration, SourceRegistry};

fn caps_wikidata() -> SourceCapabilities {
    SourceCapabilities {
        access_mode: SourceAccessMode::Api,
        authority_tier: AuthorityTier::CommunityCatalog,
        provides_text: false,
        provides_structured_statements: true,
        provides_coordinates: true,
        provides_identifiers: true,
        provides_full_text: false,
        provides_ocr: false,
        provides_iiif: false,
        provides_audiovisual: false,
        provides_authority_alignment: true,
        license_notes: "CC0".into(),
        default_confidence_structured: 0.92,
        default_confidence_ocr: 0.0,
        identifiers: vec!["qid".into()],
        document_types: vec![DocumentType::StructuredStatement],
    }
}

fn caps_wikipedia() -> SourceCapabilities {
    SourceCapabilities {
        access_mode: SourceAccessMode::Api,
        authority_tier: AuthorityTier::CommunityCatalog,
        provides_text: true,
        provides_structured_statements: false,
        provides_coordinates: false,
        provides_identifiers: true,
        provides_full_text: true,
        provides_ocr: false,
        provides_iiif: false,
        provides_audiovisual: false,
        provides_authority_alignment: true,
        license_notes: "CC BY-SA".into(),
        default_confidence_structured: 0.7,
        default_confidence_ocr: 0.0,
        identifiers: vec!["pageid".into(), "title".into(), "wikibase_item".into()],
        document_types: vec![
            DocumentType::Article,
            DocumentType::ChronologyList,
            DocumentType::Table,
        ],
    }
}

fn caps_fixture() -> SourceCapabilities {
    SourceCapabilities {
        access_mode: SourceAccessMode::File,
        authority_tier: AuthorityTier::CommunityCatalog,
        provides_text: true,
        provides_structured_statements: true,
        provides_coordinates: false,
        provides_identifiers: false,
        provides_full_text: true,
        provides_ocr: false,
        provides_iiif: false,
        provides_audiovisual: false,
        provides_authority_alignment: false,
        license_notes: "test fixture".into(),
        default_confidence_structured: 1.0,
        default_confidence_ocr: 1.0,
        identifiers: vec![],
        document_types: vec![DocumentType::Article, DocumentType::StructuredStatement],
    }
}

fn caps_theses_fr() -> SourceCapabilities {
    SourceCapabilities {
        access_mode: SourceAccessMode::Api,
        authority_tier: AuthorityTier::Institutional,
        provides_text: true,
        provides_structured_statements: true,
        provides_coordinates: false,
        provides_identifiers: true,
        provides_full_text: false,
        provides_ocr: false,
        provides_iiif: false,
        provides_audiovisual: false,
        provides_authority_alignment: true,
        license_notes: "Licence Ouverte 2.0 (metadata)".into(),
        default_confidence_structured: 0.85,
        default_confidence_ocr: 0.0,
        identifiers: vec!["nnt".into(), "ppn".into(), "doi".into(), "num_sujet".into()],
        document_types: vec![DocumentType::Thesis, DocumentType::BibliographicNotice],
    }
}

fn caps_stub(kind: &SourceKind) -> SourceCapabilities {
    let (tier, idents, types) = match kind {
        SourceKind::Hal => (
            AuthorityTier::Institutional,
            vec!["hal_id".into(), "doi".into()],
            vec![DocumentType::AcademicArticle],
        ),
        SourceKind::Crossref | SourceKind::OpenAlex => (
            AuthorityTier::ScholarlyIndex,
            vec!["doi".into()],
            vec![DocumentType::AcademicArticle],
        ),
        SourceKind::OpenEdition | SourceKind::Persee => (
            AuthorityTier::AcademicPublisher,
            vec!["doi".into()],
            vec![DocumentType::AcademicArticle],
        ),
        SourceKind::Sudoc | SourceKind::IdRef | SourceKind::Bnf => (
            AuthorityTier::Institutional,
            vec!["ppn".into(), "isbn13".into()],
            vec![
                DocumentType::BibliographicNotice,
                DocumentType::AuthorityRecord,
            ],
        ),
        SourceKind::Gallica => (
            AuthorityTier::HeritageAggregator,
            vec!["ark".into()],
            vec![
                DocumentType::BookOcr,
                DocumentType::PressOcr,
                DocumentType::Manuscript,
            ],
        ),
        SourceKind::Europeana | SourceKind::InternetArchive | SourceKind::OpenLibrary => (
            AuthorityTier::HeritageAggregator,
            vec![],
            vec![DocumentType::BibliographicNotice],
        ),
        _ => (
            AuthorityTier::CommunityCatalog,
            vec![],
            vec![DocumentType::Other("pending".into())],
        ),
    };
    SourceCapabilities {
        access_mode: SourceAccessMode::Api,
        authority_tier: tier,
        provides_text: false,
        provides_structured_statements: false,
        provides_coordinates: false,
        provides_identifiers: !idents.is_empty(),
        provides_full_text: false,
        provides_ocr: matches!(kind, SourceKind::Gallica),
        provides_iiif: matches!(kind, SourceKind::Gallica | SourceKind::Europeana),
        provides_audiovisual: false,
        provides_authority_alignment: matches!(
            kind,
            SourceKind::IdRef | SourceKind::Viaf | SourceKind::Isni | SourceKind::Bnf
        ),
        license_notes: "interface only — credentials/config required".into(),
        default_confidence_structured: 0.5,
        default_confidence_ocr: 0.4,
        identifiers: idents,
        document_types: types,
    }
}

/// Build registry with implemented connectors + stubs for future sources.
pub fn default_registry(
    fixture: Option<FixtureConnector>,
    enable_live_wikimedia: bool,
) -> anyhow::Result<SourceRegistry> {
    default_registry_with_theses(fixture, enable_live_wikimedia, None)
}

pub fn default_registry_with_theses(
    fixture: Option<FixtureConnector>,
    enable_live_wikimedia: bool,
    theses_fr: Option<ThesesFrConnector>,
) -> anyhow::Result<SourceRegistry> {
    let mut reg = SourceRegistry::new();

    if let Some(fx) = fixture {
        let kind = SourceKind::Fixture;
        reg.register(ConnectorRegistration {
            kind: kind.clone(),
            implemented: true,
            capabilities: caps_fixture(),
            connector: Some(Arc::new(fx)),
            config_notes: "deterministic local fixtures".into(),
        });
    }

    if enable_live_wikimedia {
        let wd = WikidataSourceConnector::new(WikidataSourceConnectorConfig::default())?;
        reg.register(ConnectorRegistration {
            kind: SourceKind::Wikidata,
            implemented: true,
            capabilities: caps_wikidata(),
            connector: Some(Arc::new(wd)),
            config_notes: "Wikidata MediaWiki API (wbgetentities)".into(),
        });
        let wp = WikipediaConnector::new(WikipediaConnectorConfig::default())?;
        reg.register(ConnectorRegistration {
            kind: SourceKind::Wikipedia,
            implemented: true,
            capabilities: caps_wikipedia(),
            connector: Some(Arc::new(wp)),
            config_notes: "Wikipedia MediaWiki API extracts".into(),
        });
    } else {
        for kind in [SourceKind::Wikidata, SourceKind::Wikipedia] {
            let caps = if kind == SourceKind::Wikidata {
                caps_wikidata()
            } else {
                caps_wikipedia()
            };
            reg.register(ConnectorRegistration {
                kind: kind.clone(),
                implemented: false,
                capabilities: caps,
                connector: Some(Arc::new(StubConnector::new(
                    kind,
                    "enable with --live or FixtureConnector for tests",
                ))),
                config_notes: "live disabled; use fixtures or --live".into(),
            });
        }
    }

    if let Some(tf) = theses_fr {
        reg.register(ConnectorRegistration {
            kind: SourceKind::ThesesFr,
            implemented: true,
            capabilities: caps_theses_fr(),
            connector: Some(Arc::new(tf)),
            config_notes: "theses.fr search + detail (fixture or live)".into(),
        });
    } else {
        reg.register(ConnectorRegistration {
            kind: SourceKind::ThesesFr,
            implemented: false,
            capabilities: caps_theses_fr(),
            connector: Some(Arc::new(StubConnector::new(
                SourceKind::ThesesFr,
                "pass ThesesFrConnector (fixture dir or live)",
            ))),
            config_notes: "not wired; use corpus-ingest --fixture or live".into(),
        });
    }

    for kind in [
        SourceKind::Wikisource,
        SourceKind::WikimediaCommons,
        SourceKind::Bnf,
        SourceKind::Gallica,
        SourceKind::Europeana,
        SourceKind::OpenLibrary,
        SourceKind::InternetArchive,
        SourceKind::Persee,
        SourceKind::Viaf,
        SourceKind::Isni,
        SourceKind::IdRef,
        SourceKind::Hal,
        SourceKind::Crossref,
        SourceKind::OpenAlex,
        SourceKind::OpenEdition,
        SourceKind::Sudoc,
    ] {
        let notes = match kind {
            SourceKind::Bnf => "requires BnF SPARQL/API access config",
            SourceKind::Gallica => "requires Gallica API key / IIIF endpoints",
            SourceKind::Europeana => "requires EUROPEANA_API_KEY",
            SourceKind::OpenLibrary => "public API available — Lot C",
            SourceKind::InternetArchive => "public API available — Lot C",
            SourceKind::Persee => "OAI-PMH / API — Lot C",
            SourceKind::Hal => "HAL search API — PR2+",
            SourceKind::Crossref => "Crossref REST — PR3",
            SourceKind::OpenAlex => "OpenAlex API — PR3",
            SourceKind::OpenEdition => "OpenEdition OAI-PMH — PR3",
            SourceKind::Sudoc => "Sudoc / ABES — Lot A remainder",
            SourceKind::Wikisource | SourceKind::WikimediaCommons => "Wikimedia Lot B remainder",
            _ => "alignment layer Lot C/D",
        };
        reg.register(ConnectorRegistration {
            kind: kind.clone(),
            implemented: false,
            capabilities: caps_stub(&kind),
            connector: Some(Arc::new(StubConnector::new(kind, notes))),
            config_notes: notes.into(),
        });
    }

    Ok(reg)
}

/// Re-export capability helper used by status reports.
pub use crate::registry::stub_capabilities;
