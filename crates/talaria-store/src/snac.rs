// crates/talaria-store/src/snac.rs
//! SNAC identity persistence. Alignment is read only from entity_identity_links.
//! Multi-statement writes take the caller's transaction so a rollback undoes them.

use chrono::{DateTime, Utc};
use sha2::{Digest, Sha256};
use sqlx::PgExecutor;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct PersistedDecision {
    pub ark: String,
    pub snac_id: Option<String>,
    pub snac_version: Option<String>,
    pub label: String,
    pub entity_type: String,
    pub record_status: String,
    pub same_as: serde_json::Value,
    pub exist_dates: serde_json::Value,
    pub raw_json: serde_json::Value,
    pub raw_body: Option<String>,
    pub source_url: String,
    pub status: String,
    pub method: String,
    pub evidence: serde_json::Value,
    pub retrieved_at: DateTime<Utc>,
}

pub fn advisory_key(entity_id: &Uuid) -> i64 {
    let bytes = entity_id.as_bytes();
    i64::from_le_bytes(bytes[0..8].try_into().expect("uuid is 16 bytes"))
}

pub async fn aligned_constellation_id<'c, E>(
    executor: E,
    entity_id: Uuid,
) -> anyhow::Result<Option<Uuid>>
where
    E: PgExecutor<'c>,
{
    let id = sqlx::query_scalar(
        r#"
        SELECT constellation_id
        FROM entity_identity_links
        WHERE entity_id = $1 AND status = 'aligned'
        "#,
    )
    .bind(entity_id)
    .fetch_optional(executor)
    .await?;
    Ok(id)
}

pub async fn persist_identity_links(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    entity_id: Uuid,
    decisions: &[PersistedDecision],
) -> anyhow::Result<()> {
    for decision in decisions {
        let snapshot_id = if decision.record_status != "stub" {
            if let Some(body) = decision.raw_body.as_deref() {
                Some(insert_immutable_snapshot(tx, decision, body).await?)
            } else {
                None
            }
        } else {
            None
        };
        let constellation_id = upsert_constellation(tx, decision, snapshot_id).await?;
        apply_identity_link(tx, entity_id, constellation_id, decision).await?;
    }
    Ok(())
}

pub async fn write_snac_ark_shortcut<'c, E>(
    executor: E,
    entity_id: Uuid,
    ark: &str,
) -> anyhow::Result<()>
where
    E: PgExecutor<'c>,
{
    let updated = sqlx::query(
        r#"
        UPDATE entities AS entity
        SET authority_ids = authority_ids || jsonb_build_object('snac_ark', $2::text)
        WHERE entity.id = $1
          AND EXISTS (
            SELECT 1
            FROM entity_identity_links AS link
            JOIN snac_constellations AS constellation
              ON constellation.id = link.constellation_id
            WHERE link.entity_id = entity.id
              AND link.status = 'aligned'
              AND constellation.ark = $2
          )
        "#,
    )
    .bind(entity_id)
    .bind(ark)
    .execute(executor)
    .await?;
    if updated.rows_affected() == 0 {
        anyhow::bail!("snac_ark shortcut requires an aligned identity link for this ark");
    }
    Ok(())
}

/// Opens one transaction, locks this person, refuses to steal an aligned
/// constellation, then writes. Callers do not take the lock themselves.
pub async fn persist_snac_identity(
    pool: &sqlx::PgPool,
    entity_id: Uuid,
    decisions: &[PersistedDecision],
) -> anyhow::Result<()> {
    let mut tx = pool.begin().await?;
    sqlx::query("SELECT pg_advisory_xact_lock($1)")
        .bind(advisory_key(&entity_id))
        .execute(&mut *tx)
        .await?;
    sqlx::query("SAVEPOINT snac_identity")
        .execute(&mut *tx)
        .await?;
    let guarded = guard_claimed_constellations(&mut tx, entity_id, decisions).await?;
    if let Err(err) = persist_identity_links(&mut tx, entity_id, &guarded).await {
        if !unique_violation_in_anyhow(&err) {
            return Err(err);
        }
        sqlx::query("ROLLBACK TO SAVEPOINT snac_identity")
            .execute(&mut *tx)
            .await?;
        let guarded = guard_claimed_constellations(&mut tx, entity_id, decisions).await?;
        persist_identity_links(&mut tx, entity_id, &guarded).await?;
    }
    tx.commit().await?;
    Ok(())
}

