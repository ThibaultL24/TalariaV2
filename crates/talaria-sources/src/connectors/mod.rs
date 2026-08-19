// crates/talaria-sources/src/connectors/mod.rs
mod catalog;
mod europeana;
mod fixture;
mod gallica;
mod internet_archive;
mod open_library;
mod stub;
mod wikidata;
mod wikipedia;

pub use europeana::EuropeanaConnector;
pub use fixture::FixtureConnector;
pub use gallica::GallicaConnector;
pub use internet_archive::InternetArchiveConnector;
pub use open_library::OpenLibraryConnector;
pub use stub::StubConnector;
pub use wikidata::{WikidataSourceConnector, WikidataSourceConnectorConfig};
pub use wikipedia::{WikipediaConnector, WikipediaConnectorConfig};

use std::sync::Arc;

use crate::kinds::SourceKind;
use crate::registry::{stub_capabilities, ConnectorRegistration, SourceRegistry};

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
    let mut reg = SourceRegistry::new();

    if let Some(fx) = fixture {
        let kind = SourceKind::Fixture;
        reg.register(ConnectorRegistration {
            kind: kind.clone(),
            implemented: true,
            capabilities: stub_capabilities(kind),
            connector: Some(Arc::new(fx)),
            config_notes: "deterministic local fixtures".into(),
        });
    }

    if enable_live_wikimedia {
        let wd = WikidataSourceConnector::new(WikidataSourceConnectorConfig::default())?;
        reg.register(ConnectorRegistration {
            kind: SourceKind::Wikidata,
            implemented: true,
            capabilities: stub_capabilities(SourceKind::Wikidata),
            connector: Some(Arc::new(wd)),
            config_notes: "Wikidata MediaWiki API (wbgetentities)".into(),
        });
        let wp = WikipediaConnector::new(WikipediaConnectorConfig::default())?;
        reg.register(ConnectorRegistration {
            kind: SourceKind::Wikipedia,
            implemented: true,
            capabilities: stub_capabilities(SourceKind::Wikipedia),
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
        let ia = InternetArchiveConnector::new()?;
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
        match EuropeanaConnector::from_env() {
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
        register_stub(
            &mut reg,
            SourceKind::Europeana,
            "requires --live and EUROPEANA_API_KEY",
        );
    }

    // Remaining Lot C/D — interfaces only until fetch/parse/extract exist.
    for kind in [
        SourceKind::Wikisource,
        SourceKind::WikimediaCommons,
        SourceKind::Bnf,
        SourceKind::Persee,
        SourceKind::Viaf,
        SourceKind::Isni,
        SourceKind::IdRef,
    ] {
        let notes = match kind {
            SourceKind::Bnf => "requires BnF SPARQL/API access config",
            SourceKind::Persee => "OAI-PMH / API — not yet wired",
            SourceKind::Wikisource | SourceKind::WikimediaCommons => "Wikimedia remainder",
            _ => "alignment layer — not yet wired",
        };
        register_stub(&mut reg, kind, notes);
    }

    Ok(reg)
}
