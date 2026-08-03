// crates/talaria-sources/src/connectors/fixture.rs
use std::collections::HashMap;

use async_trait::async_trait;

use crate::connector::{
    ConnectorError, ConnectorHealth, DiscoveryCursor, DiscoveryPage, FetchedDocument,
    SourceConnector,
};
use crate::kinds::{DiscoveryMethod, DocumentType, SourceKind};
use crate::plan::ResolvedSubject;
use crate::types::{DiscoveredDocument, ExternalEntityRef, SourceMetadata};

#[derive(Debug, Clone)]
pub struct FixtureDoc {
    pub external_id: String,
    pub title: String,
    pub language: String,
    pub document_type: DocumentType,
    pub text: String,
    pub relevance: f32,
    pub revision_id: String,
}

/// Deterministic connector for tests — no network.
pub struct FixtureConnector {
    docs: Vec<FixtureDoc>,
    by_id: HashMap<String, FixtureDoc>,
}

impl FixtureConnector {
    pub fn new(docs: Vec<FixtureDoc>) -> Self {
        let by_id = docs
            .iter()
            .map(|d| (d.external_id.clone(), d.clone()))
            .collect();
        Self { docs, by_id }
    }

    /// Rich multi-doc Napoleon-style corpus (generic subject label substituted).
    pub fn dense_biography_pack(subject_label: &str) -> Self {
        let s = subject_label;
        let docs = vec![
            FixtureDoc {
                external_id: format!("fixture:{s}:bio"),
                title: format!("{s}"),
                language: "en".into(),
                document_type: DocumentType::Article,
                revision_id: "r1".into(),
                relevance: 0.95,
                text: format!(
                    "{s} was born in Ajaccio in 1769.\n\
                     He studied at Brienne in 1779.\n\
                     He married Joséphine in 1796 in Paris.\n\
                     He fought at Austerlitz in 1805.\n\
                     He fought at Jena in 1806.\n\
                     He signed a treaty at Tilsit in 1807.\n\
                     He fought at Wagram in 1809.\n\
                     He invaded Russia and fought at Borodino in 1812.\n\
                     He fought at Leipzig in 1813.\n\
                     He was exiled to Elba in 1814.\n\
                     He fought at Waterloo in 1815.\n\
                     He was exiled to Saint Helena in 1815.\n\
                     He died in Saint Helena in 1821.\n\
                     In 1774 his father died; he fought in Leipzig.\n\
                     He died in Waterloo in 1798.\n"
                ),
            },
            FixtureDoc {
                external_id: format!("fixture:{s}:wikidata"),
                title: format!("{s} (structured)"),
                language: "en".into(),
                document_type: DocumentType::StructuredStatement,
                revision_id: "wd1".into(),
                relevance: 0.98,
                text: format!(
                    "STATEMENT\tbirth\tborn_in\t1769\tAjaccio\n\
                     STATEMENT\tdeath\tdied_in\t1821\tSaint Helena\n\
                     STATEMENT\tmarriage\tmarried\t1796\tParis\n\
                     STATEMENT\tbattle\tfought_at\t1805\tAusterlitz\n\
                     STATEMENT\tbattle\tfought_at\t1815\tWaterloo\n\
                     STATEMENT\texile\texiled_to\t1814\tElba\n\
                     STATEMENT\toffice\theld_office\t1804\tParis\n"
                ),
            },
            FixtureDoc {
                external_id: format!("fixture:{s}:timeline"),
                title: format!("{s} chronology"),
                language: "en".into(),
                document_type: DocumentType::ChronologyList,
                revision_id: "tl1".into(),
                relevance: 0.9,
                text: format!(
                    "Chronology of {s}\n\
                     * 1793 — Toulon — siege participation\n\
                     * 1798 — Cairo — Egyptian campaign\n\
                     * 1800 — Marengo — battle\n\
                     * 1802 — Amiens — peace treaty\n\
                     * 1804 — Notre Dame — coronation\n\
                     * 1812 — Moscow — retreat\n\
                     * 1840 — Paris — remains returned (posthumous)\n"
                ),
            },
            FixtureDoc {
                external_id: format!("fixture:{s}:travel"),
                title: format!("{s} residences"),
                language: "en".into(),
                document_type: DocumentType::Article,
                revision_id: "tr1".into(),
                relevance: 0.85,
                text: format!(
                    "{s} lived in Paris in 1792.\n\
                     He departed for Egypt in 1798.\n\
                     He arrived in Cairo in 1798.\n\
                     He resided at Malmaison in 1800.\n\
                     He stayed in Fontainebleau in 1814.\n"
                ),
            },
            FixtureDoc {
                external_id: format!("fixture:{s}:low_relevance"),
                title: "Unrelated local newsletter".into(),
                language: "en".into(),
                document_type: DocumentType::PressOcr,
                revision_id: "lr1".into(),
                relevance: 0.1,
                text: "Market prices rose in Lyon. Weather was mild.".into(),
            },
        ];
        Self::new(docs)
    }
}

#[async_trait]
impl SourceConnector for FixtureConnector {
    fn source_kind(&self) -> SourceKind {
        SourceKind::Fixture
    }

    fn connector_version(&self) -> &str {
        "fixture:v1"
    }

    async fn discover(
        &self,
        subject: &ResolvedSubject,
        cursor: Option<DiscoveryCursor>,
    ) -> Result<DiscoveryPage, ConnectorError> {
        let offset = cursor.map(|c| c.offset).unwrap_or(0) as usize;
        let page_size = 2usize;
        let slice: Vec<_> = self
            .docs
            .iter()
            .skip(offset)
            .take(page_size)
            .map(|d| DiscoveredDocument {
                source_kind: SourceKind::Fixture,
                external_id: d.external_id.clone(),
                canonical_url: Some(format!("fixture://{}", d.external_id)),
                title: d.title.clone(),
                language: Some(d.language.clone()),
                document_type: d.document_type.clone(),
                subject_links: vec![ExternalEntityRef {
                    system: "label".into(),
                    id: subject.label.clone(),
                    label: Some(subject.label.clone()),
                }],
                publication_time: None,
                discovery_method: DiscoveryMethod::Fixture,
                relevance_score: d.relevance,
                source_metadata: SourceMetadata {
                    raw: serde_json::json!({"revision": d.revision_id}),
                },
            })
            .collect();
        let next = if offset + page_size < self.docs.len() {
            Some(DiscoveryCursor {
                token: None,
                offset: (offset + page_size) as u32,
            })
        } else {
            None
        };
        Ok(DiscoveryPage {
            documents: slice,
            next_cursor: next,
        })
    }

    async fn fetch(
        &self,
        document: &DiscoveredDocument,
    ) -> Result<FetchedDocument, ConnectorError> {
        let doc = self
            .by_id
            .get(&document.external_id)
            .ok_or_else(|| ConnectorError::Parse(format!("unknown {}", document.external_id)))?;
        Ok(FetchedDocument {
            discovered: document.clone(),
            revision_id: Some(doc.revision_id.clone()),
            content_type: "text/plain".into(),
            text: doc.text.clone(),
            raw_metadata: serde_json::json!({"fixture": true}),
            license: Some("fixture".into()),
            content_bytes: doc.text.len() as u64,
        })
    }

    async fn healthcheck(&self) -> Result<ConnectorHealth, ConnectorError> {
        Ok(ConnectorHealth {
            ok: true,
            detail: format!("{} fixture docs", self.docs.len()),
        })
    }
}