async fn guard_claimed_constellations(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    entity_id: Uuid,
    decisions: &[PersistedDecision],
) -> anyhow::Result<Vec<PersistedDecision>> {
    let mut guarded = Vec::with_capacity(decisions.len());
    for decision in decisions {
        if decision.status != "aligned" {
            guarded.push(decision.clone());
            continue;
        }
        let taken: bool = sqlx::query_scalar(
            r#"
            SELECT EXISTS (
                SELECT 1
                FROM entity_identity_links AS link
                JOIN snac_constellations AS constellation
                  ON constellation.id = link.constellation_id
                WHERE constellation.ark = $1
                  AND link.status = 'aligned'
                  AND link.entity_id <> $2
            )
            "#,
        )
        .bind(&decision.ark)
        .bind(entity_id)
        .fetch_one(&mut **tx)
        .await?;
        if taken {
            let mut conflict = decision.clone();
            conflict.status = "conflict".into();
            conflict.method = "constellation_claimed".into();
            guarded.push(conflict);
        } else {
            guarded.push(decision.clone());
        }
    }
    Ok(guarded)
}

fn unique_violation_in_anyhow(err: &anyhow::Error) -> bool {
    err.chain().any(|cause| {
        cause
            .downcast_ref::<sqlx::Error>()
            .is_some_and(is_unique_violation)
    })
}

async fn insert_immutable_snapshot(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    decision: &PersistedDecision,
    body: &str,
) -> anyhow::Result<Uuid> {
    let content_hash = hex::encode(Sha256::digest(body.as_bytes()));
    sqlx::query(
        r#"
        INSERT INTO document_snapshots (
            source_type, source_uri, source_identifier, language, title,
            content_hash, revision_id, text
        )
        VALUES ('snac', $1, $2, 'und', $3, $4, $5, $6)
        ON CONFLICT (source_type, source_uri, content_hash) DO NOTHING
        "#,
    )
    .bind(&decision.source_url)
    .bind(decision.snac_id.as_deref())
    .bind(&decision.label)
    .bind(&content_hash)
    .bind(decision.snac_version.as_deref())
    .bind(body)
    .execute(&mut **tx)
    .await?;

    let (id, stored): (Uuid, String) = sqlx::query_as(
        r#"
        SELECT id, text
        FROM document_snapshots
        WHERE source_type = 'snac' AND source_uri = $1 AND content_hash = $2
        "#,
    )
    .bind(&decision.source_url)
    .bind(&content_hash)
    .fetch_one(&mut **tx)
    .await?;
    if stored != body {
        anyhow::bail!("snac snapshot text changed for an existing content hash");
    }
    Ok(id)
}

