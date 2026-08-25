// crates/talaria-wikidata/src/lib.rs
mod claims;
mod client;
mod dump;
mod geocode;
mod promote;
mod search_rank;
mod time;

pub use claims::{
    identity_year, parse_entity_claims, promoted_statement_lines, ParsedStatement, StatementInsert,
};
pub use client::{WikidataClient, WikidataSearchHit};
pub use dump::{
    for_each_entity, slugify, stream_entities_for_qids, stream_humans, DumpIngestStats,
    WikidataHuman, WikidataProfileRef, WikidataSitelink,
};
pub use geocode::{geocode_place_label, GeocodedPlace};
pub use promote::promote_event;
pub use search_rank::{person_search_score, sort_person_search_hits};
pub use time::{parse_wikibase_time, WikibaseTime};
