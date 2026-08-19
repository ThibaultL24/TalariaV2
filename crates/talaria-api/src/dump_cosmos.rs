// crates/talaria-api/src/dump_cosmos.rs
//! Lot C: Cosmos filter on dump sentence fragments. Writes no canonical_events.

use serde::Serialize;
use std::collections::HashMap;
use talaria_core::AppConfig;
use talaria_cosmos::{run_cosmos_batch, BatchInputItem, ExtractedTuple};
use talaria_quality::{
    ClauseAnalyzeInput, CosmosClauseAnalyzer, CosmosJudgment, CosmosTuple, HeuristicCosmosAnalyzer,
    COSMOS_HEURISTIC_ID, COSMOS_HEURISTIC_V1,
};
use talaria_store::{
    connect, insert_fragment_cosmos_judgment, list_sentence_fragments_for_cosmos, run_migrations,
    CosmosJudgmentInsert, CosmosJudgmentWrite, FragmentForCosmos,
};
use uuid::Uuid;

const COSMOS_SIDECAR_ID: &str = "cosmos-sidecar";
const COSMOS_SIDECAR_V1: &str = "sidecar:v1";

#[derive(Debug, Clone)]
pub struct DumpExtractOpts {
    pub run_id: Option<Uuid>,
    pub source_kind: Option<String>,
    pub min_score: f32,
    pub live: bool,
    pub skip_existing: bool,
    pub limit: usize,
    pub version: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DumpExtractReport {
    pub fragments_seen: u64,
    pub scored: u64,
    pub accepted: u64,
    pub rejected: u64,
    pub skipped_existing: u64,
    pub min_score: f32,
    pub analyzer_id: String,
    pub version: String,
    pub live: bool,
    pub canonical_events_written: u64,
}

pub async fn run_dump_extract_candidates(
    config: &AppConfig,
    opts: &DumpExtractOpts,
) -> anyhow::Result<DumpExtractReport> {
    let pool = connect(config).await?;
    run_migrations(&pool).await?;

    let heuristic = HeuristicCosmosAnalyzer::new(opts.min_score);
    let analyzer_id = if opts.live {
        COSMOS_SIDECAR_ID
    } else {
        COSMOS_HEURISTIC_ID
    };
    let version = opts
        .version
        .clone()
        .unwrap_or_else(|| {
            if opts.live {
                COSMOS_SIDECAR_V1.to_string()
            } else {
                COSMOS_HEURISTIC_V1.to_string()
            }
        });

    let fragments = list_sentence_fragments_for_cosmos(
        &pool,
        opts.run_id,
        opts.source_kind.as_deref(),
        analyzer_id,
        &version,
        opts.skip_existing,
        if opts.limit > 0 { opts.limit as i64 } else { 0 },
    )
    .await?;

    let sidecar_tuples = if opts.live {
        fetch_sidecar_tuples(config, &fragments).await?
    } else {
        HashMap::new()
    };

    let mut report = DumpExtractReport {
        fragments_seen: fragments.len() as u64,
        scored: 0,
        accepted: 0,
        rejected: 0,
        skipped_existing: 0,
        min_score: opts.min_score,
        analyzer_id: analyzer_id.into(),
        version: version.clone(),
        live: opts.live,
        canonical_events_written: 0,
    };

    for frag in &fragments {
        let input = ClauseAnalyzeInput {
            text: frag.text.clone(),
            page_title: frag.title.clone(),
            start_offset: frag.start_offset,
        };
        let mut judgment = heuristic.judge_fragment(&input);
        judgment.analyzer_id = analyzer_id.to_string();
        judgment.version = version.clone();
        if let Some(tuples) = sidecar_tuples.get(&frag.id.to_string()) {
            overlay_sidecar(&mut judgment, tuples, opts.min_score);
        }

        let insert = CosmosJudgmentInsert {
            fragment_id: frag.id,
            analyzer_id: judgment.analyzer_id.clone(),
            version: judgment.version.clone(),
            score: judgment.score,
            accepted: judgment.accepted,
            signals: serde_json::to_value(&judgment.signals)?,
            tuples: serde_json::to_value(&judgment.tuples)?,
            reject_reason: judgment.reject_reason.clone(),
        };
        let (_id, write) = insert_fragment_cosmos_judgment(&pool, &insert).await?;
        match write {
            CosmosJudgmentWrite::Existing => report.skipped_existing += 1,
            CosmosJudgmentWrite::Inserted => {
                report.scored += 1;
                if judgment.accepted {
                    report.accepted += 1;
                } else {
                    report.rejected += 1;
                }
            }
        }
    }

    // This command never inserts canonical_events; do not infer writes from a
    // global quality-event delta (races with parallel dump extract-events tests).
    report.canonical_events_written = 0;
    Ok(report)
}

async fn fetch_sidecar_tuples(
    config: &AppConfig,
    fragments: &[FragmentForCosmos],
) -> anyhow::Result<HashMap<String, Vec<ExtractedTuple>>> {
    if !config.cosmos_batch_script.exists() {
        anyhow::bail!(
            "COSMOS batch script not found at {} (omit --cosmos live)",
            config.cosmos_batch_script.display()
        );
    }
    let items: Vec<BatchInputItem> = fragments
        .iter()
        .map(|f| BatchInputItem {
            id: f.id.to_string(),
            text: f.text.clone(),
            page_title: f.title.clone(),
        })
        .collect();
    let config = config.clone();
    let script = config.cosmos_batch_script.clone();
    let outputs = tokio::task::spawn_blocking(move || run_cosmos_batch(&config, &script, &items))
        .await??;
    Ok(outputs
        .into_iter()
        .map(|o| (o.id, o.tuples))
        .collect())
}

fn overlay_sidecar(judgment: &mut CosmosJudgment, tuples: &[ExtractedTuple], min_score: f32) {
    if tuples.is_empty() {
        return;
    }
    if !judgment.signals.iter().any(|s| s == "cosmos_tuple") {
        judgment.signals.push("cosmos_tuple".into());
    }
    judgment.tuples = tuples
        .iter()
        .filter(|t| !t.person.trim().is_empty() && !t.time.trim().is_empty() && !t.place.trim().is_empty())
        .map(|t| CosmosTuple {
            person: t.person.clone(),
            time: t.time.clone(),
            place: t.place.clone(),
            verb: t.verb.clone(),
        })
        .collect();
    judgment.score = (judgment.score + 0.4).min(1.0);
    judgment.accepted = judgment.score >= min_score;
    if judgment.accepted {
        judgment.reject_reason = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dump_ingest::{run_dump_ingest, DumpIngestOpts};
    use std::path::PathBuf;
    use talaria_quality::{HeuristicCosmosAnalyzer, COSMOS_DEFAULT_MIN_SCORE};
    use talaria_store::{connect, get_fragment_cosmos_judgment, list_sentence_fragments_for_cosmos};

    fn fixture_path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/dumps/mini_events.jsonl")
    }

    fn test_config() -> AppConfig {
        let root_env = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../.env");
        let _ = dotenvy::from_path(&root_env);
        dotenvy::dotenv().ok();
        AppConfig::from_env().expect("DATABASE_URL")
    }

    #[test]
    fn unit_relevant_kept_empty_rejected() {
        let analyzer = HeuristicCosmosAnalyzer::default();
        let keep = analyzer.judge_fragment(&ClauseAnalyzeInput {
            text: "Napoleon Bonaparte was born on 15 August 1769 in Ajaccio.".into(),
            page_title: None,
            start_offset: 0,
        });
        let drop = analyzer.judge_fragment(&ClauseAnalyzeInput {
            text: "".into(),
            page_title: None,
            start_offset: 0,
        });
        assert!(keep.accepted);
        assert!(!drop.accepted);
    }

    #[tokio::test]
    async fn postgres_persists_scores_without_canonical_events() {
        let config = test_config();
        let kind = format!("jsonl-cosmos-{}", Uuid::new_v4());
        let ingest = run_dump_ingest(
            &config,
            &DumpIngestOpts {
                file: fixture_path(),
                source_kind: kind.clone(),
                subject: None,
                language: None,
                dry_run: false,
                skip_existing: false,
                limit: 0,
                resume_run: None,
            },
        )
        .await
        .unwrap();
        assert!(ingest.fragments >= 8);

        let version = format!("test-{}", Uuid::new_v4());
        let opts = DumpExtractOpts {
            run_id: ingest.run_id,
            source_kind: Some(kind),
            min_score: COSMOS_DEFAULT_MIN_SCORE,
            live: false,
            skip_existing: true,
            limit: 0,
            version: Some(version.clone()),
        };
        let first = run_dump_extract_candidates(&config, &opts).await.unwrap();
        assert!(first.scored >= 8);
        assert!(first.accepted >= 1);
        assert_eq!(first.canonical_events_written, 0);
        assert_eq!(first.analyzer_id, COSMOS_HEURISTIC_ID);

        let pool = connect(&config).await.unwrap();
        let fragments = list_sentence_fragments_for_cosmos(
            &pool,
            ingest.run_id,
            None,
            COSMOS_HEURISTIC_ID,
            &version,
            false,
            50,
        )
        .await
        .unwrap();
        let sample = fragments
            .iter()
            .find(|f| f.text.contains("Ajaccio"))
            .expect("ajaccio fragment");
        let stored = get_fragment_cosmos_judgment(
            &pool,
            sample.id,
            COSMOS_HEURISTIC_ID,
            &version,
        )
        .await
        .unwrap()
        .expect("persisted score");
        assert!(stored.score >= 0.0);
        assert!(stored.accepted);

        let second_opts = DumpExtractOpts {
            skip_existing: false,
            ..opts.clone()
        };
        let second = run_dump_extract_candidates(&config, &second_opts)
            .await
            .unwrap();
        assert_eq!(second.scored, 0);
        assert!(second.skipped_existing >= first.scored);
        assert_eq!(second.canonical_events_written, 0);

        let mut v2 = opts.clone();
        v2.version = Some(format!("{version}-b"));
        let recomputed = run_dump_extract_candidates(&config, &v2).await.unwrap();
        assert!(recomputed.scored >= 8);
        assert_eq!(recomputed.canonical_events_written, 0);
    }
}
