// crates/talaria-store/src/supervisor.rs
//! Append-only supervision journal.
//! A site insert needs a stored document, a verifiable URL, a retrieval time, and a documented relation.

use chrono::{DateTime, Utc};
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

const SOURCE_HOSTS: &[&str] = &[
    "https://www.wikidata.org/",
    "https://commons.wikimedia.org/",
    "https://en.wikipedia.org/",
    "https://fr.wikipedia.org/",
    "https://fr.wikisource.org/",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubjectCoverage {
    pub label: String,
    pub birth_year: Option<i32>,
    pub death_year: Option<i32>,
    pub has_wikipedia: bool,
    pub wikipedia_documents: u32,
    pub wikisource_count: u32,
    pub commons_documents: u32,
    pub sourced_event_count: u32,
    pub published_site_count: u32,
}

#[derive(Debug, Clone)]
pub struct TaskEffect {
    pub key: String,
    pub kind: String,
    pub status: &'static str,
    pub writes: bool,
    pub detail: Value,
}

#[derive(Debug, Clone)]
pub struct SourcedSite {
    pub qid: String,
    pub label: String,
    pub kind: String,
    pub lat: f64,
    pub lon: f64,
    pub source_url: String,
    pub retrieved_at: DateTime<Utc>,
    pub revision_id: Option<i64>,
    pub relation_role: String,
    pub relation_pid: String,
    pub statement_id: String,
    pub document_id: Uuid,
}

#[derive(Debug, Clone)]
pub struct SupervisorCall {
    pub experience: String,
    pub subject_qid: String,
    pub subject_label: String,
    pub provider: String,
    pub model: String,
    pub prompt_version: String,
    pub plan_json: Value,
    pub execute: bool,
    pub input_tokens: i32,
    pub output_tokens: i32,
    pub cost_usd_micros: i32,
    pub cost_estimated: bool,
    pub duration_ms: i32,
    pub effects: Vec<TaskEffect>,
    pub sourced_site: Option<SourcedSite>,
}

pub async fn load_subject_coverage(pool: &PgPool, qid: &str) -> anyhow::Result<SubjectCoverage> {
    let entity = sqlx::query_as::<_, (Option<String>, Option<String>)>(
        r#"
        SELECT canonical_name, wikipedia_title
        FROM entities
        WHERE qid = $1
        ORDER BY created_at
        LIMIT 1
        "#,
    )
    .bind(qid)
    .fetch_optional(pool)
    .await?;
    let label = entity
        .as_ref()
        .and_then(|(name, title)| name.clone().or_else(|| title.clone()))
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| qid.to_string());
    let has_wikipedia = entity
        .as_ref()
        .and_then(|(_, title)| title.as_ref())
        .is_some_and(|title| !title.trim().is_empty());

    let mut wikipedia_documents = 0u32;
    let mut wikisource_count = 0u32;
    let mut commons_documents = 0u32;
    let counts = sqlx::query_as::<_, (String, i64)>(
        r#"
        SELECT source_type, count(*)::bigint
        FROM raw_documents
        WHERE source_identifier = $1
        GROUP BY source_type
        "#,
    )
    .bind(qid)
    .fetch_all(pool)
    .await?;
    for (source_type, count) in counts {
        let count = u32::try_from(count).unwrap_or(u32::MAX);
        match source_type.as_str() {
            "wikipedia" => wikipedia_documents = count,
            "wikisource" => wikisource_count = count,
            "commons" => commons_documents = count,
            _ => {}
        }
    }

    let sourced_event_count = sqlx::query_scalar::<_, i64>(
        r#"
        SELECT count(*)::bigint
        FROM canonical_events ce
        JOIN entities e ON e.id = ce.entity_id
        WHERE e.qid = $1
          AND EXISTS (
              SELECT 1 FROM event_evidence ee WHERE ee.canonical_event_id = ce.id
          )
        "#,
    )
    .bind(qid)
    .fetch_one(pool)
    .await?;

    let years = sqlx::query_as::<_, (String, Option<String>)>(
        r#"
        SELECT ce.event_type, ce.time_json->>'start'
        FROM canonical_events ce
        JOIN entities e ON e.id = ce.entity_id
        WHERE e.qid = $1 AND ce.event_type IN ('birth', 'death')
        "#,
    )
    .bind(qid)
    .fetch_all(pool)
    .await?;
    let mut birth_year = None;
    let mut death_year = None;
    for (event_type, start) in years {
        let year = start.as_deref().and_then(year_from_time_start);
        if event_type == "birth" {
            birth_year = year;
        } else if event_type == "death" {
            death_year = year;
        }
    }

    let published_site_count = sqlx::query_scalar::<_, i64>(
        r#"
        SELECT count(DISTINCT s.id)::bigint
        FROM visit_sites s
        JOIN visit_site_narratives n ON n.site_id = s.id
        WHERE n.subject_qid = $1
          AND s.publication_status = 'published'
        "#,
    )
    .bind(qid)
    .fetch_one(pool)
    .await?;

    Ok(SubjectCoverage {
        label,
        birth_year,
        death_year,
        has_wikipedia: has_wikipedia || wikipedia_documents > 0,
        wikipedia_documents,
        wikisource_count,
        commons_documents,
        sourced_event_count: u32::try_from(sourced_event_count).unwrap_or(u32::MAX),
        published_site_count: u32::try_from(published_site_count).unwrap_or(u32::MAX),
    })
}

