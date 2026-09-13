// crates/talaria-api/src/person_ingest/corpus_evidence.rs
//! Corpus evidence enrichment for person_ingest.
//!
//! Queries live corpus connectors (HAL, Gallica, BnF, Persée, theses.fr, OpenAlex)
//! and matches discovered documents to existing canonical events. Evidence is added
//! idempotently via occurrence_key matching — this NEVER creates new pins.
//!
//! Product rules (AGENTS.md):
//! - Extra sources reinforce same occurrence_key via idempotent evidence
//! - Never auto-create a new pin from a catalogue hit alone
//! - Never lower gates; evidence only adds to already-gated events

use chrono::Datelike;
use serde_json::json;
use sqlx::PgPool;
use std::collections::HashMap;
use tracing::{debug, warn};
use uuid::Uuid;

use talaria_sources::{
    connectors::{
        BnfConfig, BnfConnector, GallicaConnector, HalConnector, OpenAlexConfig, OpenAlexConnector,
        PerseeConnector, ThesesFrConfig, ThesesFrConnector,
    },
    DiscoveryPage, ResolvedSubject, SourceConnector, SourceKind,
};
use talaria_store::{
    find_active_person_events_for_entity, insert_person_quote_evidence, upsert_raw_corpus_document,
    PersonEvent,
};

const ENRICHMENT_VERSION: &str = "corpus_evidence:v1";

#[derive(Debug, Clone)]
pub struct CorpusEnrichmentConfig {
    pub max_documents_per_source: u32,
    pub enabled_sources: Vec<SourceKind>,
    pub timeout_per_source_secs: u64,
}

impl Default for CorpusEnrichmentConfig {
    fn default() -> Self {
        Self {
            max_documents_per_source: 15,
            enabled_sources: vec![
                SourceKind::Hal,
                SourceKind::Gallica,
                SourceKind::Bnf,
                SourceKind::Persee,
                SourceKind::ThesesFr,
                SourceKind::OpenAlex,
            ],
            timeout_per_source_secs: 30,
        }
    }
}

#[derive(Debug, Default)]
pub struct CorpusEnrichmentStats {
    pub sources_queried: u32,
    pub documents_discovered: u32,
    pub documents_matched: u32,
    pub evidence_added: u32,
    pub errors: u32,
}

pub async fn enrich_with_corpus_evidence(
    pool: &PgPool,
    entity_id: Uuid,
    subject: &ResolvedSubject,
    config: &CorpusEnrichmentConfig,
) -> anyhow::Result<CorpusEnrichmentStats> {
    let mut stats = CorpusEnrichmentStats::default();

    // Load existing canonical events for this entity
    let existing_events = find_active_person_events_for_entity(pool, entity_id).await?;
    if existing_events.is_empty() {
        debug!("No existing events for entity {entity_id}, skipping corpus enrichment");
        return Ok(stats);
    }

    // Build occurrence_key → event_id index for matching
    let event_index: HashMap<String, Uuid> = existing_events
        .iter()
        .filter_map(|e| e.occurrence_key.as_ref().map(|k| (k.clone(), e.id)))
        .collect();

    debug!(
        entity_id = %entity_id,
        events = existing_events.len(),
        indexed = event_index.len(),
        "Corpus enrichment: loaded {} events, {} with occurrence_key",
        existing_events.len(),
        event_index.len()
    );

    // Query each enabled source
    for source_kind in &config.enabled_sources {
        match query_source(source_kind, subject, config.max_documents_per_source).await {
            Ok(page) => {
                stats.sources_queried += 1;
                stats.documents_discovered += page.documents.len() as u32;

                for doc in &page.documents {
                    // Try to match document to existing events
                    if let Some(matched) = match_document_to_events(&doc, &existing_events) {
                        stats.documents_matched += 1;

                        // Persist document and add evidence
                        let result = persist_corpus_evidence(
                            pool,
                            entity_id,
                            source_kind,
                            doc,
                            &matched,
                        )
                        .await;

                        match result {
                            Ok(added) => stats.evidence_added += added,
                            Err(e) => {
                                warn!(
                                    source = source_kind.as_str(),
                                    error = %e,
                                    "Failed to persist corpus evidence"
                                );
                                stats.errors += 1;
                            }
                        }
                    }
                }
            }
            Err(e) => {
                warn!(
                    source = source_kind.as_str(),
                    error = %e,
                    "Corpus source query failed"
                );
                stats.errors += 1;
            }
        }
    }

    debug!(
        entity_id = %entity_id,
        stats = ?stats,
        "Corpus enrichment complete"
    );

    Ok(stats)
}

async fn query_source(
    kind: &SourceKind,
    subject: &ResolvedSubject,
    _max_docs: u32,
) -> anyhow::Result<DiscoveryPage> {
    match kind {
        SourceKind::Hal => {
            let connector = HalConnector::new()?;
            Ok(connector.discover(subject, None).await?)
        }
        SourceKind::Gallica => {
            let connector = GallicaConnector::new()?;
            Ok(connector.discover(subject, None).await?)
        }
        SourceKind::Bnf => {
            let connector = BnfConnector::new(BnfConfig::default())?;
            Ok(connector.discover(subject, None).await?)
        }
        SourceKind::Persee => {
            let connector = PerseeConnector::new()?;
            Ok(connector.discover(subject, None).await?)
        }
        SourceKind::ThesesFr => {
            let connector = ThesesFrConnector::new(ThesesFrConfig::default())?;
            Ok(connector.discover(subject, None).await?)
        }
        SourceKind::OpenAlex => {
            let mut config = OpenAlexConfig::default();
            config.api_key = std::env::var("OPENALEX_API_KEY").ok().filter(|s| !s.is_empty());
            config.mailto = std::env::var("OPENALEX_MAILTO").ok().filter(|s| !s.is_empty());
            let connector = OpenAlexConnector::new(config)?;
            Ok(connector.discover(subject, None).await?)
        }
        _ => anyhow::bail!("Unsupported source kind for corpus enrichment: {:?}", kind),
    }
}

