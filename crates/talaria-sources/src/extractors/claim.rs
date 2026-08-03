// crates/talaria-sources/src/extractors/claim.rs
use sha2::{Digest, Sha256};

use talaria_quality::{normalize_surface, TypedTime};

#[derive(Debug, Clone)]
pub struct ClaimKey {
    pub subject: String,
    pub predicate: String,
    pub object_or_value: String,
    pub time_key: String,
    pub place_key: String,
}

pub fn claim_fingerprint(key: &ClaimKey) -> String {
    let mut hasher = Sha256::new();
    for part in [
        normalize_surface(&key.subject),
        key.predicate.clone(),
        normalize_surface(&key.object_or_value),
        key.time_key.clone(),
        normalize_surface(&key.place_key),
    ] {
        hasher.update(part.as_bytes());
        hasher.update(b"|");
    }
    hex::encode(hasher.finalize())
}

pub fn time_bucket(time: &TypedTime) -> String {
    time.canonical_key()
}
