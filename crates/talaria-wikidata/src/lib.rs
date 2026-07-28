// crates/talaria-wikidata/src/lib.rs
mod client;
mod geocode;

pub use client::{WikidataClient, WikidataSearchHit};
pub use geocode::{GeocodedPlace, geocode_place_label};
