// crates/talaria-api/src/person_ingest/persist.rs
//! Always write event_candidates; canonical_events only on Accept + auto-attribution.

use serde_json::json;
use talaria_quality::{
    auto_accept_attribution, event_type_is_map_locus, explorer_headline, occurrence_key_for_event,
    start_time_from_typed, time_to_json, AttributionMatch, CandidateStatus, GateContext,
    GateDecision, GroundedItem, RejectionCode, TypedTime,
};
use talaria_sources::is_plausible_place_label;
use talaria_store::{
    enrich_person_event_place_if_empty, find_active_person_event_by_fingerprint,
    find_active_person_event_by_occurrence, find_active_person_event_by_title,
    find_active_person_singleton_event, insert_claim,
    insert_claim_evidence, insert_person_candidate, insert_person_event,
    insert_person_quote_evidence, mark_candidate_assembled, ClaimInsert, PersonCandidateInsert,
    PersonEventInsert,
};
use uuid::Uuid;

use super::gating;
use super::typing;

pub enum PersistOutcome {
    Canonical { event_id: Uuid, inserted: bool },
    CandidateOnly { candidate_id: Uuid, status: CandidateStatus },
}

pub struct PersistMeta<'a> {
    pub raw_document_id: Uuid,
    pub coords: Option<(f64, f64)>,
    pub primary_object: Option<&'a str>,
    pub source_locator: &'a str,
    pub page_title: &'a str,
    pub from_followed_page: bool,
    pub structured_source: bool,
    pub military_subject: bool,
    pub aliases: &'a [String],
    /// Full document text for paragraph context in year inference.
    pub document_text: Option<&'a str>,
    /// Section heading for year inference (if extracted from Wikipedia sections).
    pub section_heading: Option<&'a str>,
}

fn person_event_fingerprint(entity_id: Uuid, occurrence_key: &str) -> String {
    format!("{entity_id}|{occurrence_key}")
}

async fn find_existing_person_event(
    pool: &sqlx::PgPool,
    entity_id: Uuid,
    occurrence_key: &str,
    title: &str,
) -> anyhow::Result<Option<Uuid>> {
    if let Some(id) =
        find_active_person_event_by_occurrence(pool, entity_id, occurrence_key).await?
    {
        return Ok(Some(id));
    }
    if let Some(id) = find_active_person_event_by_fingerprint(
        pool,
        &person_event_fingerprint(entity_id, occurrence_key),
    )
    .await?
    {
        return Ok(Some(id));
    }
    // Re-ingest often changes place_label (QID vs label) and thus occurrence_key while
    // the explorer title stays identical — collapse on display title.
    find_active_person_event_by_title(pool, entity_id, title).await
}

fn attribution_label(m: AttributionMatch) -> &'static str {
    match m {
        AttributionMatch::DirectNameMatch => "direct_name_match",
        AttributionMatch::AliasMatch => "alias_match",
        AttributionMatch::TitleSubjectMatch => "title_subject_match",
        AttributionMatch::StructuredParticipantMatch => "structured_participant_match",
        AttributionMatch::FollowedMilitaryAction => "followed_military_action",
        AttributionMatch::CoreferenceMatch => "coreference_match",
        AttributionMatch::Unattributed => "unattributed",
    }
}

fn life_singleton_type(event_type: &str) -> bool {
    matches!(event_type, "birth" | "death")
}

async fn attach_evidence_to_existing(
    pool: &sqlx::PgPool,
    existing: Uuid,
    candidate_id: Uuid,
    item: &GroundedItem,
    raw_document_id: Uuid,
    coords: Option<(f64, f64)>,
    place_identity_qid: Option<&str>,
    source_locator: &str,
) -> anyhow::Result<PersistOutcome> {
    if let Some(place) = item
        .place_surface
        .as_deref()
        .map(str::trim)
        .filter(|p| !p.is_empty() && is_plausible_place_label(p))
    {
        let map_ok = coords.is_some() && event_type_is_map_locus(&item.event_type);
        let _ = enrich_person_event_place_if_empty(
            pool,
            existing,
            place,
            coords.map(|c| c.0),
            coords.map(|c| c.1),
            place_identity_qid,
            map_ok,
        )
        .await?;
    }
    insert_person_quote_evidence(
        pool,
        existing,
        &item.quoted_text,
        Some(raw_document_id),
        item.confidence,
        source_locator,
    )
    .await?;
    mark_candidate_assembled(pool, candidate_id, existing).await?;
    Ok(PersistOutcome::Canonical {
        event_id: existing,
        inserted: false,
    })
}

