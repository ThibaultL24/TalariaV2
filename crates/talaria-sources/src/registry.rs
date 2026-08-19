// crates/talaria-sources/src/registry.rs
use std::collections::HashMap;
use std::sync::Arc;

use crate::connector::SourceConnector;
use crate::kinds::{SourceAccessMode, SourceCapabilities, SourceKind, DocumentType};

pub struct ConnectorRegistration {
    pub kind: SourceKind,
    pub implemented: bool,
    pub capabilities: SourceCapabilities,
    pub connector: Option<Arc<dyn SourceConnector>>,
    pub config_notes: String,
}

pub struct SourceRegistry {
    entries: HashMap<String, ConnectorRegistration>,
}

impl SourceRegistry {
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    pub fn register(&mut self, reg: ConnectorRegistration) {
        self.entries.insert(reg.kind.as_str().to_string(), reg);
    }

    pub fn get(&self, kind: &SourceKind) -> Option<&ConnectorRegistration> {
        self.entries.get(kind.as_str())
    }

    pub fn implemented_connectors(&self) -> Vec<Arc<dyn SourceConnector>> {
        self.entries
            .values()
            .filter_map(|e| e.connector.clone())
            .collect()
    }

    pub fn list(&self) -> Vec<&ConnectorRegistration> {
        let mut v: Vec<_> = self.entries.values().collect();
        v.sort_by_key(|e| e.kind.as_str().to_string());
        v
    }
}

impl Default for SourceRegistry {
    fn default() -> Self {
        Self::new()
    }
}

pub fn stub_capabilities(kind: SourceKind) -> SourceCapabilities {
    match kind {
        SourceKind::Wikidata => SourceCapabilities {
            access_mode: SourceAccessMode::Api,
            provides_text: false,
            provides_structured_statements: true,
            provides_coordinates: true,
            provides_identifiers: true,
            license_notes: "CC0".into(),
            default_confidence_structured: 0.92,
            default_confidence_ocr: 0.0,
            identifiers: vec!["qid".into()],
            document_types: vec![DocumentType::StructuredStatement],
        },
        SourceKind::Wikipedia => SourceCapabilities {
            access_mode: SourceAccessMode::Api,
            provides_text: true,
            provides_structured_statements: false,
            provides_coordinates: false,
            provides_identifiers: true,
            license_notes: "CC BY-SA".into(),
            default_confidence_structured: 0.7,
            default_confidence_ocr: 0.0,
            identifiers: vec!["pageid".into(), "title".into(), "wikibase_item".into()],
            document_types: vec![
                DocumentType::Article,
                DocumentType::ChronologyList,
                DocumentType::Table,
            ],
        },
        SourceKind::Fixture => SourceCapabilities {
            access_mode: SourceAccessMode::File,
            provides_text: true,
            provides_structured_statements: true,
            provides_coordinates: false,
            provides_identifiers: false,
            license_notes: "test fixture".into(),
            default_confidence_structured: 1.0,
            default_confidence_ocr: 1.0,
            identifiers: vec![],
            document_types: vec![DocumentType::Article, DocumentType::StructuredStatement],
        },
        SourceKind::OpenLibrary => SourceCapabilities {
            access_mode: SourceAccessMode::Api,
            provides_text: true,
            provides_structured_statements: true,
            provides_coordinates: false,
            provides_identifiers: true,
            license_notes: "Open Library data".into(),
            default_confidence_structured: 0.7,
            default_confidence_ocr: 0.0,
            identifiers: vec!["work_key".into()],
            document_types: vec![DocumentType::BibliographicNotice],
        },
        SourceKind::InternetArchive => SourceCapabilities {
            access_mode: SourceAccessMode::Api,
            provides_text: true,
            provides_structured_statements: false,
            provides_coordinates: false,
            provides_identifiers: true,
            license_notes: "Internet Archive metadata".into(),
            default_confidence_structured: 0.65,
            default_confidence_ocr: 0.4,
            identifiers: vec!["identifier".into()],
            document_types: vec![DocumentType::BibliographicNotice, DocumentType::BookOcr],
        },
        SourceKind::Gallica => SourceCapabilities {
            access_mode: SourceAccessMode::Api,
            provides_text: true,
            provides_structured_statements: false,
            provides_coordinates: false,
            provides_identifiers: true,
            license_notes: "BnF / Gallica".into(),
            default_confidence_structured: 0.68,
            default_confidence_ocr: 0.4,
            identifiers: vec!["ark".into()],
            document_types: vec![DocumentType::BibliographicNotice, DocumentType::PressOcr],
        },
        SourceKind::Europeana => SourceCapabilities {
            access_mode: SourceAccessMode::Api,
            provides_text: true,
            provides_structured_statements: false,
            provides_coordinates: true,
            provides_identifiers: true,
            license_notes: "Europeana item rights as provided".into(),
            default_confidence_structured: 0.66,
            default_confidence_ocr: 0.35,
            identifiers: vec!["europeana_id".into()],
            document_types: vec![DocumentType::BibliographicNotice, DocumentType::MediaCaption],
        },
        _ => SourceCapabilities {
            access_mode: SourceAccessMode::Api,
            provides_text: false,
            provides_structured_statements: false,
            provides_coordinates: false,
            provides_identifiers: true,
            license_notes: "interface only — credentials/config required".into(),
            default_confidence_structured: 0.5,
            default_confidence_ocr: 0.4,
            identifiers: vec![],
            document_types: vec![DocumentType::Other("pending".into())],
        },
    }
}
