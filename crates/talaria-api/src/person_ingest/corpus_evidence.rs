// crates/talaria-api/src/person_ingest/corpus_evidence.rs
//! Candidate corpus references for existing person events.
//!
//! Bibliographic metadata is not historical evidence. Candidate matches are
//! quarantined in event_document_candidates and never appear in event_evidence.
//! Verified quoted fragments must be added by a separate evidence-grounded path.

use serde_json::json;
use sqlx::PgPool;
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
    find_active_person_events_for_entity, upsert_raw_corpus_document,
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
    pub candidate_links_added: u32,
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

    debug!(entity_id = %entity_id, events = existing_events.len(), "Catalog candidate discovery");

    // Query each enabled source
    for source_kind in &config.enabled_sources {
        match query_source(source_kind, subject, config.max_documents_per_source).await {
            Ok(page) => {
                stats.sources_queried += 1;
                stats.documents_discovered += page.documents.len() as u32;

                for doc in page.documents.iter().take(config.max_documents_per_source as usize) {
                    // Try to match document to existing events
                    if let Some(matched) = match_document_to_events(&doc, &existing_events) {
                        stats.documents_matched += 1;

                        // Never turn a catalog title or publication year into fact evidence.
                        let result = persist_corpus_candidates(pool, source_kind, doc, &matched).await;

                        match result {
                            Ok(added) => stats.candidate_links_added += added,
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
    PlaceMention,
    TitleMention,
}

impl MatchType {
    fn as_str(&self) -> &'static str {
        match self {
            Self::PlaceMention => "place_mention",
            Self::TitleMention => "title_mention",
        }
    }
}

fn match_document_to_events(
    doc: &talaria_sources::DiscoveredDocument,
    events: &[PersonEvent],
) -> Option<Vec<DocumentMatch>> {
    let mut matches = Vec::new();

    let title_lower = doc.title.to_lowercase();

    for event in events {
        let mut matched = false;
        let mut match_type = MatchType::TitleMention;
        let mut confidence = 0.5f32;

        // Place matching in title
        if let Some(ref place) = event.place_label {
            let place_lower = place.to_lowercase();
            if place_lower.chars().count() >= 4 && title_lower.contains(&place_lower) {
                matched = true;
                match_type = MatchType::PlaceMention;
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

async fn persist_corpus_candidates(
    pool: &PgPool,
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

    let mut candidates_added = 0u32;
    for m in matches {
        let result = sqlx::query(
            r#"
            INSERT INTO event_document_candidates (
                canonical_event_id, raw_document_id, match_rule, match_version, relevance_score
            )
            VALUES ($1, $2, $3, $4, $5)
            ON CONFLICT (canonical_event_id, raw_document_id, match_rule, match_version)
            DO NOTHING
            "#,
        )
        .bind(m.event_id)
        .bind(raw_doc_id)
        .bind(m.match_type.as_str())
        .bind(ENRICHMENT_VERSION)
        .bind(m.confidence as f64)
        .execute(pool)
        .await?;
        candidates_added += result.rows_affected() as u32;
    }
    Ok(candidates_added)
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

    #[test]
    fn publication_year_alone_is_not_a_candidate() {
        let event = PersonEvent {
            id: Uuid::new_v4(),
            entity_id: Uuid::new_v4(),
            event_type: "battle".into(),
            title: Some("Battle of Austerlitz".into()),
            place_label: Some("Austerlitz".into()),
            start_time: Some("1805-12-02T00:00:00Z".parse().unwrap()),
            occurrence_key: None,
        };
        let unrelated = make_doc("Research on unrelated subjects", Some(1805));
        assert!(match_document_to_events(&unrelated, &[event]).is_none());
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn catalog_metadata_never_creates_event_evidence(pool: sqlx::PgPool) {
        let entity_id: Uuid = sqlx::query_scalar(
            "INSERT INTO entities (wikipedia_title) VALUES ('Candidate fixture') RETURNING id",
        ).fetch_one(&pool).await.unwrap();
        let event_id: Uuid = sqlx::query_scalar(
            "INSERT INTO canonical_events (entity_id, event_type, title, pipeline) VALUES ($1,'battle','Austerlitz','person') RETURNING id",
        ).bind(entity_id).fetch_one(&pool).await.unwrap();
        let doc = make_doc("Battle of Austerlitz", Some(1805));
        let matches = [DocumentMatch {
            event_id,
            match_type: MatchType::TitleMention,
            confidence: 0.7,
        }];
        let first = persist_corpus_candidates(&pool, &SourceKind::Hal, &doc, &matches).await.unwrap();
        let duplicate = persist_corpus_candidates(&pool, &SourceKind::Hal, &doc, &matches).await.unwrap();
        assert_eq!((first, duplicate), (1, 0));
        let candidate_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM event_document_candidates WHERE canonical_event_id=$1",
        ).bind(event_id).fetch_one(&pool).await.unwrap();
        let evidence_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM event_evidence WHERE canonical_event_id=$1",
        ).bind(event_id).fetch_one(&pool).await.unwrap();
        assert_eq!(candidate_count, 1);
        assert_eq!(evidence_count, 0);
    }
}

