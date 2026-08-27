// crates/talaria-api/src/person_ingest/resolve.rs
//! Resolve Wikidata QID before any person entity write.

use talaria_store::normalize_qid;
use talaria_wikidata::WikidataClient;

pub async fn resolve_person_qid(
    explicit: Option<&str>,
    subject: &str,
    lang: &str,
) -> Option<String> {
    if let Some(qid) = explicit.and_then(normalize_qid) {
        return Some(qid);
    }
    let client = WikidataClient::new().ok()?;
    if let Ok(Some(qid)) = client.search_entity(subject, lang).await {
        return Some(qid);
    }
    if lang != "en" {
        return client.search_entity(subject, "en").await.ok().flatten();
    }
    None
}

pub async fn require_person_qid(
    explicit: Option<&str>,
    subject: &str,
    lang: &str,
) -> anyhow::Result<String> {
    resolve_person_qid(explicit, subject, lang)
        .await
        .ok_or_else(|| anyhow::anyhow!("qid_unresolved"))
}
