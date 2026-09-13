// crates/talaria-sources/src/connectors/mod.rs
mod bnf;
mod catalog;
mod commons;
mod deutsche_biographie;
mod europeana;
mod fixture;
mod france_archives;
mod gallica;
mod getty_tgn;
mod hal;
mod internet_archive;
pub mod net;
mod open_library;
mod openalex;
mod persee;
mod pop_merimee;
mod stub;
mod theses_fr;
mod whg;
mod wikidata;
mod wikipedia;
mod wikisource;

pub use bnf::{BnfConfig, BnfConnector, normalize_bnf_notice};
pub use commons::{
    CommonsAsset, CommonsConnector, commonswiki_file_sitelink, file_titles_from_wikitext,
    listed_files_from_commons_sitelink, parse_mediainfo, parse_p18_filenames, parse_wiki_page_images,
};
pub use deutsche_biographie::{
    CONNECTOR_VERSION as DEUTSCHE_BIOGRAPHIE_VERSION, DeutscheBiographieConfig,
    DeutscheBiographieConnector, GndPersonRecord,
};
pub use europeana::{EuropeanaConfig, EuropeanaConnector, normalize_europeana_item};
pub use fixture::FixtureConnector;
pub use france_archives::{
    CONNECTOR_VERSION as FRANCE_ARCHIVES_VERSION, FaPersonRecord, FranceArchivesConfig,
    FranceArchivesConnector,
};
pub use gallica::GallicaConnector;
pub use getty_tgn::{
    CONNECTOR_VERSION as GETTY_TGN_VERSION, GettyTgnConfig, GettyTgnConnector, TgnPlaceMatch,
};
pub use hal::{CONNECTOR_VERSION as HAL_VERSION, HalConnector, normalize_hal_doc};
pub use internet_archive::{InternetArchiveConfig, InternetArchiveConnector, normalize_ia_item};
pub use open_library::OpenLibraryConnector;
pub use openalex::{
    CONNECTOR_VERSION as OPENALEX_VERSION, OpenAlexConfig, OpenAlexConnector,
    normalize_openalex_work, openalex_debate_query,
};
pub use persee::{CONNECTOR_VERSION as PERSEE_VERSION, PerseeConnector, normalize_persee_record};
pub use pop_merimee::{
    CONNECTOR_VERSION as POP_MERIMEE_VERSION, MerimeeRecord, PopMerimeeConfig, PopMerimeeConnector,
};
pub use stub::StubConnector;
pub use theses_fr::{
    CONNECTOR_VERSION as THESES_FR_VERSION, ThesesFrConfig, ThesesFrConnector,
    normalize_these_detail,
};
pub use whg::{CONNECTOR_VERSION as WHG_VERSION, WhgConfig, WhgConnector, WhgPlaceMatch};
pub use wikidata::{WikidataSourceConnector, WikidataSourceConnectorConfig};
pub use wikipedia::{WikipediaConnector, WikipediaConnectorConfig};
pub use wikisource::{
    WikisourceConnector, classify_genre, normalize_wikisource, parse_fetch_page,
    parse_search_titles, parse_siteinfo_namespaces, proofread_needs_review,
};

use std::sync::Arc;

use crate::kinds::{AuthorityTier, DocumentType, SourceAccessMode, SourceCapabilities, SourceKind};
use crate::registry::{ConnectorRegistration, SourceRegistry};

#[derive(Default)]
pub struct CorpusConnectors {
    pub theses_fr: Option<ThesesFrConnector>,
    pub open_alex: Option<OpenAlexConnector>,
    pub internet_archive: Option<InternetArchiveConnector>,
    pub europeana: Option<EuropeanaConnector>,
    pub bnf: Option<BnfConnector>,
    pub hal: Option<HalConnector>,
    pub persee: Option<PerseeConnector>,
}

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

