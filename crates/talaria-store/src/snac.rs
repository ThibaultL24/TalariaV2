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
        let snapshot_id = if let Some(body) = decision.raw_body.as_deref() {
            Some(insert_immutable_snapshot(tx, decision, body).await?)
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
    sqlx::query(
        r#"
        UPDATE entities
        SET authority_ids = authority_ids || jsonb_build_object('snac_ark', $2::text)
        WHERE id = $1
        "#,
    )
    .bind(entity_id)
    .bind(ark)
    .execute(executor)
    .await?;
    Ok(())
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
            snac_version = COALESCE(EXCLUDED.snac_version, snac_constellations.snac_version),
            label = EXCLUDED.label,
            record_status = CASE
                WHEN snac_constellations.record_status = 'complete'
                     AND EXCLUDED.record_status = 'stub'
                THEN snac_constellations.record_status
                ELSE EXCLUDED.record_status
            END,
            same_as = EXCLUDED.same_as,
            exist_dates = EXCLUDED.exist_dates,
            snapshot_id = COALESCE(EXCLUDED.snapshot_id, snac_constellations.snapshot_id),
            raw_json = EXCLUDED.raw_json,
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

#[cfg(test)]
mod tests {
    use super::{
        advisory_key, aligned_constellation_id, is_unique_violation, persist_identity_links,
        write_snac_ark_shortcut, PersistedDecision,
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

    async fn claim_aligned(pool: &sqlx::PgPool, entity_id: Uuid, ark: &str) -> anyhow::Result<()> {
        let mut tx = pool.begin().await?;
        sqlx::query("SELECT pg_advisory_xact_lock($1)")
            .bind(advisory_key(&entity_id))
            .execute(&mut *tx)
            .await?;
        sqlx::query("SAVEPOINT snac_claim")
            .execute(&mut *tx)
            .await?;
        let aligned = decision(ark, "aligned", "qid_same_as");
        if let Err(err) = persist_identity_links(&mut tx, entity_id, &[aligned]).await {
            return Err(err);
        }
        if let Err(err) = tx.commit().await {
            if is_unique_violation(&err) {
                return write_claimed(pool, entity_id, ark).await;
            }
            return Err(err.into());
        }
        Ok(())
    }

    async fn write_claimed(pool: &sqlx::PgPool, entity_id: Uuid, ark: &str) -> anyhow::Result<()> {
        let mut tx = pool.begin().await?;
        let conflict = decision(ark, "conflict", "constellation_claimed");
        persist_identity_links(&mut tx, entity_id, &[conflict]).await?;
        tx.commit().await?;
        Ok(())
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn snac_ark_alone_is_not_alignment(pool: sqlx::PgPool) {
        let entity_id = insert_person(&pool, "Shortcut only").await;
        write_snac_ark_shortcut(&pool, entity_id, "ark:/99166/w6example")
            .await
            .unwrap();
        let aligned = aligned_constellation_id(&pool, entity_id).await.unwrap();
        assert!(aligned.is_none());
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
        claim_aligned(&pool, first, "ark:/99166/w6one").await.unwrap();
        let err = claim_aligned(&pool, second, "ark:/99166/w6one")
            .await
            .unwrap_err();
        assert!(
            is_unique_violation(err.downcast_ref::<sqlx::Error>().unwrap_or(&sqlx::Error::RowNotFound)),
            "expected a unique violation, got {err}"
        );
        let aligned: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM entity_identity_links WHERE status = 'aligned'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(aligned, 1);
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn one_entity_cannot_align_two_constellations(pool: sqlx::PgPool) {
        let entity_id = insert_person(&pool, "Two constellations").await;
        claim_aligned(&pool, entity_id, "ark:/99166/w6first")
            .await
            .unwrap();
        let err = claim_aligned(&pool, entity_id, "ark:/99166/w6second")
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
        let status: String = sqlx::query_scalar(
            "SELECT status FROM entity_identity_links WHERE entity_id = $1",
        )
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
        let text: String = sqlx::query_scalar(
            "SELECT text FROM document_snapshots WHERE source_type = 'snac'",
        )
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
        let (left, right) = tokio::join!(
            claim_or_conflict(pool_a, first, "ark:/99166/w6shared"),
            claim_or_conflict(pool_b, second, "ark:/99166/w6shared"),
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

    async fn claim_or_conflict(pool: sqlx::PgPool, entity_id: Uuid, ark: &str) -> anyhow::Result<()> {
        let mut tx = pool.begin().await?;
        sqlx::query("SELECT pg_advisory_xact_lock($1)")
            .bind(advisory_key(&entity_id))
            .execute(&mut *tx)
            .await?;
        sqlx::query("SAVEPOINT snac_claim")
            .execute(&mut *tx)
            .await?;
        let inserted = persist_identity_links(
            &mut tx,
            entity_id,
            &[decision(ark, "aligned", "qid_same_as")],
        )
        .await;
        match inserted {
            Ok(()) => {
                if let Err(err) = tx.commit().await {
                    if is_unique_violation(&err) {
                        return write_claimed(&pool, entity_id, ark).await;
                    }
                    return Err(err.into());
                }
                Ok(())
            }
            Err(err) => {
                if err
                    .downcast_ref::<sqlx::Error>()
                    .is_some_and(is_unique_violation)
                {
                    return write_claimed(&pool, entity_id, ark).await;
                }
                Err(err)
            }
        }
    }
}
