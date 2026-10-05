// crates/talaria-api/src/lot_e_reports.rs
//! Density, exploration, and connector-status reports for the offline density CLI.
//!
//! These commands read stored counts. They do not ingest, and they do not decide
//! which candidates become canonical events.

use std::path::PathBuf;

use talaria_core::AppConfig;
use talaria_sources::load_seed_titles;
use talaria_store::{connect, density_report_counts, run_migrations, upsert_entity_with_kind};

use crate::cli_helpers::open_db_for_subject;

pub fn default_napoleon_seed() -> PathBuf {
    PathBuf::from("fixtures/seeds/napoleon_wiki_titles.txt")
}

pub fn connector_status_json() -> String {
    let europeana = std::env::var("EUROPEANA_API_KEY")
        .ok()
        .filter(|k| !k.trim().is_empty())
        .map(|_| "extraction_ready")
        .unwrap_or("needs_EUROPEANA_API_KEY");
    serde_json::to_string_pretty(&serde_json::json!({
        "wikipedia": "extraction_ready",
        "wikidata": "fetch_ready",
        "wikisource": "extraction_ready",
        "commons": "extraction_ready",
        "fixture": "production_ready",
        "bnf": "extraction_ready",
        "gallica": "extraction_ready",
        "persee": "extraction_ready",
        "hal": "extraction_ready",
        "theses_fr": "extraction_ready",
        "idref": "stub",
        "sudoc": "stub",
        "archives_nationales": "stub",
        "open_library": "extraction_ready",
        "internet_archive": "extraction_ready",
        "europeana": europeana,
        "loc": "stub",
        "viaf": "metadata_only",
        "isni": "metadata_only",
        "openalex": "extraction_ready",
        "crossref": "stub",
        "note": "Executable with --live from explorer search or ingest-quality: wikipedia, wikidata, wikisource, commons, hal, persee, gallica, theses_fr, open_library, open_alex, internet_archive, bnf. Europeana needs EUROPEANA_API_KEY. VIAF/ISNI remain stubs."
    }))
    .unwrap_or_else(|_| "{}".into())
}


