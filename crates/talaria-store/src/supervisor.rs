// crates/talaria-store/src/supervisor.rs
//! Append-only supervision journal. Tasks cannot record a write.

use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use uuid::Uuid;

const SITE_KINDS: &[&str] = &[
    "building",
    "museum",
    "monument",
    "memorial",
    "burial_site",
    "residence",
    "bridge",
    "cultural_institution",
];

pub async fn record_supervisor_run(
    pool: &PgPool,
    experience: &str,
    subject_qid: &str,
    subject_label: &str,
    provider: &str,
    model: &str,
    prompt_version: &str,
    plan_json: &Value,
    execute: bool,
    site_qid: Option<&str>,
    site_label: Option<&str>,
    lat: Option<f64>,
    lon: Option<f64>,
    site_kind: &str,
) -> anyhow::Result<Value> {
    let mode = if execute { "execute" } else { "read_only" };
    let hash = hex::encode(Sha256::digest(
        serde_json::to_vec(&(
            experience,
            prompt_version,
            subject_qid,
            subject_label,
            mode,
            site_qid.unwrap_or(""),
            lat,
            lon,
        ))
        .expect("hash input serializes"),
    ));
    let inserted = sqlx::query_scalar::<_, Uuid>(
        r#"
        INSERT INTO supervisor_runs (
            experience, mode, subject_qid, subject_label, provider, model,
            prompt_version, input_hash, plan_json, status
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
        ON CONFLICT (experience, prompt_version, input_hash) DO NOTHING
        RETURNING id
        "#,
    )
    .bind(experience)
    .bind(mode)
    .bind(subject_qid)
    .bind(subject_label)
    .bind(provider)
    .bind(model)
    .bind(prompt_version)
    .bind(&hash)
    .bind(plan_json)
    .bind(if execute { "completed" } else { "planned" })
    .fetch_optional(pool)
    .await?;

    let (run_id, idempotent) = if let Some(id) = inserted {
        (id, false)
    } else {
        let id = sqlx::query_scalar::<_, Uuid>(
            r#"
            SELECT id FROM supervisor_runs
            WHERE experience = $1 AND prompt_version = $2 AND input_hash = $3
            "#,
        )
        .bind(experience)
        .bind(prompt_version)
        .bind(&hash)
        .fetch_one(pool)
        .await?;
        (id, true)
    };

    let mut quarantine_site_id: Option<Uuid> = None;
    let mut site_status: Option<String> = None;
    if !idempotent {
        let tasks = plan_json
            .get("tasks")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        for task in tasks {
            let kind = task.get("kind").and_then(Value::as_str).unwrap_or("unknown");
            let key = task
                .get("key")
                .and_then(Value::as_str)
                .unwrap_or(kind)
                .to_string();
            let (status, detail) = task_outcome(
                pool,
                experience,
                execute,
                kind,
                site_qid,
                site_label,
                lat,
                lon,
                site_kind,
                &mut quarantine_site_id,
                &mut site_status,
            )
            .await?;
            sqlx::query(
                r#"
                INSERT INTO supervisor_tasks (run_id, task_key, kind, status, writes, detail_json)
                VALUES ($1, $2, $3, $4, FALSE, $5)
                ON CONFLICT (run_id, task_key) DO NOTHING
                "#,
            )
            .bind(run_id)
            .bind(key)
            .bind(kind)
            .bind(status)
            .bind(detail)
            .execute(pool)
            .await?;
        }
    }

    let canonical = sqlx::query_scalar::<_, i64>("SELECT count(*)::bigint FROM canonical_events")
        .fetch_one(pool)
        .await?;
    Ok(json!({
        "run_id": run_id,
        "idempotent": idempotent,
        "mode": mode,
        "publication": "forbidden",
        "canonical_events": canonical,
        "quarantine_site_id": quarantine_site_id,
        "site_publication_status": site_status,
    }))
}

async fn task_outcome(
    pool: &PgPool,
    experience: &str,
    execute: bool,
    kind: &str,
    site_qid: Option<&str>,
    site_label: Option<&str>,
    lat: Option<f64>,
    lon: Option<f64>,
    site_kind: &str,
    quarantine_site_id: &mut Option<Uuid>,
    site_status: &mut Option<String>,
) -> anyhow::Result<(&'static str, Value)> {
    if !execute {
        return Ok(("queued", json!({"reason": "read_only"})));
    }
    if experience != "poreia" || kind != "discover_heritage_sites" {
        return Ok(("refused", json!({"reason": "canonical_write_and_publication_forbidden"})));
    }
    let Some(qid) = site_qid.filter(|value| valid_qid(value)) else {
        return Ok(("refused", json!({"reason": "site_qid_required"})));
    };
    let Some(label) = site_label.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(("refused", json!({"reason": "site_label_required"})));
    };
    if !SITE_KINDS.contains(&site_kind) {
        return Ok(("refused", json!({"reason": "invalid_site_kind"})));
    }
    let (Some(lat), Some(lon)) = (lat, lon) else {
        return Ok(("refused", json!({"reason": "coordinates_required"})));
    };
    if !lat.is_finite() || !lon.is_finite() || !(-90.0..=90.0).contains(&lat) || !(-180.0..=180.0).contains(&lon)
    {
        return Ok(("refused", json!({"reason": "invalid_coordinates"})));
    }
    let row = sqlx::query_as::<_, (Uuid, String)>(
        r#"
        INSERT INTO visit_sites (
            wikidata_qid, label, kind, geom, publication_status, visitability,
            retrieved_at, created_at, updated_at
        )
        VALUES (
            $1, $2, $3, ST_SetSRID(ST_MakePoint($4, $5), 4326)::geography,
            'quarantine', 'unknown', NOW(), NOW(), NOW()
        )
        ON CONFLICT (wikidata_qid) DO UPDATE
        SET label = CASE
                WHEN visit_sites.publication_status = 'quarantine' THEN EXCLUDED.label
                ELSE visit_sites.label
            END,
            geom = CASE
                WHEN visit_sites.publication_status = 'quarantine' THEN EXCLUDED.geom
                ELSE visit_sites.geom
            END,
            updated_at = CASE
                WHEN visit_sites.publication_status = 'quarantine' THEN NOW()
                ELSE visit_sites.updated_at
            END
        RETURNING id, publication_status
        "#,
    )
    .bind(qid)
    .bind(label)
    .bind(site_kind)
    .bind(lon)
    .bind(lat)
    .fetch_one(pool)
    .await?;
    *quarantine_site_id = Some(row.0);
    *site_status = Some(row.1.clone());
    if row.1 != "quarantine" {
        return Ok(("refused", json!({"reason": "existing_site_not_republished", "publication_status": row.1})));
    }
    Ok(("completed", json!({"publication_status": "quarantine", "site_id": row.0})))
}

