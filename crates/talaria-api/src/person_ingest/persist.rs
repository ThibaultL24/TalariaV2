// crates/talaria-api/src/person_ingest/persist.rs
//! Always write event_candidates; canonical_events only on Accept + auto-attribution.

use serde_json::json;
use talaria_quality::{
    concise_event_title, event_type_is_map_locus,
    occurrence_key_for_event, publishes_to_public_corpus, quote_may_attest, read_clause_time,
    start_time_from_typed, time_to_json, AttributionMatch, CandidateStatus, ClauseTime, GateContext,
    GateDecision, GroundedItem, RejectionCode, TypedTime,
};
use talaria_sources::is_plausible_place_label;
use talaria_store::{
    eligibility_for_event_type, enrich_person_event_place_if_empty,
    find_active_person_singleton_event, find_nearby_visit_heritage, find_recognized_person_event,
    insert_claim, insert_claim_evidence, insert_person_candidate, insert_person_event,
    insert_person_quote_evidence, mark_candidate_assembled, ClaimInsert, PersonCandidateInsert,
    PersonEventInsert,
};
use uuid::Uuid;

use super::gating;
use super::typing;

pub enum PersistOutcome {
    Canonical {
        event_id: Uuid,
        inserted: bool,
    },
    CandidateOnly {
        candidate_id: Uuid,
        status: CandidateStatus,
    },
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
) -> anyhow::Result<Option<Uuid>> {
    // Exact reading only. A shared title, or a key changed by a refined date or place,
    // stays a separate fact until an explicit merge.
    find_recognized_person_event(pool, entity_id, occurrence_key).await
}

fn attribution_label(m: AttributionMatch) -> &'static str {
    match m {
        AttributionMatch::DirectNameMatch => "direct_name_match",
        AttributionMatch::AliasMatch => "alias_match",
        AttributionMatch::TitleSubjectMatch => "title_subject_match",
        AttributionMatch::StructuredParticipantMatch => "structured_participant_match",
        AttributionMatch::FollowedMilitaryAction => "followed_military_action",
        AttributionMatch::MentionedNotActing => "mentioned_not_acting",
        AttributionMatch::CoreferenceMatch => "coreference_match",
        AttributionMatch::Comparison => "subject_used_as_comparison",
        AttributionMatch::Unattributed => "unattributed",
    }
}

fn place_role(item: &GroundedItem) -> talaria_ai::PlaceRole {
    let label = item.place_surface.clone().unwrap_or_default();
    let record = talaria_ai::HistoricalRecord {
        event_id: String::new(),
        title: item.summary.clone(),
        event_type: item.event_type.clone(),
        anchors: vec![label.clone()],
        stored_year: item.year,
        stored_precision: "year".into(),
        place_label: label,
        place_qid: None,
        lat: None,
        lon: None,
        passages: vec![talaria_ai::CitedPassage {
            quote: item.quoted_text.clone(),
            url: String::new(),
        }],
        identities: Vec::new(),
        occurrence_key: None,
    };
    talaria_ai::classify_place_role(&record)
}

fn life_singleton_type(event_type: &str) -> bool {
    matches!(event_type, "birth" | "death")
}