fn caps_open_alex() -> SourceCapabilities {
    SourceCapabilities {
        access_mode: SourceAccessMode::Api,
        authority_tier: AuthorityTier::ScholarlyIndex,
        provides_text: true,
        provides_structured_statements: true,
        provides_coordinates: false,
        provides_identifiers: true,
        provides_full_text: false,
        provides_ocr: false,
        provides_iiif: false,
        provides_audiovisual: false,
        provides_authority_alignment: false,
        license_notes: "OpenAlex metadata (CC0); abstracts only, never PDF".into(),
        default_confidence_structured: 0.8,
        default_confidence_ocr: 0.0,
        identifiers: vec!["doi".into(), "openalex".into()],
        document_types: vec![DocumentType::AcademicArticle, DocumentType::Thesis],
    }
}

fn caps_notice(kind: SourceKind) -> SourceCapabilities {
    let (tier, idents) = match kind {
        SourceKind::Bnf => (AuthorityTier::Institutional, vec!["ark".into()]),
        SourceKind::Europeana => (AuthorityTier::HeritageAggregator, vec!["europeana".into()]),
        _ => (AuthorityTier::HeritageAggregator, vec!["ia".into()]),
    };
    SourceCapabilities {
        access_mode: SourceAccessMode::Api,
        authority_tier: tier,
        provides_text: true,
        provides_structured_statements: true,
        provides_coordinates: false,
        provides_identifiers: true,
        provides_full_text: false,
        provides_ocr: false,
        provides_iiif: matches!(kind, SourceKind::Europeana),
        provides_audiovisual: false,
        provides_authority_alignment: matches!(kind, SourceKind::Bnf),
        license_notes: "metadata notices only; never PDF/OCR/media bytes".into(),
        default_confidence_structured: 0.7,
        default_confidence_ocr: 0.0,
        identifiers: idents,
        document_types: vec![DocumentType::BibliographicNotice],
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

fn caps_hal() -> SourceCapabilities {
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
        provides_authority_alignment: false,
        license_notes: "HAL open archive (CC BY)".into(),
        default_confidence_structured: 0.80,
        default_confidence_ocr: 0.0,
        identifiers: vec!["hal_id".into(), "doi".into()],
        document_types: vec![DocumentType::AcademicArticle, DocumentType::Thesis],
    }
}

fn caps_persee() -> SourceCapabilities {
    SourceCapabilities {
        access_mode: SourceAccessMode::Api,
        authority_tier: AuthorityTier::AcademicPublisher,
        provides_text: true,
        provides_structured_statements: true,
        provides_coordinates: false,
        provides_identifiers: true,
        provides_full_text: false,
        provides_ocr: false,
        provides_iiif: false,
        provides_audiovisual: false,
        provides_authority_alignment: false,
        license_notes: "Persée open access journals".into(),
        default_confidence_structured: 0.80,
        default_confidence_ocr: 0.0,
        identifiers: vec!["doi".into()],
        document_types: vec![DocumentType::AcademicArticle],
    }
}

fn caps_grounding(kind: SourceKind) -> SourceCapabilities {
    let (tier, idents, provides_coords) = match kind {
        SourceKind::GettyTgn => (
            AuthorityTier::Institutional,
            vec!["tgn".into()],
            false,
        ),
        SourceKind::Whg => (
            AuthorityTier::Institutional,
            vec!["whg".into()],
            true,
        ),
        SourceKind::PopMerimee => (
            AuthorityTier::Institutional,
            vec!["merimee_ref".into()],
            true,
        ),
        _ => (AuthorityTier::CommunityCatalog, vec![], false),
    };
    SourceCapabilities {
        access_mode: SourceAccessMode::Sparql,
        authority_tier: tier,
        provides_text: false,
        provides_structured_statements: true,
        provides_coordinates: provides_coords,
        provides_identifiers: true,
        provides_full_text: false,
        provides_ocr: false,
        provides_iiif: false,
        provides_audiovisual: false,
        provides_authority_alignment: true,
        license_notes: "grounding service — place-identity resolution".into(),
        default_confidence_structured: 0.85,
        default_confidence_ocr: 0.0,
        identifiers: idents,
        document_types: vec![DocumentType::AuthorityRecord],
    }
}

fn caps_life_facts(kind: SourceKind) -> SourceCapabilities {
    let (tier, idents) = match kind {
        SourceKind::FranceArchives => (
            AuthorityTier::Institutional,
            vec!["fa_id".into()],
        ),
        SourceKind::DeutscheBiographie => (
            AuthorityTier::Institutional,
            vec!["gnd".into()],
        ),
        _ => (AuthorityTier::CommunityCatalog, vec![]),
    };
    SourceCapabilities {
        access_mode: SourceAccessMode::Api,
        authority_tier: tier,
        provides_text: true,
        provides_structured_statements: true,
        provides_coordinates: false,
        provides_identifiers: true,
        provides_full_text: false,
        provides_ocr: false,
        provides_iiif: false,
        provides_audiovisual: false,
        provides_authority_alignment: true,
        license_notes: "structured life facts (birth/death/office)".into(),
        default_confidence_structured: 0.85,
        default_confidence_ocr: 0.0,
        identifiers: idents,
        document_types: vec![DocumentType::AuthorityRecord, DocumentType::StructuredStatement],
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

fn register_stub(reg: &mut SourceRegistry, kind: SourceKind, notes: &str) {
    reg.register(ConnectorRegistration {
        kind: kind.clone(),
        implemented: false,
        capabilities: stub_capabilities(kind.clone()),
        connector: Some(Arc::new(StubConnector::new(kind, notes))),
        config_notes: notes.into(),
    });
}

/// Build registry with implemented connectors + stubs for future sources.
pub fn default_registry(
    fixture: Option<FixtureConnector>,
    enable_live_wikimedia: bool,
) -> anyhow::Result<SourceRegistry> {
    default_registry_corpus(fixture, enable_live_wikimedia, None, None)
}

pub fn default_registry_with_theses(
    fixture: Option<FixtureConnector>,
    enable_live_wikimedia: bool,
    theses_fr: Option<ThesesFrConnector>,
) -> anyhow::Result<SourceRegistry> {
    default_registry_corpus(fixture, enable_live_wikimedia, theses_fr, None)
}

pub fn default_registry_corpus(
    fixture: Option<FixtureConnector>,
    enable_live_wikimedia: bool,
    theses_fr: Option<ThesesFrConnector>,
    open_alex: Option<OpenAlexConnector>,
) -> anyhow::Result<SourceRegistry> {
    default_registry_with_corpus(
        fixture,
        enable_live_wikimedia,
        CorpusConnectors {
            theses_fr,
            open_alex,
            ..CorpusConnectors::default()
        },
    )
}

pub fn default_registry_with_corpus(
    fixture: Option<FixtureConnector>,
    enable_live_wikimedia: bool,
    corpus: CorpusConnectors,
) -> anyhow::Result<SourceRegistry> {
    let CorpusConnectors {
        theses_fr,
        open_alex,
        internet_archive,
        europeana,
        bnf,
        hal,
        persee,
    } = corpus;
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
        let ol = OpenLibraryConnector::new()?;
        reg.register(ConnectorRegistration {
            kind: SourceKind::OpenLibrary,
            implemented: true,
            capabilities: stub_capabilities(SourceKind::OpenLibrary),
            connector: Some(Arc::new(ol)),
            config_notes: "Open Library search.json (public)".into(),
        });
        let ia = InternetArchiveConnector::new(InternetArchiveConfig::default())?;
        reg.register(ConnectorRegistration {
            kind: SourceKind::InternetArchive,
            implemented: true,
            capabilities: stub_capabilities(SourceKind::InternetArchive),
            connector: Some(Arc::new(ia)),
            config_notes: "Internet Archive advanced search (public)".into(),
        });
        let gallica = GallicaConnector::new()?;
        reg.register(ConnectorRegistration {
            kind: SourceKind::Gallica,
            implemented: true,
            capabilities: stub_capabilities(SourceKind::Gallica),
            connector: Some(Arc::new(gallica)),
            config_notes: "Gallica SRU (public)".into(),
        });
        let wikisource = WikisourceConnector::new()?;
        reg.register(ConnectorRegistration {
            kind: SourceKind::Wikisource,
            implemented: true,
            capabilities: caps_stub(&SourceKind::Wikisource),
            connector: Some(Arc::new(wikisource)),
            config_notes: "Wikisource FR Action API (public)".into(),
        });
        let commons = CommonsConnector::new()?;
        reg.register(ConnectorRegistration {
            kind: SourceKind::WikimediaCommons,
            implemented: true,
            capabilities: caps_stub(&SourceKind::WikimediaCommons),
            connector: Some(Arc::new(commons)),
            config_notes: "Wikimedia Commons Action API (public)".into(),
        });
        let hal_conn = HalConnector::new()?;
        reg.register(ConnectorRegistration {
            kind: SourceKind::Hal,
            implemented: true,
            capabilities: caps_hal(),
            connector: Some(Arc::new(hal_conn)),
            config_notes: "HAL open archive Solr API (public)".into(),
        });
        let persee_conn = PerseeConnector::new()?;
        reg.register(ConnectorRegistration {
            kind: SourceKind::Persee,
            implemented: true,
            capabilities: caps_persee(),
            connector: Some(Arc::new(persee_conn)),
            config_notes: "Persée OAI-PMH Dublin Core (public)".into(),
        });
        match ThesesFrConnector::new(ThesesFrConfig::default()) {
            Ok(conn) => {
                reg.register(ConnectorRegistration {
                    kind: SourceKind::ThesesFr,
                    implemented: true,
                    capabilities: caps_theses_fr(),
                    connector: Some(Arc::new(conn)),
                    config_notes: "theses.fr search (public)".into(),
                });
            }
            Err(_) => register_stub(
                &mut reg,
                SourceKind::ThesesFr,
                "theses.fr connector init failed",
            ),
        }
        let mut oa_cfg = OpenAlexConfig::default();
        oa_cfg.api_key = std::env::var("OPENALEX_API_KEY")
            .ok()
            .filter(|s| !s.trim().is_empty());
        oa_cfg.mailto = std::env::var("OPENALEX_MAILTO")
            .ok()
            .filter(|s| !s.trim().is_empty());
        match OpenAlexConnector::new(oa_cfg) {
            Ok(conn) => {
                reg.register(ConnectorRegistration {
                    kind: SourceKind::OpenAlex,
                    implemented: true,
                    capabilities: caps_open_alex(),
                    connector: Some(Arc::new(conn)),
                    config_notes: "OpenAlex works API (public)".into(),
                });
            }
            Err(_) => register_stub(
                &mut reg,
                SourceKind::OpenAlex,
                "OpenAlex connector init failed",
            ),
        }
        match BnfConnector::new(BnfConfig::default()) {
            Ok(conn) => {
                reg.register(ConnectorRegistration {
                    kind: SourceKind::Bnf,
                    implemented: true,
                    capabilities: caps_notice(SourceKind::Bnf),
                    connector: Some(Arc::new(conn)),
                    config_notes: "BnF catalogue (public SPARQL/SRU)".into(),
                });
            }
            Err(_) => register_stub(&mut reg, SourceKind::Bnf, "BnF connector init failed"),
        }
        let eu_config = EuropeanaConfig {
            api_key: std::env::var("EUROPEANA_API_KEY").ok(),
            ..EuropeanaConfig::default()
        };
        match EuropeanaConnector::new(eu_config) {
            Ok(eu) => {
                reg.register(ConnectorRegistration {
                    kind: SourceKind::Europeana,
                    implemented: true,
                    capabilities: stub_capabilities(SourceKind::Europeana),
                    connector: Some(Arc::new(eu)),
                    config_notes: "Europeana Search API v2 (EUROPEANA_API_KEY)".into(),
                });
            }
            Err(_) => register_stub(
                &mut reg,
                SourceKind::Europeana,
                "requires EUROPEANA_API_KEY",
            ),
        }
    } else {
        for kind in [SourceKind::Wikidata, SourceKind::Wikipedia] {
            register_stub(
                &mut reg,
                kind,
                "enable with --live or FixtureConnector for tests",
            );
        }
        register_stub(&mut reg, SourceKind::OpenLibrary, "enable with --live");
        register_stub(&mut reg, SourceKind::InternetArchive, "enable with --live");
        register_stub(&mut reg, SourceKind::Gallica, "enable with --live");
        register_stub(&mut reg, SourceKind::Wikisource, "enable with --live");
        register_stub(&mut reg, SourceKind::WikimediaCommons, "enable with --live");
        register_stub(&mut reg, SourceKind::Hal, "enable with --live");
        register_stub(&mut reg, SourceKind::Persee, "enable with --live");
        register_stub(&mut reg, SourceKind::ThesesFr, "enable with --live");
        register_stub(&mut reg, SourceKind::OpenAlex, "enable with --live");
        register_stub(&mut reg, SourceKind::Bnf, "enable with --live");
        register_stub(
            &mut reg,
            SourceKind::Europeana,
            "requires --live and EUROPEANA_API_KEY",
        );
    }

    // Remaining Lot C/D — interfaces only until fetch/parse/extract exist.
    for kind in [
        SourceKind::Viaf,
        SourceKind::Isni,
        SourceKind::IdRef,
        SourceKind::Crossref,
        SourceKind::OpenEdition,
        SourceKind::Sudoc,
    ] {
        register_stub(&mut reg, kind, "alignment layer — not yet wired");
    }

    // Vague B: grounding / structured life facts connectors
    if enable_live_wikimedia {
        // Getty TGN — place-identity grounding (SPARQL)
        match GettyTgnConnector::new(GettyTgnConfig::default()) {
            Ok(conn) => {
                reg.register(ConnectorRegistration {
                    kind: SourceKind::GettyTgn,
                    implemented: true,
                    capabilities: caps_grounding(SourceKind::GettyTgn),
                    connector: Some(Arc::new(conn)),
                    config_notes: "Getty TGN SPARQL (public, place-identity grounding)".into(),
                });
            }
            Err(_) => register_stub(
                &mut reg,
                SourceKind::GettyTgn,
                "Getty TGN connector init failed",
            ),
        }

        // WHG — place-identity with historical attestations (requires WHG_API_TOKEN)
        let whg_config = WhgConfig::from_env();
        if whg_config.api_token.is_some() {
            match WhgConnector::new(whg_config) {
                Ok(conn) => {
                    reg.register(ConnectorRegistration {
                        kind: SourceKind::Whg,
                        implemented: true,
                        capabilities: caps_grounding(SourceKind::Whg),
                        connector: Some(Arc::new(conn)),
                        config_notes: "WHG API (requires WHG_API_TOKEN)".into(),
                    });
                }
                Err(_) => register_stub(&mut reg, SourceKind::Whg, "WHG connector init failed"),
            }
        } else {
            register_stub(&mut reg, SourceKind::Whg, "requires WHG_API_TOKEN");
        }

        // FranceArchives — structured life facts (SPARQL)
        match FranceArchivesConnector::new(FranceArchivesConfig::default()) {
            Ok(conn) => {
                reg.register(ConnectorRegistration {
                    kind: SourceKind::FranceArchives,
                    implemented: true,
                    capabilities: caps_life_facts(SourceKind::FranceArchives),
                    connector: Some(Arc::new(conn)),
                    config_notes: "FranceArchives SPARQL (public, life facts)".into(),
                });
            }
            Err(_) => register_stub(
                &mut reg,
                SourceKind::FranceArchives,
                "FranceArchives connector init failed",
            ),
        }

        // Deutsche Biographie + GND — structured life facts (lobid.org)
        match DeutscheBiographieConnector::new(DeutscheBiographieConfig::default()) {
            Ok(conn) => {
                reg.register(ConnectorRegistration {
                    kind: SourceKind::DeutscheBiographie,
                    implemented: true,
                    capabilities: caps_life_facts(SourceKind::DeutscheBiographie),
                    connector: Some(Arc::new(conn)),
                    config_notes: "GND via lobid.org (public, life facts)".into(),
                });
            }
            Err(_) => register_stub(
                &mut reg,
                SourceKind::DeutscheBiographie,
                "GND connector init failed",
            ),
        }

        // POP/Mérimée — place entity with WGS84 coords
        match PopMerimeeConnector::new(PopMerimeeConfig::default()) {
            Ok(conn) => {
                reg.register(ConnectorRegistration {
                    kind: SourceKind::PopMerimee,
                    implemented: true,
                    capabilities: caps_grounding(SourceKind::PopMerimee),
                    connector: Some(Arc::new(conn)),
                    config_notes: "POP Mérimée API (public, place entity + WGS84)".into(),
                });
            }
            Err(_) => register_stub(
                &mut reg,
                SourceKind::PopMerimee,
                "POP Mérimée connector init failed",
            ),
        }
    } else {
        register_stub(&mut reg, SourceKind::GettyTgn, "enable with --live");
        register_stub(&mut reg, SourceKind::Whg, "requires --live and WHG_API_TOKEN");
        register_stub(&mut reg, SourceKind::FranceArchives, "enable with --live");
        register_stub(&mut reg, SourceKind::DeutscheBiographie, "enable with --live");
        register_stub(&mut reg, SourceKind::PopMerimee, "enable with --live");
    }

    // Register corpus connectors supplied by the caller (override stubs when present).
    if let Some(ia) = internet_archive {
        reg.register(ConnectorRegistration {
            kind: SourceKind::InternetArchive,
            implemented: true,
            capabilities: stub_capabilities(SourceKind::InternetArchive),
            connector: Some(Arc::new(ia)),
            config_notes: "Internet Archive (corpus fixture)".into(),
        });
    }
    if let Some(eu) = europeana {
        reg.register(ConnectorRegistration {
            kind: SourceKind::Europeana,
            implemented: true,
            capabilities: stub_capabilities(SourceKind::Europeana),
            connector: Some(Arc::new(eu)),
            config_notes: "Europeana (corpus fixture)".into(),
        });
    }
    if let Some(bnf) = bnf {
        reg.register(ConnectorRegistration {
            kind: SourceKind::Bnf,
            implemented: true,
            capabilities: stub_capabilities(SourceKind::Bnf),
            connector: Some(Arc::new(bnf)),
            config_notes: "BnF (corpus fixture)".into(),
        });
    }
    if let Some(theses) = theses_fr {
        reg.register(ConnectorRegistration {
            kind: SourceKind::ThesesFr,
            implemented: true,
            capabilities: stub_capabilities(SourceKind::ThesesFr),
            connector: Some(Arc::new(theses)),
            config_notes: "theses.fr (corpus fixture)".into(),
        });
    }
    if let Some(oa) = open_alex {
        reg.register(ConnectorRegistration {
            kind: SourceKind::OpenAlex,
            implemented: true,
            capabilities: stub_capabilities(SourceKind::OpenAlex),
            connector: Some(Arc::new(oa)),
            config_notes: "OpenAlex (corpus fixture)".into(),
        });
    }
    if let Some(h) = hal {
        reg.register(ConnectorRegistration {
            kind: SourceKind::Hal,
            implemented: true,
            capabilities: caps_hal(),
            connector: Some(Arc::new(h)),
            config_notes: "HAL (corpus override)".into(),
        });
    }
    if let Some(p) = persee {
        reg.register(ConnectorRegistration {
            kind: SourceKind::Persee,
            implemented: true,
            capabilities: caps_persee(),
            connector: Some(Arc::new(p)),
            config_notes: "Persée (corpus override)".into(),
        });
    }

    Ok(reg)
}

/// Re-export capability helper used by status reports.
pub use crate::registry::stub_capabilities;
