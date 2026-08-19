// crates/talaria-store/src/multi_source.rs
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct DiscoveryRunInsert {
    pub subject_entity_id: Uuid,
    pub subject_qid: Option<String>,
    pub subject_label: String,
    pub plan_json: serde_json::Value,
    pub budgets_json: serde_json::Value,
    pub connector_versions: serde_json::Value,
}

pub async fn start_discovery_run(pool: &PgPool, run: &DiscoveryRunInsert) -> anyhow::Result<Uuid> {
    let id: Uuid = sqlx::query_scalar(
        r#"
        INSERT INTO source_discovery_runs (
            subject_entity_id, subject_qid, subject_label, plan_json, budgets_json,
            status, connector_versions, started_at
        )
        VALUES ($1,$2,$3,$4,$5,'running',$6,NOW())
        RETURNING id
        "#,
    )
    .bind(run.subject_entity_id)
    .bind(&run.subject_qid)
    .bind(&run.subject_label)
    .bind(&run.plan_json)
    .bind(&run.budgets_json)
    .bind(&run.connector_versions)
    .fetch_one(pool)
    .await?;
    Ok(id)
}

pub async fn finish_discovery_run(
    pool: &PgPool,
    run_id: Uuid,
    status: &str,
    metrics: &serde_json::Value,
    error: Option<&serde_json::Value>,
) -> anyhow::Result<()> {
    sqlx::query(
        r#"
        UPDATE source_discovery_runs SET
            status = $2,
            metrics_json = $3,
            error_json = $4,
            finished_at = NOW()
        WHERE id = $1
        "#,
    )
    .bind(run_id)
    .bind(status)
    .bind(metrics)
    .bind(error)
    .execute(pool)
    .await?;
    Ok(())
}

#[derive(Debug, Clone)]
pub struct DiscoveredDocumentInsert {
    pub run_id: Uuid,
    pub source_kind: String,
    pub external_id: String,
    pub canonical_url: Option<String>,
    pub title: String,
    pub language: Option<String>,
    pub document_type: String,
    pub discovery_method: String,
    pub relevance_score: f32,
    pub subject_links: serde_json::Value,
    pub source_metadata: serde_json::Value,
}

pub async fn upsert_discovered_document(
    pool: &PgPool,
    doc: &DiscoveredDocumentInsert,
) -> anyhow::Result<(Uuid, bool)> {
    let row: Option<(Uuid,)> = sqlx::query_as(
        r#"
        INSERT INTO discovered_documents (
            run_id, source_kind, external_id, canonical_url, title, language,
            document_type, discovery_method, relevance_score, subject_links, source_metadata
        )
        VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11)
        ON CONFLICT (run_id, source_kind, external_id) DO NOTHING
        RETURNING id
        "#,
    )
    .bind(doc.run_id)
    .bind(&doc.source_kind)
    .bind(&doc.external_id)
    .bind(&doc.canonical_url)
    .bind(&doc.title)
    .bind(&doc.language)
    .bind(&doc.document_type)
    .bind(&doc.discovery_method)
    .bind(doc.relevance_score)
    .bind(&doc.subject_links)
    .bind(&doc.source_metadata)
    .fetch_optional(pool)
    .await?;
    if let Some((id,)) = row {
        return Ok((id, true));
    }
    let id: Uuid = sqlx::query_scalar(
        r#"
        SELECT id FROM discovered_documents
        WHERE run_id = $1 AND source_kind = $2 AND external_id = $3
        "#,
    )
    .bind(doc.run_id)
    .bind(&doc.source_kind)
    .bind(&doc.external_id)
    .fetch_one(pool)
    .await?;
    Ok((id, false))
}

