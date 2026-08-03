// crates/talaria-api/src/quality.rs
//! Quality pipeline orchestration (Livrable 1).
//! Coexists with legacy phrase_candidates → judge-candidates path.

use sha2::{Digest, Sha256};
use talaria_core::AppConfig;
use talaria_judge::{parse_place_surface, parse_time_surface};
use talaria_quality::{
    apply_gates, candidate_fingerprint, event_fingerprint, parse_typed_time, resolve_mentions,
    BuildProjections, ClauseAnalyzeInput, ClauseAnalyzer, DerivedLabelProjections,
    DeterministicClauseAnalyzer, EntityKind, EvidencePtr, GazetteerResolver, GateContext,
    ParticipantRole, TypedTime, ASSEMBLER_V1, EXTRACTOR_DETERMINISTIC_V1,
};
use talaria_store::{
    connect, count_active_quality_by_type, find_active_quality_event_by_fingerprint,
    find_active_singleton, insert_document_fragment, insert_document_snapshot,
    insert_quality_canonical_event, mark_candidate_assembled, quality_lifespan_years,
    quality_report_counts, rejection_reason_counts, run_migrations,
    update_event_candidate_judgment, upsert_entity_with_kind, upsert_event_candidate,
    DocumentFragmentInsert, DocumentSnapshotInsert, EventCandidateInsert, QualityEventInsert,
};
use uuid::Uuid;

fn content_hash(text: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(text.as_bytes());
    hex::encode(hasher.finalize())
}

fn time_to_json(time: &TypedTime) -> serde_json::Value {
    serde_json::to_value(time).unwrap_or_else(|_| serde_json::json!({"kind":"unknown"}))
}

fn start_time_from_typed(time: &TypedTime) -> Option<chrono::DateTime<chrono::Utc>> {
    time.year_for_gates()
        .and_then(|y| parse_time_surface(&y.to_string()).map(|p| p.start))
}

fn split_sentences(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    for ch in text.chars() {
        cur.push(ch);
        if ch == '.' || ch == '\n' {
            let t = cur.trim().to_string();
            if !t.is_empty() {
                out.push(t);
            }
            cur.clear();
        }
    }
    let t = cur.trim().to_string();
    if !t.is_empty() {
        out.push(t);
    }
    out
}

#[derive(Debug, Default, Clone)]
pub struct QualityRunStats {
    pub extractions: usize,
    pub candidates_inserted: usize,
    pub candidates_deduped: usize,
    pub accepted: usize,
    pub rejected: usize,
    pub needs_review: usize,
    pub assembled: usize,
    pub assembled_deduped: usize,
}