async fn hold_bibliographic_candidate(
    pool: &sqlx::PgPool,
    candidate_id: Uuid,
) -> anyhow::Result<()> {
    sqlx::query(
        "UPDATE event_candidates SET status = 'needs_review' WHERE id = $1 AND status <> 'assembled'",
    )
    .bind(candidate_id)
    .execute(pool)
    .await?;
    Ok(())
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
    if !quote_may_attest(&item.quoted_text, item.place_surface.as_deref()) {
        // The candidate stays out of event_evidence. A catalog title is not a passage.
        hold_bibliographic_candidate(pool, candidate_id).await?;
        return Ok(PersistOutcome::CandidateOnly {
            candidate_id,
            status: CandidateStatus::NeedsReview,
        });
    }
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
    time_reading: &str,
    place_reading: &str,
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
            judgment_json: json!({
                "attribution": attribution_label(attribution),
                "time_reading": time_reading,
                "place_reading": place_reading,
            }),
            raw_document_id,
        },
    )
    .await?;

    let accept_canonical = publishes_to_public_corpus(decision, attribution);
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
    let title = concise_event_title(
        subject,
        &item.event_type,
        item.year,
        item.place_surface.as_deref(),
        Some(item.quoted_text.as_str()),
        Some(item.summary.as_str()),
    );

    if let Some(existing) = find_existing_person_event(pool, entity_id, occurrence_key).await? {
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

    let (timeline_eligible, visit_eligible) = eligibility_for_event_type(&item.event_type);
    if visit_eligible && !timeline_eligible {
        if let Some((lat, lon)) = coords {
            if let Some(existing) =
                find_nearby_visit_heritage(pool, entity_id, &item.event_type, lat, lon, 80.0)
                    .await?
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
        Err(err)
            if life_singleton_type(&item.event_type) && is_singleton_unique_violation(&err) =>
        {
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

fn event_time(
    item: &GroundedItem,
    ctx: &GateContext,
    meta: &PersistMeta<'_>,
) -> (TypedTime, &'static str) {
    match read_clause_time(&item.quoted_text) {
        ClauseTime::Exact {
            year,
            month,
            day,
            surface,
        } => {
            let reading = if day.is_some() {
                "explicit_day"
            } else if month.is_some() {
                "explicit_month"
            } else {
                "explicit_year"
            };
            (
                TypedTime::Exact {
                    year,
                    month,
                    day,
                    surface: Some(surface),
                },
                reading,
            )
        }
        ClauseTime::Range {
            start_year,
            end_year,
            start_month,
            start_day,
            end_month,
            end_day,
            surface,
        } => (
            TypedTime::Range {
                start_year,
                end_year,
                start_month,
                start_day,
                end_month,
                end_day,
                surface: Some(surface),
            },
            "range",
        ),
        ClauseTime::Ambiguous { reason } => (
            TypedTime::Unknown {
                surface: Some(reason.to_string()),
            },
            reason,
        ),
        ClauseTime::Missing if meta.structured_source => {
            (typing::typed_time_from_year(item.year), "structured_year")
        }
        ClauseTime::Missing => {
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
            let time =
                typing::typed_time_from_year_with_context(item.year, &item.event_type, &infer_ctx);
            let reading = match &time {
                TypedTime::Approx { .. } => "inferred_year",
                TypedTime::Exact { .. } => "explicit_year",
                _ => "missing",
            };
            (time, reading)
        }
    }
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

    let (time, time_reading) = event_time(item, ctx, &meta);
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
    let role = place_role(item);
    let point = talaria_sources::event_place_point(
        role.as_str(),
        item.place_surface.as_deref().unwrap_or(""),
        grounding.coords.or(meta.coords),
    );
    let coords = point.lat.zip(point.lon);
    let place_qid = if role == talaria_ai::PlaceRole::Locus {
        point.qid.clone().or(grounding.identity_qid.clone())
    } else {
        None
    };

    // Persist place resolution to audit trail if identity was resolved
    if let Some(ref identity) = grounding.identity {
        if let Err(e) = typing::persist_place_resolution(
            pool,
            item.place_surface.as_deref().unwrap_or_default(),
            identity,
            coords,
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
        coords,
        place_qid.as_deref(),
        meta.source_locator,
        time_reading,
        point.reason,
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

    #[sqlx::test(migrations = "../../migrations")]
    async fn a_tenth_of_may_1796_keeps_day_precision_in_the_api_shape(pool: sqlx::PgPool) {
        let entity_id = talaria_store::ensure_subject_entity(&pool, "Q9001796", "Napoleon")
            .await
            .unwrap();
        let quote = "Napoleon fought the Battle of Lodi on 10 May 1796 at Lodi.";
        let raw_id = talaria_store::upsert_raw_wikipedia_document(
            &pool,
            "https://en.wikipedia.org/wiki/Battle_of_Lodi",
            "Battle of Lodi",
            "en",
            quote,
        )
        .await
        .unwrap();
        let item = GroundedItem {
            lane: talaria_quality::Lane::Fact,
            event_type: "battle".into(),
            role: "direct".into(),
            year: Some(1796),
            place_surface: Some("Lodi".into()),
            summary: quote.into(),
            quoted_text: quote.into(),
            confidence: 0.9,
        };
        let mut ctx = GateContext {
            subject_birth_year: Some(1769),
            subject_death_year: Some(1821),
            has_active_birth: true,
            has_active_death: false,
            fingerprint_exists: false,
            cross_clause_join_detected: false,
            place_entity_kind: None,
            subject_confirmed_alive: false,
        };
        let outcome = persist_fact_item(
            &pool,
            entity_id,
            "Napoleon",
            &item,
            &mut ctx,
            PersistMeta {
                raw_document_id: raw_id,
                coords: Some((45.314, 9.503)),
                primary_object: Some("Battle of Lodi"),
                source_locator: "https://en.wikipedia.org/wiki/Battle_of_Lodi",
                page_title: "Napoleon",
                from_followed_page: false,
                structured_source: false,
                military_subject: true,
                aliases: &[],
                document_text: None,
                section_heading: None,
            },
        )
        .await
        .unwrap();
        let PersistOutcome::Canonical { event_id, .. } = outcome else {
            panic!("the dated battle must be published");
        };
        let (time_json, title, lat): (serde_json::Value, String, Option<f64>) = sqlx::query_as(
            "SELECT time_json, title, ST_Y(geom::geometry) FROM canonical_events WHERE id = $1",
        )
        .bind(event_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        let api = json!({ "time": time_json, "title": title });
        assert_eq!(api["time"]["kind"], "exact");
        assert_eq!(api["time"]["precision"], "day");
        assert!(api["time"]["start"]
            .as_str()
            .unwrap()
            .starts_with("1796-05-10"));
        assert_eq!(api["title"], "Battle of Lodi (1796)");
        assert!((lat.unwrap() - 45.314).abs() < 0.01);

        let mondovi = "Napoleon fought the Battle of Mondovì on 21 April 1796.";
        let mondovi_doc = talaria_store::upsert_raw_wikipedia_document(
            &pool,
            "https://en.wikipedia.org/wiki/Battle_of_Mondovi",
            "Battle of Mondovì",
            "en",
            mondovi,
        )
        .await
        .unwrap();
        let item = GroundedItem {
            place_surface: Some("Mondovì".into()),
            summary: mondovi.into(),
            quoted_text: mondovi.into(),
            ..item
        };
        let outcome = persist_fact_item(
            &pool,
            entity_id,
            "Napoleon",
            &item,
            &mut ctx,
            PersistMeta {
                raw_document_id: mondovi_doc,
                coords: Some((59.60, 29.67)),
                primary_object: Some("Battle of Mondovì"),
                source_locator: "https://en.wikipedia.org/wiki/Battle_of_Mondovi",
                page_title: "Napoleon",
                from_followed_page: false,
                structured_source: false,
                military_subject: true,
                aliases: &[],
                document_text: None,
                section_heading: None,
            },
        )
        .await
        .unwrap();
        let PersistOutcome::Canonical { event_id, .. } = outcome else {
            panic!("Mondovì must stay a battle, with the Italian point");
        };
        let (lat, lon): (f64, f64) = sqlx::query_as(
            "SELECT ST_Y(geom::geometry), ST_X(geom::geometry) FROM canonical_events WHERE id = $1",
        )
        .bind(event_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert!((lat - 44.3833).abs() < 0.01, "lat {lat}");
        assert!((lon - 7.8167).abs() < 0.01, "lon {lon}");
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn reviewed_italian_events_persist_once_on_a_fresh_database(pool: sqlx::PgPool) {
        let entity_id = talaria_store::ensure_subject_entity(&pool, "Q517", "Napoleon")
            .await
            .unwrap();
        let mut ctx = GateContext {
            subject_birth_year: Some(1769),
            subject_death_year: Some(1821),
            has_active_birth: true,
            has_active_death: true,
            fingerprint_exists: false,
            cross_clause_join_detected: false,
            place_entity_kind: None,
            subject_confirmed_alive: false,
        };
        let cases = [
            (
                "https://en.wikipedia.org/wiki/Battle_of_Arcole",
                "Battle of Arcole",
                "The Battle of Arcole (15–17 November 1796) was fought southeast of Verona. The battle saw a bold maneuver by Napoleon Bonaparte's French Army of Italy.",
                "battle",
                "Arcole",
                "Battle of Arcole",
                true,
            ),
            (
                "https://en.wikipedia.org/wiki/Battle_of_Castiglione",
                "Battle of Castiglione",
                "The Battle of Castiglione saw the French Army of Italy under General Napoleon Bonaparte attack the Austrian army on 5 August 1796.",
                "battle",
                "Castiglione",
                "Battle of Castiglione",
                true,
            ),
            (
                "https://en.wikipedia.org/wiki/Siege_of_Mantua_(1796%E2%80%931797)",
                "Siege of Mantua",
                "The siege of Mantua lasted from 4 June 1796 to 2 February 1797 under the overall command of Napoleon Bonaparte.",
                "siege",
                "Mantua",
                "Siege of Mantua",
                true,
            ),
            (
                "https://en.wikipedia.org/wiki/Armistice_of_Cherasco",
                "Armistice of Cherasco",
                "The Armistice of Cherasco was a truce signed at Cherasco on 28 April 1796 between Victor Amadeus III of Sardinia and Napoleon Bonaparte.",
                "diplomatic",
                "Cherasco",
                "Armistice of Cherasco",
                true,
            ),
            (
                "https://en.wikipedia.org/wiki/Second_Battle_of_Dego",
                "Second Battle of Dego",
                "The Second Battle of Dego was fought on 14 and 15 April 1796 near Dego.",
                "battle",
                "Dego",
                "Second Battle of Dego",
                false,
            ),
            (
                "https://en.wikipedia.org/wiki/Treaty_of_Campo_Formio",
                "Treaty of Campo Formio",
                "The Treaty of Campo Formio was a peace treaty signed on 17 October 1797 between the French Republic and Austria.",
                "treaty",
                "Campo Formio",
                "Treaty of Campo Formio",
                false,
            ),
            (
                "https://en.wikipedia.org/wiki/Battle_of_Ceva",
                "Battle of Ceva",
                "The Battle of Ceva was fought by the French under General Pierre Augereau.",
                "battle",
                "Ceva",
                "Battle of Ceva",
                false,
            ),
            (
                "https://en.wikipedia.org/wiki/Battle_of_Tolentino",
                "Battle of Tolentino",
                "The Battle of Tolentino was fought by the Napoleonic King of Naples Joachim Murat.",
                "battle",
                "Tolentino",
                "Battle of Tolentino",
                false,
            ),
        ];
        let before: i64 =
            sqlx::query_scalar("SELECT count(*)::bigint FROM canonical_events WHERE entity_id = $1")
                .bind(entity_id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(before, 0);

        for (url, title, quote, event_type, place, object, publish) in cases {
            let raw_id = talaria_store::upsert_raw_wikipedia_document(
                &pool, url, title, "en", quote,
            )
            .await
            .unwrap();
            let item = GroundedItem {
                lane: talaria_quality::Lane::Fact,
                event_type: event_type.into(),
                role: if event_type == "diplomatic" || event_type == "treaty" {
                    "signed".into()
                } else {
                    "commanded".into()
                },
                year: Some(1796),
                place_surface: Some(place.into()),
                summary: quote.into(),
                quoted_text: quote.into(),
                confidence: 0.9,
            };
            let outcome = persist_fact_item(
                &pool,
                entity_id,
                "Napoleon",
                &item,
                &mut ctx,
                PersistMeta {
                    raw_document_id: raw_id,
                    coords: None,
                    primary_object: Some(object),
                    source_locator: url,
                    page_title: title,
                    from_followed_page: true,
                    structured_source: false,
                    military_subject: true,
                    aliases: &[],
                    document_text: None,
                    section_heading: None,
                },
            )
            .await
            .unwrap();
            assert_eq!(
                matches!(outcome, PersistOutcome::Canonical { inserted: true, .. }),
                publish,
                "{title}"
            );
        }

        let after: i64 =
            sqlx::query_scalar("SELECT count(*)::bigint FROM canonical_events WHERE entity_id = $1 AND is_active")
                .bind(entity_id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(after, 4);
        let mantua: serde_json::Value = sqlx::query_scalar(
            "SELECT time_json FROM canonical_events WHERE place_label = 'Mantua' AND entity_id = $1",
        )
        .bind(entity_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(mantua["kind"], "range");
        assert_eq!(mantua["start"], "1796-06-04");
        assert_eq!(mantua["end"], "1797-02-02");
        assert_eq!(mantua["precision"], "day");
        let castiglione: serde_json::Value = sqlx::query_scalar(
            "SELECT time_json FROM canonical_events WHERE place_label = 'Castiglione' AND entity_id = $1",
        )
        .bind(entity_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(castiglione["kind"], "exact");
        assert_eq!(castiglione["start"], "1796-08-05");
        let held_back: i64 = sqlx::query_scalar(
            "SELECT count(*)::bigint FROM canonical_events WHERE entity_id = $1 AND place_label IN ('Dego', 'Campo Formio', 'Ceva', 'Tolentino')",
        )
        .bind(entity_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(held_back, 0);
        let unmapped: i64 = sqlx::query_scalar(
            "SELECT count(*)::bigint FROM canonical_events WHERE entity_id = $1 AND map_eligible AND geom IS NULL",
        )
        .bind(entity_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(unmapped, 0);

        for (url, title, quote, event_type, place, object, _) in cases {
            let raw_id = talaria_store::upsert_raw_wikipedia_document(
                &pool, url, title, "en", quote,
            )
            .await
            .unwrap();
            let item = GroundedItem {
                lane: talaria_quality::Lane::Fact,
                event_type: event_type.into(),
                role: if event_type == "diplomatic" || event_type == "treaty" {
                    "signed".into()
                } else {
                    "commanded".into()
                },
                year: Some(1796),
                place_surface: Some(place.into()),
                summary: quote.into(),
                quoted_text: quote.into(),
                confidence: 0.9,
            };
            persist_fact_item(
                &pool,
                entity_id,
                "Napoleon",
                &item,
                &mut ctx,
                PersistMeta {
                    raw_document_id: raw_id,
                    coords: None,
                    primary_object: Some(object),
                    source_locator: url,
                    page_title: title,
                    from_followed_page: true,
                    structured_source: false,
                    military_subject: true,
                    aliases: &[],
                    document_text: None,
                    section_heading: None,
                },
            )
            .await
            .unwrap();
        }

        let replayed: i64 =
            sqlx::query_scalar("SELECT count(*)::bigint FROM canonical_events WHERE entity_id = $1 AND is_active")
                .bind(entity_id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(replayed, after);
        let evidence: i64 = sqlx::query_scalar(
            "SELECT count(*)::bigint FROM event_evidence ee JOIN canonical_events ce ON ce.id = ee.canonical_event_id WHERE ce.entity_id = $1",
        )
        .bind(entity_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(evidence, 4);
        let logical: i64 = sqlx::query_scalar(
            "SELECT count(DISTINCT logical_event_id)::bigint FROM canonical_events WHERE entity_id = $1 AND is_active",
        )
        .bind(entity_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(logical, 4);
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn an_absent_person_keeps_one_identity_and_only_accepted_facts(pool: sqlx::PgPool) {
        let first = talaria_store::ensure_subject_entity(&pool, "Q868", "Aristotle")
            .await
            .unwrap();
        let again = talaria_store::ensure_subject_entity(&pool, "Q868", "Aristotle")
            .await
            .unwrap();
        assert_eq!(first, again);
        let other = talaria_store::ensure_subject_entity(&pool, "Q535", "Victor Hugo")
            .await
            .unwrap();
        assert_ne!(first, other);

        let mut ctx = GateContext {
            subject_birth_year: Some(-384),
            subject_death_year: Some(-322),
            has_active_birth: false,
            has_active_death: false,
            fingerprint_exists: false,
            cross_clause_join_detected: false,
            place_entity_kind: None,
            subject_confirmed_alive: false,
        };
        let birth = "Aristotle was born in Stagira in 384 BC.";
        let birth_doc = talaria_store::upsert_raw_wikipedia_document(
            &pool,
            "https://en.wikipedia.org/wiki/Aristotle",
            "Aristotle",
            "en",
            birth,
        )
        .await
        .unwrap();
        let birth_item = GroundedItem {
            lane: talaria_quality::Lane::Fact,
            event_type: "birth".into(),
            role: "born".into(),
            year: None,
            place_surface: Some("Stagira".into()),
            summary: birth.into(),
            quoted_text: birth.into(),
            confidence: 0.9,
        };
        let outcome = persist_fact_item(
            &pool,
            first,
            "Aristotle",
            &birth_item,
            &mut ctx,
            PersistMeta {
                raw_document_id: birth_doc,
                coords: None,
                primary_object: Some("birth"),
                source_locator: "https://en.wikipedia.org/wiki/Aristotle",
                page_title: "Aristotle",
                from_followed_page: false,
                structured_source: false,
                military_subject: false,
                aliases: &[],
                document_text: None,
                section_heading: None,
            },
        )
        .await
        .unwrap();
        assert!(matches!(outcome, PersistOutcome::Canonical { inserted: true, .. }));
        let time: serde_json::Value = sqlx::query_scalar(
            "SELECT time_json FROM canonical_events WHERE entity_id = $1 AND event_type = 'birth'",
        )
        .bind(first)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(time["kind"], "exact");
        assert_eq!(time["precision"], "year");
        assert_eq!(time["start"], "-384");
        let (birth_map, birth_geom): (bool, bool) = sqlx::query_as(
            "SELECT map_eligible, geom IS NOT NULL FROM canonical_events \
             WHERE entity_id = $1 AND event_type = 'birth'",
        )
        .bind(first)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(birth_map, birth_geom);

        for index in 0..6 {
            let quote = format!("Aristotle published a treatise in Athens in {} BC.", 340 - index);
            let url = format!("https://example.test/aristotle-work-{index}");
            let doc = talaria_store::upsert_raw_wikipedia_document(
                &pool, &url, "Aristotle", "en", &quote,
            )
            .await
            .unwrap();
            let item = GroundedItem {
                event_type: "publication".into(),
                role: "published".into(),
                place_surface: Some("Athens".into()),
                summary: quote.clone(),
                quoted_text: quote,
                ..birth_item.clone()
            };
            persist_fact_item(
                &pool,
                first,
                "Aristotle",
                &item,
                &mut ctx,
                PersistMeta {
                    raw_document_id: doc,
                    coords: None,
                    primary_object: Some("treatise"),
                    source_locator: &url,
                    page_title: "Aristotle",
                    from_followed_page: false,
                    structured_source: false,
                    military_subject: false,
                    aliases: &[],
                    document_text: None,
                    section_heading: None,
                },
            )
            .await
            .unwrap();
        }
        let rows: Vec<(String, i64)> = sqlx::query_as(
            "SELECT event_type, count(*)::bigint FROM canonical_events \
             WHERE entity_id = $1 AND is_active GROUP BY event_type",
        )
        .bind(first)
        .fetch_all(&pool)
        .await
        .unwrap();
        let counts: Vec<(&str, usize)> = rows
            .iter()
            .map(|(kind, count)| (kind.as_str(), *count as usize))
            .collect();
        let suggestions = talaria_quality::suggest_chapters(&counts);
        assert_eq!(suggestions.len(), 1);
        assert_eq!(suggestions[0].slug, "works");
        assert_eq!(suggestions[0].standing, "suggestion");
        let published: i64 = sqlx::query_scalar(
            "SELECT count(*)::bigint FROM editorial_groups WHERE entity_id = $1 AND status = 'published'",
        )
        .bind(first)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(published, 0);

        ctx.subject_birth_year = Some(1802);
        ctx.subject_death_year = Some(1885);
        let hugo = "In 1855 Victor Hugo lived in Guernsey.";
        let hugo_doc = talaria_store::upsert_raw_wikipedia_document(
            &pool,
            "https://en.wikipedia.org/wiki/Victor_Hugo",
            "Victor Hugo",
            "en",
            hugo,
        )
        .await
        .unwrap();
        let hugo_item = GroundedItem {
            lane: talaria_quality::Lane::Fact,
            event_type: "residence".into(),
            role: "lived".into(),
            year: None,
            place_surface: Some("Guernsey".into()),
            summary: hugo.into(),
            quoted_text: hugo.into(),
            confidence: 0.9,
        };
        persist_fact_item(
            &pool,
            other,
            "Victor Hugo",
            &hugo_item,
            &mut ctx,
            PersistMeta {
                raw_document_id: hugo_doc,
                coords: None,
                primary_object: Some("residence"),
                source_locator: "https://en.wikipedia.org/wiki/Victor_Hugo",
                page_title: "Victor Hugo",
                from_followed_page: false,
                structured_source: false,
                military_subject: false,
                aliases: &[],
                document_text: None,
                section_heading: None,
            },
        )
        .await
        .unwrap();
        let (map_eligible, has_geom): (bool, bool) = sqlx::query_as(
            "SELECT map_eligible, geom IS NOT NULL FROM canonical_events \
             WHERE entity_id = $1 AND event_type = 'residence'",
        )
        .bind(other)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(map_eligible, has_geom);

        ctx.subject_birth_year = Some(1786);
        ctx.subject_death_year = Some(1859);
        ctx.has_active_birth = false;
        let sparse = talaria_store::ensure_subject_entity(&pool, "Q466987", "Marceline Desbordes-Valmore")
            .await
            .unwrap();
        let note = "Marceline Desbordes-Valmore was born in Douai in 1786.";
        let note_doc = talaria_store::upsert_raw_wikipedia_document(
            &pool,
            "https://fr.wikipedia.org/wiki/Marceline_Desbordes-Valmore",
            "Marceline Desbordes-Valmore",
            "fr",
            note,
        )
        .await
        .unwrap();
        let note_item = GroundedItem {
            event_type: "birth".into(),
            role: "born".into(),
            place_surface: Some("Douai".into()),
            summary: note.into(),
            quoted_text: note.into(),
            ..hugo_item
        };
        let note_outcome = persist_fact_item(
            &pool,
            sparse,
            "Marceline Desbordes-Valmore",
            &note_item,
            &mut ctx,
            PersistMeta {
                raw_document_id: note_doc,
                coords: None,
                primary_object: Some("birth"),
                source_locator: "https://fr.wikipedia.org/wiki/Marceline_Desbordes-Valmore",
                page_title: "Marceline Desbordes-Valmore",
                from_followed_page: false,
                structured_source: false,
                military_subject: false,
                aliases: &[],
                document_text: None,
                section_heading: None,
            },
        )
        .await
        .unwrap();
        assert!(matches!(note_outcome, PersistOutcome::Canonical { inserted: true, .. }));
        let sparse_counts: Vec<(String, i64)> = sqlx::query_as(
            "SELECT event_type, count(*)::bigint FROM canonical_events WHERE entity_id = $1 AND is_active GROUP BY event_type",
        )
        .bind(sparse)
        .fetch_all(&pool)
        .await
        .unwrap();
        let sparse_pairs: Vec<(&str, usize)> = sparse_counts
            .iter()
            .map(|(kind, count)| (kind.as_str(), *count as usize))
            .collect();
        assert!(talaria_quality::suggest_chapters(&sparse_pairs).is_empty());
    }

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