fn valid_qid(qid: &str) -> bool {
    let Some(rest) = qid.strip_prefix('Q') else {
        return false;
    };
    !rest.is_empty() && !rest.starts_with('0') && rest.bytes().all(|b| b.is_ascii_digit())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn plan() -> Value {
        json!({
            "tasks": [
                {"key": "discover_heritage_sites:Q5582", "kind": "discover_heritage_sites"},
                {"key": "flag_unverified_visit_facts:Q5582", "kind": "flag_unverified_visit_facts"}
            ]
        })
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn read_only_run_is_idempotent_and_does_not_publish(pool: PgPool) {
        let first = record_supervisor_run(
            &pool, "poreia", "Q5582", "Vincent van Gogh", "deterministic", "none",
            "supervisor:v1", &plan(), false, None, None, None, None, "monument",
        )
        .await
        .unwrap();
        let second = record_supervisor_run(
            &pool, "poreia", "Q5582", "Vincent van Gogh", "deterministic", "none",
            "supervisor:v1", &plan(), false, None, None, None, None, "monument",
        )
        .await
        .unwrap();
        assert_eq!(first["idempotent"], false);
        assert_eq!(second["idempotent"], true);
        assert_eq!(first["run_id"], second["run_id"]);
        let queued = sqlx::query_scalar::<_, i64>(
            "SELECT count(*)::bigint FROM supervisor_tasks WHERE status = 'queued'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(queued, 2);
        let sites = sqlx::query_scalar::<_, i64>("SELECT count(*)::bigint FROM visit_sites")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(sites, 0);
        let deleted = sqlx::query("DELETE FROM supervisor_runs").execute(&pool).await;
        assert!(deleted.is_err());
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn execute_without_coordinates_stays_unpublished(pool: PgPool) {
        let report = record_supervisor_run(
            &pool, "poreia", "Q5582", "Vincent van Gogh", "deterministic", "none",
            "supervisor:v1", &plan(), true, Some("Q17076145"), Some("Fondation"), None, None, "cultural_institution",
        )
        .await
        .unwrap();
        assert!(report["quarantine_site_id"].is_null());
        let refused = sqlx::query_scalar::<_, i64>(
            "SELECT count(*)::bigint FROM supervisor_tasks WHERE kind = 'discover_heritage_sites' AND status = 'refused'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(refused, 1);
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn execute_inserts_a_quarantine_site_and_leaves_history_untouched(pool: PgPool) {
        let before = sqlx::query_scalar::<_, i64>("SELECT count(*)::bigint FROM canonical_events")
            .fetch_one(&pool)
            .await
            .unwrap();
        let report = record_supervisor_run(
            &pool, "poreia", "Q5582", "Vincent van Gogh", "deterministic", "none",
            "supervisor:v1", &plan(), true, Some("Q17076145"), Some("Fondation Vincent van Gogh Arles"),
            Some(43.678131), Some(4.625651), "cultural_institution",
        )
        .await
        .unwrap();
        assert_eq!(report["site_publication_status"], "quarantine");
        assert_eq!(report["canonical_events"], before);
        let again = record_supervisor_run(
            &pool, "poreia", "Q5582", "Vincent van Gogh", "deterministic", "none",
            "supervisor:v1", &plan(), true, Some("Q17076145"), Some("Fondation Vincent van Gogh Arles"),
            Some(43.678131), Some(4.625651), "cultural_institution",
        )
        .await
        .unwrap();
        assert_eq!(again["idempotent"], true);
        let published = sqlx::query_scalar::<_, i64>(
            "SELECT count(*)::bigint FROM visit_sites WHERE publication_status = 'published'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(published, 0);
    }
}
