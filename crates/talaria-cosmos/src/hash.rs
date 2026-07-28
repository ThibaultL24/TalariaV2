// crates/talaria-cosmos/src/hash.rs
use sha2::{Digest, Sha256};
use uuid::Uuid;

pub fn combinator_hash(
    sentence_id: Uuid,
    person: &str,
    time: &str,
    place: &str,
    verb: Option<&str>,
) -> String {
    let verb = verb.unwrap_or("");
    let payload = format!("{sentence_id}|{person}|{time}|{place}|{verb}");
    hex::encode(Sha256::digest(payload.as_bytes()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::uuid;

    #[test]
    fn hash_is_stable() {
        let id = uuid!("550e8400-e29b-41d4-a716-446655440000");
        let a = combinator_hash(id, "Ada Lovelace", "1815", "London", Some("born"));
        let b = combinator_hash(id, "Ada Lovelace", "1815", "London", Some("born"));
        assert_eq!(a, b);
        assert_eq!(a.len(), 64);
    }
}
