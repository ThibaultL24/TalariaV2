// crates/talaria-store/src/lib.rs
#![allow(clippy::too_many_arguments)]

pub mod canonical_events;
pub mod claims;
pub mod corpus;
pub mod corpus_dump;
pub mod cosmos_judgments;
pub mod dump_runs;
pub mod entities;
pub mod intuition;
pub mod judgments;
pub mod multi_source;
pub mod phrase_candidates;
pub mod places;
pub mod pool;
pub mod profiles;
pub mod quality;
pub mod sentences;
pub mod wiki_pages;
pub mod wiki_sections;

pub use canonical_events::{
    find_existing_event, get_canonical_event, insert_canonical_event, insert_event_evidence,
    list_event_evidence, list_event_narrative_context, list_geojson_events, list_timeline_events,
    refresh_event_source_refs, CanonicalEventInsert, CanonicalEventRow, EventEvidenceRow,
    NarrativeContextRow,
};
pub use claims::{
    backfill_life_event_claims, find_claim_by_text, insert_claim, insert_claim_evidence,
    insert_claim_relation, list_claim_evidence, list_claims_for_entity, list_sentences_for_claims,
    ClaimEvidenceRow, ClaimInsert, ClaimRow, SentenceForClaims,
};
pub use corpus::{
    count_corpus_snapshots, get_corpus_document, link_corpus_snapshot, list_document_contributions,
    list_document_identifiers, list_entity_corpus_passages, list_entity_documents,
    mark_discovered_corpus_document,
    replace_document_contributions, replace_document_identifiers, replace_document_subjects,
    upsert_corpus_document, upsert_entity_document_link, ContributionInsert, CorpusDocumentInsert,
    CorpusDocumentRow, CorpusPassageRow, DocumentContributionRow, DocumentIdentifierRow,
    EntityDocumentLinkInsert, EntityDocumentsFilter, EntityLinkedDocumentRow, SubjectInsert,
};
pub use corpus_dump::{
    corpus_dump_document_status, corpus_dump_document_status_counts, finish_corpus_dump_run,
    get_corpus_dump_run, latest_corpus_dump_run, mark_corpus_dump_running, start_corpus_dump_run, update_corpus_dump_progress,
    upsert_corpus_dump_document, CorpusDumpDocumentUpsert, CorpusDumpRunInsert, CorpusDumpRunRow,
};
pub use cosmos_judgments::{
    count_fragment_cosmos_judgments, get_fragment_cosmos_judgment, insert_fragment_cosmos_judgment,
    list_cosmos_accepted_fragments, list_sentence_fragments_for_cosmos, CosmosJudgmentInsert,
    CosmosJudgmentRow, CosmosJudgmentWrite, FragmentForCosmos,
};
pub use dump_runs::{finish_dump_run, start_dump_run};
pub use entities::{
    find_entity_by_qid, find_entity_by_wikipedia_title, get_entity, search_local_entities,
    update_entity_qid, upsert_entity_from_wikidata, upsert_entity_surface, EntityRow,
};
pub use intuition::{
    find_quality_event_for_stem, get_intuition_publication_by_fingerprint, get_quality_event_pointer,
    list_conflict_quality_claims, list_exportable_soft_claims, mark_intuition_failed,
    mark_intuition_pin_failed, mark_intuition_published, upsert_intuition_publication, EventPointerRow, IntuitionPublicationInsert,
    IntuitionPublicationRow, QualityConflictRow, SoftClaimExportRow,
};
pub use judgments::insert_judgment;
pub use multi_source::{
    add_claim_support, density_report_counts, finish_discovery_run, link_claim_to_event,
    list_place_labels_for_occurrence_stem, mark_discovered_skipped, mark_discovered_snapshotted,
    mark_quality_claims_conflict_by_stem, mark_quality_events_uncertain_by_stem,
    start_discovery_run, upsert_discovered_document, upsert_quality_claim, DensityReportCounts,
    DiscoveredDocumentInsert, DiscoveryRunInsert, QualityClaimInsert,
};
pub use phrase_candidates::{
    insert_phrase_candidate, list_pending_candidates, update_candidate_status, PendingCandidateRow,
    PhraseCandidateRecord,
};
pub use places::{
    apply_geocode_to_events, get_place_geocode, list_place_labels_needing_geocode,
    upsert_place_geocode, PlaceGeocodeRow,
};
pub use pool::{connect, run_migrations, DbPool};
pub use profiles::{
    get_period_by_slug, link_entity_period, link_entity_to_centuries, list_entity_profiles,
    list_periods, list_profile_catalog, seed_default_periods, upsert_entity_profile, upsert_period,
    EntityProfileRow, PeriodRow,
};
pub use quality::{
    apply_place_to_quality_event, count_active_quality_by_type,
    find_active_quality_event_by_fingerprint, find_active_quality_event_by_occurrence_key,
    find_active_singleton, get_entity_kind, get_event_candidate_by_fingerprint,
    count_sentence_fragments, find_document_snapshot, insert_document_fragment,
    insert_document_snapshot, insert_quality_canonical_event,
    list_event_candidates_by_status, mark_candidate_assembled, quality_lifespan_years,
    quality_report_counts, reinforce_quality_event, rejection_reason_counts,
    update_event_candidate_judgment, upsert_entity_with_kind, upsert_event_candidate,
    DocumentFragmentInsert, DocumentSnapshotInsert, EventCandidateInsert, EventCandidateRow,
    QualityEventInsert, QualityReportCounts, RejectionReasonCount,
};
pub use sentences::{
    list_sentences_for_extraction, replace_sentences_for_page, SentenceRecord, SentenceRow,
};
pub use wiki_pages::{
    list_pages_for_sentence_split, store_extracted_page, WikiPageRecord, WikiPageRow,
};
pub use wiki_sections::{
    list_pages_for_section_split, list_sections_for_title, list_sections_matching_page,
    replace_sections_for_page, HistoriographySectionRow, WikiSectionRecord, WikiSectionRow,
};