fn is_singleton_unique_violation(err: &anyhow::Error) -> bool {
    let msg = format!("{err:#}");
    msg.contains("uq_canonical_active_singleton_birth_death")
}

pub async fn persist_gated_item(
    pool: &sqlx::PgPool,
    entity_id: Uuid,
    subject: &str,
    item: &GroundedItem,
    decision: &GateDecision,
    attribution: AttributionMatch,
    raw_document_id: Uuid,
    occurrence_key: &str,
    time: &TypedTime,
    coords: Option<(f64, f64)>,
    place_identity_qid: Option<&str>,
    source_locator: &str,
) -> anyhow::Result<PersistOutcome> {
    let fingerprint = gating::fingerprint_for(subject, item, time, raw_document_id);
    let status = decision.status();
    let candidate_id = insert_person_candidate(
        pool,
        &PersonCandidateInsert {
            subject_surface: subject.to_string(),
            subject_entity_id: entity_id,
            event_type: item.event_type.clone(),
            predicate: item.role.clone(),
            time_json: time_to_json(time),
            place_label: item.place_surface.clone(),
            evidence_ptrs: json!([{
                "quoted_text": item.quoted_text,
                "source_locator": source_locator,
            }]),
            extractor_version: "person_ingest:v1".into(),
            fingerprint,
            occurrence_key: occurrence_key.to_string(),
            primary_object: None,
            action_role: Some(item.role.clone()),
            status: status.as_str().to_string(),
            rejection_codes: decision.codes(),
            judgment_json: json!({ "attribution": attribution_label(attribution) }),
            raw_document_id,
        },
    )
    .await?;

    let accept_canonical =
        matches!(decision, GateDecision::Accept) && auto_accept_attribution(attribution);
    let singleton_reject = matches!(decision, GateDecision::Reject(ref codes)
        if codes.contains(&RejectionCode::SingletonCardinalityViolation));

    if life_singleton_type(&item.event_type) && (accept_canonical || singleton_reject) {
        if let Some(existing) =
            find_active_person_singleton_event(pool, entity_id, &item.event_type).await?
        {
            return attach_evidence_to_existing(
                pool,
                existing,
                candidate_id,
                item,
                raw_document_id,
                coords,
                place_identity_qid,
                source_locator,
            )
            .await;
        }
    }

    if !accept_canonical {
        return Ok(PersistOutcome::CandidateOnly {
            candidate_id,
            status,
        });
    }

    let place_label = item.place_surface.clone();
    let map_eligible = coords.is_some() && event_type_is_map_locus(&item.event_type);
    let title = explorer_headline(
        subject,
        &item.event_type,
        item.year,
        item.place_surface.as_deref(),
        Some(item.quoted_text.as_str()),
        Some(item.summary.as_str()),
    );

    if let Some(existing) =
        find_existing_person_event(pool, entity_id, occurrence_key, &title).await?
    {
        return attach_evidence_to_existing(
            pool,
            existing,
            candidate_id,
            item,
            raw_document_id,
            coords,
            place_identity_qid,
            source_locator,
        )
        .await;
    }

    let inserted = insert_person_event(
        pool,
        &PersonEventInsert {
            entity_id,
            event_type: item.event_type.clone(),
            epistemic_status: "attested".into(),
            title,
            summary: Some(item.summary.clone()),
            start_time: start_time_from_typed(time),
            time_json: time_to_json(time),
            place_label: place_label.clone(),
            lat: coords.map(|c| c.0),
            lon: coords.map(|c| c.1),
            confidence: item.confidence,
            map_eligible,
            fingerprint: person_event_fingerprint(entity_id, occurrence_key),
            occurrence_key: occurrence_key.to_string(),
            occurrence_stem: None,
            predicate: item.role.clone(),
            place_identity_qid: place_identity_qid.map(String::from),
        },
    )
    .await;
    let event_id = match inserted {
        Ok(id) => id,
        Err(err) if life_singleton_type(&item.event_type) && is_singleton_unique_violation(&err) => {
            if let Some(existing) =
                find_active_person_singleton_event(pool, entity_id, &item.event_type).await?
            {
                return attach_evidence_to_existing(
                    pool,
                    existing,
                    candidate_id,
                    item,
                    raw_document_id,
                    coords,
                    place_identity_qid,
                    source_locator,
                )
                .await;
            }
            return Err(err);
        }
        Err(err) => return Err(err),
    };
    insert_person_quote_evidence(
        pool,
        event_id,
        &item.quoted_text,
        Some(raw_document_id),
        item.confidence,
        source_locator,
    )
    .await?;
    mark_candidate_assembled(pool, candidate_id, event_id).await?;
    Ok(PersistOutcome::Canonical {
        event_id,
        inserted: true,
    })
}

