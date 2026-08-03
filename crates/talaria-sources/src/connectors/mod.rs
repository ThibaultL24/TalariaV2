// crates/talaria-sources/src/connectors/mod.rs
mod fixture;
mod stub;
mod wikidata;
mod wikipedia;

pub use fixture::FixtureConnector;
pub use stub::StubConnector;
pub use wikidata::{WikidataSourceConnector, WikidataSourceConnectorConfig};
pub use wikipedia::{WikipediaConnector, WikipediaConnectorConfig};

use std::sync::Arc;

use crate::kinds::SourceKind;
use crate::registry::{stub_capabilities, ConnectorRegistration, SourceRegistry};

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
    } else {
        for kind in [SourceKind::Wikidata, SourceKind::Wikipedia] {
            reg.register(ConnectorRegistration {
                kind: kind.clone(),
                implemented: false,
                capabilities: stub_capabilities(kind.clone()),
                connector: Some(Arc::new(StubConnector::new(
                    kind,
                    "enable with --live or FixtureConnector for tests",
                ))),
                config_notes: "live disabled; use fixtures or --live".into(),
            });
        }
    }

    // Lot C stubs — interfaces only, not claimed as integrated.
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
    ] {
        let notes = match kind {
            SourceKind::Bnf => "requires BnF SPARQL/API access config",
            SourceKind::Gallica => "requires Gallica API key / IIIF endpoints",
            SourceKind::Europeana => "requires EUROPEANA_API_KEY",
            SourceKind::OpenLibrary => "public API available — Lot C",
            SourceKind::InternetArchive => "public API available — Lot C",
            SourceKind::Persee => "OAI-PMH / API — Lot C",
            SourceKind::Wikisource | SourceKind::WikimediaCommons => "Wikimedia Lot B remainder",
            _ => "alignment layer Lot C/D",
        };
        reg.register(ConnectorRegistration {
            kind: kind.clone(),
            implemented: false,
            capabilities: stub_capabilities(kind.clone()),
            connector: Some(Arc::new(StubConnector::new(kind, notes))),
            config_notes: notes.into(),
        });
    }

    Ok(reg)
}