pub fn supervisor_input_hash(call: &SupervisorCall) -> String {
    let mut tasks: Vec<&str> = call
        .effects
        .iter()
        .map(|effect| effect.key.as_str())
        .collect();
    if tasks.is_empty() {
        if let Some(items) = call.plan_json.get("tasks").and_then(Value::as_array) {
            tasks = items
                .iter()
                .filter_map(|item| item.get("key").and_then(Value::as_str))
                .collect();
        }
    }
    tasks.sort_unstable();
    tasks.dedup();
    let body = json!({
        "execute": call.execute,
        "experience": call.experience,
        "label": call.subject_label,
        "prompt_version": call.prompt_version,
        "qid": call.subject_qid,
        "tasks": tasks,
    });
    hex::encode(Sha256::digest(
        serde_json::to_vec(&body).expect("hash input serializes"),
    ))
}

pub async fn reusable_supervisor_report(
    pool: &PgPool,
    call: &SupervisorCall,
) -> anyhow::Result<Option<Value>> {
    let hash = supervisor_input_hash(call);
    let run_id = sqlx::query_scalar::<_, Uuid>(
        r#"
        SELECT id FROM supervisor_runs
        WHERE experience = $1 AND prompt_version = $2 AND input_hash = $3
        "#,
    )
    .bind(&call.experience)
    .bind(&call.prompt_version)
    .bind(&hash)
    .fetch_optional(pool)
    .await?;
    let Some(run_id) = run_id else {
        return Ok(None);
    };
    let failed = sqlx::query_scalar::<_, i64>(
        r#"
        SELECT count(*)::bigint
        FROM supervisor_tasks
        WHERE run_id = $1 AND status = 'failed'
        "#,
    )
    .bind(run_id)
    .fetch_one(pool)
    .await?;
    if failed > 0 {
        return Ok(None);
    }
    Ok(Some(
        report(pool, run_id, &hash, true, &call.experience).await?,
    ))
}

pub async fn supervisor_subject_spend_usd_micros(
    pool: &PgPool,
    qid: &str,
    window_days: i64,
) -> anyhow::Result<u64> {
    let spent: i64 = sqlx::query_scalar(
        r#"
        SELECT COALESCE(SUM(cost_usd_micros), 0)::bigint
        FROM supervisor_runs
        WHERE subject_qid = $1
          AND created_at >= NOW() - ($2::int * interval '1 day')
        "#,
    )
    .bind(qid)
    .bind(window_days)
    .fetch_one(pool)
    .await?;
    Ok(u64::try_from(spent).unwrap_or(0))
}