async fn judge_and_maybe_assemble(
    pool: &sqlx::PgPool,
    shell: &mut talaria_quality::EventCandidate,
    candidate_id: Uuid,
    inserted: bool,
    cross_clause_join: bool,
    invalid_place_attempt: bool,
    clause_text: &str,
    assemble: bool,
    stats: &mut QualityRunStats,
) -> anyhow::Result<()> {
    let projections = DerivedLabelProjections;
    let subject_entity_id = shell
        .subject_entity_id
        .ok_or_else(|| anyhow::anyhow!("subject_entity_id required"))?;

    let (birth_year, death_year, has_birth, has_death) =
        quality_lifespan_years(pool, subject_entity_id).await?;

    let place_entity_kind = match shell.place_entity_id {
        Some(pid) => {
            let k = talaria_store::get_entity_kind(pool, pid).await?;
            k.map(|s| EntityKind::parse(&s))
        }
        None => None,
    };

    let ctx = GateContext {
        subject_birth_year: birth_year,
        subject_death_year: death_year,
        has_active_birth: has_birth,
        has_active_death: has_death,
        fingerprint_exists: !inserted,
        cross_clause_join_detected: cross_clause_join,
        place_entity_kind,
    };

    let decision = apply_gates(shell, &ctx);
    let status = decision.status().as_str();
    let codes = decision.codes();
    let judgment = serde_json::json!({
        "decision": status,
        "codes": codes,
        "cross_clause_join": cross_clause_join,
        "invalid_place_attempt": invalid_place_attempt,
    });

    update_event_candidate_judgment(
        pool,
        candidate_id,
        status,
        &codes,
        &judgment,
        shell.subject_entity_id,
        shell.place_entity_id,
        shell.place_label.as_deref(),
        &serde_json::to_value(&shell.place_mentions)?,
        &serde_json::to_value(&shell.object_mentions)?,
        &serde_json::to_value(&shell.participant_mentions)?,
    )
    .await?;

    match status {
        "accepted" => stats.accepted += 1,
        "rejected" => stats.rejected += 1,
        "needs_review" => stats.needs_review += 1,
        _ => {}
    }

    if !(assemble && status == "accepted") {
        return Ok(());
    }

    let proj = projections.from_candidate(shell, &shell.subject_surface);
    let title_derived = projections.display_label(&proj);
    let place = shell.place_label.as_deref().map(parse_place_surface);
    let map_eligible = place.as_ref().is_some_and(|p| p.map_eligible());
    let (lat, lon) = place
        .as_ref()
        .map(|p| (p.lat, p.lon))
        .unwrap_or((None, None));

    let participant_ids: Vec<String> = shell
        .participant_mentions
        .iter()
        .filter_map(|m| m.entity_id.map(|id| id.to_string()))
        .collect();
    let fp = event_fingerprint(
        &subject_entity_id.to_string(),
        &shell.event_type,
        &shell.predicate,
        &shell.time,
        shell.place_entity_id.map(|id| id.to_string()).as_deref(),
        &participant_ids,
    );

    if let Some(existing) = find_active_quality_event_by_fingerprint(pool, &fp).await? {
        talaria_store::reinforce_quality_event(pool, existing).await?;
        mark_candidate_assembled(pool, candidate_id, existing).await?;
        stats.assembled_deduped += 1;
        return Ok(());
    }

    if shell.event_type == "birth" || shell.event_type == "death" {
        if find_active_singleton(pool, subject_entity_id, &shell.event_type)
            .await?
            .is_some()
        {
            update_event_candidate_judgment(
                pool,
                candidate_id,
                "rejected",
                &["singleton_cardinality_violation".into()],
                &serde_json::json!({"at":"assemble"}),
                shell.subject_entity_id,
                shell.place_entity_id,
                shell.place_label.as_deref(),
                &serde_json::to_value(&shell.place_mentions)?,
                &serde_json::to_value(&shell.object_mentions)?,
                &serde_json::to_value(&shell.participant_mentions)?,
            )
            .await?;
            stats.rejected += 1;
            stats.accepted = stats.accepted.saturating_sub(1);
            return Ok(());
        }
    }

    let event_id = insert_quality_canonical_event(
        pool,
        &QualityEventInsert {
            entity_id: subject_entity_id,
            event_type: shell.event_type.clone(),
            epistemic_status: "attested".into(),
            title: title_derived,
            summary: Some(clause_text.to_string()),
            start_time: start_time_from_typed(&shell.time),
            time_json: time_to_json(&shell.time),
            place_label: shell.place_label.clone(),
            place_entity_id: shell.place_entity_id,
            lat,
            lon,
            confidence: 0.8,
            map_eligible,
            historically_valid: true,
            timeline_eligible: true,
            fingerprint: fp,
            predicate: shell.predicate.clone(),
            assembler_version: ASSEMBLER_V1.into(),
            event_candidate_id: candidate_id,
            supersedes: None,
            source_count: 1,
            evidence_count: 1,
        },
    )
    .await?;

    mark_candidate_assembled(pool, candidate_id, event_id).await?;
    stats.assembled += 1;
    Ok(())
}

