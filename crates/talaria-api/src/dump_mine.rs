// crates/talaria-api/src/dump_mine.rs
//! Scan dump sentences for anecdotes and extra life-event keywords.

use talaria_core::AppConfig;
use talaria_cosmos::combinator_hash;
use talaria_judge::mine_sentence;
use talaria_store::{
    connect, insert_phrase_candidate, list_sentences_for_dump_mine, run_migrations,
    upsert_entity_surface, PhraseCandidateRecord,
};

pub async fn run_dump_mine(config: &AppConfig, limit: i64) -> anyhow::Result<()> {
    let pool = connect(config).await?;
    run_migrations(&pool).await?;

    let fetch_limit = if limit > 0 { limit } else { i64::MAX };
    let sentences =
        list_sentences_for_dump_mine(&pool, &config.wiki_lang, fetch_limit).await?;
    tracing::info!(sentences = sentences.len(), "dump-mine scanning sentences");

    let mut inserted = 0usize;
    let mut skipped = 0usize;
    let mut anecdotes = 0usize;

    for row in sentences {
        let mined = mine_sentence(&row.text, &row.page_title);
        if mined.is_empty() {
            skipped += 1;
            continue;
        }
        for hit in mined {
            if hit.extractor.contains("anecdote") {
                anecdotes += 1;
            }
            let entity_id =
                upsert_entity_surface(&pool, &config.wiki_lang, &hit.person).await?;
            let hash = combinator_hash(
                row.id,
                &hit.person,
                &hit.time,
                &hit.place,
                Some(&hit.verb),
            );
            let record = PhraseCandidateRecord {
                sentence_id: row.id,
                entity_id: Some(entity_id),
                person_surface: hit.person,
                time_surface: Some(hit.time),
                place_surface: Some(hit.place),
                verb_pivot: Some(hit.verb),
                combinator_hash: hash,
                extractor: hit.extractor.into(),
            };
            if insert_phrase_candidate(&pool, &record).await?.is_some() {
                inserted += 1;
            } else {
                skipped += 1;
            }
        }
    }

    tracing::info!(inserted, skipped, anecdotes, "dump-mine complete");
    Ok(())
}
