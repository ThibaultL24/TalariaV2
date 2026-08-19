// crates/talaria-api/src/judge.rs
use talaria_core::AppConfig;
use talaria_judge::{judge_candidate, CandidateInput, JudgeLabel};
use talaria_store::{
    connect, find_existing_event, insert_canonical_event, insert_event_evidence, insert_judgment,
    list_pending_candidates, run_migrations, update_candidate_status, upsert_entity_surface,
    CanonicalEventInsert,
};

const JUDGE_KIND: &str = "rules:v1";

pub async fn run_judge_candidates(config: &AppConfig, limit: i64) -> anyhow::Result<()> {
    let pool = connect(config).await?;
    run_migrations(&pool).await?;

    let candidate_limit = if limit > 0 { limit } else { i64::MAX };
    let candidates = list_pending_candidates(&pool, candidate_limit).await?;

    tracing::info!(candidates = candidates.len(), "judging phrase candidates");

    let mut accepted = 0usize;
    let mut rejected = 0usize;
    let mut events_created = 0usize;

    for candidate in candidates {
        let entity_id = match candidate.entity_id {
            Some(id) => id,
            None => {
                upsert_entity_surface(&pool, &config.wiki_lang, &candidate.person_surface).await?
            }
        };

        let input = CandidateInput {
            person_surface: candidate.person_surface.clone(),
            time_surface: candidate.time_surface.clone(),
            place_surface: candidate.place_surface.clone(),
            verb_pivot: candidate.verb_pivot.clone(),
            sentence_text: candidate.sentence_text.clone(),
        };

        let verdict = judge_candidate(&input);
        let label = match verdict.label {
            JudgeLabel::Accept => "accept",
            JudgeLabel::Reject => "reject",
        };

        insert_judgment(
            &pool,
            candidate.id,
            JUDGE_KIND,
            verdict.score,
            label,
            serde_json::json!({
                "reason": verdict.reason,
                "event_type": verdict.event_type,
                "epistemic_status": verdict.epistemic_status,
            }),
        )
        .await?;

        if verdict.label == JudgeLabel::Accept {
            let event_id = match find_existing_event(
                &pool,
                entity_id,
                &verdict.event_type,
                verdict.start_time,
                verdict.place_label.as_deref(),
            )
            .await?
            {
                Some(id) => id,
                None => {
                    let id = insert_canonical_event(
                        &pool,
                        &CanonicalEventInsert {
                            entity_id,
                            event_type: verdict.event_type.clone(),
                            epistemic_status: verdict.epistemic_status.clone(),
                            title: verdict.title.clone(),
                            summary: Some(verdict.summary.clone()),
                            start_time: verdict.start_time,
                            time_json: verdict.time_json.clone(),
                            place_label: verdict.place_label.clone(),
                            lat: verdict.lat,
                            lon: verdict.lon,
                            confidence: verdict.confidence,
                            map_eligible: verdict.map_eligible,
                        },
                    )
                    .await?;
                    events_created += 1;
                    id
                }
            };

            insert_event_evidence(
                &pool,
                event_id,
                candidate.sentence_id,
                candidate.id,
                &candidate.sentence_text,
                verdict.confidence,
            )
            .await?;

            update_candidate_status(&pool, candidate.id, "accepted").await?;
            accepted += 1;
        } else {
            update_candidate_status(&pool, candidate.id, "rejected").await?;
            rejected += 1;
        }
    }

    tracing::info!(accepted, rejected, events_created, "judging complete");
    Ok(())
}
