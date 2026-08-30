// crates/talaria-api/src/person_ingest/persist.rs
//! Always write event_candidates; canonical_events only on Accept + auto-attribution.

use serde_json::json;
use talaria_quality::{
    auto_accept_attribution, event_type_is_map_locus, explorer_headline, occurrence_key_for_event,
    start_time_from_typed, time_to_json, AttributionMatch, CandidateStatus, GateContext,
    GateDecision, GroundedItem, TypedTime,
};
use talaria_store::{
    find_active_person_event_by_fingerprint, find_active_person_event_by_occurrence, insert_claim,
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
}

fn person_event_fingerprint(entity_id: Uuid, occurrence_key: &str) -> String {
    format!("{entity_id}|{occurrence_key}")
}

async fn find_existing_person_event(
    pool: &sqlx::PgPool,
    entity_id: Uuid,
    occurrence_key: &str,
) -> anyhow::Result<Option<Uuid>> {
    if let Some(id) =
        find_active_person_event_by_occurrence(pool, entity_id, occurrence_key).await?
    {
        return Ok(Some(id));
    }
    find_active_person_event_by_fingerprint(
        pool,
        &person_event_fingerprint(entity_id, occurrence_key),
    )
    .await
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
    if !accept_canonical {
        return Ok(PersistOutcome::CandidateOnly {
            candidate_id,
            status,
        });
    }

    if let Some(existing) = find_existing_person_event(pool, entity_id, occurrence_key).await? {
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
        return Ok(PersistOutcome::Canonical {
            event_id: existing,
            inserted: false,
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
    let event_id = insert_person_event(
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
        },
    )
    .await?;
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
    let time = typing::typed_time_from_year(item.year);
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
    let coords = typing::resolve_coords(item.place_surface.as_deref(), meta.coords).await;
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
        coords,
        meta.source_locator,
    )
    .await?;
    if matches!(
        outcome,
        PersistOutcome::Canonical {
            inserted: true,
            ..
        }
    ) {
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