pub async fn record_supervisor_run(pool: &PgPool, call: &SupervisorCall) -> anyhow::Result<Value> {
    let hash = supervisor_input_hash(call);
    let mut tx = pool.begin().await?;
    let inserted = sqlx::query_scalar::<_, Uuid>(
        r#"
        INSERT INTO supervisor_runs (
            experience, mode, subject_qid, subject_label, provider, model,
            prompt_version, input_hash, plan_json, status,
            token_input, token_output, cost_usd_micros, cost_estimated, duration_ms
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15)
        ON CONFLICT (experience, prompt_version, input_hash) DO NOTHING
        RETURNING id
        "#,
    )
    .bind(&call.experience)
    .bind(if call.execute { "execute" } else { "read_only" })
    .bind(&call.subject_qid)
    .bind(&call.subject_label)
    .bind(&call.provider)
    .bind(&call.model)
    .bind(&call.prompt_version)
    .bind(&hash)
    .bind(&call.plan_json)
    .bind(run_status(call))
    .bind(call.input_tokens)
    .bind(call.output_tokens)
    .bind(call.cost_usd_micros)
    .bind(call.cost_estimated)
    .bind(call.duration_ms)
    .fetch_optional(&mut *tx)
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
        .bind(&call.experience)
        .bind(&call.prompt_version)
        .bind(&hash)
        .fetch_one(&mut *tx)
        .await?;
        (id, true)
    };

    if !idempotent {
        for effect in &call.effects {
            apply_effect(&mut tx, run_id, call, effect).await?;
        }
    }
    tx.commit().await?;
    report(pool, run_id, &hash, idempotent, &call.experience).await
}

pub async fn resume_failed_tasks(pool: &PgPool, call: &SupervisorCall) -> anyhow::Result<Value> {
    let hash = supervisor_input_hash(call);
    let run_id = sqlx::query_scalar::<_, Uuid>(
        r#"
        SELECT id FROM supervisor_runs
        WHERE experience = $1 AND prompt_version = $2 AND input_hash = $3
        "#,
    )
    .bind(&call.experience)
    .bind(&call.prompt_version)
    .bind(&hash)
    .fetch_one(pool)
    .await?;
    let mut tx = pool.begin().await?;
    for effect in &call.effects {
        let existing = sqlx::query_as::<_, (Uuid, String)>(
            r#"
            SELECT id, status FROM supervisor_tasks
            WHERE run_id = $1 AND task_key = $2
            "#,
        )
        .bind(run_id)
        .bind(&effect.key)
        .fetch_optional(&mut *tx)
        .await?;
        if let Some((task_id, status)) = existing {
            if status == "failed" {
                sqlx::query(
                    r#"
                    UPDATE supervisor_tasks
                    SET status = $2, writes = $3, detail_json = $4
                    WHERE id = $1
                    "#,
                )
                .bind(task_id)
                .bind(effect.status)
                .bind(effect.writes)
                .bind(&effect.detail)
                .execute(&mut *tx)
                .await?;
                insert_event(&mut tx, task_id, effect).await?;
            }
        }
    }
    tx.commit().await?;
    report(pool, run_id, &hash, true, &call.experience).await
}

pub async fn upsert_supervised_document(
    pool: &PgPool,
    source_type: &str,
    source_uri: &str,
    title: &str,
    language: &str,
    subject_qid: &str,
    revision_id: Option<&str>,
    license: Option<&str>,
    text: &str,
    passages: &Value,
) -> anyhow::Result<(Uuid, bool)> {
    if !verifiable_source_url(source_uri) {
        anyhow::bail!("unverifiable_source_url");
    }
    let row = sqlx::query_as::<_, (Uuid, bool)>(
        r#"
        INSERT INTO raw_documents (
            source_type, source_uri, source_identifier, title, language,
            revision_id, license, payload, metadata
        )
        VALUES (
            $1, $2, $3, $4, $5, $6, $7,
            jsonb_build_object('char_len', $8::int, 'text_sha256', $9::text, 'passages', $10::jsonb),
            jsonb_build_object('subject_qid', $3::text, 'retrieved_at', NOW())
        )
        ON CONFLICT (source_type, source_uri) DO UPDATE
        SET title = EXCLUDED.title,
            revision_id = COALESCE(EXCLUDED.revision_id, raw_documents.revision_id),
            license = COALESCE(EXCLUDED.license, raw_documents.license),
            payload = EXCLUDED.payload,
            metadata = raw_documents.metadata || EXCLUDED.metadata
        RETURNING id, (xmax = 0) AS inserted
        "#,
    )
    .bind(source_type)
    .bind(source_uri)
    .bind(subject_qid)
    .bind(title)
    .bind(language)
    .bind(revision_id)
    .bind(license)
    .bind(text.chars().count() as i32)
    .bind(hex::encode(Sha256::digest(text.as_bytes())))
    .bind(passages)
    .fetch_one(pool)
    .await?;
    Ok(row)
}

