// crates/talaria-store/src/lib.rs
pub mod canonical_events;
pub mod dump_runs;
pub mod entities;
pub mod judgments;
pub mod multi_source;
pub mod phrase_candidates;
pub mod places;
pub mod pool;
pub mod quality;
pub mod sentences;
pub mod wiki_pages;

pub use canonical_events::{
    find_existing_event, get_canonical_event, insert_canonical_event, insert_event_evidence,
    list_event_evidence, list_event_narrative_context, list_geojson_events, list_timeline_events,
    refresh_event_source_refs, CanonicalEventInsert, CanonicalEventRow, EventEvidenceRow,
    NarrativeContextRow,
};
pub use dump_runs::{finish_dump_run, start_dump_run};
pub use entities::{
    find_entity_by_qid, get_entity, search_local_entities, update_entity_qid, upsert_entity_surface,
    EntityRow,
};
pub use judgments::insert_judgment;
pub use multi_source::{
    add_claim_support, density_report_counts, finish_discovery_run, link_claim_to_event,
    mark_discovered_skipped, mark_discovered_snapshotted, start_discovery_run,
    upsert_discovered_document, upsert_quality_claim, DensityReportCounts, DiscoveredDocumentInsert,
    DiscoveryRunInsert, QualityClaimInsert,
};
pub use places::{
    apply_geocode_to_events, get_place_geocode, list_place_labels_needing_geocode,
    upsert_place_geocode, PlaceGeocodeRow,
};
pub use phrase_candidates::{
    insert_phrase_candidate, list_pending_candidates, update_candidate_status, PendingCandidateRow,
    PhraseCandidateRecord,
};
pub use pool::{connect, run_migrations, DbPool};
pub use quality::{
    apply_place_to_quality_event, count_active_quality_by_type,
    find_active_quality_event_by_fingerprint, find_active_singleton, get_entity_kind,
    get_event_candidate_by_fingerprint, insert_document_fragment, insert_document_snapshot,
    insert_quality_canonical_event, list_event_candidates_by_status, mark_candidate_assembled,
    quality_lifespan_years, quality_report_counts, reinforce_quality_event,
    rejection_reason_counts, update_event_candidate_judgment, upsert_entity_with_kind,
    upsert_event_candidate, DocumentFragmentInsert, DocumentSnapshotInsert, EventCandidateInsert,
    EventCandidateRow, QualityEventInsert, QualityReportCounts, RejectionReasonCount,
};
pub use sentences::{
    list_sentences_for_extraction, replace_sentences_for_page, SentenceRecord, SentenceRow,
};
pub use wiki_pages::{
    list_pages_for_sentence_split, store_extracted_page, WikiPageRecord, WikiPageRow,
};
