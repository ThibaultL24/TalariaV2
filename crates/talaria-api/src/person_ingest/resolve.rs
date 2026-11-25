// crates/talaria-api/src/person_ingest/resolve.rs
//! Resolve Wikidata QID before any person entity write.

use talaria_store::normalize_qid;
use talaria_wikidata::{resolve_person_hits, PersonResolution, WikidataClient};

pub async fn resolve_person_qid(
    explicit: Option<&str>,
    subject: &str,
    lang: &str,
) -> Option<String> {
    if let Some(qid) = explicit.and_then(normalize_qid) {
        return Some(qid);
    }
    let client = WikidataClient::new().ok()?;
    if let Some(qid) = choose_person(&client, subject, lang).await {
        return Some(qid);
    }
    if lang != "en" {
        return choose_person(&client, subject, "en").await;
    }
    None
}

async fn choose_person(client: &WikidataClient, subject: &str, lang: &str) -> Option<String> {
    let hits = client.search_entities(subject, lang, 8).await.ok()?;
    match resolve_person_hits(&hits, &[]) {
        PersonResolution::Resolved(qid) => Some(qid),
        PersonResolution::Ambiguous(_) | PersonResolution::Unresolved => None,
    }
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