pub async fn ensure_subject_entity(pool: &PgPool, qid: &str, label: &str) -> anyhow::Result<Uuid> {
    if let Some(id) = sqlx::query_scalar::<_, Uuid>("SELECT id FROM entities WHERE qid = $1")
        .bind(qid)
        .fetch_optional(pool)
        .await?
    {
        return Ok(id);
    }
    let title = if label.trim().is_empty() {
        qid
    } else {
        label.trim()
    };
    let id = sqlx::query_scalar::<_, Uuid>(
        r#"
        INSERT INTO entities (qid, wikipedia_title, wiki_lang, canonical_name)
        VALUES ($1, $2, 'und', $2)
        ON CONFLICT (wiki_lang, wikipedia_title) DO UPDATE
        SET qid = COALESCE(entities.qid, EXCLUDED.qid)
        RETURNING id
        "#,
    )
    .bind(qid)
    .bind(title)
    .fetch_one(pool)
    .await?;
    Ok(id)
}

async fn apply_effect(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    run_id: Uuid,
    call: &SupervisorCall,
    effect: &TaskEffect,
) -> anyhow::Result<()> {
    let (status, writes, detail, site_id) = if effect.kind == "discover_heritage_sites"
        && effect.status == "completed"
    {
        if call.sourced_site.is_some() {
            match insert_quarantine_site(tx, call).await? {
                SiteInsert::Stored { site_id, wrote } => (
                    "completed",
                    wrote,
                    json!({
                        "publication_status": "quarantine",
                        "visitability": "unknown",
                        "visitability_asserted": false,
                        "site_id": site_id
                    }),
                    Some(site_id),
                ),
                SiteInsert::Refused(reason) => ("refused", false, json!({"reason": reason}), None),
            }
        } else if effect
            .detail
            .get("reason")
            .and_then(Value::as_str)
            .is_some_and(|reason| {
                matches!(
                    reason,
                    "no_documented_place" | "coordinates_missing" | "no_commons_file"
                )
            })
        {
            (effect.status, effect.writes, effect.detail.clone(), None)
        } else {
            (
                "refused",
                false,
                json!({"reason": "source_document_required"}),
                None,
            )
        }
    } else {
        (effect.status, effect.writes, effect.detail.clone(), None)
    };
    let _ = site_id;
    let task_id = sqlx::query_scalar::<_, Uuid>(
        r#"
        INSERT INTO supervisor_tasks (run_id, task_key, kind, status, writes, detail_json)
        VALUES ($1, $2, $3, $4, $5, $6)
        ON CONFLICT (run_id, task_key) DO NOTHING
        RETURNING id
        "#,
    )
    .bind(run_id)
    .bind(&effect.key)
    .bind(&effect.kind)
    .bind(status)
    .bind(writes)
    .bind(&detail)
    .fetch_optional(&mut **tx)
    .await?;
    if let Some(task_id) = task_id {
        insert_event(
            tx,
            task_id,
            &TaskEffect {
                key: effect.key.clone(),
                kind: effect.kind.clone(),
                status,
                writes,
                detail,
            },
        )
        .await?;
    }
    Ok(())
}

