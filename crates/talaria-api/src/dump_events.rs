// crates/talaria-api/src/dump_events.rs
//! Lot D: Cosmos-accepted phrases → extractors → event_candidates → quality events.

use serde::Serialize;
use talaria_core::AppConfig;
use talaria_quality::{DerivedLabelProjections, GazetteerResolver, COSMOS_HEURISTIC_ID, COSMOS_HEURISTIC_V1};
use talaria_sources::extractors::{default_extractor_stack, CandidateExtractor, ExtractorInput};
use talaria_sources::ResolvedSubject;
use talaria_store::{
    connect, list_cosmos_accepted_fragments, run_migrations, upsert_entity_with_kind,
};
use uuid::Uuid;

use crate::ingest::{process_raw_candidate, IngestMetrics};

#[derive(Debug, Clone)]
pub struct DumpEventsOpts {
    pub run_id: Option<Uuid>,
    pub source_kind: Option<String>,
    pub subject: String,
    pub extractors: Option<Vec<String>>,
    pub analyzer_id: String,
    pub version: String,
    pub limit: usize,
    pub assemble: bool,
}

impl Default for DumpEventsOpts {
    fn default() -> Self {
        Self {
            run_id: None,
            source_kind: None,
            subject: String::new(),
            extractors: None,
            analyzer_id: COSMOS_HEURISTIC_ID.into(),
            version: COSMOS_HEURISTIC_V1.into(),
            limit: 0,
            assemble: false,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct DumpEventsReport {
    pub fragments: u64,
    pub raw_extracted: u64,
    pub candidates: u64,
    pub accepted: u64,
    pub rejected: u64,
    pub events_created: u64,
    pub events_reinforced: u64,
    pub assemble: bool,
    pub pipeline: String,
}

pub async fn run_dump_extract_events(
    config: &AppConfig,
    opts: &DumpEventsOpts,
) -> anyhow::Result<DumpEventsReport> {
    run_dump_events(config, opts, false).await
}

pub async fn run_dump_canonicalize(
    config: &AppConfig,
    opts: &DumpEventsOpts,
) -> anyhow::Result<DumpEventsReport> {
    let mut opts = opts.clone();
    opts.assemble = true;
    run_dump_events(config, &opts, true).await
}

async fn run_dump_events(
    config: &AppConfig,
    opts: &DumpEventsOpts,
    assemble: bool,
) -> anyhow::Result<DumpEventsReport> {
    let pool = connect(config).await?;
    run_migrations(&pool).await?;

    let subject_id =
        upsert_entity_with_kind(&pool, &config.wiki_lang, &opts.subject, "person").await?;
    let subject = ResolvedSubject {
        entity_id: Some(subject_id),
        qid: None,
        label: opts.subject.clone(),
        languages: vec![config.wiki_lang.clone()],
        birth_year: None,
        death_year: None,
        countries: vec![],
        occupations: vec![],
        known_identifiers: vec![],
    };

    let fragments = list_cosmos_accepted_fragments(
        &pool,
        opts.run_id,
        opts.source_kind.as_deref(),
        &opts.analyzer_id,
        &opts.version,
        if opts.limit > 0 { opts.limit as i64 } else { 0 },
    )
    .await?;

    let extractors = select_extractors(opts.extractors.as_deref());
    let resolver = GazetteerResolver;
    let projections = DerivedLabelProjections;
    let mut metrics = IngestMetrics::default();
    let mut raw_extracted = 0u64;

    for frag in &fragments {
        let input = ExtractorInput {
            text: frag.text.clone(),
            page_title: frag.title.clone().or_else(|| Some(opts.subject.clone())),
            subject_label: Some(opts.subject.clone()),
            document_type: "encyclopedia_article".into(),
            subject_death_year: None,
            ..Default::default()
        };
        let mut raws = Vec::new();
        for ex in &extractors {
            raws.extend(ex.extract(&input));
        }
        raw_extracted += raws.len() as u64;
        for raw in raws {
            process_raw_candidate(
                &pool,
                config,
                &subject,
                subject_id,
                frag.snapshot_id,
                frag.id,
                "dump",
                &raw,
                &resolver,
                &projections,
                assemble,
                &mut metrics,
            )
            .await?;
        }
    }

    Ok(DumpEventsReport {
        fragments: fragments.len() as u64,
        raw_extracted,
        candidates: metrics.candidates,
        accepted: metrics.accepted,
        rejected: metrics.rejected,
        events_created: metrics.events_created,
        events_reinforced: metrics.events_reinforced,
        assemble,
        pipeline: "quality".into(),
    })
}

fn select_extractors(filter: Option<&[String]>) -> Vec<Box<dyn CandidateExtractor>> {
    let stack = default_extractor_stack();
    match filter {
        Some(ids) if !ids.is_empty() => stack
            .into_iter()
            .filter(|ex| ids.iter().any(|id| id == ex.extractor_id()))
            .collect(),
        _ => stack,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Datelike;
    use crate::dump_cosmos::{run_dump_extract_candidates, DumpExtractOpts};
    use crate::dump_ingest::{run_dump_ingest, DumpIngestOpts};
    use std::path::PathBuf;
    use talaria_quality::COSMOS_DEFAULT_MIN_SCORE;
    use talaria_store::connect;

    fn fixture_path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/dumps/mini_events.jsonl")
    }

    fn test_config() -> AppConfig {
        let root_env = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../.env");
        let _ = dotenvy::from_path(&root_env);
        dotenvy::dotenv().ok();
        AppConfig::from_env().expect("DATABASE_URL")
    }

    async fn seed_accepted(
        config: &AppConfig,
    ) -> (Uuid, String, String) {
        let kind = format!("jsonl-lotd-{}", Uuid::new_v4());
        let ingest = run_dump_ingest(
            config,
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
        let version = format!("lotd-{}", Uuid::new_v4());
        let cosmos = run_dump_extract_candidates(
            config,
            &DumpExtractOpts {
                run_id: ingest.run_id,
                source_kind: Some(kind.clone()),
                min_score: COSMOS_DEFAULT_MIN_SCORE,
                live: false,
                skip_existing: true,
                limit: 0,
                version: Some(version.clone()),
            },
        )
        .await
        .unwrap();
        assert!(cosmos.accepted >= 1);
        (ingest.run_id.unwrap(), kind, version)
    }

    fn events_opts(
        run_id: Uuid,
        kind: &str,
        version: &str,
        subject: &str,
        assemble: bool,
    ) -> DumpEventsOpts {
        DumpEventsOpts {
            run_id: Some(run_id),
            source_kind: Some(kind.into()),
            subject: subject.into(),
            extractors: Some(vec!["dense_clause".into()]),
            analyzer_id: COSMOS_HEURISTIC_ID.into(),
            version: version.into(),
            limit: 0,
            assemble,
        }
    }

    #[tokio::test]
    async fn extract_then_canonicalize_quality_events_from_fragments() {
        let config = test_config();
        let (run_id, kind, version) = seed_accepted(&config).await;
        // Unique entity so leftover Napoleon quality rows cannot swallow assemble.
        let subject = format!("Napoleon LotD {}", Uuid::new_v4());
        let opts = events_opts(run_id, &kind, &version, &subject, false);

        let extracted = run_dump_extract_events(&config, &opts).await.unwrap();
        assert!(extracted.candidates >= 2, "{extracted:?}");
        assert!(extracted.accepted >= 2, "{extracted:?}");
        assert_eq!(extracted.events_created, 0);
        assert_eq!(extracted.pipeline, "quality");

        let canon = run_dump_canonicalize(&config, &opts).await.unwrap();
        assert!(canon.events_created >= 2, "{canon:?}");

        let pool = connect(&config).await.unwrap();
        let subject_id = upsert_entity_with_kind(&pool, &config.wiki_lang, &subject, "person")
            .await
            .unwrap();

        let rows: Vec<(String, serde_json::Value, Option<chrono::DateTime<chrono::Utc>>)> =
            sqlx::query_as(
                r#"
                SELECT event_type, time_json, start_time
                FROM canonical_events
                WHERE entity_id = $1 AND pipeline = 'quality' AND is_active
                "#,
            )
            .bind(subject_id)
            .fetch_all(&pool)
            .await
            .unwrap();
        assert!(
            rows.iter().any(|(t, _, _)| t == "birth"),
            "missing birth: {rows:?}"
        );
        assert!(
            rows.iter().any(|(t, _, _)| t == "death"),
            "missing death: {rows:?}"
        );

        let birth = rows.iter().find(|(t, _, _)| t == "birth").unwrap();
        let month = birth.1.get("month").and_then(|v| v.as_u64());
        let day = birth.1.get("day").and_then(|v| v.as_u64());
        assert_eq!(month, Some(8), "exact birth date: {}", birth.1);
        assert_eq!(day, Some(15), "exact birth date: {}", birth.1);

        if let Some(exile) = rows.iter().find(|(t, _, _)| t == "exile") {
            assert!(exile.1.get("month").is_none() || exile.1.get("month") == Some(&serde_json::Value::Null));
            if let Some(start) = exile.2 {
                assert_ne!(start.month(), 1, "year-only must not coerce to 1 Jan: {start}");
            }
        }

        let deaths: Vec<_> = rows.iter().filter(|(t, _, _)| t == "death").collect();
        assert_eq!(deaths.len(), 1, "singleton death: {deaths:?}");

        let cand_deaths: i64 = sqlx::query_scalar(
            r#"
            SELECT COUNT(*)::bigint FROM event_candidates
            WHERE subject_entity_id = $1 AND event_type = 'death'
            "#,
        )
        .bind(subject_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert!(
            cand_deaths >= 2,
            "contradictory death years must both exist as candidates, got {cand_deaths}"
        );

        let sourced: i64 = sqlx::query_scalar(
            r#"
            SELECT COUNT(*)::bigint
            FROM canonical_events ce
            JOIN event_candidates ec ON ec.id = ce.event_candidate_id
            JOIN document_fragments f ON f.id = ec.fragment_id
            WHERE ce.entity_id = $1 AND ce.pipeline = 'quality' AND ce.is_active
            "#,
        )
        .bind(subject_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert!(sourced >= 2);

        let occs: Vec<(String, i64)> = sqlx::query_as(
            r#"
            SELECT occurrence_key, COUNT(*)::bigint
            FROM canonical_events
            WHERE entity_id = $1 AND pipeline = 'quality' AND is_active
              AND occurrence_key IS NOT NULL
            GROUP BY occurrence_key
            HAVING COUNT(*) > 1
            "#,
        )
        .bind(subject_id)
        .fetch_all(&pool)
        .await
        .unwrap();
        assert!(occs.is_empty(), "duplicate occurrence_key rows: {occs:?}");

        let again = run_dump_canonicalize(&config, &opts).await.unwrap();
        assert_eq!(again.events_created, 0);
    }
}
