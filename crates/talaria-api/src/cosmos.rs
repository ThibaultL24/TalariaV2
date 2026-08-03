// crates/talaria-api/src/cosmos.rs
use talaria_core::AppConfig;
use talaria_cosmos::{
    combinator_hash, mock_extract, run_cosmos_batch, BatchInputItem, BatchOutputItem,
};
use talaria_store::{
    connect, insert_phrase_candidate, list_sentences_for_extraction, run_migrations,
    upsert_entity_surface, PhraseCandidateRecord,
};
use uuid::Uuid;

const EXTRACTOR_COSMOS: &str = "cosmos:tuple_extraction";
const EXTRACTOR_MOCK: &str = "mock:life_events";

pub async fn run_cosmos_extract(
    config: &AppConfig,
    batch_size: usize,
    limit: i64,
    skip_existing: bool,
    mock: bool,
) -> anyhow::Result<()> {
    let pool = connect(config).await?;
    run_migrations(&pool).await?;

    if !mock && !config.cosmos_batch_script.exists() {
        anyhow::bail!(
            "COSMOS batch script not found at {} (use --mock for dev)",
            config.cosmos_batch_script.display()
        );
    }

    let sentence_limit = if limit > 0 { limit } else { i64::MAX };
    let sentences =
        list_sentences_for_extraction(&pool, &config.wiki_lang, sentence_limit, skip_existing)
            .await?;

    tracing::info!(
        sentences = sentences.len(),
        batch_size,
        mock,
        skip_existing,
        "starting phrase-candidate extraction"
    );

    let mut candidates_inserted = 0usize;
    let mut candidates_skipped = 0usize;
    let mut sentences_processed = 0usize;

    for chunk in sentences.chunks(batch_size.max(1)) {
        let batch_inputs: Vec<BatchInputItem> = chunk
            .iter()
            .map(|row| BatchInputItem {
                id: row.id.to_string(),
                text: row.text.clone(),
            })
            .collect();

        let outputs = if mock {
            mock_extract(&batch_inputs)
        } else {
            tokio::task::spawn_blocking({
                let config = config.clone();
                let script = config.cosmos_batch_script.clone();
                move || run_cosmos_batch(&config, &script, &batch_inputs)
            })
            .await??
        };

        let (inserted, skipped) =
            persist_batch(&pool, &config.wiki_lang, mock, &outputs).await?;
        candidates_inserted += inserted;
        candidates_skipped += skipped;
        sentences_processed += chunk.len();

        if sentences_processed.is_multiple_of(500) {
            tracing::info!(
                sentences_processed,
                candidates_inserted,
                "cosmos extraction progress"
            );
        }
    }

    tracing::info!(
        sentences_processed,
        candidates_inserted,
        candidates_skipped,
        "phrase-candidate extraction complete"
    );

    Ok(())
}

async fn persist_batch(
    pool: &sqlx::PgPool,
    wiki_lang: &str,
    mock: bool,
    outputs: &[BatchOutputItem],
) -> anyhow::Result<(usize, usize)> {
    let extractor = if mock {
        EXTRACTOR_MOCK
    } else {
        EXTRACTOR_COSMOS
    };

    let mut inserted = 0usize;
    let mut skipped = 0usize;

    for output in outputs {
        let sentence_id = Uuid::parse_str(&output.id)?;

        for tuple in &output.tuples {
            let entity_id =
                upsert_entity_surface(pool, wiki_lang, &tuple.person).await?;
            let hash = combinator_hash(
                sentence_id,
                &tuple.person,
                &tuple.time,
                &tuple.place,
                tuple.verb.as_deref(),
            );

            let record = PhraseCandidateRecord {
                sentence_id,
                entity_id: Some(entity_id),
                person_surface: tuple.person.clone(),
                time_surface: Some(tuple.time.clone()),
                place_surface: Some(tuple.place.clone()),
                verb_pivot: tuple.verb.clone(),
                combinator_hash: hash,
                extractor: extractor.into(),
            };

            if insert_phrase_candidate(pool, &record).await?.is_some() {
                inserted += 1;
            } else {
                skipped += 1;
            }
        }
    }

    Ok((inserted, skipped))
}