enum SiteInsert {
    Stored { site_id: Uuid, wrote: bool },
    Refused(&'static str),
}

async fn insert_quarantine_site(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    call: &SupervisorCall,
) -> anyhow::Result<SiteInsert> {
    let Some(site) = &call.sourced_site else {
        return Ok(SiteInsert::Refused("source_document_required"));
    };
    if !verifiable_source_url(&site.source_url) {
        return Ok(SiteInsert::Refused("unverifiable_source_url"));
    }
    if !valid_qid(&site.qid)
        || site.label.trim().is_empty()
        || !SITE_KINDS.contains(&site.kind.as_str())
    {
        return Ok(SiteInsert::Refused("invalid_site"));
    }
    if !site.lat.is_finite()
        || !site.lon.is_finite()
        || !(-90.0..=90.0).contains(&site.lat)
        || !(-180.0..=180.0).contains(&site.lon)
    {
        return Ok(SiteInsert::Refused("invalid_coordinates"));
    }
    if !relation_matches(&site.relation_pid, &site.relation_role)
        || site.statement_id.trim().is_empty()
    {
        return Ok(SiteInsert::Refused("documented_relation_required"));
    }
    let document_exists =
        sqlx::query_scalar::<_, bool>("SELECT EXISTS(SELECT 1 FROM raw_documents WHERE id = $1)")
            .bind(site.document_id)
            .fetch_one(&mut **tx)
            .await?;
    if !document_exists {
        return Ok(SiteInsert::Refused("source_document_required"));
    }
    let row = sqlx::query_as::<_, (Uuid, String, bool)>(
        r#"
        INSERT INTO visit_sites (
            wikidata_qid, label, kind, geom, publication_status, visitability, retrieved_at
        )
        VALUES (
            $1, $2, $3, ST_SetSRID(ST_MakePoint($4, $5), 4326)::geography,
            'quarantine', 'unknown', $6
        )
        ON CONFLICT (wikidata_qid) DO UPDATE
        SET label = CASE
                WHEN visit_sites.publication_status = 'quarantine' THEN EXCLUDED.label
                ELSE visit_sites.label
            END,
            updated_at = CASE
                WHEN visit_sites.publication_status = 'quarantine' THEN NOW()
                ELSE visit_sites.updated_at
            END
        RETURNING id, publication_status, (xmax = 0) AS inserted
        "#,
    )
    .bind(&site.qid)
    .bind(site.label.trim())
    .bind(&site.kind)
    .bind(site.lon)
    .bind(site.lat)
    .bind(site.retrieved_at)
    .fetch_one(&mut **tx)
    .await?;
    if row.1 != "quarantine" {
        return Ok(SiteInsert::Refused("existing_site_not_republished"));
    }
    sqlx::query(
        r#"
        INSERT INTO visit_site_narratives (
            site_id, subject_qid, relation_role, relation_pid, summary,
            source_url, retrieved_at, wikidata_revision_id, publication_status
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, 'quarantine')
        ON CONFLICT (site_id, subject_qid, relation_role, source_url) DO NOTHING
        "#,
    )
    .bind(row.0)
    .bind(&call.subject_qid)
    .bind(&site.relation_role)
    .bind(&site.relation_pid)
    .bind(format!(
        "Wikidata statement {} ({}) links the subject to this place. The relation does not show that the place can be visited.",
        site.statement_id, site.relation_pid
    ))
    .bind(&site.source_url)
    .bind(site.retrieved_at)
    .bind(site.revision_id)
    .execute(&mut **tx)
    .await?;
    Ok(SiteInsert::Stored {
        site_id: row.0,
        wrote: true,
    })
}

async fn insert_event(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    task_id: Uuid,
    effect: &TaskEffect,
) -> anyhow::Result<()> {
    sqlx::query(
        r#"
        INSERT INTO supervisor_task_events (task_id, status, writes, detail_json)
        VALUES ($1, $2, $3, $4)
        "#,
    )
    .bind(task_id)
    .bind(effect.status)
    .bind(effect.writes)
    .bind(&effect.detail)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

async fn report(
    pool: &PgPool,
    run_id: Uuid,
    hash: &str,
    idempotent: bool,
    experience: &str,
) -> anyhow::Result<Value> {
    let canonical = sqlx::query_scalar::<_, i64>("SELECT count(*)::bigint FROM canonical_events")
        .fetch_one(pool)
        .await?;
    let published = sqlx::query_scalar::<_, i64>(
        "SELECT count(*)::bigint FROM visit_sites WHERE publication_status = 'published'",
    )
    .fetch_one(pool)
    .await?;
    let usage = sqlx::query_as::<_, (i32, i32, i32, bool, i32)>(
        r#"
        SELECT token_input, token_output, cost_usd_micros, cost_estimated, duration_ms
        FROM supervisor_runs WHERE id = $1
        "#,
    )
    .bind(run_id)
    .fetch_one(pool)
    .await?;
    let tasks = sqlx::query_as::<_, (String, String, bool)>(
        "SELECT kind, status, writes FROM supervisor_tasks WHERE run_id = $1 ORDER BY task_key",
    )
    .bind(run_id)
    .fetch_all(pool)
    .await?;
    Ok(json!({
        "run_id": run_id,
        "input_hash": hash,
        "idempotent": idempotent,
        "experience": experience,
        "publication": "forbidden",
        "canonical_events": canonical,
        "published_sites": published,
        "usage": {
            "input_tokens": usage.0,
            "output_tokens": usage.1,
            "cost_usd_micros": usage.2,
            "estimated": usage.3,
            "duration_ms": usage.4
        },
        "tasks": tasks.into_iter().map(|(kind, status, writes)| json!({
            "kind": kind,
            "status": status,
            "writes": writes
        })).collect::<Vec<_>>()
    }))
}

fn run_status(call: &SupervisorCall) -> &'static str {
    if !call.execute {
        "planned"
    } else if call.effects.iter().any(|effect| effect.status == "failed") {
        "failed"
    } else if call.effects.iter().all(|effect| effect.status == "refused") {
        "refused"
    } else {
        "completed"
    }
}

fn relation_matches(pid: &str, role: &str) -> bool {
    matches!(
        (pid, role),
        ("P119", "burial")
            | ("P138", "named_after")
            | ("P547", "commemoration")
            | ("P551", "residence")
    )
}

pub fn verifiable_source_url(url: &str) -> bool {
    SOURCE_HOSTS.iter().any(|prefix| url.starts_with(prefix)) && !url.contains(' ')
}

fn valid_qid(qid: &str) -> bool {
    let Some(rest) = qid.strip_prefix('Q') else {
        return false;
    };
    !rest.is_empty() && !rest.starts_with('0') && rest.bytes().all(|b| b.is_ascii_digit())
}

fn year_from_time_start(start: &str) -> Option<i32> {
    let trimmed = start.trim();
    let (negative, rest) = if let Some(rest) = trimmed.strip_prefix('-') {
        (true, rest)
    } else if let Some(rest) = trimmed.strip_prefix('+') {
        (false, rest)
    } else {
        (false, trimmed)
    };
    let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
    if digits.len() < 3 {
        return None;
    }
    let year: i32 = digits.parse().ok()?;
    Some(if negative { -year } else { year })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn effect(kind: &str, status: &'static str, writes: bool) -> TaskEffect {
        TaskEffect {
            key: format!("{kind}:Q5582"),
            kind: kind.into(),
            status,
            writes,
            detail: json!({"reason": status}),
        }
    }

    fn call(effects: Vec<TaskEffect>, site: Option<SourcedSite>) -> SupervisorCall {
        SupervisorCall {
            experience: "poreia".into(),
            subject_qid: "Q5582".into(),
            subject_label: "Vincent van Gogh".into(),
            provider: "deterministic".into(),
            model: "none".into(),
            prompt_version: "supervisor:v2".into(),
            plan_json: json!({"tasks": effects.iter().map(|item| json!({"key": item.key})).collect::<Vec<_>>()}),
            execute: true,
            input_tokens: 0,
            output_tokens: 0,
            cost_usd_micros: 0,
            cost_estimated: false,
            duration_ms: 4,
            effects,
            sourced_site: site,
        }
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn read_only_run_is_idempotent_and_stores_coverage(pool: PgPool) {
        let coverage = load_subject_coverage(&pool, "Q517").await.unwrap();
        assert_eq!(coverage.sourced_event_count, 0);
        assert_eq!(coverage.published_site_count, 0);
        let mut request = call(
            vec![effect("flag_unverified_visit_facts", "queued", false)],
            None,
        );
        request.execute = false;
        let first = record_supervisor_run(&pool, &request).await.unwrap();
        let second = record_supervisor_run(&pool, &request).await.unwrap();
        assert_eq!(first["idempotent"], false);
        assert_eq!(second["idempotent"], true);
        assert_eq!(first["run_id"], second["run_id"]);
        assert_eq!(first["canonical_events"], 0);
        let events =
            sqlx::query_scalar::<_, i64>("SELECT count(*)::bigint FROM supervisor_task_events")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(events, 1);
        assert!(sqlx::query("DELETE FROM supervisor_task_events")
            .execute(&pool)
            .await
            .is_err());
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn a_new_detector_version_keeps_the_previous_report(pool: PgPool) {
        fn review(version: &str) -> SupervisorCall {
            let key = format!("review_historical_consistency:Q517:{version}:same-evidence");
            let mut request = call(
                vec![effect("review_historical_consistency", "completed", false)],
                None,
            );
            request.experience = "istoria".into();
            request.subject_qid = "Q517".into();
            request.subject_label = "Napoleon".into();
            request.execute = false;
            request.effects[0].key = key.clone();
            request.plan_json = json!({"tasks":[{"key": key}]});
            request
        }
        let older = review("place-role:v2");
        let newer = review("place-role:v3");
        assert_ne!(supervisor_input_hash(&older), supervisor_input_hash(&newer));
        let first = record_supervisor_run(&pool, &older).await.unwrap();
        let second = record_supervisor_run(&pool, &newer).await.unwrap();
        assert_ne!(first["run_id"], second["run_id"]);
        let again = record_supervisor_run(&pool, &older).await.unwrap();
        assert_eq!(again["run_id"], first["run_id"]);
        assert_eq!(again["idempotent"], true);
        let kept = reusable_supervisor_report(&pool, &older).await.unwrap();
        assert!(kept.is_some());
        let runs = sqlx::query_scalar::<_, i64>(
            "SELECT count(*)::bigint FROM supervisor_runs WHERE subject_qid = 'Q517'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(runs, 2);
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn a_site_without_a_source_document_is_refused(pool: PgPool) {
        let request = call(
            vec![effect("discover_heritage_sites", "completed", false)],
            None,
        );
        let report = record_supervisor_run(&pool, &request).await.unwrap();
        let refused = report["tasks"].as_array().unwrap().iter().any(|task| {
            task["kind"] == "discover_heritage_sites"
                && task["status"] == "refused"
                && task["writes"] == false
        });
        assert!(refused);
        let sites = sqlx::query_scalar::<_, i64>("SELECT count(*)::bigint FROM visit_sites")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(sites, 0);
        assert_eq!(report["published_sites"], 0);
        assert_eq!(report["canonical_events"], 0);
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn a_documented_site_stays_in_quarantine_and_records_the_write(pool: PgPool) {
        let (document_id, inserted) = upsert_supervised_document(
            &pool,
            "wikidata",
            "https://www.wikidata.org/wiki/Q5582",
            "Vincent van Gogh",
            "en",
            "Q5582",
            Some("99"),
            Some("CC0"),
            "P119 Q170481",
            &json!([]),
        )
        .await
        .unwrap();
        assert!(inserted);
        let site = SourcedSite {
            qid: "Q170481".into(),
            label: "Les Invalides".into(),
            kind: "burial_site".into(),
            lat: 48.855,
            lon: 2.313,
            source_url: "https://www.wikidata.org/wiki/Q5582".into(),
            retrieved_at: Utc::now(),
            revision_id: Some(99),
            relation_role: "burial".into(),
            relation_pid: "P119".into(),
            statement_id: "Q5582$burial".into(),
            document_id,
        };
        let before = sqlx::query_scalar::<_, i64>("SELECT count(*)::bigint FROM canonical_events")
            .fetch_one(&pool)
            .await
            .unwrap();
        let report = record_supervisor_run(
            &pool,
            &call(
                vec![effect("discover_heritage_sites", "completed", false)],
                Some(site),
            ),
        )
        .await
        .unwrap();
        assert_eq!(report["canonical_events"], before);
        assert_eq!(report["published_sites"], 0);
        let task = &report["tasks"][0];
        assert_eq!(task["status"], "completed");
        assert_eq!(task["writes"], true);
        let status = sqlx::query_scalar::<_, String>(
            "SELECT publication_status FROM visit_sites WHERE wikidata_qid = 'Q170481'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(status, "quarantine");
        let visitability = sqlx::query_scalar::<_, String>(
            "SELECT visitability FROM visit_sites WHERE wikidata_qid = 'Q170481'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(visitability, "unknown");
        let summary = sqlx::query_scalar::<_, String>(
            "SELECT summary FROM visit_site_narratives WHERE relation_pid = 'P119'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert!(summary.contains("does not show that the place can be visited"));
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn a_failed_task_can_be_resumed_without_a_second_run(pool: PgPool) {
        let failed = call(vec![effect("harvest_wikipedia", "failed", false)], None);
        let first = record_supervisor_run(&pool, &failed).await.unwrap();
        let mut resumed = failed.clone();
        resumed.effects[0].status = "completed";
        resumed.effects[0].writes = true;
        resumed.effects[0].detail = json!({"documents": 1});
        let second = resume_failed_tasks(&pool, &resumed).await.unwrap();
        assert_eq!(first["run_id"], second["run_id"]);
        assert_eq!(second["tasks"][0]["status"], "completed");
        assert_eq!(second["tasks"][0]["writes"], true);
        let events =
            sqlx::query_scalar::<_, i64>("SELECT count(*)::bigint FROM supervisor_task_events")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(events, 2);
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn a_critic_report_is_stored_without_a_canonical_event(pool: PgPool) {
        let mut sample = call(
            vec![effect("review_historical_consistency", "completed", false)],
            None,
        );
        sample.experience = "istoria".into();
        sample.effects[0].detail = json!({
            "origin": "historical_critic:v1",
            "canonical_write": false,
            "integration": "review_historical_record",
            "findings": [{"event_id": "valmy", "status": "proposal", "canonical_write": false}]
        });
        let stored = record_supervisor_run(&pool, &sample).await.unwrap();
        assert_eq!(stored["tasks"][0]["writes"], false);
        let detail = sqlx::query_scalar::<_, serde_json::Value>(
            "SELECT detail_json FROM supervisor_tasks WHERE kind = 'review_historical_consistency'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(detail["origin"], "historical_critic:v1");
        assert_eq!(detail["canonical_write"], false);
        let canonical =
            sqlx::query_scalar::<_, i64>("SELECT count(*)::bigint FROM canonical_events")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(canonical, 0);
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn qid_spend_outside_the_window_does_not_block_a_later_review(pool: PgPool) {
        sqlx::query(
            r#"
            INSERT INTO supervisor_runs (
                experience, mode, subject_qid, subject_label, provider, model,
                prompt_version, input_hash, plan_json, status, cost_usd_micros, created_at
            ) VALUES
            ('istoria', 'read_only', 'Q900001', 'Critic Subject', 'deterministic', 'none',
             'supervisor:v2', $1, '{}'::jsonb, 'completed', 500000, NOW() - interval '40 days'),
            ('istoria', 'read_only', 'Q900001', 'Critic Subject', 'deterministic', 'none',
             'supervisor:v2', $2, '{}'::jsonb, 'completed', 1000, NOW())
            "#,
        )
        .bind("a".repeat(64))
        .bind("b".repeat(64))
        .execute(&pool)
        .await
        .unwrap();
        let spent = supervisor_subject_spend_usd_micros(&pool, "Q900001", 30)
            .await
            .unwrap();
        assert_eq!(spent, 1000);
    }

    #[test]
    fn an_http_url_is_not_a_verifiable_source() {
        assert!(!verifiable_source_url("http://www.wikidata.org/wiki/Q517"));
        assert!(verifiable_source_url(
            "https://fr.wikisource.org/wiki/Napol%C3%A9on"
        ));
    }
}