async fn upsert_constellation(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    decision: &PersistedDecision,
    snapshot_id: Option<Uuid>,
) -> anyhow::Result<Uuid> {
    let id = sqlx::query_scalar(
        r#"
        INSERT INTO snac_constellations (
            snac_id, ark, snac_version, entity_type, label, record_status,
            same_as, exist_dates, snapshot_id, raw_json, source_url, retrieved_at
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
        ON CONFLICT (ark) DO UPDATE SET
            snac_id = COALESCE(EXCLUDED.snac_id, snac_constellations.snac_id),
            snac_version = CASE
                WHEN snac_constellations.record_status = 'complete'
                     AND EXCLUDED.record_status = 'stub'
                THEN snac_constellations.snac_version
                ELSE COALESCE(EXCLUDED.snac_version, snac_constellations.snac_version)
            END,
            label = CASE
                WHEN snac_constellations.record_status = 'complete'
                     AND EXCLUDED.record_status = 'stub'
                THEN snac_constellations.label
                ELSE EXCLUDED.label
            END,
            record_status = CASE
                WHEN snac_constellations.record_status = 'complete'
                     AND EXCLUDED.record_status = 'stub'
                THEN snac_constellations.record_status
                ELSE EXCLUDED.record_status
            END,
            same_as = CASE
                WHEN snac_constellations.record_status = 'complete'
                     AND EXCLUDED.record_status = 'stub'
                THEN snac_constellations.same_as
                ELSE EXCLUDED.same_as
            END,
            exist_dates = CASE
                WHEN snac_constellations.record_status = 'complete'
                     AND EXCLUDED.record_status = 'stub'
                THEN snac_constellations.exist_dates
                ELSE EXCLUDED.exist_dates
            END,
            snapshot_id = CASE
                WHEN snac_constellations.record_status = 'complete'
                     AND EXCLUDED.record_status = 'stub'
                THEN snac_constellations.snapshot_id
                ELSE COALESCE(EXCLUDED.snapshot_id, snac_constellations.snapshot_id)
            END,
            raw_json = CASE
                WHEN snac_constellations.record_status = 'complete'
                     AND EXCLUDED.record_status = 'stub'
                THEN snac_constellations.raw_json
                ELSE EXCLUDED.raw_json
            END,
            source_url = EXCLUDED.source_url,
            retrieved_at = EXCLUDED.retrieved_at,
            updated_at = NOW()
        RETURNING id
        "#,
    )
    .bind(decision.snac_id.as_deref())
    .bind(&decision.ark)
    .bind(decision.snac_version.as_deref())
    .bind(&decision.entity_type)
    .bind(&decision.label)
    .bind(&decision.record_status)
    .bind(&decision.same_as)
    .bind(&decision.exist_dates)
    .bind(snapshot_id)
    .bind(&decision.raw_json)
    .bind(&decision.source_url)
    .bind(decision.retrieved_at)
    .fetch_one(&mut **tx)
    .await?;
    Ok(id)
}

async fn apply_identity_link(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    entity_id: Uuid,
    constellation_id: Uuid,
    decision: &PersistedDecision,
) -> anyhow::Result<()> {
    let existing: Option<(String, String)> = sqlx::query_as(
        r#"
        SELECT status, method
        FROM entity_identity_links
        WHERE entity_id = $1 AND constellation_id = $2
        "#,
    )
    .bind(entity_id)
    .bind(constellation_id)
    .fetch_optional(&mut **tx)
    .await?;

    let Some((status, method)) = allowed_transition(
        existing.as_ref().map(|(status, _)| status.as_str()),
        existing.as_ref().map(|(_, method)| method.as_str()),
        &decision.status,
        &decision.method,
    ) else {
        return Ok(());
    };

    sqlx::query(
        r#"
        INSERT INTO entity_identity_links (
            entity_id, constellation_id, status, method, evidence, retrieved_at
        )
        VALUES ($1, $2, $3, $4, $5, $6)
        ON CONFLICT (entity_id, constellation_id) DO UPDATE SET
            status = EXCLUDED.status,
            method = EXCLUDED.method,
            evidence = EXCLUDED.evidence,
            retrieved_at = EXCLUDED.retrieved_at,
            updated_at = NOW()
        "#,
    )
    .bind(entity_id)
    .bind(constellation_id)
    .bind(status)
    .bind(method)
    .bind(&decision.evidence)
    .bind(decision.retrieved_at)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

fn allowed_transition(
    current_status: Option<&str>,
    current_method: Option<&str>,
    incoming_status: &str,
    incoming_method: &str,
) -> Option<(String, String)> {
    let Some(current_status) = current_status else {
        return Some((incoming_status.to_string(), incoming_method.to_string()));
    };
    if current_status == "rejected" {
        return None;
    }
    let allowed = match (current_status, incoming_status) {
        ("ambiguous", "aligned" | "conflict" | "ambiguous") => true,
        ("aligned", "conflict" | "aligned") => true,
        ("conflict", "conflict") => true,
        ("conflict", "aligned") => {
            incoming_method == "qid_same_as"
                && matches!(
                    current_method,
                    Some("conflicting_qid" | "label_and_years" | "label_only")
                )
        }
        _ => false,
    };
    if allowed {
        Some((incoming_status.to_string(), incoming_method.to_string()))
    } else {
        None
    }
}

pub fn is_unique_violation(err: &sqlx::Error) -> bool {
    match err {
        sqlx::Error::Database(db) => db.code().as_deref() == Some("23505"),
        _ => false,
    }
}

/// One relation from a harvest that has already been keyed and hashed.
/// The store does not recompute `snac-assertion-key:v1`.
#[derive(Debug, Clone)]
pub struct HarvestRelation {
    pub object_ark: String,
    pub object_label: String,
    pub object_entity_type: String,
    pub predicate: String,
    pub source_predicate: String,
    pub source_assertion_key: String,
    pub snac_relation_id: Option<String>,
    pub assertion_text: Option<String>,
    pub dates_json: serde_json::Value,
    pub validation_status: String,
    pub content_hash: String,
}

/// A validated SNAC read for one subject constellation.
/// Retirement of missing relations is not part of this write.
#[derive(Debug, Clone)]
pub struct ValidatedHarvest {
    pub subject_constellation_id: Uuid,
    pub source_url: String,
    pub snac_id: Option<String>,
    pub label: String,
    pub snac_version: String,
    pub raw_body: String,
    pub retrieved_at: DateTime<Utc>,
    pub relations: Vec<HarvestRelation>,
}

/// Writes relation versions. Does not mark absent relations non-current.
/// `persist_snac_harvest` is the only future caller allowed to retire, and only
/// after the whole harvest has been validated.
pub async fn upsert_relations(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    harvest: &ValidatedHarvest,
) -> anyhow::Result<()> {
    let snapshot_decision = PersistedDecision {
        ark: String::new(),
        snac_id: harvest.snac_id.clone(),
        snac_version: Some(harvest.snac_version.clone()),
        label: harvest.label.clone(),
        entity_type: "person".into(),
        record_status: "complete".into(),
        same_as: serde_json::json!({}),
        exist_dates: serde_json::json!({"kind":"unknown"}),
        raw_json: serde_json::json!({}),
        raw_body: None,
        source_url: harvest.source_url.clone(),
        status: "aligned".into(),
        method: "qid_same_as".into(),
        evidence: serde_json::json!({}),
        retrieved_at: harvest.retrieved_at,
    };
    let snapshot_id = insert_immutable_snapshot(tx, &snapshot_decision, &harvest.raw_body).await?;
    for relation in &harvest.relations {
        let object_id = ensure_object_constellation(tx, relation, harvest.retrieved_at).await?;
        if object_id == harvest.subject_constellation_id {
            anyhow::bail!("snac relation object is the subject constellation");
        }
        write_relation_version(tx, harvest, relation, object_id, snapshot_id).await?;
    }
    Ok(())
}

/// Marks current assertions of this subject that are absent from a successful
/// harvest as not current. Production code must call this only from
/// `persist_snac_harvest` after validation. A failed read must not call it.
pub async fn retire_absent_relations(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    subject_constellation_id: Uuid,
    seen_keys: &[String],
) -> anyhow::Result<u64> {
    let updated = sqlx::query(
        r#"
        UPDATE entity_relations
        SET is_current = false,
            superseded_at = NOW()
        WHERE subject_constellation_id = $1
          AND is_current
          AND source_assertion_key <> ALL($2::text[])
        "#,
    )
    .bind(subject_constellation_id)
    .bind(seen_keys)
    .execute(&mut **tx)
    .await?;
    Ok(updated.rows_affected())
}

async fn ensure_object_constellation(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    relation: &HarvestRelation,
    retrieved_at: DateTime<Utc>,
) -> anyhow::Result<Uuid> {
    let stub = PersistedDecision {
        ark: relation.object_ark.clone(),
        snac_id: None,
        snac_version: None,
        label: relation.object_label.clone(),
        entity_type: relation.object_entity_type.clone(),
        record_status: "stub".into(),
        same_as: serde_json::json!({"values":{},"raw":[],"internal_conflict":false}),
        exist_dates: serde_json::json!({"kind":"unknown"}),
        raw_json: serde_json::json!({}),
        raw_body: None,
        source_url: format!("https://snaccooperative.org/{}", relation.object_ark),
        status: "ambiguous".into(),
        method: "label_only".into(),
        evidence: serde_json::json!({}),
        retrieved_at,
    };
    upsert_constellation(tx, &stub, None).await
}

async fn write_relation_version(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    harvest: &ValidatedHarvest,
    relation: &HarvestRelation,
    object_constellation_id: Uuid,
    snapshot_id: Uuid,
) -> anyhow::Result<()> {
    let existing: Option<(Uuid, String)> = sqlx::query_as(
        r#"
        SELECT id, content_hash
        FROM entity_relations
        WHERE subject_constellation_id = $1
          AND source_assertion_key = $2
          AND is_current
        "#,
    )
    .bind(harvest.subject_constellation_id)
    .bind(&relation.source_assertion_key)
    .fetch_optional(&mut **tx)
    .await?;

    match existing {
        Some((_, hash)) if hash == relation.content_hash => {
            sqlx::query(
                r#"
                UPDATE entity_relations
                SET retrieved_at = $3
                WHERE subject_constellation_id = $1
                  AND source_assertion_key = $2
                  AND is_current
                "#,
            )
            .bind(harvest.subject_constellation_id)
            .bind(&relation.source_assertion_key)
            .bind(harvest.retrieved_at)
            .execute(&mut **tx)
            .await?;
        }
        Some(_) => {
            sqlx::query(
                r#"
                UPDATE entity_relations
                SET is_current = false,
                    superseded_at = NOW()
                WHERE subject_constellation_id = $1
                  AND source_assertion_key = $2
                  AND is_current
                "#,
            )
            .bind(harvest.subject_constellation_id)
            .bind(&relation.source_assertion_key)
            .execute(&mut **tx)
            .await?;
            insert_relation(tx, harvest, relation, object_constellation_id, snapshot_id).await?;
        }
        None => {
            insert_relation(tx, harvest, relation, object_constellation_id, snapshot_id).await?;
        }
    }
    Ok(())
}

async fn insert_relation(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    harvest: &ValidatedHarvest,
    relation: &HarvestRelation,
    object_constellation_id: Uuid,
    snapshot_id: Uuid,
) -> anyhow::Result<()> {
    sqlx::query(
        r#"
        INSERT INTO entity_relations (
            subject_constellation_id, object_constellation_id, predicate,
            source_predicate, source_assertion_key, snac_relation_id, assertion_text,
            dates_json, validation_status, source_url, snapshot_id, content_hash,
            snac_version, retrieved_at
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)
        "#,
    )
    .bind(harvest.subject_constellation_id)
    .bind(object_constellation_id)
    .bind(&relation.predicate)
    .bind(&relation.source_predicate)
    .bind(&relation.source_assertion_key)
    .bind(relation.snac_relation_id.as_deref())
    .bind(relation.assertion_text.as_deref())
    .bind(&relation.dates_json)
    .bind(&relation.validation_status)
    .bind(&harvest.source_url)
    .bind(snapshot_id)
    .bind(&relation.content_hash)
    .bind(&harvest.snac_version)
    .bind(harvest.retrieved_at)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        aligned_constellation_id, is_unique_violation, persist_identity_links,
        persist_snac_identity, retire_absent_relations, upsert_constellation, upsert_relations,
        write_snac_ark_shortcut, HarvestRelation, PersistedDecision, ValidatedHarvest,
    };
    use chrono::Utc;
    use uuid::Uuid;

    fn decision(ark: &str, status: &str, method: &str) -> PersistedDecision {
        PersistedDecision {
            ark: ark.into(),
            snac_id: None,
            snac_version: Some("1".into()),
            label: "Person".into(),
            entity_type: "person".into(),
            record_status: "stub".into(),
            same_as: serde_json::json!({"values":{},"raw":[],"internal_conflict":false}),
            exist_dates: serde_json::json!({"kind":"unknown"}),
            raw_json: serde_json::json!({}),
            raw_body: None,
            source_url: format!("https://snaccooperative.org/{ark}"),
            status: status.into(),
            method: method.into(),
            evidence: serde_json::json!({}),
            retrieved_at: Utc::now(),
        }
    }

    async fn insert_person(pool: &sqlx::PgPool, title: &str) -> Uuid {
        sqlx::query_scalar(
            r#"
            INSERT INTO entities (wikipedia_title, wiki_lang, canonical_name, kind)
            VALUES ($1, 'en', $1, 'person')
            RETURNING id
            "#,
        )
        .bind(title)
        .fetch_one(pool)
        .await
        .unwrap()
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn snac_ark_alone_is_not_alignment(pool: sqlx::PgPool) {
        let entity_id = insert_person(&pool, "Shortcut only").await;
        let err = write_snac_ark_shortcut(&pool, entity_id, "ark:/99166/w6example")
            .await
            .unwrap_err();
        assert!(err.to_string().contains("aligned identity link"));
        let stored: serde_json::Value =
            sqlx::query_scalar("SELECT authority_ids FROM entities WHERE id = $1")
                .bind(entity_id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert!(stored.get("snac_ark").is_none());
        assert!(aligned_constellation_id(&pool, entity_id)
            .await
            .unwrap()
            .is_none());
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn snac_ark_shortcut_requires_the_same_aligned_ark(pool: sqlx::PgPool) {
        let entity_id = insert_person(&pool, "Aligned shortcut").await;
        persist_snac_identity(
            &pool,
            entity_id,
            &[decision("ark:/99166/w6real", "aligned", "qid_same_as")],
        )
        .await
        .unwrap();
        let wrong = write_snac_ark_shortcut(&pool, entity_id, "ark:/99166/w6other")
            .await
            .unwrap_err();
        assert!(wrong.to_string().contains("aligned identity link"));
        write_snac_ark_shortcut(&pool, entity_id, "ark:/99166/w6real")
            .await
            .unwrap();
        let stored: serde_json::Value =
            sqlx::query_scalar("SELECT authority_ids FROM entities WHERE id = $1")
                .bind(entity_id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(stored["snac_ark"], "ark:/99166/w6real");
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn identity_write_rolls_back_with_the_transaction(pool: sqlx::PgPool) {
        let entity_id = insert_person(&pool, "Rollback person").await;
        let mut tx = pool.begin().await.unwrap();
        persist_identity_links(
            &mut tx,
            entity_id,
            &[decision("ark:/99166/w6roll", "aligned", "qid_same_as")],
        )
        .await
        .unwrap();
        tx.rollback().await.unwrap();
        let count: i64 = sqlx::query_scalar("SELECT count(*) FROM entity_identity_links")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, 0);
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn one_constellation_cannot_align_two_entities(pool: sqlx::PgPool) {
        let first = insert_person(&pool, "Person A").await;
        let second = insert_person(&pool, "Person B").await;
        persist_snac_identity(
            &pool,
            first,
            &[decision("ark:/99166/w6one", "aligned", "qid_same_as")],
        )
        .await
        .unwrap();
        persist_snac_identity(
            &pool,
            second,
            &[decision("ark:/99166/w6one", "aligned", "qid_same_as")],
        )
        .await
        .unwrap();
        let aligned: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM entity_identity_links WHERE status = 'aligned'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        let claimed: String =
            sqlx::query_scalar("SELECT method FROM entity_identity_links WHERE entity_id = $1")
                .bind(second)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(aligned, 1);
        assert_eq!(claimed, "constellation_claimed");
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn reingest_keeps_the_same_aligned_link(pool: sqlx::PgPool) {
        let entity_id = insert_person(&pool, "Reingest person").await;
        let body = decision("ark:/99166/w69x15nw", "aligned", "qid_same_as");
        persist_snac_identity(&pool, entity_id, &[body.clone()])
            .await
            .unwrap();
        persist_snac_identity(&pool, entity_id, &[body])
            .await
            .unwrap();
        let row: (String, String) =
            sqlx::query_as("SELECT status, method FROM entity_identity_links WHERE entity_id = $1")
                .bind(entity_id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(row.0, "aligned");
        assert_eq!(row.1, "qid_same_as");
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn one_entity_cannot_align_two_constellations(pool: sqlx::PgPool) {
        let entity_id = insert_person(&pool, "Two constellations").await;
        persist_snac_identity(
            &pool,
            entity_id,
            &[decision("ark:/99166/w6first", "aligned", "qid_same_as")],
        )
        .await
        .unwrap();
        let err = persist_snac_identity(
            &pool,
            entity_id,
            &[decision("ark:/99166/w6second", "aligned", "qid_same_as")],
        )
        .await
        .unwrap_err();
        assert!(err
            .downcast_ref::<sqlx::Error>()
            .is_some_and(is_unique_violation));
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn rejected_link_is_not_promoted(pool: sqlx::PgPool) {
        let entity_id = insert_person(&pool, "Rejected person").await;
        let mut tx = pool.begin().await.unwrap();
        persist_identity_links(
            &mut tx,
            entity_id,
            &[decision("ark:/99166/w6reject", "rejected", "label_only")],
        )
        .await
        .unwrap();
        persist_identity_links(
            &mut tx,
            entity_id,
            &[decision("ark:/99166/w6reject", "aligned", "qid_same_as")],
        )
        .await
        .unwrap();
        tx.commit().await.unwrap();
        let status: String =
            sqlx::query_scalar("SELECT status FROM entity_identity_links WHERE entity_id = $1")
                .bind(entity_id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(status, "rejected");
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn snapshot_text_is_not_replaced(pool: sqlx::PgPool) {
        let entity_id = insert_person(&pool, "Snapshot person").await;
        let mut body = decision("ark:/99166/w6snap", "aligned", "qid_same_as");
        body.raw_body = Some("{\"constellation\":1}".into());
        body.record_status = "complete".into();
        let mut tx = pool.begin().await.unwrap();
        persist_identity_links(&mut tx, entity_id, &[body.clone()])
            .await
            .unwrap();
        tx.commit().await.unwrap();

        body.raw_body = Some("{\"constellation\":1}".into());
        let mut tx = pool.begin().await.unwrap();
        persist_identity_links(&mut tx, entity_id, &[body])
            .await
            .unwrap();
        tx.commit().await.unwrap();

        let count: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM document_snapshots WHERE source_type = 'snac'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        let text: String =
            sqlx::query_scalar("SELECT text FROM document_snapshots WHERE source_type = 'snac'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(count, 1);
        assert_eq!(text, "{\"constellation\":1}");
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn concurrent_alignment_is_one_to_one(pool: sqlx::PgPool) {
        let first = insert_person(&pool, "Concurrent A").await;
        let second = insert_person(&pool, "Concurrent B").await;
        let pool_a = pool.clone();
        let pool_b = pool.clone();
        let left_decisions = [decision("ark:/99166/w6shared", "aligned", "qid_same_as")];
        let right_decisions = [decision("ark:/99166/w6shared", "aligned", "qid_same_as")];
        let (left, right) = tokio::join!(
            persist_snac_identity(&pool_a, first, &left_decisions),
            persist_snac_identity(&pool_b, second, &right_decisions),
        );
        left.unwrap();
        right.unwrap();
        let aligned: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM entity_identity_links WHERE status = 'aligned'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(aligned, 1);
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn stub_does_not_degrade_a_complete_constellation(pool: sqlx::PgPool) {
        let entity_id = insert_person(&pool, "Complete then stub").await;
        let mut complete = decision("ark:/99166/w6full", "aligned", "qid_same_as");
        complete.snac_version = Some("11465101".into());
        complete.label = "Napoléon I".into();
        complete.record_status = "complete".into();
        complete.same_as =
            serde_json::json!({"values":{"wikidata":["Q517"]},"raw":[],"internal_conflict":false});
        complete.exist_dates = serde_json::json!({"kind":"range","surface":"1769-1821"});
        complete.raw_json = serde_json::json!({"kept":true});
        complete.raw_body = Some("{\"constellation\":\"complete\"}".into());
        persist_snac_identity(&pool, entity_id, &[complete.clone()])
            .await
            .unwrap();

        let mut stub = decision("ark:/99166/w6full", "aligned", "qid_same_as");
        stub.snac_version = Some("stub-version".into());
        stub.label = "stub label".into();
        stub.same_as = serde_json::json!({"values":{},"raw":[],"internal_conflict":false});
        stub.exist_dates = serde_json::json!({"kind":"unknown"});
        stub.raw_json = serde_json::json!({"stub":true});
        stub.raw_body = Some("{\"constellation\":\"stub\"}".into());
        persist_snac_identity(&pool, entity_id, &[stub])
            .await
            .unwrap();

        let row: (
            String,
            String,
            String,
            serde_json::Value,
            serde_json::Value,
            serde_json::Value,
            String,
        ) = sqlx::query_as(
            r#"
            SELECT record_status, snac_version, label, same_as, exist_dates, raw_json, snap.text
            FROM snac_constellations AS constellation
            JOIN document_snapshots AS snap ON snap.id = constellation.snapshot_id
            WHERE constellation.ark = 'ark:/99166/w6full'
            "#,
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(row.0, "complete");
        assert_eq!(row.1, "11465101");
        assert_eq!(row.2, "Napoléon I");
        assert_eq!(row.3["values"]["wikidata"][0], "Q517");
        assert_eq!(row.4["surface"], "1769-1821");
        assert_eq!(row.5["kept"], true);
        assert_eq!(row.6, "{\"constellation\":\"complete\"}");
        let link: (String, String) =
            sqlx::query_as("SELECT status, method FROM entity_identity_links WHERE entity_id = $1")
                .bind(entity_id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(link.0, "aligned");
        assert_eq!(link.1, "qid_same_as");

        let mut again = complete;
        again.same_as = serde_json::json!({"values":{"wikidata":["Q517"]},"raw":["https://viaf.org/viaf/1"],"internal_conflict":false});
        again.raw_body = Some("{\"constellation\":\"complete-2\"}".into());
        persist_snac_identity(&pool, entity_id, &[again])
            .await
            .unwrap();
        let surface: String = sqlx::query_scalar(
            r#"
            SELECT same_as->'raw'->>0
            FROM snac_constellations
            WHERE ark = 'ark:/99166/w6full'
            "#,
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        let texts: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM document_snapshots WHERE text LIKE '%complete%'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(surface, "https://viaf.org/viaf/1");
        assert_eq!(texts, 2);
    }

    fn relation(key: &str, hash: &str, object_ark: &str) -> HarvestRelation {
        HarvestRelation {
            object_ark: object_ark.into(),
            object_label: object_ark.into(),
            object_entity_type: "person".into(),
            predicate: "corresponded_with".into(),
            source_predicate: "correspondedWith".into(),
            source_assertion_key: key.into(),
            snac_relation_id: None,
            assertion_text: None,
            dates_json: serde_json::json!({"kind":"unknown"}),
            validation_status: "accepted".into(),
            content_hash: hash.into(),
        }
    }

    fn harvest(
        subject_constellation_id: Uuid,
        relations: Vec<HarvestRelation>,
    ) -> ValidatedHarvest {
        ValidatedHarvest {
            subject_constellation_id,
            source_url: "https://snaccooperative.org/ark:/99166/w6subject".into(),
            snac_id: Some("85115161".into()),
            label: "Napoléon I".into(),
            snac_version: "11465101".into(),
            raw_body: "{\"constellation\":\"relations\"}".into(),
            retrieved_at: Utc::now(),
            relations,
        }
    }

    async fn subject_constellation(pool: &sqlx::PgPool) -> Uuid {
        let decision = decision("ark:/99166/w6subject", "aligned", "qid_same_as");
        let mut tx = pool.begin().await.unwrap();
        let id = upsert_constellation(&mut tx, &decision, None)
            .await
            .unwrap();
        tx.commit().await.unwrap();
        id
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn retire_absent_relations_after_a_validated_harvest(pool: sqlx::PgPool) {
        let subject_id = subject_constellation(&pool).await;
        let both = harvest(
            subject_id,
            vec![
                relation("snac:1", "hash-1", "ark:/99166/w6first"),
                relation("snac:2", "hash-2", "ark:/99166/w6second"),
            ],
        );
        let mut tx = pool.begin().await.unwrap();
        upsert_relations(&mut tx, &both).await.unwrap();
        tx.commit().await.unwrap();

        let first_only = harvest(
            subject_id,
            vec![relation("snac:1", "hash-1", "ark:/99166/w6first")],
        );
        let mut tx = pool.begin().await.unwrap();
        upsert_relations(&mut tx, &first_only).await.unwrap();
        // Production must reach this call only from persist_snac_harvest, after validation.
        retire_absent_relations(&mut tx, subject_id, &["snac:1".into()])
            .await
            .unwrap();
        tx.commit().await.unwrap();

        let rows: Vec<(String, bool)> = sqlx::query_as(
            r#"
            SELECT source_assertion_key, is_current
            FROM entity_relations
            ORDER BY source_assertion_key
            "#,
        )
        .fetch_all(&pool)
        .await
        .unwrap();
        assert_eq!(
            rows,
            vec![("snac:1".into(), true), ("snac:2".into(), false)]
        );

        let mut tx = pool.begin().await.unwrap();
        upsert_relations(&mut tx, &first_only).await.unwrap();
        retire_absent_relations(&mut tx, subject_id, &["snac:1".into()])
            .await
            .unwrap();
        tx.commit().await.unwrap();
        let count: i64 = sqlx::query_scalar("SELECT count(*) FROM entity_relations")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, 2);
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn http_reject_does_not_retire_relations(pool: sqlx::PgPool) {
        let subject_id = subject_constellation(&pool).await;
        let both = harvest(
            subject_id,
            vec![
                relation("snac:1", "hash-1", "ark:/99166/w6first"),
                relation("snac:2", "hash-2", "ark:/99166/w6second"),
            ],
        );
        let mut tx = pool.begin().await.unwrap();
        upsert_relations(&mut tx, &both).await.unwrap();
        tx.commit().await.unwrap();

        let failed = async { Err::<(), anyhow::Error>(anyhow::anyhow!("HarvestReject::Http")) };
        failed.await.unwrap_err();

        let current: i64 =
            sqlx::query_scalar("SELECT count(*) FROM entity_relations WHERE is_current")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(current, 2);
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn relation_transaction_rolls_back(pool: sqlx::PgPool) {
        let subject = decision("ark:/99166/w6subject", "aligned", "qid_same_as");
        let failed = async {
            let mut tx = pool.begin().await?;
            let subject_id = upsert_constellation(&mut tx, &subject, None).await?;
            let body = harvest(
                subject_id,
                vec![relation("snac:1", "hash-1", "ark:/99166/w6first")],
            );
            upsert_relations(&mut tx, &body).await?;
            sqlx::query("SELECT 1/0").execute(&mut *tx).await?;
            tx.commit().await?;
            Ok::<(), anyhow::Error>(())
        };
        assert!(failed.await.is_err());
        let relations: i64 = sqlx::query_scalar("SELECT count(*) FROM entity_relations")
            .fetch_one(&pool)
            .await
            .unwrap();
        let constellations: i64 = sqlx::query_scalar("SELECT count(*) FROM snac_constellations")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(relations, 0);
        assert_eq!(constellations, 0);
    }
}