pub async fn persist_fact_item(
    pool: &sqlx::PgPool,
    entity_id: Uuid,
    subject: &str,
    item: &GroundedItem,
    ctx: &mut GateContext,
    meta: PersistMeta<'_>,
) -> anyhow::Result<PersistOutcome> {
    let clean_place = item
        .place_surface
        .as_deref()
        .filter(|p| is_plausible_place_label(p))
        .map(str::to_string);
    let mut item = item.clone();
    item.place_surface = clean_place;
    let item = &item;

    let time = if item.year.is_none() && !meta.structured_source {
        let local = meta
            .document_text
            .map(|doc| talaria_quality::local_context_window(doc, &item.quoted_text, 400));
        let infer_ctx = typing::YearInferenceContext {
            clause_text: &item.quoted_text,
            paragraph_context: local,
            section_heading: meta.section_heading,
            birth_year: ctx.subject_birth_year,
            death_year: ctx.subject_death_year,
        };
        typing::typed_time_from_year_with_context(item.year, &item.event_type, &infer_ctx)
    } else {
        typing::typed_time_from_year(item.year)
    };
    let occ = occurrence_key_for_event(
        subject,
        &item.event_type,
        &item.role,
        &time,
        item.place_surface.as_deref(),
        meta.primary_object,
    );
    let fp = gating::fingerprint_for(subject, item, &time, meta.raw_document_id);
    let candidate = gating::event_candidate_from_item(entity_id, subject, item, &time, &fp);
    let attribution = gating::classify_item(
        subject,
        meta.aliases,
        item,
        meta.page_title,
        meta.from_followed_page,
        meta.structured_source,
        meta.military_subject,
    );
    let decision = gating::judge_item(&candidate, ctx, attribution);

    // Full place grounding: identity resolution (TGN/WHG) + geocoding
    let grounding = typing::ground_place_full(item.place_surface.as_deref(), meta.coords).await;

    // Persist place resolution to audit trail if identity was resolved
    if let Some(ref identity) = grounding.identity {
        if let Err(e) = typing::persist_place_resolution(
            pool,
            item.place_surface.as_deref().unwrap_or_default(),
            identity,
            grounding.coords,
        )
        .await
        {
            tracing::warn!(error = %e, "failed to persist place resolution audit trail");
        }
    }

    let outcome = persist_gated_item(
        pool,
        entity_id,
        subject,
        item,
        &decision,
        attribution,
        meta.raw_document_id,
        &occ,
        &time,
        grounding.coords,
        grounding.identity_qid.as_deref(),
        meta.source_locator,
    )
    .await?;
    if matches!(outcome, PersistOutcome::Canonical { .. }) {
        if item.event_type == "birth" {
            ctx.has_active_birth = true;
            ctx.subject_birth_year = ctx.subject_birth_year.or(item.year);
        }
        if item.event_type == "death" {
            ctx.has_active_death = true;
            ctx.subject_death_year = ctx.subject_death_year.or(item.year);
        }
    }
    Ok(outcome)
}

pub async fn persist_debate(
    pool: &sqlx::PgPool,
    entity_id: Uuid,
    item: &GroundedItem,
    uri: &str,
) -> anyhow::Result<()> {
    let claim_id = insert_claim(
        pool,
        &ClaimInsert {
            entity_id,
            claim_kind: "controversy".into(),
            text: item.summary.clone(),
            epistemic_status: "theory".into(),
            relation_to_subject: "historiography".into(),
            event_time: None,
            place_label: item.place_surface.clone(),
            confidence: item.confidence,
            canonical_event_id: None,
            debate_type: Some("controversy".into()),
            evidence_layer: Some("llm_grounded".into()),
        },
    )
    .await?;
    insert_claim_evidence(
        pool,
        claim_id,
        "wikipedia",
        Some(uri),
        Some(item.quoted_text.as_str()),
        None,
        item.confidence,
    )
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_singleton_unique_index_name() {
        let err = anyhow::anyhow!(
            "error returned from database: duplicate key value violates unique constraint \"uq_canonical_active_singleton_birth_death\""
        );
        assert!(is_singleton_unique_violation(&err));
        assert!(!is_singleton_unique_violation(&anyhow::anyhow!(
            "duplicate key value violates unique constraint \"uq_canonical_active_occurrence\""
        )));
    }
}

