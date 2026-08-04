// crates/talaria-api/src/claim_extract.rs
//! Soft claim extraction from sentences (no geo/time gate).

use talaria_core::AppConfig;
use talaria_judge::classify_claim_text;
use talaria_store::{
    backfill_life_event_claims, connect, find_claim_by_text, insert_claim, insert_claim_evidence,
    list_sentences_for_claims, run_migrations, ClaimInsert,
};

pub async fn run_claims_extract(config: &AppConfig, limit: i64) -> anyhow::Result<()> {
    let pool = connect(config).await?;
    run_migrations(&pool).await?;

    let backfilled = backfill_life_event_claims(&pool).await?;
    tracing::info!(backfilled, "life_event claims from canonical_events");

    let fetch_limit = if limit <= 0 { 10_000 } else { limit };
    let sentences = list_sentences_for_claims(&pool, fetch_limit).await?;
    tracing::info!(count = sentences.len(), "sentences pending claim extract");

    let mut inserted = 0usize;
    let mut skipped = 0usize;

    for row in sentences {
        let Some(entity_id) = row.entity_id else {
            skipped += 1;
            continue;
        };
        let text = row.text.trim();
        if text.len() < 24 {
            skipped += 1;
            continue;
        }
        if is_identity_blurb(text) {
            skipped += 1;
            continue;
        }
        if find_claim_by_text(&pool, entity_id, text).await?.is_some() {
            skipped += 1;
            continue;
        }

        let class = classify_claim_text(text);
        let claim_id = insert_claim(
            &pool,
            &ClaimInsert {
                entity_id,
                claim_kind: class.kind.into(),
                text: text.to_string(),
                epistemic_status: class.epistemic_status.into(),
                relation_to_subject: class.relation_to_subject.into(),
                event_time: None,
                place_label: None,
                confidence: class.confidence,
                canonical_event_id: None,
            },
        )
        .await?;

        let locator = match row.revision_id {
            Some(oldid) => format!(
                "https://{}.wikipedia.org/w/index.php?title={}&oldid={oldid}",
                row.wiki_lang,
                row.page_title.replace(' ', "_")
            ),
            None => format!(
                "https://{}.wikipedia.org/wiki/{}",
                row.wiki_lang,
                row.page_title.replace(' ', "_")
            ),
        };

        insert_claim_evidence(
            &pool,
            claim_id,
            "wikipedia",
            Some(&locator),
            Some(text),
            Some(row.id),
            class.confidence,
        )
        .await?;
        inserted += 1;
    }

    tracing::info!(inserted, skipped, "claims extract done");
    Ok(())
}

fn is_identity_blurb(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    (lower.contains("was a ") || lower.contains("is a ") || lower.contains("was an "))
        && lower.len() < 140
        && !lower.contains("born")
        && !lower.contains("died")
}