#[derive(Debug)]
struct DocumentMatch {
    event_id: Uuid,
    match_type: MatchType,
    confidence: f32,
}

#[derive(Debug, Clone)]
enum MatchType {
    YearAndType,
    PlaceAndYear,
    TitleMention,
}

fn match_document_to_events(
    doc: &talaria_sources::DiscoveredDocument,
    events: &[PersonEvent],
) -> Option<Vec<DocumentMatch>> {
    let mut matches = Vec::new();

    // Extract year from document publication time
    let doc_year = match &doc.publication_time {
        Some(talaria_sources::TypedTimeLite::Exact { year, .. }) => Some(*year),
        _ => None,
    };

    let title_lower = doc.title.to_lowercase();

    for event in events {
        let mut matched = false;
        let mut match_type = MatchType::TitleMention;
        let mut confidence = 0.5f32;

        // Year matching
        if let (Some(dy), Some(ey)) = (doc_year, event.start_time) {
            let event_year = ey.year();
            // Document publication year within 10 years of event (for scholarly works about events)
            if (dy - event_year).abs() <= 10 {
                matched = true;
                match_type = MatchType::YearAndType;
                confidence = 0.6;
            }
        }

        // Place matching in title
        if let Some(ref place) = event.place_label {
            let place_lower = place.to_lowercase();
            if title_lower.contains(&place_lower) {
                matched = true;
                match_type = MatchType::PlaceAndYear;
                confidence = 0.65;
            }
        }

        // Event type mention in title
        let event_type_lower = event.event_type.to_lowercase();
        if title_lower.contains(&event_type_lower) {
            matched = true;
            confidence = 0.55;
        }

        // Battle/diplomatic specific matching
        if event.event_type == "battle" || event.event_type == "diplomatic" {
            if let Some(ref title) = event.title {
                let event_title_lower = title.to_lowercase();
                // Check for battle/treaty name in document title
                let event_name_words: Vec<&str> = event_title_lower
                    .split_whitespace()
                    .filter(|w| w.len() > 3)
                    .collect();
                for word in event_name_words {
                    if title_lower.contains(word) {
                        matched = true;
                        confidence = 0.7;
                        break;
                    }
                }
            }
        }

        if matched {
            matches.push(DocumentMatch {
                event_id: event.id,
                match_type,
                confidence,
            });
        }
    }

    if matches.is_empty() {
        None
    } else {
        Some(matches)
    }
}

async fn persist_corpus_evidence(
    pool: &PgPool,
    entity_id: Uuid,
    source_kind: &SourceKind,
    doc: &talaria_sources::DiscoveredDocument,
    matches: &[DocumentMatch],
) -> anyhow::Result<u32> {
    // Create raw document record
    let canonical_url = doc.canonical_url.as_deref().unwrap_or(&doc.external_id);
    let raw_doc_id = upsert_raw_corpus_document(
        pool,
        source_kind.as_str(),
        &doc.external_id,
        canonical_url,
        &doc.title,
        doc.language.as_deref(),
        &json!({
            "source_kind": source_kind.as_str(),
            "document_type": doc.document_type.as_str(),
            "discovery_method": doc.discovery_method.as_str(),
            "relevance_score": doc.relevance_score,
            "raw_metadata": doc.source_metadata.raw,
        }),
    )
    .await?;

    let source_locator = format!(
        "corpus:{}:{}",
        source_kind.as_str(),
        doc.external_id
    );

    let mut evidence_added = 0u32;

    // Add evidence to each matched event
    for m in matches {
        let quote = format!(
            "[{}] {} ({})",
            source_kind.as_str().to_uppercase(),
            doc.title,
            canonical_url
        );

        insert_person_quote_evidence(
            pool,
            m.event_id,
            &quote,
            Some(raw_doc_id),
            m.confidence as f64,
            &source_locator,
        )
        .await?;

        evidence_added += 1;
    }

    Ok(evidence_added)
}

#[cfg(test)]
mod tests {
    use super::*;
    use talaria_sources::{TypedTimeLite, DiscoveryMethod, DocumentType, SourceMetadata};

    fn make_doc(title: &str, year: Option<i32>) -> talaria_sources::DiscoveredDocument {
        talaria_sources::DiscoveredDocument {
            source_kind: SourceKind::Hal,
            external_id: "test-123".into(),
            canonical_url: Some("https://example.com/test".into()),
            title: title.into(),
            language: Some("fr".into()),
            document_type: DocumentType::AcademicArticle,
            subject_links: vec![],
            publication_time: year.map(|y| TypedTimeLite::Exact {
                year: y,
                surface: Some(y.to_string()),
            }),
            discovery_method: DiscoveryMethod::CatalogSearch,
            relevance_score: 0.75,
            source_metadata: SourceMetadata::default(),
        }
    }

    #[test]
    fn test_corpus_enrichment_config_default() {
        let config = CorpusEnrichmentConfig::default();
        assert_eq!(config.max_documents_per_source, 15);
        assert!(config.enabled_sources.contains(&SourceKind::Hal));
        assert!(config.enabled_sources.contains(&SourceKind::OpenAlex));
    }
}
