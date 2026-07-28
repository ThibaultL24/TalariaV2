// crates/talaria-store/src/lib.rs
pub mod canonical_events;
pub mod dump_runs;
pub mod entities;
pub mod judgments;
pub mod phrase_candidates;
pub mod places;
pub mod pool;
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
pub use places::{
    apply_geocode_to_events, get_place_geocode, list_place_labels_needing_geocode,
    upsert_place_geocode, PlaceGeocodeRow,
};
pub use phrase_candidates::{
    insert_phrase_candidate, list_pending_candidates, update_candidate_status, PendingCandidateRow,
    PhraseCandidateRecord,
};
pub use pool::{connect, run_migrations, DbPool};
pub use sentences::{
    list_sentences_for_extraction, replace_sentences_for_page, SentenceRecord, SentenceRow,
};
pub use wiki_pages::{
    list_pages_for_sentence_split, store_extracted_page, WikiPageRecord, WikiPageRow,
};