/// Ingest a document through: snapshot → clauses → candidates → gates → assemble.
pub async fn run_quality_fixture(
    config: &AppConfig,
    title: &str,
    text: &str,
    assemble: bool,
) -> anyhow::Result<QualityRunStats> {
    let pool = connect(config).await?;
    run_migrations(&pool).await?;

    let analyzer = DeterministicClauseAnalyzer;
    let resolver = GazetteerResolver;
    let mut stats = QualityRunStats::default();

    let hash = content_hash(text);
    let snapshot_id = insert_document_snapshot(
        &pool,
        &DocumentSnapshotInsert {
            source_type: "fixture".into(),
            source_uri: format!("fixture://{}", title.replace(' ', "_")),
            source_identifier: Some(title.into()),
            language: config.wiki_lang.clone(),
            title: Some(title.into()),
            content_hash: hash,
            revision_id: None,
            wiki_page_id: None,
            raw_document_id: None,
            text: text.into(),
            metadata: serde_json::json!({"pipeline": "quality"}),
        },
    )
    .await?;

    let mut offset = 0i32;
    let sentences = split_sentences(text);

    for (ordinal, sentence) in sentences.iter().enumerate() {
        let start_offset = offset;
        let end_offset = start_offset + sentence.len() as i32;
        offset = end_offset + 1;

        let sentence_frag_id = insert_document_fragment(
            &pool,
            &DocumentFragmentInsert {
                snapshot_id,
                fragment_kind: "sentence".into(),
                parent_fragment_id: None,
                sentence_id: None,
                text: sentence.clone(),
                start_offset,
                end_offset,
                clause_index: None,
                ordinal: ordinal as i32,
            },
        )
        .await?;

        let extractions = analyzer.analyze_sentence(&ClauseAnalyzeInput {
            text: sentence.clone(),
            page_title: Some(title.into()),
            start_offset,
        });

        for ex in extractions {
            stats.extractions += 1;

            let clause_frag_id = insert_document_fragment(
                &pool,
                &DocumentFragmentInsert {
                    snapshot_id,
                    fragment_kind: "clause".into(),
                    parent_fragment_id: Some(sentence_frag_id),
                    sentence_id: None,
                    text: ex.clause_text.clone(),
                    start_offset: ex.clause_start_offset,
                    end_offset: ex.clause_end_offset,
                    clause_index: Some(ex.clause_index),
                    ordinal: ex.clause_index,
                },
            )
            .await?;

            let time = parse_typed_time(ex.time_surface.as_deref());
            let mut shell = talaria_quality::EventCandidate {
                id: Uuid::nil(),
                snapshot_id,
                fragment_id: clause_frag_id,
                clause_index: ex.clause_index,
                subject_surface: ex.subject_surface.clone(),
                subject_entity_id: None,
                event_type: ex.event_type.clone(),
                predicate: ex.predicate.clone(),
                time: time.clone(),
                place_mentions: vec![],
                object_mentions: vec![],
                participant_mentions: vec![],
                place_entity_id: None,
                place_label: None,
                evidence_ptrs: vec![],
                extractor_version: analyzer.version().to_string(),
                fingerprint: String::new(),
                status: talaria_quality::CandidateStatus::Pending,
                rejection_codes: vec![],
            };

            let participants: Vec<(String, ParticipantRole)> = ex
                .participant_surfaces
                .iter()
                .map(|s| (s.clone(), ParticipantRole::Participant))
                .collect();

            let resolved = resolve_mentions(
                &shell,
                &resolver,
                ex.place_surface.as_deref(),
                ex.object_surface.as_deref(),
                &participants,
            );

            shell.place_mentions = resolved.place_mentions.clone();
            shell.object_mentions = resolved.object_mentions.clone();
            shell.participant_mentions = resolved.participant_mentions.clone();
            shell.place_label = resolved.place_label.clone();

            let subject_kind = resolved
                .subject_kind
                .unwrap_or(EntityKind::Person)
                .as_str();
            let subject_entity_id = upsert_entity_with_kind(
                &pool,
                &config.wiki_lang,
                &ex.subject_surface,
                subject_kind,
            )
            .await?;
            shell.subject_entity_id = Some(subject_entity_id);

            if resolved.place_kind == Some(EntityKind::Place) {
                if let Some(label) = &resolved.place_label {
                    shell.place_entity_id = Some(
                        upsert_entity_with_kind(&pool, &config.wiki_lang, label, "place").await?,
                    );
                }
            }

            for m in &mut shell.participant_mentions {
                let kind = m.kind.unwrap_or(EntityKind::Unknown).as_str();
                m.entity_id = Some(
                    upsert_entity_with_kind(&pool, &config.wiki_lang, &m.surface, kind).await?,
                );
            }

            shell.evidence_ptrs = vec![EvidencePtr {
                fragment_id: clause_frag_id,
                clause_index: ex.clause_index,
                start_offset: ex.clause_start_offset,
                end_offset: ex.clause_end_offset,
                quoted_text: ex.clause_text.clone(),
            }];

            shell.fingerprint = candidate_fingerprint(
                analyzer.version(),
                &shell.subject_surface,
                &shell.event_type,
                &shell.predicate,
                &shell.time,
                shell.place_label.as_deref(),
                &snapshot_id.to_string(),
                shell.clause_index,
                ex.clause_start_offset,
                ex.clause_end_offset,
                &shell.participant_mentions,
            );

            let (candidate_id, inserted) = upsert_event_candidate(
                &pool,
                &EventCandidateInsert {
                    snapshot_id,
                    fragment_id: clause_frag_id,
                    clause_index: shell.clause_index,
                    subject_surface: shell.subject_surface.clone(),
                    subject_entity_id: shell.subject_entity_id,
                    event_type: shell.event_type.clone(),
                    predicate: shell.predicate.clone(),
                    time_json: time_to_json(&shell.time),
                    place_mentions: serde_json::to_value(&shell.place_mentions)?,
                    object_mentions: serde_json::to_value(&shell.object_mentions)?,
                    participant_mentions: serde_json::to_value(&shell.participant_mentions)?,
                    place_entity_id: shell.place_entity_id,
                    place_label: shell.place_label.clone(),
                    evidence_ptrs: serde_json::to_value(&shell.evidence_ptrs)?,
                    extractor_version: shell.extractor_version.clone(),
                    fingerprint: shell.fingerprint.clone(),
                    status: "pending".into(),
                    rejection_codes: vec![],
                    judgment_json: serde_json::json!({}),
                },
            )
            .await?;
            shell.id = candidate_id;
            if inserted {
                stats.candidates_inserted += 1;
                judge_and_maybe_assemble(
                    &pool,
                    &mut shell,
                    candidate_id,
                    inserted,
                    ex.cross_clause_join,
                    resolved.invalid_place_attempt,
                    &ex.clause_text,
                    assemble,
                    &mut stats,
                )
                .await?;
            } else {
                stats.candidates_deduped += 1;
            }
        }
    }

    Ok(stats)
}

