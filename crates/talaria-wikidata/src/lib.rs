// crates/talaria-wikidata/src/lib.rs
mod client;
mod dump;
mod geocode;

pub use client::{WikidataClient, WikidataSearchHit};
pub use dump::{
    for_each_entity, slugify, stream_humans, DumpIngestStats, WikidataHuman, WikidataProfileRef,
    WikidataSitelink,
};
pub use geocode::{geocode_place_label, GeocodedPlace};
