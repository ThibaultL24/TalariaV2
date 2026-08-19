// crates/talaria-api/src/historiography.rs
//! Deterministic historiography extract → soft_claims (never canonical events).

use std::path::Path;

use talaria_core::AppConfig;
use talaria_sources::historiography::{
    is_historiography_section, scan_bibliographic, scan_passage, HistoriographyHit,
};
use talaria_store::{
    connect, find_active_singleton, find_claim_by_text, find_entity_by_wikipedia_title,
    insert_claim, insert_claim_evidence, list_entity_corpus_passages, list_sections_matching_page,
    run_migrations, search_local_entities, upsert_entity_with_kind, ClaimInsert,
};
use talaria_text::split_sentences;
use uuid::Uuid;

pub async fn run_historiography_extract(
    config: &AppConfig,
    subject: &str,
    file: Option<&Path>,
) -> anyhow::Result<()> {
    let pool = connect(config).await?;
    run_migrations(&pool).await?;
    let (entity_id, label, wiki_title) = if file.is_some() {
        let id = upsert_entity_with_kind(&pool, &config.wiki_lang, subject, "person").await?;
        (id, subject.to_string(), subject.to_string())
    } else {
        resolve_subject(&pool, &config.wiki_lang, subject).await?
    };

    let mut inserted = 0usize;
    let mut skipped = 0usize;
    let mut scanned = 0usize;

    if let Some(path) = file {
        let text = std::fs::read_to_string(path)?;
        let locator = format!("fixture:{}", path.display());
        let hits = scan_passage(&text);
        scanned += hits.len().max(1);
        for hit in hits {
            match persist_hit(&pool, entity_id, &hit, "fixture", locator.clone(), 0.55).await? {
                Persist::Inserted => inserted += 1,
                Persist::Skipped => skipped += 1,
            }
        }
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "subject": label,
                "entity_id": entity_id,
                "source": "fixture",
                "passages_scanned": scanned,
                "inserted": inserted,
                "skipped": skipped,
            }))?
        );
        return Ok(());
    }

    let pattern = format!("%{wiki_title}%");
    let sections = list_sections_matching_page(&pool, &config.wiki_lang, &pattern).await?;
    for section in sections {
        if !is_historiography_section(&section.title) {
            continue;
        }
        let sentences = split_sentences(&section.text);
        let quotes: Vec<String> = if sentences.is_empty() {
            scan_passage(&section.text)
                .into_iter()
                .map(|h| h.quote)
                .collect()
        } else {
            sentences.into_iter().map(|s| s.text).collect()
        };
        for quote in quotes {
            scanned += 1;
            let hits = scan_passage(&quote);
            for hit in hits {
                match persist_hit(
                    &pool,
                    entity_id,
                    &hit,
                    "wikipedia",
                    wiki_locator(&section.wiki_lang, &section.page_title, section.revision_id),
                    0.55,
                )
                .await?
                {
                    Persist::Inserted => inserted += 1,
                    Persist::Skipped => skipped += 1,
                }
            }
        }
    }

    let docs = list_entity_corpus_passages(&pool, entity_id, 200).await?;
    for doc in docs {
        scanned += 1;
        let hits = scan_bibliographic(&doc.title, doc.abstract_text.as_deref());
        let confidence = if doc.academic_status == "doctoral_defended" {
            0.55
        } else if doc.academic_status == "academic_unreviewed" {
            0.35
        } else {
            0.4
        };
        let locator = doc
            .canonical_url
            .clone()
            .unwrap_or_else(|| format!("{}:{}", doc.source_kind, doc.id));
        for hit in hits {
            match persist_hit(
                &pool,
                entity_id,
                &hit,
                &doc.source_kind,
                locator.clone(),
                confidence,
            )
            .await?
            {
                Persist::Inserted => inserted += 1,
                Persist::Skipped => skipped += 1,
            }
        }
    }

    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "subject": label,
            "entity_id": entity_id,
            "passages_scanned": scanned,
            "inserted": inserted,
            "skipped": skipped,
        }))?
    );
    Ok(())
}

async fn resolve_subject(
    pool: &sqlx::PgPool,
    wiki_lang: &str,
    subject: &str,
) -> anyhow::Result<(Uuid, String, String)> {
    if let Some(e) = find_entity_by_wikipedia_title(pool, wiki_lang, subject).await? {
        let label = e
            .canonical_name
            .clone()
            .unwrap_or(e.wikipedia_title.clone());
        return Ok((e.id, label, e.wikipedia_title));
    }
    let hits = search_local_entities(pool, subject, 1).await?;
    let Some(e) = hits.into_iter().next() else {
        anyhow::bail!("no entity matching {subject:?}");
    };
    let label = e
        .canonical_name
        .clone()
        .unwrap_or(e.wikipedia_title.clone());
    Ok((e.id, label, e.wikipedia_title))
}

enum Persist {
    Inserted,
    Skipped,
}

async fn persist_hit(
    pool: &sqlx::PgPool,
    entity_id: Uuid,
    hit: &HistoriographyHit,
    source_system: &str,
    locator: String,
    confidence: f64,
) -> anyhow::Result<Persist> {
    if find_claim_by_text(pool, entity_id, &hit.quote).await?.is_some() {
        return Ok(Persist::Skipped);
    }
    let event_id = match hit.event_hint {
        Some(hint) => find_active_singleton(pool, entity_id, hint.event_type()).await?,
        None => None,
    };
    let claim_id = insert_claim(
        pool,
        &ClaimInsert {
            entity_id,
            claim_kind: hit.claim_kind.into(),
            text: hit.quote.clone(),
            epistemic_status: hit.epistemic_status.into(),
            relation_to_subject: "historiography".into(),
            event_time: None,
            place_label: None,
            confidence,
            canonical_event_id: event_id,
            debate_type: Some(hit.debate_type.as_str().into()),
            evidence_layer: Some(hit.evidence_layer.as_str().into()),
        },
    )
    .await?;
    insert_claim_evidence(
        pool,
        claim_id,
        source_system,
        Some(&locator),
        Some(&hit.quote),
        None,
        confidence,
    )
    .await?;
    Ok(Persist::Inserted)
}

fn wiki_locator(lang: &str, title: &str, revision_id: Option<i64>) -> String {
    let slug = title.replace(' ', "_");
    match revision_id {
        Some(oldid) => format!("https://{lang}.wikipedia.org/w/index.php?title={slug}&oldid={oldid}"),
        None => format!("https://{lang}.wikipedia.org/wiki/{slug}"),
    }
}