pub async fn run_density_report(
    config: &AppConfig,
    subject: Option<&str>,
    show_bottlenecks: bool,
    show_source_coverage: bool,
    show_unresolved_places: bool,
) -> anyhow::Result<String> {
    let pool = connect(config).await?;
    run_migrations(&pool).await?;
    let sid = if let Some(label) = subject {
        Some(upsert_entity_with_kind(&pool, &config.wiki_lang, label, "person").await?)
    } else {
        None
    };
    let counts = density_report_counts(&pool, sid).await?;

    let mut report = serde_json::json!({
        "documents_discovered": counts.documents_discovered,
        "documents_snapshotted": counts.documents_snapshotted,
        "fragments": counts.fragments,
        "candidates": counts.candidates,
        "rejected": counts.rejected,
        "needs_review": counts.needs_review,
        "claims": counts.claims,
        "accepted_events": counts.accepted_events,
        "timeline_eligible": counts.timeline_eligible,
        "map_eligible": counts.map_eligible,
        "events_without_place": counts.events_without_place,
        "multi_source_events": counts.multi_source_events,
        "targets": {
            "timeline": 500,
            "map": 500,
            "gap_timeline": 500i64.saturating_sub(counts.timeline_eligible),
            "gap_map": 500i64.saturating_sub(counts.map_eligible),
            "status": if counts.map_eligible >= 500 && counts.timeline_eligible >= 500 {
                "target_reached"
            } else {
                "target_not_reached"
            }
        }
    });

    if show_bottlenecks {
        let reasons: Vec<(String, i64)> = sqlx::query_as(
            r#"
            SELECT code, COUNT(*)::bigint FROM (
              SELECT jsonb_array_elements_text(rejection_codes) AS code
              FROM event_candidates
              WHERE status = 'rejected'
                AND ($1::uuid IS NULL OR subject_entity_id = $1)
            ) t
            GROUP BY code ORDER BY COUNT(*) DESC LIMIT 20
            "#,
        )
        .bind(sid)
        .fetch_all(&pool)
        .await
        .unwrap_or_default();
        report["bottlenecks"] = serde_json::json!({
            "rejection_codes": reasons.iter().map(|(c,n)| serde_json::json!({"code": c, "n": n})).collect::<Vec<_>>(),
            "primary": if counts.documents_snapshotted < 50 {
                "insufficient_documents"
            } else if counts.candidates > 0 && counts.accepted_events * 3 < counts.candidates {
                "gates_or_dedupe"
            } else if counts.timeline_eligible > counts.map_eligible + 10 {
                "unresolved_places"
            } else {
                "exploration_or_extractors"
            }
        });
    }

    if show_source_coverage {
        report["connectors"] = serde_json::from_str(&connector_status_json()).unwrap_or_default();
        let by_source: Vec<(String, i64)> = sqlx::query_as(
            r#"
            SELECT source_type, COUNT(*)::bigint FROM document_snapshots
            GROUP BY source_type ORDER BY COUNT(*) DESC
            "#,
        )
        .fetch_all(&pool)
        .await
        .unwrap_or_default();
        report["snapshots_by_source"] = serde_json::json!(by_source
            .iter()
            .map(|(s, n)| serde_json::json!({"source": s, "n": n}))
            .collect::<Vec<_>>());
    }

    if show_unresolved_places {
        let places: Vec<(String, i64)> = if let Some(id) = sid {
            sqlx::query_as(
                r#"
                SELECT COALESCE(place_label, '(null)'), COUNT(*)::bigint
                FROM canonical_events
                WHERE pipeline = 'person' AND is_active AND timeline_eligible AND NOT map_eligible
                  AND entity_id = $1
                GROUP BY 1 ORDER BY COUNT(*) DESC LIMIT 40
                "#,
            )
            .bind(id)
            .fetch_all(&pool)
            .await
            .unwrap_or_default()
        } else {
            Vec::new()
        };
        report["unresolved_places"] = serde_json::json!(places
            .iter()
            .map(|(l, n)| serde_json::json!({"label": l, "n": n}))
            .collect::<Vec<_>>());
    }

    Ok(serde_json::to_string_pretty(&report)?)
}

pub async fn run_exploration_report(config: &AppConfig, subject: &str) -> anyhow::Result<String> {
    let (pool, subject_id) = open_db_for_subject(config, subject, "person").await?;
    let queue: Vec<(String, i64)> = sqlx::query_as(
        r#"
        SELECT status, COUNT(*)::bigint FROM exploration_targets
        WHERE subject_entity_id = $1
        GROUP BY status
        "#,
    )
    .bind(subject_id)
    .fetch_all(&pool)
    .await
    .unwrap_or_default();

    let seed_path = default_napoleon_seed();
    let seed_n = load_seed_titles(&seed_path).map(|t| t.len()).unwrap_or(0);
    let wiki_snaps: i64 = sqlx::query_scalar(
        r#"SELECT COUNT(*)::bigint FROM document_snapshots WHERE source_type = 'wikipedia'"#,
    )
    .fetch_one(&pool)
    .await
    .unwrap_or(0);

    Ok(serde_json::to_string_pretty(&serde_json::json!({
        "subject": subject,
        "seed_titles_available": seed_n,
        "wikipedia_snapshots": wiki_snaps,
        "exploration_queue_by_status": queue.iter().map(|(s,n)| serde_json::json!({"status": s, "n": n})).collect::<Vec<_>>(),
        "note": "Lot E primarily drives exploration from fixtures/seeds/napoleon_wiki_titles.txt; exploration_targets table is ready for resume/queue."
    }))?)
}


#[cfg(test)]
mod connector_status_tests {
    use super::connector_status_json;

    #[test]
    fn connector_status_marks_wikisource_extraction_ready() {
        let v: serde_json::Value = serde_json::from_str(&connector_status_json()).unwrap();
        assert_eq!(v["wikisource"], "extraction_ready");
        assert_eq!(v["commons"], "extraction_ready");
    }
}
