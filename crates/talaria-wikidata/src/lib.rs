// crates/talaria-wikidata/src/lib.rs
mod client;
mod dump;
mod geocode;
mod search_rank;

pub use client::{WikidataClient, WikidataSearchHit};
pub use dump::{
    for_each_entity, slugify, stream_humans, DumpIngestStats, WikidataHuman, WikidataProfileRef,
    WikidataSitelink,
};
pub use geocode::{geocode_place_label, GeocodedPlace};
pub use search_rank::{person_search_score, sort_person_search_hits};