/// Inject an adversarial candidate (for deterministic gate tests).
pub async fn inject_adversarial_candidate(
    config: &AppConfig,
    subject: &str,
    event_type: &str,
    predicate: &str,
    year: i32,
    place_surface: Option<&str>,
    force_person_as_place: bool,
    cross_clause_join: bool,
) -> anyhow::Result<(Uuid, Vec<String>)> {
    let pool = connect(config).await?;
    run_migrations(&pool).await?;
    let resolver = GazetteerResolver;

    let text = format!(
        "ADVERSARIAL {} {} {} {:?}",
        subject, event_type, year, place_surface
    );
    let snap = insert_document_snapshot(
        &pool,
        &DocumentSnapshotInsert {
            source_type: "adversarial".into(),
            source_uri: format!(
                "adversarial://{}/{}/{}/{}",
                subject,
                event_type,
                year,
                place_surface.unwrap_or("-")
            ),
            source_identifier: Some(subject.into()),
            language: config.wiki_lang.clone(),
            title: Some(subject.into()),
            content_hash: content_hash(&text),
            revision_id: None,
            wiki_page_id: None,
            raw_document_id: None,
            text: text.clone(),
            metadata: serde_json::json!({"adversarial": true}),
        },
    )
    .await?;
    let frag = insert_document_fragment(
        &pool,
        &DocumentFragmentInsert {
            snapshot_id: snap,
            fragment_kind: "sentence".into(),
            parent_fragment_id: None,
            sentence_id: None,
            text: text.clone(),
            start_offset: 0,
            end_offset: text.len() as i32,
            clause_index: None,
            ordinal: 0,
        },
    )
    .await?;

    let time = parse_typed_time(Some(&year.to_string()));
    let subject_id = upsert_entity_with_kind(&pool, &config.wiki_lang, subject, "person").await?;

    let mut shell = talaria_quality::EventCandidate {
        id: Uuid::nil(),
        snapshot_id: snap,
        fragment_id: frag,
        clause_index: 0,
        subject_surface: subject.into(),
        subject_entity_id: Some(subject_id),
        event_type: event_type.into(),
        predicate: predicate.into(),
        time: time.clone(),
        place_mentions: vec![],
        object_mentions: vec![],
        participant_mentions: vec![],
        place_entity_id: None,
        place_label: None,
        evidence_ptrs: vec![EvidencePtr {
            fragment_id: frag,
            clause_index: 0,
            start_offset: 0,
            end_offset: text.len() as i32,
            quoted_text: text.clone(),
        }],
        extractor_version: EXTRACTOR_DETERMINISTIC_V1.into(),
        fingerprint: String::new(),
        status: talaria_quality::CandidateStatus::Pending,
        rejection_codes: vec![],
    };

    let mut invalid_place = false;
    if force_person_as_place {
        if let Some(ps) = place_surface {
            let person_id =
                upsert_entity_with_kind(&pool, &config.wiki_lang, ps, "person").await?;
            shell.place_entity_id = Some(person_id);
            shell.place_label = Some(ps.into());
            shell.place_mentions.push(talaria_quality::Mention {
                surface: ps.into(),
                entity_id: Some(person_id),
                kind: Some(EntityKind::Person),
                role: None,
            });
            invalid_place = true;
        }
    } else {
        let resolved = resolve_mentions(&shell, &resolver, place_surface, None, &[]);
        invalid_place = resolved.invalid_place_attempt;
        shell.place_mentions = resolved.place_mentions;
        shell.participant_mentions = resolved.participant_mentions;
        shell.place_label = resolved.place_label;
        if resolved.place_kind == Some(EntityKind::Place) {
            if let Some(label) = &shell.place_label.clone() {
                shell.place_entity_id = Some(
                    upsert_entity_with_kind(&pool, &config.wiki_lang, label, "place").await?,
                );
            }
        }
    }

    shell.fingerprint = candidate_fingerprint(
        EXTRACTOR_DETERMINISTIC_V1,
        &shell.subject_surface,
        &shell.event_type,
        &shell.predicate,
        &shell.time,
        shell.place_label.as_deref(),
        &snap.to_string(),
        0,
        0,
        text.len() as i32,
        &shell.participant_mentions,
    );

    let (candidate_id, inserted) = upsert_event_candidate(
        &pool,
        &EventCandidateInsert {
            snapshot_id: snap,
            fragment_id: frag,
            clause_index: 0,
            subject_surface: shell.subject_surface.clone(),
            subject_entity_id: shell.subject_entity_id,
            event_type: shell.event_type.clone(),
            predicate: shell.predicate.clone(),
            time_json: time_to_json(&shell.time),
            place_mentions: serde_json::to_value(&shell.place_mentions)?,
            object_mentions: serde_json::json!([]),
            participant_mentions: serde_json::to_value(&shell.participant_mentions)?,
            place_entity_id: shell.place_entity_id,
            place_label: shell.place_label.clone(),
            evidence_ptrs: serde_json::to_value(&shell.evidence_ptrs)?,
            extractor_version: shell.extractor_version.clone(),
            fingerprint: shell.fingerprint.clone(),
            status: "pending".into(),
            rejection_codes: vec![],
            judgment_json: serde_json::json!({}),
        },
    )
    .await?;
    shell.id = candidate_id;

    let mut stats = QualityRunStats::default();
    judge_and_maybe_assemble(
        &pool,
        &mut shell,
        candidate_id,
        inserted,
        cross_clause_join,
        invalid_place,
        &text,
        false,
        &mut stats,
    )
    .await?;

    let codes: Vec<String> =
        sqlx::query_scalar(r#"SELECT rejection_codes FROM event_candidates WHERE id = $1"#)
            .bind(candidate_id)
            .fetch_one(&pool)
            .await?;

    Ok((candidate_id, codes))
}

pub async fn run_quality_report(config: &AppConfig) -> anyhow::Result<String> {
    let pool = connect(config).await?;
    run_migrations(&pool).await?;
    let counts = quality_report_counts(&pool).await?;
    let reasons = rejection_reason_counts(&pool).await?;

    let mut report = String::new();
    report.push_str(&format!("candidats\t{}\n", counts.candidates));
    report.push_str(&format!("rejetés\t{}\n", counts.rejected));
    for r in &reasons {
        report.push_str(&format!(
            "  rejetés par raison\t{}\t{}\n",
            r.code, r.count
        ));
    }
    report.push_str(&format!("à revoir\t{}\n", counts.needs_review));
    report.push_str("claims consolidées\t0\n");
    report.push_str(&format!(
        "événements acceptés\t{}\n",
        counts.quality_events_active
    ));
    report.push_str(&format!(
        "événements cartographiables\t{}\n",
        counts.quality_events_map_eligible
    ));
    print!("{report}");
    Ok(report)
}

pub async fn run_quality_supersede_death(
    config: &AppConfig,
    subject: &str,
    new_year: i32,
    place: &str,
) -> anyhow::Result<Uuid> {
    let pool = connect(config).await?;
    run_migrations(&pool).await?;
    let subject_id = upsert_entity_with_kind(&pool, &config.wiki_lang, subject, "person").await?;
    let old = find_active_singleton(&pool, subject_id, "death").await?;
    let place_id = upsert_entity_with_kind(&pool, &config.wiki_lang, place, "place").await?;
    let time = parse_typed_time(Some(&new_year.to_string()));
    let fp = event_fingerprint(
        &subject_id.to_string(),
        "death",
        "died_in",
        &time,
        Some(&place_id.to_string()),
        &[],
    );

    let text = format!("{subject} died in {place} in {new_year}.");
    let snap = insert_document_snapshot(
        &pool,
        &DocumentSnapshotInsert {
            source_type: "correction".into(),
            source_uri: format!("correction://{subject}/death/{new_year}"),
            source_identifier: Some(subject.into()),
            language: config.wiki_lang.clone(),
            title: Some(subject.into()),
            content_hash: content_hash(&text),
            revision_id: None,
            wiki_page_id: None,
            raw_document_id: None,
            text: text.clone(),
            metadata: serde_json::json!({"kind":"supersession"}),
        },
    )
    .await?;
    let frag = insert_document_fragment(
        &pool,
        &DocumentFragmentInsert {
            snapshot_id: snap,
            fragment_kind: "sentence".into(),
            parent_fragment_id: None,
            sentence_id: None,
            text: text.clone(),
            start_offset: 0,
            end_offset: text.len() as i32,
            clause_index: None,
            ordinal: 0,
        },
    )
    .await?;
    let cand_fp = candidate_fingerprint(
        EXTRACTOR_DETERMINISTIC_V1,
        subject,
        "death",
        "died_in",
        &time,
        Some(place),
        &snap.to_string(),
        0,
        0,
        text.len() as i32,
        &[],
    );
    let (cand_id, _) = upsert_event_candidate(
        &pool,
        &EventCandidateInsert {
            snapshot_id: snap,
            fragment_id: frag,
            clause_index: 0,
            subject_surface: subject.into(),
            subject_entity_id: Some(subject_id),
            event_type: "death".into(),
            predicate: "died_in".into(),
            time_json: time_to_json(&time),
            place_mentions: serde_json::json!([{"surface": place, "kind": "place"}]),
            object_mentions: serde_json::json!([]),
            participant_mentions: serde_json::json!([]),
            place_entity_id: Some(place_id),
            place_label: Some(place.into()),
            evidence_ptrs: serde_json::json!([{
                "fragment_id": frag,
                "clause_index": 0,
                "start_offset": 0,
                "end_offset": text.len() as i32,
                "quoted_text": text,
            }]),
            extractor_version: EXTRACTOR_DETERMINISTIC_V1.into(),
            fingerprint: cand_fp,
            status: "accepted".into(),
            rejection_codes: vec![],
            judgment_json: serde_json::json!({"supersession": true}),
        },
    )
    .await?;

    let place_parsed = parse_place_surface(place);
    // Distinct fingerprint for superseding correction when same year/place already active.
    let fp = format!("{fp}:correction");
    let new_id = insert_quality_canonical_event(
        &pool,
        &QualityEventInsert {
            entity_id: subject_id,
            event_type: "death".into(),
            epistemic_status: "established".into(),
            title: format!("{subject} — death ({new_year}) @ {place}"),
            summary: Some(text),
            start_time: start_time_from_typed(&time),
            time_json: time_to_json(&time),
            place_label: Some(place.into()),
            place_entity_id: Some(place_id),
            lat: place_parsed.lat,
            lon: place_parsed.lon,
            confidence: 0.95,
            map_eligible: place_parsed.map_eligible(),
            historically_valid: true,
            timeline_eligible: true,
            fingerprint: fp,
            predicate: "died_in".into(),
            assembler_version: ASSEMBLER_V1.into(),
            event_candidate_id: cand_id,
            supersedes: old,
            source_count: 1,
            evidence_count: 1,
        },
    )
    .await?;
    mark_candidate_assembled(&pool, cand_id, new_id).await?;
    tracing::info!(?old, new = %new_id, "supersession complete (append-only)");
    Ok(new_id)
}

/// Clean fixture (valid events). Adversarial cases injected separately.
pub const NAPOLEON_CLEAN_FIXTURE: &str = r#"
Napoleon was born in Ajaccio in 1769.
He married Joséphine in 1796 in Paris.
He fought at Leipzig in 1813.
He died in Saint Helena in 1821.
"#;

pub async fn run_quality_napoleon_demo(config: &AppConfig) -> anyhow::Result<String> {
    let stats = run_quality_fixture(config, "Napoleon", NAPOLEON_CLEAN_FIXTURE, true).await?;
    tracing::info!(?stats, "clean fixture complete");

    let (_, leipzig_codes) = inject_adversarial_candidate(
        config,
        "Napoleon",
        "battle",
        "fought_at",
        1774,
        Some("Leipzig"),
        false,
        false,
    )
    .await?;
    tracing::info!(?leipzig_codes, "Leipzig 1774 codes");

    let (_, cross_codes) = inject_adversarial_candidate(
        config,
        "Napoleon",
        "battle",
        "fought_at",
        1813,
        Some("Moscow"),
        false,
        true,
    )
    .await?;
    tracing::info!(?cross_codes, "cross_clause codes");

    let (_, waterloo_codes) = inject_adversarial_candidate(
        config,
        "Napoleon",
        "death",
        "died_in",
        1798,
        Some("Waterloo"),
        false,
        false,
    )
    .await?;
    tracing::info!(?waterloo_codes, "Waterloo 1798 codes");

    let (_, jose_codes) = inject_adversarial_candidate(
        config,
        "Napoleon",
        "marriage",
        "married",
        1796,
        Some("Joséphine"),
        true,
        false,
    )
    .await?;
    tracing::info!(?jose_codes, "Joséphine-as-place codes");

    let stats2 = run_quality_fixture(config, "Napoleon", NAPOLEON_CLEAN_FIXTURE, true).await?;
    tracing::info!(?stats2, "retry complete");

    let pool = connect(config).await?;
    let subject_id =
        upsert_entity_with_kind(&pool, &config.wiki_lang, "Napoleon", "person").await?;
    let deaths = count_active_quality_by_type(&pool, subject_id, "death").await?;
    tracing::info!(deaths, "active quality deaths");

    let new_death = run_quality_supersede_death(config, "Napoleon", 1821, "Saint Helena").await?;
    let deaths_after = count_active_quality_by_type(&pool, subject_id, "death").await?;
    tracing::info!(%new_death, deaths_after, "after supersession");

    run_quality_report(config).await
}