pub async fn mark_discovered_snapshotted(
    pool: &PgPool,
    doc_id: Uuid,
    snapshot_id: Uuid,
) -> anyhow::Result<()> {
    sqlx::query(
        r#"
        UPDATE discovered_documents SET
            fetch_status = 'snapshotted',
            snapshot_id = $2
        WHERE id = $1
        "#,
    )
    .bind(doc_id)
    .bind(snapshot_id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn mark_discovered_skipped(
    pool: &PgPool,
    doc_id: Uuid,
    reason: &str,
) -> anyhow::Result<()> {
    sqlx::query(
        r#"
        UPDATE discovered_documents SET
            fetch_status = 'skipped',
            skip_reason = $2
        WHERE id = $1
        "#,
    )
    .bind(doc_id)
    .bind(reason)
    .execute(pool)
    .await?;
    Ok(())
}

#[derive(Debug, Clone)]
pub struct QualityClaimInsert {
    pub subject_entity_id: Uuid,
    pub fingerprint: String,
    pub predicate: String,
    pub event_type: String,
    pub object_json: serde_json::Value,
    pub time_json: serde_json::Value,
    pub place_entity_id: Option<Uuid>,
    pub place_label: Option<String>,
    pub occurrence_stem: Option<String>,
}

pub async fn upsert_quality_claim(
    pool: &PgPool,
    claim: &QualityClaimInsert,
) -> anyhow::Result<(Uuid, bool)> {
    // support_count starts at 0; add_claim_support increments only when a
    // new support row is actually inserted (retry-safe).
    let inserted: Option<(Uuid,)> = sqlx::query_as(
        r#"
        INSERT INTO quality_claims (
            subject_entity_id, fingerprint, predicate, event_type,
            object_json, time_json, place_entity_id, place_label, occurrence_stem,
            status, support_count
        )
        VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,'open',0)
        ON CONFLICT (fingerprint) DO NOTHING
        RETURNING id
        "#,
    )
    .bind(claim.subject_entity_id)
    .bind(&claim.fingerprint)
    .bind(&claim.predicate)
    .bind(&claim.event_type)
    .bind(&claim.object_json)
    .bind(&claim.time_json)
    .bind(claim.place_entity_id)
    .bind(&claim.place_label)
    .bind(&claim.occurrence_stem)
    .fetch_optional(pool)
    .await?;
    if let Some((id,)) = inserted {
        return Ok((id, true));
    }
    let id: Uuid = sqlx::query_scalar(r#"SELECT id FROM quality_claims WHERE fingerprint = $1"#)
        .bind(&claim.fingerprint)
        .fetch_one(pool)
        .await?;
    Ok((id, false))
}

pub async fn add_claim_support(
    pool: &PgPool,
    claim_id: Uuid,
    event_candidate_id: Option<Uuid>,
    snapshot_id: Option<Uuid>,
    source_kind: &str,
    evidence_ptr: &serde_json::Value,
) -> anyhow::Result<bool> {
    let inserted: Option<(Uuid,)> = sqlx::query_as(
        r#"
        INSERT INTO quality_claim_supports (
            claim_id, event_candidate_id, snapshot_id, source_kind, evidence_ptr
        )
        VALUES ($1,$2,$3,$4,$5)
        ON CONFLICT (claim_id, event_candidate_id) DO NOTHING
        RETURNING id
        "#,
    )
    .bind(claim_id)
    .bind(event_candidate_id)
    .bind(snapshot_id)
    .bind(source_kind)
    .bind(evidence_ptr)
    .fetch_optional(pool)
    .await?;
    if inserted.is_some() {
        sqlx::query(
            r#"
            UPDATE quality_claims SET
                support_count = support_count + 1,
                updated_at = NOW()
            WHERE id = $1
            "#,
        )
        .bind(claim_id)
        .execute(pool)
        .await?;
        return Ok(true);
    }
    Ok(false)
}

pub async fn list_place_labels_for_occurrence_stem(
    pool: &PgPool,
    subject_entity_id: Uuid,
    stem: &str,
) -> anyhow::Result<Vec<String>> {
    let rows: Vec<(String,)> = sqlx::query_as(
        r#"
        SELECT DISTINCT place_label FROM (
            SELECT place_label FROM quality_claims
            WHERE subject_entity_id = $1
              AND occurrence_stem = $2
              AND place_label IS NOT NULL
              AND btrim(place_label) <> ''
            UNION
            SELECT place_label FROM canonical_events
            WHERE entity_id = $1
              AND occurrence_stem = $2
              AND pipeline = 'quality'
              AND is_active
              AND place_label IS NOT NULL
              AND btrim(place_label) <> ''
        ) s
        "#,
    )
    .bind(subject_entity_id)
    .bind(stem)
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(|(s,)| s).collect())
}

pub async fn mark_quality_claims_conflict_by_stem(
    pool: &PgPool,
    subject_entity_id: Uuid,
    stem: &str,
    conflict_json: &serde_json::Value,
) -> anyhow::Result<()> {
    sqlx::query(
        r#"
        UPDATE quality_claims SET
            status = 'conflict',
            conflict_json = $3,
            updated_at = NOW()
        WHERE subject_entity_id = $1 AND occurrence_stem = $2
        "#,
    )
    .bind(subject_entity_id)
    .bind(stem)
    .bind(conflict_json)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn mark_quality_events_uncertain_by_stem(
    pool: &PgPool,
    subject_entity_id: Uuid,
    stem: &str,
) -> anyhow::Result<()> {
    sqlx::query(
        r#"
        UPDATE canonical_events SET
            epistemic_status = 'uncertain'
        WHERE entity_id = $1
          AND occurrence_stem = $2
          AND pipeline = 'quality'
          AND is_active
        "#,
    )
    .bind(subject_entity_id)
    .bind(stem)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn link_claim_to_event(
    pool: &PgPool,
    claim_id: Uuid,
    event_id: Uuid,
) -> anyhow::Result<()> {
    sqlx::query(
        r#"
        UPDATE quality_claims SET
            status = 'consolidated',
            canonical_event_id = $2,
            updated_at = NOW()
        WHERE id = $1
        "#,
    )
    .bind(claim_id)
    .bind(event_id)
    .execute(pool)
    .await?;
    Ok(())
}

#[derive(Debug, Clone, Default)]
pub struct DensityReportCounts {
    pub documents_discovered: i64,
    pub documents_snapshotted: i64,
    pub fragments: i64,
    pub candidates: i64,
    pub rejected: i64,
    pub needs_review: i64,
    pub claims: i64,
    pub accepted_events: i64,
    pub timeline_eligible: i64,
    pub map_eligible: i64,
    pub events_without_place: i64,
    pub multi_source_events: i64,
}

pub async fn density_report_counts(
    pool: &PgPool,
    subject_entity_id: Option<Uuid>,
) -> anyhow::Result<DensityReportCounts> {
    let documents_discovered: i64 = if let Some(sid) = subject_entity_id {
        sqlx::query_scalar(
            r#"
            SELECT COUNT(*)::bigint FROM discovered_documents dd
            JOIN source_discovery_runs r ON r.id = dd.run_id
            WHERE r.subject_entity_id = $1
            "#,
        )
        .bind(sid)
        .fetch_one(pool)
        .await?
    } else {
        sqlx::query_scalar(r#"SELECT COUNT(*)::bigint FROM discovered_documents"#)
            .fetch_one(pool)
            .await?
    };

    let documents_snapshotted: i64 =
        sqlx::query_scalar(r#"SELECT COUNT(*)::bigint FROM document_snapshots"#)
            .fetch_one(pool)
            .await?;

    let fragments: i64 = sqlx::query_scalar(r#"SELECT COUNT(*)::bigint FROM document_fragments"#)
        .fetch_one(pool)
        .await?;

    let candidates: i64 = sqlx::query_scalar(r#"SELECT COUNT(*)::bigint FROM event_candidates"#)
        .fetch_one(pool)
        .await?;
    let rejected: i64 = sqlx::query_scalar(
        r#"SELECT COUNT(*)::bigint FROM event_candidates WHERE status = 'rejected'"#,
    )
    .fetch_one(pool)
    .await?;
    let needs_review: i64 = sqlx::query_scalar(
        r#"SELECT COUNT(*)::bigint FROM event_candidates WHERE status = 'needs_review'"#,
    )
    .fetch_one(pool)
    .await?;
    let claims: i64 = sqlx::query_scalar(r#"SELECT COUNT(*)::bigint FROM quality_claims"#)
        .fetch_one(pool)
        .await?;

    let subject_filter = subject_entity_id;
    let accepted_events: i64 = if let Some(sid) = subject_filter {
        sqlx::query_scalar(
            r#"
            SELECT COUNT(*)::bigint FROM canonical_events
            WHERE pipeline = 'quality' AND is_active AND entity_id = $1
            "#,
        )
        .bind(sid)
        .fetch_one(pool)
        .await?
    } else {
        sqlx::query_scalar(
            r#"SELECT COUNT(*)::bigint FROM canonical_events WHERE pipeline = 'quality' AND is_active"#,
        )
        .fetch_one(pool)
        .await?
    };

    let timeline_eligible: i64 = if let Some(sid) = subject_filter {
        sqlx::query_scalar(
            r#"
            SELECT COUNT(*)::bigint FROM canonical_events
            WHERE pipeline = 'quality' AND is_active AND timeline_eligible AND entity_id = $1
            "#,
        )
        .bind(sid)
        .fetch_one(pool)
        .await?
    } else {
        sqlx::query_scalar(
            r#"
            SELECT COUNT(*)::bigint FROM canonical_events
            WHERE pipeline = 'quality' AND is_active AND timeline_eligible
            "#,
        )
        .fetch_one(pool)
        .await?
    };

    let map_eligible: i64 = if let Some(sid) = subject_filter {
        sqlx::query_scalar(
            r#"
            SELECT COUNT(*)::bigint FROM canonical_events
            WHERE pipeline = 'quality' AND is_active AND map_eligible AND entity_id = $1
            "#,
        )
        .bind(sid)
        .fetch_one(pool)
        .await?
    } else {
        sqlx::query_scalar(
            r#"
            SELECT COUNT(*)::bigint FROM canonical_events
            WHERE pipeline = 'quality' AND is_active AND map_eligible
            "#,
        )
        .fetch_one(pool)
        .await?
    };

    let events_without_place: i64 = if let Some(sid) = subject_filter {
        sqlx::query_scalar(
            r#"
            SELECT COUNT(*)::bigint FROM canonical_events
            WHERE pipeline = 'quality' AND is_active AND timeline_eligible
              AND NOT map_eligible AND entity_id = $1
            "#,
        )
        .bind(sid)
        .fetch_one(pool)
        .await?
    } else {
        sqlx::query_scalar(
            r#"
            SELECT COUNT(*)::bigint FROM canonical_events
            WHERE pipeline = 'quality' AND is_active AND timeline_eligible AND NOT map_eligible
            "#,
        )
        .fetch_one(pool)
        .await?
    };

    let multi_source_events: i64 = if let Some(sid) = subject_filter {
        sqlx::query_scalar(
            r#"
            SELECT COUNT(*)::bigint FROM canonical_events
            WHERE pipeline = 'quality' AND is_active AND source_count > 1 AND entity_id = $1
            "#,
        )
        .bind(sid)
        .fetch_one(pool)
        .await?
    } else {
        sqlx::query_scalar(
            r#"
            SELECT COUNT(*)::bigint FROM canonical_events
            WHERE pipeline = 'quality' AND is_active AND source_count > 1
            "#,
        )
        .fetch_one(pool)
        .await?
    };

    Ok(DensityReportCounts {
        documents_discovered,
        documents_snapshotted,
        fragments,
        candidates,
        rejected,
        needs_review,
        claims,
        accepted_events,
        timeline_eligible,
        map_eligible,
        events_without_place,
        multi_source_events,
    })
}
